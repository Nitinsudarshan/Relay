//! Telling somebody a meeting is about to happen, and letting them act on it.
//!
//! Three questions decide whether a reminder exists at all, and they are asked
//! of two independent sources. The calendar knows what was scheduled and who
//! was invited; `detection` knows which conferencing app is actually on screen.
//! Neither alone is enough — a calendar entry for a call the user quietly left
//! is not a meeting, and a stray idle Zoom window is not one either.
//!
//! Three properties this module holds:
//!
//! * **A queue, never a slot.** Every tracked reminder is one entry keyed by
//!   `(key, kind)`. The subsystem this replaces held a single overwritable
//!   payload, so a second reminder silently erased the first one before anybody
//!   had seen it (`docs/decisions.md` Decision 45, Broken #2).
//! * **Fired once, per transition.** `recompute` returns what *became* due on
//!   this call, not everything currently due, so the caller raises one
//!   notification per reminder rather than one per poll tick.
//! * **A key is not a session id.** A reminder names a calendar event or a
//!   window, both of which exist before any recording does. Handing such a
//!   string to a command that wants a real session id is what made the old
//!   popup's primary action fail for every reminder it ever showed (Decision
//!   45, Broken #1); the type keeps them apart.

pub mod detection;
pub mod notification;
pub mod scheduler;

use crate::calendar::CalendarEvent;
use crate::meetings_v2::types::{MeetingSession, MeetingState};
use crate::settings::MeetingSettings;
use crate::sync::MutexExt;
use chrono::{DateTime, Duration, Utc};
use detection::WindowMatch;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};
use std::sync::Mutex;

pub use notification::{MeetingReminderPayload, NotificationService};

/// Why a reminder exists.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReminderKind {
    /// A scheduled meeting is about to start.
    Upcoming,
    /// A scheduled meeting has started and nothing is recording it.
    Unrecorded,
    /// A conferencing call is on screen that the calendar knows nothing about.
    Detected,
}

/// `Pending -> Fired -> { Snoozed | Dismissed | Actioned | Expired }`.
///
/// `Expired` is passive: the window passed with no interaction. It is a
/// terminal record, never an interruption of its own.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum ReminderStatus {
    Pending,
    Fired,
    Snoozed { until: DateTime<Utc> },
    Dismissed,
    Actioned,
    Expired,
}

/// One tracked reminder.
#[derive(Debug, Clone, Serialize)]
pub struct ReminderEvent {
    /// Stable identity of what is being reminded about — `cal:<event id>` for a
    /// calendar event, `win:<provider>:<title>` for a detected call.
    ///
    /// Deliberately not called `meeting_id`: nothing is recorded yet, so there
    /// is no session for this to be the id of.
    pub key: String,
    pub kind: ReminderKind,
    pub title: String,
    pub provider: String,
    pub participants: Vec<String>,
    /// When the meeting itself starts, where that is known.
    pub starts_at: Option<DateTime<Utc>>,
    /// The conferencing link, when the event carries one. What "Join" opens.
    pub join_url: Option<String>,
    pub fire_at: DateTime<Utc>,
    pub status: ReminderStatus,
}

impl ReminderEvent {
    pub fn key_for_event(event_id: &str) -> String {
        format!("cal:{event_id}")
    }

    pub fn key_for_window(window: &WindowMatch) -> String {
        format!("win:{}:{}", window.provider, window.title.to_lowercase())
    }
}

/// Every tracked reminder, plus how many consecutive ticks each detected window
/// has been seen for. Tauri-managed state.
#[derive(Default)]
pub struct ReminderQueue {
    entries: Mutex<Vec<ReminderEvent>>,
    /// Consecutive sightings per detection key, for the graduation rule below.
    sightings: Mutex<HashMap<String, u32>>,
}

/// How long before a scheduled start an `Upcoming` reminder appears.
///
/// Five minutes is enough to finish a sentence, open the call and press record;
/// much earlier and it is noise the user learns to swipe away.
const UPCOMING_LEAD_SECONDS: i64 = 300;

/// The window in which an `Unrecorded` reminder appears, measured from the
/// scheduled start.
///
/// It opens late on purpose: a meeting that is five minutes old and still
/// unrecorded is a meeting somebody meant to record and forgot, whereas one
/// that is thirty seconds old is somebody still saying hello.
const UNRECORDED_FROM_SECONDS: i64 = 300;
const UNRECORDED_TO_SECONDS: i64 = 420;

/// How long a `Fired` or `Snoozed` reminder can go un-actioned before it is
/// `Expired`.
const EXPIRE_AFTER_MINUTES: i64 = 10;

/// Consecutive sightings a topicless conferencing window needs before it earns
/// a `Detected` reminder.
///
/// A window titled "Zoom Meeting" is as likely to be an idle app as a live
/// call. Requiring it to persist across ticks is what keeps an interruption
/// tied to evidence rather than to a single frame.
const GENERIC_SIGHTINGS_REQUIRED: u32 = 2;

/// Confidence at or above which a detected window is specific enough to
/// interrupt somebody on the first sighting.
const CONFIDENT_DETECTION: f32 = 0.8;

/// Everything `recompute` needs to decide what should exist right now.
pub struct ReminderInputs<'a> {
    pub events: &'a [CalendarEvent],
    pub windows: &'a [WindowMatch],
    pub settings: &'a MeetingSettings,
    /// Whether Relay is recording anything at all. One session runs at a time,
    /// so this answers "is something being recorded right now".
    pub is_recording: bool,
    /// Recordings that already exist, so a meeting somebody recorded and
    /// stopped early is not then reported as unrecorded.
    ///
    /// "Nothing is recording" is not the same question as "this meeting went
    /// unrecorded", and answering the second with the first is what makes a
    /// reminder arrive about a meeting the user already dealt with.
    pub sessions: &'a [MeetingSession],
    pub now: DateTime<Utc>,
}

/// How far a recording's start may sit from a meeting's start and still be
/// taken as a recording *of* that meeting.
///
/// Generous on purpose: somebody who starts recording eight minutes late has
/// still recorded the meeting, and reminding them otherwise is the false
/// positive that gets reminders switched off.
const SESSION_COVERS_WITHIN_MINUTES: i64 = 30;

/// Whether an existing recording covers this meeting.
///
/// A recording counts whether it is still running or already finished — the
/// question is whether this meeting was captured, not whether it is being
/// captured at this instant.
fn a_recording_covers(
    sessions: &[MeetingSession],
    event_start: DateTime<Utc>,
    event_end: Option<DateTime<Utc>>,
) -> bool {
    sessions.iter().any(|session| {
        if !matches!(
            session.state,
            MeetingState::Recording
                | MeetingState::Paused
                | MeetingState::Stopping
                | MeetingState::Finalizing
                | MeetingState::Completed
                | MeetingState::Recovered
        ) {
            return false;
        }

        let Some(started) = session
            .started_at
            .as_deref()
            .and_then(|at| DateTime::parse_from_rfc3339(at).ok())
            .map(|at| at.with_timezone(&Utc))
        else {
            return false;
        };

        let began_before_the_meeting_ended = event_end.is_none_or(|end| started < end);
        let began_near_the_meeting = (started - event_start).num_minutes().abs()
            <= SESSION_COVERS_WITHIN_MINUTES;

        began_before_the_meeting_ended && began_near_the_meeting
    })
}

fn participants_of(event: &CalendarEvent) -> Vec<String> {
    event
        .likely_attendees()
        .iter()
        .map(|attendee| attendee.name.clone())
        .collect()
}

fn provider_of(event: &CalendarEvent) -> String {
    event
        .conference_url
        .as_deref()
        .map(detection::provider_from_url)
        .unwrap_or(detection::PROVIDER_OTHER)
        .to_string()
}

/// Whether a live conferencing window plausibly belongs to this event.
///
/// Matching on provider rather than on title: window titles and calendar titles
/// agree far less often than they look like they should, and a false negative
/// here only costs a reminder, whereas matching on nothing at all resurrects
/// "reminded me about a meeting I had already left".
fn event_is_on_screen(event: &CalendarEvent, windows: &[WindowMatch]) -> bool {
    let provider = provider_of(event);
    if provider == detection::PROVIDER_OTHER {
        // An in-person meeting has no window to find, so its absence proves
        // nothing and must not suppress the reminder.
        return true;
    }
    windows
        .iter()
        .any(|window| window.provider.eq_ignore_ascii_case(&provider))
}

/// Adds an entry unless one already exists for this `(key, kind)`.
///
/// Never overwrites: an existing entry carries the user's snooze or dismissal,
/// and replacing it is how a dismissed reminder comes back.
fn ensure_entry(entries: &mut Vec<ReminderEvent>, candidate: ReminderEvent) {
    if entries
        .iter()
        .any(|e| e.key == candidate.key && e.kind == candidate.kind)
    {
        return;
    }
    entries.push(candidate);
}

/// Reconciles the queue against what should exist right now.
///
/// Returns `(everything tracked, what became due on this call)`. The second is
/// what the caller notifies on — once per transition into `Fired`, not once per
/// tick while it stays there.
pub fn recompute(queue: &ReminderQueue, inputs: &ReminderInputs<'_>) -> (Vec<ReminderEvent>, Vec<ReminderEvent>) {
    let ReminderInputs {
        events,
        windows,
        settings,
        is_recording,
        sessions,
        now,
    } = *inputs;

    let mut entries = queue.entries.lock_or_recover();

    for event in events {
        let Some(starts_at) = event.starts() else {
            continue;
        };
        let ends_at = event.ends();
        // A meeting that has already finished has nothing left to remind about.
        if ends_at.is_some_and(|end| end < now) {
            continue;
        }

        let seconds_until_start = (starts_at - now).num_seconds();

        if settings.remind_before_meeting
            && seconds_until_start > 0
            && seconds_until_start <= UPCOMING_LEAD_SECONDS
        {
            ensure_entry(
                &mut entries,
                ReminderEvent {
                    key: ReminderEvent::key_for_event(&event.id),
                    kind: ReminderKind::Upcoming,
                    title: event.title.clone(),
                    provider: provider_of(event),
                    participants: participants_of(event),
                    starts_at: Some(starts_at),
                    join_url: event.conference_url.clone(),
                    fire_at: now,
                    status: ReminderStatus::Pending,
                },
            );
        }

        let seconds_since_start = (now - starts_at).num_seconds();
        if settings.remind_if_unrecorded
            && !is_recording
            && (UNRECORDED_FROM_SECONDS..=UNRECORDED_TO_SECONDS).contains(&seconds_since_start)
            && !a_recording_covers(sessions, starts_at, ends_at)
            && event_is_on_screen(event, windows)
        {
            ensure_entry(
                &mut entries,
                ReminderEvent {
                    key: ReminderEvent::key_for_event(&event.id),
                    kind: ReminderKind::Unrecorded,
                    title: event.title.clone(),
                    provider: provider_of(event),
                    participants: participants_of(event),
                    starts_at: Some(starts_at),
                    join_url: event.conference_url.clone(),
                    fire_at: now,
                    status: ReminderStatus::Pending,
                },
            );
        }
    }

    // Detection: only for calls the calendar does not already account for, so a
    // scheduled meeting never produces two reminders about itself.
    let scheduled_providers: Vec<String> = events
        .iter()
        .filter(|event| {
            event
                .starts()
                .is_some_and(|start| (now - start).num_seconds().abs() < 3600)
        })
        .map(provider_of)
        .collect();

    let mut sightings = queue.sightings.lock_or_recover();
    let visible_keys: Vec<String> = windows.iter().map(ReminderEvent::key_for_window).collect();
    sightings.retain(|key, _| visible_keys.contains(key));

    for window in windows {
        let key = ReminderEvent::key_for_window(window);
        let seen = sightings.entry(key.clone()).or_insert(0);
        *seen = seen.saturating_add(1);

        if !settings.remind_on_detection || is_recording {
            continue;
        }
        if scheduled_providers
            .iter()
            .any(|provider| provider.eq_ignore_ascii_case(&window.provider))
        {
            continue;
        }
        // A specific title is evidence enough on its own; a topicless one has
        // to persist before it earns an interruption.
        let earned = window.confidence >= CONFIDENT_DETECTION || *seen >= GENERIC_SIGHTINGS_REQUIRED;
        if !earned {
            continue;
        }

        ensure_entry(
            &mut entries,
            ReminderEvent {
                key,
                kind: ReminderKind::Detected,
                title: window.title.clone(),
                provider: window.provider.clone(),
                participants: Vec::new(),
                starts_at: None,
                join_url: None,
                fire_at: now,
                status: ReminderStatus::Pending,
            },
        );
    }
    drop(sightings);

    let mut newly_fired = Vec::new();
    for entry in entries.iter_mut() {
        let was_fired = matches!(entry.status, ReminderStatus::Fired);

        let due = match &entry.status {
            ReminderStatus::Pending => entry.fire_at <= now,
            ReminderStatus::Snoozed { until } => *until <= now,
            _ => false,
        };

        if due {
            entry.status = ReminderStatus::Fired;
        } else {
            let stale = match &entry.status {
                ReminderStatus::Fired => (now - entry.fire_at).num_minutes() > EXPIRE_AFTER_MINUTES,
                ReminderStatus::Snoozed { until } => {
                    (now - *until).num_minutes() > EXPIRE_AFTER_MINUTES
                }
                _ => false,
            };
            if stale {
                tracing::info!(
                    "[reminders] '{}' ({:?}) expired unactioned after {} minutes",
                    entry.title,
                    entry.kind,
                    EXPIRE_AFTER_MINUTES
                );
                entry.status = ReminderStatus::Expired;
            }
        }

        if !was_fired && matches!(entry.status, ReminderStatus::Fired) {
            newly_fired.push(entry.clone());
        }
    }

    // Drop reminders whose subject is gone: the calendar event ended or was
    // deleted, or the conferencing window closed.
    let live_event_keys: Vec<String> = events
        .iter()
        .filter(|event| event.ends().is_none_or(|end| end >= now))
        .map(|event| ReminderEvent::key_for_event(&event.id))
        .collect();
    entries.retain(|entry| match entry.kind {
        ReminderKind::Detected => visible_keys.contains(&entry.key),
        _ => live_event_keys.contains(&entry.key),
    });

    (entries.clone(), newly_fired)
}

/// The next reminder to put on screen, taken from what is waiting to be shown.
///
/// One card is on screen at a time, so reminders that become due while another
/// is up wait in a backlog rather than stacking or — as in the implementation
/// this replaces — being dropped on the floor. Two meetings starting in the
/// same minute is the ordinary case this exists for.
///
/// An entry the user has answered in the meantime is skipped rather than shown
/// late: dismissing the card for one meeting and then being shown a reminder
/// for a meeting already recording is worse than saying nothing.
pub fn take_next_to_show(
    queue: &ReminderQueue,
    backlog: &mut VecDeque<ReminderEvent>,
) -> Option<ReminderEvent> {
    while let Some(candidate) = backlog.pop_front() {
        let still_due = find(queue, &candidate.key, candidate.kind)
            .is_some_and(|entry| matches!(entry.status, ReminderStatus::Fired));
        if still_due {
            return Some(candidate);
        }
    }
    None
}

/// Queues newly-due reminders behind whatever is already waiting.
///
/// Deduplicated on `(key, kind)`: a reminder that was snoozed and came back
/// must not sit in the backlog twice and interrupt twice.
pub fn enqueue_to_show(backlog: &mut VecDeque<ReminderEvent>, newly_fired: Vec<ReminderEvent>) {
    for entry in newly_fired {
        let already_queued = backlog
            .iter()
            .any(|queued| queued.key == entry.key && queued.kind == entry.kind);
        if !already_queued {
            backlog.push_back(entry);
        }
    }
}

/// The reminder the popup should be showing, if any — the earliest-firing entry
/// that is currently `Fired`.
///
/// Derived from the queue rather than stored beside it, so there is nowhere for
/// "the second reminder replaced the first" to reappear.
pub fn current(queue: &ReminderQueue) -> Option<ReminderEvent> {
    queue
        .entries
        .lock_or_recover()
        .iter()
        .filter(|entry| matches!(entry.status, ReminderStatus::Fired))
        .min_by_key(|entry| entry.fire_at)
        .cloned()
}

pub fn find(queue: &ReminderQueue, key: &str, kind: ReminderKind) -> Option<ReminderEvent> {
    queue
        .entries
        .lock_or_recover()
        .iter()
        .find(|entry| entry.key == key && entry.kind == kind)
        .cloned()
}

pub fn dismiss(queue: &ReminderQueue, key: &str, kind: ReminderKind) {
    let mut entries = queue.entries.lock_or_recover();
    if let Some(entry) = entries
        .iter_mut()
        .find(|entry| entry.key == key && entry.kind == kind)
    {
        entry.status = ReminderStatus::Dismissed;
    }
}

pub fn snooze(queue: &ReminderQueue, key: &str, kind: ReminderKind, minutes: i64) {
    let mut entries = queue.entries.lock_or_recover();
    if let Some(entry) = entries
        .iter_mut()
        .find(|entry| entry.key == key && entry.kind == kind)
    {
        entry.status = ReminderStatus::Snoozed {
            until: Utc::now() + Duration::minutes(minutes),
        };
    }
}

/// Resolves every open reminder for one subject as `Actioned`.
///
/// Called as a side effect of starting that meeting's recording, wherever that
/// was triggered from — the popup, the meetings list, or the tray. One function
/// clearing all of them is what keeps those entry points from disagreeing about
/// whether a meeting has been seen (Decision 45, Refactor #1).
pub fn mark_actioned(queue: &ReminderQueue, key: &str) {
    let mut entries = queue.entries.lock_or_recover();
    for entry in entries.iter_mut().filter(|entry| entry.key == key) {
        if matches!(
            entry.status,
            ReminderStatus::Pending | ReminderStatus::Fired | ReminderStatus::Snoozed { .. }
        ) {
            entry.status = ReminderStatus::Actioned;
        }
    }
}

/// Testing only: injects an already-`Fired` reminder, bypassing the timing and
/// detection gates.
///
/// Backs Settings › Developer's mock reminder buttons, so the popup's real
/// actions can be exercised without waiting for a real meeting.
pub fn inject_mock(queue: &ReminderQueue, kind: ReminderKind) -> ReminderEvent {
    let (title, provider, join_url) = match kind {
        ReminderKind::Upcoming => (
            "Placement review",
            detection::PROVIDER_GOOGLE_MEET,
            Some("https://meet.google.com/abc-defg-hij".to_string()),
        ),
        ReminderKind::Unrecorded => ("Sprint planning", detection::PROVIDER_ZOOM, None),
        ReminderKind::Detected => ("Google Meet Session", detection::PROVIDER_GOOGLE_MEET, None),
    };

    let event = ReminderEvent {
        key: format!("mock:{kind:?}").to_lowercase(),
        kind,
        title: title.to_string(),
        provider: provider.to_string(),
        participants: vec!["Pranjali Sharma".to_string(), "Ayush Kumar".to_string()],
        starts_at: Some(Utc::now() + Duration::minutes(2)),
        join_url,
        fire_at: Utc::now(),
        status: ReminderStatus::Fired,
    };

    let mut entries = queue.entries.lock_or_recover();
    entries.retain(|entry| entry.key != event.key);
    entries.push(event.clone());
    event
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::calendar::{AttendanceResponse, CalendarAttendee};

    fn settings_all_on() -> MeetingSettings {
        MeetingSettings {
            remind_before_meeting: true,
            remind_if_unrecorded: true,
            remind_on_detection: true,
            ..MeetingSettings::default()
        }
    }

    fn event_starting_in(id: &str, seconds: i64) -> CalendarEvent {
        let start = Utc::now() + Duration::seconds(seconds);
        CalendarEvent {
            id: id.to_string(),
            title: format!("Meeting {id}"),
            starts_at: start.to_rfc3339(),
            ends_at: (start + Duration::minutes(30)).to_rfc3339(),
            description: None,
            location: None,
            attendees: vec![CalendarAttendee {
                name: "Pranjali Sharma".to_string(),
                email: None,
                response: AttendanceResponse::Accepted,
                is_organizer: true,
                is_self: false,
            }],
            conference_url: Some("https://meet.google.com/abc-defg-hij".to_string()),
            organizer: None,
            calendar_id: None,
            calendar_name: None,
            calendar_color: None,
        }
    }

    fn zoom_window(title: &str) -> WindowMatch {
        WindowMatch {
            provider: detection::PROVIDER_ZOOM.to_string(),
            title: title.to_string(),
            raw_title: format!("{title} - Zoom"),
            source: "window_detector".to_string(),
            confidence: detection::score_confidence(title),
        }
    }

    fn inputs<'a>(
        events: &'a [CalendarEvent],
        windows: &'a [WindowMatch],
        settings: &'a MeetingSettings,
        is_recording: bool,
    ) -> ReminderInputs<'a> {
        ReminderInputs {
            events,
            windows,
            settings,
            is_recording,
            sessions: &[],
            now: Utc::now(),
        }
    }

    fn session_started_at(state: MeetingState, started: DateTime<Utc>) -> MeetingSession {
        let mut session = MeetingSession::new("meet_test".to_string(), None);
        session.state = state;
        session.started_at = Some(started.to_rfc3339());
        session
    }

    #[test]
    fn a_meeting_inside_the_lead_window_fires_once_and_stays_fired() {
        let settings = settings_all_on();
        let events = vec![event_starting_in("evt_1", 120)];
        let queue = ReminderQueue::default();

        let (all, fired) = recompute(&queue, &inputs(&events, &[], &settings, false));
        assert_eq!(fired.len(), 1, "the reminder becomes due on this call");
        assert!(matches!(all[0].status, ReminderStatus::Fired));

        // A second pass while still due must not raise a second notification.
        let (_, fired_again) = recompute(&queue, &inputs(&events, &[], &settings, false));
        assert!(fired_again.is_empty(), "already-fired reminders stay quiet");
    }

    #[test]
    fn a_meeting_beyond_the_lead_window_is_not_yet_a_reminder() {
        let settings = settings_all_on();
        let events = vec![event_starting_in("evt_1", UPCOMING_LEAD_SECONDS + 120)];
        let queue = ReminderQueue::default();

        let (all, fired) = recompute(&queue, &inputs(&events, &[], &settings, false));
        assert!(all.is_empty());
        assert!(fired.is_empty());
    }

    #[test]
    fn a_second_reminder_never_erases_the_first() {
        // The single-slot design this replaces lost one of these entirely.
        let settings = settings_all_on();
        let events = vec![event_starting_in("evt_1", 60), event_starting_in("evt_2", 90)];
        let queue = ReminderQueue::default();

        let (all, fired) = recompute(&queue, &inputs(&events, &[], &settings, false));
        assert_eq!(all.len(), 2);
        assert_eq!(fired.len(), 2);
    }

    #[test]
    fn an_unrecorded_reminder_needs_the_call_to_still_be_on_screen() {
        let settings = settings_all_on();
        let mut event = event_starting_in("evt_1", -330);
        event.conference_url = Some("https://zoom.us/j/123".to_string());
        let events = vec![event];

        // Nobody is on the call any more — the user left, so there is nothing
        // to record and nothing to say.
        let queue = ReminderQueue::default();
        let (all, _) = recompute(&queue, &inputs(&events, &[], &settings, false));
        assert!(all.iter().all(|e| e.kind != ReminderKind::Unrecorded));

        // With Zoom actually open, the reminder is real.
        let queue = ReminderQueue::default();
        let windows = vec![zoom_window("Sprint planning")];
        let (all, _) = recompute(&queue, &inputs(&events, &windows, &settings, false));
        assert!(all.iter().any(|e| e.kind == ReminderKind::Unrecorded));
    }

    #[test]
    fn nothing_is_raised_while_a_recording_is_already_running() {
        let settings = settings_all_on();
        let mut event = event_starting_in("evt_1", -330);
        event.conference_url = Some("https://zoom.us/j/123".to_string());
        let events = vec![event];
        let windows = vec![zoom_window("Sprint planning")];
        let queue = ReminderQueue::default();

        let (all, _) = recompute(&queue, &inputs(&events, &windows, &settings, true));
        assert!(all.iter().all(|e| e.kind != ReminderKind::Unrecorded));
        assert!(all.iter().all(|e| e.kind != ReminderKind::Detected));
    }

    #[test]
    fn a_topicless_window_has_to_persist_before_it_interrupts() {
        let settings = settings_all_on();
        let windows = vec![zoom_window("Zoom Meeting")];
        let queue = ReminderQueue::default();

        let (all, _) = recompute(&queue, &inputs(&[], &windows, &settings, false));
        assert!(all.is_empty(), "one sighting of a generic title proves nothing");

        let (all, fired) = recompute(&queue, &inputs(&[], &windows, &settings, false));
        assert_eq!(all.len(), 1);
        assert_eq!(fired.len(), 1);
    }

    #[test]
    fn a_named_call_is_evidence_enough_on_the_first_sighting() {
        let settings = settings_all_on();
        let windows = vec![zoom_window("Placement review")];
        let queue = ReminderQueue::default();

        let (_, fired) = recompute(&queue, &inputs(&[], &windows, &settings, false));
        assert_eq!(fired.len(), 1);
        assert_eq!(fired[0].kind, ReminderKind::Detected);
    }

    #[test]
    fn a_scheduled_call_does_not_also_report_itself_as_detected() {
        let settings = settings_all_on();
        let mut event = event_starting_in("evt_1", -60);
        event.conference_url = Some("https://zoom.us/j/123".to_string());
        let events = vec![event];
        let windows = vec![zoom_window("Sprint planning")];
        let queue = ReminderQueue::default();

        let (all, _) = recompute(&queue, &inputs(&events, &windows, &settings, false));
        assert!(all.iter().all(|e| e.kind != ReminderKind::Detected));
    }

    #[test]
    fn a_closed_window_drops_its_reminder() {
        let settings = settings_all_on();
        let windows = vec![zoom_window("Placement review")];
        let queue = ReminderQueue::default();
        recompute(&queue, &inputs(&[], &windows, &settings, false));

        let (all, _) = recompute(&queue, &inputs(&[], &[], &settings, false));
        assert!(all.is_empty(), "the call ended, so the reminder goes with it");
    }

    #[test]
    fn a_snoozed_reminder_stays_quiet_until_its_time() {
        let settings = settings_all_on();
        let events = vec![event_starting_in("evt_1", 120)];
        let queue = ReminderQueue::default();
        recompute(&queue, &inputs(&events, &[], &settings, false));

        snooze(&queue, "cal:evt_1", ReminderKind::Upcoming, 5);
        let (all, fired) = recompute(&queue, &inputs(&events, &[], &settings, false));
        assert!(fired.is_empty());
        assert!(matches!(all[0].status, ReminderStatus::Snoozed { .. }));

        // Rewinding the snooze to the past is what "5 minutes later" looks like
        // without sleeping in a test.
        snooze(&queue, "cal:evt_1", ReminderKind::Upcoming, -1);
        let (_, fired) = recompute(&queue, &inputs(&events, &[], &settings, false));
        assert_eq!(fired.len(), 1, "the snooze elapsed, so it comes back once");
    }

    #[test]
    fn a_dismissal_is_final_for_that_kind_and_leaves_the_others_alone() {
        let settings = settings_all_on();
        let events = vec![event_starting_in("evt_1", 120)];
        let queue = ReminderQueue::default();
        recompute(&queue, &inputs(&events, &[], &settings, false));

        dismiss(&queue, "cal:evt_1", ReminderKind::Upcoming);
        let (all, fired) = recompute(&queue, &inputs(&events, &[], &settings, false));
        assert!(fired.is_empty());
        assert!(matches!(all[0].status, ReminderStatus::Dismissed));
    }

    #[test]
    fn starting_a_recording_resolves_every_reminder_for_that_meeting() {
        let settings = settings_all_on();
        let events = vec![event_starting_in("evt_1", 120)];
        let queue = ReminderQueue::default();
        recompute(&queue, &inputs(&events, &[], &settings, false));

        mark_actioned(&queue, "cal:evt_1");
        let entries = queue.entries.lock_or_recover();
        assert!(entries
            .iter()
            .filter(|e| e.key == "cal:evt_1")
            .all(|e| matches!(e.status, ReminderStatus::Actioned)));
    }

    #[test]
    fn each_reminder_kind_can_be_switched_off_on_its_own() {
        let settings = MeetingSettings {
            remind_before_meeting: false,
            remind_if_unrecorded: true,
            remind_on_detection: true,
            ..MeetingSettings::default()
        };
        let events = vec![event_starting_in("evt_1", 120)];
        let queue = ReminderQueue::default();

        let (all, _) = recompute(&queue, &inputs(&events, &[], &settings, false));
        assert!(all.is_empty());
    }

    #[test]
    fn the_popup_shows_the_earliest_fired_reminder() {
        let settings = settings_all_on();
        let events = vec![event_starting_in("evt_1", 60), event_starting_in("evt_2", 90)];
        let queue = ReminderQueue::default();
        recompute(&queue, &inputs(&events, &[], &settings, false));

        assert!(current(&queue).is_some());
        dismiss(&queue, "cal:evt_1", ReminderKind::Upcoming);
        dismiss(&queue, "cal:evt_2", ReminderKind::Upcoming);
        assert!(current(&queue).is_none());
    }

    #[test]
    fn a_meeting_that_was_already_recorded_is_not_called_unrecorded() {
        // The user recorded this meeting and stopped early. Nothing is
        // recording *now*, but the meeting did not go uncaptured — and saying
        // it did is the false positive that gets reminders switched off.
        let settings = settings_all_on();
        let mut event = event_starting_in("evt_1", -330);
        event.conference_url = Some("https://zoom.us/j/123".to_string());
        let events = vec![event];
        let windows = vec![zoom_window("Sprint planning")];
        let finished = vec![session_started_at(
            MeetingState::Completed,
            Utc::now() - Duration::seconds(330),
        )];

        let queue = ReminderQueue::default();
        let (all, _) = recompute(
            &queue,
            &ReminderInputs {
                events: &events,
                windows: &windows,
                settings: &settings,
                is_recording: false,
                sessions: &finished,
                now: Utc::now(),
            },
        );
        assert!(all.iter().all(|e| e.kind != ReminderKind::Unrecorded));
    }

    #[test]
    fn a_recording_of_some_other_meeting_does_not_cover_this_one() {
        let settings = settings_all_on();
        let mut event = event_starting_in("evt_1", -330);
        event.conference_url = Some("https://zoom.us/j/123".to_string());
        let events = vec![event];
        let windows = vec![zoom_window("Sprint planning")];
        // Recorded three hours ago — a different meeting entirely.
        let unrelated = vec![session_started_at(
            MeetingState::Completed,
            Utc::now() - Duration::hours(3),
        )];

        let queue = ReminderQueue::default();
        let (all, _) = recompute(
            &queue,
            &ReminderInputs {
                events: &events,
                windows: &windows,
                settings: &settings,
                is_recording: false,
                sessions: &unrelated,
                now: Utc::now(),
            },
        );
        assert!(all.iter().any(|e| e.kind == ReminderKind::Unrecorded));
    }

    #[test]
    fn two_reminders_due_at_once_are_both_shown_in_turn() {
        // One card is on screen at a time. The second must wait, not vanish —
        // this is the failure the single-slot design was known for.
        let settings = settings_all_on();
        let events = vec![event_starting_in("evt_1", 60), event_starting_in("evt_2", 90)];
        let queue = ReminderQueue::default();
        let (_, newly_fired) = recompute(&queue, &inputs(&events, &[], &settings, false));

        let mut backlog = VecDeque::new();
        enqueue_to_show(&mut backlog, newly_fired);

        let first = take_next_to_show(&queue, &mut backlog).expect("a card to show");
        let second = take_next_to_show(&queue, &mut backlog).expect("the second still waiting");
        assert_ne!(first.key, second.key);
        assert!(take_next_to_show(&queue, &mut backlog).is_none());
    }

    #[test]
    fn a_reminder_answered_while_another_was_on_screen_is_not_shown_late() {
        let settings = settings_all_on();
        let events = vec![event_starting_in("evt_1", 60), event_starting_in("evt_2", 90)];
        let queue = ReminderQueue::default();
        let (_, newly_fired) = recompute(&queue, &inputs(&events, &[], &settings, false));

        let mut backlog = VecDeque::new();
        enqueue_to_show(&mut backlog, newly_fired);
        take_next_to_show(&queue, &mut backlog).expect("the first card");

        // The user starts recording the second meeting from the Meetings list
        // while the first card is still up.
        mark_actioned(&queue, "cal:evt_2");
        assert!(take_next_to_show(&queue, &mut backlog).is_none());
    }

    #[test]
    fn the_same_reminder_never_waits_in_the_backlog_twice() {
        let settings = settings_all_on();
        let events = vec![event_starting_in("evt_1", 60)];
        let queue = ReminderQueue::default();
        let (_, newly_fired) = recompute(&queue, &inputs(&events, &[], &settings, false));

        let mut backlog = VecDeque::new();
        enqueue_to_show(&mut backlog, newly_fired.clone());
        enqueue_to_show(&mut backlog, newly_fired);
        assert_eq!(backlog.len(), 1);
    }

    #[test]
    fn a_finished_meeting_is_not_reminded_about() {
        let settings = settings_all_on();
        let mut event = event_starting_in("evt_1", -3600);
        event.ends_at = (Utc::now() - Duration::minutes(30)).to_rfc3339();
        let queue = ReminderQueue::default();

        let (all, _) = recompute(&queue, &inputs(&[event], &[], &settings, false));
        assert!(all.is_empty());
    }
}
