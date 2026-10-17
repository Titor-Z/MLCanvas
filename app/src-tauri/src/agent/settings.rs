//! 模型接入配置：~/YuCanvas/.settings.json（仿 .index.json 的读写模式）。
//! 支持多份配置（厂商 + 模型），其中一份为「当前使用」。
//! 密钥是本机明文存储（本地单用户应用的取舍），文件权限收紧到 0600。

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::path::PathBuf;

/// 单份配置展开成协议适配器需要的字段。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentSettings {
    /// "openai"（OpenAI 兼容）| "anthropic"
    pub protocol: String,
    /// 接口地址（OpenAI 兼容含 /v1 等版本段；Anthropic 可空 = 官方地址）
    pub base_url: String,
    pub api_key: String,
    pub model: String,
}

impl AgentSettings {
    pub fn validate(&self) -> Result<(), String> {
        if self.protocol != "openai" && self.protocol != "anthropic" {
            return Err(format!("未知的服务类型: {}", self.protocol));
        }
        if self.model.trim().is_empty() {
            return Err("模型名称不能为空".to_string());
        }
        if self.protocol == "openai" && self.base_url.trim().is_empty() {
            return Err("接口地址不能为空".to_string());
        }
        if !self.base_url.trim().is_empty()
            && !self.base_url.starts_with("http://")
            && !self.base_url.starts_with("https://")
        {
            return Err("接口地址应以 http:// 或 https:// 开头".to_string());
        }
        Ok(())
    }
}

/// 一份模型配置（服务商展示名 + 协议适配器字段）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigEntry {
    pub id: String,
    /// 展示用服务商名（DeepSeek / 智谱 GLM / 自定义…）
    #[serde(default)]
    pub provider: String,
    pub protocol: String,
    pub base_url: String,
    pub api_key: String,
    pub model: String,
}

impl ConfigEntry {
    pub fn wire(&self) -> AgentSettings {
        AgentSettings {
            protocol: self.protocol.clone(),
            base_url: self.base_url.clone(),
            api_key: self.api_key.clone(),
            model: self.model.clone(),
        }
    }

    /// 已配置 = 够发一次请求的最小信息（本地服务允许空密钥）
    pub fn is_configured(&self) -> bool {
        !self.model.trim().is_empty()
            && (self.protocol == "anthropic" || !self.base_url.trim().is_empty())
    }

    pub fn validate(&self) -> Result<(), String> {
        self.wire().validate()
    }

    fn from_wire(w: AgentSettings, provider: impl Into<String>) -> Self {
        ConfigEntry {
            id: new_id("cfg"),
            provider: provider.into(),
            protocol: w.protocol,
            base_url: w.base_url,
            api_key: w.api_key,
            model: w.model,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SettingsStore {
    /// 当前使用配置的 id；空或未命中时回退第一份
    #[serde(default)]
    pub current: String,
    #[serde(default)]
    pub configs: Vec<ConfigEntry>,
    /// 新建项目默认父目录（空 = ~/YuCanvas）
    #[serde(default)]
    pub default_parent: String,
}

impl SettingsStore {
    /// 当前生效的配置（current 未命中回退第一份）
    pub fn active(&self) -> Option<&ConfigEntry> {
        self.configs
            .iter()
            .find(|c| c.id == self.current)
            .or_else(|| self.configs.first())
    }

    pub fn is_configured(&self) -> bool {
        self.active().map(ConfigEntry::is_configured).unwrap_or(false)
    }
}

fn new_id(prefix: &str) -> String {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    format!("{prefix}-{nanos:x}")
}

fn settings_file() -> PathBuf {
    crate::state::projects_root().join(".settings.json")
}

/// 新配置 id：带服务商 slug 前缀，便于辨认（时间戳保证唯一）。
pub fn new_config_id(provider: &str) -> String {
    let slug: String = provider
        .chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .collect::<String>()
        .to_lowercase();
    if slug.is_empty() {
        new_id("cfg")
    } else {
        new_id(&slug)
    }
}

/// 把旧的单配置结构 `{protocol,base_url,api_key,model}` 迁移成多配置仓库。
fn migrate(value: &Value) -> SettingsStore {
    let mut store = SettingsStore::default();
    if let Ok(w) = serde_json::from_value::<AgentSettings>(value.clone()) {
        if !w.model.trim().is_empty() || !w.base_url.trim().is_empty() {
            let entry = ConfigEntry::from_wire(w, "自定义");
            store.current = entry.id.clone();
            store.configs.push(entry);
        }
    }
    store
}

pub fn load() -> SettingsStore {
    let Some(text) = std::fs::read_to_string(settings_file()).ok() else {
        return SettingsStore::default();
    };
    let Ok(value) = serde_json::from_str::<Value>(&text) else {
        return SettingsStore::default();
    };
    if value.get("configs").is_some() {
        serde_json::from_value(value).unwrap_or_default()
    } else {
        migrate(&value)
    }
}

pub fn save(store: &SettingsStore) -> Result<(), String> {
    let _ = std::fs::create_dir_all(crate::state::projects_root());
    let json = serde_json::to_string_pretty(store).map_err(|e| e.to_string())?;
    let file = settings_file();
    std::fs::write(&file, json).map_err(|e| format!("写入模型配置失败: {e}"))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o600));
    }
    Ok(())
}

/// 新建项目的默认父目录（空 → ~/YuCanvas）。
pub fn default_parent() -> PathBuf {
    let raw = load().default_parent;
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        crate::state::projects_root()
    } else {
        let expanded = if let Some(rest) = trimmed.strip_prefix("~/") {
            crate::state::home_dir().join(rest)
        } else {
            PathBuf::from(trimmed)
        };
        expanded
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validate_rejects_bad_config() {
        let mut cfg = AgentSettings {
            protocol: "openai".into(),
            base_url: String::new(),
            api_key: String::new(),
            model: String::new(),
        };
        assert!(cfg.validate().is_err());
        cfg.model = "deepseek-chat".to_string();
        cfg.base_url = "https://api.deepseek.com/v1".to_string();
        assert!(cfg.validate().is_ok());
        cfg.base_url = "ftp://x".to_string();
        assert!(cfg.validate().is_err());
    }

    #[test]
    fn migrate_single_config() {
        let old = serde_json::json!({
            "protocol": "openai",
            "base_url": "https://api.deepseek.com/v1",
            "api_key": "sk-x",
            "model": "deepseek-chat"
        });
        let store = migrate(&old);
        assert_eq!(store.configs.len(), 1);
        assert_eq!(store.current, store.configs[0].id);
        assert!(store.is_configured());
    }

    #[test]
    fn active_falls_back_to_first() {
        let store = SettingsStore {
            current: "missing".into(),
            configs: vec![ConfigEntry {
                id: "a".into(),
                provider: "P".into(),
                protocol: "openai".into(),
                base_url: "https://x/v1".into(),
                api_key: String::new(),
                model: "m".into(),
            }],
            default_parent: String::new(),
        };
        assert!(store.active().is_some());
    }
}
