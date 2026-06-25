mod commands;
mod codex_config;
mod db;
mod proxy;

use db::Database;
use proxy::{ProxyManager, SharedProxyManager};
use std::sync::Arc;
use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let db = Database::new().expect("Failed to initialize database");
    let port: u16 = db.get_setting("proxy_port").unwrap_or_else(|_| "15731".to_string())
        .parse().unwrap_or(15731);
    let proxy: SharedProxyManager = Arc::new(ProxyManager::new(port));

    tauri::Builder::default()
        .plugin(tauri_plugin_log::Builder::new().build())
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::default(),
            None as Option<Vec<&str>>,
        ))
        .manage(db)
        .manage(proxy.clone())
        .setup(move |app| {
            // Auto-start proxy if setting enabled
            let auto_start = app.state::<Database>()
                .get_setting("auto_start")
                .map(|v| v == "true")
                .unwrap_or(false);

            if auto_start {
                let proxy_state = app.state::<SharedProxyManager>();
                if let Ok(proxy_path) = find_proxy_path(app) {
                    let _ = proxy_state.start(&proxy_path);
                }
            }

            // Tray icon — left click toggles window, right click menu with Exit
            use tauri::tray::{TrayIconBuilder, MouseButton, MouseButtonState, TrayIconEvent};
            use tauri::menu::{MenuBuilder, MenuItemBuilder};
            
            let quit_item = MenuItemBuilder::with_id("quit", "Exit").build(app)?;
            let tray_menu = MenuBuilder::new(app).item(&quit_item).build()?;
            
            let mut tray_builder = TrayIconBuilder::new()
                .menu(&tray_menu)
                .tooltip("Coding Plan Proxy");
            if let Some(icon) = app.default_window_icon().cloned() {
                tray_builder = tray_builder.icon(icon);
            }
            let _tray = tray_builder
                .on_menu_event(move |app, event| {
                    if event.id() == "quit" {
                        app.exit(0);
                    }
                })
                .on_tray_icon_event(|tray, event| {
                    if let TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, .. } = event {
                        let app = tray.app_handle();
                        if let Some(window) = app.get_webview_window("main") {
                            if window.is_visible().unwrap_or(false) {
                                let _ = window.hide();
                            } else {
                                let _ = window.show();
                                let _ = window.set_focus();
                            }
                        }
                    }
                })
                .build(app)?;

            // Prevent window close from quitting — hide to tray instead
            if let Some(window) = app.get_webview_window("main") {
                let window_clone = window.clone();
                window.on_window_event(move |event| {
                    if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                        api.prevent_close();
                        let _ = window_clone.hide();
                    }
                });
            }

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::list_providers,
            commands::save_provider,
            commands::delete_provider,
            commands::generate_id,
            commands::test_connection,
            commands::start_proxy,
            commands::stop_proxy,
            commands::proxy_status,
            commands::proxy_port,
            commands::apply_to_codex,
            commands::read_codex_config,
            commands::set_verified,
            commands::get_setting,
            commands::set_setting,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

fn find_proxy_path(app: &tauri::App) -> Result<String, String> {
    // Try bundled resource first
    let resource_dir = app.path().resource_dir().map_err(|e| e.to_string())?;
    let bundled = resource_dir.join("proxy").join("index.mjs");
    if bundled.exists() {
        return Ok(bundled.to_string_lossy().to_string());
    }
    // Dev fallback: look relative to the exe and common project structures
    if let Ok(exe) = std::env::current_exe() {
        for ancestor in exe.ancestors().take(4) {
            let candidate = ancestor.join("proxy").join("index.mjs");
            if candidate.exists() { return Ok(candidate.to_string_lossy().to_string()); }
        }
    }
    Err("proxy/index.mjs not found".into())
}
