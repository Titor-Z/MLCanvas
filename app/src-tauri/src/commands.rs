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
    /// 画板页面地址：资源服务（相对路径可解析）优先，未启动时回退 asset 协议
    pub url: String,
    /// 画板 HTML 是否显式声明了位置
    pub declared_pos: bool,
}

#[derive(Serialize, Clone)]
pub struct ProjectInfo {
    pub name: String,
    /// 项目文件夹绝对路径（前端显示时把用户目录缩短为 ~）
    pub path: String,
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

/// 从画板/组件 HTML 解析 `<title>` 作为显示名称，缺省用文件名去扩展名
pub fn parse_artboard_name(html: &str, file_name: &str) -> String {
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
    let mut index = state::load_index();
    let index_len_before = index.len();
    let mut projects = Vec::new();
    for entry in &index {
        // 项目可在任意目录（路径来自索引）；文件夹已消失的记录顺手清掉
        let dir = PathBuf::from(&entry.path);
        let artboards_dir = dir.join("artboards");
        if !artboards_dir.is_dir() {
            continue;
        }

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
        projects.push(ProjectInfo {
            name: entry.name.clone(),
            path: entry.path.clone(),
            artboards: count,
            updated_at: latest,
        });
    }
    // 索引里指向已消失文件夹的记录顺手清掉
    index.retain(|e| PathBuf::from(&e.path).join("artboards").is_dir());
    if index.len() != index_len_before {
        state::save_index(&index)?;
    }
    projects.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
    Ok(projects)
}

/// 展开路径前导 `~` 为用户目录
fn expand_home(path: &str) -> PathBuf {
    if let Some(rest) = path.strip_prefix("~/") {
        state::home_dir().join(rest)
    } else {
        PathBuf::from(path)
    }
}

/// 创建项目：目录默认 ~/YuCanvas/<名称>，也可指定任意目录（目录名须与项目名一致）。
/// 目录已存在时是"重新挂回索引"——删除只是移出索引，文件夹一直都在。
#[tauri::command]
pub fn create_project(name: String, dir: Option<String>) -> Result<ProjectInfo, String> {
    let name = state::validate_project_name(&name)?;
    let target = match dir.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
        Some(d) => {
            let expanded = expand_home(d);
            let base = expanded
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_default();
            if base != name {
                return Err("目录名与项目名不一致".to_string());
            }
            expanded
        }
        None => state::project_dir(&name),
    };
    if !target.is_dir() {
        std::fs::create_dir_all(&target).map_err(|e| format!("创建项目失败: {e}"))?;
    }
    let target = target.canonicalize().unwrap_or(target);
    let artboards = target.join("artboards");
    if !artboards.is_dir() {
        std::fs::create_dir_all(&artboards).map_err(|e| format!("创建项目失败: {e}"))?;
    }

    let mut index = state::load_index();
    state::index_add(&mut index, &name, &target.to_string_lossy());
    state::save_index(&index)?;

    Ok(ProjectInfo {
        path: target.to_string_lossy().to_string(),
        name,
        artboards: 0,
        updated_at: 0,
    })
}

/// 删除项目：只从项目索引里移除记录，文件夹原地保留，同名重建即挂回
#[tauri::command]
pub fn delete_project(app: AppHandle, name: String, state: State<AppState>) -> Result<(), String> {
    let name = state::validate_project_name(&name)?;
    let mut index = state::load_index();
    let before = index.len();
    index.retain(|e| e.name != name);
    if index.len() == before {
        return Err(format!("项目不在索引中: {name}"));
    }
    state::save_index(&index)?;

    // 删的是当前激活项目时，回到"未打开"态：MCP 工具重新要求用户先打开项目
    let mut active = state.active_project.lock().expect("active_project lock poisoned");
    if *active == name {
        *active = state::setup_default_name();
        state
            .project_opened
            .store(false, std::sync::atomic::Ordering::SeqCst);
        crate::watcher::remount("");
        let _ = app.emit("project-opened", (*active).clone());
    }
    Ok(())
}

/// 打开项目：设为激活项目，创建/聚焦设计窗口，并广播给设计窗口
#[tauri::command]
pub fn open_project(app: AppHandle, name: String, state: State<AppState>) -> Result<(), String> {
    let name = state::validate_project_name(&name)?;
    if !state::artboards_dir_of(&name).is_dir() {
        return Err(format!("项目不存在: {name}"));
    }
    *state.active_project.lock().expect("active_project lock poisoned") = name.clone();
    state
        .project_opened
        .store(true, std::sync::atomic::Ordering::SeqCst);
    // watcher 重挂到新项目的 artboards/
    crate::watcher::remount(&name);

    if let Some(win) = app.get_webview_window("design") {
        let _ = win.set_focus();
    } else {
        WebviewWindowBuilder::new(&app, "design", WebviewUrl::App("index.html".into()))
            .title("聿Canvas")
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

/// 最近接入的 Agent 客户端名（MCP initialize 的 clientInfo），空 = 尚未接入
#[tauri::command]
pub fn agent_client(state: State<AppState>) -> String {
    state.agent_client.lock().expect("agent_client lock poisoned").clone()
}

/// 壳页面 JS 错误落 stderr（无控制台环境的前端诊断通道）
#[tauri::command]
pub fn debug_log(msg: String) {
    eprintln!("[mlcanvas:js] {msg}");
}

/// 系统目录选择器：返回所选目录绝对路径，取消返回 None
#[tauri::command]
pub fn pick_directory(app: AppHandle) -> Result<Option<String>, String> {
    let (tx, rx) = std::sync::mpsc::channel::<Option<String>>();
    app.run_on_main_thread(move || {
        let picked = pick_directory_on_main();
        let _ = tx.send(picked);
    })
    .map_err(|e| format!("无法打开目录选择器: {e}"))?;
    rx.recv_timeout(std::time::Duration::from_secs(300))
        .map_err(|_| "目录选择超时".to_string())
}

#[cfg(not(target_os = "macos"))]
fn pick_directory_on_main() -> Option<String> {
    // 非 macOS 平台暂未接入系统目录选择器；前端保留手填路径即可
    None
}

#[cfg(target_os = "macos")]
fn pick_directory_on_main() -> Option<String> {
    use objc2_app_kit::{NSModalResponseOK, NSOpenPanel};
    use objc2_foundation::NSString;

    let mtm = objc2::MainThreadMarker::new().expect("目录选择器必须在主线程");
    let panel = NSOpenPanel::openPanel(mtm);
    panel.setCanChooseDirectories(true);
    panel.setCanChooseFiles(false);
    panel.setAllowsMultipleSelection(false);
    panel.setMessage(Some(&NSString::from_str("选择项目目录")));
    panel.setPrompt(Some(&NSString::from_str("选择目录")));
    let response = panel.runModal();
    if response != NSModalResponseOK {
        return None;
    }
    panel.URLs().firstObject()
        .and_then(|url| url.path())
        .map(|p| p.to_string())
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
                url: {
                    let project = state
                        .active_project
                        .lock()
                        .expect("active_project lock poisoned")
                        .clone();
                    crate::artboard_http::artboard_url(&project, &path)
                        .unwrap_or_else(|| crate::artboard_http::asset_url(&path))
                },
                declared_pos: pos.is_some(),
            });
        }
    }
    boards.sort_by(|a, b| a.file_name.cmp(&b.file_name));
    const GAP: i32 = 120;

    // 用户排过序（.order.json 存在）时，列表按用户顺序显示——
    // 只影响导航顺序与未声明位置画板的平铺次序；已声明位置是构图，不被列表侵犯
    let order = load_order(&state);
    if !order.is_empty() {
        boards.sort_by_key(|b| order.iter().position(|f| f == &b.file_name).unwrap_or(usize::MAX));
    }

    // 未声明位置的画板自动排成网格：一行放不下换行，基线接在已声明画板右侧
    let auto_x = boards
        .iter()
        .filter(|b| b.declared_pos)
        .map(|b| b.x + b.width as i32)
        .max()
        .unwrap_or(0)
        + 120;
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
pub fn register_artboard(state: State<AppState>, file_name: String, title: Option<String>) -> Result<String, String> {    let name = validate_file_name(&file_name)?;
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

/// 画板顺序文件：<项目>/.order.json。用户在左栏拖拽排序后写入——
/// 存在时画布按该顺序单行重排（用户显式排序优先于 Agent 声明的位置）
fn order_path(state: &AppState) -> PathBuf {
    let dir = artboards_dir(state);
    dir.parent()
        .map(|p| p.join(".order.json"))
        .unwrap_or_else(|| state::projects_root().join(".order.json"))
}

fn load_order(state: &AppState) -> Vec<String> {
    std::fs::read_to_string(order_path(state))
        .ok()
        .and_then(|raw| serde_json::from_str::<Vec<String>>(&raw).ok())
        .unwrap_or_default()
}

fn save_order(state: &AppState, files: &[String]) -> Result<(), String> {
    std::fs::write(
        order_path(state),
        serde_json::to_string(files).map_err(|e| e.to_string())?,
    )
    .map_err(|e| format!("保存画板顺序失败: {e}"))
}

/// 画板排序：左栏拖拽落点后由前端调用；画布布局随 list_artboards 重算
#[tauri::command]
pub fn save_artboard_order(state: State<AppState>, files: Vec<String>) -> Result<(), String> {
    for f in &files {
        validate_file_name(f)?;
    }
    save_order(&state, &files)
}

/// 修改画板标题（用户可见的语义名称，即 <title>；文件名不动）
#[tauri::command]
pub fn set_artboard_title(state: State<AppState>, file_name: String, title: String) -> Result<(), String> {
    let title = title.trim();
    if title.is_empty() {
        return Err("名称不能为空".to_string());
    }
    if title.contains('<') || title.contains('>') {
        return Err("名称不能包含 < 或 >".to_string());
    }
    let path = artboard_path(&artboards_dir(&state), &file_name)?;
    let html = read_board(&path)?;
    let new_html = match (html.find("<title>"), html.find("</title>")) {
        (Some(s), Some(e)) if e > s => {
            format!("{}<title>{title}</title>{}", &html[..s], &html[e + "</title>".len()..])
        }
        _ => html.replacen("<head>", &format!("<head>\n  <title>{title}</title>"), 1),
    };
    std::fs::write(&path, new_html.as_bytes()).map_err(|e| format!("写入画板失败: {e}"))
}

/// 导入项目：把磁盘上任意现有文件夹挂回索引（「打开项目」的后端）。
/// 缺 artboards/ 时补建——任意文件夹收编为项目；项目名取目录名。
#[tauri::command]
pub fn import_project(dir: String) -> Result<ProjectInfo, String> {
    let expanded = expand_home(dir.trim());
    if !expanded.is_dir() {
        return Err("目录不存在".to_string());
    }
    let canon = expanded.canonicalize().map_err(|e| format!("目录无效: {e}"))?;
    let base = canon
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_default();
    let name = state::validate_project_name(&base)?;
    let artboards = canon.join("artboards");
    if !artboards.is_dir() {
        std::fs::create_dir_all(&artboards).map_err(|e| format!("初始化项目失败: {e}"))?;
    }
    let mut index = state::load_index();
    state::index_add(&mut index, &name, &canon.to_string_lossy());
    state::save_index(&index)?;
    Ok(ProjectInfo {
        path: canon.to_string_lossy().to_string(),
        name,
        artboards: 0,
        updated_at: 0,
    })
}

/// 删除画板：真删文件（画布是设计工作台，废弃稿的清理是正当操作）
#[tauri::command]
pub fn delete_artboard(state: State<AppState>, file_name: String) -> Result<(), String> {
    let path = artboard_path(&artboards_dir(&state), &file_name)?;
    if !path.is_file() {
        return Err(format!("画板不存在: {file_name}"));
    }
    std::fs::remove_file(&path).map_err(|e| format!("删除画板失败: {e}"))?;
    let mut order = load_order(&state);
    if order.iter().any(|f| f == &file_name) {
        order.retain(|f| f != &file_name);
        save_order(&state, &order)?;
    }
    Ok(())
}

/// 重命名画板：换文件名（语义命名是 Agent 的职责）；顺序记录同步改名
#[tauri::command]
pub fn rename_artboard(state: State<AppState>, file_name: String, new_name: String) -> Result<String, String> {
    let dir = artboards_dir(&state);
    let from = artboard_path(&dir, &file_name)?;
    let to = artboard_path(&dir, &new_name)?;
    if !from.is_file() {
        return Err(format!("画板不存在: {file_name}"));
    }
    if to.exists() {
        return Err(format!("目标画板已存在: {new_name}"));
    }
    std::fs::rename(&from, &to).map_err(|e| format!("重命名失败: {e}"))?;
    let mut order = load_order(&state);
    if let Some(pos) = order.iter().position(|f| f == &file_name) {
        order[pos] = new_name.clone();
        save_order(&state, &order)?;
    }
    Ok(to.to_string_lossy().to_string())
}

// ── 风格规范与组件（资源面板的数据源）：项目根下与 artboards/ 平级的 design.md / tokens.css / components/ ──

#[derive(Serialize, Clone)]
pub struct DesignSpec {
    /// 风格名（design.md 首个 `# ` 标题），未写时为空
    pub name: String,
    pub design_md: String,
    pub tokens_css: String,
}

#[derive(Serialize, Clone)]
pub struct Component {
    pub file_name: String,
    /// 组件名（<title>），UI 一律用名称
    pub name: String,
    /// 来源画板（首行 `<!-- 来自: X -->` 标注），未标注时为空
    pub source: String,
    pub path: String,
    /// 组件 HTML 原文（面板里 iframe 预览用）
    pub html: String,
}

/// 项目根目录（artboards/ 的上级）
fn project_root(state: &AppState) -> PathBuf {
    let dir = artboards_dir(state);
    dir.parent().map(|p| p.to_path_buf()).unwrap_or(dir)
}

/// 读取风格规范；两个文件都不存在时各字段为空串（前端展示空态，MCP 返回引导）
#[tauri::command]
pub fn get_design_spec(state: State<AppState>) -> Result<DesignSpec, String> {
    let root = project_root(&state);
    let design_md = std::fs::read_to_string(root.join("design.md")).unwrap_or_default();
    let tokens_css = std::fs::read_to_string(root.join("tokens.css")).unwrap_or_default();
    let name = design_md
        .lines()
        .find_map(|l| l.strip_prefix("# ").map(|s| s.trim().to_string()))
        .unwrap_or_default();
    Ok(DesignSpec { name, design_md, tokens_css })
}

/// 保存风格规范（Agent 经 MCP 写入）
pub fn save_design_spec(state: &AppState, design_md: String, tokens_css: String) -> Result<(), String> {
    let root = project_root(state);
    std::fs::create_dir_all(&root).map_err(|e| format!("保存规范失败: {e}"))?;
    std::fs::write(root.join("design.md"), design_md.trim()).map_err(|e| format!("保存规范失败: {e}"))?;
    std::fs::write(root.join("tokens.css"), tokens_css.trim()).map_err(|e| format!("保存规范失败: {e}"))?;
    Ok(())
}

/// 从组件 HTML 解析 `<!-- 来自: X -->` 来源标注
fn parse_component_source(html: &str) -> String {
    let Some(start) = html.find("<!-- 来自:") else { return String::new() };
    let rest = &html[start + "<!-- 来自:".len()..];
    let Some(end) = rest.find("-->") else { return String::new() };
    rest[..end].trim().to_string()
}

/// 列出项目组件（components/ 下的静态实物预览片段）
#[tauri::command]
pub fn list_components(state: State<AppState>) -> Result<Vec<Component>, String> {
    let dir = project_root(&state).join("components");
    if !dir.is_dir() {
        return Ok(vec![]);
    }
    let mut items = Vec::new();
    for entry in std::fs::read_dir(&dir).map_err(|e| e.to_string())?.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("html") {
            continue;
        }
        let file_name = path
            .file_name()
            .expect("read_dir entries always have a file name")
            .to_string_lossy()
            .to_string();
        let html = std::fs::read_to_string(&path).unwrap_or_default();
        let name = parse_artboard_name(&html, &file_name);
        let source = parse_component_source(&html);
        items.push(Component {
            name,
            source,
            file_name,
            path: path.to_string_lossy().to_string(),
            html,
        });
    }
    items.sort_by(|a, b| a.file_name.cmp(&b.file_name));
    Ok(items)
}

/// 保存组件片段（Agent 经 MCP 写入）
pub fn save_component(state: &AppState, file_name: String, content: String) -> Result<PathBuf, String> {
    let dir = project_root(state).join("components");
    std::fs::create_dir_all(&dir).map_err(|e| format!("保存组件失败: {e}"))?;
    let path = dir.join(validate_file_name(&file_name)?);
    std::fs::write(&path, content.trim()).map_err(|e| format!("保存组件失败: {e}"))?;
    Ok(path)
}

#[tauri::command]
pub fn list_design_systems() -> Result<serde_json::Value, String> {
    crate::design_systems::list()
}

#[tauri::command]
pub fn read_design_system(name: String, file: String) -> Result<serde_json::Value, String> {
    crate::design_systems::get(&serde_json::json!({ "name": name, "file": file }))
}
