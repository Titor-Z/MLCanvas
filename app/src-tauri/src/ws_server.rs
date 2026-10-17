//! MCP 桥：监听 127.0.0.1:9915，NDJSON 协议。
//! mcp/server.mjs（stdio MCP 服务器）把工具调用转发到这里。
//! 请求: {"id": "...", "tool": "...", "args": {...}}
//! 响应: {"id": "...", "ok": true, "result": ...} | {"id": "...", "ok": false, "error": "..."}

use base64::Engine as _;
use serde_json::{json, Value};
use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use tauri::{Emitter, Manager};

use crate::commands;
use crate::inspector::Inspector;
use crate::state::AppState;

const BRIDGE_ADDR: &str = "127.0.0.1:9915";

pub fn start(app: tauri::AppHandle) {
    std::thread::spawn(move || match TcpListener::bind(BRIDGE_ADDR) {
        Ok(listener) => {
            eprintln!("[mlcanvas] MCP bridge listening on {BRIDGE_ADDR}");
            for stream in listener.incoming().flatten() {
                let app = app.clone();
                std::thread::spawn(move || handle_connection(stream, app));
            }
        }
        Err(e) => eprintln!("[mlcanvas] MCP bridge 绑定 {BRIDGE_ADDR} 失败: {e}"),
    });
}

fn handle_connection(stream: TcpStream, app: tauri::AppHandle) {
    let mut writer = stream
        .try_clone()
        .expect("clone tcp stream for writing is supported on unix");
    let reader = BufReader::new(stream);
    for line in reader.lines() {
        let Ok(line) = line else { break };
        if line.trim().is_empty() {
            continue;
        }
        let response = match serde_json::from_str::<Value>(&line) {
            Ok(req) => handle_request(req, &app),
            Err(e) => error_value(None, format!("请求不是合法 JSON: {e}")),
        };
        let mut payload = response.to_string().into_bytes();
        payload.push(b'\n');
        if writer.write_all(&payload).is_err() {
            break;
        }
    }
}

fn param_str(args: &Value, key: &str) -> Result<String, String> {
    args.get(key)
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .ok_or(format!("缺少参数 {key}"))
}

fn param_f64(args: &Value, key: &str) -> Result<f64, String> {
    args.get(key)
        .and_then(|v| v.as_f64())
        .ok_or(format!("缺少数值参数 {key}"))
}

/// 去掉 /* */ 注释，避免注释内容干扰令牌扫描
fn strip_css_comments(css: &str) -> String {
    let mut out = String::with_capacity(css.len());
    let mut in_comment = false;
    let mut chars = css.chars().peekable();
    while let Some(c) = chars.next() {
        if in_comment {
            if c == '*' && chars.peek() == Some(&'/') {
                chars.next();
                in_comment = false;
            }
        } else if c == '/' && chars.peek() == Some(&'*') {
            chars.next();
            in_comment = true;
        } else {
            out.push(c);
        }
    }
    out
}

/// 风格规范软提示：design.md 不该出现具体色值（值只活在 tokens.css）
fn hex_in_design_md(design_md: &str) -> bool {
    let bytes = design_md.as_bytes();
    for (i, &b) in bytes.iter().enumerate() {
        if b != b'#' {
            continue;
        }
        let hex_len = bytes[i + 1..]
            .iter()
            .take_while(|&&c| c.is_ascii_hexdigit())
            .count();
        if hex_len == 3 || hex_len == 6 {
            return true;
        }
    }
    false
}

/// 风格规范软提示：令牌名称强制英文
fn non_ascii_token_name(tokens_css: &str) -> bool {
    strip_css_comments(tokens_css)
        .split("--")
        .skip(1)
        .any(|seg| seg.chars().take_while(|c| !matches!(c, ' ' | ':' | ';')).any(|c| !c.is_ascii()))
}

/// 风格规范软提示：tokens.css 里有、design.md「令牌对照」没登记中文名的令牌
fn unregistered_tokens(design_md: &str, tokens_css: &str) -> Vec<String> {
    let mut registered = std::collections::HashSet::new();
    let mut section = "";
    for line in design_md.lines() {
        let t = line.trim();
        if let Some(title) = t.strip_prefix("## ") {
            section = if title.trim() == "令牌对照" { "labels" } else { "" };
            continue;
        }
        let Some(item) = t.strip_prefix("- ") else { continue };
        if section != "labels" {
            continue;
        }
        if let Some(rest) = item.trim().strip_prefix("--") {
            let name: String = rest
                .chars()
                .take_while(|c| !matches!(c, ':' | '：' | ' '))
                .collect();
            if !name.is_empty() {
                registered.insert(format!("--{name}"));
            }
        }
    }
    let mut missing: Vec<String> = Vec::new();
    for seg in strip_css_comments(tokens_css).split("--").skip(1) {
        let name: String = seg
            .chars()
            .take_while(|c| !matches!(c, ' ' | ':' | ';'))
            .collect();
        if name.is_empty() {
            continue;
        }
        let full = format!("--{name}");
        // 字阶是同名配对：--font-weight-X 由 --font-size-X 的对照覆盖
        let weight_twin = full.strip_prefix("--font-weight-").map(|n| format!("--font-size-{n}"));
        let known = registered.contains(&full)
            || weight_twin.is_some_and(|t| registered.contains(&t));
        if !known && !missing.contains(&full) {
            missing.push(full);
        }
    }
    missing
}

/// 画板索引软提示：design.md「画板索引」与画板目录实况对账——
/// 防索引陈旧（删除未更新）与漏登记（新建未登记）；hash 命名保证陈旧条目不会误认领新画板
fn artboard_registry_hints(design_md: &str, files: &[String]) -> Vec<String> {
    let mut registered: Vec<String> = Vec::new();
    let mut section = "";
    for line in design_md.lines() {
        let t = line.trim();
        if let Some(title) = t.strip_prefix("## ") {
            section = if title.trim() == "画板索引" { "index" } else { "" };
            continue;
        }
        if section != "index" {
            continue;
        }
        let Some(item) = t.strip_prefix("- ") else { continue };
        let name = item
            .trim()
            .split(|c| c == '：' || c == ':')
            .next()
            .unwrap_or("")
            .trim()
            .to_string();
        if name.ends_with(".html") && !registered.contains(&name) {
            registered.push(name);
        }
    }
    let mut hints = Vec::new();
    let missing: Vec<String> = registered
        .iter()
        .filter(|n| !files.contains(n))
        .cloned()
        .collect();
    if !missing.is_empty() {
        let list = missing.iter().take(5).cloned().collect::<Vec<_>>().join("、");
        let more = if missing.len() > 5 { " 等" } else { "" };
        hints.push(format!(
            "「画板索引」中 {} 个画板文件已不存在（可能已被删除）：{list}{more}——请核对并从索引移除",
            missing.len()
        ));
    }
    let unregistered: Vec<&String> = files.iter().filter(|f| !registered.contains(f)).collect();
    if !unregistered.is_empty() {
        let list = unregistered.iter().take(5).map(|s| s.as_str()).collect::<Vec<_>>().join("、");
        let more = if unregistered.len() > 5 { " 等" } else { "" };
        hints.push(format!(
            "画板目录中有 {} 个画板未登记「画板索引」：{list}{more}——请在 design.md 补登记用途",
            unregistered.len()
        ));
    }
    hints
}

fn error_value(id: Option<Value>, error: String) -> Value {
    json!({ "id": id, "ok": false, "error": error })
}

/// 会话契约：用户必须先在聿Canvas 中创建/打开项目，Agent 工具才可用
fn ensure_project_opened(app: &tauri::AppHandle) -> Result<(), String> {
    if app
        .state::<AppState>()
        .project_opened
        .load(std::sync::atomic::Ordering::SeqCst)
    {
        return Ok(());
    }
    Err("尚未打开项目：请让用户在聿Canvas 中创建或打开一个项目，然后重试".to_string())
}

/// Agent 状态：进入一次真实调用（ping/调试工具不算）。进程内的内置助手回合也用它
pub fn busy_enter(app: &tauri::AppHandle) {
    let was = app
        .state::<AppState>()
        .agent_busy
        .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    if was == 0 {
        let _ = app.emit("agent-state", json!({ "state": "busy" }));
    }
}

/// Agent 状态：调用结束，归零即回空闲
pub fn busy_exit(app: &tauri::AppHandle) {
    let was = app
        .state::<AppState>()
        .agent_busy
        .fetch_sub(1, std::sync::atomic::Ordering::SeqCst);
    if was == 1 {
        let _ = app.emit("agent-state", json!({ "state": "idle" }));
    }
}

/// 「读」类动作进入活动流；写类动作由 watcher 的「更新了」覆盖，避免重复记录
fn activity_of(tool: &str, args: &Value) -> Option<(&'static str, Option<String>)> {
    let file = |k: &str| args.get(k).and_then(|v| v.as_str()).map(String::from);
    match tool {
        "list_skills" => Some(("skills", None)),
        "get_skill" => Some((
            "skill",
            args.get("name").and_then(|v| v.as_str()).map(String::from),
        )),
        "list_artboards" => Some(("list", None)),
        "read_artboard" => Some(("read", file("file_name"))),
        "screenshot_artboard" => Some(("screenshot", file("file_name"))),
        "query_element" => Some(("query", file("file_name"))),
        _ => None,
    }
}

/// MCP JSON-RPC → 桥工具调用的公共执行层：会话契约、busy 计数、活动流都在这里。
/// TCP 桥（9915）与进程内 MCP HTTP（mcp_http.rs）都走它
pub fn handle_request(req: Value, app: &tauri::AppHandle) -> Value {
    let id = req.get("id").cloned();
    let Some(tool) = req.get("tool").and_then(|v| v.as_str()) else {
        return error_value(id, "缺少 tool 字段".to_string());
    };
    let args = req.get("args").cloned().unwrap_or(Value::Null);

    // ping 与 debug_* 是探测/自检，不代表 Agent 在协作
    let observed = !matches!(tool, "ping" | "debug_shell_screenshot" | "debug_shell_eval");
    if observed {
        busy_enter(app);
    }

    let outcome: Result<Value, String> = match tool {
        "ping" => Ok(json!("pong")),
        // 技能库是全局参考资源，不依赖项目（不受会话契约约束）
        "list_skills" => crate::skills::list(),
        "list_design_systems" => crate::design_systems::list(),
        "get_design_system" => crate::design_systems::get(&args),
        "get_skill" => crate::skills::get(&args),
        "get_conventions" => crate::skills::conventions(),
        "get_project" => ensure_project_opened(app).and_then(|()| {
            let st = app.state::<AppState>();
            let name = st
                .active_project
                .lock()
                .expect("active_project lock poisoned")
                .clone();
            if name.is_empty() {
                return Err("当前没有可用的项目".to_string());
            }
            Ok(json!({
                "name": name,
                "path": crate::state::project_path(&name).to_string_lossy(),
            }))
        }),
        "list_artboards" => ensure_project_opened(app).and_then(|()| {
            commands::list_artboards(app.state::<AppState>())
                .map(|boards| serde_json::to_value(boards).expect("serialize artboards"))
        }),
        "read_artboard" => ensure_project_opened(app)
            .and_then(|()| param_str(&args, "file_name"))
            .and_then(|f| commands::read_artboard(app.state::<AppState>(), f))
            .map(Value::String),
        "write_artboard" => ensure_project_opened(app).and_then(|()| (|| -> Result<Value, String> {
            let f = param_str(&args, "file_name")?;
            let content = param_str(&args, "content")?;
            let path = commands::write_artboard(app.state::<AppState>(), f.clone(), content)?;
            crate::watcher::mark_agent_written(&f);
            Ok(Value::String(path))
        })()),
        "register_artboard" => ensure_project_opened(app).and_then(|()| (|| -> Result<Value, String> {
            let f = param_str(&args, "file_name")?;
            let title = args.get("title").and_then(|v| v.as_str()).map(String::from);
            let path = commands::register_artboard(app.state::<AppState>(), f.clone(), title)?;
            crate::watcher::mark_agent_written(&f);
            Ok(Value::String(path))
        })()),
        "delete_artboard" => ensure_project_opened(app).and_then(|()| (|| -> Result<Value, String> {
            let f = param_str(&args, "file_name")?;
            commands::delete_artboard(app.state::<AppState>(), f.clone())?;
            crate::watcher::mark_agent_written(&f);
            let project = app.state::<AppState>().active_project.lock()
                .expect("active_project lock poisoned").clone();
            crate::watcher::emit_changed(app, &project, &f)?;
            Ok(json!(true))
        })()),
        "rename_artboard" => ensure_project_opened(app).and_then(|()| (|| -> Result<Value, String> {
            let f = param_str(&args, "file_name")?;
            let n = param_str(&args, "new_name")?;
            let path = commands::rename_artboard(app.state::<AppState>(), f.clone(), n.clone())?;
            let project = app.state::<AppState>().active_project.lock()
                .expect("active_project lock poisoned").clone();
            crate::watcher::emit_changed(app, &project, &n)?;
            Ok(json!(path))
        })()),
        // Agent 主动通知：精确触发一次刷新（兜底是 watcher 防抖）
        "notify_ready" => ensure_project_opened(app).and_then(|()| (|| -> Result<Value, String> {
            let f = param_str(&args, "file_name")?;
            let name = commands::validate_file_name(&f)?;
            let project = app
                .state::<AppState>()
                .active_project
                .lock()
                .expect("active_project lock poisoned")
                .clone();
            crate::watcher::emit_changed(app, &project, &name)?;
            Ok(json!(true))
        })()),
        "screenshot_artboard" => ensure_project_opened(app).and_then(|()| (|| -> Result<Value, String> {
            let f = param_str(&args, "file_name")?;
            let png = app.state::<Inspector>().screenshot(&f)?;
            Ok(json!({
                "file_name": f,
                "mime": "image/png",
                "data_base64": base64::engine::general_purpose::STANDARD.encode(png),
            }))
        })()),
        "query_element" => ensure_project_opened(app).and_then(|()| (|| -> Result<Value, String> {
            let f = param_str(&args, "file_name")?;
            let x = param_f64(&args, "x")?;
            let y = param_f64(&args, "y")?;
            let info = app.state::<Inspector>().query_element(&f, x, y)?;
            Ok(json!({ "file_name": f, "element": info }))
        })()),
        // 风格规范：项目的品牌契约（资源面板同源）。尚无规范时引导 Agent 先与用户确认风格再写入
        "get_design_spec" => ensure_project_opened(app).and_then(|()| {
            let spec = commands::get_design_spec(app.state::<AppState>())?;
            if spec.design_md.trim().is_empty() && spec.tokens_css.trim().is_empty() {
                return Err(
                    "项目还没有风格规范。请先与用户确认设计风格，再调用 save_design_spec 写入（design.md 风格法则 + tokens.css 设计令牌）。".to_string(),
                );
            }
            Ok(json!({
                "name": spec.name,
                "design_md": spec.design_md,
                "tokens_css": spec.tokens_css,
            }))
        }),
        "save_design_spec" => ensure_project_opened(app).and_then(|()| (|| -> Result<Value, String> {
            let design_md = args.get("design_md").and_then(|v| v.as_str()).unwrap_or("");
            let tokens_css = args.get("tokens_css").and_then(|v| v.as_str()).unwrap_or("");
            if design_md.trim().is_empty() && tokens_css.trim().is_empty() {
                return Err("design_md 与 tokens_css 不能同时为空".to_string());
            }
            commands::save_design_spec(&app.state::<AppState>(), design_md.to_string(), tokens_css.to_string())?;
            let _ = app.emit("resources-changed", json!({}));
            let mut hints: Vec<String> = Vec::new();
            if hex_in_design_md(design_md) {
                hints.push("design.md 中不应出现具体色值——请改用令牌名引用样式，值只写在 tokens.css".to_string());
            }
            if non_ascii_token_name(tokens_css) {
                hints.push("令牌名称应使用英文——发现非 ASCII 名称，请改为英文命名".to_string());
            }
            let missing = unregistered_tokens(design_md, tokens_css);
            if !missing.is_empty() {
                let list = missing.iter().take(5).cloned().collect::<Vec<_>>().join("、");
                let more = if missing.len() > 5 { " 等" } else { "" };
                hints.push(format!("以下令牌未在 design.md 的「令牌对照」登记中文名：{list}{more}"));
            }
            let files = commands::list_artboards(app.state::<AppState>())?
                .into_iter()
                .map(|b| b.file_name)
                .collect::<Vec<_>>();
            hints.extend(artboard_registry_hints(design_md, &files));
            Ok(json!({ "hints": hints }))
        })()),
        "save_component" => ensure_project_opened(app).and_then(|()| (|| -> Result<Value, String> {
            let f = param_str(&args, "file_name")?;
            let content = param_str(&args, "content")?;
            let name = commands::parse_artboard_name(&content, "");
            commands::save_component(&app.state::<AppState>(), f, content)?;
            let _ = app.emit("resources-changed", json!({}));
            let _ = app.emit("agent-activity", json!({ "action": "component", "file": name }));
            Ok(json!(true))
        })()),
        // 调试视图专用：读取应用现有窗口的当前 URL（主线程安全）
        "debug_window_url" => (|| -> Result<Value, String> {
            let label = param_str(&args, "label")?;
            let url = app.state::<Inspector>().window_url(&label)?;
            Ok(json!({ "label": label, "url": url }))
        })(),
        // 调试视图专用：截取应用现有窗口（main/design），用于无屏幕录制权限时自检壳 UI
        "debug_shell_screenshot" => (|| -> Result<Value, String> {
            let label = param_str(&args, "label")?;
            let png = app.state::<Inspector>().screenshot_window(&label)?;
            let path = std::env::temp_dir().join(format!("mlc-shell-{label}.png"));
            let path = path.to_string_lossy().to_string();
            std::fs::write(&path, &png).map_err(|e| format!("写入截图失败: {e}"))?;
            Ok(json!({ "label": label, "path": path, "bytes": png.len() }))
        })(),
        // 调试视图专用：在应用现有窗口里执行 JS 并回传结果
        "debug_shell_eval" => (|| -> Result<Value, String> {
            let label = param_str(&args, "label")?;
            let script = param_str(&args, "script")?;
            let value = app.state::<Inspector>().eval_in_window(&label, &script)?;
            Ok(json!({ "label": label, "value": value }))
        })(),
        other => Err(format!("未知工具: {other}")),
    };

    if observed {
        busy_exit(app);
        if let Some((action, file)) = activity_of(tool, &args) {
            if outcome.is_ok() {
                let _ = app.emit("agent-activity", json!({ "action": action, "file": file }));
            }
        }
        // 规范的写入不走 watcher（监听的是 artboards/），活动流在这里补
        if outcome.is_ok() && tool == "save_design_spec" {
            let _ = app.emit("agent-activity", json!({ "action": "spec", "file": null }));
        }
    }

    match outcome {
        Ok(result) => json!({ "id": id, "ok": true, "result": result }),
        Err(error) => error_value(id, error),
    }
}
