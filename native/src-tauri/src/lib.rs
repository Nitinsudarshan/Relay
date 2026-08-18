pub mod capture;
pub mod commands;
pub mod mcp;
pub mod pipeline;
pub mod providers;
pub mod triggers;
pub mod vault;

use capture::AudioRecorder;
use commands::AppState;
use std::path::PathBuf;
use vault::VaultManager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let base_dir = std::env::current_dir()
        .unwrap_or_else(|_| PathBuf::from("."))
        .join(".relay");

    let vault_dir = base_dir.join("vault");
    let config_dir = base_dir.join("config");

    let state = AppState {
        recorder: AudioRecorder::new(),
        vault: VaultManager::new(vault_dir),
        config_dir,
    };

    tauri::Builder::default()
        .manage(state)
        .invoke_handler(tauri::generate_handler![
            commands::start_capture,
            commands::stop_capture,
            commands::get_kanban_cards,
            commands::get_triggers,
            commands::save_triggers,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
