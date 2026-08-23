use crate::meetings::{CalendarMeetingEvent, WindowMatch};
use crate::vault::{Meeting, VaultManager, MEETING_STATUS_CANCELLED, MEETING_STATUS_COMPLETED};
use chrono::{DateTime, Duration, Utc};
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

/// The outcome of resolving a signal. `Created` vs `Updated` is what lets
/// the engine emit a `meeting-updated` event only when there's genuinely
/// something new for an open meetings list to show, instead of re-emitting
/// the same record every 15-second tick.
#[derive(Debug, Clone)]
pub enum ResolvedMeeting {
    Created(Meeting),
    Updated(Meeting),
    Candidate(CandidateMeeting),
}

impl ResolvedMeeting {
    pub fn meeting(&self) -> Option<&Meeting> {
        match self {
            ResolvedMeeting::Created(m) | ResolvedMeeting::Updated(m) => Some(m),
            ResolvedMeeting::Candidate(_) => None,
        }
    }

    pub fn was_created(&self) -> bool {
        matches!(self, ResolvedMeeting::Created(_))
    }
}

#[derive(Debug, Clone)]
pub struct CandidateMeeting {
    pub provider: String,
    pub title: String,
    pub detection_key: String,
    pub confidence: f32,
    pub first_seen_at: DateTime<Utc>,
    pub last_seen_at: DateTime<Utc>,
    pub hits: u32,
}

/// In-memory holding area for window-detected signals that haven't
/// graduated to a persisted `Meeting` yet. Tauri-managed state, the same
/// pattern the removed `scheduler.rs` used for its own reminder state.
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

/// How long a persisted window-detected meeting stays eligible to be
/// re-matched by its `detection_key`. This is what keeps the key a
/// *short-lived dedup fingerprint* rather than an identity: a live meeting
/// is re-corroborated on every tick so it stays fresh indefinitely, but a
/// meeting nobody has seen for this long stops absorbing new signals —
/// so a second, unrelated "Zoom Meeting" hours later becomes its own
/// record instead of silently merging into the morning's call.
const DETECTION_FRESHNESS_MINUTES: i64 = 20;

/// A tracked candidate that stops being seen is dropped after this long,
/// so a one-off glimpse of a stray window doesn't occupy memory forever.
const CANDIDATE_TTL_MINUTES: i64 = 30;

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
///
/// Callers must hold `RESOLVE_LOCK` (via [`resolve_lock`]) so the
/// check-then-create below is atomic against a concurrent caller; the
/// background engine and the manual "Import and Prepare Meeting" command
/// can otherwise both find no existing record and both create one.
pub fn resolve_calendar_signal(
    vault: &VaultManager,
    event: &CalendarMeetingEvent,
) -> Result<ResolvedMeeting, String> {
    let _guard = resolve_lock().lock().unwrap();
    let meetings = vault.list_meetings().map_err(|e| e.to_string())?;

    // Tier 1: exact calendar_event_id match.
    if let Some(existing) = meetings
        .iter()
        .find(|m| m.calendar_event_id.as_deref() == Some(event.id.as_str()))
    {
        return update_from_calendar_event(vault, existing.clone(), event).map(ResolvedMeeting::Updated);
    }

    // Tier 3: time + title fallback, only reached if the event carries no
    // usable ID at all — a defensive path, since Google's API always
    // returns one. Tier 2 (matching a recurring series by
    // `calendar_series_id`) is deliberately absent: `singleEvents=true` in
    // `calendar.rs` expands every occurrence into its own event with its
    // own unique id, so tier 1 already matches the right occurrence, and
    // matching by series would wrongly fold a *new* occurrence into the
    // previous one's record.
    if event.id.trim().is_empty() {
        if let Some(existing) =
            find_meeting_by_scheduled_time_and_title(&meetings, &event.provider, &event.title, &event.scheduled_start)
        {
            return update_from_calendar_event(vault, existing.clone(), event).map(ResolvedMeeting::Updated);
        }
    }

    let meeting = Meeting::from_calendar_event(event);
    vault.save_meeting(&meeting).map_err(|e| e.to_string())?;
    Ok(ResolvedMeeting::Created(meeting))
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

fn find_meeting_by_scheduled_time_and_title<'a>(
    meetings: &'a [Meeting],
    provider: &str,
    title: &str,
    scheduled_start: &str,
) -> Option<&'a Meeting> {
    let target = parse_rfc3339(scheduled_start);
    meetings.iter().find(|m| {
        is_open(m)
            && m.provider == provider
            && titles_are_similar(&m.title, title)
            && match (target, m.scheduled_start.as_deref().and_then(parse_rfc3339)) {
                (Some(a), Some(b)) => (a - b).num_minutes().abs() <= TEMPORAL_PROXIMITY_MINUTES,
                _ => false,
            }
    })
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
    let _guard = resolve_lock().lock().unwrap();
    let now = Utc::now();
    let meetings = vault.list_meetings().map_err(|e| e.to_string())?;

    // Tier 1: a meeting URL extracted from the signal, correlated against a
    // calendar-sourced meeting's own stored `meeting_url`. In practice this
    // rarely has anything to compare, since OS window titles don't usually
    // carry the meeting code (see meetings_implementation.md §4.1) — this
    // exists so it's ready the day a richer signal exists, not because it's
    // expected to fire often today.
    let (_, extracted_url) = crate::meetings::identify_meeting_provider(&signal.raw_title);
    if let Some(url) = extracted_url {
        if let Some(existing) = find_meeting_by_meeting_url(&meetings, &url) {
            return corroborate(vault, existing.clone()).map(ResolvedMeeting::Updated);
        }
    }

    // Tier 2: provider + temporal proximity + title similarity against an
    // existing meeting. A match here corroborates that meeting — it never
    // creates a second one for the same real-world event.
    if let Some(existing) = find_meeting_by_proximity_and_title(&meetings, &signal.provider, &signal.title, now) {
        return corroborate(vault, existing.clone()).map(ResolvedMeeting::Updated);
    }

    // A previously graduated candidate resolving again on a later tick —
    // recognized by its detection fingerprint rather than creating a
    // duplicate. Restricted to *open* meetings that are still being
    // actively seen (`DETECTION_FRESHNESS_MINUTES`), so this stays a
    // short-lived dedup mechanism and never becomes identity: a completed
    // meeting, or one nobody has seen for a while, no longer absorbs new
    // signals that merely share a provider and a generic title.
    let detection_key = build_detection_key(&signal.provider, &signal.title, now);
    if let Some(existing) = find_fresh_meeting_by_detection_key(&meetings, &detection_key, now) {
        // Corroborated so `updated_at` keeps advancing — that's what keeps
        // a genuinely long-running meeting matching here tick after tick.
        return corroborate(vault, existing.clone()).map(ResolvedMeeting::Updated);
    }

    // Tier 3/4: genuinely new — track as a candidate, graduating once it's
    // earned a vault record.
    let mut store = candidates.0.lock().unwrap();
    prune_stale_candidates(&mut store, now);

    let entry = store.entry(detection_key.clone()).or_insert_with(|| CandidateMeeting {
        provider: signal.provider.clone(),
        title: signal.title.clone(),
        detection_key: detection_key.clone(),
        confidence: signal.confidence,
        first_seen_at: now,
        last_seen_at: now,
        hits: 0,
    });
    entry.hits += 1;
    entry.last_seen_at = now;
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
        return Ok(ResolvedMeeting::Created(meeting));
    }

    Ok(ResolvedMeeting::Candidate(entry.clone()))
}

/// Serializes the check-then-create inside both resolve functions. Without
/// it, the background engine's tick and a user-triggered
/// `import_calendar_event` can interleave between "no existing meeting
/// found" and "save the new one", each creating a record for the same
/// calendar event.
fn resolve_lock() -> &'static Mutex<()> {
    static RESOLVE_LOCK: Mutex<()> = Mutex::new(());
    &RESOLVE_LOCK
}

fn prune_stale_candidates(store: &mut HashMap<String, CandidateMeeting>, now: DateTime<Utc>) {
    store.retain(|_, c| (now - c.last_seen_at) <= Duration::minutes(CANDIDATE_TTL_MINUTES));
}

fn corroborate(vault: &VaultManager, mut existing: Meeting) -> Result<Meeting, String> {
    if existing.actual_start.is_none() {
        existing.actual_start = Some(Utc::now().to_rfc3339());
    }
    vault.update_meeting(&existing).map_err(|e| e.to_string())
}

fn build_detection_key(provider: &str, title: &str, now: DateTime<Utc>) -> String {
    format!("{}:{}:{}", provider, normalize_title(title), now.format("%Y-%m-%d"))
}

fn find_fresh_meeting_by_detection_key<'a>(
    meetings: &'a [Meeting],
    detection_key: &str,
    now: DateTime<Utc>,
) -> Option<&'a Meeting> {
    meetings.iter().find(|m| {
        is_open(m)
            && m.detection_key.as_deref() == Some(detection_key)
            && parse_rfc3339(&m.updated_at)
                .map(|seen| (now - seen).num_minutes() <= DETECTION_FRESHNESS_MINUTES)
                .unwrap_or(false)
    })
}

fn find_meeting_by_meeting_url<'a>(meetings: &'a [Meeting], url: &str) -> Option<&'a Meeting> {
    meetings.iter().find(|m| {
        is_open(m) && m.provider_metadata.get("meeting_url").and_then(|v| v.as_str()) == Some(url)
    })
}

fn find_meeting_by_proximity_and_title<'a>(
    meetings: &'a [Meeting],
    provider: &str,
    title: &str,
    now: DateTime<Utc>,
) -> Option<&'a Meeting> {
    let near = |t: &Option<String>| {
        t.as_deref()
            .and_then(parse_rfc3339)
            .map(|d| (now - d).num_minutes().abs() <= TEMPORAL_PROXIMITY_MINUTES)
            .unwrap_or(false)
    };
    meetings.iter().find(|m| {
        is_open(m)
            && m.provider == provider
            && titles_are_similar(&m.title, title)
            && (near(&m.scheduled_start) || near(&m.scheduled_end) || near(&m.actual_start))
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::meetings::PROVIDER_ZOOM;
    use crate::vault::MEETING_STATUS_COMPLETED;

    fn temp_vault() -> VaultManager {
        let dir = std::env::temp_dir().join(format!("relay_test_resolver_{}", uuid::Uuid::new_v4()));
        VaultManager::new(dir)
    }

    fn sample_event(id: &str, title: &str, minutes_from_now: i64) -> CalendarMeetingEvent {
        let start = Utc::now() + Duration::minutes(minutes_from_now);
        CalendarMeetingEvent {
            id: id.to_string(),
            title: title.to_string(),
            provider: PROVIDER_ZOOM.to_string(),
            meeting_url: None,
            scheduled_start: start.to_rfc3339(),
            scheduled_end: (start + Duration::minutes(30)).to_rfc3339(),
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

        assert!(first.was_created());
        assert!(!second.was_created());
        assert_eq!(first.meeting().unwrap().id, second.meeting().unwrap().id);
        assert_eq!(vault.list_meetings().unwrap().len(), 1);
    }

    #[test]
    fn test_calendar_and_window_signals_reconcile_to_one_meeting() {
        let vault = temp_vault();
        let event = sample_event("gcal_2", "Design Review", 0);
        let calendar_meeting = resolve_calendar_signal(&vault, &event).unwrap();
        let calendar_id = calendar_meeting.meeting().unwrap().id.clone();
        assert!(calendar_meeting.meeting().unwrap().actual_start.is_none());

        let candidates = CandidateStore::default();
        let resolved = resolve_window_signal(&vault, &candidates, &specific_signal("Design Review")).unwrap();

        let m = resolved.meeting().expect("window signal should corroborate the calendar meeting");
        assert_eq!(m.id, calendar_id);
        assert!(m.actual_start.is_some());
        assert!(!resolved.was_created());
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
        assert!(second.was_created());
        assert_eq!(vault.list_meetings().unwrap().len(), 1);
    }

    #[test]
    fn test_specific_title_graduates_immediately() {
        let vault = temp_vault();
        let candidates = CandidateStore::default();

        let resolved = resolve_window_signal(&vault, &candidates, &specific_signal("Sprint Architecture Planning")).unwrap();
        assert!(resolved.was_created());
        assert_eq!(vault.list_meetings().unwrap().len(), 1);
    }

    #[test]
    fn test_unrelated_candidates_do_not_merge() {
        let vault = temp_vault();
        let candidates = CandidateStore::default();

        resolve_window_signal(&vault, &candidates, &generic_signal("Zoom Meeting")).unwrap();
        let mut other = generic_signal("Teams Meeting");
        other.provider = "teams".to_string();
        resolve_window_signal(&vault, &candidates, &other).unwrap();

        assert_eq!(candidates.0.lock().unwrap().len(), 2);
    }

    /// The collision `meetings_implementation.md` §0.1 item 1 exists to
    /// prevent: two genuinely distinct same-day meetings sharing a
    /// provider and a generic title must not merge into one record just
    /// because their `detection_key` matches.
    #[test]
    fn test_completed_same_day_meeting_does_not_absorb_a_later_signal() {
        let vault = temp_vault();
        let candidates = CandidateStore::default();

        // Morning call graduates, then finishes.
        let morning = resolve_window_signal(&vault, &candidates, &specific_signal("Zoom Meeting"))
            .unwrap()
            .meeting()
            .unwrap()
            .clone();
        let mut done = morning.clone();
        done.status = MEETING_STATUS_COMPLETED.to_string();
        vault.save_meeting(&done).unwrap();

        // A separate afternoon call with the identical generic title.
        let afternoon = resolve_window_signal(&vault, &candidates, &specific_signal("Zoom Meeting")).unwrap();

        assert!(afternoon.was_created(), "a completed meeting must not absorb a new signal");
        assert_ne!(afternoon.meeting().unwrap().id, morning.id);
        assert_eq!(vault.list_meetings().unwrap().len(), 2);
    }

    /// A meeting nobody has seen for longer than the freshness window also
    /// stops absorbing new signals, even while still technically open.
    #[test]
    fn test_stale_open_meeting_does_not_absorb_a_later_signal() {
        let vault = temp_vault();
        let candidates = CandidateStore::default();

        let first = resolve_window_signal(&vault, &candidates, &specific_signal("Zoom Meeting"))
            .unwrap()
            .meeting()
            .unwrap()
            .clone();

        // Backdate both the record's freshness and its timing so neither
        // the freshness check nor tier-2 proximity can match it.
        let long_ago = (Utc::now() - Duration::hours(5)).to_rfc3339();
        let mut stale = first.clone();
        stale.updated_at = long_ago.clone();
        stale.scheduled_start = Some(long_ago.clone());
        stale.scheduled_end = Some(long_ago.clone());
        stale.actual_start = Some(long_ago);
        vault.save_meeting(&stale).unwrap();

        let later = resolve_window_signal(&vault, &candidates, &specific_signal("Zoom Meeting")).unwrap();
        assert!(later.was_created(), "a stale meeting must not absorb a new signal");
        assert_ne!(later.meeting().unwrap().id, first.id);
    }

    /// A live, still-being-seen meeting must keep matching tick after tick
    /// — the freshness window must not cause duplicate records for one
    /// long-running call.
    #[test]
    fn test_live_meeting_keeps_matching_across_ticks() {
        let vault = temp_vault();
        let candidates = CandidateStore::default();

        let first = resolve_window_signal(&vault, &candidates, &specific_signal("Zoom Meeting")).unwrap();
        let id = first.meeting().unwrap().id.clone();

        for _ in 0..3 {
            let again = resolve_window_signal(&vault, &candidates, &specific_signal("Zoom Meeting")).unwrap();
            assert!(!again.was_created());
            assert_eq!(again.meeting().unwrap().id, id);
        }
        assert_eq!(vault.list_meetings().unwrap().len(), 1);
    }

    #[test]
    fn test_stale_candidates_are_pruned() {
        let mut store: HashMap<String, CandidateMeeting> = HashMap::new();
        let now = Utc::now();
        store.insert(
            "stale".to_string(),
            CandidateMeeting {
                provider: PROVIDER_ZOOM.to_string(),
                title: "Zoom Meeting".to_string(),
                detection_key: "stale".to_string(),
                confidence: 0.55,
                first_seen_at: now - Duration::hours(2),
                last_seen_at: now - Duration::hours(2),
                hits: 1,
            },
        );
        store.insert(
            "fresh".to_string(),
            CandidateMeeting {
                provider: PROVIDER_ZOOM.to_string(),
                title: "Zoom Meeting".to_string(),
                detection_key: "fresh".to_string(),
                confidence: 0.55,
                first_seen_at: now,
                last_seen_at: now,
                hits: 1,
            },
        );

        prune_stale_candidates(&mut store, now);
        assert!(store.contains_key("fresh"));
        assert!(!store.contains_key("stale"));
    }
}
