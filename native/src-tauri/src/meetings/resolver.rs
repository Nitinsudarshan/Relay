use crate::meetings::{CalendarMeetingEvent, WindowMatch};
use crate::vault::{Meeting, VaultManager, MEETING_STATUS_CANCELLED, MEETING_STATUS_COMPLETED};
use chrono::{DateTime, Utc};
use std::collections::HashMap;
use std::sync::Mutex;

/// A signal describing a possible meeting, from either source Relay
/// currently listens to. Every signal goes through `resolve_calendar_signal`
/// or `resolve_window_signal` — nothing downstream (the reminder engine,
/// the popup, the meetings list) constructs a `Meeting` directly, which is
/// what keeps calendar and window signals for the same real-world meeting
/// from ever producing two separate records.
#[derive(Debug, Clone)]
pub enum MeetingSignal {
    Calendar(CalendarMeetingEvent),
    WindowDetected(WindowMatch),
}

/// The outcome of resolving a signal: either a real, already-persisted
/// meeting, or a candidate that hasn't earned a vault record yet.
#[derive(Debug, Clone)]
pub enum ResolvedMeeting {
    Persisted(Meeting),
    Candidate(CandidateMeeting),
}

#[derive(Debug, Clone)]
pub struct CandidateMeeting {
    pub provider: String,
    pub title: String,
    pub detection_key: String,
    pub confidence: f32,
    pub first_seen_at: DateTime<Utc>,
    pub hits: u32,
}

/// In-memory holding area for window-detected signals that haven't
/// graduated to a persisted `Meeting` yet. Tauri-managed state, the same
/// pattern the removed `scheduler.rs` used for `ReminderMap`/
/// `ActiveReminderState`.
#[derive(Default)]
pub struct CandidateStore(pub Mutex<HashMap<String, CandidateMeeting>>);

/// A generic title (see `detection::score_confidence`) needs to be seen
/// this many times before it's trusted as a real meeting rather than a
/// stray window; a specific one graduates immediately.
const SUSTAINED_HITS_REQUIRED: u32 = 2;
const HIGH_CONFIDENCE_THRESHOLD: f32 = 0.8;

/// How close (in minutes) a window-detection signal's timing must be to an
/// existing meeting's scheduled/actual window to count as corroborating it,
/// rather than describing a distinct, unrelated meeting.
const TEMPORAL_PROXIMITY_MINUTES: i64 = 10;

fn normalize_title(title: &str) -> String {
    title.trim().to_lowercase()
}

/// Deliberately simple containment check, matching the heuristic already
/// used elsewhere in this codebase (the removed `scheduler.rs`) rather than
/// introducing a fuzzy-matching dependency for a comparison this coarse.
/// Known limitation, accepted per `meetings_implementation.md` §4.1: two
/// distinct meetings with near-identical titles can still be conflated.
fn titles_are_similar(a: &str, b: &str) -> bool {
    let (a, b) = (normalize_title(a), normalize_title(b));
    if a.is_empty() || b.is_empty() {
        return false;
    }
    a == b || a.contains(&b) || b.contains(&a)
}

pub(crate) fn is_open(meeting: &Meeting) -> bool {
    meeting.status != MEETING_STATUS_CANCELLED && meeting.status != MEETING_STATUS_COMPLETED
}

pub(crate) fn parse_rfc3339(value: &str) -> Option<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(value)
        .ok()
        .map(|d| d.with_timezone(&Utc))
}

/// Resolves a calendar signal onto a persisted `Meeting`, updating an
/// existing record if one already links to this event and creating one
/// only otherwise. Calendar events are always persisted immediately
/// regardless of confidence — a scheduled event is already real, explicit
/// evidence, independent of whether the user actually attends.
pub fn resolve_calendar_signal(
    vault: &VaultManager,
    event: &CalendarMeetingEvent,
) -> Result<Meeting, String> {
    // Tier 1: exact calendar_event_id match.
    if let Some(existing) = vault
        .find_meeting_by_calendar_event_id(&event.id)
        .map_err(|e| e.to_string())?
    {
        return update_from_calendar_event(vault, existing, event);
    }

    // Tier 3: time + title fallback, only reached if the event carries no
    // usable ID at all — a defensive path, since Google's API always
    // returns one; tier 2 (matching by calendar_series_id) is handled by
    // tier 1 already covering "same occurrence," so a genuinely new
    // occurrence of a known series still correctly creates a fresh record
    // below rather than overwriting the previous occurrence.
    if event.id.trim().is_empty() {
        if let Some(existing) =
            find_meeting_by_scheduled_time_and_title(vault, &event.provider, &event.title, &event.scheduled_start)?
        {
            return update_from_calendar_event(vault, existing, event);
        }
    }

    let meeting = Meeting::from_calendar_event(event);
    vault.save_meeting(&meeting).map_err(|e| e.to_string())?;
    Ok(meeting)
}

fn update_from_calendar_event(
    vault: &VaultManager,
    mut existing: Meeting,
    event: &CalendarMeetingEvent,
) -> Result<Meeting, String> {
    existing.title = event.title.clone();
    existing.calendar_series_id = event.calendar_series_id.clone().or(existing.calendar_series_id);
    existing.scheduled_start = Some(event.scheduled_start.clone());
    existing.scheduled_end = Some(event.scheduled_end.clone());
    existing.participants = event.participants.clone();
    if let Some(url) = &event.meeting_url {
        existing.provider_metadata = serde_json::json!({ "meeting_url": url });
    }
    vault.update_meeting(&existing).map_err(|e| e.to_string())
}

fn find_meeting_by_scheduled_time_and_title(
    vault: &VaultManager,
    provider: &str,
    title: &str,
    scheduled_start: &str,
) -> Result<Option<Meeting>, String> {
    let target = parse_rfc3339(scheduled_start);
    let meetings = vault.list_meetings().map_err(|e| e.to_string())?;
    Ok(meetings.into_iter().find(|m| {
        is_open(m)
            && m.provider == provider
            && titles_are_similar(&m.title, title)
            && match (target, m.scheduled_start.as_deref().and_then(parse_rfc3339)) {
                (Some(a), Some(b)) => (a - b).num_minutes().abs() <= TEMPORAL_PROXIMITY_MINUTES,
                _ => false,
            }
    }))
}

/// Resolves a window-detection signal. Three possible outcomes: it
/// corroborates an existing `Meeting` (calendar-sourced, or already
/// confirmed-detected) and updates it in place; it's tracked as an
/// in-memory candidate that hasn't earned a vault record yet; or a tracked
/// candidate has now been seen confidently/consistently enough to graduate
/// into a persisted `Meeting`.
pub fn resolve_window_signal(
    vault: &VaultManager,
    candidates: &CandidateStore,
    signal: &WindowMatch,
) -> Result<ResolvedMeeting, String> {
    // Tier 1: a meeting URL extracted from the signal, correlated against a
    // calendar-sourced meeting's own stored `meeting_url`. In practice this
    // rarely has anything to compare, since OS window titles don't usually
    // carry the meeting code (see meetings_implementation.md §4.1) — this
    // exists so it's ready the day a richer signal exists, not because it's
    // expected to fire often today.
    let (_, extracted_url) = crate::meetings::identify_meeting_provider(&signal.raw_title);
    if let Some(url) = extracted_url {
        if let Some(existing) = find_meeting_by_meeting_url(vault, &url)? {
            return corroborate(vault, existing).map(ResolvedMeeting::Persisted);
        }
    }

    // Tier 2: provider + temporal proximity + title similarity against an
    // existing meeting. A match here corroborates that meeting — it never
    // creates a second one for the same real-world event.
    if let Some(existing) = find_meeting_by_proximity_and_title(vault, &signal.provider, &signal.title)? {
        return corroborate(vault, existing).map(ResolvedMeeting::Persisted);
    }

    // A previously graduated candidate can resolve again on a later tick
    // (the reminder engine re-resolving signals) — recognize it by its
    // detection_key rather than creating a duplicate or re-candidating it.
    let detection_key = build_detection_key(&signal.provider, &signal.title);
    if let Some(existing) = vault
        .find_meeting_by_detection_key(&detection_key)
        .map_err(|e| e.to_string())?
    {
        return Ok(ResolvedMeeting::Persisted(existing));
    }

    // Tier 3/4: genuinely new — track as a candidate, graduating once it's
    // earned a vault record.
    let mut store = candidates.0.lock().unwrap();
    let now = Utc::now();
    let entry = store.entry(detection_key.clone()).or_insert_with(|| CandidateMeeting {
        provider: signal.provider.clone(),
        title: signal.title.clone(),
        detection_key: detection_key.clone(),
        confidence: signal.confidence,
        first_seen_at: now,
        hits: 0,
    });
    entry.hits += 1;
    entry.confidence = entry.confidence.max(signal.confidence);

    let should_graduate = entry.confidence >= HIGH_CONFIDENCE_THRESHOLD || entry.hits >= SUSTAINED_HITS_REQUIRED;

    if should_graduate {
        let candidate = entry.clone();
        store.remove(&detection_key);
        drop(store);
        let meeting = Meeting::from_window_detection(
            &candidate.provider,
            &candidate.title,
            &candidate.detection_key,
            candidate.confidence,
        );
        vault.save_meeting(&meeting).map_err(|e| e.to_string())?;
        return Ok(ResolvedMeeting::Persisted(meeting));
    }

    Ok(ResolvedMeeting::Candidate(entry.clone()))
}

fn corroborate(vault: &VaultManager, mut existing: Meeting) -> Result<Meeting, String> {
    if existing.actual_start.is_none() {
        existing.actual_start = Some(Utc::now().to_rfc3339());
    }
    vault.update_meeting(&existing).map_err(|e| e.to_string())
}

fn build_detection_key(provider: &str, title: &str) -> String {
    format!("{}:{}:{}", provider, normalize_title(title), Utc::now().format("%Y-%m-%d"))
}

fn find_meeting_by_meeting_url(vault: &VaultManager, url: &str) -> Result<Option<Meeting>, String> {
    let meetings = vault.list_meetings().map_err(|e| e.to_string())?;
    Ok(meetings.into_iter().find(|m| {
        is_open(m) && m.provider_metadata.get("meeting_url").and_then(|v| v.as_str()) == Some(url)
    }))
}

fn find_meeting_by_proximity_and_title(
    vault: &VaultManager,
    provider: &str,
    title: &str,
) -> Result<Option<Meeting>, String> {
    let now = Utc::now();
    let near = |t: &Option<String>| {
        t.as_deref()
            .and_then(parse_rfc3339)
            .map(|d| (now - d).num_minutes().abs() <= TEMPORAL_PROXIMITY_MINUTES)
            .unwrap_or(false)
    };
    let meetings = vault.list_meetings().map_err(|e| e.to_string())?;
    Ok(meetings.into_iter().find(|m| {
        is_open(m)
            && m.provider == provider
            && titles_are_similar(&m.title, title)
            && (near(&m.scheduled_start) || near(&m.scheduled_end) || near(&m.actual_start))
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::meetings::PROVIDER_ZOOM;

    fn temp_vault() -> VaultManager {
        let dir = std::env::temp_dir().join(format!("relay_test_resolver_{}", uuid::Uuid::new_v4()));
        VaultManager::new(dir)
    }

    fn sample_event(id: &str, title: &str, minutes_from_now: i64) -> CalendarMeetingEvent {
        let start = Utc::now() + chrono::Duration::minutes(minutes_from_now);
        CalendarMeetingEvent {
            id: id.to_string(),
            title: title.to_string(),
            provider: PROVIDER_ZOOM.to_string(),
            meeting_url: None,
            scheduled_start: start.to_rfc3339(),
            scheduled_end: (start + chrono::Duration::minutes(30)).to_rfc3339(),
            participants: vec![],
            recurrence_rule: None,
            calendar_series_id: None,
        }
    }

    fn generic_signal(title: &str) -> WindowMatch {
        WindowMatch {
            provider: PROVIDER_ZOOM.to_string(),
            title: title.to_string(),
            raw_title: title.to_string(),
            source: "window_detector".to_string(),
            confidence: 0.55,
        }
    }

    fn specific_signal(title: &str) -> WindowMatch {
        WindowMatch {
            provider: PROVIDER_ZOOM.to_string(),
            title: title.to_string(),
            raw_title: title.to_string(),
            source: "window_detector".to_string(),
            confidence: 0.85,
        }
    }

    #[test]
    fn test_calendar_signal_is_idempotent() {
        let vault = temp_vault();
        let event = sample_event("gcal_1", "Weekly Sync", 5);

        let first = resolve_calendar_signal(&vault, &event).unwrap();
        let second = resolve_calendar_signal(&vault, &event).unwrap();

        assert_eq!(first.id, second.id);
        assert_eq!(vault.list_meetings().unwrap().len(), 1);
    }

    #[test]
    fn test_calendar_and_window_signals_reconcile_to_one_meeting() {
        let vault = temp_vault();
        let event = sample_event("gcal_2", "Design Review", 0);
        let calendar_meeting = resolve_calendar_signal(&vault, &event).unwrap();
        assert!(calendar_meeting.actual_start.is_none());

        let candidates = CandidateStore::default();
        let signal = specific_signal("Design Review");
        let resolved = resolve_window_signal(&vault, &candidates, &signal).unwrap();

        match resolved {
            ResolvedMeeting::Persisted(m) => {
                assert_eq!(m.id, calendar_meeting.id);
                assert!(m.actual_start.is_some());
            }
            ResolvedMeeting::Candidate(_) => panic!("expected the window signal to corroborate the calendar meeting"),
        }
        assert_eq!(vault.list_meetings().unwrap().len(), 1);
    }

    #[test]
    fn test_generic_title_needs_sustained_hits() {
        let vault = temp_vault();
        let candidates = CandidateStore::default();
        let signal = generic_signal("Zoom Meeting");

        let first = resolve_window_signal(&vault, &candidates, &signal).unwrap();
        assert!(matches!(first, ResolvedMeeting::Candidate(_)));
        assert_eq!(vault.list_meetings().unwrap().len(), 0);

        let second = resolve_window_signal(&vault, &candidates, &signal).unwrap();
        assert!(matches!(second, ResolvedMeeting::Persisted(_)));
        assert_eq!(vault.list_meetings().unwrap().len(), 1);
    }

    #[test]
    fn test_specific_title_graduates_immediately() {
        let vault = temp_vault();
        let candidates = CandidateStore::default();
        let signal = specific_signal("Sprint Architecture Planning");

        let resolved = resolve_window_signal(&vault, &candidates, &signal).unwrap();
        assert!(matches!(resolved, ResolvedMeeting::Persisted(_)));
        assert_eq!(vault.list_meetings().unwrap().len(), 1);
    }

    #[test]
    fn test_unrelated_candidates_do_not_merge() {
        let vault = temp_vault();
        let candidates = CandidateStore::default();

        resolve_window_signal(&vault, &candidates, &generic_signal("Zoom Meeting")).unwrap();
        let other = generic_signal("Teams Meeting");
        let mut other = other;
        other.provider = "teams".to_string();
        resolve_window_signal(&vault, &candidates, &other).unwrap();

        assert_eq!(candidates.0.lock().unwrap().len(), 2);
    }
}
