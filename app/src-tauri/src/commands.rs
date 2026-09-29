use serde::Serialize;
use std::path::PathBuf;
use tauri::{AppHandle, Emitter, Manager, State, WebviewUrl, WebviewWindowBuilder};

use crate::state::{self, AppState};

#[derive(Serialize, Clone)]
pub struct Artboard {
    pub id: String,
    /// 显示名称：取自画板 <title>，UI 一律用名称而非文件名
    pub name: String,
    pub file_name: String,
    pub path: String,
    pub width: u32,
    pub height: u32,
    /// 画布位置（画板自身 CSS 像素），未声明时自动平铺
    pub x: i32,
    pub y: i32,
    /// 画板 HTML 是否显式声明了位置
    pub declared_pos: bool,
}

#[derive(Serialize, Clone)]
pub struct ProjectInfo {
    pub name: String,
    pub artboards: u32,
    /// 最近一次画板变动的 Unix 时间戳（秒）
    pub updated_at: i64,
}

/// 当前激活项目的画板目录
pub fn artboards_dir(state: &AppState) -> PathBuf {
    let name = state.active_project.lock().expect("active_project lock poisoned").clone();
    state::artboards_dir_of(&name)
}

/// 从画板 HTML 解析 `<meta name="artboard" content="宽x高[, x=X, y=Y]">`。
/// 位置可省略（返回 None，由工具自动平铺）；尺寸非法时用缺省 320x640。
pub fn parse_artboard_meta(html: &str) -> ((u32, u32), Option<(i32, i32)>) {
    const DEFAULT_SIZE: (u32, u32) = (320, 640);
    let Some(pos) = html.find("name=\"artboard\"") else {
        return (DEFAULT_SIZE, None);
    };
    let rest = &html[pos..];
    let Some(marker) = rest.find("content=\"") else {
        return (DEFAULT_SIZE, None);
    };
    let after = &rest[marker + "content=\"".len()..];
    let value = &after[..after.find('"').unwrap_or(0)];

    let mut size = DEFAULT_SIZE;
    let mut xy: Option<(i32, i32)> = None;
    for token in value.split(',') {
        let token = token.trim();
        let (key, val) = match token.split_once('=') {
            Some((k, v)) => (k.trim().to_lowercase(), v.trim()),
            None => (String::new(), token),
        };
        match key.as_str() {
            "x" => {
                let x = val.parse().unwrap_or(0);
                xy = Some((x, xy.map(|p| p.1).unwrap_or(0)));
            }
            "y" => {
                let y = val.parse().unwrap_or(0);
                xy = Some((xy.map(|p| p.0).unwrap_or(0), y));
            }
            _ => {
                let mut parts = token.split(['x', 'X']);
                let w = parts.next().and_then(|s| s.trim().parse().ok()).unwrap_or(0);
                let h = parts.next().and_then(|s| s.trim().parse().ok()).unwrap_or(0);
                if w > 0 && h > 0 {
                    size = (w, h);
                }
            }
        }
    }
    (size, xy)
}

/// 从画板 HTML 解析 `<meta name="artboard" content="宽x高">`，无声明或非法时返回缺省值
pub fn parse_artboard_size(html: &str) -> (u32, u32) {
    parse_artboard_meta(html).0
}

/// 从画板 HTML 解析 `<title>` 作为显示名称，缺省用文件名去扩展名
fn parse_artboard_name(html: &str, file_name: &str) -> String {
    let start = html.find("<title>").map(|p| p + "<title>".len());
    let end = html.find("</title>");
    if let (Some(start), Some(end)) = (start, end) {
        if end > start {
            let name = html[start..end].trim();
            if !name.is_empty() {
                return name.to_string();
            }
        }
    }
    file_name.trim_end_matches(".html").to_string()
}

/// 文件名校验：防路径穿越，只允许 artboards/ 下的 .html 文件
pub fn validate_file_name(file_name: &str) -> Result<String, String> {
    let name = file_name.trim();
    if name.is_empty() {
        return Err("file_name 不能为空".to_string());
    }
    if name.contains('/') || name.contains('\\') || name.contains("..") {
        return Err(format!("非法文件名: {name}"));
    }
    if !name.ends_with(".html") {
        return Err(format!("画板必须是 .html 文件: {name}"));
    }
    Ok(name.to_string())
}

fn artboard_path(dir: &PathBuf, file_name: &str) -> Result<PathBuf, String> {
    Ok(dir.join(validate_file_name(file_name)?))
}

fn read_board(path: &PathBuf) -> Result<String, String> {
    std::fs::read_to_string(path).map_err(|e| format!("读取画板失败: {e}"))
}

#[tauri::command]
pub fn list_projects() -> Result<Vec<ProjectInfo>, String> {
    let root = state::projects_root();
    let mut projects = Vec::new();
    for entry in std::fs::read_dir(&root).map_err(|e| e.to_string())?.flatten() {
        let dir = entry.path();
        let artboards_dir = dir.join("artboards");
        if !artboards_dir.is_dir() {
            continue;
        }
        let Some(name) = dir.file_name().map(|n| n.to_string_lossy().to_string()) else { continue };

        let mut count = 0u32;
        let mut latest = 0i64;
        if let Ok(entries) = std::fs::read_dir(&artboards_dir) {
            for e in entries.flatten() {
                let p = e.path();
                if p.extension().and_then(|x| x.to_str()) == Some("html") {
                    count += 1;
                    if let Ok(meta) = e.metadata() {
                        if let Ok(modified) = meta.modified() {
                            if let Ok(secs) = modified.duration_since(std::time::UNIX_EPOCH) {
                                latest = latest.max(secs.as_secs() as i64);
                            }
                        }
                    }
                }
            }
        }
        projects.push(ProjectInfo { name, artboards: count, updated_at: latest });
    }
    projects.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
    Ok(projects)
}

#[tauri::command]
pub fn create_project(name: String) -> Result<ProjectInfo, String> {
    let name = state::validate_project_name(&name)?;
    let dir = state::project_dir(&name);
    if dir.exists() {
        return Err(format!("项目已存在: {name}"));
    }
    std::fs::create_dir_all(state::artboards_dir_of(&name))
        .map_err(|e| format!("创建项目失败: {e}"))?;
    Ok(ProjectInfo { name, artboards: 0, updated_at: 0 })
}

/// 打开项目：设为激活项目，创建/聚焦设计窗口，并广播给设计窗口
#[tauri::command]
pub fn open_project(app: AppHandle, name: String, state: State<AppState>) -> Result<(), String> {
    let name = state::validate_project_name(&name)?;
    if !state::artboards_dir_of(&name).is_dir() {
        return Err(format!("项目不存在: {name}"));
    }
    *state.active_project.lock().expect("active_project lock poisoned") = name.clone();

    if let Some(win) = app.get_webview_window("design") {
        let _ = win.set_focus();
    } else {
        WebviewWindowBuilder::new(&app, "design", WebviewUrl::App("index.html".into()))
            .title("MLCanvas")
            .inner_size(1280.0, 800.0)
            .min_inner_size(960.0, 600.0)
            .decorations(false)
            .build()
            .map_err(|e| format!("打开设计窗口失败: {e}"))?;
    }
    // 进入设计模式后主页窗口退出舞台；设计窗口关闭时由 main.rs 重建主页
    if let Some(home) = app.get_webview_window("main") {
        let _ = home.close();
    }
    app.emit_to("design", "project-opened", name).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_active_project(state: State<AppState>) -> String {
    state.active_project.lock().expect("active_project lock poisoned").clone()
}

#[tauri::command]
pub fn bridge_status() -> bool {
    true
}

#[tauri::command]
pub fn list_artboards(state: State<AppState>) -> Result<Vec<Artboard>, String> {
    let dir = artboards_dir(&state);
    if !dir.is_dir() {
        return Ok(vec![]);
    }

    let mut boards = Vec::new();
    let entries = std::fs::read_dir(&dir).map_err(|e| e.to_string())?;
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) == Some("html") {
            let file_name = path
                .file_name()
                .expect("read_dir entries always have a file name")
                .to_string_lossy()
                .to_string();
            let html = read_board(&path).unwrap_or_default();
            let ((width, height), pos) = parse_artboard_meta(&html);
            let name = parse_artboard_name(&html, &file_name);
            boards.push(Artboard {
                id: file_name.clone(),
                name,
                file_name,
                path: path.to_string_lossy().to_string(),
                width,
                height,
                x: pos.map(|p| p.0).unwrap_or(0),
                y: pos.map(|p| p.1).unwrap_or(0),
                declared_pos: pos.is_some(),
            });
        }
    }
    boards.sort_by(|a, b| a.file_name.cmp(&b.file_name));
    // 未声明位置的画板自动排成网格：一行放不下换行，基线接在已声明画板右侧
    let auto_x = boards
        .iter()
        .filter(|b| b.declared_pos)
        .map(|b| b.x + b.width as i32)
        .max()
        .unwrap_or(0)
        + 120;
    const GAP: i32 = 120;
    const MAX_ROW_W: i32 = 2400;
    let (mut cx, mut cy, mut row_h) = (auto_x, 0i32, 0i32);
    for b in boards.iter_mut() {
        if b.declared_pos {
            continue;
        }
        if cx > auto_x && (cx - auto_x) + b.width as i32 > MAX_ROW_W {
            cx = auto_x;
            cy += row_h + GAP;
            row_h = 0;
        }
        b.x = cx;
        b.y = cy;
        cx += b.width as i32 + GAP;
        row_h = row_h.max(b.height as i32);
    }
    Ok(boards)
}

#[tauri::command]
pub fn read_artboard(state: State<AppState>, file_name: String) -> Result<String, String> {
    let path = artboard_path(&artboards_dir(&state), &file_name)?;
    if !path.is_file() {
        return Err(format!("画板不存在: {file_name}"));
    }
    read_board(&path)
}

#[tauri::command]
pub fn write_artboard(state: State<AppState>, file_name: String, content: String) -> Result<String, String> {
    let dir = artboards_dir(&state);
    let path = artboard_path(&dir, &file_name)?;
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    std::fs::write(&path, content.as_bytes()).map_err(|e| format!("写入画板失败: {e}"))?;
    Ok(path.to_string_lossy().to_string())
}

#[tauri::command]
pub fn register_artboard(state: State<AppState>, file_name: String, title: Option<String>) -> Result<String, String> {
    let name = validate_file_name(&file_name)?;
    let dir = artboards_dir(&state);
    let path = dir.join(&name);
    if path.is_file() {
        return Err(format!("画板已存在: {name}（用 write_artboard 覆盖内容）"));
    }
    let title = title.unwrap_or_else(|| name.trim_end_matches(".html").to_string());
    let template = format!(
        "<!doctype html>\n<html lang=\"zh-CN\">\n<head>\n  <meta charset=\"UTF-8\">\n  <title>{title}</title>\n</head>\n<body>\n  <h1>{title}</h1>\n</body>\n</html>\n"
    );
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    std::fs::write(&path, template.as_bytes()).map_err(|e| format!("创建画板失败: {e}"))?;
    Ok(path.to_string_lossy().to_string())
}
