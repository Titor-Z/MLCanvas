//! OpenDesign skill 参考库（resources/opendesign/skills，./dev.sh vendor 落盘）。
//! 只读参考：list_skills 解析各技能 SKILL.md 的 frontmatter 做目录，
//! get_skill 按需返回单个文件全文。不做启动预载，零常驻内存。

use serde_json::{json, Value};
use std::path::PathBuf;

/// SKILL.md 小于这个尺寸视为「指针卡」（只有描述 + 上游链接，无正文）
const STUB_BYTES: u64 = 2048;

fn skills_dir() -> Result<PathBuf, String> {
    let dir = crate::resources::dir("opendesign/skills");
    if dir.is_dir() {
        Ok(dir)
    } else {
        Err("技能库未就位：请在开发机运行 ./dev.sh vendor".to_string())
    }
}

/// skill 名只允许小写 slug，防路径穿越
fn safe_slug(name: &str) -> Result<String, String> {
    let ok = !name.is_empty()
        && name.len() <= 80
        && name
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-');
    if ok {
        Ok(name.to_string())
    } else {
        Err(format!("非法的技能名: {name}"))
    }
}

/// 技能内文件名：同仓约束，禁止目录穿越
fn safe_file(file: &str) -> Result<String, String> {
    let ok = !file.is_empty()
        && !file.contains("..")
        && !file.starts_with('/')
        && file
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'));
    if ok {
        Ok(file.to_string())
    } else {
        Err(format!("非法的文件名: {file}"))
    }
}

/// 解析 SKILL.md frontmatter 的 name 与 description（多行折叠为单行）
fn frontmatter(text: &str) -> (String, String) {
    let mut name = String::new();
    let mut desc = String::new();
    let mut in_desc = false;
    let mut lines = text.lines();
    if !lines.next().map(|l| l.trim() == "---").unwrap_or(false) {
        return (name, desc);
    }
    for line in lines {
        if line.trim().is_empty() {
            continue;
        }
        let indented = line.starts_with(' ') || line.starts_with('\t');
        if !indented {
            let t = line.trim_end();
            if t == "---" || t == "..." {
                break;
            }
            in_desc = false;
            if let Some(rest) = t.strip_prefix("name:") {
                name = rest.trim().trim_matches('"').to_string();
            } else if let Some(rest) = t.strip_prefix("description:") {
                let v = rest.trim();
                if v.is_empty() || matches!(v, "|" | "|-" | ">" | ">-") {
                    in_desc = true;
                } else {
                    desc = v.trim_matches('"').to_string();
                }
            }
        } else if in_desc {
            if !desc.is_empty() {
                desc.push(' ');
            }
            desc.push_str(line.trim());
        }
    }
    (name, desc)
}

/// 技能目录：列出全部技能（name / category / description / 是否有正文）
pub fn list() -> Result<Value, String> {
    let dir = skills_dir()?;
    let cats: Value = serde_json::from_str(include_str!("../resources/skill-categories.json"))
        .expect("skill-categories.json 是合法 JSON");
    let map = cats["map"].as_object().expect("categories map");
    let mut items = Vec::new();
    for entry in std::fs::read_dir(&dir).map_err(|e| format!("读取技能库失败: {e}"))? {
        let entry = entry.map_err(|e| e.to_string())?;
        if !entry.file_type().map(|t| t.is_dir()).unwrap_or(false) {
            continue;
        }
        let slug = entry.file_name().to_string_lossy().to_string();
        let md_path = entry.path().join("SKILL.md");
        let (fm_name, desc) = std::fs::read_to_string(&md_path)
            .map(|t| frontmatter(&t))
            .unwrap_or_default();
        let size = std::fs::metadata(&md_path).map(|m| m.len()).unwrap_or(0);
        items.push(json!({
            "name": if fm_name.is_empty() { slug.clone() } else { fm_name },
            "dir": slug,
            "category": map.get(&slug).and_then(|v| v.as_str()).unwrap_or("其他"),
            "description": desc,
            "has_body": size >= STUB_BYTES,
        }));
    }
    items.sort_by(|a, b| {
        a["category"]
            .as_str()
            .cmp(&b["category"].as_str())
            .then_with(|| a["dir"].as_str().cmp(&b["dir"].as_str()))
    });
    Ok(json!({ "total": items.len(), "skills": items }))
}

/// 读取技能库中的一篇文档；file 省略返回 SKILL.md 主文档
pub fn get(args: &Value) -> Result<Value, String> {
    let dir = skills_dir()?;
    let name = args
        .get("name")
        .and_then(|v| v.as_str())
        .ok_or("缺少参数 name")?;
    let slug = safe_slug(name)?;
    let skill_dir = dir.join(&slug);
    if !skill_dir.is_dir() {
        return Err(format!("技能不存在: {name}（用 list_skills 查看可用技能）"));
    }
    let file = match args.get("file").and_then(|v| v.as_str()) {
        Some(f) => safe_file(f)?,
        None => "SKILL.md".to_string(),
    };
    let path = skill_dir.join(&file);
    if !path.is_file() {
        let mut files: Vec<String> = std::fs::read_dir(&skill_dir)
            .map_err(|e| e.to_string())?
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().map(|t| t.is_file()).unwrap_or(false))
            .map(|e| e.file_name().to_string_lossy().to_string())
            .collect();
        files.sort();
        return Err(format!(
            "技能 {name} 里没有文件 {file}。可用文件: {}",
            files.join(", ")
        ));
    }
    let content = std::fs::read_to_string(&path).map_err(|e| format!("读取失败: {e}"))?;
    Ok(json!({ "name": slug, "file": file, "content": content }))
}

/// 聿Canvas 自身的产物规范（画板格式 / 令牌命名 / design.md 宪法 / mc- BEM）
pub fn conventions() -> Result<Value, String> {
    Ok(json!({ "content": include_str!("../resources/conventions.md") }))
}
