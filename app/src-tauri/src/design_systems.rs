//! OpenDesign 风格包参考库（resources/opendesign/design-systems，vendor 落盘）。
//! 列表只读 vendor 期生成的 index.json（152 包一次读入，进程内驻留）；
//! 详情与大窗按需读单包文件。不预载任何包内容。

use serde_json::{json, Value};
use std::path::PathBuf;
use std::sync::OnceLock;

fn ds_dir() -> Result<PathBuf, String> {
    let dir = crate::resources::dir("opendesign/design-systems");
    if dir.is_dir() {
        Ok(dir)
    } else {
        Err("风格包库未就位：请在开发机运行 ./dev.sh vendor".to_string())
    }
}

/// index.json 进程内驻留：面板开合与 MCP 调用都走这一份
fn index() -> Result<&'static Value, String> {
    static INDEX: OnceLock<Option<Value>> = OnceLock::new();
    let cached = INDEX.get_or_init(|| {
        let path = ds_dir().ok()?.join("index.json");
        let text = std::fs::read_to_string(path).ok()?;
        serde_json::from_str(&text).ok()
    });
    cached
        .as_ref()
        .ok_or_else(|| "风格包索引缺失：请重新运行 ./dev.sh vendor".to_string())
}

/// 包名只允许小写 slug，防路径穿越
fn safe_slug(name: &str) -> Result<String, String> {
    let ok = !name.is_empty()
        && name.len() <= 80
        && name
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-');
    if ok {
        Ok(name.to_string())
    } else {
        Err(format!("非法的包名: {name}"))
    }
}

/// 包内文件名：白名单后缀，禁止目录穿越
fn safe_file(file: &str) -> Result<String, String> {
    let ok = !file.contains("..")
        && !file.starts_with('/')
        && matches!(
            file.rsplit('.').next(),
            Some("html") | Some("css") | Some("md") | Some("json")
        );
    if ok {
        Ok(file.to_string())
    } else {
        Err(format!("非法的文件名: {file}"))
    }
}

/// 市场列表：vendor 期索引按分类排序返回（name/category/description/封面色/有无组件页）
pub fn list() -> Result<Value, String> {
    let idx = index()?;
    let mut packs: Vec<Value> = idx["packs"].as_array().cloned().unwrap_or_default();
    packs.sort_by(|a, b| {
        a["category"]
            .as_str()
            .cmp(&b["category"].as_str())
            .then_with(|| a["id"].as_str().cmp(&b["id"].as_str()))
    });
    Ok(json!({ "total": idx["total"], "packs": packs }))
}

/// 读单包文件；file 省略返回 manifest.json。UI 详情读 tokens.css，大窗读组件页
pub fn get(args: &Value) -> Result<Value, String> {
    let dir = ds_dir()?;
    let name = args
        .get("name")
        .and_then(|v| v.as_str())
        .ok_or("缺少参数 name")?;
    let slug = safe_slug(name)?;
    let file = match args.get("file").and_then(|v| v.as_str()) {
        Some(f) => safe_file(f)?,
        None => "manifest.json".to_string(),
    };
    let path = dir.join(&slug).join(&file);
    if !path.is_file() {
        return Err(format!("风格包 {name} 里没有文件 {file}"));
    }
    let content = std::fs::read_to_string(&path).map_err(|e| format!("读取失败: {e}"))?;
    Ok(json!({ "name": slug, "file": file, "content": content }))
}
