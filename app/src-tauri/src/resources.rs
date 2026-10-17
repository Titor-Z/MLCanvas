//! 内置参考库（skills / design-systems）的定位。
//! 开发时读源码树 resources/；打包后读应用资源目录（tauri.conf 的 bundle.resources 映射）。

use std::path::PathBuf;
use std::sync::OnceLock;

static RESOURCE_ROOT: OnceLock<PathBuf> = OnceLock::new();

/// 启动时调用一次：记录打包后的资源根目录（不存在时保持未设置 → 回退源码树）
pub fn init(resource_dir: Option<PathBuf>) {
    if let Some(dir) = resource_dir.filter(|p| p.is_dir()) {
        let _ = RESOURCE_ROOT.set(dir);
    }
}

/// 返回 resources/<rel>：打包后优先，开发时回退 CARGO_MANIFEST_DIR/resources
pub fn dir(rel: &str) -> PathBuf {
    if let Some(root) = RESOURCE_ROOT.get() {
        let bundled = root.join(rel);
        if bundled.is_dir() {
            return bundled;
        }
    }
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources").join(rel)
}
