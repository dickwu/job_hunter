pub mod analysis_agent;
mod commands;
mod db;
mod mcp;
mod settings;
mod state;

use tauri::Manager;
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_store::Builder::default().build())
        .setup(|app| {
            if cfg!(debug_assertions) {
                app.handle().plugin(
                    tauri_plugin_log::Builder::default()
                        .level(log::LevelFilter::Info)
                        .build(),
                )?;
            }

            settings::ensure_defaults(app.handle())?;
            let db = db::Db::new(app.handle())?;
            let mcp_port = mcp::start(app.handle().clone(), db.clone())?;
            app.manage(state::AppState { mcp_port, db });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_settings,
            commands::update_settings,
            commands::start_analysis,
            commands::list_job_matches,
            commands::clear_job_matches,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
