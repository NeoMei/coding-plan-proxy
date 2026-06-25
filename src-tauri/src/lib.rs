mod commands;
mod codex_config;
mod db;
mod proxy;

use db::Database;
use proxy::{ProxyManager, SharedProxyManager};
use std::sync::{Arc, Mutex as StdMutex};
use tauri::Manager;
use tauri::tray::TrayIcon;

pub struct TrayState(pub StdMutex<Option<TrayIcon>>);

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let db = Database::new().expect("Failed to initialize database");
    let port: u16 = db.get_setting("proxy_port").unwrap_or_else(|_| "15731".to_string())
        .parse().unwrap_or(15731);
    let proxy: SharedProxyManager = Arc::new(ProxyManager::new(port));
    let tray_state = TrayState(StdMutex::new(None));

    tauri::Builder::default()
        .plugin(tauri_plugin_log::Builder::new().build())
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::default(),
            None as Option<Vec<&str>>,
        ))
        .manage(db)
        .manage(proxy.clone())
        .manage(tray_state)
        .setup(move |app| {
            let auto_start = app.state::<Database>()
                .get_setting("auto_start")
                .map(|v| v == "true")
                .unwrap_or(false);
            if auto_start {
                let proxy_state = app.state::<SharedProxyManager>();
                if let Ok(proxy_path) = find_proxy_path(app.handle()) {
                    let _ = proxy_state.start(&proxy_path);
                }
            }

            // Build initial tray menu
            use tauri::tray::{TrayIconBuilder, MouseButton, MouseButtonState, TrayIconEvent};
            let menu = build_tray_menu(app.handle(), &[], false)?;
            
            let mut tray_builder = TrayIconBuilder::new()
                .menu(&menu)
                .tooltip("Coding Plan Proxy");
            if let Some(icon) = app.default_window_icon().cloned() {
                tray_builder = tray_builder.icon(icon);
            }
            let tray = tray_builder
                .on_menu_event({
                    let app_handle = app.handle().clone();
                    move |_app, event| {
                        let id = event.id().0.clone();
                        match id.as_str() {
                            "quit" => _app.exit(0),
                            "toggle_window" => {
                                if let Some(window) = _app.get_webview_window("main") {
                                    if window.is_visible().unwrap_or(false) { let _ = window.hide(); }
                                    else { let _ = window.show(); let _ = window.set_focus(); }
                                }
                            }
                            "toggle_proxy" => {
                                let proxy = app_handle.state::<SharedProxyManager>();
                                if proxy.is_running() { let _ = proxy.stop(); }
                                else {
                                    let db = app_handle.state::<Database>();
                                    if let Ok(proxy_path) = find_proxy_path(&app_handle) {
                                        let _ = proxy.start(&proxy_path);
                                    }
                                }
                            }
                            id if id.starts_with("model:") => {
                                let model = id.strip_prefix("model:").unwrap_or("").to_string();
                                let db = app_handle.state::<Database>();
                                let proxy = app_handle.state::<SharedProxyManager>();
                                if let Ok(providers) = db.list_providers() {
                                    let _ = codex_config::write_codex_config(&model, proxy.port(), 262144, &providers);
                                    let _ = codex_config::write_model_catalog(&providers);
                                    let _ = codex_config::write_codex_auth();
                                }
                            }
                            _ => {}
                        }
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

            // Store tray handle
            if let Ok(mut guard) = app.state::<TrayState>().0.lock() {
                *guard = Some(tray);
            }

            // Prevent window close from quitting
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
            commands::fetch_models,
            commands::rebuild_tray_menu,
            commands::get_setting,
            commands::set_setting,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

fn build_tray_menu(app: &tauri::AppHandle, providers: &[(&str, &str, &str)], proxy_running: bool) -> Result<tauri::menu::Menu<tauri::Wry>, tauri::Error> {
    use tauri::menu::{MenuBuilder, MenuItemBuilder, SubmenuBuilder};
    use std::collections::BTreeMap;
    
    // Group models by vendor name
    let mut vendors: BTreeMap<&str, Vec<(&str, &str)>> = BTreeMap::new();
    for (model, name, _vendor) in providers {
        // Extract vendor from name: "Kimi Coding Plan" → vendor = "Kimi"
        let vendor = name.split_whitespace().next().unwrap_or(name);
        vendors.entry(vendor).or_default().push((*model, *name));
    }
    
    let mut builder = MenuBuilder::new(app);
    
    builder = builder.item(&MenuItemBuilder::with_id("toggle_window", "Show/Hide").build(app)?);
    builder = builder.separator();
    
    // Submenus for each vendor
    for (vendor, models) in &vendors {
        let mut sub = SubmenuBuilder::new(app, *vendor);
        for (model, name) in models {
            sub = sub.item(&MenuItemBuilder::with_id(&format!("model:{model}"), *name).build(app)?);
        }
        builder = builder.item(&sub.build()?);
    }
    builder = builder.separator();
    
    let proxy_label = if proxy_running { "Stop Proxy" } else { "Start Proxy" };
    builder = builder.item(&MenuItemBuilder::with_id("toggle_proxy", proxy_label).build(app)?);
    builder = builder.separator();
    
    builder = builder.item(&MenuItemBuilder::with_id("quit", "Exit").build(app)?);
    builder.build()
}

fn find_proxy_path(_app: &tauri::AppHandle) -> Result<String, String> {
    if let Ok(exe) = std::env::current_exe() {
        for ancestor in exe.ancestors().take(4) {
            let candidate = ancestor.join("proxy").join("index.mjs");
            if candidate.exists() { return Ok(candidate.to_string_lossy().to_string()); }
        }
    }
    Err("proxy/index.mjs not found".into())
}
