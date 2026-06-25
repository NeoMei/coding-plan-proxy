use crate::db::Provider;
use serde_json::json;
use std::fs;
use std::path::PathBuf;

fn codex_dir() -> PathBuf {
    dirs::home_dir().unwrap_or_else(|| PathBuf::from(".")).join(".codex")
}

fn catalog_file() -> PathBuf {
    codex_dir().join("models-catalog.json")
}

/// Write Codex config.toml to point to the proxy with model mappings
pub fn write_codex_config(model: &str, proxy_port: u16, context_window: u64, verified_providers: &[Provider]) -> Result<(), String> {
    let dir = codex_dir();
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;

    let config = format!(
        r#"model = "{model}"
model_provider = "custom"
model_context_window = {ctx}

[model_providers.custom]
name = "Coding Plan"
wire_api = "responses"
requires_openai_auth = true
base_url = "http://127.0.0.1:{port}/v1"

[model_providers.custom.models]
{models_toml}

[features]
js_repl = false
"#,
        model = model,
        ctx = context_window,
        port = proxy_port,
        models_toml = verified_providers.iter()
            .filter(|p| p.verified && !p.api_key.is_empty())
            .map(|p| format!("\"{}\" = \"{}\"", p.model, p.model))
            .collect::<Vec<_>>()
            .join("\n")
    );

    fs::write(dir.join("config.toml"), &config).map_err(|e| e.to_string())?;
    Ok(())
}

/// Write auth.json (proxy handles real auth)
pub fn write_codex_auth() -> Result<(), String> {
    let dir = codex_dir();
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let auth = json!({"OPENAI_API_KEY": "proxy-managed"});
    fs::write(dir.join("auth.json"), serde_json::to_string_pretty(&auth).unwrap())
        .map_err(|e| e.to_string())?;
    Ok(())
}

/// Read current Codex config.toml
pub fn read_codex_config() -> Result<String, String> {
    let path = codex_dir().join("config.toml");
    if path.exists() {
        fs::read_to_string(&path).map_err(|e| e.to_string())
    } else {
        Ok(String::new())
    }
}

/// Generate model catalog JSON for verified providers only
pub fn write_model_catalog(providers: &[Provider]) -> Result<(), String> {
    let models: Vec<serde_json::Value> = providers.iter()
        .filter(|p| p.enabled && p.verified && !p.api_key.is_empty())
        .map(|p| {
            let efforts: Vec<serde_json::Value> = if p.model.contains("deepseek") {
                vec!["low","medium","high","xhigh"]
            } else if p.model.contains("glm") {
                vec!["low","medium","high"]
            } else {
                vec!["medium"]
            }.into_iter().map(|e| json!({"effort": e, "description": e})).collect();

            json!({
                "slug": p.model,
                "display_name": p.name,
                "description": "",
                "default_reasoning_level": "medium",
                "supported_reasoning_levels": efforts,
                "shell_type": "shell_command",
                "visibility": "list",
                "supported_in_api": true,
                "priority": 0,
                "additional_speed_tiers": [],
                "supports_reasoning_summaries": false,
                "default_reasoning_summary": "none",
                "support_verbosity": false,
                "default_verbosity": "low",
                "apply_patch_tool_type": "freeform",
                "web_search_tool_type": "text_and_image",
                "truncation_policy": {"mode": "tokens", "limit": 10000},
                "supports_parallel_tool_calls": true,
                "supports_image_detail_original": false,
                "experimental_supported_tools": [],
                "input_modalities": ["text"],
                "supports_search_tool": false,
                "use_responses_lite": false,
                "base_instructions": "",
                "instructions_variables": {
                    "personality_default": "",
                    "personality_friendly": "",
                    "personality_pragmatic": ""
                },
                "availability_nux": null,
                "upgrade": null,
                "service_tiers": [],
                "context_window": p.context_window,
                "max_context_window": p.context_window,
                "max_output_tokens": p.max_output_tokens,
                "effective_context_window_percent": 95,
            })
        })
        .collect();

    let catalog = json!({"models": models});
    let path = catalog_file();
    fs::write(&path, serde_json::to_string_pretty(&catalog).unwrap())
        .map_err(|e| e.to_string())?;
    Ok(())
}

/// Test connectivity to a provider's upstream
pub async fn test_provider_connection(provider: &Provider) -> Result<String, String> {
    let url = provider.upstream.trim_end_matches('/').to_string();
    let is_chat = url.contains("/v1") && !url.contains("/anthropic");
    
    let body = if is_chat {
        serde_json::json!({"model": provider.model, "messages": [{"role": "user", "content": "hi"}], "max_tokens": 5})
    } else {
        serde_json::json!({"model": provider.model, "max_tokens": 5, "messages": [{"role": "user", "content": "hi"}]})
    };

    let endpoint = if is_chat { format!("{url}/chat/completions") } else { format!("{url}/messages") };
    let auth_header = if is_chat { format!("Authorization: Bearer {}", provider.api_key) } else { format!("x-api-key: {}", provider.api_key) };

    let mut cmd = std::process::Command::new("curl");
    cmd.arg("-s").arg("--max-time").arg("10").arg("--noproxy").arg("*")
        .arg(&endpoint)
        .arg("-H").arg(&auth_header)
        .arg("-H").arg("content-type: application/json")
        .arg("-d").arg(serde_json::to_string(&body).unwrap_or_default());

    if !is_chat { cmd.arg("-H").arg("anthropic-version: 2023-06-01"); }

    // Hide console window on Windows
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x08000000); // CREATE_NO_WINDOW
    }

    let output = cmd.output().map_err(|e| format!("curl error: {}", e))?;

    if output.status.success() {
        let body_str = String::from_utf8_lossy(&output.stdout);
        let ok = body_str.contains("\"type\":\"message\"") || body_str.contains("\"content\"") || body_str.contains("\"choices\"");
        if ok { Ok("ok".to_string()) }
        else { Err(format!("Unexpected: {}", body_str.chars().take(200).collect::<String>())) }
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);
        let msg = if stderr.is_empty() { stdout } else { stderr };
        Err(msg.chars().take(200).collect())
    }
}
