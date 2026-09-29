//! MCP 桥：监听 127.0.0.1:9915，NDJSON 协议。
//! mcp/server.mjs（stdio MCP 服务器）把工具调用转发到这里。
//! 请求: {"id": "...", "tool": "...", "args": {...}}
//! 响应: {"id": "...", "ok": true, "result": ...} | {"id": "...", "ok": false, "error": "..."}

use base64::Engine as _;
use serde_json::{json, Value};
use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use tauri::Manager;

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

fn error_value(id: Option<Value>, error: String) -> Value {
    json!({ "id": id, "ok": false, "error": error })
}

fn handle_request(req: Value, app: &tauri::AppHandle) -> Value {
    let id = req.get("id").cloned();
    let Some(tool) = req.get("tool").and_then(|v| v.as_str()) else {
        return error_value(id, "缺少 tool 字段".to_string());
    };
    let args = req.get("args").cloned().unwrap_or(Value::Null);

    let outcome: Result<Value, String> = match tool {
        "ping" => Ok(json!("pong")),
        "list_artboards" => commands::list_artboards(app.state::<AppState>())
            .map(|boards| serde_json::to_value(boards).expect("serialize artboards")),
        "read_artboard" => param_str(&args, "file_name")
            .and_then(|f| commands::read_artboard(app.state::<AppState>(), f))
            .map(Value::String),
        "write_artboard" => (|| -> Result<Value, String> {
            let f = param_str(&args, "file_name")?;
            let content = param_str(&args, "content")?;
            commands::write_artboard(app.state::<AppState>(), f, content).map(Value::String)
        })(),
        "register_artboard" => (|| -> Result<Value, String> {
            let f = param_str(&args, "file_name")?;
            let title = args.get("title").and_then(|v| v.as_str()).map(String::from);
            commands::register_artboard(app.state::<AppState>(), f, title).map(Value::String)
        })(),
        // Agent 主动通知：精确触发一次刷新（兜底是 watcher 防抖）
        "notify_ready" => (|| -> Result<Value, String> {
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
        })(),
        "screenshot_artboard" => (|| -> Result<Value, String> {
            let f = param_str(&args, "file_name")?;
            let png = app.state::<Inspector>().screenshot(&f)?;
            Ok(json!({
                "file_name": f,
                "mime": "image/png",
                "data_base64": base64::engine::general_purpose::STANDARD.encode(png),
            }))
        })(),
        "query_element" => (|| -> Result<Value, String> {
            let f = param_str(&args, "file_name")?;
            let x = param_f64(&args, "x")?;
            let y = param_f64(&args, "y")?;
            let info = app.state::<Inspector>().query_element(&f, x, y)?;
            Ok(json!({ "file_name": f, "element": info }))
        })(),
        // 调试视图专用：截取应用现有窗口（main/design），用于无屏幕录制权限时自检壳 UI
        "debug_shell_screenshot" => (|| -> Result<Value, String> {
            let label = param_str(&args, "label")?;
            let png = app.state::<Inspector>().screenshot_window(&label)?;
            let path = format!("/tmp/mlc-shell-{label}.png");
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

    match outcome {
        Ok(result) => json!({ "id": id, "ok": true, "result": result }),
        Err(error) => error_value(id, error),
    }
}
