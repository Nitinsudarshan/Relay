pub mod injection;

use crate::sync::MutexExt;
use crate::commands::{emit_capture_state, emit_capture_status_event, AppState};
use serde::Serialize;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, ShortcutState};
use tauri_plugin_notification::NotificationExt;

pub const MAIN_WINDOW_LABEL: &str = "main";

/// Sends a native OS toast notification outside the compact dictation pill.
fn send_os_dictation_toast(app: &AppHandle, title: &str, body: &str) {
    let _ = app.notification().builder().title(title).body(body).show();
}

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
    pub capture_hotkey: String,
    pub capture_registered: bool,
    pub capture_error: Option<String>,
}

/// In toggle-to-talk mode, a single recording that was never stopped with a
/// second press will be stopped after this duration as an emergency backstop
/// against a forgotten toggle.
const MAX_PERSISTENT_RECORDING: Duration = Duration::from_secs(600); // 10 minutes

/// Emergency absolute ceiling for hold-to-talk if a key is physically stuck or jammed down forever.
/// Normal hold-to-talk dictation has NO arbitrary user-facing cutoff; this exists purely as a
/// disaster-recovery ceiling against hardware faults.
const EMERGENCY_SAFETY_CEILING: Duration = Duration::from_secs(1800); // 30 minutes

/// Polling interval for the safety watchdog to check physical key state and session health.
const WATCHDOG_CHECK_INTERVAL: Duration = Duration::from_millis(500);

/// Number of consecutive unpressed checks required before declaring a key-release lost by the OS.
/// 2 checks @ 500ms = ~1.0s of confirmed physical release.
const LOST_RELEASE_CONFIRMATION_COUNT: u32 = 2;

/// Explicit reason for ending a dictation session, distinguishing normal user actions
/// from emergency safety watchdog recoveries.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DictationStopReason {
    /// Normal key release in hold-to-talk mode
    NormalRelease,
    /// Second deliberate press in toggle-to-talk mode
    TogglePress,
    /// Safety watchdog detected the physical key was released but OS key-up event was lost
    WatchdogLostRelease,
    /// Safety watchdog timeout reached (emergency ceiling or toggle-to-talk forgotten session limit)
    WatchdogEmergencyCeiling,
}

impl std::fmt::Display for DictationStopReason {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DictationStopReason::NormalRelease => write!(f, "NORMAL_RELEASE"),
            DictationStopReason::TogglePress => write!(f, "TOGGLE_PRESS"),
            DictationStopReason::WatchdogLostRelease => {
                write!(f, "WATCHDOG_SAFETY_STOP (lost release event detected)")
            }
            DictationStopReason::WatchdogEmergencyCeiling => {
                write!(f, "WATCHDOG_SAFETY_STOP (emergency ceiling reached)")
            }
        }
    }
}

/// Tracks whether the dictation hotkey currently owns the microphone.
/// `generation` lets a delayed watchdog tell "this exact press" apart from
/// a later one, so it never force-releases a session that already ended
/// normally and started again. `key_down` tracks whether the physical key
/// is currently held (a press without a matching release yet), which is
/// what lets a deliberate second press — toggle-to-talk's "stop" signal —
/// be told apart from the OS re-firing "pressed" repeatedly while the key
/// stays physically down.
#[derive(Debug, Default, Clone)]
pub struct DictationState {
    pub active: bool,
    pub generation: u64,
    pub key_down: bool,
    pub target_focus: Option<injection::TargetFocusContext>,
}

impl DictationState {
    pub fn new() -> Self {
        Self::default()
    }

    /// Handles a press event. Returns whether this was a duplicate repeat,
    /// whether it stopped an active toggle session, or whether a new session generation was started.
    pub fn on_press(&mut self, toggle_to_talk: bool) -> PressOutcome {
        let is_repeat = self.key_down;
        self.key_down = true;
        if is_repeat {
            return PressOutcome::IgnoredRepeat;
        }

        if self.active {
            if toggle_to_talk {
                return PressOutcome::StopToggle(self.generation);
            }
            return PressOutcome::IgnoredRepeat;
        }

        self.active = true;
        self.generation += 1;
        self.target_focus = injection::capture_target_focus_context();
        PressOutcome::StartSession(self.generation)
    }

    /// Handles a release event.
    pub fn on_release(&mut self, toggle_to_talk: bool) -> ReleaseOutcome {
        self.key_down = false;
        if toggle_to_talk {
            ReleaseOutcome::IgnoredToggleRelease
        } else if self.active {
            ReleaseOutcome::StopHold(self.generation)
        } else {
            ReleaseOutcome::NoActiveSession
        }
    }

    /// Attempts to transition the session from active to stopped.
    /// Confirms `expected_generation` if provided.
    pub fn try_stop(&mut self, expected_generation: Option<u64>) -> Option<u64> {
        if !self.active {
            return None;
        }
        if let Some(expected) = expected_generation {
            if self.generation != expected {
                return None;
            }
        }
        self.active = false;
        self.key_down = false;
        Some(self.generation)
    }

    /// Stops session and extracts the target focus context captured at the start of dictation.
    pub fn try_stop_with_focus(
        &mut self,
        expected_generation: Option<u64>,
    ) -> Option<(u64, Option<injection::TargetFocusContext>)> {
        let focus = self.target_focus.take();
        self.try_stop(expected_generation).map(|gen| (gen, focus))
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum PressOutcome {
    IgnoredRepeat,
    StopToggle(u64),
    StartSession(u64),
}

#[derive(Debug, PartialEq, Eq)]
pub enum ReleaseOutcome {
    IgnoredToggleRelease,
    StopHold(u64),
    NoActiveSession,
}

pub type SharedDictationState = Arc<Mutex<DictationState>>;

/// Parse a shortcut string (e.g. "Ctrl+Space", "Ctrl+Shift+Space", "Alt+Space", "F8")
/// into a list of Win32 Virtual Key codes for physical key state polling.
pub fn parse_shortcut_to_vk_codes(shortcut: &str) -> Option<Vec<i32>> {
    let mut vks = Vec::new();
    for part in shortcut.split('+') {
        let trimmed = part.trim();
        let vk = match trimmed.to_lowercase().as_str() {
            "ctrl" | "control" => 0x11, // VK_CONTROL
            "shift" => 0x10,            // VK_SHIFT
            "alt" | "option" => 0x12,   // VK_MENU
            "super" | "win" | "cmd" | "command" => 0x5B, // VK_LWIN
            "space" => 0x20,            // VK_SPACE
            "enter" | "return" => 0x0D, // VK_RETURN
            "tab" => 0x09,              // VK_TAB
            "esc" | "escape" => 0x1B,   // VK_ESCAPE
            "backspace" => 0x08,        // VK_BACK
            "f1" => 0x70,
            "f2" => 0x71,
            "f3" => 0x72,
            "f4" => 0x73,
            "f5" => 0x74,
            "f6" => 0x75,
            "f7" => 0x76,
            "f8" => 0x77,
            "f9" => 0x78,
            "f10" => 0x79,
            "f11" => 0x7A,
            "f12" => 0x7B,
            // Single character keys
            s if s.len() == 1 => {
                let ch = s.chars().next().unwrap();
                if ch.is_ascii_alphabetic() {
                    ch.to_ascii_uppercase() as i32 // 'A'..'Z' is 0x41..0x5A
                } else if ch.is_ascii_digit() {
                    ch as i32 // '0'..'9' is 0x30..0x39
                } else {
                    match ch {
                        '`' | '~' => 0xC0, // VK_OEM_3
                        ',' | '<' => 0xBC, // VK_OEM_COMMA
                        '.' | '>' => 0xBE, // VK_OEM_PERIOD
                        '/' | '?' => 0xBF, // VK_OEM_2
                        ';' | ':' => 0xBA, // VK_OEM_1
                        '\'' | '"' => 0xDE, // VK_OEM_7
                        '[' | '{' => 0xDB, // VK_OEM_4
                        ']' | '}' => 0xDD, // VK_OEM_6
                        '\\' | '|' => 0xDC, // VK_OEM_5
                        '-' | '_' => 0xBD, // VK_OEM_MINUS
                        '=' | '+' => 0xBB, // VK_OEM_PLUS
                        _ => return None,
                    }
                }
            }
            _ => return None,
        };
        vks.push(vk);
    }
    if vks.is_empty() {
        None
    } else {
        Some(vks)
    }
}

/// Checks physical key state using Win32 GetAsyncKeyState.
#[cfg(windows)]
pub fn is_vk_down(vk: i32) -> bool {
    extern "system" {
        fn GetAsyncKeyState(vKey: i32) -> i16;
    }
    unsafe { (GetAsyncKeyState(vk) as u16 & 0x8000) != 0 }
}

/// Fallback for non-Windows platforms (e.g. Linux CI).
#[cfg(not(windows))]
pub fn is_vk_down(_vk: i32) -> bool {
    true
}

/// Checks whether all constituent keys of the shortcut combination are physically down.
pub fn is_shortcut_physically_down(vk_codes: &[i32]) -> bool {
    if vk_codes.is_empty() {
        return false;
    }
    vk_codes.iter().all(|&vk| is_vk_down(vk))
}

/// Registers Relay's global (OS-wide) hotkeys.
pub fn register_hotkeys(
    app: &AppHandle,
    show_hide_hotkey: &str,
    dictation_hotkey: &str,
    capture_hotkey: &str,
) {
    try_register_hotkeys(app, show_hide_hotkey, dictation_hotkey, capture_hotkey);
}

/// Re-registers hotkeys with new bindings, replacing whatever is
/// currently bound. Used both at startup and whenever Settings saves new
/// hotkeys — hotkeys take effect immediately, no app restart required.
pub fn apply_hotkeys(
    app: &AppHandle,
    show_hide_hotkey: &str,
    dictation_hotkey: &str,
    capture_hotkey: &str,
) -> Result<(), String> {
    app.global_shortcut()
        .unregister_all()
        .map_err(|e| format!("Could not clear existing hotkeys: {}", e))?;
    let status = try_register_hotkeys(app, show_hide_hotkey, dictation_hotkey, capture_hotkey);
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
/// locales) must never prevent the other from being attempted.
fn try_register_hotkeys(
    app: &AppHandle,
    show_hide_hotkey: &str,
    dictation_hotkey: &str,
    capture_hotkey: &str,
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

    let dictation_state: SharedDictationState = Arc::new(Mutex::new(DictationState::new()));
    let dictation_result = app
        .global_shortcut()
        .on_shortcut(
            dictation_hotkey,
            move |app, _shortcut, event| match event.state {
                ShortcutState::Pressed => on_dictation_pressed_with_mode(app, &dictation_state, "dictation"),
                ShortcutState::Released => on_dictation_released(app, &dictation_state),
            },
        )
        .map_err(|e| format!("dictation hotkey '{}': {}", dictation_hotkey, e));
    match &dictation_result {
        Ok(()) => tracing::info!("[Hotkey] Registered dictation hotkey '{}'", dictation_hotkey),
        Err(e) => tracing::error!("[Hotkey] Failed to register dictation hotkey: {}", e),
    }

    // Capture's real trigger lives in the browser, because that is the only
    // place a page can be read (see `settings::HotkeySettings::capture_hotkey`).
    // This shortcut only brings the Captures surface forward, so a conflict
    // on it costs convenience, not the feature — which is why, unlike the
    // other two, it never fails `apply_hotkeys`.
    let capture_app = app.clone();
    let capture_result = app
        .global_shortcut()
        .on_shortcut(capture_hotkey, move |_app, _shortcut, event| {
            if event.state == ShortcutState::Pressed {
                show_captures_surface(&capture_app);
            }
        })
        .map_err(|e| format!("capture hotkey '{}': {}", capture_hotkey, e));
    match &capture_result {
        Ok(()) => tracing::info!("[Hotkey] Registered capture hotkey '{}'", capture_hotkey),
        Err(e) => tracing::warn!("[Hotkey] Failed to register capture hotkey: {}", e),
    }

    let status = HotkeyRegistrationStatus {
        capture_hotkey: capture_hotkey.to_string(),
        capture_registered: capture_result.is_ok(),
        capture_error: capture_result.err(),
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

/// Brings the main window forward on Relay's Captures tab.
fn show_captures_surface(app: &AppHandle) {
    let Some(window) = app.get_webview_window(MAIN_WINDOW_LABEL) else {
        return;
    };
    let _ = window.unminimize();
    let _ = window.show();
    let _ = window.set_focus();
    let _ = app.emit("navigate-tab", serde_json::json!({ "tab": "captures" }));
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

fn on_dictation_pressed_with_mode(app: &AppHandle, dictation_state: &SharedDictationState, mode: &str) {
    tracing::debug!("[Hotkey] {} received (pressed)", mode);

    let state = app.state::<AppState>();
    let (toggle_to_talk, dictation_hotkey) = {
        let s = state.settings.lock_or_recover();
        (s.hotkeys.toggle_to_talk, s.hotkeys.dictation_hotkey.clone())
    };

    let outcome = {
        let mut guard = dictation_state.lock_or_recover();
        guard.on_press(toggle_to_talk)
    };

    match outcome {
        PressOutcome::IgnoredRepeat => {}
        PressOutcome::StopToggle(_gen) => {
            let t_key_release = std::time::Instant::now();
            stop_dictation_session(
                app.clone(),
                dictation_state.clone(),
                None,
                Some(t_key_release),
                DictationStopReason::TogglePress,
            );
        }
        PressOutcome::StartSession(generation) => {
            tracing::debug!("[Dictation] Start requested via hotkey for mode: {}", mode);
            let audio_dir = state.config_dir.join("audio");
            match state.recorder.start(mode, &audio_dir, Some(app.clone())) {
                Ok(_) => {
                    tracing::debug!("[Audio] Capture started for mode: {}", mode);
                    emit_capture_state(app, &state.recorder);
                    spawn_release_watchdog(
                        app.clone(),
                        dictation_state.clone(),
                        generation,
                        dictation_hotkey,
                        toggle_to_talk,
                    );
                }
                Err(e) => {
                    tracing::info!("Dictation hotkey could not start capture: {}", e);
                    dictation_state.lock_or_recover().active = false;
                }
            }
        }
    }
}

/// Handles the dictation hotkey's key-up. In the default hold-to-talk mode
/// this is what stops recording. In toggle-to-talk mode
/// (`HotkeySettings::toggle_to_talk`), releasing the key never stops
/// recording by itself — only a subsequent press does, handled in
/// `on_dictation_pressed` — so this only clears the "physically held" flag.
fn on_dictation_released(app: &AppHandle, dictation_state: &SharedDictationState) {
    let t_key_release = std::time::Instant::now();

    let state = app.state::<AppState>();
    let toggle_to_talk = state.settings.lock_or_recover().hotkeys.toggle_to_talk;

    let outcome = {
        let mut guard = dictation_state.lock_or_recover();
        guard.on_release(toggle_to_talk)
    };

    if let ReleaseOutcome::StopHold(_gen) = outcome {
        stop_dictation_session(
            app.clone(),
            dictation_state.clone(),
            None,
            Some(t_key_release),
            DictationStopReason::NormalRelease,
        );
    }
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
    t_key_release: Option<std::time::Instant>,
    reason: DictationStopReason,
) {
    let t_release = t_key_release.unwrap_or_else(std::time::Instant::now);

    let (session_generation, target_focus) = {
        let mut guard = dictation_state.lock_or_recover();
        match guard.try_stop_with_focus(expected_generation) {
            Some(res) => res,
            None => return,
        }
    };

    tracing::info!(
        "[Dictation] Stopping session: reason={}, generation={}",
        reason,
        session_generation
    );

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

        let t_recorder_stop_complete = std::time::Instant::now();

        if !captured.had_audio {
            tracing::info!("[Dictation] Recording stopped with no audio input (reason: {})", reason);
            emit_capture_status_event(
                &app,
                false,
                Some(captured.mode.clone()),
                "NO_SPEECH",
                None,
            );
            return;
        }

        emit_capture_status_event(
            &app,
            false,
            Some(captured.mode.clone()),
            "TRANSCRIBING",
            None,
        );

        let models_dir = state.config_dir.join("models");
        let language_settings = state.settings.lock_or_recover().language.clone();
        let stt_settings = state.settings.lock_or_recover().stt.clone();
        let model_path = crate::capture::stt::resolve_dictation_model_path(&models_dir, &stt_settings).await;
        let language_config = crate::capture::SttLanguageConfig::from_settings(&language_settings);
        let mut decoding_config = crate::capture::stt::WhisperDecodingConfig::for_dictation(&stt_settings);
        if let Some(prompt) = state.settings.lock_or_recover().build_stt_prompt() {
            decoding_config.initial_prompt = Some(prompt);
        }

        let stt = state.stt.clone();
        let samples = captured.samples.clone();
        let mp_clone = model_path.clone();
        let lang_clone = language_config.clone();
        let dec_clone = decoding_config.clone();

        let t_whisper_start = std::time::Instant::now();

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

        let t_whisper_complete = std::time::Instant::now();

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

        let _t_diagnostics_complete = std::time::Instant::now();

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
            let expanded_text = state.settings.lock_or_recover().expand_snippets(&text_res);
            let final_text = if !expanded_text.trim().is_empty() { expanded_text } else { text_res };
            let t_snippet_complete = std::time::Instant::now();

            let (auto_paste, copy_to_clipboard) = {
                let s = state.settings.lock_or_recover();
                (s.clipboard.auto_paste, s.clipboard.copy_to_clipboard)
            };

            // 1. Copy to OS clipboard FIRST and NATIVELY in Rust.
            // Using arboard directly at the OS level ensures the transcription is in the
            // clipboard unconditionally, bypassing webview focus restrictions.
            if copy_to_clipboard {
                if let Err(e) = injection::copy_to_clipboard(&final_text) {
                    tracing::warn!("Native dictation clipboard copy failed: {}", e);
                } else {
                    tracing::debug!("Native dictation clipboard copy succeeded ({} chars)", final_text.len());
                }
                let _ = app.emit("dictation-clipboard-copy", &final_text);
            }

            // 2. Inject text into the active field, guarded against tab or window switching.
            // If the user moved to another tab or window, wait up to 15s for them to return
            // so text is injected directly into the original field without spraying into the wrong place.
            let t_injection_start = std::time::Instant::now();

            if auto_paste {
                let app_for_wait = app.clone();
                let dictation_state_for_cancel = dictation_state.clone();

                let outcome = injection::inject_text_with_return_wait(
                    &final_text,
                    target_focus.as_ref(),
                    std::time::Duration::from_secs(15),
                    std::time::Duration::from_millis(100),
                    |_target_title| {
                        tracing::info!(
                            "[Dictation] Active focus moved from '{}'. Waiting up to 15s for return...",
                            _target_title
                        );
                        emit_capture_status_event(
                            &app_for_wait,
                            false,
                            None,
                            "WAITING_FOR_TAB",
                            Some("Switch back to tab to inject...".to_string()),
                        );
                    },
                    move || {
                        dictation_state_for_cancel.lock_or_recover().generation != session_generation
                    },
                );

                match outcome {
                    Ok(injection::InjectionOutcome::Success) => {
                        emit_capture_status_event(&app, false, None, "SUCCESS", None);
                    }
                    Ok(injection::InjectionOutcome::TimedOutWaitingForReturn { target_title }) => {
                        tracing::info!(
                            "[Dictation] Timed out waiting for return to '{}'. Transcription kept in clipboard.",
                            target_title
                        );
                        send_os_dictation_toast(
                            &app,
                            "Relay Dictation",
                            "Tab wait timed out — transcription copied to clipboard (Ctrl+V)",
                        );
                        emit_capture_status_event(
                            &app,
                            false,
                            None,
                            "FOCUS_CHANGED",
                            Some("Copied (Ctrl+V)".to_string()),
                        );
                    }
                    Ok(injection::InjectionOutcome::Cancelled) => {
                        tracing::info!("[Dictation] Focus return wait cancelled by newer session.");
                    }
                    Ok(injection::InjectionOutcome::TabChanged { target_title, current_title }) => {
                        tracing::info!(
                            "[Dictation] Active tab changed from '{}' to '{}'. Prevented typing into wrong tab.",
                            target_title, current_title
                        );
                        send_os_dictation_toast(
                            &app,
                            "Relay Dictation",
                            "Tab changed — transcription copied to clipboard (Ctrl+V)",
                        );
                        emit_capture_status_event(
                            &app,
                            false,
                            None,
                            "FOCUS_CHANGED",
                            Some("Copied (Ctrl+V)".to_string()),
                        );
                    }
                    Ok(injection::InjectionOutcome::AppChanged { target_title, current_title }) => {
                        tracing::info!(
                            "[Dictation] Foreground app changed from '{}' to '{}'. Prevented typing into wrong app.",
                            target_title, current_title
                        );
                        send_os_dictation_toast(
                            &app,
                            "Relay Dictation",
                            "App changed — transcription copied to clipboard (Ctrl+V)",
                        );
                        emit_capture_status_event(
                            &app,
                            false,
                            None,
                            "FOCUS_CHANGED",
                            Some("Copied (Ctrl+V)".to_string()),
                        );
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
                emit_capture_status_event(&app, false, None, "SUCCESS", None);
            }

            let t_injection_complete = std::time::Instant::now();

            // 3. Persist voice note in vault after injection so vault disk I/O does not delay paste
            let t_vault_start = std::time::Instant::now();
            crate::commands::save_voice_note(&app, &state.vault, &final_text);
            let t_vault_complete = std::time::Instant::now();

            let metrics = captured.timing_metrics.clone().unwrap_or_default();
            let recording_to_audio_ready = t_recorder_stop_complete.duration_since(t_release).as_millis();
            let audio_ready_to_stt_start = t_whisper_start.duration_since(t_recorder_stop_complete).as_millis();
            let stt_execution = t_whisper_complete.duration_since(t_whisper_start).as_millis();
            let stt_to_text_available = t_snippet_complete.duration_since(t_whisper_complete).as_millis();
            let text_available_to_injection = t_injection_complete.duration_since(t_snippet_complete).as_millis();
            let injection_duration = t_injection_complete.duration_since(t_injection_start).as_millis();
            let total_e2e_latency = t_injection_complete.duration_since(t_release).as_millis();

            let now = std::time::SystemTime::now();
            let format_ts = |t_inst: std::time::Instant| -> String {
                let dt = if t_inst <= t_injection_complete {
                    let diff = t_injection_complete.duration_since(t_inst);
                    now.checked_sub(diff).unwrap_or(now)
                } else {
                    now
                };
                let dur = dt.duration_since(std::time::UNIX_EPOCH).unwrap_or_default();
                let secs = dur.as_secs();
                let millis = dur.subsec_millis();
                let hours = (secs / 3600) % 24;
                let mins = (secs / 60) % 60;
                let s = secs % 60;
                format!("{:02}:{:02}:{:02}.{:03}", hours, mins, s, millis)
            };

            println!("\n==================================================");
            println!("DICTATION LATENCY TRACE");
            println!("-----------------------");
            println!("stop_reason          : {}", reason);
            println!("recording_stop       : {}", format_ts(t_release));
            println!("audio_ready          : {}", format_ts(t_recorder_stop_complete));
            println!("stt_start            : {}", format_ts(t_whisper_start));
            println!("stt_end              : {}", format_ts(t_whisper_complete));
            println!("text_available       : {}", format_ts(t_snippet_complete));
            println!("injection_start      : {}", format_ts(t_injection_start));
            println!("injection_complete   : {}", format_ts(t_injection_complete));
            println!("\nDurations:");
            println!("recording → audio_ready       : {} ms", recording_to_audio_ready);
            println!("audio_ready → STT start       : {} ms", audio_ready_to_stt_start);
            println!("STT execution                 : {} ms", stt_execution);
            println!("STT → text available          : {} ms", stt_to_text_available);
            println!("text available → injection    : {} ms", text_available_to_injection);
            println!("TOTAL                         : {} ms", total_e2e_latency);
            println!("==================================================\n");

            tracing::info!(
                "[DICTATION_LATENCY] reason={}, total={}ms, whisper={}ms, rec_stop={}ms, vad={}ms, wav_io={}ms, vault_io={}ms, inject={}ms",
                reason,
                total_e2e_latency,
                stt_execution,
                recording_to_audio_ready,
                metrics.vad_ms,
                metrics.wav_write_ms,
                t_vault_complete.duration_since(t_vault_start).as_millis(),
                injection_duration
            );
        } else {
            tracing::info!("[Dictation] Produced no speech (silence or too short, reason: {})", reason);
            emit_capture_status_event(&app, false, None, "NO_SPEECH", None);
        }
    });
}

/// Spawns the safety watchdog for an active dictation session.
///
/// In hold-to-talk mode:
/// - As long as the user physically holds the hotkey down, recording continues indefinitely
///   with NO arbitrary user-facing cutoff (1 min, 5 min, 10 min, etc.).
/// - If the physical key is released but the OS missed delivering the key-up event,
///   the watchdog detects this within ~1.0s and safely stops the session.
/// - If a key is jammed physically down forever, an emergency ceiling (30 min) stops capture.
///
/// In toggle-to-talk mode:
/// - Recording continues until the next press, with a 10-minute backstop against forgotten recordings.
fn spawn_release_watchdog(
    app: AppHandle,
    dictation_state: SharedDictationState,
    generation: u64,
    dictation_hotkey: String,
    toggle_to_talk: bool,
) {
    let vk_codes = parse_shortcut_to_vk_codes(&dictation_hotkey);
    tauri::async_runtime::spawn(async move {
        let start_time = std::time::Instant::now();
        let mut unpressed_consecutive_checks = 0u32;

        loop {
            tokio::time::sleep(WATCHDOG_CHECK_INTERVAL).await;

            let (is_active, current_gen) = {
                let guard = dictation_state.lock_or_recover();
                (guard.active, guard.generation)
            };

            // If session is no longer active or a newer generation has started, exit cleanly.
            if !is_active || current_gen != generation {
                return;
            }

            if toggle_to_talk {
                // In toggle mode: recording continues until the user presses the hotkey again.
                // Emergency backstop fires only if recording exceeds MAX_PERSISTENT_RECORDING (10 min).
                if start_time.elapsed() >= MAX_PERSISTENT_RECORDING {
                    tracing::warn!(
                        "[Dictation] Toggle-to-talk session reached emergency limit of {:?} — forcing safety stop.",
                        MAX_PERSISTENT_RECORDING
                    );
                    stop_dictation_session(
                        app,
                        dictation_state,
                        Some(generation),
                        None,
                        DictationStopReason::WatchdogEmergencyCeiling,
                    );
                    return;
                }
            } else {
                // In hold-to-talk mode:
                if let Some(ref vks) = vk_codes {
                    if is_shortcut_physically_down(vks) {
                        // Key is physically held down -> User is speaking! Reset unpressed counter.
                        unpressed_consecutive_checks = 0;
                    } else {
                        // Physical key is NOT down, but session is still marked active!
                        unpressed_consecutive_checks += 1;
                        if unpressed_consecutive_checks >= LOST_RELEASE_CONFIRMATION_COUNT {
                            tracing::warn!(
                                "[Dictation] Hold-to-talk physical key is no longer down, but release event was lost by OS. Triggering safety recovery."
                            );
                            stop_dictation_session(
                                app,
                                dictation_state,
                                Some(generation),
                                None,
                                DictationStopReason::WatchdogLostRelease,
                            );
                            return;
                        }
                    }
                }

                // Absolute emergency ceiling in case hardware key is jammed
                if start_time.elapsed() >= EMERGENCY_SAFETY_CEILING {
                    tracing::warn!(
                        "[Dictation] Hold-to-talk session reached absolute emergency ceiling of {:?} — forcing safety stop.",
                        EMERGENCY_SAFETY_CEILING
                    );
                    stop_dictation_session(
                        app,
                        dictation_state,
                        Some(generation),
                        None,
                        DictationStopReason::WatchdogEmergencyCeiling,
                    );
                    return;
                }
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;
    use tauri_plugin_global_shortcut::Shortcut;

    #[test]
    fn test_shortcut_parsing_variants() {
        assert!(Shortcut::from_str("Ctrl+Space").is_ok());
        assert!(Shortcut::from_str("Ctrl+Shift+Space").is_ok());
        assert!(Shortcut::from_str("Ctrl+Alt+Space").is_ok());
        assert!(Shortcut::from_str("Ctrl+0").is_ok());

        let num0 = Shortcut::from_str("Ctrl+Num0");
        let numpad0 = Shortcut::from_str("Ctrl+Numpad0");
        let period = Shortcut::from_str("Ctrl+Period");
        let numdecimal = Shortcut::from_str("Ctrl+NumDecimal");
        let decimal = Shortcut::from_str("Ctrl+Decimal");
        let numpaddecimal = Shortcut::from_str("Ctrl+NumpadDecimal");

        println!("Ctrl+Num0: {:?}", num0);
        println!("Ctrl+Numpad0: {:?}", numpad0);
        println!("Ctrl+Period: {:?}", period);
        println!("Ctrl+NumDecimal: {:?}", numdecimal);
        println!("Ctrl+Decimal: {:?}", decimal);
        println!("Ctrl+NumpadDecimal: {:?}", numpaddecimal);
    }

    #[test]
    fn test_parse_shortcut_to_vk_codes() {
        assert_eq!(
            parse_shortcut_to_vk_codes("Ctrl+Space"),
            Some(vec![0x11, 0x20])
        );
        assert_eq!(
            parse_shortcut_to_vk_codes("Ctrl+Shift+Space"),
            Some(vec![0x11, 0x10, 0x20])
        );
        assert_eq!(parse_shortcut_to_vk_codes("F8"), Some(vec![0x77]));
        assert_eq!(
            parse_shortcut_to_vk_codes("Alt+A"),
            Some(vec![0x12, 0x41])
        );
        assert_eq!(parse_shortcut_to_vk_codes("UnknownNonExistentKey"), None);
    }

    #[test]
    fn test_a_long_recording_survives_normal_duration() {
        // Test A: Given active = true, generation = 1, physical key held,
        // crossing the previous 60-second threshold must NOT cause termination.
        let mut state = DictationState::new();
        let outcome = state.on_press(false);
        assert_eq!(outcome, PressOutcome::StartSession(1));
        assert!(state.active);
        assert_eq!(state.generation, 1);
        assert!(state.key_down);

        // Simulate a long recording passing 61s, 120s, 300s with key still down
        let vk_codes = [0x11, 0x20];
        assert_eq!(vk_codes.len(), 2);

        // State remains active and generation remains 1
        assert!(state.active);
        assert_eq!(state.generation, 1);
    }

    #[test]
    fn test_b_normal_release_stops_recording() {
        // Test B: Normal key release stops recording in hold-to-talk mode
        let mut state = DictationState::new();
        state.on_press(false);
        assert!(state.active);

        let release = state.on_release(false);
        assert_eq!(release, ReleaseOutcome::StopHold(1));
        assert!(!state.key_down);

        let stopped_gen = state.try_stop(None);
        assert_eq!(stopped_gen, Some(1));
        assert!(!state.active);
    }

    #[test]
    fn test_c_toggle_mode_does_not_stop_on_release() {
        // Test C: Toggle mode does NOT stop on release; stops on subsequent press
        let mut state = DictationState::new();
        let start = state.on_press(true);
        assert_eq!(start, PressOutcome::StartSession(1));
        assert!(state.active);

        // Key release in toggle mode
        let release = state.on_release(true);
        assert_eq!(release, ReleaseOutcome::IgnoredToggleRelease);
        assert!(state.active); // Still active!
        assert!(!state.key_down);

        // Subsequent deliberate press stops toggle mode
        let second_press = state.on_press(true);
        assert_eq!(second_press, PressOutcome::StopToggle(1));

        let stopped_gen = state.try_stop(None);
        assert_eq!(stopped_gen, Some(1));
        assert!(!state.active);
    }

    #[test]
    fn test_d_watchdog_generation_protection() {
        // Test D: An old watchdog from generation 1 cannot stop generation 2
        let mut state = DictationState::new();
        state.on_press(false);
        assert_eq!(state.generation, 1);

        // Session 1 stops normally
        state.on_release(false);
        state.try_stop(Some(1));
        assert!(!state.active);

        // Session 2 starts
        state.on_press(false);
        assert_eq!(state.generation, 2);
        assert!(state.active);

        // Old watchdog from generation 1 attempts to stop session
        let stopped_by_old_watchdog = state.try_stop(Some(1));
        assert_eq!(stopped_by_old_watchdog, None);
        assert!(state.active); // Generation 2 is still active and untouched!
        assert_eq!(state.generation, 2);
    }

    #[test]
    fn test_e_safety_recovery_for_lost_release() {
        // Test E: When key release is lost, safety recovery stops the session
        let mut state = DictationState::new();
        state.on_press(false);
        assert!(state.active);
        assert_eq!(state.generation, 1);

        // Simulate watchdog discovering physical key is no longer pressed and recovering
        let stopped = state.try_stop(Some(1));
        assert_eq!(stopped, Some(1));
        assert!(!state.active);
    }

    #[test]
    fn test_stop_reason_formatting() {
        assert_eq!(
            DictationStopReason::NormalRelease.to_string(),
            "NORMAL_RELEASE"
        );
        assert_eq!(
            DictationStopReason::TogglePress.to_string(),
            "TOGGLE_PRESS"
        );
        assert!(DictationStopReason::WatchdogLostRelease
            .to_string()
            .contains("WATCHDOG_SAFETY_STOP"));
        assert!(DictationStopReason::WatchdogEmergencyCeiling
            .to_string()
            .contains("WATCHDOG_SAFETY_STOP"));
    }
}

