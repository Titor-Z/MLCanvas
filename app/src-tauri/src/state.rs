//! 项目数据层：一个项目 = ~/MLCanvas/<名称>/artboards/ 下的画板集合。

use std::path::PathBuf;
use std::sync::Mutex;

pub struct AppState {
    pub active_project: Mutex<String>,
}

/// 项目根目录：~/MLCanvas
pub fn projects_root() -> PathBuf {
    PathBuf::from(std::env::var("HOME").unwrap_or_default()).join("MLCanvas")
}

pub fn project_dir(name: &str) -> PathBuf {
    projects_root().join(name)
}

pub fn artboards_dir_of(name: &str) -> PathBuf {
    project_dir(name).join("artboards")
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

/// 首次启动：建根目录；把旧版根目录 artboards/ 的画板迁移进默认项目「示例项目」
pub fn setup_default() -> String {
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

    if project_dir(default_name).is_dir() {
        return default_name.to_string();
    }
    // 兜底：取根目录下第一个含 artboards 的项目
    for entry in std::fs::read_dir(&root).into_iter().flatten() {
        let Ok(entry) = entry else { continue };
        let path = entry.path();
        if path.is_dir() && path.join("artboards").is_dir() {
            if let Some(name) = path.file_name() {
                return name.to_string_lossy().to_string();
            }
        }
    }
    let _ = std::fs::create_dir_all(&default_artboards);
    default_name.to_string()
}
