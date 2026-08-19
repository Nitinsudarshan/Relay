pub mod injection;

use crate::commands::{emit_capture_state, emit_capture_status_event, AppState};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tauri::{AppHandle, Manager};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, ShortcutState};

pub const MAIN_WINDOW_LABEL: &str = "main";

/// Some platforms/desktop environments can eat the key-up for a held
/// combination before Relay's global hook ever sees it (e.g. an OS or
/// input-method shortcut also bound to it), which would otherwise leave the
/// microphone "stuck" recording forever — and every later start attempt,
/// hotkey or UI, would then fail with "a recording session is already
/// active". If no release arrives within this long, force one.
const MAX_DICTATION_HOLD: Duration = Duration::from_secs(60);

/// Tracks whether the dictation hotkey currently owns the microphone.
/// `generation` lets a delayed watchdog tell "this exact press" apart from
/// a later one, so it never force-releases a session that already ended
/// normally and started again.
struct DictationState {
    active: bool,
    generation: u64,
}

type SharedDictationState = Arc<Mutex<DictationState>>;

/// Registers Relay's two global (OS-wide) hotkeys used by push-to-talk
/// dictation.
///
/// - `show_hide_hotkey` toggles the main window's visibility from anywhere.
/// - `dictation_hotkey` is push-to-talk: held down it records, and on
///   release the transcript is typed into whatever field currently has OS
///   focus (not necessarily Relay's own window). The floating dictation
///   pill (see `overlay::ensure_pill_window`) is the only visual surface
///   for this — there is no separate "listening" indicator window.
///
/// Safe to call again after [`apply_hotkeys`] has unregistered the previous
/// bindings — e.g. when the user changes a hotkey in Settings — since it
/// only ever registers, never assumes it's the first registration.
pub fn register_hotkeys(app: &AppHandle, show_hide_hotkey: &str, dictation_hotkey: &str) {
    if let Err(e) = try_register_hotkeys(app, show_hide_hotkey, dictation_hotkey) {
        tracing::error!("Failed to register hotkeys: {}", e);
    }
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
    try_register_hotkeys(app, show_hide_hotkey, dictation_hotkey)
}

fn try_register_hotkeys(
    app: &AppHandle,
    show_hide_hotkey: &str,
    dictation_hotkey: &str,
) -> Result<(), String> {
    let show_hide_app = app.clone();
    app.global_shortcut()
        .on_shortcut(show_hide_hotkey, move |_app, _shortcut, event| {
            if event.state == ShortcutState::Pressed {
                toggle_main_window(&show_hide_app);
            }
        })
        .map_err(|e| format!("show/hide hotkey '{}': {}", show_hide_hotkey, e))?;

    let dictation_state: SharedDictationState = Arc::new(Mutex::new(DictationState {
        active: false,
        generation: 0,
    }));
    app.global_shortcut()
        .on_shortcut(
            dictation_hotkey,
            move |app, _shortcut, event| match event.state {
                ShortcutState::Pressed => on_dictation_pressed(app, &dictation_state),
                ShortcutState::Released => on_dictation_released(app, &dictation_state, None),
            },
        )
        .map_err(|e| format!("dictation hotkey '{}': {}", dictation_hotkey, e))?;

    Ok(())
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
    let generation = {
        let mut guard = dictation_state.lock().unwrap();
        if guard.active {
            // Key-repeat re-fires "pressed" while held on some platforms; ignore.
            return;
        }
        guard.active = true;
        guard.generation += 1;
        guard.generation
    };

    let state = app.state::<AppState>();
    let audio_dir = state.config_dir.join("audio");
    match state.recorder.start("dictation", &audio_dir, Some(app.clone())) {
        Ok(_) => {
            // `emit_capture_state` broadcasts `active: true`; the floating
            // pill (the only PTT visual surface) reacts to that itself by
            // expanding — no separate window to show here.
            emit_capture_state(app, &state.recorder);
            spawn_release_watchdog(app.clone(), dictation_state.clone(), generation);
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

/// `expected_generation` is set only when called from the watchdog, so it
/// can confirm the session it's about to force-stop is still the one it
/// started — never a later, legitimately-in-progress one.
fn on_dictation_released(
    app: &AppHandle,
    dictation_state: &SharedDictationState,
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

    let app_handle = app.clone();
    tauri::async_runtime::spawn(async move {
        let state = app_handle.state::<AppState>();
        let captured = match state.recorder.stop().await {
            Ok(c) => c,
            Err(e) => {
                tracing::warn!("Dictation capture failed to stop cleanly: {}", e);
                emit_capture_status_event(
                    &app_handle,
                    false,
                    None,
                    "ERROR",
                    Some(e.to_string()),
                );
                return;
            }
        };
        // The mic has stopped but there's real work left (transcription,
        // then injection) — without this, the pill would flash straight
        // back to idle on key-up and stay silent while that happens.
        emit_capture_status_event(
            &app_handle,
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
        let model_path = match configured_model_path {
            Some(path) => Some(path),
            None => {
                let models_dir = state.config_dir.join("models");
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
                        tracing::warn!("Could not auto-provision a default Whisper model: {}", e);
                        None
                    }
                }
            }
        };
        let stt = state.stt.clone();
        let samples = captured.samples;

        let transcript = tauri::async_runtime::spawn_blocking(move || {
            stt.transcribe(model_path.as_deref(), &samples)
        })
        .await;

        match transcript {
            Ok(Ok(text)) if !text.trim().is_empty() => match injection::inject_text(&text) {
                Ok(()) => {
                    emit_capture_status_event(&app_handle, false, None, "SUCCESS", None);
                }
                Err(e) => {
                    tracing::error!("Dictation text injection failed: {}", e);
                    emit_capture_status_event(
                        &app_handle,
                        false,
                        None,
                        "ERROR",
                        Some("Couldn't insert text".to_string()),
                    );
                }
            },
            Ok(Ok(_)) => {
                tracing::info!("Dictation produced no speech (silence or too short)");
                emit_capture_status_event(&app_handle, false, None, "NO_SPEECH", None);
            }
            Ok(Err(e)) => {
                tracing::error!("Dictation transcription failed: {}", e);
                emit_capture_status_event(
                    &app_handle,
                    false,
                    None,
                    "ERROR",
                    Some("Transcription failed".to_string()),
                );
            }
            Err(e) => {
                tracing::error!("Dictation transcription task panicked: {}", e);
                emit_capture_status_event(
                    &app_handle,
                    false,
                    None,
                    "ERROR",
                    Some("Transcription failed".to_string()),
                );
            }
        }
    });
}

fn spawn_release_watchdog(app: AppHandle, dictation_state: SharedDictationState, generation: u64) {
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(MAX_DICTATION_HOLD).await;
        let still_pending = {
            let guard = dictation_state.lock().unwrap();
            guard.active && guard.generation == generation
        };
        if still_pending {
            tracing::warn!(
                "Dictation hotkey held past {:?} without a release event — forcing stop so the microphone isn't left stuck.",
                MAX_DICTATION_HOLD
            );
            on_dictation_released(&app, &dictation_state, Some(generation));
        }
    });
}

