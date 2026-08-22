pub mod capture;
pub mod commands;
pub mod diagnostics;
pub mod hotkeys;
pub mod identity;
pub mod mcp;
pub mod meetings;
pub mod overlay;
pub mod pipeline;
pub mod providers;
pub mod settings;
pub mod triggers;
pub mod tts;
pub mod updates;
pub mod vault;

use capture::{AudioRecorder, SttEngine};
use commands::AppState;
use settings::AppSettings;
use std::path::PathBuf;
use std::sync::Mutex;
use vault::VaultManager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Load environment variables from .env if present
    dotenvy::dotenv().ok();

    let base_dir = std::env::current_dir()
        .unwrap_or_else(|_| PathBuf::from("."))
        .join(".relay");

    let default_vault_dir = base_dir.join("vault");
    let config_dir = base_dir.join("config");

    let settings = AppSettings::load(&config_dir.join("settings.json")).unwrap_or_default();
    let hotkeys_config = settings.hotkeys.clone();
    let pill_position = settings.ui.pill_position;

    // An explicitly configured Vault Directory Location always wins; a
    // fresh install (or one where the user never confirmed a location)
    // keeps using the same process-relative default this already used
    // before Voice Notes existed, so existing notes/Kanban cards never
    // silently move.
    let vault_dir = settings
        .vault
        .directory
        .as_ref()
        .map(PathBuf::from)
        .unwrap_or_else(|| default_vault_dir.clone());

    let state = AppState {
        recorder: AudioRecorder::new(),
        vault: VaultManager::new(vault_dir),
        default_vault_dir,
        config_dir,
        settings: Mutex::new(settings),
        stt: SttEngine::new(),
        last_stt_diagnostics: Mutex::new(None),
    };

    tauri::Builder::default()
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .plugin(tauri_plugin_dialog::init())
        .manage(state)
        .setup(move |app| {
            hotkeys::register_hotkeys(
                app.handle(),
                &hotkeys_config.show_hide_hotkey,
                &hotkeys_config.dictation_hotkey,
            );
            // The dictation pill is now the one, permanent PTT surface — no
            // more docked/floating product-mode choice to hide it behind
            // (see docs/decisions.md Decision 36) — so it's always shown.
            overlay::ensure_pill_window(app.handle(), true, pill_position);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::start_capture,
            commands::stop_capture,
            commands::get_capture_status,
            commands::update_hotkeys,
            commands::set_pill_position,
            commands::set_pill_expanded,
            commands::set_pill_window_mode,
            commands::ensure_local_llm_ready,
            commands::ensure_stt_model_ready,
            commands::get_kanban_cards,
            commands::get_triggers,
            commands::save_triggers,
            commands::get_settings,
            commands::save_settings,
            commands::open_settings_window,
            commands::get_voice_notes,
            commands::update_voice_note,
            commands::delete_voice_note,
            commands::merge_voice_notes,
            commands::get_vault_location,
            commands::choose_vault_folder,
            commands::set_vault_location,
            commands::get_app_version,
            commands::get_changelog,
            commands::diagnose_stt_variants,
            commands::run_stt_evaluation,
            commands::get_last_stt_diagnostics,
            commands::get_stt_corpus,
            commands::get_scribbles,
            commands::get_scribble,
            commands::create_scribble,
            commands::promote_voice_note_to_scribble,
            commands::create_file_scribble,
            commands::update_scribble,
            commands::delete_scribble,
            commands::merge_scribbles,
            commands::get_trash_items,
            commands::restore_trash_item,
            commands::delete_trash_item_permanently,
            commands::empty_trash,
            commands::add_scribble_relationship,
            commands::remove_scribble_relationship,
            commands::search_knowledge,
            commands::get_knowledge_graph,
            commands::trigger_enrich_scribble,
            commands::summarize_scribble,
            commands::get_meetings,
            commands::get_meeting,
            commands::create_meeting,
            commands::save_meeting,
            commands::update_meeting,
            commands::delete_meeting,
            commands::get_meeting_series,
            commands::save_meeting_series,
            commands::delete_meeting_series,
            commands::start_meeting_recording,
            commands::stop_meeting_recording,
            commands::trigger_enrich_meeting,
            commands::create_scribble_from_meeting,
            commands::get_upcoming_calendar_events,
            commands::check_meeting_detection,
            commands::get_calendar_connection_status,
            commands::start_google_calendar_oauth,
            commands::disconnect_google_calendar,
            commands::sync_google_calendar,
            commands::get_google_oauth_config,
            commands::save_google_oauth_config,
            commands::get_account_state,
            commands::start_google_sign_in,
            commands::sign_out_account,
            commands::get_installation_info,
            commands::check_for_app_updates,
            commands::set_diagnostics_consent,
            commands::complete_first_run,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
