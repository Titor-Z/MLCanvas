#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod commands;
mod inspector;
mod state;
mod watcher;
mod ws_server;

use inspector::Inspector;
use state::AppState;
use tauri::{Listener, Manager};

fn main() {
    tauri::Builder::default()
        .setup(|app| {
            let handle = app.handle().clone();

            let default_project = state::setup_default();
            handle.manage(AppState {
                active_project: std::sync::Mutex::new(default_project),
            });
            handle.manage(Inspector::new(handle.clone()));

            // 隐藏窗口里 eval 的结果经事件回传（asset 页面不能调用应用命令）
            let listener_handle = handle.clone();
            handle.listen_any("mlcanvas-js-result", move |event| {
                listener_handle
                    .state::<Inspector>()
                    .push_js_result(event.payload().to_string());
            });
            watcher::start(handle.clone());
            ws_server::start(handle);
            Ok(())
        })
        // 设计窗口关闭时回到主页，而不是退出应用
        .on_window_event(|window, event| {
            if window.label() == "design" {
                if let tauri::WindowEvent::Destroyed = event {
                    let app = window.app_handle();
                    if app.get_webview_window("main").is_none() {
                        let _ = tauri::WebviewWindowBuilder::new(
                            app,
                            "main",
                            tauri::WebviewUrl::App("index.html".into()),
                        )
                        .title("MLCanvas")
                        .inner_size(960.0, 640.0)
                        .min_inner_size(720.0, 480.0)
                        .decorations(false)
                        .build();
                    }
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            commands::list_projects,
            commands::create_project,
            commands::open_project,
            commands::get_active_project,
            commands::bridge_status,
            commands::list_artboards,
            commands::read_artboard,
            commands::write_artboard,
            commands::register_artboard
        ])
        .run(tauri::generate_context!())
        .expect("error while running MLCanvas");
}
