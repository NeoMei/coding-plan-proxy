use serde::{Deserialize, Serialize};
use rusqlite::{Connection, params};
use std::path::PathBuf;
use std::sync::Mutex;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Provider {
    pub id: String,
    pub name: String,
    pub model: String,
    pub upstream: String,
    pub api_key: String,
    pub context_window: u64,
    pub max_output_tokens: u64,
    pub enabled: bool,
    pub sort_index: i32,
    pub verified: bool,
}

pub struct Database {
    pub conn: Mutex<Connection>,
}

impl Database {
    pub fn new() -> Result<Self, String> {
        let db_path = Self::db_path();
        if let Some(parent) = db_path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        let conn = Connection::open(&db_path).map_err(|e| e.to_string())?;
        conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON;")
            .map_err(|e| e.to_string())?;
        let db = Database { conn: Mutex::new(conn) };
        db.migrate()?;
        Ok(db)
    }

    fn db_path() -> PathBuf {
        dirs::data_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("coding-plan-proxy")
            .join("data.db")
    }

    fn migrate(&self) -> Result<(), String> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS providers (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                model TEXT NOT NULL,
                upstream TEXT NOT NULL,
                api_key TEXT NOT NULL DEFAULT '',
                context_window INTEGER NOT NULL DEFAULT 262144,
                max_output_tokens INTEGER NOT NULL DEFAULT 32768,
                enabled INTEGER NOT NULL DEFAULT 1,
                sort_index INTEGER NOT NULL DEFAULT 0,
                verified INTEGER NOT NULL DEFAULT 0
            );
            CREATE TABLE IF NOT EXISTS settings (
                key TEXT PRIMARY KEY,
                value TEXT NOT NULL
            );
            INSERT OR IGNORE INTO settings (key, value) VALUES ('proxy_port', '15731');
            INSERT OR IGNORE INTO settings (key, value) VALUES ('auto_start', 'false');
            INSERT OR IGNORE INTO settings (key, value) VALUES ('current_model', '');
            "
        ).map_err(|e| e.to_string())?;

        // Add verified column for databases created before v1.2
        conn.execute("ALTER TABLE providers ADD COLUMN verified INTEGER NOT NULL DEFAULT 0", []).ok();

        // Seed built-in presets if table is empty
        let count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM providers", [], |row| row.get(0)
        ).unwrap_or(0);
        if count == 0 {
            let presets = vec![
                ("preset-kimi-cp", "Kimi Coding Plan", "kimi-for-coding", "https://api.kimi.com/coding/v1", 262144, 32768),
                ("preset-kimi-api", "Kimi API", "kimi-k2.7-code", "https://api.moonshot.cn/v1", 262144, 32768),
                ("preset-glm-cp", "GLM Coding Plan", "glm-5.2", "https://open.bigmodel.cn/api/anthropic/v1", 200000, 32768),
                ("preset-glm-api", "GLM API", "glm-5.2", "https://open.bigmodel.cn/api/anthropic/v1", 200000, 32768),
                ("preset-deepseek", "DeepSeek", "deepseek-v4-pro", "https://api.deepseek.com/anthropic/v1", 1000000, 384000),
                ("preset-volc-cp", "Volcengine AgentPlan", "doubao-seed-2.0", "https://ark.cn-beijing.volces.com/api/anthropic/v1", 200000, 32768),
                ("preset-volc-api", "Volcengine API", "doubao-1.5-pro-256k", "https://ark.cn-beijing.volces.com/api/v3", 256000, 16384),
                ("preset-bailian-cp", "Bailian Coding Plan", "qwen-plus", "https://coding.dashscope.aliyuncs.com/v1", 200000, 32768),
                ("preset-bailian-api", "Bailian API", "qwen-max", "https://dashscope.aliyuncs.com/compatible-mode/v1", 32768, 8192),
                ("preset-openai", "OpenAI GPT-5.5", "gpt-5.5", "https://api.openai.com/v1", 272000, 128000),
                ("preset-anthropic", "Claude Opus 4", "claude-opus-4-20250514", "https://api.anthropic.com/v1", 200000, 32768),
                ("preset-google", "Gemini 2.5 Pro", "gemini-2.5-pro", "https://generativelanguage.googleapis.com/v1beta", 1048576, 65536),
            ];
            for (i, (id, name, model, upstream, ctx, max_tok)) in presets.iter().enumerate() {
                conn.execute(
                    "INSERT INTO providers (id, name, model, upstream, api_key, context_window, max_output_tokens, enabled, sort_index, verified)
                     VALUES (?1, ?2, ?3, ?4, '', ?5, ?6, 1, ?7, 0)",
                    rusqlite::params![id, name, model, upstream, *ctx as i64, *max_tok as i64, i as i32],
                ).ok();
            }
        }
        Ok(())
    }

    pub fn list_providers(&self) -> Result<Vec<Provider>, String> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        let mut stmt = conn.prepare(
            "SELECT id, name, model, upstream, api_key, context_window, max_output_tokens, enabled, sort_index, 
                    COALESCE(verified, 0) as verified
             FROM providers ORDER BY sort_index"
        ).map_err(|e| e.to_string())?;
        let rows = stmt.query_map([], |row| {
            Ok(Provider {
                id: row.get(0)?,
                name: row.get(1)?,
                model: row.get(2)?,
                upstream: row.get(3)?,
                api_key: row.get(4)?,
                context_window: row.get::<_, i64>(5)? as u64,
                max_output_tokens: row.get::<_, i64>(6)? as u64,
                enabled: row.get::<_, i64>(7)? != 0,
                sort_index: row.get(8)?,
                verified: row.get::<_, i64>(9)? != 0,
            })
        }).map_err(|e| e.to_string())?;
        let mut providers = Vec::new();
        for row in rows {
            providers.push(row.map_err(|e| e.to_string())?);
        }
        Ok(providers)
    }

    pub fn upsert_provider(&self, p: &Provider) -> Result<(), String> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        conn.execute(
            "INSERT INTO providers (id, name, model, upstream, api_key, context_window, max_output_tokens, enabled, sort_index, verified)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
             ON CONFLICT(id) DO UPDATE SET
                name=excluded.name, model=excluded.model, upstream=excluded.upstream,
                api_key=excluded.api_key, context_window=excluded.context_window,
                max_output_tokens=excluded.max_output_tokens, enabled=excluded.enabled,
                sort_index=excluded.sort_index, verified=excluded.verified",
            params![p.id, p.name, p.model, p.upstream, p.api_key, p.context_window as i64, p.max_output_tokens as i64, p.enabled as i64, p.sort_index, p.verified as i64],
        ).map_err(|e| e.to_string())?;
        Ok(())
    }

    pub fn delete_provider(&self, id: &str) -> Result<(), String> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        conn.execute("DELETE FROM providers WHERE id = ?1", params![id])
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    pub fn get_setting(&self, key: &str) -> Result<String, String> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        conn.query_row(
            "SELECT value FROM settings WHERE key = ?1",
            params![key],
            |row| row.get(0),
        ).map_err(|e| e.to_string())
    }

    pub fn set_setting(&self, key: &str, value: &str) -> Result<(), String> {
        let conn = self.conn.lock().map_err(|e| e.to_string())?;
        conn.execute(
            "INSERT INTO settings (key, value) VALUES (?1, ?2)
             ON CONFLICT(key) DO UPDATE SET value=excluded.value",
            params![key, value],
        ).map_err(|e| e.to_string())?;
        Ok(())
    }
}
