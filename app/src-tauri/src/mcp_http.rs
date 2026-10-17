//! MCP over Streamable HTTP：应用进程内直接对外提供 MCP 服务（原 mcp/server.mjs 的 Rust 化）。
//! 工具执行复用 ws_server::handle_request（会话契约、busy 计数、活动流都在那层），
//! 这里只做协议：JSON-RPC ↔ 桥请求的转换、initialize 的接入登记、鉴权与错误码。

use crate::state::AppState;
use serde_json::{json, Value};
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use tauri::{AppHandle, Emitter, Manager};

const SERVER_INFO: &str = "mlcanvas";
const SERVER_VERSION: &str = "0.3.0";
const MCP_PATH: &str = "/mcp";

fn tools_json() -> &'static str {
    include_str!("mcp_tools.json")
}

fn env_or(key: &str, default: &str) -> String {
    std::env::var(key).unwrap_or_else(|_| default.to_string())
}

fn token() -> Option<String> {
    std::env::var("MLCANVAS_TOKEN").ok().filter(|t| !t.is_empty())
}

/// 接入登记：写 AppState、落盘、广播事件
fn register_presence(app: &AppHandle, client: &str, version: &str) {
    *app.state::<AppState>()
        .agent_client
        .lock()
        .expect("agent_client lock poisoned") = client.to_string();
    crate::state::save_presence(client);
    let _ = app.emit("agent-presence", json!({ "client": client, "version": version }));
}

/// 工具结果 → MCP content（沿用原 JS 层的格式化规则）
fn to_content(tool: &str, args: &Value, result: Value) -> Value {
    let text = |t: &str| json!([{ "type": "text", "text": t }]);
    match tool {
        "get_project" => text(&format!(
            "当前项目：{}\n画板目录：{}\n下一步：调用 get_conventions 装载工作流程与规范，list_skills 了解可用设计技能",
            result["name"].as_str().unwrap_or(""),
            result["path"].as_str().unwrap_or("")
        )),
        "list_artboards" => {
            let boards = result.as_array().cloned().unwrap_or_default();
            if boards.is_empty() {
                return text("artboards/ 目录为空");
            }
            let lines: Vec<String> = boards
                .iter()
                .map(|b| {
                    format!(
                        "{} — {}",
                        b["file_name"].as_str().unwrap_or(""),
                        b["path"].as_str().unwrap_or("")
                    )
                })
                .collect();
            text(&lines.join("\n"))
        }
        "read_artboard" => text(result.as_str().unwrap_or("")),
        "write_artboard" => text(&format!(
            "已写入 {}，画板将自动刷新——用 screenshot_artboard 自检渲染（色彩/字阶/间距/色差），确认无误再做下一块",
            result.as_str().unwrap_or("")
        )),
        "register_artboard" => text(&format!("已创建画板 {}", result.as_str().unwrap_or(""))),
        "delete_artboard" => text(&format!(
            "已删除画板 {}",
            args["file_name"].as_str().unwrap_or("")
        )),
        "rename_artboard" => text(&format!(
            "已重命名：{} → {}",
            args["file_name"].as_str().unwrap_or(""),
            args["new_name"].as_str().unwrap_or("")
        )),
        "notify_ready" => text(&format!("已通知刷新 {}", args["file_name"].as_str().unwrap_or(""))),
        "screenshot_artboard" => json!([
            { "type": "text", "text": format!("{} 渲染截图（image/png）", args["file_name"].as_str().unwrap_or("")) },
            { "type": "image", "data": result["data_base64"].as_str().unwrap_or(""), "mimeType": "image/png" }
        ]),
        "query_element" => text(&serde_json::to_string_pretty(&result["element"]).unwrap_or_default()),
        "get_design_spec" => text(&format!(
            "# 风格规范：{}\n\n== design.md ==\n{}\n\n== tokens.css ==\n{}",
            result["name"].as_str().unwrap_or("（未命名）"),
            result["design_md"].as_str().unwrap_or(""),
            result["tokens_css"].as_str().unwrap_or("")
        )),
        "save_design_spec" => {
            let mut msg = "已保存风格规范，资源面板与画板令牌引用将随之更新".to_string();
            if let Some(hints) = result["hints"].as_array() {
                for h in hints.iter().filter_map(|h| h.as_str()) {
                    msg.push_str(&format!("\n提示：{h}"));
                }
            }
            text(&msg)
        }
        "save_component" => text(&format!("已保存组件 {}", args["file_name"].as_str().unwrap_or(""))),
        "get_conventions" => text(result["content"].as_str().unwrap_or("")),
        "get_skill" => text(result["content"].as_str().unwrap_or("")),
        "get_design_system" => text(result["content"].as_str().unwrap_or("")),
        "list_design_systems" => {
            let mut out = String::new();
            let mut last_cat = String::new();
            for s in result["packs"].as_array().cloned().unwrap_or_default() {
                let cat = s["category"].as_str().unwrap_or("").to_string();
                if cat != last_cat {
                    if !out.is_empty() { out.push('\n'); }
                    out.push_str(&format!("## {cat}\n"));
                    last_cat = cat;
                }
                let comps = if s["hasComponents"].as_bool().unwrap_or(false) { " · 含组件页" } else { "" };
                out.push_str(&format!(
                    "- {} — {}{}\n",
                    s["id"].as_str().unwrap_or(""),
                    s["description"].as_str().unwrap_or(""),
                    comps
                ));
            }
            text(&format!("共 {} 个风格包：\n\n{}", result["total"], out))
        }
        "list_skills" => {
            let mut out = String::new();
            let mut last_cat = String::new();
            for s in result["skills"].as_array().cloned().unwrap_or_default() {
                let cat = s["category"].as_str().unwrap_or("").to_string();
                if cat != last_cat {
                    if !out.is_empty() {
                        out.push('\n');
                    }
                    out.push_str(&format!("## {cat}\n"));
                    last_cat = cat;
                }
                let body = if s["has_body"].as_bool().unwrap_or(false) {
                    ""
                } else {
                    "（指针卡）"
                };
                out.push_str(&format!(
                    "- {} {body} — {}\n",
                    s["dir"].as_str().unwrap_or(""),
                    s["description"].as_str().unwrap_or("")
                ));
            }
            text(&format!("共 {} 个技能：\n\n{}", result["total"], out))
        }
        _ => text(&serde_json::to_string(&result).unwrap_or_default()),
    }
}

/// JSON-RPC 请求 → JSON-RPC 响应 Value；通知返回 None
fn dispatch(app: &AppHandle, body: &Value) -> Result<Option<Value>, (i64, String)> {
    let method = body.get("method").and_then(|m| m.as_str()).unwrap_or("");
    let id = body.get("id").cloned();

    // 通知（无 id）不产生响应
    if id.is_none() || id == Some(Value::Null) {
        return Ok(None);
    }

    let result = match method {
        "initialize" => {
            let (client, version) = body
                .pointer("/params/clientInfo/name")
                .and_then(|n| n.as_str())
                .map(|n| (n.to_string(), body.pointer("/params/clientInfo/version").and_then(|v| v.as_str()).unwrap_or("").to_string()))
                .unwrap_or_default();
            if !client.is_empty() {
                register_presence(app, &client, &version);
            }
            json!({
                "protocolVersion": "2025-03-26",
                "capabilities": { "tools": {} },
                "serverInfo": { "name": SERVER_INFO, "version": SERVER_VERSION }
            })
        }
        "tools/list" => {
            let tools: Value = serde_json::from_str(tools_json()).expect("mcp_tools.json 是合法 JSON");
            json!({ "tools": tools })
        }
        "tools/call" => {
            let name = body.pointer("/params/name").and_then(|n| n.as_str()).unwrap_or("").to_string();
            let args = body.pointer("/params/arguments").cloned().unwrap_or(Value::Null);
            let req = json!({ "id": id.clone().unwrap_or(Value::Null), "tool": name, "args": args });
            let reply = crate::ws_server::handle_request(req, app);
            if reply["ok"].as_bool().unwrap_or(false) {
                let content = to_content(&name, &args, reply["result"].clone());
                json!({ "content": content })
            } else {
                let err = reply["error"].as_str().unwrap_or("未知桥错误");
                json!({ "content": [{ "type": "text", "text": err }], "isError": true })
            }
        }
        "ping" => json!({}),
        other => return Err((-32601, format!("方法不存在: {other}"))),
    };
    Ok(Some(json!({ "jsonrpc": "2.0", "id": id, "result": result })))
}

fn http_response(status: &str, body: &str, content_type: &str, close: bool) -> Vec<u8> {
    let conn = if close { "Connection: close\r\n" } else { "" };
    format!(
        "HTTP/1.1 {status}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\n{conn}\r\n{body}",
        body.len()
    )
    .into_bytes()
}

fn serve_json(app: &AppHandle, body: &Value, close: bool) -> Vec<u8> {
    match dispatch(app, body) {
        Ok(Some(reply)) => http_response(
            "200 OK",
            &reply.to_string(),
            "application/json",
            close,
        ),
        Ok(None) => http_response("202 Accepted", "", "text/plain", close),
        Err((code, message)) => {
            let status = match code {
                -32601 => "404 Not Found",
                _ => "400 Bad Request",
            };
            let reply = json!({ "jsonrpc": "2.0", "id": Value::Null, "error": { "code": code, "message": message } });
            http_response(status, &reply.to_string(), "application/json", close)
        }
    }
}

fn handle_connection(app: AppHandle, stream: TcpStream) {
    let peer_ok = stream.peek(&mut [0u8; 1]).is_ok();
    if !peer_ok {
        return;
    }
    let mut reader = BufReader::new(match stream.try_clone() {
        Ok(s) => s,
        Err(_) => return,
    });
    let mut writer = stream;

    loop {
        // 请求行
        let mut line = String::new();
        if reader.read_line(&mut line).map(|n| n == 0).unwrap_or(true) {
            return;
        }
        let mut parts = line.split_whitespace();
        let method = parts.next().unwrap_or("").to_string();
        let path = parts.next().unwrap_or("").to_string();

        // 头部
        let mut content_length = 0usize;
        let mut accept_ok = false;
        let mut auth_ok = token().is_none();
        let mut keep_alive = true;
        loop {
            let mut header = String::new();
            if reader.read_line(&mut header).map(|n| n <= 2).unwrap_or(true) {
                break;
            }
            let header = header.trim_end();
            if header.is_empty() {
                break;
            }
            let lower = header.to_ascii_lowercase();
            if let Some(v) = lower.strip_prefix("content-length:") {
                content_length = v.trim().parse().unwrap_or(0);
            } else if lower.starts_with("accept:") && header[7..].to_ascii_lowercase().contains("application/json") {
                accept_ok = true;
            } else if lower.starts_with("authorization:") {
                if let Some(t) = token() {
                    auth_ok = header[15..].trim() == format!("Bearer {t}");
                }
            } else if lower.starts_with("connection:") && lower.contains("close") {
                keep_alive = false;
            }
        }

        // Body
        let mut body_bytes = vec![0u8; content_length];
        if content_length > 0 && reader.read_exact(&mut body_bytes).is_err() {
            return;
        }

        let path_only = path.split('?').next().unwrap_or("");
        let response = if path_only != MCP_PATH {
            http_response("404 Not Found", "{\"error\":\"not found\"}", "application/json", !keep_alive)
        } else if !auth_ok {
            http_response("401 Unauthorized", "{\"error\":\"unauthorized\"}", "application/json", !keep_alive)
        } else if method != "POST" {
            http_response(
                "405 Method Not Allowed",
                "{\"error\":\"method not allowed\"}",
                "application/json",
                !keep_alive,
            )
        } else if !accept_ok {
            http_response(
                "406 Not Acceptable",
                "{\"error\":\"Accept 必须包含 application/json\"}",
                "application/json",
                !keep_alive,
            )
        } else {
            match serde_json::from_slice::<Value>(&body_bytes) {
                Err(_) => {
                    let reply = json!({ "jsonrpc": "2.0", "id": Value::Null, "error": { "code": -32700, "message": "Parse error" } });
                    http_response("400 Bad Request", &reply.to_string(), "application/json", !keep_alive)
                }
                // 批量请求不支持（JSON-RPC 2.0 批量在 MCP 中已弃用）
                Ok(Value::Array(_)) => {
                    let reply = json!({ "jsonrpc": "2.0", "id": Value::Null, "error": { "code": -32600, "message": "不支持批量请求" } });
                    http_response("400 Bad Request", &reply.to_string(), "application/json", !keep_alive)
                }
                Ok(body) => serve_json(&app, &body, !keep_alive),
            }
        };

        if writer.write_all(&response).is_err() {
            return;
        }
        let _ = writer.flush();
        if !keep_alive {
            return;
        }
    }
}

/// 启动 MCP HTTP 服务（应用进程内，随应用生灭）
pub fn start(app: AppHandle) {
    std::thread::spawn(move || {
        let host = env_or("MLCANVAS_HOST", "127.0.0.1");
        let port = env_or("MLCANVAS_PORT", "9916");
        let listener = match TcpListener::bind(format!("{}:{}", host, port)) {
            Ok(l) => l,
            Err(e) => {
                eprintln!("[mlcanvas] MCP http 启动失败: {e}");
                return;
            }
        };
        eprintln!("[mlcanvas] MCP http listening on http://{}:{}/mcp", host, port);
        for stream in listener.incoming().flatten() {
            let app = app.clone();
            std::thread::spawn(move || handle_connection(app, stream));
        }
    });
}
