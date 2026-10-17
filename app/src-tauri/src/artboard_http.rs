//! 画板资源面：应用进程内的本机静态服务（中间拦截层的一半）。
//! iframe 与截图/审查窗口统一从这里加载画板——相对路径（../tokens.css、图片、组件）由
//! HTTP 天然解析，asset 协议子资源加载不可靠的问题不复存在；HTML 响应注入画板运行时
//! （/__rt.js：导航拦截 + 样式失败可见性），服务时注入、不改用户文件。
//! 安全模型与 MCP 面不同：只绑 127.0.0.1、无鉴权（<link> 带不了 Bearer）、只读、
//! 仅限索引内项目目录——画板资源不需要也不应该暴露局域网。

use crate::state::AppState;
use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use tauri::Manager;

/// 实际监听端口；0 = 服务未启动（调用方回退 asset 协议）
static PORT: AtomicUsize = AtomicUsize::new(0);

pub fn port() -> u16 {
    PORT.load(Ordering::SeqCst) as u16
}

const RUNTIME_JS: &str = include_str!("artboard_runtime.js");

fn mime_of(path: &Path) -> &'static str {
    match path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase()
        .as_str()
    {
        "html" | "htm" => "text/html; charset=utf-8",
        "css" => "text/css; charset=utf-8",
        "js" | "mjs" => "text/javascript; charset=utf-8",
        "json" => "application/json",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "svg" => "image/svg+xml",
        "ico" => "image/x-icon",
        "woff" => "font/woff",
        "woff2" => "font/woff2",
        "ttf" | "otf" => "font/ttf",
        "txt" | "md" => "text/plain; charset=utf-8",
        _ => "application/octet-stream",
    }
}

/// 逐字节 percent-encode（保留 RFC 3986 unreserved 与路径分隔符由调用方处理）
fn encode_segment(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char)
            }
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

fn decode_segment(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 3 <= bytes.len() {
            let hex = std::str::from_utf8(&bytes[i + 1..i + 3]).ok();
            if let Some(b) = hex.and_then(|h| u8::from_str_radix(h, 16).ok()) {
                out.push(b);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// 画板页面 URL：资源服务可用时返回 http 地址；否则回退 asset 协议
pub fn page_url(app: &tauri::AppHandle, path: &Path) -> String {
    let project = app
        .state::<AppState>()
        .active_project
        .lock()
        .expect("active_project lock poisoned")
        .clone();
    match artboard_url(&project, path) {
        Some(u) => u,
        None => asset_url(path),
    }
}

/// 相对项目根的画板/资源 http 地址；服务未启动返回 None
pub fn artboard_url(project: &str, path: &Path) -> Option<String> {
    let p = port();
    if p == 0 {
        return None;
    }
    let root = crate::state::project_path(project);
    let rel = path.strip_prefix(&root).ok()?;
    let mut url = format!(
        "http://127.0.0.1:{}/files/{}",
        p,
        encode_segment(project)
    );
    for seg in rel.iter() {
        url.push('/');
        url.push_str(&encode_segment(&seg.to_string_lossy()));
    }
    Some(url)
}

/// 本地文件路径 → asset 协议地址（回退路径；macOS: asset://localhost/<percent-encoded path>）
pub fn asset_url(path: &Path) -> String {
    let mut encoded = String::with_capacity(path.to_string_lossy().len());
    for b in path.to_string_lossy().as_bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' | b'/' => {
                encoded.push(*b as char)
            }
            _ => encoded.push_str(&format!("%{b:02X}")),
        }
    }
    format!("asset://localhost/{encoded}")
}

/// 404/403 用可见页面——失败必须能被看见，空白是最贵的错误
fn error_page(status: &str, title: &str, detail: &str) -> Vec<u8> {
    let body = format!(
        "<!doctype html><html lang=\"zh\"><meta charset=\"utf-8\"><style>\
         html{{background:#f6f6f8;font:14px/1.6 -apple-system,'PingFang SC',sans-serif;color:#3a3a45}}\
         body{{margin:0;display:grid;place-items:center;height:100vh}}\
         div{{text-align:center}}b{{font-size:17px}}p{{color:#8a8a95;margin:6px 0 0}}</style>\
         <div><b>{title}</b><p>{detail}</p></div></html>"
    );
    response(status, body.as_bytes(), "text/html; charset=utf-8")
}

fn response(status: &str, body: &[u8], content_type: &str) -> Vec<u8> {
    let mut head = format!(
        "HTTP/1.1 {status}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nCache-Control: no-store\r\nConnection: close\r\n\r\n",
        body.len()
    )
    .into_bytes();
    head.extend_from_slice(body);
    head
}

/// 在 HTML 的 <head> 最前注入画板运行时（服务时注入，不改用户文件）
fn inject_runtime(html: &str) -> String {
    const TAG: &str = "<script src=\"/__rt.js\"></script>";
    let lower = html.to_ascii_lowercase();
    if let Some(pos) = lower.find("<head") {
        let end = lower[pos..].find('>').map(|i| pos + i + 1).unwrap_or(pos);
        let mut out = String::with_capacity(html.len() + TAG.len());
        out.push_str(&html[..end]);
        out.push_str(TAG);
        out.push_str(&html[end..]);
        out
    } else {
        format!("{TAG}{html}")
    }
}

/// GET /files/<项目名>/<相对路径>：经索引解析项目目录，防穿越后返回文件
fn serve_file(raw_path: &str) -> Vec<u8> {
    let rest = match raw_path.strip_prefix("/files/") {
        Some(r) if !r.is_empty() => r,
        _ => return error_page("404 Not Found", "资源不存在", "请求的画板资源没有找到"),
    };
    let mut segs = rest.split('/').filter(|s| !s.is_empty()).map(decode_segment);
    let project = segs.next().unwrap_or_default();
    let rel: PathBuf = segs.collect();
    if project.is_empty() || rel.as_os_str().is_empty() {
        return error_page("404 Not Found", "资源不存在", "请求的画板资源没有找到");
    }

    let root = crate::state::project_path(&project);
    let Ok(root) = root.canonicalize() else {
        return error_page("404 Not Found", "项目不存在", "索引里没有这个项目或目录已移动");
    };
    let target = root.join(&rel);
    let Ok(target) = target.canonicalize() else {
        return error_page(
            "404 Not Found",
            "资源不存在",
            "画板引用的文件在项目里没有找到",
        );
    };
    if !target.starts_with(&root) {
        return error_page("403 Forbidden", "禁止访问", "只能访问项目目录内的资源");
    }
    if !target.is_file() {
        return error_page("404 Not Found", "资源不存在", "请求的画板资源没有找到");
    }

    let Ok(bytes) = std::fs::read(&target) else {
        return error_page("404 Not Found", "资源不存在", "文件读取失败");
    };

    if mime_of(&target).starts_with("text/html") {
        let html = String::from_utf8_lossy(&bytes);
        return response(
            "200 OK",
            inject_runtime(&html).as_bytes(),
            "text/html; charset=utf-8",
        );
    }
    response("200 OK", &bytes, mime_of(&target))
}

fn handle_connection(mut stream: TcpStream) {
    let mut reader = BufReader::new(match stream.try_clone() {
        Ok(s) => s,
        Err(_) => return,
    });
    let mut line = String::new();
    if reader.read_line(&mut line).unwrap_or(0) == 0 {
        return;
    }
    let mut parts = line.split_whitespace();
    let method = parts.next().unwrap_or("");
    let path = parts.next().unwrap_or("").to_string();

    // 消费剩余头部与 body（本服务只读，无需解析）
    loop {
        let mut h = String::new();
        if reader.read_line(&mut h).unwrap_or(0) <= 2 {
            break;
        }
    }

    let path_only = path.split('?').next().unwrap_or("");
    let response = match (method, path_only) {
        ("GET", "/__rt.js") => response(
            "200 OK",
            RUNTIME_JS.as_bytes(),
            "text/javascript; charset=utf-8",
        ),
        ("GET", p) if p.starts_with("/files/") => serve_file(p),
        ("GET", _) => error_page("404 Not Found", "资源不存在", "未知路径"),
        _ => response("405 Method Not Allowed", b"method not allowed", "text/plain"),
    };
    let _ = stream.write_all(&response);
    let _ = stream.flush();
}

/// 启动画板资源服务（只绑 127.0.0.1；9917 起自动顺延 4 个端口）
pub fn start() {
    std::thread::spawn(move || {
        let mut bound = None;
        for port in 9917..=9921 {
            match TcpListener::bind(("127.0.0.1", port)) {
                Ok(l) => {
                    bound = Some((l, port));
                    break;
                }
                Err(_) => continue,
            }
        }
        let Some((listener, port)) = bound else {
            eprintln!("[mlcanvas] 画板资源服务启动失败（9917-9921 均被占用），画布回退 asset 协议");
            return;
        };
        PORT.store(port as usize, Ordering::SeqCst);
        eprintln!("[mlcanvas] 画板资源服务 listening on http://127.0.0.1:{port}/files/");
        for stream in listener.incoming().flatten() {
            std::thread::spawn(move || handle_connection(stream));
        }
    });
}
