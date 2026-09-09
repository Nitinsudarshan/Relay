pub mod actions;
pub mod capture;
pub mod commands;
pub mod context;
pub mod developer;
pub mod diagnostics;
pub mod entities;
pub mod hotkeys;
pub mod identity;
pub mod mcp;
pub mod memory;
pub mod oauth;
pub mod overlay;
pub mod pipeline;
pub mod providers;
pub mod relationships;
pub mod retrieval;
pub mod settings;
pub mod startup;
pub mod sync;
pub mod triggers;
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

    // The ONNX packaging spike, when this binary was built with it. First
    // thing, before any window or plugin, because what it is measuring is
    // whether the ONNX Runtime this executable was *shipped with* can be
    // loaded at all — and that answer must survive anything else failing.
    #[cfg(feature = "onnx-spike")]
    developer::onnx_spike::run_and_record();
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
    let startup_config = settings.startup.clone();

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

    let stt = SttEngine::new();
    let recorder = AudioRecorder::new();
    recorder.set_keep_warm_duration(settings.audio_input.parse_keep_warm_duration());
    crate::capture::device::set_preference(&settings.audio_input);

    let memory_store = Arc::new(memory::MemoryStore::new(&vault_dir));
    let relationship_store = Arc::new(relationships::RelationshipStore::new(&vault_dir));
    let entity_store = Arc::new(entities::EntityStore::new(&vault_dir));

    let state = AppState {
        recorder,
        vault: VaultManager::new(vault_dir),
        default_vault_dir,
        config_dir,
        settings: Mutex::new(settings),
        stt,
        last_stt_diagnostics: Mutex::new(None),
        last_dictation: Mutex::new(None),
        capture_bridge: Mutex::new(None),
        memory_store,
        relationship_store,
        entity_store,
    };

    tauri::Builder::default()
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_opener::init())
        // Writes the OS launch entry for Settings › Startup › "Launch at
        // login". Registered here; driven from `startup::reconcile_launch_at_login`,
        // never from the frontend — the plugin's JS commands are deliberately
        // not granted in `capabilities/`.
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            None,
        ))
        .manage(state)
        .setup(move |app| {
            let handle = app.handle();

            // First, and before anything that can fail: the main window is
            // configured hidden so "start minimized" does not flash the
            // control panel on screen, which means *something* has to show it.
            // A tray builder that errors below must not be what decides
            // whether Relay has a window.
            startup::apply_at_launch(handle, &startup_config);

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
            commands::get_available_llm_models,
            commands::test_llm_prompt,
            commands::ensure_stt_model_ready,
            commands::get_stt_decode_summary,
            commands::rewrite_dictation,
            commands::get_cleanup_target,
            commands::apply_dictation_cleanup,
            commands::get_available_stt_models,
            commands::download_stt_model,
            commands::test_stt_model,
            commands::copy_to_clipboard,
            commands::get_kanban_cards,
            commands::get_triggers,
            commands::save_triggers,
            commands::get_audio_devices,
            commands::get_settings,
            commands::save_settings,
            commands::open_settings_window,
            commands::get_voice_notes,
            commands::update_voice_note,
            commands::correct_voice_note_phrase,
            commands::undo_voice_note_correction,
            commands::add_dictionary_word,
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
            commands::get_account_state,
            commands::start_google_sign_in,
            commands::sign_out_account,
            commands::delete_relay_account,
            commands::get_installation_info,
            commands::check_for_app_updates,
            commands::set_diagnostics_consent,
            commands::complete_first_run,
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
            commands::inspect_ai_conversation_export_bytes,
            commands::import_ai_conversation_export,
            commands::import_ai_conversation_export_bytes,
            commands::pick_ai_conversation_export_file,
            commands::open_external_url,
            commands::unified_retrieve,
            commands::assemble_context_pack,
            commands::get_knowledge_telemetry,
            commands::form_memory_candidate,
            commands::list_entities,
            commands::list_memories,
            commands::create_memory,
            commands::supersede_memory,
            commands::extract_and_resolve_entities,
            commands::dispatch_universal_action,
            commands::list_relationships,
            commands::add_relationship,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
