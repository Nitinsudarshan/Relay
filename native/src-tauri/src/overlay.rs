use crate::hotkeys::MAIN_WINDOW_LABEL;
use tauri::{AppHandle, Manager, WebviewUrl, WebviewWindowBuilder};

pub const PILL_WINDOW_LABEL: &str = "dictation-pill";
const PILL_WIDTH: f64 = 420.0;
const PILL_HEIGHT: f64 = 72.0;

/// The floating "Click to dictate" pill: a small always-on-top window that
/// lives outside the main app window entirely (like the dictation-listening
/// indicator), so it's reachable from anywhere on the desktop — including
/// while the main Relay window is hidden — rather than boxed inside the
/// dashboard's Capture tab.
///
/// Idempotent: if the window already exists this just shows/hides it,
/// matching `visible`, instead of trying to build it again.
pub fn ensure_pill_window(app: &AppHandle, visible: bool) {
    if let Some(window) = app.get_webview_window(PILL_WINDOW_LABEL) {
        let _ = if visible { window.show() } else { window.hide() };
        return;
    }

    let position = compute_bottom_center_position(app, PILL_WIDTH, PILL_HEIGHT);

    let mut builder = WebviewWindowBuilder::new(
        app,
        PILL_WINDOW_LABEL,
        WebviewUrl::App("index.html#/dictation-pill".into()),
    )
    .title("Relay — Dictation")
    .inner_size(PILL_WIDTH, PILL_HEIGHT)
    .resizable(false)
    .decorations(false)
    .always_on_top(true)
    .skip_taskbar(true)
    .transparent(true)
    .shadow(false)
    .visible(visible)
    // Don't steal OS focus merely by appearing; a user click on it is a
    // deliberate action and is free to focus it at that point.
    .focused(false);

    if let Some((x, y)) = position {
        builder = builder.position(x, y);
    }

    if let Err(e) = builder.build() {
        tracing::error!("Failed to create floating dictation pill window: {}", e);
    }
}

fn compute_bottom_center_position(app: &AppHandle, width: f64, height: f64) -> Option<(f64, f64)> {
    let window = app.get_webview_window(MAIN_WINDOW_LABEL)?;
    let monitor = window.primary_monitor().ok().flatten()?;
    let scale = monitor.scale_factor();
    let logical_w = monitor.size().width as f64 / scale;
    let logical_h = monitor.size().height as f64 / scale;
    Some(((logical_w - width) / 2.0, logical_h - height - 48.0))
}
