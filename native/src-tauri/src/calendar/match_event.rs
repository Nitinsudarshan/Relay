//! Deciding which calendar event a recording was.
//!
//! The naive rule — "the event whose window contains the start time" — is wrong
//! often enough to matter. Recordings start late because somebody had to find
//! the button. Meetings run over. Two events overlap because one is a
//! declined all-hands nobody left. And a recording made at four in the
//! afternoon with nothing in the calendar must match *nothing*, not the nearest
//! thing available.
//!
//! So matching scores candidates on overlap and rejects everything below a bar.
//! The bar exists because a wrong match is worse than no match: it retitles the
//! meeting, populates a participant list with people who were not there, and
//! feeds somebody else's agenda to the summarizer as this meeting's intent.
//! Where two events fit comparably well, Relay picks neither and says so.

use super::model::CalendarEvent;
use serde::{Deserialize, Serialize};

/// Share of the recording that must fall inside an event before it can match.
///
/// Half, which tolerates a recording started late or stopped early while
/// rejecting a meeting that merely happened to be adjacent.
const MIN_OVERLAP_OF_RECORDING: f64 = 0.5;

/// How much better the best candidate must fit than the next.
///
/// Two overlapping invitations are common — a team stand-up inside a longer
/// block, an all-hands nobody declined. Picking the marginally better one would
/// be a guess, and a wrong retitle is the visible, annoying kind of wrong.
const MIN_MARGIN_OVER_RUNNER_UP: f64 = 0.2;

/// A recording matched to an event, and how well.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EventMatch {
    pub event: CalendarEvent,
    /// Share of the recording that fell inside the event, `0.0..=1.0`.
    pub overlap: f64,
}

/// Why no event was matched. Reported rather than reduced to `None`, because
/// "nothing was scheduled" and "two things were, equally" call for different
/// responses from the user.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum NoMatchReason {
    /// The calendar had nothing overlapping this recording.
    NothingScheduled,
    /// Something overlapped, but too little of the recording fell inside it.
    TooLittleOverlap,
    /// Two events fit about equally well, so choosing would be a guess.
    Ambiguous,
}

/// The outcome of matching one recording against a day's events.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "SCREAMING_SNAKE_CASE")]
pub enum MatchOutcome {
    Matched(EventMatch),
    /// Nothing was chosen, with the reason and the candidates that were close,
    /// so the user can pick one by hand rather than being told "no".
    None {
        reason: NoMatchReason,
        candidates: Vec<EventMatch>,
    },
}

impl MatchOutcome {
    pub fn matched(&self) -> Option<&EventMatch> {
        match self {
            Self::Matched(found) => Some(found),
            Self::None { .. } => None,
        }
    }
}

/// Picks the event a recording was, or explains why none was picked.
///
/// `started_at` and `ended_at` are the recording's own bounds.
pub fn match_recording(
    events: &[CalendarEvent],
    started_at: chrono::DateTime<chrono::Utc>,
    ended_at: chrono::DateTime<chrono::Utc>,
) -> MatchOutcome {
    let recording_seconds = (ended_at - started_at).num_seconds().max(1) as f64;

    let mut scored: Vec<EventMatch> = events
        .iter()
        .filter_map(|event| {
            let starts = event.starts()?;
            let ends = event.ends()?;
            let overlap_seconds = (ended_at.min(ends) - started_at.max(starts))
                .num_seconds()
                .max(0) as f64;
            (overlap_seconds > 0.0).then(|| EventMatch {
                event: event.clone(),
                overlap: (overlap_seconds / recording_seconds).clamp(0.0, 1.0),
            })
        })
        .collect();

    if scored.is_empty() {
        return MatchOutcome::None {
            reason: NoMatchReason::NothingScheduled,
            candidates: Vec::new(),
        };
    }

    scored.sort_by(|a, b| {
        b.overlap
            .partial_cmp(&a.overlap)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    let best = &scored[0];
    if best.overlap < MIN_OVERLAP_OF_RECORDING {
        return MatchOutcome::None {
            reason: NoMatchReason::TooLittleOverlap,
            candidates: scored,
        };
    }

    if let Some(runner_up) = scored.get(1) {
        if best.overlap - runner_up.overlap < MIN_MARGIN_OVER_RUNNER_UP {
            return MatchOutcome::None {
                reason: NoMatchReason::Ambiguous,
                candidates: scored,
            };
        }
    }

    MatchOutcome::Matched(scored.remove(0))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::calendar::model::{AttendanceResponse, CalendarAttendee};

    fn at(raw: &str) -> chrono::DateTime<chrono::Utc> {
        chrono::DateTime::parse_from_rfc3339(raw)
            .unwrap()
            .with_timezone(&chrono::Utc)
    }

    fn event(id: &str, starts: &str, ends: &str) -> CalendarEvent {
        CalendarEvent {
            id: id.into(),
            title: format!("Event {id}"),
            starts_at: starts.into(),
            ends_at: ends.into(),
            description: None,
            location: None,
            attendees: vec![CalendarAttendee {
                name: "Pranjali".into(),
                email: None,
                response: AttendanceResponse::Accepted,
                is_organizer: true,
                is_self: false,
            }],
            conference_url: None,
            organizer: None,
        }
    }

    #[test]
    fn a_recording_started_late_still_matches_its_meeting() {
        // The common case: the meeting began, somebody remembered to record.
        let events = vec![event("a", "2026-09-04T09:00:00Z", "2026-09-04T10:00:00Z")];
        let outcome = match_recording(
            &events,
            at("2026-09-04T09:12:00Z"),
            at("2026-09-04T09:58:00Z"),
        );

        let matched = outcome.matched().expect("this is plainly that meeting");
        assert_eq!(matched.event.id, "a");
        assert!(matched.overlap > 0.99);
    }

    #[test]
    fn a_recording_that_runs_past_the_scheduled_end_still_matches() {
        let events = vec![event("a", "2026-09-04T09:00:00Z", "2026-09-04T09:30:00Z")];
        let outcome = match_recording(
            &events,
            at("2026-09-04T09:00:00Z"),
            at("2026-09-04T09:50:00Z"),
        );
        assert_eq!(outcome.matched().unwrap().event.id, "a");
    }

    #[test]
    fn a_recording_with_nothing_scheduled_matches_nothing() {
        // Four in the afternoon, empty calendar. Reaching for the nearest event
        // would retitle the meeting and invent a participant list.
        let events = vec![event("a", "2026-09-04T09:00:00Z", "2026-09-04T10:00:00Z")];
        let outcome = match_recording(
            &events,
            at("2026-09-04T16:00:00Z"),
            at("2026-09-04T16:30:00Z"),
        );

        assert!(outcome.matched().is_none());
        assert!(matches!(
            outcome,
            MatchOutcome::None { reason: NoMatchReason::NothingScheduled, .. }
        ));
    }

    #[test]
    fn a_meeting_that_merely_ran_adjacent_is_not_a_match() {
        // Five minutes of overlap out of forty is somebody's next call.
        let events = vec![event("a", "2026-09-04T09:00:00Z", "2026-09-04T09:35:00Z")];
        let outcome = match_recording(
            &events,
            at("2026-09-04T09:30:00Z"),
            at("2026-09-04T10:10:00Z"),
        );

        match outcome {
            MatchOutcome::None { reason, candidates } => {
                assert_eq!(reason, NoMatchReason::TooLittleOverlap);
                assert_eq!(candidates.len(), 1, "the near miss is still offered");
            }
            other => panic!("expected no match, got {other:?}"),
        }
    }

    #[test]
    fn two_events_that_fit_equally_well_produce_neither() {
        // A stand-up inside a longer block nobody declined. Picking the
        // marginally better one would be a guess, and a wrong retitle is the
        // visible kind of wrong.
        let events = vec![
            event("standup", "2026-09-04T09:00:00Z", "2026-09-04T10:00:00Z"),
            event("allhands", "2026-09-04T09:00:00Z", "2026-09-04T10:00:00Z"),
        ];
        let outcome = match_recording(
            &events,
            at("2026-09-04T09:05:00Z"),
            at("2026-09-04T09:55:00Z"),
        );

        match outcome {
            MatchOutcome::None { reason, candidates } => {
                assert_eq!(reason, NoMatchReason::Ambiguous);
                assert_eq!(candidates.len(), 2, "both are offered for the user to pick");
            }
            other => panic!("expected ambiguity, got {other:?}"),
        }
    }

    #[test]
    fn a_clearly_better_fit_wins_over_a_partial_overlap() {
        let events = vec![
            event("exact", "2026-09-04T09:00:00Z", "2026-09-04T10:00:00Z"),
            event("tail", "2026-09-04T09:50:00Z", "2026-09-04T11:00:00Z"),
        ];
        let outcome = match_recording(
            &events,
            at("2026-09-04T09:00:00Z"),
            at("2026-09-04T10:00:00Z"),
        );
        assert_eq!(outcome.matched().unwrap().event.id, "exact");
    }

    #[test]
    fn an_event_with_unreadable_times_is_skipped_rather_than_crashing() {
        let mut broken = event("broken", "not a time", "also not");
        broken.title = "Broken".into();
        let events = vec![
            broken,
            event("good", "2026-09-04T09:00:00Z", "2026-09-04T10:00:00Z"),
        ];
        let outcome = match_recording(
            &events,
            at("2026-09-04T09:05:00Z"),
            at("2026-09-04T09:55:00Z"),
        );
        assert_eq!(outcome.matched().unwrap().event.id, "good");
    }

    #[test]
    fn an_empty_calendar_matches_nothing_without_error() {
        let outcome = match_recording(
            &[],
            at("2026-09-04T09:00:00Z"),
            at("2026-09-04T10:00:00Z"),
        );
        assert!(matches!(
            outcome,
            MatchOutcome::None { reason: NoMatchReason::NothingScheduled, .. }
        ));
    }

    #[test]
    fn a_zero_length_recording_does_not_divide_by_zero() {
        let events = vec![event("a", "2026-09-04T09:00:00Z", "2026-09-04T10:00:00Z")];
        let instant = at("2026-09-04T09:30:00Z");
        // Must return rather than panic; which branch it takes is immaterial.
        let _ = match_recording(&events, instant, instant);
    }
}
