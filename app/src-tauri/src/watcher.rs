//! 递归监听 ~/MLCanvas 项目根目录，画板文件变化后向前端广播 {project, file}。
//! 递归监听根目录免去项目切换时重新挂载 watcher。

use notify::{recommended_watcher, RecursiveMode, Watcher};
use serde_json::json;
use std::sync::mpsc;
use std::time::Duration;
use tauri::{AppHandle, Emitter};

use crate::state;

/// 防抖间隔：Agent 忘了通知时兜底刷新的等待窗口
const DEBOUNCE: Duration = Duration::from_millis(200);
/// 无事件时的轮询等待（仅为了让 recv_timeout 循环保活）
const IDLE_POLL: Duration = Duration::from_secs(3600);

pub fn start(app: AppHandle) {
    std::thread::spawn(move || {
        let (tx, rx) = mpsc::channel();
        let mut watcher = match recommended_watcher(move |res| {
            let _ = tx.send(res);
        }) {
            Ok(w) => w,
            Err(_) => return,
        };

        let root = state::projects_root();
        let _ = std::fs::create_dir_all(&root);
        if watcher.watch(&root, RecursiveMode::Recursive).is_err() {
            return;
        }

        let mut pending: Vec<(String, String)> = Vec::new(); // (project, file)
        loop {
            let timeout = if pending.is_empty() { IDLE_POLL } else { DEBOUNCE };
            match rx.recv_timeout(timeout) {
                Ok(Ok(event)) => {
                    for path in event.paths {
                        // <root>/<project>/artboards/<file>.html
                        let mut it = path.strip_prefix(&root).ok().into_iter().flat_map(|p| p.components());
                        let (Some(project), Some(dir), Some(file)) =
                            (it.next(), it.next(), it.next())
                        else {
                            continue;
                        };
                        if dir.as_os_str() != "artboards" {
                            continue;
                        }
                        let file = file.as_os_str().to_string_lossy().to_string();
                        if !file.ends_with(".html") {
                            continue;
                        }
                        let project = project.as_os_str().to_string_lossy().to_string();
                        if !pending.iter().any(|(p, f)| *p == project && *f == file) {
                            pending.push((project, file));
                        }
                    }
                }
                Ok(Err(_)) | Err(mpsc::RecvTimeoutError::Disconnected) => break,
                Err(mpsc::RecvTimeoutError::Timeout) => {
                    if !pending.is_empty() {
                        let payload: Vec<_> = pending.drain(..).collect();
                        let _ = app.emit("artboards-changed", json!({ "changes": payload }));
                    }
                }
            }
        }
    });
}

/// 供 MCP notify_ready / 其他模块复用：直接广播某个画板变化
pub fn emit_changed(app: &AppHandle, project: &str, file: &str) -> Result<(), String> {
    app.emit(
        "artboards-changed",
        json!({ "changes": [{ "project": project, "file": file }] }),
    )
    .map_err(|e| e.to_string())
}
