pub mod capture;
pub mod commands;
pub mod developer;
pub mod diagnostics;
pub mod hotkeys;
pub mod identity;
pub mod mcp;
pub mod meetings;
pub mod oauth;
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
use tauri::{Emitter, Manager};
use vault::VaultManager;

#[cfg(target_os = "windows")]
fn set_app_user_model_id() {
    use std::ffi::OsStr;
    use std::os::windows::ffi::OsStrExt;

    let app_id: Vec<u16> = OsStr::new("com.relay.app")
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();

    unsafe {
        #[link(name = "shell32")]
        extern "system" {
            fn SetCurrentProcessExplicitAppUserModelID(AppID: *const u16) -> i32;
        }
        let _ = SetCurrentProcessExplicitAppUserModelID(app_id.as_ptr());
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    #[cfg(target_os = "windows")]
    set_app_user_model_id();
    // Load environment variables from .env — search CWD and ancestor directories
    // so the repo-root .env is found even when Tauri runs from native/src-tauri/.
    if dotenvy::dotenv().is_err() {
        // Walk up parent directories looking for .env
        if let Ok(mut dir) = std::env::current_dir() {
            loop {
                let candidate = dir.join(".env");
                if candidate.exists() {
                    let _ = dotenvy::from_filename_override(candidate);
                    break;
                }
                if !dir.pop() {
                    break;
                }
            }
        }
    }

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
        .plugin(tauri_plugin_notification::init())
        .manage(state)
        .setup(move |app| {
            let handle = app.handle();
            let quit_i = tauri::menu::MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
            let show_i = tauri::menu::MenuItem::with_id(app, "show", "Show Relay", true, None::<&str>)?;
            let record_i = tauri::menu::MenuItem::with_id(app, "record", "Start Recording", true, None::<&str>)?;
            
            let menu = tauri::menu::Menu::with_items(app, &[&show_i, &record_i, &quit_i])?;
            
            let _tray = tauri::tray::TrayIconBuilder::new()
                .menu(&menu)
                .on_menu_event(|app, event| match event.id.as_ref() {
                    "quit" => {
                        app.exit(0);
                    }
                    "show" => {
                        if let Some(window) = app.get_webview_window("main") {
                            let _ = window.unminimize();
                            let _ = window.show();
                            let _ = window.set_focus();
                        }
                    }
                    "record" => {
                        // Wired to the same meeting the reminder popup is
                        // currently showing (soonest/active), if any — the
                        // frontend's shared `startMeetingRecording` handles
                        // it identically to the popup's own button
                        // (meetings_implementation.md §4.2). Previously
                        // this emitted an event nothing listened for
                        // (Decision 45, Broken #3b).
                        if let Some(reminders) = app.try_state::<crate::meetings::reminders::ReminderQueue>() {
                            if let Some(current) = crate::meetings::reminders::current_popup_reminder(&reminders) {
                                let _ = app.emit("start-meeting-recording-for", &current.meeting_id);
                            }
                        }
                        if let Some(window) = app.get_webview_window("main") {
                            let _ = window.unminimize();
                            let _ = window.show();
                            let _ = window.set_focus();
                        }
                    }
                    _ => {}
                })
                .build(app)?;

            hotkeys::register_hotkeys(
                handle,
                &hotkeys_config.show_hide_hotkey,
                &hotkeys_config.dictation_hotkey,
            );
            // The dictation pill is now the one, permanent PTT surface — no
            // more docked/floating product-mode choice to hide it behind
            // (see docs/decisions.md Decision 36) — so it's always shown.
            overlay::ensure_pill_window(handle, true, pill_position);
            // Create the meeting reminder overlay once, hidden, at startup
            overlay::ensure_reminder_window(handle);

            crate::meetings::engine::start(handle.clone());
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
            commands::import_calendar_event,
            commands::dismiss_meeting_reminder,
            commands::snooze_meeting_reminder,
            commands::get_pending_meeting_reminder,
            commands::meeting_reminder_ready,
            commands::meeting_reminder_hover_changed,
            commands::get_current_meeting_reminder,
            commands::trigger_mock_meeting_reminder,
            commands::get_active_recording_meeting_id,
            commands::debug_detect_conferencing_windows,
            commands::get_calendar_connection_status,
            commands::start_google_calendar_oauth,
            commands::disconnect_google_calendar,
            commands::sync_google_calendar,
            commands::get_relay_profile,
            commands::update_profile_display_name,
            commands::complete_profile_onboarding,
            commands::get_developer_settings,
            commands::set_developer_force_onboarding,
            commands::set_developer_notification_surface_mode,
            commands::get_account_state,
            commands::start_google_sign_in,
            commands::sign_out_account,
            commands::delete_relay_account,
            commands::get_installation_info,
            commands::check_for_app_updates,
            commands::set_diagnostics_consent,
            commands::complete_first_run,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
