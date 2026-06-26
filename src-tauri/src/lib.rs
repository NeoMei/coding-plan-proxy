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
                let db_state = app.state::<Database>();
                let _ = commands::start_proxy_service(&app.handle(), &proxy_state, &db_state);
            }

            // Build initial tray menu from verified providers
            use tauri::tray::{TrayIconBuilder, MouseButton, MouseButtonState, TrayIconEvent};
            let db_for_tray = app.state::<Database>();
            let providers_list = db_for_tray.list_providers().unwrap_or_default();
            let active_id = db_for_tray.get_setting("current_provider_id").unwrap_or_default();
            let initial_providers: Vec<(&str, &str, &str)> = providers_list
                .iter()
                .filter(|p| p.verified && !p.api_key.is_empty())
                .map(|p| (p.id.as_str(), p.name.as_str(), p.model.as_str()))
                .collect();
            let menu = build_tray_menu(app.handle(), &initial_providers, false, &active_id)?;
            
            let mut tray_builder = TrayIconBuilder::new()
                .menu(&menu)
                .tooltip("CodexProxy");
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
                                if proxy.is_running() {
                                    let _ = proxy.stop();
                                } else {
                                    let _db = app_handle.state::<Database>();
                                    let _ = commands::start_proxy_service(&app_handle, &proxy, &_db);
                                }
                            }
                            id if id.starts_with("provider:") => {
                                let pid = id.strip_prefix("provider:").unwrap_or("").to_string();
                                let _db = app_handle.state::<Database>();
                                let proxy = app_handle.state::<SharedProxyManager>();
                                if let Ok(providers) = _db.list_providers() {
                                    let Some(selected) = providers.iter().find(|p| p.id == pid && p.verified && !p.api_key.is_empty()) else { return; };
                                    let model = selected.model.clone();
                                    let ctx = selected.context_window;
                                    let _ = commands::write_proxy_config(&providers, &pid);
                                    let _ = _db.set_setting("current_provider_id", &pid);
                                    // Restart proxy with new config
                                    let was_running = proxy.is_running();
                                    if was_running { let _ = proxy.stop(); }
                                    let _ = codex_config::write_codex_config(&model, proxy.port(), ctx, &providers);
                                    let _ = codex_config::write_model_catalog(&providers);
                                    let _ = codex_config::write_codex_auth();
                                    if was_running {
                                        if let Ok(proxy_path) = commands::proxy_path(&app_handle) {
                                            let _ = proxy.start(&proxy_path);
                                        }
                                    }
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
            commands::save_providers,
            commands::delete_provider,
            commands::generate_id,
            commands::test_connection,
            commands::start_proxy,
            commands::stop_proxy,
            commands::proxy_status,
            commands::proxy_port,
            commands::apply_to_codex,
            commands::deactivate_model,
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

fn build_tray_menu(app: &tauri::AppHandle, providers: &[(&str, &str, &str)], proxy_running: bool, active_id: &str) -> Result<tauri::menu::Menu<tauri::Wry>, tauri::Error> {
    use tauri::menu::{MenuBuilder, MenuItemBuilder, SubmenuBuilder};
    
    let mut builder = MenuBuilder::new(app);

    builder = builder.item(&MenuItemBuilder::with_id("toggle_window", "Show/Hide").build(app)?);
    builder = builder.separator();

    // Each provider gets a top-level submenu; its model slug lives inside, keyed by provider id.
    for (id, name, model) in providers {
        let mut sub = SubmenuBuilder::new(app, *name);
        let label = if *id == active_id { format!("● {model}") } else { (*model).to_string() };
        sub = sub.item(&MenuItemBuilder::with_id(&format!("provider:{id}"), label).build(app)?);
        builder = builder.item(&sub.build()?);
    }
    builder = builder.separator();
    
    let proxy_label = if proxy_running { "Stop Proxy" } else { "Start Proxy" };
    builder = builder.item(&MenuItemBuilder::with_id("toggle_proxy", proxy_label).build(app)?);
    builder = builder.separator();
    
    builder = builder.item(&MenuItemBuilder::with_id("quit", "Exit").build(app)?);
    builder.build()
}
