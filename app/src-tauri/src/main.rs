#![windows_subsystem = "windows"]

mod agent;
mod artboard_http;
mod commands;
mod design_systems;
mod inspector;
mod mcp_http;
mod resources;
mod skills;
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
            resources::init(app.path().resource_dir().ok().map(|p| p.join("resources")));

            let default_project = state::setup_default();
            handle.manage(AppState {
                active_project: std::sync::Mutex::new(default_project),
                agent_busy: std::sync::atomic::AtomicUsize::new(0),
                project_opened: std::sync::atomic::AtomicBool::new(false),
                agent_client: std::sync::Mutex::new(state::load_presence()),
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
            mcp_http::start(handle.clone());
            artboard_http::start();

            // 兜底：本机偶发首次导航被吞（WKWebView URL 为空、页面白屏）。
            // 启动后延迟重导航一次；若页面已正常加载则无副作用。
            let nav_handle = handle.clone();
            std::thread::spawn(move || {
                std::thread::sleep(std::time::Duration::from_secs(2));
                if let Some(win) = nav_handle.get_webview_window("main") {
                    let url: tauri::Url = "tauri://localhost/index.html"
                        .parse()
                        .expect("parse app url");
                    let _ = win.navigate(url);
                }
            });

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
                        .title("聿Canvas")
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
            commands::import_project,
            commands::open_project,
            commands::delete_project,
            commands::pick_directory,
            commands::get_active_project,
            commands::agent_client,
            commands::debug_log,
            commands::list_artboards,
            commands::read_artboard,
            commands::write_artboard,
            commands::register_artboard,
            commands::delete_artboard,
            commands::rename_artboard,
            commands::set_artboard_title,
            commands::save_artboard_order,
            commands::get_design_spec,
            commands::list_components,
            commands::list_design_systems,
            commands::read_design_system,
            agent::agent_get_settings,
            agent::agent_save_config,
            agent::agent_delete_config,
            agent::agent_set_current,
            agent::agent_set_default_parent,
            agent::agent_model_catalog,
            agent::agent_test_connection,
            agent::agent_chat_turn,
            agent::agent_chat_cancel,
            agent::agent_chat_compact
        ])
        .run(tauri::generate_context!())
        .expect("error while running YuCanvas");
}
