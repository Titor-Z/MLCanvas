//! 项目数据层：一个项目 = ~/YuCanvas/<名称>/artboards/ 下的画板集合。

use std::path::PathBuf;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicUsize};

/// 项目索引：应用维护的项目清单（删除 = 移除索引记录，文件夹原地不动）
const INDEX_FILE: &str = ".index.json";

#[derive(serde::Serialize, serde::Deserialize, Clone)]
pub struct IndexEntry {
    pub name: String,
    pub path: String,
}

pub fn index_file() -> PathBuf {
    projects_root().join(INDEX_FILE)
}

pub fn load_index() -> Vec<IndexEntry> {
    let text = match std::fs::read_to_string(index_file()) {
        Ok(t) => t,
        Err(_) => return Vec::new(),
    };
    serde_json::from_str::<Vec<IndexEntry>>(&text).unwrap_or_default()
}

pub fn save_index(entries: &[IndexEntry]) -> Result<(), String> {
    let _ = std::fs::create_dir_all(projects_root());
    let json = serde_json::to_string_pretty(entries).map_err(|e| e.to_string())?;
    std::fs::write(index_file(), json).map_err(|e| format!("写入项目索引失败: {e}"))
}

/// 把一个项目加入索引（已存在则更新路径）
pub fn index_add(entries: &mut Vec<IndexEntry>, name: &str, path: &str) {
    if let Some(e) = entries.iter_mut().find(|e| e.name == name) {
        e.path = path.to_string();
    } else {
        entries.push(IndexEntry { name: name.to_string(), path: path.to_string() });
    }
}

pub struct AppState {
    pub active_project: Mutex<String>,
    /// 进行中的 Agent 桥调用数：0 = 空闲，>0 = 处理中
    pub agent_busy: AtomicUsize,
    /// 用户本次启动后是否显式打开过项目（新建/点选都算）。MCP 工具以此为会话门槛
    pub project_opened: AtomicBool,
    /// 最近一次接入的 MCP 客户端身份（initialize 的 clientInfo.name），空 = 尚无客户端接入
    pub agent_client: Mutex<String>,
}

/// 用户主目录：macOS/Linux 用 HOME，Windows 用 USERPROFILE
pub fn home_dir() -> PathBuf {
    std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .map(PathBuf::from)
        .unwrap_or_default()
}

/// 项目根目录：~/YuCanvas
pub fn projects_root() -> PathBuf {
    home_dir().join("YuCanvas")
}

/// 一次性迁移：产品改名 MLCanvas → YuCanvas，数据根目录 ~/MLCanvas 搬往 ~/YuCanvas。
/// 幂等：旧根不存在直接返回；旧根整体搬得动就 rename，搬不动（跨卷等）退回逐项搬移；
/// 索引里指向旧根的绝对路径同步改写。任何失败只记日志，不阻断启动。
fn migrate_legacy_root() {
    let home = home_dir();
    let old_root = home.join("MLCanvas");
    let new_root = home.join("YuCanvas");
    if !old_root.is_dir() {
        return;
    }
    eprintln!("[mlcanvas] 迁移数据目录 {} -> {}", old_root.display(), new_root.display());
    if !new_root.is_dir() {
        if std::fs::rename(&old_root, &new_root).is_ok() {
            rewrite_index_paths(&new_root, &old_root, &new_root);
            return;
        }
        let _ = std::fs::create_dir_all(&new_root);
    }
    // 旧根仍在：把新根缺少的条目（项目目录与隐藏文件）逐个搬过去
    if let Ok(entries) = std::fs::read_dir(&old_root) {
        for entry in entries.flatten() {
            let from = entry.path();
            let Some(name) = from.file_name() else { continue };
            let to = new_root.join(name);
            if to.exists() {
                continue;
            }
            if from.is_dir() {
                let _ = std::fs::rename(&from, &to);
            } else {
                let _ = std::fs::copy(&from, &to);
            }
        }
    }
    rewrite_index_paths(&new_root, &old_root, &new_root);
    // 搬空即删旧根；删不掉（仍有占用）就留着，不影响运行
    if let Ok(entries) = std::fs::read_dir(&old_root) {
        if entries.count() == 0 {
            let _ = std::fs::remove_dir(&old_root);
        }
    }
}

/// 把 <root>/.index.json 里以 <old_root>/ 为前缀的项目路径改写为 <new_root>/ 前缀
fn rewrite_index_paths(root: &std::path::Path, old_root: &std::path::Path, new_root: &std::path::Path) {
    let index = root.join(INDEX_FILE);
    let Ok(text) = std::fs::read_to_string(&index) else { return };
    let Ok(mut entries) = serde_json::from_str::<Vec<IndexEntry>>(&text) else { return };
    let old_prefix = format!("{}/", old_root.display());
    let new_prefix = format!("{}/", new_root.display());
    let mut changed = false;
    for e in entries.iter_mut() {
        if let Some(rest) = e.path.strip_prefix(&old_prefix) {
            e.path = format!("{new_prefix}{rest}");
            changed = true;
        }
    }
    if changed {
        if let Ok(json) = serde_json::to_string_pretty(&entries) {
            let _ = std::fs::write(&index, json);
        }
    }
}

/// 接入登记：MCP initialize 的 clientInfo 落盘，应用重启后状态不丢
/// （客户端只在它自己的会话启动时发一次 initialize，不会因我们重启而重发）
const PRESENCE_FILE: &str = ".presence.json";
const PRESENCE_TTL_MS: u128 = 24 * 60 * 60 * 1000;

fn presence_file() -> PathBuf {
    projects_root().join(PRESENCE_FILE)
}

/// 载回最近接入的客户端名（超过 24h 视为过期返回空）
pub fn load_presence() -> String {
    let Ok(raw) = std::fs::read_to_string(presence_file()) else {
        return String::new();
    };
    let Ok(value) = serde_json::from_str::<serde_json::Value>(&raw) else {
        return String::new();
    };
    let Some(name) = value.get("client").and_then(|v| v.as_str()) else {
        return String::new();
    };
    let seen = value.get("lastSeen").and_then(|v| v.as_u64()).unwrap_or(0) as u128;
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    if now.saturating_sub(seen) > PRESENCE_TTL_MS {
        return String::new();
    }
    name.to_string()
}

pub fn save_presence(client: &str) {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    let body = serde_json::json!({ "client": client, "lastSeen": now });
    let _ = std::fs::create_dir_all(projects_root());
    let _ = std::fs::write(presence_file(), body.to_string());
}

pub fn project_dir(name: &str) -> PathBuf {
    crate::agent::default_parent().join(name)
}

/// 项目路径：优先项目索引（支持任意目录的项目），索引未收录时回退 ~/YuCanvas/<名称>
pub fn project_path(name: &str) -> PathBuf {
    load_index()
        .iter()
        .find(|e| e.name == name)
        .map(|e| PathBuf::from(&e.path))
        .unwrap_or_else(|| project_dir(name))
}

pub fn artboards_dir_of(name: &str) -> PathBuf {
    project_path(name).join("artboards")
}

pub fn validate_project_name(name: &str) -> Result<String, String> {
    let name = name.trim();
    if name.is_empty() {
        return Err("项目名称不能为空".to_string());
    }
    if name.len() > 50 || name.contains('/') || name.contains('\\') || name.contains("..") {
        return Err(format!("非法项目名称: {name}"));
    }
    Ok(name.to_string())
}

/// 兜底取名：优先示例项目，否则取第一个含 artboards 的项目，没有则空串
pub fn setup_default_name() -> String {
    let default_name = "示例项目";
    if project_dir(default_name).is_dir() {
        return default_name.to_string();
    }
    for entry in std::fs::read_dir(projects_root()).into_iter().flatten().flatten() {
        let path = entry.path();
        if path.is_dir() && path.join("artboards").is_dir() {
            if let Some(name) = path.file_name() {
                return name.to_string_lossy().to_string();
            }
        }
    }
    String::new()
}

/// 首次启动：先做旧数据根目录改名迁移；建根目录；把旧版仓库 artboards/ 的画板迁移进
/// 默认项目「示例项目」；索引文件缺失时，把磁盘上现有项目全部登记进索引（向后兼容）
pub fn setup_default() -> String {
    migrate_legacy_root();

    let root = projects_root();
    let _ = std::fs::create_dir_all(&root);

    let default_name = "示例项目";
    let default_artboards = artboards_dir_of(default_name);
    if !default_artboards.is_dir() {
        let _ = std::fs::create_dir_all(&default_artboards);
        let legacy = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(|p| p.parent())
            .map(|p| p.join("artboards"))
            .unwrap_or_default();
        if legacy.is_dir() {
            for entry in std::fs::read_dir(&legacy).into_iter().flatten() {
                let Ok(entry) = entry else { continue };
                let path = entry.path();
                if path.extension().and_then(|e| e.to_str()) == Some("html") {
                    let Some(name) = path.file_name() else { continue };
                    let _ = std::fs::copy(&path, default_artboards.join(name));
                }
            }
        }
    }

    if !index_file().exists() {
        let mut entries = Vec::new();
        for entry in std::fs::read_dir(&root).into_iter().flatten().flatten() {
            let path = entry.path();
            if path.is_dir() && path.join("artboards").is_dir() {
                if let Some(name) = path.file_name() {
                    entries.push(IndexEntry {
                        name: name.to_string_lossy().to_string(),
                        path: path.to_string_lossy().to_string(),
                    });
                }
            }
        }
        let _ = save_index(&entries);
    }

    let fallback = setup_default_name();
    if !fallback.is_empty() {
        return fallback;
    }
    let _ = std::fs::create_dir_all(&default_artboards);
    default_name.to_string()
}
