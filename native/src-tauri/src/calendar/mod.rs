//! Google Calendar: what a meeting was called, who was invited, and what for.
//!
//! Rung 3 of `Meeting-rules/meeting_speaker_identification.md`, and the answer
//! to three questions a recording cannot answer for itself. A recorder knows
//! there were three voices; only the calendar knows they were Pranjali, Ayush
//! and Rahul, that the meeting was the placement review, and that it existed to
//! decide a launch date.
//!
//! Three properties this module holds:
//!
//! * **Read-only.** The scope is `calendar.events.readonly`. Relay cannot
//!   create, move or delete an event, and a meeting assistant that could is a
//!   different and scarier product.
//! * **A wrong match is worse than none.** Matching scores candidates and
//!   refuses to choose between two that fit equally well, because a wrong match
//!   retitles the meeting and populates its participants with people who were
//!   not there.
//! * **An event is data, never an instruction.** Titles and descriptions are
//!   written by whoever sent the invitation. They are stored and shown; where
//!   they reach a model they go inside the untrusted-source boundary.

pub mod google;
pub mod match_event;
pub mod model;

pub use match_event::{match_recording, EventMatch, MatchOutcome, NoMatchReason};
pub use model::{AttendanceResponse, CalendarAttendee, CalendarEvent};

use serde::{Deserialize, Serialize};

/// Whether Relay can read the calendar, and as whom.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CalendarConnection {
    pub connected: bool,
    #[serde(default)]
    pub account_email: Option<String>,
    #[serde(default)]
    pub account_name: Option<String>,
    /// Set when a stored connection exists but cannot currently be used, in
    /// words naming the fix.
    #[serde(default)]
    pub problem: Option<String>,
}

impl CalendarConnection {
    pub fn disconnected() -> Self {
        Self {
            connected: false,
            account_email: None,
            account_name: None,
            problem: None,
        }
    }
}

/// What the calendar had to say about one recording.
///
/// Kept on the meeting rather than merged into it, so a wrong match can be
/// cleared without having to work out which fields it wrote.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MeetingCalendarLink {
    pub outcome: MatchOutcome,
    /// When the match was made, so a stale link is recognisable.
    pub linked_at: String,
    /// True when a person chose this event rather than Relay matching it.
    #[serde(default)]
    pub chosen_by_user: bool,
}

impl MeetingCalendarLink {
    pub fn event(&self) -> Option<&CalendarEvent> {
        self.outcome.matched().map(|m| &m.event)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_disconnected_calendar_reports_no_account_and_no_problem() {
        // "Not connected" is a state, not a fault, and must not read as one.
        let connection = CalendarConnection::disconnected();
        assert!(!connection.connected);
        assert_eq!(connection.account_email, None);
        assert_eq!(connection.problem, None);
    }

    #[test]
    fn a_link_with_no_match_exposes_no_event() {
        let link = MeetingCalendarLink {
            outcome: MatchOutcome::None {
                reason: NoMatchReason::NothingScheduled,
                candidates: Vec::new(),
            },
            linked_at: "2026-09-04T10:00:00Z".into(),
            chosen_by_user: false,
        };
        assert!(link.event().is_none());
    }
}
