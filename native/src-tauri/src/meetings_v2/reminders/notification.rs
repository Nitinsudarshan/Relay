//! Putting a reminder on screen, and taking it away again.
//!
//! The surface is an app-owned Tauri overlay window, not an OS toast. That
//! choice was made twice and reversed once: `tauri-plugin-notification`'s
//! desktop implementation maps only title, body and icon, silently discarding
//! action buttons, so a toast can announce a meeting but can never carry Join,
//! Record or Snooze (`docs/decisions.md` Decision 46 and its reversal). A
//! reminder that cannot be acted on is not this feature.
//!
//! The OS toast is kept as a display-only second signal, because it survives a
//! fullscreen app that the overlay would be drawn behind.
//!
//! Two properties this module holds:
//!
//! * **A title is untrusted text.** Meeting titles are written by whoever sent
//!   the invitation. They are sanitized and clamped before they reach a window
//!   (`rules/security.md`).
//! * **The window shows itself only once the view is mounted.** Revealing an
//!   empty webview and filling it afterwards is what produced the flash of
//!   white the old implementation was known for.

use super::{ReminderEvent, ReminderKind};
use crate::overlay;
use crate::settings::MeetingSettings;
use crate::sync::MutexExt;
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicBool, AtomicI64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tauri::{async_runtime::JoinHandle, AppHandle, Emitter, Manager};
use tauri_plugin_notification::NotificationExt;

pub const MEETING_REMINDER_EVENT: &str = "meeting-reminder";

/// Longest title the card can show without the layout breaking.
const MAX_TITLE_LEN: usize = 80;
/// Longest subtitle or provider string.
const MAX_BODY_LEN: usize = 120;
/// Longest participant name.
const MAX_NAME_LEN: usize = 40;

/// How long the card stays up with no interaction.
pub const AUTO_DISMISS_MS: i64 = 15_000;

/// The floor the countdown resumes at when the pointer leaves the card.
///
/// Without it, moving the mouse off a card with 300 ms left dismisses it out
/// from under the click that was on its way.
pub const MIN_RESUME_MS: i64 = 5_000;

/// How often the countdown ticks.
const TICK_MS: i64 = 200;

/// How long the backend waits for the overlay to report itself mounted before
/// showing it anyway.
const READY_TIMEOUT_MS: u64 = 3_000;

/// Strips control characters and Unicode bidirectional overrides, then clamps.
///
/// Meeting titles come from calendar invitations and window titles — neither is
/// under the user's control. A bidi override in a title can make a card read as
/// something other than what it says, and an unbounded one can push the buttons
/// off the card entirely.
pub fn sanitize_and_clamp_text(input: &str, max_len: usize) -> String {
    let mut cleaned = String::with_capacity(input.len().min(max_len));
    for c in input.chars() {
        if c.is_control() && c != ' ' {
            continue;
        }
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

/// What the overlay window is given. Sanitized; nothing else crosses.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MeetingReminderPayload {
    pub key: String,
    pub kind: ReminderKind,
    pub title: String,
    pub provider: String,
    pub provider_name: String,
    /// When the meeting starts, in words — "Starts in 4 minutes".
    pub time_label: String,
    pub participants: Vec<String>,
    /// Whether this reminder has a conferencing link behind its Join button.
    pub can_join: bool,
}

/// "Starts in 4 minutes", "Started 6 minutes ago", and the honest cases either
/// side of them.
///
/// Derived from the real start time rather than from the reminder's kind. The
/// implementation this replaces hardcoded "Starts in 5 minutes" onto a reminder
/// that fired two minutes out, which is a small lie the user can check against
/// their own calendar.
fn time_label(entry: &ReminderEvent) -> String {
    match entry.kind {
        ReminderKind::Detected => "Meeting in progress".to_string(),
        _ => {
            let Some(starts_at) = entry.starts_at else {
                return match entry.kind {
                    ReminderKind::Unrecorded => "Meeting in progress".to_string(),
                    _ => "Starting soon".to_string(),
                };
            };
            let seconds = (starts_at - chrono::Utc::now()).num_seconds();
            match seconds {
                s if s > 90 => format!("Starts in {} minutes", (s as f64 / 60.0).round() as i64),
                s if s > 30 => "Starts in a minute".to_string(),
                s if s >= -30 => "Starting now".to_string(),
                s if s >= -90 => "Started a minute ago".to_string(),
                s => format!(
                    "Started {} minutes ago",
                    ((-s) as f64 / 60.0).round() as i64
                ),
            }
        }
    }
}

impl MeetingReminderPayload {
    pub fn from_reminder(entry: &ReminderEvent) -> Self {
        let title = sanitize_and_clamp_text(&entry.title, MAX_TITLE_LEN);
        let provider_name = super::detection::provider_display_name(&entry.provider);

        Self {
            key: entry.key.clone(),
            kind: entry.kind,
            // A card with no title at all reads as a bug; a neutral one reads
            // as a meeting whose name the calendar did not carry.
            title: if title.is_empty() {
                "Upcoming meeting".to_string()
            } else {
                title
            },
            provider: entry.provider.clone(),
            provider_name: provider_name.to_string(),
            time_label: sanitize_and_clamp_text(&time_label(entry), MAX_BODY_LEN),
            participants: entry
                .participants
                .iter()
                .map(|p| sanitize_and_clamp_text(p, MAX_NAME_LEN))
                .filter(|p| !p.is_empty())
                .collect(),
            can_join: entry
                .join_url
                .as_deref()
                .is_some_and(|url| url.starts_with("https://") || url.starts_with("http://")),
        }
    }
}

/// Owns the on-screen life of a reminder: what is showing, for how long, and
/// whether the pointer is on it.
///
/// Deliberately owns nothing else. It never starts a recording and never writes
/// to the queue — it raises an intent and the command layer decides
/// (`MEETINGS_LEGACY_REMOVAL.md`, Lesson 6).
pub struct NotificationService {
    showing: Mutex<Option<ReminderEvent>>,
    pending: Mutex<Option<MeetingReminderPayload>>,
    view_is_ready: AtomicBool,
    is_hovered: AtomicBool,
    remaining_ms: AtomicI64,
    countdown: Mutex<Option<JoinHandle<()>>>,
}

impl Default for NotificationService {
    fn default() -> Self {
        Self {
            showing: Mutex::new(None),
            pending: Mutex::new(None),
            view_is_ready: AtomicBool::new(false),
            is_hovered: AtomicBool::new(false),
            remaining_ms: AtomicI64::new(0),
            countdown: Mutex::new(None),
        }
    }
}

impl NotificationService {
    pub fn new() -> Self {
        Self::default()
    }

    /// What the overlay should render, for a view that mounted after the event
    /// was emitted.
    pub fn pending(&self) -> Option<MeetingReminderPayload> {
        self.pending.lock_or_recover().clone()
    }

    /// The reminder currently on screen, so an action can be resolved back to
    /// the queue entry that raised it.
    pub fn showing(&self) -> Option<ReminderEvent> {
        self.showing.lock_or_recover().clone()
    }

    /// The overlay reporting itself mounted and subscribed.
    ///
    /// The second half of the show protocol: Rust stages the payload and waits
    /// here, so the window is revealed with the card already painted.
    pub fn on_view_ready(&self, app: &AppHandle) {
        self.view_is_ready.store(true, Ordering::SeqCst);
        if self.pending.lock_or_recover().is_some() {
            overlay::show_reminder_window(app);
        }
    }

    /// Pointer entering or leaving the card.
    ///
    /// Entering pauses the countdown; leaving resumes it, never with less than
    /// `MIN_RESUME_MS` left.
    pub fn on_hover_changed(&self, hovered: bool) {
        let was_hovered = self.is_hovered.swap(hovered, Ordering::SeqCst);
        if was_hovered && !hovered {
            let remaining = self.remaining_ms.load(Ordering::SeqCst);
            if remaining > 0 && remaining < MIN_RESUME_MS {
                self.remaining_ms.store(MIN_RESUME_MS, Ordering::SeqCst);
            }
        }
    }

    /// Raises one reminder: stages it, reveals the overlay behind the ready
    /// handshake, arms the auto-dismiss countdown, and emits the OS toast.
    ///
    /// Suppression and deduplication happen here rather than at the call site,
    /// so every path that can raise a reminder — the scheduler, the developer
    /// mock triggers — obeys the same rules.
    pub fn show(
        self: &Arc<Self>,
        app: &AppHandle,
        entry: &ReminderEvent,
        is_recording: bool,
        settings: &MeetingSettings,
    ) {
        if is_recording {
            tracing::info!(
                "[reminders] '{}' ({:?}) suppressed: a recording is already running",
                entry.title,
                entry.kind
            );
            return;
        }

        let enabled = match entry.kind {
            ReminderKind::Upcoming => settings.remind_before_meeting,
            ReminderKind::Unrecorded => settings.remind_if_unrecorded,
            ReminderKind::Detected => settings.remind_on_detection,
        };
        if !enabled {
            tracing::info!(
                "[reminders] '{}' ({:?}) suppressed by settings",
                entry.title,
                entry.kind
            );
            return;
        }

        // Already on screen: re-showing it would restart its countdown under
        // the user's pointer.
        if let Some(active) = self.showing.lock_or_recover().as_ref() {
            if active.key == entry.key && active.kind == entry.kind {
                return;
            }
        }

        let payload = MeetingReminderPayload::from_reminder(entry);
        *self.showing.lock_or_recover() = Some(entry.clone());
        *self.pending.lock_or_recover() = Some(payload.clone());
        self.is_hovered.store(false, Ordering::SeqCst);

        let _ = app.emit(MEETING_REMINDER_EVENT, &payload);

        let config_dir = app
            .path()
            .app_config_dir()
            .unwrap_or_else(|_| std::path::PathBuf::from("."));
        let surface = crate::developer::load_developer_settings(&config_dir).notification_surface_mode;
        use crate::developer::NotificationSurfaceMode;
        let show_overlay = matches!(
            surface,
            NotificationSurfaceMode::Tauri | NotificationSurfaceMode::Both
        );
        let show_toast = matches!(
            surface,
            NotificationSurfaceMode::System | NotificationSurfaceMode::Both
        );

        if show_overlay {
            self.remaining_ms.store(AUTO_DISMISS_MS, Ordering::SeqCst);
            self.start_countdown(app.clone());

            if self.view_is_ready.load(Ordering::SeqCst) {
                overlay::show_reminder_window(app);
            } else {
                // The window is created hidden at startup, but its webview may
                // still be booting on the first reminder of a session. Show it
                // anyway rather than losing the reminder to a handshake that
                // never arrived.
                let service = self.clone();
                let app = app.clone();
                let payload = payload.clone();
                tauri::async_runtime::spawn(async move {
                    tokio::time::sleep(Duration::from_millis(READY_TIMEOUT_MS)).await;
                    if !service.view_is_ready.load(Ordering::SeqCst) {
                        tracing::warn!(
                            "[reminders] overlay did not report ready within {READY_TIMEOUT_MS}ms; showing it anyway"
                        );
                        let _ = app.emit(MEETING_REMINDER_EVENT, &payload);
                        overlay::show_reminder_window(&app);
                    }
                });
            }
        }

        if show_toast {
            self.emit_os_toast(app, &payload);
        }
    }

    /// Takes the card off screen and clears what it was showing.
    pub fn dismiss(&self, app: &AppHandle) {
        overlay::hide_reminder_window(app);
        *self.showing.lock_or_recover() = None;
        *self.pending.lock_or_recover() = None;
        self.remaining_ms.store(0, Ordering::SeqCst);
        if let Some(countdown) = self.countdown.lock_or_recover().take() {
            countdown.abort();
        }
    }

    fn start_countdown(self: &Arc<Self>, app: AppHandle) {
        if let Some(previous) = self.countdown.lock_or_recover().take() {
            previous.abort();
        }

        let service = self.clone();
        let handle = tauri::async_runtime::spawn(async move {
            let mut ticker = tokio::time::interval(Duration::from_millis(TICK_MS as u64));
            loop {
                ticker.tick().await;
                if service.is_hovered.load(Ordering::SeqCst) {
                    continue;
                }
                let remaining = service.remaining_ms.fetch_sub(TICK_MS, Ordering::SeqCst);
                if remaining <= TICK_MS {
                    service.dismiss(&app);
                    break;
                }
            }
        });

        *self.countdown.lock_or_recover() = Some(handle);
    }

    /// The display-only OS toast.
    ///
    /// Carries no actions, because the desktop plugin cannot deliver them. It
    /// exists for the case the overlay cannot cover: a fullscreen app the
    /// always-on-top window is drawn behind.
    fn emit_os_toast(&self, app: &AppHandle, payload: &MeetingReminderPayload) {
        // A topicless title says less than the provider does, so lead with the
        // time and let the provider carry the identification.
        let generic = super::detection::is_generic_fallback_title(&payload.title);
        let (title, body) = if generic {
            (
                payload.time_label.clone(),
                format!("{} · active call", payload.provider_name),
            )
        } else {
            (
                payload.title.clone(),
                format!("{} · {}", payload.time_label, payload.provider_name),
            )
        };

        if let Err(e) = app.notification().builder().title(&title).body(&body).show() {
            tracing::warn!("[reminders] OS toast could not be shown: {e}");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{Duration as ChronoDuration, Utc};

    fn reminder(kind: ReminderKind, title: &str, starts_in_seconds: i64) -> ReminderEvent {
        ReminderEvent {
            key: "cal:evt_1".to_string(),
            kind,
            title: title.to_string(),
            provider: super::super::detection::PROVIDER_GOOGLE_MEET.to_string(),
            participants: vec!["Pranjali Sharma".to_string()],
            starts_at: Some(Utc::now() + ChronoDuration::seconds(starts_in_seconds)),
            join_url: Some("https://meet.google.com/abc-defg-hij".to_string()),
            fire_at: Utc::now(),
            status: super::super::ReminderStatus::Fired,
        }
    }

    #[test]
    fn a_title_loses_its_control_characters_and_bidi_overrides() {
        let sanitized = sanitize_and_clamp_text("Meeting with Alice\u{202E} and Bob\n\t", 80);
        assert_eq!(sanitized, "Meeting with Alice and Bob");

        let sanitized =
            sanitize_and_clamp_text("\u{202A}Evil meeting\u{202C}\u{0000}\u{001F}\u{2066}Title\u{2069}", 80);
        assert_eq!(sanitized, "Evil meetingTitle");
    }

    #[test]
    fn an_overlong_title_is_clamped_rather_than_pushing_the_buttons_off_the_card() {
        let clamped = sanitize_and_clamp_text(&"A".repeat(500), MAX_TITLE_LEN);
        assert_eq!(clamped.chars().count(), MAX_TITLE_LEN);
    }

    #[test]
    fn a_payload_carries_the_sanitized_title_and_the_readable_provider() {
        let payload =
            MeetingReminderPayload::from_reminder(&reminder(ReminderKind::Upcoming, "Sprint planning\u{202D}", 240));
        assert_eq!(payload.title, "Sprint planning");
        assert_eq!(payload.provider_name, "Google Meet");
        assert!(payload.can_join);
    }

    #[test]
    fn a_title_that_sanitizes_to_nothing_still_reads_as_a_meeting() {
        let payload =
            MeetingReminderPayload::from_reminder(&reminder(ReminderKind::Upcoming, "\u{202E} \t \n", 60));
        assert_eq!(payload.title, "Upcoming meeting");
    }

    #[test]
    fn the_time_label_matches_the_real_start_rather_than_the_reminder_kind() {
        assert_eq!(
            MeetingReminderPayload::from_reminder(&reminder(ReminderKind::Upcoming, "Standup", 240)).time_label,
            "Starts in 4 minutes"
        );
        assert_eq!(
            MeetingReminderPayload::from_reminder(&reminder(ReminderKind::Upcoming, "Standup", 5)).time_label,
            "Starting now"
        );
        assert_eq!(
            MeetingReminderPayload::from_reminder(&reminder(ReminderKind::Unrecorded, "Standup", -360)).time_label,
            "Started 6 minutes ago"
        );
    }

    #[test]
    fn a_reminder_with_no_link_offers_no_join() {
        let mut entry = reminder(ReminderKind::Detected, "Zoom Meeting", 0);
        entry.join_url = None;
        assert!(!MeetingReminderPayload::from_reminder(&entry).can_join);
    }

    #[test]
    fn leaving_the_card_never_resumes_with_less_than_the_floor() {
        let service = NotificationService::new();
        service.remaining_ms.store(2_000, Ordering::SeqCst);

        service.on_hover_changed(true);
        service.on_hover_changed(false);
        assert_eq!(service.remaining_ms.load(Ordering::SeqCst), MIN_RESUME_MS);
    }

    #[test]
    fn a_countdown_with_time_to_spare_is_left_alone_by_a_hover() {
        let service = NotificationService::new();
        service.remaining_ms.store(AUTO_DISMISS_MS, Ordering::SeqCst);

        service.on_hover_changed(true);
        service.on_hover_changed(false);
        assert_eq!(service.remaining_ms.load(Ordering::SeqCst), AUTO_DISMISS_MS);
    }
}
