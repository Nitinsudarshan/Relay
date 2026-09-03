pub mod capture;
pub mod commands;
pub mod developer;
pub mod diagnostics;
pub mod hotkeys;
pub mod identity;
pub mod mcp;
pub mod meetings_v2;
pub mod oauth;
pub mod overlay;
pub mod pipeline;
pub mod providers;
pub mod settings;
pub mod sync;
pub mod talkback;
pub mod triggers;
pub mod tts;
pub mod updates;
pub mod vault;

use capture::{AudioRecorder, SttEngine};
use commands::AppState;
use settings::AppSettings;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use tauri::Manager;
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

    // Route whisper.cpp/GGML's own logging through whisper-rs's hooks. Without
    // this, every decode dumps its full token-by-token trace to the terminal,
    // which at the live clock's cadence buries everything else.
    #[cfg(feature = "whisper-local")]
    whisper_rs::install_logging_hooks();

    // A crash mid-synthesis leaves a WAV behind and nothing else will
    // ever remove it. One phrase per sentence makes that worth doing.
    let tts_root = tts::discovery::default_tts_root(&config_dir);
    tts::discovery::clear_scratch(&tts_root);
    // A process killed mid-download cannot run its own cleanup, and a
    // half-extracted engine left in staging would be mistaken for a
    // finished one by the next run.
    tts::installer::clear_staging(&tts_root);

    let stt = SttEngine::new();
    let meetings_v2 = Arc::new(meetings_v2::MeetingsV2Engine::new(
        vault_dir.clone(),
        stt.clone(),
    ));

    // Run startup crash recovery on launch: reconcile any interrupted recordings
    if let Ok(recovered) = meetings_v2.recover_interrupted_sessions() {
        if !recovered.is_empty() {
            tracing::info!(
                "Startup: Reconciled {} interrupted meeting recording session(s).",
                recovered.len()
            );
        }
    }

    let meeting_processor = Arc::new(meetings_v2::MeetingProcessor::new(meetings_v2.store()));

    let recorder = AudioRecorder::new();
    recorder.set_keep_warm_duration(settings.audio_input.parse_keep_warm_duration());

    let state = AppState {
        recorder,
        vault: VaultManager::new(vault_dir),
        default_vault_dir,
        config_dir,
        settings: Mutex::new(settings),
        stt,
        last_stt_diagnostics: Mutex::new(None),
        meetings_v2,
        meeting_processor,
        talkback: Arc::new(talkback::TalkbackEngine::new()),
        tts_root,
        voice_install: Arc::new(commands::VoiceInstall::default()),
        capture_bridge: Mutex::new(None),
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
                &hotkeys_config.capture_hotkey,
            );

            // Opens the loopback capture listener only when the user has
            // switched capture on; a fresh install opens no socket.
            commands::apply_capture_bridge(handle, &handle.state::<commands::AppState>());
            // The dictation pill is now the one, permanent PTT surface — no
            // more docked/floating product-mode choice to hide it behind
            // (see docs/decisions.md Decision 36) — so it's always shown.
            overlay::ensure_pill_window(handle, true, pill_position);

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
            commands::get_audio_devices,
            commands::get_settings,
            commands::save_settings,
            commands::open_settings_window,
            commands::get_voice_notes,
            commands::update_voice_note,
            commands::delete_voice_note,
            commands::delete_voice_notes,
            commands::merge_voice_notes,
            commands::unmerge_voice_note,
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
            commands::start_meeting_v2,
            commands::stop_meeting_v2,
            commands::pause_meeting_v2,
            commands::resume_meeting_v2,
            commands::get_active_meeting_v2,
            commands::list_meetings_v2,
            commands::get_meeting_v2,
            commands::get_meeting_v2_transcript,
            commands::get_meeting_v2_diagnostics,
            commands::delete_meeting_v2,
            commands::summarize_meeting_v2,
            commands::get_meeting_v2_processing,
            commands::prepare_meeting_v2,
            commands::generate_meeting_v2_summary,
            commands::rename_meeting_v2_speaker,
            commands::get_meeting_v2_notes,
            commands::save_meeting_v2_notes,
            commands::set_meeting_v2_action_item_status,
            commands::get_meeting_v2_related,
            commands::get_meeting_v2_processing_log,
            commands::get_meeting_v2_extensions,
            commands::list_meeting_v2_processing,
            commands::promote_meeting_v2_to_scribble,
            commands::push_meeting_v2_action_items_to_kanban,
            commands::set_meeting_overlay_expanded,
            commands::start_talkback,
            commands::stop_talkback,
            commands::get_talkback_state,
            commands::get_talkback_session,
            commands::submit_talkback_turn,
            commands::interrupt_talkback,
            commands::search_talkback_context,
            commands::get_tts_status,
            commands::browse_for_piper_binary,
            commands::browse_for_piper_voice,
            commands::set_tts_configuration,
            commands::test_tts_voice,
            commands::prepare_tts_folders,
            commands::install_local_voice,
            commands::cancel_voice_install,
            commands::import_vault_file,
            commands::import_vault_file_bytes,
            commands::analyze_vault_file,
            commands::get_vault_files,
            commands::get_vault_file,
            commands::summarize_vault_file,
            commands::enrich_vault_file,
            commands::update_vault_file_tags,
            commands::create_scribble_from_vault_file,
            commands::reprocess_vault_file,
            commands::delete_vault_file,
            commands::open_vault_file_location,
            commands::get_capture_bridge_status,
            commands::set_capture_bridge_enabled,
            commands::set_capture_bridge_port,
            commands::set_capture_analyze_on_capture,
            commands::regenerate_capture_pairing_token,
            commands::get_captures,
            commands::get_capture,
            commands::get_capture_payload,
            commands::renormalize_capture,
            commands::delete_capture,
            commands::import_web_capture,
            commands::get_capture_context,
            commands::analyze_capture_context,
            commands::inspect_ai_conversation_export,
            commands::import_ai_conversation_export,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
