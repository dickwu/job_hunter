use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::HashMap;
use tauri::AppHandle;
use tauri_plugin_store::{StoreBuilder, StoreExt};

const STORE_FILENAME: &str = "job_settings.json";
const SETTINGS_KEY: &str = "settings";

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct JobSettings {
    pub preferred_titles: Vec<String>,
    pub locations: Vec<String>,
    pub keywords: Vec<String>,
    pub remote_only: bool,
    pub salary_min: Option<i64>,
    pub salary_max: Option<i64>,
    pub company_blacklist: Vec<String>,
}

impl Default for JobSettings {
    fn default() -> Self {
        Self {
            preferred_titles: vec![
                "Software Engineer".to_string(),
                "Full Stack Engineer".to_string(),
                "Frontend Engineer".to_string(),
                "Backend Engineer".to_string(),
            ],
            locations: vec!["Remote".to_string(), "United States".to_string()],
            keywords: vec![
                "TypeScript".to_string(),
                "React".to_string(),
                "Node.js".to_string(),
                "Rust".to_string(),
                "Tauri".to_string(),
                "Next.js".to_string(),
            ],
            remote_only: true,
            salary_min: Some(120_000),
            salary_max: Some(200_000),
            company_blacklist: Vec::new(),
        }
    }
}

pub fn ensure_defaults(app: &AppHandle) -> Result<JobSettings, String> {
    let defaults = JobSettings::default();
    let mut default_map = HashMap::new();
    default_map.insert(SETTINGS_KEY.to_string(), json!(defaults));

    let _store = StoreBuilder::new(app, STORE_FILENAME)
        .defaults(default_map)
        .auto_save(std::time::Duration::from_millis(400))
        .build()
        .map_err(|err| format!("store build: {err}"))?;
    match load_settings(app)? {
        Some(settings) => Ok(settings),
        None => save_settings(app, &defaults),
    }
}

pub fn load_settings(app: &AppHandle) -> Result<Option<JobSettings>, String> {
    let store = app
        .store(STORE_FILENAME)
        .map_err(|err| format!("store load: {err}"))?;
    let value = store.get(SETTINGS_KEY);
    match value {
        Some(val) => serde_json::from_value(val)
            .map(Some)
            .map_err(|err| format!("settings parse: {err}")),
        None => Ok(None),
    }
}

pub fn save_settings(app: &AppHandle, settings: &JobSettings) -> Result<JobSettings, String> {
    let store = app
        .store(STORE_FILENAME)
        .map_err(|err| format!("store load: {err}"))?;
    store.set(SETTINGS_KEY.to_string(), json!(settings));
    store.save().map_err(|err| format!("store save: {err}"))?;
    Ok(settings.clone())
}
