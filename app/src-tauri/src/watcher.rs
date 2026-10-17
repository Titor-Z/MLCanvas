//! 按活动项目挂载 watcher：监听画板文件（artboards-changed）与资源文件
//! （项目根的 design.md / tokens.css、components/ 组件 → resources-changed），防抖后广播。
//! 项目可位于任意目录（路径来自项目索引），项目切换时由 open_project 调 remount 重挂。

use notify::{recommended_watcher, RecursiveMode, Watcher};
use serde_json::json;
use std::path::PathBuf;
use std::sync::mpsc;
use std::sync::Mutex;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter};

use crate::state;

/// 防抖间隔：Agent 忘了通知时兜底刷新的等待窗口
const DEBOUNCE: Duration = Duration::from_millis(200);
/// 无事件时的轮询等待（仅为了让 recv_timeout 循环保活）
const IDLE_POLL: Duration = Duration::from_secs(3600);

static WATCHER: Mutex<Option<Box<dyn Watcher + Send>>> = Mutex::new(None);
static WATCHED: Mutex<Option<PathBuf>> = Mutex::new(None);
static WATCHED_ROOT: Mutex<Option<PathBuf>> = Mutex::new(None);
static WATCHED_COMPONENTS: Mutex<Option<PathBuf>> = Mutex::new(None);
static WATCHED_PROJECT: Mutex<String> = Mutex::new(String::new());
/// MCP 画板写入的近期标记：变化事件据此带 agent:true，活动流得以区分
/// 「Agent 更新了」与匿名的用户手改（文件系统事件本身无法区分来源）
static AGENT_WRITTEN: Mutex<Vec<(String, Instant)>> = Mutex::new(Vec::new());

/// MCP 侧刚写过某画板文件——之后 2 秒内该文件的变化事件标记 agent
pub fn mark_agent_written(file: &str) {
    AGENT_WRITTEN
        .lock()
        .expect("agent_written lock poisoned")
        .push((file.to_string(), Instant::now()));
}

fn take_agent_flag(file: &str) -> bool {
    let mut guard = AGENT_WRITTEN.lock().expect("agent_written lock poisoned");
    if let Some(pos) = guard.iter().position(|(f, _)| f == file) {
        let fresh = guard[pos].1.elapsed() < Duration::from_secs(2);
        guard.swap_remove(pos);
        return fresh;
    }
    false
}

/// 切换监听目标：挂到指定项目的 artboards/ + 项目根 + components/；name 为空 = 取消监听
pub fn remount(name: &str) {
    let mut guard = WATCHER.lock().expect("watcher lock poisoned");
    let Some(watcher) = guard.as_mut() else {
        return; // start 尚未运行
    };
    // 先取消旧目录
    for slot in [
        &WATCHED,
        &WATCHED_ROOT,
        &WATCHED_COMPONENTS,
    ] {
        if let Some(old) = slot.lock().expect("watched lock poisoned").take() {
            let _ = watcher.unwatch(&old);
        }
    }
    let mut targets: (Option<PathBuf>, Option<PathBuf>, Option<PathBuf>) = (None, None, None);
    if !name.is_empty() {
        let root = state::project_path(name);
        let artboards = root.join("artboards");
        let components = root.join("components");
        for (dir, created) in [(&artboards, true), (&root, false), (&components, true)] {
            if created {
                let _ = std::fs::create_dir_all(dir);
            }
            if let Err(e) = watcher.watch(dir, RecursiveMode::NonRecursive) {
                eprintln!("[mlcanvas] watcher 挂载失败 {}: {e}", dir.display());
            }
        }
        eprintln!("[mlcanvas:watcher] 已挂载 {name}: {} (+ root, components)", artboards.display());
        targets = (Some(artboards), Some(root), Some(components));
    }
    *WATCHED.lock().expect("watched lock poisoned") = targets.0;
    *WATCHED_ROOT.lock().expect("watched lock poisoned") = targets.1;
    *WATCHED_COMPONENTS.lock().expect("watched lock poisoned") = targets.2;
    *WATCHED_PROJECT.lock().expect("watched project lock poisoned") = name.to_string();
}

pub fn start(app: AppHandle) {
    let (tx, rx) = mpsc::channel();
    match recommended_watcher(move |res| {
        let _ = tx.send(res);
    }) {
        Ok(w) => *WATCHER.lock().expect("watcher lock poisoned") = Some(Box::new(w)),
        Err(_) => return,
    }

    std::thread::spawn(move || {
        let mut pending: Vec<(String, String)> = Vec::new(); // (project, file)
        let mut resources_dirty = false;
        loop {
            let timeout = if pending.is_empty() && !resources_dirty { IDLE_POLL } else { DEBOUNCE };
            match rx.recv_timeout(timeout) {
                Ok(Ok(event)) => {
                    let project = WATCHED_PROJECT.lock().expect("watched project lock poisoned").clone();
                    let dir = WATCHED.lock().expect("watched lock poisoned").clone();
                    let root = WATCHED_ROOT.lock().expect("watched lock poisoned").clone();
                    let comps = WATCHED_COMPONENTS.lock().expect("watched lock poisoned").clone();
                    for path in event.paths {
                        let Some(file) = path.file_name().map(|f| f.to_string_lossy().to_string()) else {
                            continue;
                        };
                        let parent = path.parent().map(|p| p.to_path_buf());
                        if parent.is_some() && parent == dir && file.ends_with(".html") {
                            // 只认直接位于监听目录下的画板文件
                            if !pending.iter().any(|(_, f)| f == &file) {
                                pending.push((project.clone(), file));
                            }
                        } else if parent.is_some() && parent == root
                            && matches!(file.as_str(), "design.md" | "tokens.css")
                        {
                            resources_dirty = true;
                        } else if comps.is_some() && parent == comps && file.ends_with(".html") {
                            resources_dirty = true;
                        }
                    }
                }
                // notify 的错误事件（如瞬时 FSEvents 异常）只记录不终止——监听循环必须活着
                Ok(Err(e)) => eprintln!("[mlcanvas:watcher] 错误事件: {e}"),
                Err(mpsc::RecvTimeoutError::Disconnected) => break,
                Err(mpsc::RecvTimeoutError::Timeout) => {
                    if !pending.is_empty() {
                        let payload: Vec<_> = pending
                            .drain(..)
                            .map(|(project, file)| {
                                let agent = take_agent_flag(&file);
                                json!({ "project": project, "file": file, "agent": agent })
                            })
                            .collect();
                        let _ = app.emit("artboards-changed", json!({ "changes": payload }));
                    }
                    if resources_dirty {
                        resources_dirty = false;
                        let _ = app.emit("resources-changed", json!({}));
                    }
                }
            }
        }
    });
}

/// 供 MCP notify_ready / 其他模块复用：直接广播某个画板变化（MCP 来源 = agent）
pub fn emit_changed(app: &AppHandle, project: &str, file: &str) -> Result<(), String> {
    app.emit(
        "artboards-changed",
        json!({ "changes": [{ "project": project, "file": file, "agent": true }] }),
    )
    .map_err(|e| e.to_string())
}
