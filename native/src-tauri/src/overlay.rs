use crate::settings::PillPosition;
use tauri::{AppHandle, LogicalPosition, LogicalSize, Manager, WebviewUrl, WebviewWindowBuilder};

pub const PILL_WINDOW_LABEL: &str = "dictation-pill";

/// The compact, non-expanded pill: just big enough for the logo/mic dot —
/// intentionally tiny so there's no invisible-but-clickable dead zone
/// around it (a larger fixed window with a small visible pill inside it
/// is exactly what swallows clicks meant for whatever's underneath, and
/// can clip/hide any dialog that pill ever opens).
const RESTING_SIZE: (f64, f64) = (56.0, 56.0);
/// The full listening/processing/success/error pill body.
const EXPANDED_SIZE: (f64, f64) = (420.0, 72.0);
/// Gap between the pill and the edge of the work area it's anchored to.
const EDGE_MARGIN: f64 = 16.0;

/// Creates the floating dictation pill window if it doesn't exist yet, at
/// its resting size, anchored per `position`. If it already exists, just
/// shows/hides it and re-anchors at its current size — this is the ONLY
/// PTT visual overlay; there is no separate "listening" window anymore.
///
/// Idempotent: safe to call on every startup and every time the "show
/// floating pill" setting changes.
pub fn ensure_pill_window(app: &AppHandle, visible: bool, position: PillPosition) {
    if let Some(window) = app.get_webview_window(PILL_WINDOW_LABEL) {
        let _ = if visible { window.show() } else { window.hide() };
        reposition(app, &window, RESTING_SIZE, position);
        return;
    }

    let mut builder = WebviewWindowBuilder::new(
        app,
        PILL_WINDOW_LABEL,
        WebviewUrl::App("index.html#/dictation-pill".into()),
    )
    .title("Relay — Dictation")
    .inner_size(RESTING_SIZE.0, RESTING_SIZE.1)
    .resizable(false)
    .decorations(false)
    .always_on_top(true)
    .skip_taskbar(true)
    .transparent(true)
    .shadow(false)
    .visible(visible)
    // Don't steal OS focus merely by appearing/expanding; a user click on
    // it is a deliberate action and is free to focus it at that point —
    // critical for keyboard PTT, where the target of text injection is
    // whatever window the user was already in, not this one.
    .focused(false);

    if let Some((x, y)) = compute_anchor(app, RESTING_SIZE, position) {
        builder = builder.position(x, y);
    }

    if let Err(e) = builder.build() {
        tracing::error!("Failed to create floating dictation pill window: {}", e);
    }
}

/// Re-anchors the pill at whatever size it's currently at (resting or
/// expanded) — used when the position *setting* changes, as opposed to
/// [`set_expanded`], which is used when the resting/expanded size changes.
pub fn reposition_pill(app: &AppHandle, position: PillPosition) {
    let Some(window) = app.get_webview_window(PILL_WINDOW_LABEL) else {
        return;
    };
    let current_size = window
        .inner_size()
        .ok()
        .map(|physical| {
            let scale = window.scale_factor().unwrap_or(1.0);
            (physical.width as f64 / scale, physical.height as f64 / scale)
        })
        .unwrap_or(RESTING_SIZE);
    reposition(app, &window, current_size, position);
}

/// Grows/shrinks the pill window to tightly match its actual on-screen
/// content (resting dot vs. the full expanded body) and re-anchors it.
/// Called by the frontend (via the `set_pill_expanded` command) whenever
/// its own RESTING/EXPANDED presentation state changes — the window
/// geometry itself can only be changed from the Rust side.
pub fn set_expanded(app: &AppHandle, expanded: bool, position: PillPosition) {
    let Some(window) = app.get_webview_window(PILL_WINDOW_LABEL) else {
        return;
    };
    let size = if expanded { EXPANDED_SIZE } else { RESTING_SIZE };
    reposition(app, &window, size, position);
}

/// Re-anchors the window (at whatever size it currently is, or `size` if
/// given a fresh one) using a freshly recomputed monitor + work area —
/// this is what keeps the pill correctly placed across monitor changes,
/// resolution changes, DPI changes, and taskbar/work-area changes: rather
/// than caching a position, it's recomputed from scratch every time the
/// pill is (re)shown or (re)sized, which is every time it actually matters
/// to the user.
fn reposition(
    app: &AppHandle,
    window: &tauri::WebviewWindow,
    size: (f64, f64),
    position: PillPosition,
) {
    if let Err(e) = window.set_size(LogicalSize::new(size.0, size.1)) {
        tracing::warn!("Failed to resize dictation pill window: {}", e);
    }
    if let Some((x, y)) = compute_anchor(app, size, position) {
        if let Err(e) = window.set_position(LogicalPosition::new(x, y)) {
            tracing::warn!("Failed to reposition dictation pill window: {}", e);
        }
    }
}

/// Anchors `size` to an edge of the *work area* (the OS-reported usable
/// desktop region, excluding the taskbar/dock — whether it's fixed,
/// auto-hidden, or on a different edge, the OS already accounts for that)
/// of whichever monitor is currently under the cursor. Tauri's public API
/// has no cross-platform way to ask "which monitor is the foreground
/// application's window on", so the cursor's monitor is the closest
/// available proxy — in practice the same monitor almost always, since the
/// user's cursor is wherever they were just typing/working.
fn compute_anchor(app: &AppHandle, size: (f64, f64), position: PillPosition) -> Option<(f64, f64)> {
    let monitor = active_monitor(app)?;
    let scale = monitor.scale_factor();
    let work_area = monitor.work_area();

    let wa_x = work_area.position.x as f64 / scale;
    let wa_y = work_area.position.y as f64 / scale;
    let wa_w = work_area.size.width as f64 / scale;
    let wa_h = work_area.size.height as f64 / scale;

    let (x, y) = match position {
        PillPosition::BottomCenter => (
            wa_x + (wa_w - size.0) / 2.0,
            wa_y + wa_h - size.1 - EDGE_MARGIN,
        ),
        PillPosition::TopCenter => (wa_x + (wa_w - size.0) / 2.0, wa_y + EDGE_MARGIN),
        PillPosition::LeftCenter => (wa_x + EDGE_MARGIN, wa_y + (wa_h - size.1) / 2.0),
        PillPosition::RightCenter => (
            wa_x + wa_w - size.0 - EDGE_MARGIN,
            wa_y + (wa_h - size.1) / 2.0,
        ),
    };
    Some((x, y))
}

fn active_monitor(app: &AppHandle) -> Option<tauri::Monitor> {
    if let Ok(cursor) = app.cursor_position() {
        if let Ok(Some(monitor)) = app.monitor_from_point(cursor.x, cursor.y) {
            return Some(monitor);
        }
    }
    // Cursor lookup can fail on some platforms/headless setups — fall back
    // to whichever monitor the main window considers primary.
    app.get_webview_window(crate::hotkeys::MAIN_WINDOW_LABEL)?
        .primary_monitor()
        .ok()
        .flatten()
}
