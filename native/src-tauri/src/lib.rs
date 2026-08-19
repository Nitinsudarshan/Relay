pub mod capture;
pub mod commands;
pub mod hotkeys;
pub mod mcp;
pub mod overlay;
pub mod pipeline;
pub mod providers;
pub mod settings;
pub mod triggers;
pub mod tts;
pub mod vault;

use capture::{AudioRecorder, SttEngine};
use commands::AppState;
use settings::AppSettings;
use std::path::PathBuf;
use std::sync::Mutex;
use vault::VaultManager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let base_dir = std::env::current_dir()
        .unwrap_or_else(|_| PathBuf::from("."))
        .join(".relay");

    let vault_dir = base_dir.join("vault");
    let config_dir = base_dir.join("config");

    let settings = AppSettings::load(&config_dir.join("settings.json")).unwrap_or_default();
    let hotkeys_config = settings.hotkeys.clone();
    let show_floating_pill = settings.ui.show_floating_pill;
    let pill_position = settings.ui.pill_position;

    let state = AppState {
        recorder: AudioRecorder::new(),
        vault: VaultManager::new(vault_dir),
        config_dir,
        settings: Mutex::new(settings),
        stt: SttEngine::new(),
    };

    tauri::Builder::default()
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .manage(state)
        .setup(move |app| {
            hotkeys::register_hotkeys(
                app.handle(),
                &hotkeys_config.show_hide_hotkey,
                &hotkeys_config.dictation_hotkey,
            );
            overlay::ensure_pill_window(app.handle(), show_floating_pill, pill_position);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::start_capture,
            commands::stop_capture,
            commands::get_capture_status,
            commands::update_hotkeys,
            commands::set_pill_visible,
            commands::set_pill_position,
            commands::set_pill_expanded,
            commands::ensure_local_llm_ready,
            commands::ensure_stt_model_ready,
            commands::get_kanban_cards,
            commands::get_triggers,
            commands::save_triggers,
            commands::get_settings,
            commands::save_settings,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
