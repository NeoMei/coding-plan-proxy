use crate::db::{Database, Provider};
use crate::proxy::SharedProxyManager;
use crate::codex_config;
use tauri::State;
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
pub fn start_proxy(proxy: State<SharedProxyManager>, db: State<Database>) -> Result<(), String> {
    // Write provider config for the Node.js proxy
    let providers = db.list_providers().map_err(|e| format!("DB error: {}", e))?;
    let enabled_providers: Vec<&Provider> = providers.iter()
        .filter(|p| p.enabled && !p.api_key.is_empty())
        .collect();

    let config: BTreeMap<String, serde_json::Value> = enabled_providers.iter()
        .map(|p| (p.model.clone(), serde_json::json!({
            "upstream": p.upstream,
            "apiKey": p.api_key
        })))
        .collect();

    let config_path = dirs::home_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join(".coding-plan-proxy.json");
    let json_str = serde_json::to_string_pretty(&config).unwrap_or_else(|_| "{}".to_string());
    std::fs::write(&config_path, &json_str).map_err(|e| format!("Cannot write proxy config: {}", e))?;

    // Start the proxy (returns error if node not found)
    let proxy_path = proxy_path();
    proxy.start(&proxy_path)?;

    // Write Codex config only if we have verified providers
    let verified: Vec<&Provider> = providers.iter().filter(|p| p.verified).collect();
    if !verified.is_empty() {
        codex_config::write_model_catalog(&providers).map_err(|e| format!("Catalog: {}", e))?;
        codex_config::write_codex_auth().map_err(|e| format!("Auth: {}", e))?;
        let current_model = db.get_setting("current_model").unwrap_or_default();
        let model = if current_model.is_empty() {
            verified.first().map(|p| p.model.clone()).unwrap_or_default()
        } else { current_model };
        let ctx = verified.iter().find(|p| p.model == model).map(|p| p.context_window).unwrap_or(262144);
        codex_config::write_codex_config(&model, proxy.port(), ctx, &providers).map_err(|e| format!("Config: {}", e))?;
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
pub fn apply_to_codex(db: State<Database>, proxy: State<SharedProxyManager>, model: String) -> Result<(), String> {
    let all_providers = db.list_providers()?;
    let verified: Vec<&Provider> = all_providers.iter().filter(|p| p.verified && !p.api_key.is_empty()).collect();
    
    let provider = all_providers.iter().find(|p| p.model == model)
        .ok_or_else(|| format!("Model not found: {}", model))?;
    
    // Write proxy config with ALL verified providers (so proxy supports them all)
    let config: BTreeMap<String, serde_json::Value> = verified.iter()
        .map(|p| (p.model.clone(), serde_json::json!({"upstream": p.upstream, "apiKey": p.api_key})))
        .collect();
    let config_path = dirs::home_dir().unwrap_or_default().join(".coding-plan-proxy.json");
    std::fs::write(&config_path, serde_json::to_string_pretty(&config).unwrap_or_default())
        .map_err(|e| format!("Cannot write proxy config: {}", e))?;
    
    // Restart proxy to pick up new config (if it was running)
    let was_running = proxy.is_running();
    if was_running {
        proxy.stop()?;
    }
    
    // Write Codex config with selected model
    codex_config::write_codex_config(&provider.model, proxy.port(), provider.context_window, &all_providers)?;
    codex_config::write_model_catalog(&all_providers)?;
    codex_config::write_codex_auth()?;
    db.set_setting("current_model", &model)?;
    
    // Restart proxy
    if was_running {
        let proxy_path = proxy_path();
        proxy.start(&proxy_path)?;
    }
    
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
    cmd.arg("-s").arg("--max-time").arg("8").arg("--noproxy").arg("*")
        .arg(&url)
        .arg("-H").arg(format!("x-api-key: {api_key}"))
        .arg("-H").arg("anthropic-version: 2023-06-01");
    
    #[cfg(windows)] { cmd.creation_flags(0x08000000); }
    
    let output = cmd.output().map_err(|e| format!("curl: {e}"))?;
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
        cmd2.arg("-s").arg("--max-time").arg("8").arg("--noproxy").arg("*")
            .arg(&fallback_url)
            .arg("-H").arg(format!("Authorization: Bearer {api_key}"));
        #[cfg(windows)] { cmd2.creation_flags(0x08000000); }
        let out2 = cmd2.output().map_err(|e| format!("curl: {e}"))?;
        let body2 = String::from_utf8_lossy(&out2.stdout).to_string();
        if body2.trim().is_empty() {
            return Err("Endpoint does not support model listing. Use preset or enter model ID manually.".into());
        }
        let json2: serde_json::Value = serde_json::from_str(&body2)
            .map_err(|_| format!("Invalid response: {}", &body2[..body2.len().min(200)]))?;
        let data2 = json2["data"].as_array()
            .ok_or_else(|| "No 'data' array".to_string())?;
        let models: Vec<serde_json::Value> = data2.iter().map(|m| {
            serde_json::json!({ "id": m["id"].as_str().unwrap_or(""), "name": m["id"].as_str().unwrap_or(""), "context_length": 0 })
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

fn proxy_path() -> String {
    // Production: bundled resource next to the executable
    if let Ok(exe) = std::env::current_exe() {
        if let Some(parent) = exe.parent() {
            let candidates = [
                parent.join("proxy").join("index.mjs"),
                parent.parent().map(|p| p.join("proxy").join("index.mjs")).unwrap_or_default(),
                parent.parent().and_then(|p| p.parent()).map(|p| p.join("proxy").join("index.mjs")).unwrap_or_default(),
            ];
            for c in &candidates {
                if c.exists() { return c.to_string_lossy().to_string(); }
            }
        }
    }

    // Dev: common locations relative to CWD
    let cwd = std::env::current_dir().unwrap_or_default();
    let dev_candidates = [
        cwd.join("proxy").join("index.mjs"),
        cwd.join("coding-plan-tauri").join("proxy").join("index.mjs"),
        cwd.parent().map(|p| p.join("proxy").join("index.mjs")).unwrap_or_default(),
    ];
    for c in &dev_candidates {
        if c.exists() { return c.to_string_lossy().to_string(); }
    }

    // Hardcoded dev fallback
    let fallback = dirs::home_dir()
        .unwrap_or_default()
        .join("AppData").join("Roaming").join("reasonix").join("global-workspace")
        .join("coding-plan-tauri").join("proxy").join("index.mjs");
    if fallback.exists() { return fallback.to_string_lossy().to_string(); }

    log::error!("Proxy index.mjs not found at any known location");
    "proxy/index.mjs".to_string()
}
