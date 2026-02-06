use crate::db::JobMatch;
use crate::settings::{load_settings, save_settings, JobSettings};
use crate::state::AppState;
use serde::Serialize;
use tauri::{AppHandle, Emitter, State};
use uuid::Uuid;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AnalysisStart {
    pub analysis_id: String,
    pub mcp_port: u16,
}

#[tauri::command]
pub fn get_settings(app: AppHandle) -> Result<JobSettings, String> {
    Ok(load_settings(&app)?.unwrap_or_default())
}

#[tauri::command]
pub fn update_settings(app: AppHandle, settings: JobSettings) -> Result<JobSettings, String> {
    save_settings(&app, &settings)
}

#[tauri::command]
pub fn list_job_matches(
    state: State<AppState>,
    limit: Option<usize>,
) -> Result<Vec<JobMatch>, String> {
    let limit = limit.unwrap_or(50);
    state.db.list_matches(limit)
}

#[tauri::command]
pub fn clear_job_matches(state: State<AppState>) -> Result<(), String> {
    state.db.clear()
}

#[tauri::command]
pub fn start_analysis(
    app: AppHandle,
    state: State<AppState>,
    url: String,
) -> Result<AnalysisStart, String> {
    let analysis_id = Uuid::new_v4().to_string();
    let exe = std::env::current_exe().map_err(|err| format!("locate executable: {err}"))?;

    std::process::Command::new(exe)
        .arg("--analysis-agent")
        .env("JOB_HUNTER_MCP_PORT", state.mcp_port.to_string())
        .env("JOB_HUNTER_TARGET_URL", url)
        .env("JOB_HUNTER_ANALYSIS_ID", analysis_id.clone())
        .spawn()
        .map_err(|err| format!("spawn analysis agent: {err}"))?;

    let analysis_id_for_emit = analysis_id.clone();
    let _ = app.emit(
        "analysis:started",
        serde_json::json!({ "analysisId": analysis_id_for_emit, "mcpPort": state.mcp_port }),
    );

    Ok(AnalysisStart {
        analysis_id,
        mcp_port: state.mcp_port,
    })
}
