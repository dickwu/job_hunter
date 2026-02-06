// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    if std::env::args().any(|arg| arg == "--analysis-agent") {
        app_lib::analysis_agent::run();
        return;
    }

    app_lib::run();
}
