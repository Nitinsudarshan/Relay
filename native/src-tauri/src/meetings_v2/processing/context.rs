//! The canonical meeting context.
//!
//! One place that decides what a model is told about a meeting, and in what
//! shape. Before this existed, Stage A received a rendered transcript and three
//! loose strings interpolated into a format literal, and there was nowhere to
//! put anything else — which is why the user's own notes, the meeting's
//! duration, and its metadata never reached the model at all.
//!
//! ```text
//!            transcript  ─┐
//!    speaker attribution ─┤
//!               metadata ─┼──►  MeetingContext  ──►  Stage A
//!             user notes ─┤
//!  pre-meeting notes (rare)┘
//! ```
//!
//! Two rules hold this layer honest:
//!
//! * **Absent is absent.** An optional block that has no content is not
//!   rendered — not as an empty heading, not as the word "None". A model shown
//!   `# Pre-Meeting Notes\n\nNone.` will write a summary that mentions their
//!   absence, and a user who never writes notes would see that on every meeting.
//! * **Nothing is included because it exists.** Fields are here because they
//!   change how the meeting reads, not because the struct has them.
//!
//! The transcript remains the source of truth. Notes are corroboration and
//! emphasis; metadata is framing; pre-meeting notes are intent. None of them
//! outrank what was actually said.

use super::model::{NormalizedSegment, Speaker};
use crate::meetings_v2::types::MeetingNotes;

/// How much of the window a single extraction pass may fill with transcript
/// before the meeting is processed in more than one pass.
///
/// Below this the overhead of chunking outweighs its benefit; above it, the
/// provider starts silently discarding the front of the prompt.
const MIN_WINDOW_CHARS: usize = 4_000;

/// Segments repeated at the head of the next window so a commitment that
/// straddles a boundary is not lost by either pass.
const WINDOW_OVERLAP_SEGMENTS: usize = 2;

/// Everything about one meeting that is worth telling a model.
pub struct MeetingContext<'a> {
    pub title: &'a str,
    /// `YYYY-MM-DD`. Used to resolve "Friday" into a real date, and for nothing
    /// else — it is never presented as a fact about the meeting.
    pub date_iso: &'a str,
    /// Recorded duration, when the session knows it.
    pub duration_minutes: Option<u64>,
    pub speakers: &'a [Speaker],
    pub segments: &'a [NormalizedSegment],
    /// The user's own notes. Empty is the common case.
    pub notes: &'a MeetingNotes,
    /// Canonical spellings the transcript may have mangled.
    pub glossary: &'a [String],
}

impl<'a> MeetingContext<'a> {
    /// Words of real content, after deterministic cleanup.
    pub fn transcript_words(&self) -> usize {
        self.segments
            .iter()
            .map(|s| s.text.split_whitespace().count())
            .sum()
    }

    /// The participant roster, as the model should read it.
    ///
    /// Says plainly when nobody could be identified. A model that is told the
    /// roster is empty assigns nothing; a model that is told nothing about
    /// speakers invents them.
    pub fn render_roster(&self) -> String {
        if self.speakers.is_empty() {
            return "No speakers could be identified from the recording. Every owner is \
therefore \"unassigned\", and no statement may be attributed to a named person."
                .to_string();
        }

        self.speakers
            .iter()
            .map(|s| {
                let role = if s.is_local_user {
                    " — the local user, whose microphone this is"
                } else {
                    ""
                };
                let named = if s.display_name.is_some() {
                    ""
                } else {
                    " (identified by audio channel only; their real name is unknown)"
                };
                format!("- {} (id: {}){}{}", s.label(), s.id, role, named)
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// The meeting's framing: what it is called, when it happened, how long it
    /// ran. Omits whatever the session does not know.
    pub fn render_metadata(&self) -> String {
        let mut lines = vec![format!("Title as recorded: {}", self.title)];
        if !self.date_iso.trim().is_empty() {
            lines.push(format!("Meeting date: {}", self.date_iso));
        }
        if let Some(minutes) = self.duration_minutes.filter(|m| *m > 0) {
            lines.push(format!("Recorded length: about {} minutes", minutes));
        }
        lines.push(format!(
            "Transcribed content: about {} words",
            self.transcript_words()
        ));
        lines.join("\n")
    }

    /// The user's notes, or nothing at all.
    ///
    /// Two separately labelled blocks, because the model is told to use them
    /// differently: notes taken during the meeting are evidence about what
    /// mattered, and notes written beforehand are evidence about what was
    /// *intended*, which the meeting itself may have overtaken.
    pub fn render_notes(&self) -> String {
        // Free-text directives are folded into these two blocks rather than
        // given a third. To a summarizer, "remember the vault rewrite is
        // blocked" typed as a directive and the same sentence typed in the
        // paragraph box are the same kind of evidence; a separate block would
        // only invite the model to weigh one above the other. The directives
        // that are *not* prose — a name correction, a misheard term — never
        // reach a model at all, because the registry and the glossary act on
        // them directly.
        let mut out = String::new();
        let during = self.notes.during_for_model();
        if !during.trim().is_empty() {
            out.push_str("USER NOTES (written by a participant during or after the meeting)\n");
            out.push_str(during.trim());
            out.push_str("\n\n");
        }
        let before = self.notes.before_for_model();
        if !before.trim().is_empty() {
            out.push_str("NOTES WRITTEN BEFORE THE MEETING (intent and agenda, not outcome)\n");
            out.push_str(before.trim());
            out.push_str("\n\n");
        }
        out
    }

    /// The glossary block, or nothing.
    pub fn render_glossary(&self) -> String {
        let terms: Vec<&str> = self
            .glossary
            .iter()
            .map(|t| t.trim())
            .filter(|t| !t.is_empty())
            .collect();
        if terms.is_empty() {
            return String::new();
        }
        format!(
            "TERMS THIS TEAM USES (the transcript may have misheard them)\n{}\n\n",
            terms.join(", ")
        )
    }

    /// Renders a stretch of the transcript with segment ids, so every extracted
    /// claim can cite where it came from.
    pub fn render_transcript(&self, window: &Window) -> String {
        let mut out = String::new();
        for segment in &self.segments[window.start..window.end] {
            let label = super::speakers::resolve_label(self.speakers, segment.speaker_id.as_deref());
            out.push_str(&format!(
                "[{}] {} ({}): {}\n",
                segment.id,
                label,
                super::conversation::format_timestamp(segment.start_time_s),
                segment.text
            ));
        }
        out
    }

    /// The complete user message for one extraction pass.
    ///
    /// Order matters: framing, then who was there, then the terms, then what a
    /// human thought was important, then what was actually said. The transcript
    /// goes last because it is the longest block and the one the model should
    /// still have in view when it starts answering.
    pub fn render_extraction_input(&self, window: &Window) -> String {
        let mut out = String::new();
        out.push_str("MEETING\n");
        out.push_str(&self.render_metadata());
        out.push_str("\n\nPARTICIPANTS\n");
        out.push_str(&self.render_roster());
        out.push_str("\n\n");
        out.push_str(&self.render_glossary());
        out.push_str(&self.render_notes());

        if window.is_partial {
            out.push_str(&format!(
                "TRANSCRIPT — part {} of {}\nThis is one stretch of a longer meeting. Extract only \
what this stretch supports; another pass covers the rest.\n",
                window.index + 1,
                window.total
            ));
        } else {
            out.push_str("TRANSCRIPT\n");
        }
        out.push_str(&self.render_transcript(window));
        out
    }

    /// Splits the transcript into stretches that fit the model's window.
    ///
    /// A single window is returned whenever the whole meeting fits, which is the
    /// common case and the one that stays simplest. When it does not fit, the
    /// alternative is not truncation: every segment appears in some window, so
    /// nothing said in the first ten minutes can vanish because the meeting ran
    /// for ninety.
    pub fn windows(&self, budget_chars: usize) -> Vec<Window> {
        let budget = budget_chars.max(MIN_WINDOW_CHARS);
        if self.segments.is_empty() {
            return vec![Window {
                start: 0,
                end: 0,
                index: 0,
                total: 1,
                is_partial: false,
            }];
        }

        // Everything except the transcript is repeated in every window, so it
        // has to come out of the budget once per pass.
        let overhead = self.render_metadata().len()
            + self.render_roster().len()
            + self.render_glossary().len()
            + self.render_notes().len()
            + 400;
        let transcript_budget = budget.saturating_sub(overhead).max(MIN_WINDOW_CHARS / 2);

        let mut bounds: Vec<(usize, usize)> = Vec::new();
        let mut start = 0usize;
        while start < self.segments.len() {
            let mut end = start;
            let mut used = 0usize;
            while end < self.segments.len() {
                // Approximates the rendered line: id, label, timestamp, text.
                let cost = self.segments[end].text.len() + 48;
                if used + cost > transcript_budget && end > start {
                    break;
                }
                used += cost;
                end += 1;
            }
            bounds.push((start, end));
            if end >= self.segments.len() {
                break;
            }
            start = end.saturating_sub(WINDOW_OVERLAP_SEGMENTS).max(start + 1);
        }

        let total = bounds.len();
        bounds
            .into_iter()
            .enumerate()
            .map(|(index, (start, end))| Window {
                start,
                end,
                index,
                total,
                is_partial: total > 1,
            })
            .collect()
    }
}

/// One stretch of the transcript, as a half-open segment range.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Window {
    pub start: usize,
    pub end: usize,
    pub index: usize,
    pub total: usize,
    /// True only when the meeting needed more than one pass. Drives whether the
    /// model is told it is reading part of something larger.
    pub is_partial: bool,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::meetings_v2::processing::model::{
        SegmentChannel, SpeakerOrigin, SPEAKER_ID_ME, SPEAKER_ID_REMOTE,
    };

    fn speaker(id: &str, fallback: &str, name: Option<&str>, local: bool) -> Speaker {
        Speaker {
            id: id.to_string(),
            display_name: name.map(str::to_string),
            fallback_label: fallback.to_string(),
            origin: SpeakerOrigin::Channel,
            channel: if local {
                SegmentChannel::Mic
            } else {
                SegmentChannel::System
            },
            is_local_user: local,
            segment_count: 4,
        }
    }

    fn segments(count: usize, words_each: usize) -> Vec<NormalizedSegment> {
        (0..count)
            .map(|i| NormalizedSegment {
                id: format!("seg_{:05}", i),
                chunk_index: i,
                utterance_index: None,
                start_time_s: i as f64 * 30.0,
                end_time_s: (i + 1) as f64 * 30.0,
                text: vec!["word"; words_each].join(" "),
                raw_text: String::new(),
                channel: SegmentChannel::Mic,
                speaker_id: Some(SPEAKER_ID_ME.to_string()),
                applied_rules: Vec::new(),
            })
            .collect()
    }

    fn context<'a>(
        segments: &'a [NormalizedSegment],
        speakers: &'a [Speaker],
        notes: &'a MeetingNotes,
        glossary: &'a [String],
    ) -> MeetingContext<'a> {
        MeetingContext {
            title: "Release Planning",
            date_iso: "2026-08-27",
            duration_minutes: Some(42),
            speakers,
            segments,
            notes,
            glossary,
        }
    }

    #[test]
    fn absent_notes_produce_no_block_at_all() {
        let segs = segments(2, 10);
        let speakers = vec![speaker(SPEAKER_ID_ME, "Me", None, true)];
        let notes = MeetingNotes::default();
        let ctx = context(&segs, &speakers, &notes, &[]);

        let rendered = ctx.render_extraction_input(&ctx.windows(50_000)[0]);
        assert!(!rendered.contains("USER NOTES"));
        assert!(!rendered.contains("BEFORE THE MEETING"));
        assert!(
            !rendered.to_lowercase().contains("none"),
            "the absence of notes must never be stated"
        );
    }

    #[test]
    fn notes_are_labelled_by_when_they_were_written() {
        let segs = segments(2, 10);
        let speakers = vec![speaker(SPEAKER_ID_ME, "Me", None, true)];
        let notes = MeetingNotes {
            directives: Vec::new(),
            during: "budget is the blocker".to_string(),
            before: "agenda: budget, hiring".to_string(),
            updated_at: None,
        };
        let ctx = context(&segs, &speakers, &notes, &[]);
        let rendered = ctx.render_extraction_input(&ctx.windows(50_000)[0]);

        assert!(rendered.contains("during or after the meeting"));
        assert!(rendered.contains("budget is the blocker"));
        assert!(rendered.contains("intent and agenda, not outcome"));
        assert!(rendered.contains("agenda: budget, hiring"));
        // Order: what a human wrote comes before the raw transcript.
        assert!(rendered.find("budget is the blocker") < rendered.find("[seg_00000]"));
    }

    #[test]
    fn free_text_directives_reach_the_model_as_notes() {
        use crate::meetings_v2::types::{DirectiveKind, MeetingDirective};

        let segs = segments(2, 30);
        let speakers = vec![speaker(SPEAKER_ID_ME, "Me", None, true)];
        let mut notes = MeetingNotes::default();
        notes.directives.push(
            MeetingDirective::new(DirectiveKind::Note, None, "the vault rewrite is blocked")
                .unwrap(),
        );
        notes.directives.push(
            MeetingDirective::new(DirectiveKind::Agenda, None, "decide the launch date").unwrap(),
        );
        // Neither of these is prose, so neither belongs in a prompt.
        notes.directives.push(
            MeetingDirective::new(DirectiveKind::SpeakerName, Some("Speaker 1"), "Pranjali")
                .unwrap(),
        );
        notes.directives.push(
            MeetingDirective::new(DirectiveKind::Term, Some("Lance TV"), "LanceDB").unwrap(),
        );

        let rendered = context(&segs, &speakers, &notes, &[]).render_notes();
        assert!(rendered.contains("the vault rewrite is blocked"), "{rendered}");
        assert!(rendered.contains("decide the launch date"), "{rendered}");
        assert!(
            !rendered.contains("Pranjali"),
            "a name correction is applied to the registry, not described to a model: {rendered}"
        );
        assert!(
            !rendered.contains("Lance TV"),
            "a misheard term is applied by the normalizer, not described to a model: {rendered}"
        );
    }

    #[test]
    fn pre_meeting_notes_alone_do_not_change_the_shape_of_anything_else() {
        let segs = segments(2, 10);
        let speakers = vec![speaker(SPEAKER_ID_ME, "Me", None, true)];
        let without = MeetingNotes::default();
        let with = MeetingNotes {
            before: "agenda".to_string(),
            ..Default::default()
        };

        let a = context(&segs, &speakers, &without, &[]);
        let b = context(&segs, &speakers, &with, &[]);
        assert_eq!(a.windows(50_000).len(), b.windows(50_000).len());
        assert_eq!(a.render_roster(), b.render_roster());
        assert_eq!(a.render_metadata(), b.render_metadata());
    }

    #[test]
    fn an_empty_roster_says_so_rather_than_leaving_the_model_to_guess() {
        let segs = segments(1, 10);
        let notes = MeetingNotes::default();
        let ctx = context(&segs, &[], &notes, &[]);
        assert!(ctx.render_roster().contains("unassigned"));
        assert!(ctx.render_roster().contains("No speakers could be identified"));
    }

    #[test]
    fn an_unnamed_speaker_is_marked_as_channel_only() {
        let segs = segments(1, 10);
        let speakers = vec![
            speaker(SPEAKER_ID_ME, "Me", None, true),
            speaker(SPEAKER_ID_REMOTE, "Speaker 1", Some("Pranjali"), false),
        ];
        let notes = MeetingNotes::default();
        let ctx = context(&segs, &speakers, &notes, &[]);
        let roster = ctx.render_roster();

        assert!(roster.contains("Me (id: speaker_me)"));
        assert!(roster.contains("their real name is unknown"));
        // A named speaker carries no such caveat.
        let pranjali_line = roster.lines().find(|l| l.contains("Pranjali")).unwrap();
        assert!(!pranjali_line.contains("real name is unknown"));
    }

    #[test]
    fn a_meeting_that_fits_is_one_window() {
        let segs = segments(20, 30);
        let speakers = vec![speaker(SPEAKER_ID_ME, "Me", None, true)];
        let notes = MeetingNotes::default();
        let ctx = context(&segs, &speakers, &notes, &[]);

        let windows = ctx.windows(200_000);
        assert_eq!(windows.len(), 1);
        assert!(!windows[0].is_partial);
        assert_eq!((windows[0].start, windows[0].end), (0, 20));
    }

    #[test]
    fn a_meeting_that_does_not_fit_is_covered_completely_rather_than_cut_off() {
        // The regression this pins: a transcript longer than the window used to
        // be handed over whole, and the provider dropped the front of it.
        let segs = segments(120, 60);
        let speakers = vec![speaker(SPEAKER_ID_ME, "Me", None, true)];
        let notes = MeetingNotes::default();
        let ctx = context(&segs, &speakers, &notes, &[]);

        let windows = ctx.windows(8_000);
        assert!(windows.len() > 1, "a long meeting must be split");
        assert_eq!(windows[0].start, 0, "the meeting must start at its start");
        assert_eq!(
            windows.last().unwrap().end,
            segs.len(),
            "and run to its end"
        );

        let mut covered = vec![false; segs.len()];
        for window in &windows {
            for slot in &mut covered[window.start..window.end] {
                *slot = true;
            }
        }
        assert!(
            covered.iter().all(|c| *c),
            "every segment must appear in some window"
        );
        assert!(windows.iter().all(|w| w.is_partial));
    }

    #[test]
    fn consecutive_windows_overlap_so_a_boundary_cannot_swallow_a_commitment() {
        let segs = segments(120, 60);
        let speakers = vec![speaker(SPEAKER_ID_ME, "Me", None, true)];
        let notes = MeetingNotes::default();
        let ctx = context(&segs, &speakers, &notes, &[]);

        let windows = ctx.windows(8_000);
        for pair in windows.windows(2) {
            assert!(
                pair[1].start < pair[0].end,
                "windows {:?} and {:?} do not overlap",
                pair[0],
                pair[1]
            );
        }
    }

    #[test]
    fn windowing_always_terminates_even_on_a_single_enormous_segment() {
        let segs = segments(3, 20_000);
        let speakers = vec![speaker(SPEAKER_ID_ME, "Me", None, true)];
        let notes = MeetingNotes::default();
        let ctx = context(&segs, &speakers, &notes, &[]);

        let windows = ctx.windows(4_000);
        assert!(windows.len() <= segs.len() + 1);
        assert_eq!(windows.last().unwrap().end, segs.len());
    }

    #[test]
    fn an_empty_transcript_yields_one_empty_window() {
        let notes = MeetingNotes::default();
        let ctx = context(&[], &[], &notes, &[]);
        let windows = ctx.windows(8_000);
        assert_eq!(windows.len(), 1);
        assert_eq!((windows[0].start, windows[0].end), (0, 0));
        assert!(!windows[0].is_partial);
    }

    #[test]
    fn a_partial_window_tells_the_model_it_is_reading_part_of_something() {
        let segs = segments(120, 60);
        let speakers = vec![speaker(SPEAKER_ID_ME, "Me", None, true)];
        let notes = MeetingNotes::default();
        let ctx = context(&segs, &speakers, &notes, &[]);

        let windows = ctx.windows(8_000);
        let rendered = ctx.render_extraction_input(&windows[1]);
        assert!(rendered.contains("part 2 of"));
        assert!(rendered.contains("another pass covers the rest"));
    }

    #[test]
    fn the_glossary_reaches_the_model_when_there_is_one() {
        let segs = segments(2, 10);
        let speakers = vec![speaker(SPEAKER_ID_ME, "Me", None, true)];
        let notes = MeetingNotes::default();
        let glossary = vec!["NavGurukul".to_string(), "  ".to_string()];
        let ctx = context(&segs, &speakers, &notes, &glossary);

        let rendered = ctx.render_extraction_input(&ctx.windows(50_000)[0]);
        assert!(rendered.contains("NavGurukul"));
        assert!(!rendered.contains("TERMS THIS TEAM USES\n,"));

        let empty_glossary = context(&segs, &speakers, &notes, &[]);
        assert!(empty_glossary.render_glossary().is_empty());
    }

    #[test]
    fn metadata_omits_what_the_session_does_not_know() {
        let segs = segments(2, 10);
        let speakers = vec![speaker(SPEAKER_ID_ME, "Me", None, true)];
        let notes = MeetingNotes::default();
        let mut ctx = context(&segs, &speakers, &notes, &[]);
        ctx.duration_minutes = None;
        ctx.date_iso = "";

        let metadata = ctx.render_metadata();
        assert!(!metadata.contains("Meeting date"));
        assert!(!metadata.contains("Recorded length"));
        assert!(metadata.contains("Transcribed content"));
    }
}
