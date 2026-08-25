use crate::meetings::reminders::{ReminderEvent, ReminderKind};
use crate::overlay;
use crate::settings::MeetingSettings;
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicBool, AtomicI64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tauri::{async_runtime::JoinHandle, AppHandle, Emitter, Manager};
use tauri_plugin_notification::NotificationExt;

pub const MEETING_REMINDER_EVENT: &str = "meeting-reminder";

/// Maximum permitted characters for titles rendered in overlay window.
const MAX_TITLE_LEN: usize = 80;
/// Maximum permitted characters for subtitle / provider strings.
const MAX_BODY_LEN: usize = 120;

/// Minimum resume time in milliseconds when pointer leaves the overlay,
/// preventing dismiss expiration mid-hover/click.
pub const MIN_RESUME_MS: i64 = 5000;

/// Auto-dismiss duration for all meeting reminders (15 seconds with hover pause).
pub const AUTO_DISMISS_MS: i64 = 15_000;

/// Sanitizes untrusted meeting titles: strips control characters, bidi-overrides,
/// and clamps length to prevent UI overflow / soft denial of service.
pub fn sanitize_and_clamp_text(input: &str, max_len: usize) -> String {
    let mut cleaned = String::with_capacity(input.len().min(max_len));
    for c in input.chars() {
        // Strip control characters (except basic space)
        if c.is_control() && c != ' ' {
            continue;
        }
        // Strip Unicode Bidirectional Override characters (U+202A to U+202E, U+2066 to U+2069)
        if matches!(c, '\u{202A}'..='\u{202E}' | '\u{2066}'..='\u{2069}') {
            continue;
        }
        cleaned.push(c);
        if cleaned.chars().count() >= max_len {
            break;
        }
    }
    cleaned.trim().to_string()
}

/// The sanitized data model sent over IPC to the React overlay window.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MeetingReminderPayload {
    pub meeting_id: String,
    pub kind: ReminderKind,
    pub title: String,
    pub provider: String,
    pub provider_name: String,
    pub time_label: String,
    pub participants: Vec<String>,
}

impl MeetingReminderPayload {
    pub fn from_reminder_event(event: &ReminderEvent) -> Self {
        let title = sanitize_and_clamp_text(&event.title, MAX_TITLE_LEN);
        let safe_title = if title.is_empty() {
            "Upcoming Meeting".to_string()
        } else {
            title
        };

        let provider_name = match event.provider.to_lowercase().as_str() {
            "google_meet" | "google meet" => "Google Meet",
            "zoom" => "Zoom",
            "teams" => "Teams",
            "webex" => "Webex",
            _ => "In Person",
        };

        let raw_time_label = match event.kind {
            ReminderKind::Upcoming => "Starts in 5 minutes",
            ReminderKind::Unrecorded => "Meeting in progress",
            ReminderKind::Detected => "Meeting detected",
        };
        let time_label = sanitize_and_clamp_text(raw_time_label, MAX_BODY_LEN);

        Self {
            meeting_id: event.meeting_id.clone(),
            kind: event.kind,
            title: safe_title,
            provider: event.provider.clone(),
            provider_name: sanitize_and_clamp_text(provider_name, MAX_BODY_LEN),
            time_label,
            participants: event
                .participants
                .iter()
                .map(|p| sanitize_and_clamp_text(p, 40))
                .filter(|p| !p.is_empty())
                .collect(),
        }
    }
}

/// Notification service managing the lifecycle, deduplication, suppression,
/// readiness handshake, and auto-dismiss countdown for meeting reminders.
pub struct NotificationService {
    active_reminder: Mutex<Option<ReminderEvent>>,
    pending_payload: Mutex<Option<MeetingReminderPayload>>,
    ready_handshake_done: AtomicBool,
    is_hovered: AtomicBool,
    timer_remaining_ms: AtomicI64,
    timer_task: Mutex<Option<JoinHandle<()>>>,
}

impl Default for NotificationService {
    fn default() -> Self {
        Self {
            active_reminder: Mutex::new(None),
            pending_payload: Mutex::new(None),
            ready_handshake_done: AtomicBool::new(false),
            is_hovered: AtomicBool::new(false),
            timer_remaining_ms: AtomicI64::new(0),
            timer_task: Mutex::new(None),
        }
    }
}

impl NotificationService {
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns the currently pending or active reminder payload for the frontend.
    pub fn get_pending_reminder(&self) -> Option<MeetingReminderPayload> {
        self.pending_payload.lock().unwrap().clone()
    }

    /// Signals from the frontend overlay view that it is mounted and ready.
    /// Reveals the window if a reminder was queued.
    pub fn on_frontend_ready(&self, app: &AppHandle) {
        self.ready_handshake_done.store(true, Ordering::SeqCst);
        let has_pending = self.pending_payload.lock().unwrap().is_some();
        if has_pending {
            tracing::info!("[notifications] Frontend signaled ready; revealing overlay window.");
            overlay::show_reminder_window(app);
        }
    }

    /// Updates hover state from the frontend card.
    pub fn on_hover_changed(&self, hovered: bool) {
        let prev = self.is_hovered.swap(hovered, Ordering::SeqCst);
        if prev && !hovered {
            // Pointer left the card — enforce minimum resume floor
            let current = self.timer_remaining_ms.load(Ordering::SeqCst);
            if current > 0 && current < MIN_RESUME_MS {
                self.timer_remaining_ms.store(MIN_RESUME_MS, Ordering::SeqCst);
                tracing::debug!(
                    "[notifications] Hover ended; countdown clamped to MIN_RESUME_MS ({} ms)",
                    MIN_RESUME_MS
                );
            }
        }
    }

    /// Dispatches a reminder through the service: checks suppression, stores
    /// pending payload, initiates the show protocol with 3s fallback, arms
    /// the auto-dismiss timer, and dispatches the native OS toast as a display-only signal.
    pub fn show_reminder(
        self: &Arc<Self>,
        app: &AppHandle,
        entry: &ReminderEvent,
        is_capture_active: bool,
        active_meeting_id: Option<&str>,
        settings: &MeetingSettings,
    ) {
        // 1. Suppression check
        if is_capture_active {
            tracing::info!(
                "[notifications] Reminder suppressed (audio recording active): '{}' ({:?})",
                entry.title,
                entry.kind
            );
            return;
        }

        if active_meeting_id == Some(entry.meeting_id.as_str()) {
            tracing::info!(
                "[notifications] Reminder suppressed (meeting '{}' already recording)",
                entry.meeting_id
            );
            return;
        }

        let enabled_for_kind = match entry.kind {
            ReminderKind::Upcoming => settings.remind_before_meeting,
            ReminderKind::Unrecorded => settings.remind_if_unrecorded,
            ReminderKind::Detected => settings.remind_on_detection,
        };

        if !enabled_for_kind {
            tracing::info!(
                "[notifications] Reminder suppressed by user settings: '{}' ({:?})",
                entry.title,
                entry.kind
            );
            return;
        }

        // 2. Deduplication check: drop if identical active reminder is already showing
        {
            let current = self.active_reminder.lock().unwrap();
            if let Some(active) = &*current {
                if active.meeting_id == entry.meeting_id && active.kind == entry.kind {
                    tracing::debug!(
                        "[notifications] Reminder already active on screen: '{}' ({:?})",
                        entry.title,
                        entry.kind
                    );
                    return;
                }
            }
        }

        let payload = MeetingReminderPayload::from_reminder_event(entry);

        *self.active_reminder.lock().unwrap() = Some(entry.clone());
        *self.pending_payload.lock().unwrap() = Some(payload.clone());
        self.is_hovered.store(false, Ordering::SeqCst);

        // 3. Emit Tauri event with payload
        let _ = app.emit(MEETING_REMINDER_EVENT, &payload);

        let config_dir = app.path().app_config_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
        let dev_settings = crate::developer::load_developer_settings(&config_dir);
        let show_tauri = matches!(
            dev_settings.notification_surface_mode,
            crate::developer::NotificationSurfaceMode::Tauri | crate::developer::NotificationSurfaceMode::Both
        );
        let show_system = matches!(
            dev_settings.notification_surface_mode,
            crate::developer::NotificationSurfaceMode::System | crate::developer::NotificationSurfaceMode::Both
        );

        if show_tauri {
            // 4. Start auto-dismiss countdown timer (15 seconds with hover pause)
            self.timer_remaining_ms.store(AUTO_DISMISS_MS, Ordering::SeqCst);
            self.start_timer_loop(app.clone());

            // 5. Show protocol: if already ready, show immediately; otherwise arm 3s fallback
            if self.ready_handshake_done.load(Ordering::SeqCst) {
                overlay::show_reminder_window(app);
            } else {
                let service_arc = self.clone();
                let app_clone = app.clone();
                let payload_clone = payload.clone();
                tauri::async_runtime::spawn(async move {
                    tokio::time::sleep(Duration::from_millis(3000)).await;
                    if !service_arc.ready_handshake_done.load(Ordering::SeqCst) {
                        tracing::warn!(
                            "[notifications] Ready handshake timed out after 3000ms. Force-revealing reminder overlay."
                        );
                        let _ = app_clone.emit(MEETING_REMINDER_EVENT, &payload_clone);
                        overlay::show_reminder_window(&app_clone);
                    }
                });
            }
        }

        if show_system {
            // 6. Native OS Toast (display-only secondary signal)
            self.dispatch_demoted_native_toast(app, &payload);
        }
    }

    /// Hides the overlay and cleans up active reminder state.
    pub fn dismiss_overlay(&self, app: &AppHandle) {
        overlay::hide_reminder_window(app);
        *self.active_reminder.lock().unwrap() = None;
        *self.pending_payload.lock().unwrap() = None;
        self.timer_remaining_ms.store(0, Ordering::SeqCst);

        if let Some(handle) = self.timer_task.lock().unwrap().take() {
            handle.abort();
        }
    }

    /// Ticking timer loop for auto-dismiss with hover pause.
    fn start_timer_loop(self: &Arc<Self>, app: AppHandle) {
        if let Some(old) = self.timer_task.lock().unwrap().take() {
            old.abort();
        }

        let service = self.clone();
        let handle = tauri::async_runtime::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_millis(200));
            loop {
                interval.tick().await;

                // If hover is active, pause countdown
                if service.is_hovered.load(Ordering::SeqCst) {
                    continue;
                }

                let remaining = service.timer_remaining_ms.fetch_sub(200, Ordering::SeqCst);
                if remaining <= 200 {
                    tracing::info!("[notifications] Auto-dismiss timer expired. Hiding reminder overlay.");
                    service.dismiss_overlay(&app);
                    break;
                }
            }
        });

        *self.timer_task.lock().unwrap() = Some(handle);
    }

    /// Dispatches the native OS toast as a display-only fallback signal with clean title formatting.
    fn dispatch_demoted_native_toast(&self, app: &AppHandle, payload: &MeetingReminderPayload) {
        let is_generic_title = payload.title.eq_ignore_ascii_case("google meet session")
            || payload.title.eq_ignore_ascii_case("google meet")
            || payload.title.eq_ignore_ascii_case("zoom meeting")
            || payload.title.eq_ignore_ascii_case("teams meeting")
            || payload.title.eq_ignore_ascii_case("webex meeting");

        let (toast_title, toast_body) = if is_generic_title {
            (
                payload.time_label.clone(),
                format!("{} · Active Call", payload.provider_name),
            )
        } else {
            (
                payload.title.clone(),
                format!("{} · {}", payload.time_label, payload.provider_name),
            )
        };

        tracing::info!(
            "[notifications] Handing off native OS toast (display-only) for meeting '{}': '{}' / '{}'",
            payload.meeting_id,
            toast_title,
            toast_body
        );

        let _ = app
            .notification()
            .builder()
            .title(&toast_title)
            .body(&toast_body)
            .show();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::meetings::reminders::ReminderStatus;

    #[test]
    fn test_sanitize_and_clamp_text() {
        let input = "Meeting with Alice\u{202E} and Bob\n\t";
        let cleaned = sanitize_and_clamp_text(input, 30);
        assert_eq!(cleaned, "Meeting with Alice and Bob");

        let long = "A".repeat(150);
        let clamped = sanitize_and_clamp_text(&long, 50);
        assert_eq!(clamped.len(), 50);
    }

    #[test]
    fn test_sanitize_strips_bidi_overrides_and_control_chars() {
        let malicious_title = "\u{202A}Evil Meeting\u{202C}\u{0000}\u{001F}\u{2066}Title\u{2069}";
        let sanitized = sanitize_and_clamp_text(malicious_title, 80);
        assert_eq!(sanitized, "Evil MeetingTitle");
    }

    #[test]
    fn test_payload_from_reminder_event() {
        let event = ReminderEvent {
            meeting_id: "m_123".to_string(),
            kind: ReminderKind::Upcoming,
            title: "Sprint Planning\u{202D}".to_string(),
            provider: "google_meet".to_string(),
            participants: vec!["Alice".to_string(), "Bob\0".to_string()],
            fire_at: chrono::Utc::now(),
            status: ReminderStatus::Fired,
        };

        let payload = MeetingReminderPayload::from_reminder_event(&event);
        assert_eq!(payload.title, "Sprint Planning");
        assert_eq!(payload.provider_name, "Google Meet");
        assert_eq!(payload.time_label, "Starts in 5 minutes");
        assert_eq!(payload.participants, vec!["Alice", "Bob"]);
    }

    #[test]
    fn test_empty_title_falls_back_gracefully() {
        let event = ReminderEvent {
            meeting_id: "m_empty".to_string(),
            kind: ReminderKind::Detected,
            title: "\u{202E} \t \n".to_string(),
            provider: "zoom".to_string(),
            participants: vec![],
            fire_at: chrono::Utc::now(),
            status: ReminderStatus::Fired,
        };

        let payload = MeetingReminderPayload::from_reminder_event(&event);
        assert_eq!(payload.title, "Upcoming Meeting");
        assert_eq!(payload.provider_name, "Zoom");
    }

    #[test]
    fn test_hover_pause_and_minimum_resume_floor() {
        let service = NotificationService::new();
        service.timer_remaining_ms.store(2000, Ordering::SeqCst);

        // Enter hover
        service.on_hover_changed(true);
        assert!(service.is_hovered.load(Ordering::SeqCst));

        // Leave hover with timer < 5000ms -> should clamp to MIN_RESUME_MS
        service.on_hover_changed(false);
        assert_eq!(service.timer_remaining_ms.load(Ordering::SeqCst), MIN_RESUME_MS);

        // Leave hover when timer > 5000ms -> should NOT clamp down
        service.timer_remaining_ms.store(15000, Ordering::SeqCst);
        service.on_hover_changed(true);
        service.on_hover_changed(false);
        assert_eq!(service.timer_remaining_ms.load(Ordering::SeqCst), 15000);
    }

    #[test]
    fn test_auto_dismiss_duration_is_15_seconds() {
        assert_eq!(AUTO_DISMISS_MS, 15_000);
    }
}
