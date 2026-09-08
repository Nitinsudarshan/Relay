//! What Relay does before the user has asked it anything: whether it starts
//! with the OS, and whether it puts a window on screen when it does.
//!
//! Both were settings long before this module existed. `launch_at_login` and
//! `start_minimized` were parsed, persisted, defaulted, rendered as switches in
//! Settings › Provider, round-tripped by tests — and read by no Rust anywhere,
//! which is the one thing they could not be read by anything else. A toggle for
//! OS launch behaviour cannot work from a webview; there is no frontend where
//! this could have been quietly implemented instead.
//!
//! So this module is the reader they never had. It is deliberately the only
//! one: `apply_at_launch` runs once from the Tauri setup hook, and
//! [`reconcile_launch_at_login`] runs again from `save_settings` so flipping the
//! switch takes effect now rather than at the next launch — the same shape
//! `apply_capture_bridge` and `apply_hotkeys` already use.

use tauri::{AppHandle, Manager};
use tauri_plugin_autostart::ManagerExt;

use crate::settings::StartupSettings;

/// Applies both startup preferences, once, as the app comes up.
///
/// Called first in the setup hook and before anything fallible, because the
/// window decision is the one step whose failure the user would experience as
/// "Relay did not start".
pub fn apply_at_launch(app: &AppHandle, startup: &StartupSettings) {
    apply_start_minimized(app, startup.start_minimized);
    reconcile_launch_at_login(app, startup.launch_at_login);
}

/// Makes the OS launch entry match the setting.
///
/// When the setting is on the entry is written every time rather than only when
/// it is missing. `is_enabled` answers "is there an entry?", not "does it point
/// at this build", and Relay's install path changes under it — an update, a
/// move, a reinstall — so an entry checked and left alone is how autostart
/// stops working with the switch still showing on.
///
/// Never fatal. A registry write refused by policy, or a `LaunchAgents`
/// directory that cannot be written, is a preference Relay could not honour;
/// it is not a reason to fail to start.
pub fn reconcile_launch_at_login(app: &AppHandle, enabled: bool) {
    let manager = app.autolaunch();

    let outcome = if enabled {
        manager.enable()
    } else {
        // Only when there is something to remove. `disable` on an absent entry
        // is an error on some platforms, and a warning logged on every launch
        // of the default configuration is noise that trains people to ignore
        // the log.
        match manager.is_enabled() {
            Ok(false) => return,
            _ => manager.disable(),
        }
    };

    match outcome {
        Ok(()) => tracing::info!(
            "[Startup] Launch at login {}",
            if enabled { "enabled" } else { "disabled" }
        ),
        Err(e) => tracing::warn!(
            "[Startup] Could not {} launch at login: {e}",
            if enabled { "enable" } else { "disable" }
        ),
    }
}

/// Shows or withholds the main window as the app comes up.
///
/// The window is configured `"visible": false` in `tauri.conf.json` and shown
/// here instead, so "start minimized" does not mean "flash the control panel on
/// screen and then take it away". Withheld rather than minimized: the tray icon
/// and the show/hide hotkey are both already written against
/// `Window::is_visible`, so a hidden window is the state they can bring back.
fn apply_start_minimized(app: &AppHandle, minimized: bool) {
    let Some(window) = app.get_webview_window(crate::hotkeys::MAIN_WINDOW_LABEL) else {
        return;
    };

    if minimized {
        // Configured hidden, so this is belt-and-braces against a future
        // config change rather than the mechanism.
        let _ = window.hide();
        tracing::info!("[Startup] Starting minimized to the tray");
    } else {
        let _ = window.show();
        let _ = window.set_focus();
    }
}
