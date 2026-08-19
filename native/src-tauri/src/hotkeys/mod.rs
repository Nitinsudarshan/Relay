pub mod injection;

use crate::commands::AppState;
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Manager, WebviewUrl, WebviewWindowBuilder};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, ShortcutState};

pub const INDICATOR_WINDOW_LABEL: &str = "dictation-indicator";
pub const MAIN_WINDOW_LABEL: &str = "main";

/// Registers Relay's two global (OS-wide) hotkeys and the always-on-top
/// listening indicator window used by push-to-talk dictation.
///
/// - `show_hide_hotkey` toggles the main window's visibility from anywhere.
/// - `dictation_hotkey` is push-to-talk: held down it records, and on
///   release the transcript is typed into whatever field currently has OS
///   focus (not necessarily Relay's own window).
pub fn register_hotkeys(app: &AppHandle, show_hide_hotkey: &str, dictation_hotkey: &str) {
    ensure_indicator_window(app);

    let show_hide_app = app.clone();
    if let Err(e) =
        app.global_shortcut()
            .on_shortcut(show_hide_hotkey, move |_app, _shortcut, event| {
                if event.state == ShortcutState::Pressed {
                    toggle_main_window(&show_hide_app);
                }
            })
    {
        tracing::error!(
            "Failed to register show/hide hotkey '{}': {}",
            show_hide_hotkey,
            e
        );
    }

    let dictation_active = Arc::new(Mutex::new(false));
    if let Err(e) = app.global_shortcut().on_shortcut(
        dictation_hotkey,
        move |app, _shortcut, event| match event.state {
            ShortcutState::Pressed => on_dictation_pressed(app, &dictation_active),
            ShortcutState::Released => on_dictation_released(app, &dictation_active),
        },
    ) {
        tracing::error!(
            "Failed to register dictation hotkey '{}': {}",
            dictation_hotkey,
            e
        );
    }
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

fn on_dictation_pressed(app: &AppHandle, dictation_active: &Arc<Mutex<bool>>) {
    let mut active = dictation_active.lock().unwrap();
    if *active {
        // Key-repeat re-fires "pressed" while held on some platforms; ignore.
        return;
    }

    let state = app.state::<AppState>();
    let audio_dir = state.config_dir.join("audio");
    match state.recorder.start("dictation", &audio_dir) {
        Ok(_) => {
            *active = true;
            show_indicator(app);
        }
        Err(e) => {
            tracing::warn!("Could not start dictation capture: {}", e);
        }
    }
}

fn on_dictation_released(app: &AppHandle, dictation_active: &Arc<Mutex<bool>>) {
    {
        let mut active = dictation_active.lock().unwrap();
        if !*active {
            return;
        }
        *active = false;
    }

    hide_indicator(app);

    let app_handle = app.clone();
    tauri::async_runtime::spawn(async move {
        let state = app_handle.state::<AppState>();
        let captured = match state.recorder.stop().await {
            Ok(c) => c,
            Err(e) => {
                tracing::warn!("Dictation capture failed to stop cleanly: {}", e);
                return;
            }
        };

        let model_path = state
            .settings
            .lock()
            .unwrap()
            .stt
            .whisper_model_path
            .clone();
        let stt = state.stt.clone();
        let samples = captured.samples;

        let transcript = tauri::async_runtime::spawn_blocking(move || {
            stt.transcribe(model_path.as_deref(), &samples)
        })
        .await;

        match transcript {
            Ok(Ok(text)) if !text.trim().is_empty() => {
                if let Err(e) = injection::inject_text(&text) {
                    tracing::error!("Dictation text injection failed: {}", e);
                }
            }
            Ok(Ok(_)) => tracing::info!("Dictation produced no speech (silence or too short)"),
            Ok(Err(e)) => tracing::error!("Dictation transcription failed: {}", e),
            Err(e) => tracing::error!("Dictation transcription task panicked: {}", e),
        }
    });
}

fn ensure_indicator_window(app: &AppHandle) {
    if app.get_webview_window(INDICATOR_WINDOW_LABEL).is_some() {
        return;
    }

    let (width, height) = (240.0, 64.0);
    let position = compute_bottom_right_position(app, width, height);

    let mut builder = WebviewWindowBuilder::new(
        app,
        INDICATOR_WINDOW_LABEL,
        WebviewUrl::App("index.html#/dictation-indicator".into()),
    )
    .title("Relay — Listening")
    .inner_size(width, height)
    .resizable(false)
    .decorations(false)
    .always_on_top(true)
    .skip_taskbar(true)
    .transparent(true)
    .shadow(false)
    .visible(false)
    // Critical: must never steal OS focus, or the dictated text would type
    // into this indicator instead of whatever field the user was in.
    .focused(false);

    if let Some((x, y)) = position {
        builder = builder.position(x, y);
    }

    if let Err(e) = builder.build() {
        tracing::error!("Failed to create dictation indicator window: {}", e);
    }
}

fn compute_bottom_right_position(app: &AppHandle, width: f64, height: f64) -> Option<(f64, f64)> {
    let window = app.get_webview_window(MAIN_WINDOW_LABEL)?;
    let monitor = window.primary_monitor().ok().flatten()?;
    let scale = monitor.scale_factor();
    let logical_w = monitor.size().width as f64 / scale;
    let logical_h = monitor.size().height as f64 / scale;
    Some((logical_w - width - 24.0, logical_h - height - 80.0))
}

fn show_indicator(app: &AppHandle) {
    if let Some(window) = app.get_webview_window(INDICATOR_WINDOW_LABEL) {
        let _ = window.show();
    }
}

fn hide_indicator(app: &AppHandle) {
    if let Some(window) = app.get_webview_window(INDICATOR_WINDOW_LABEL) {
        let _ = window.hide();
    }
}
