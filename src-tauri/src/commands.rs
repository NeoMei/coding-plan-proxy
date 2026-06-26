use crate::db::{Database, Provider};
use crate::proxy::SharedProxyManager;
use crate::codex_config;
use tauri::{State, Manager};
use uuid::Uuid;
use std::collections::BTreeMap;
#[cfg(windows)]
use std::os::windows::process::CommandExt;

#[tauri::command]
pub fn list_providers(db: State<Database>) -> Result<Vec<Provider>, String> {
    db.list_providers()
}

#[tauri::command]
pub fn save_provider(db: State<Database>, provider: Provider) -> Result<(), String> {
    db.upsert_provider(&provider)
}

#[tauri::command]
pub fn save_providers(db: State<Database>, providers: Vec<Provider>) -> Result<(), String> {
    for p in &providers { db.upsert_provider(p)?; }
    Ok(())
}

#[tauri::command]
pub fn delete_provider(db: State<Database>, id: String) -> Result<(), String> {
    db.delete_provider(&id)
}

#[tauri::command]
pub fn generate_id() -> String {
    Uuid::new_v4().to_string()
}

#[tauri::command]
pub async fn test_connection(provider: Provider) -> Result<String, String> {
    codex_config::test_provider_connection(&provider).await
}

#[tauri::command]
pub fn start_proxy(app: tauri::AppHandle, proxy: State<SharedProxyManager>, db: State<Database>) -> Result<(), String> {
    start_proxy_service(&app, &proxy, &db)
}

/// Build the proxy config (model slug -> upstream/key/protocol) from verified providers.
/// When several providers share a slug, the active one (by id) wins so the proxy routes
/// that slug to the channel the user actually activated.
pub fn build_proxy_config(providers: &[Provider], active_id: &str) -> BTreeMap<String, serde_json::Value> {
    let mut config: BTreeMap<String, serde_json::Value> = BTreeMap::new();
    let mut active_entry = None;
    for p in providers.iter().filter(|p| p.verified && !p.api_key.is_empty()) {
        let entry = serde_json::json!({
            "upstream": p.upstream,
            "apiKey": p.api_key,
            "protocol": if !p.upstream.contains("/anthropic") { "chat" } else { "anthropic" }
        });
        if p.id == active_id { active_entry = Some((p.model.clone(), entry)); }
        else { config.insert(p.model.clone(), entry); }
    }
    if let Some((slug, entry)) = active_entry { config.insert(slug, entry); }
    config
}

pub fn write_proxy_config(providers: &[Provider], active_id: &str) -> Result<(), String> {
    let config = build_proxy_config(providers, active_id);
    let path = dirs::home_dir().unwrap_or_default().join(".coding-plan-proxy.json");
    std::fs::write(&path, serde_json::to_string_pretty(&config).unwrap_or_default())
        .map_err(|e| format!("Cannot write proxy config: {}", e))
}

/// Shared logic used by the start command and by auto-start.
pub fn start_proxy_service(app: &tauri::AppHandle, proxy: &SharedProxyManager, db: &Database) -> Result<(), String> {
    let providers = db.list_providers().map_err(|e| format!("DB error: {}", e))?;
    let active_id = db.get_setting("current_provider_id").unwrap_or_default();
    write_proxy_config(&providers, &active_id)?;

    let proxy_path = proxy_path(app)?;
    proxy.start(&proxy_path)?;

    let verified: Vec<&Provider> = providers.iter().filter(|p| p.verified && !p.api_key.is_empty()).collect();
    if verified.is_empty() { return Ok(()); }

    // Empty active_id = deliberately deactivated. A stale id falls back to the first verified.
    let active = if active_id.is_empty() {
        // Migrate legacy current_model (slug) → provider id once.
        let legacy = db.get_setting("current_model").unwrap_or_default();
        if !legacy.is_empty() {
            if let Some(p) = verified.iter().find(|p| p.model == legacy).copied() {
                let _ = db.set_setting("current_provider_id", &p.id);
                let _ = db.set_setting("current_model", "");
                Some(p)
            } else { None }
        } else { None }
    } else {
        match verified.iter().find(|p| p.id == active_id).copied() {
            Some(p) => Some(p),
            None => {
                let fallback = verified.first().copied().unwrap();
                let _ = db.set_setting("current_provider_id", &fallback.id);
                Some(fallback)
            }
        }
    };
    match active {
        Some(p) => {
            codex_config::write_model_catalog(&providers).map_err(|e| format!("Catalog: {}", e))?;
            codex_config::write_codex_auth().map_err(|e| format!("Auth: {}", e))?;
            codex_config::write_codex_config(&p.model, proxy.port(), p.context_window, &providers).map_err(|e| format!("Config: {}", e))?;
        }
        None => {
            codex_config::write_model_catalog(&[]).ok();
            codex_config::write_codex_auth().ok();
            codex_config::write_codex_config("", proxy.port(), 0, &[]).ok();
        }
    }
    Ok(())
}

#[tauri::command]
pub fn stop_proxy(proxy: State<SharedProxyManager>) -> Result<(), String> {
    proxy.stop()
}

#[tauri::command]
pub fn proxy_status(proxy: State<SharedProxyManager>) -> Result<bool, String> {
    Ok(proxy.is_running())
}

#[tauri::command]
pub fn proxy_port(proxy: State<SharedProxyManager>) -> Result<u16, String> {
    Ok(proxy.port())
}

#[tauri::command]
pub fn apply_to_codex(app: tauri::AppHandle, db: State<Database>, proxy: State<SharedProxyManager>, id: String) -> Result<(), String> {
    let all_providers = db.list_providers()?;
    let provider = all_providers.iter()
        .find(|p| p.id == id && p.verified && !p.api_key.is_empty())
        .ok_or_else(|| format!("Provider not found or not verified: {}", id))?;

    write_proxy_config(&all_providers, &id)?;
    codex_config::write_codex_config(&provider.model, proxy.port(), provider.context_window, &all_providers)?;
    codex_config::write_model_catalog(&all_providers)?;
    codex_config::write_codex_auth()?;
    db.set_setting("current_provider_id", &id)?;

    if proxy.is_running() {
        proxy.stop()?;
        let proxy_path = proxy_path(&app)?;
        proxy.start(&proxy_path)?;
    }
    Ok(())
}

#[tauri::command]
pub fn deactivate_model(db: State<Database>, proxy: State<SharedProxyManager>) -> Result<(), String> {
    db.set_setting("current_provider_id", "")?;
    codex_config::write_model_catalog(&[])?;
    codex_config::write_codex_auth()?;
    codex_config::write_codex_config("", proxy.port(), 0, &[])?;
    Ok(())
}

#[tauri::command]
pub fn read_codex_config() -> Result<String, String> {
    codex_config::read_codex_config()
}

#[tauri::command]
pub fn get_setting(db: State<Database>, key: String) -> Result<String, String> {
    db.get_setting(&key)
}

#[tauri::command]
pub fn set_setting(db: State<Database>, key: String, value: String) -> Result<(), String> {
    db.set_setting(&key, &value)
}

#[tauri::command]
pub fn set_verified(db: State<Database>, id: String, verified: bool) -> Result<(), String> {
    let conn = db.conn.lock().map_err(|e| e.to_string())?;
    // Sort index: verified come first (0-999), unverified get high numbers
    let sort_idx: i32 = if verified {
        // Count existing verified and place after them
        let count: i32 = conn.query_row(
            "SELECT COUNT(*) FROM providers WHERE verified = 1 AND id != ?1",
            rusqlite::params![id],
            |r| r.get(0),
        ).unwrap_or(0);
        count
    } else { 999 };
    conn.execute(
        "UPDATE providers SET verified = ?1, sort_index = ?2 WHERE id = ?3",
        rusqlite::params![verified as i64, sort_idx, id],
    ).map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn fetch_models(upstream: String, api_key: String) -> Result<serde_json::Value, String> {
    let base = upstream.trim_end_matches('/');
    let url = format!("{base}/models");
    
    let mut cmd = std::process::Command::new("curl");
    let header_path = codex_config::write_curl_header_file(&format!("x-api-key: {api_key}"))?;
    cmd.arg("-s").arg("--fail").arg("--max-time").arg("8").arg("--noproxy").arg("*")
        .arg(&url)
        .arg("-H").arg(format!("@{}", header_path.display()))
        .arg("-H").arg("anthropic-version: 2023-06-01");
    
    #[cfg(windows)] { cmd.creation_flags(0x08000000); }
    
    let output_res = cmd.output();
    let _ = std::fs::remove_file(&header_path);
    let output = output_res.map_err(|e| format!("curl: {e}"))?;
    let body = String::from_utf8_lossy(&output.stdout).to_string();
    
    // If Anthropic endpoint returns empty, try base domain's /v1/models with Bearer
    if body.trim().is_empty() {
        // Extract origin: https://api.deepseek.com/anthropic/v1 → https://api.deepseek.com
        let origin = if let Some(after_scheme) = upstream.find("://") {
            let rest = &upstream[after_scheme + 3..];
            if let Some(first_slash) = rest.find('/') {
                &upstream[..after_scheme + 3 + first_slash]
            } else { upstream.trim_end_matches('/') }
        } else { upstream.trim_end_matches('/') };
        let fallback_url = format!("{origin}/v1/models");
        let mut cmd2 = std::process::Command::new("curl");
        let header2_path = codex_config::write_curl_header_file(&format!("Authorization: Bearer {api_key}"))?;
        cmd2.arg("-s").arg("--fail").arg("--max-time").arg("8").arg("--noproxy").arg("*")
            .arg(&fallback_url)
            .arg("-H").arg(format!("@{}", header2_path.display()));
        #[cfg(windows)] { cmd2.creation_flags(0x08000000); }
        let out2_res = cmd2.output();
        let _ = std::fs::remove_file(&header2_path);
        let out2 = out2_res.map_err(|e| format!("curl: {e}"))?;
        let body2 = String::from_utf8_lossy(&out2.stdout).to_string();
        if body2.trim().is_empty() {
            return Err("Endpoint does not support model listing. Use preset or enter model ID manually.".into());
        }
        let json2: serde_json::Value = serde_json::from_str(&body2)
            .map_err(|_| format!("Invalid response: {}", &body2[..body2.len().min(200)]))?;
        let data2 = json2["data"].as_array()
            .ok_or_else(|| "No 'data' array".to_string())?;
        let models: Vec<serde_json::Value> = data2.iter().map(|m| {
            let id = m["id"].as_str().unwrap_or("");
            // Known model defaults (context_window, max_tokens)
            let (ctx, max_tok) = match id {
                "deepseek-v4-pro" => (1_000_000, 384_000),
                "deepseek-v4-flash" => (1_000_000, 384_000),
                "deepseek-chat" => (131_072, 8_192),
                "deepseek-reasoner" => (131_072, 65_536),
                _ => (0, 0),
            };
            serde_json::json!({ "id": id, "name": m["id"].as_str().unwrap_or(""), "context_length": ctx, "max_tokens": max_tok })
        }).collect();
        if models.is_empty() { return Err("No models found".into()); }
        return Ok(serde_json::json!({"models": models}));
    }
    
    let json: serde_json::Value = serde_json::from_str(&body)
        .map_err(|_| format!("Invalid response: {}", &body[..body.len().min(200)]))?;
    
    let data = json["data"].as_array()
        .ok_or_else(|| "No 'data' array in response — endpoint may not support model listing".to_string())?;
    
    if data.is_empty() { return Err("No models found".into()); }
    
    // Return enriched: { models: [{id, name, context_length?}], ... }
    let models: Vec<serde_json::Value> = data.iter().map(|m| {
        serde_json::json!({
            "id": m["id"].as_str().unwrap_or(""),
            "name": m["display_name"].as_str().unwrap_or(m["id"].as_str().unwrap_or("")),
            "context_length": m["context_length"].as_u64().unwrap_or(0),
        })
    }).collect();
    
    Ok(serde_json::json!({"models": models}))
}

#[tauri::command]
pub fn rebuild_tray_menu(app: tauri::AppHandle, db: State<Database>, tray: State<crate::TrayState>, proxy: State<SharedProxyManager>) -> Result<(), String> {
    let providers = db.list_providers()?;
    let verified: Vec<(&str, &str, &str)> = providers.iter()
        .filter(|p| p.verified && !p.api_key.is_empty())
        .map(|p| (p.id.as_str(), p.name.as_str(), p.model.as_str()))
        .collect();
    let active_id = db.get_setting("current_provider_id").unwrap_or_default();

    let menu = crate::build_tray_menu(&app, &verified, proxy.is_running(), &active_id).map_err(|e| e.to_string())?;
    if let Ok(guard) = tray.0.lock() {
        if let Some(ref tray_icon) = *guard {
            tray_icon.set_menu(Some(menu)).map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}

pub fn proxy_path(app: &tauri::AppHandle) -> Result<String, String> {
    // Strip Windows verbatim path prefix (\\?\) before returning, otherwise Node.js
    // fails to resolve the script entry point with EISDIR.
    fn clean_path(p: &std::path::Path) -> String {
        let s = p.to_string_lossy().to_string();
        if s.starts_with(r"\\?\") { s[4..].to_string() } else { s }
    }

    #[cfg(debug_assertions)]
    if let Ok(exe) = std::env::current_exe() {
        for ancestor in exe.ancestors().skip(1) {
            if ancestor.ends_with("target") { continue; }
            let candidate = ancestor.join("proxy").join("index.mjs");
            log::info!("proxy_path dev candidate: {}", candidate.display());
            if candidate.exists() {
                return Ok(clean_path(&candidate));
            }
        }
    }

    // Production: bundled resource directory
    if let Ok(resource_path) = app.path().resolve("proxy/index.mjs", tauri::path::BaseDirectory::Resource) {
        log::info!("proxy_path resource candidate: {}", resource_path.display());
        if resource_path.exists() {
            return Ok(clean_path(&resource_path));
        }
    }

    // Fallback: executable-adjacent locations (various install layouts)
    if let Ok(exe) = std::env::current_exe() {
        if let Some(parent) = exe.parent() {
            let candidates = [
                parent.join("resources").join("proxy").join("index.mjs"),
                parent.join("proxy").join("index.mjs"),
                parent.parent().map(|p| p.join("Resources").join("proxy").join("index.mjs")).unwrap_or_default(), // macOS bundle
                parent.parent().map(|p| p.join("resources").join("proxy").join("index.mjs")).unwrap_or_default(),
                parent.parent().map(|p| p.join("proxy").join("index.mjs")).unwrap_or_default(),
                parent.parent().and_then(|p| p.parent()).map(|p| p.join("resources").join("proxy").join("index.mjs")).unwrap_or_default(),
                parent.parent().and_then(|p| p.parent()).map(|p| p.join("proxy").join("index.mjs")).unwrap_or_default(),
            ];
            for c in &candidates {
                log::info!("proxy_path fallback candidate: {}", c.display());
                if c.exists() { return Ok(clean_path(c)); }
            }
        }
    }

    // Linux package install layouts (deb/rpm)
    let linux_candidates = [
        std::path::PathBuf::from("/usr/lib/CodexProxy/proxy/index.mjs"),
        std::path::PathBuf::from("/usr/lib/codex-proxy/proxy/index.mjs"),
        std::path::PathBuf::from("/usr/lib/codexproxy/proxy/index.mjs"),
        std::path::PathBuf::from("/opt/CodexProxy/proxy/index.mjs"),
        std::path::PathBuf::from("/opt/codexproxy/proxy/index.mjs"),
    ];
    for c in &linux_candidates {
        log::info!("proxy_path linux candidate: {}", c.display());
        if c.exists() { return Ok(clean_path(c)); }
    }

    // Dev: common locations relative to CWD
    let cwd = std::env::current_dir().unwrap_or_default();
    let dev_candidates = [
        cwd.join("proxy").join("index.mjs"),
        cwd.join("coding-plan-tauri").join("proxy").join("index.mjs"),
        cwd.parent().map(|p| p.join("proxy").join("index.mjs")).unwrap_or_default(),
    ];
    for c in &dev_candidates {
        log::info!("proxy_path cwd candidate: {}", c.display());
        if c.exists() { return Ok(clean_path(c)); }
    }

    log::error!("proxy/index.mjs not found in any candidate location");
    Err("proxy/index.mjs not found".into())
}
