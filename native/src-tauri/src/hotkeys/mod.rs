pub mod injection;

use crate::commands::{emit_capture_state, emit_capture_status_event, AppState};
use serde::Serialize;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, ShortcutState};

pub const MAIN_WINDOW_LABEL: &str = "main";

/// Broadcast after every (re-)registration attempt so any surface can show
/// the *actual* OS-level outcome instead of optimistically assuming
/// "registered" — a hotkey can fail to register (most commonly a conflict
/// with another app or the OS's own IME/input-switch binding on that exact
/// combination), and that failure was previously only ever logged, never
/// surfaced anywhere a user could see it.
pub const HOTKEY_STATUS_EVENT: &str = "hotkey-status-changed";

#[derive(Debug, Clone, Serialize)]
pub struct HotkeyRegistrationStatus {
    pub dictation_hotkey: String,
    pub dictation_registered: bool,
    pub dictation_error: Option<String>,
    pub show_hide_hotkey: String,
    pub show_hide_registered: bool,
    pub show_hide_error: Option<String>,
}

/// Some platforms/desktop environments can eat the key-up for a held
/// combination before Relay's global hook ever sees it (e.g. an OS or
/// input-method shortcut also bound to it), which would otherwise leave the
/// microphone "stuck" recording forever — and every later start attempt,
/// hotkey or UI, would then fail with "a recording session is already
/// active". If no release arrives within this long, force one. In
/// toggle-to-talk mode this also bounds a single recording that was never
/// stopped with a second press.
const MAX_DICTATION_HOLD: Duration = Duration::from_secs(60);

/// Toggle-to-talk exists specifically so longer recordings aren't tedious,
/// so it gets a much longer safety-net timeout than hold-to-talk's — a
/// backstop against a forgotten/stuck toggle, not a normal length limit.
const MAX_PERSISTENT_RECORDING: Duration = Duration::from_secs(600);

/// Tracks whether the dictation hotkey currently owns the microphone.
/// `generation` lets a delayed watchdog tell "this exact press" apart from
/// a later one, so it never force-releases a session that already ended
/// normally and started again. `key_down` tracks whether the physical key
/// is currently held (a press without a matching release yet), which is
/// what lets a deliberate second press — toggle-to-talk's "stop" signal —
/// be told apart from the OS re-firing "pressed" repeatedly while the key
/// stays physically down.
struct DictationState {
    active: bool,
    generation: u64,
    key_down: bool,
}

type SharedDictationState = Arc<Mutex<DictationState>>;

/// Registers Relay's two global (OS-wide) hotkeys used by push-to-talk
/// dictation.
///
/// - `show_hide_hotkey` toggles the main window's visibility from anywhere.
/// - `dictation_hotkey` is push-to-talk: by default, held down it records
///   and on release the transcript is typed into whatever field currently
///   has OS focus (not necessarily Relay's own window). If
///   `HotkeySettings::toggle_to_talk` is enabled, one press starts
///   recording and a second press stops it instead, with releasing the key
///   in between doing nothing — see `on_dictation_pressed`/
///   `on_dictation_released`. The floating dictation pill (see
///   `overlay::ensure_pill_window`) is the only visual surface for this —
///   there is no separate "listening" indicator window.
///
/// Safe to call again after [`apply_hotkeys`] has unregistered the previous
/// bindings — e.g. when the user changes a hotkey in Settings — since it
/// only ever registers, never assumes it's the first registration.
pub fn register_hotkeys(app: &AppHandle, show_hide_hotkey: &str, dictation_hotkey: &str) {
    try_register_hotkeys(app, show_hide_hotkey, dictation_hotkey);
}

/// Re-registers both hotkeys with new bindings, replacing whatever is
/// currently bound. Used both at startup and whenever Settings saves new
/// hotkeys — hotkeys take effect immediately, no app restart required.
pub fn apply_hotkeys(
    app: &AppHandle,
    show_hide_hotkey: &str,
    dictation_hotkey: &str,
) -> Result<(), String> {
    app.global_shortcut()
        .unregister_all()
        .map_err(|e| format!("Could not clear existing hotkeys: {}", e))?;
    let status = try_register_hotkeys(app, show_hide_hotkey, dictation_hotkey);
    if !status.dictation_registered {
        return Err(status
            .dictation_error
            .unwrap_or_else(|| "Dictation hotkey registration failed".to_string()));
    }
    if !status.show_hide_registered {
        return Err(status
            .show_hide_error
            .unwrap_or_else(|| "Show/hide hotkey registration failed".to_string()));
    }
    Ok(())
}

/// Registers each hotkey *independently* — one binding failing (e.g. a
/// conflict with another app, or an OS-reserved combination such as
/// Ctrl+Space being bound to IME/input-language switching on some Windows
/// locales) must never prevent the other from being attempted. Previously
/// both were chained with `?` through one `Result`, so a failure on
/// `show_hide_hotkey` silently skipped registering `dictation_hotkey`
/// entirely — the dictation hotkey could end up completely unregistered
/// with no error ever reaching anywhere visible.
fn try_register_hotkeys(
    app: &AppHandle,
    show_hide_hotkey: &str,
    dictation_hotkey: &str,
) -> HotkeyRegistrationStatus {
    let show_hide_app = app.clone();
    let show_hide_result = app
        .global_shortcut()
        .on_shortcut(show_hide_hotkey, move |_app, _shortcut, event| {
            if event.state == ShortcutState::Pressed {
                toggle_main_window(&show_hide_app);
            }
        })
        .map_err(|e| format!("show/hide hotkey '{}': {}", show_hide_hotkey, e));
    match &show_hide_result {
        Ok(()) => tracing::info!("[Hotkey] Registered show/hide hotkey '{}'", show_hide_hotkey),
        Err(e) => tracing::error!("[Hotkey] Failed to register show/hide hotkey: {}", e),
    }

    let dictation_state: SharedDictationState = Arc::new(Mutex::new(DictationState {
        active: false,
        generation: 0,
        key_down: false,
    }));
    let dictation_result = app
        .global_shortcut()
        .on_shortcut(
            dictation_hotkey,
            move |app, _shortcut, event| match event.state {
                ShortcutState::Pressed => on_dictation_pressed(app, &dictation_state),
                ShortcutState::Released => on_dictation_released(app, &dictation_state),
            },
        )
        .map_err(|e| format!("dictation hotkey '{}': {}", dictation_hotkey, e));
    match &dictation_result {
        Ok(()) => tracing::info!("[Hotkey] Registered dictation hotkey '{}'", dictation_hotkey),
        Err(e) => tracing::error!("[Hotkey] Failed to register dictation hotkey: {}", e),
    }

    let status = HotkeyRegistrationStatus {
        dictation_hotkey: dictation_hotkey.to_string(),
        dictation_registered: dictation_result.is_ok(),
        dictation_error: dictation_result.err(),
        show_hide_hotkey: show_hide_hotkey.to_string(),
        show_hide_registered: show_hide_result.is_ok(),
        show_hide_error: show_hide_result.err(),
    };
    let _ = app.emit(HOTKEY_STATUS_EVENT, &status);
    status
}

fn toggle_main_window(app: &AppHandle) {
    let Some(window) = app.get_webview_window(MAIN_WINDOW_LABEL) else {
        return;
    };
    let is_visible = window.is_visible().unwrap_or(false);
    if is_visible {
        let _ = window.hide();
    } else {
        let _ = window.show();
        let _ = window.set_focus();
    }
}

fn on_dictation_pressed(app: &AppHandle, dictation_state: &SharedDictationState) {
    tracing::debug!("[Hotkey] Ctrl+Space received (pressed)");

    let (is_repeat, session_active) = {
        let mut guard = dictation_state.lock().unwrap();
        let is_repeat = guard.key_down;
        guard.key_down = true;
        (is_repeat, guard.active)
    };
    if is_repeat {
        // OS key-repeat re-fires "pressed" while physically still held;
        // ignore in both hold-to-talk and toggle-to-talk mode.
        return;
    }

    let state = app.state::<AppState>();
    let toggle_to_talk = state.settings.lock().unwrap().hotkeys.toggle_to_talk;

    if session_active {
        // Only reachable in toggle-to-talk mode — a hold-to-talk session is
        // always stopped by its own key release before a genuine next press
        // could land. This fresh press is the user's "stop now" signal.
        if toggle_to_talk {
            stop_dictation_session(app.clone(), dictation_state.clone(), None);
        }
        return;
    }

    let generation = {
        let mut guard = dictation_state.lock().unwrap();
        guard.active = true;
        guard.generation += 1;
        guard.generation
    };

    tracing::debug!("[Dictation] Start requested via hotkey");
    let audio_dir = state.config_dir.join("audio");
    match state.recorder.start("dictation", &audio_dir, Some(app.clone())) {
        Ok(_) => {
            tracing::debug!("[Audio] Capture started");
            // `emit_capture_state` broadcasts `active: true`, which the
            // floating dictation pill reacts to by expanding — its window
            // always exists (just possibly hidden-and-idle) and is always
            // on top, so this alone is enough to make a hotkey-triggered
            // recording visible; there's no docked/main-window fallback to
            // compensate for anymore (see docs/decisions.md Decision 36).
            emit_capture_state(app, &state.recorder);
            let timeout = if toggle_to_talk {
                MAX_PERSISTENT_RECORDING
            } else {
                MAX_DICTATION_HOLD
            };
            spawn_release_watchdog(app.clone(), dictation_state.clone(), generation, timeout);
        }
        Err(e) => {
            // Most commonly: the in-app Click-to-dictate button already
            // owns the microphone. Back off quietly rather than erroring —
            // the hotkey will simply work again once that session ends.
            tracing::info!("Dictation hotkey could not start capture: {}", e);
            dictation_state.lock().unwrap().active = false;
        }
    }
}

/// Handles the dictation hotkey's key-up. In the default hold-to-talk mode
/// this is what stops recording. In toggle-to-talk mode
/// (`HotkeySettings::toggle_to_talk`), releasing the key never stops
/// recording by itself — only a subsequent press does, handled in
/// `on_dictation_pressed` — so this only clears the "physically held" flag.
fn on_dictation_released(app: &AppHandle, dictation_state: &SharedDictationState) {
    {
        let mut guard = dictation_state.lock().unwrap();
        guard.key_down = false;
    }

    let state = app.state::<AppState>();
    let toggle_to_talk = state.settings.lock().unwrap().hotkeys.toggle_to_talk;
    if toggle_to_talk {
        return;
    }

    stop_dictation_session(app.clone(), dictation_state.clone(), None);
}

/// Stops the current dictation session (if any) and, in the background,
/// transcribes and injects the result. `expected_generation` is set only
/// when called from the release watchdog, so it can confirm the session
/// it's about to force-stop is still the one it started — never a later,
/// legitimately-in-progress one.
fn stop_dictation_session(
    app: AppHandle,
    dictation_state: SharedDictationState,
    expected_generation: Option<u64>,
) {
    {
        let mut guard = dictation_state.lock().unwrap();
        if !guard.active {
            return;
        }
        if let Some(expected) = expected_generation {
            if guard.generation != expected {
                return;
            }
        }
        guard.active = false;
    }

    tauri::async_runtime::spawn(async move {
        let state = app.state::<AppState>();
        let captured = match state.recorder.stop().await {
            Ok(c) => c,
            Err(e) => {
                tracing::warn!("Dictation capture failed to stop cleanly: {}", e);
                emit_capture_status_event(&app, false, None, "ERROR", Some(e.to_string()));
                return;
            }
        };
        if !captured.had_audio {
            // Recording genuinely happened, but nothing crossed the mic
            // input threshold the whole time it was open — never hand
            // silence to Whisper (which can hallucinate text on it) and
            // never claim to be transcribing something that was never said.
            tracing::info!("[Dictation] Recording stopped with no audio input");
            emit_capture_status_event(
                &app,
                false,
                Some(captured.mode.clone()),
                "NO_SPEECH",
                None,
            );
            return;
        }

        // The mic has stopped but there's real work left (transcription,
        // then injection) — without this, the pill would flash straight
        // back to idle and stay silent while that happens.
        emit_capture_status_event(
            &app,
            false,
            Some(captured.mode.clone()),
            "TRANSCRIBING",
            None,
        );

        let configured_model_path = state
            .settings
            .lock()
            .unwrap()
            .stt
            .whisper_model_path
            .clone()
            .filter(|p| !p.trim().is_empty());
        let models_dir = state.config_dir.join("models");
        let model_path = match configured_model_path {
            Some(path) => {
                let p = std::path::Path::new(&path);
                if crate::capture::stt::is_legacy_default_model(p) {
                    match crate::capture::stt::ensure_default_model(&models_dir).await {
                        Ok(small_path) => {
                            let path_str = small_path.to_string_lossy().to_string();
                            let settings_path = state.config_dir.join("settings.json");
                            let mut guard = state.settings.lock().unwrap();
                            guard.stt.whisper_model_path = Some(path_str.clone());
                            let _ = guard.save(&settings_path);
                            Some(path_str)
                        }
                        Err(_) => Some(path),
                    }
                } else {
                    Some(path)
                }
            }
            None => {
                match crate::capture::stt::ensure_default_model(&models_dir).await {
                    Ok(path) => {
                        let path_str = path.to_string_lossy().to_string();
                        let settings_path = state.config_dir.join("settings.json");
                        let mut guard = state.settings.lock().unwrap();
                        guard.stt.whisper_model_path = Some(path_str.clone());
                        let _ = guard.save(&settings_path);
                        Some(path_str)
                    }
                    Err(e) => {
                        tracing::warn!("Could not auto-provision production Whisper model: {}", e);
                        None
                    }
                }
            }
        };
        let language_settings = state.settings.lock().unwrap().language.clone();
        let stt_settings = state.settings.lock().unwrap().stt.clone();
        let language_config = crate::capture::SttLanguageConfig::from_settings(&language_settings);
        let decoding_config = crate::capture::stt::WhisperDecodingConfig::from_settings(&stt_settings);

        let stt = state.stt.clone();
        let samples = captured.samples.clone();
        let mp_clone = model_path.clone();
        let lang_clone = language_config.clone();
        let dec_clone = decoding_config.clone();

        let (text_res, diag, err) = tauri::async_runtime::spawn_blocking(move || {
            match stt.transcribe_with_config(
                mp_clone.as_deref(),
                &samples,
                &lang_clone,
                &dec_clone,
            ) {
                Ok((t, d)) => (t, Some(d), None),
                Err(e) => (String::new(), None, Some(e.to_string())),
            }
        })
        .await
        .unwrap_or_else(|e| (String::new(), None, Some(e.to_string())));

        let model_str = model_path.as_deref().unwrap_or(crate::capture::stt::DEFAULT_MODEL_FILENAME);
        let snapshot = crate::capture::build_diagnostic_snapshot(
            &captured.mode,
            Some(captured.audio_path.clone()),
            &captured,
            &language_settings,
            &language_config,
            &decoding_config,
            model_str,
            &text_res,
            diag.as_ref(),
            err.clone(),
        );
        crate::commands::record_stt_diagnostics(&app, &state, snapshot);

        if let Some(err_msg) = err {
            tracing::error!("Dictation transcription failed: {}", err_msg);
            emit_capture_status_event(
                &app,
                false,
                None,
                "ERROR",
                Some("Transcription failed".to_string()),
            );
            return;
        }

        if !text_res.trim().is_empty() {
            // Voice Note persistence happens from the successful
            // transcript itself, not from injection's outcome — it must
            // still be saved below even if injection fails.
            crate::commands::save_voice_note(&app, &state.vault, &text_res);
            match injection::inject_text(&text_res) {
                Ok(()) => {
                    emit_capture_status_event(&app, false, None, "SUCCESS", None);
                }
                Err(e) => {
                    tracing::error!("Dictation text injection failed: {}", e);
                    emit_capture_status_event(
                        &app,
                        false,
                        None,
                        "ERROR",
                        Some("Couldn't insert text".to_string()),
                    );
                }
            }
        } else {
            tracing::info!("Dictation produced no speech (silence or too short)");
            emit_capture_status_event(&app, false, None, "NO_SPEECH", None);
        }
    });
}

fn spawn_release_watchdog(
    app: AppHandle,
    dictation_state: SharedDictationState,
    generation: u64,
    timeout: Duration,
) {
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(timeout).await;
        let still_pending = {
            let guard = dictation_state.lock().unwrap();
            guard.active && guard.generation == generation
        };
        if still_pending {
            tracing::warn!(
                "Dictation session exceeded {:?} without being stopped — forcing stop so the microphone isn't left stuck.",
                timeout
            );
            stop_dictation_session(app, dictation_state, Some(generation));
        }
    });
}

