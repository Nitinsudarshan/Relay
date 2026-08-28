//! The summary quality evaluation set.
//!
//! A test suite says the pipeline still works. This says whether its output is
//! any *good*, which is a different question and the one that was previously
//! unanswerable: before this existed, a change to a prompt or a threshold could
//! only be spot-checked, so quality could drift a long way before anyone
//! noticed. `Meeting-rules/meeting_pipeline_gap_analysis.md` lists it as gap 10.
//!
//! Two pieces:
//!
//! * **Cases** — meetings with hand-checked expectations. What was decided, what
//!   was committed to and by whom, what was left open, and — the half that
//!   matters most — what a summary of this meeting must *not* say.
//! * **A scorer** — deterministic measurement of one summary against one case's
//!   expectations, along the axes a reader actually cares about: did the
//!   decisions survive, did the commitments survive with the right owner and
//!   date, did the reasoning survive, and did anything appear that nobody said.
//!
//! Deliberately model-free. The scorer takes summary text and returns a
//! scorecard, so it runs in `cargo test` with no provider, no network, and no
//! run-to-run variance — and the same function can be pointed at real model
//! output when one is available. What it measures is the pipeline's behaviour,
//! not a particular model's mood on a particular afternoon.
//!
//! Hallucination is the one axis with no threshold. A single invented owner,
//! deadline, or decision fails the case outright, because a summary that is
//! ninety per cent right about work someone now believes they own is worse than
//! one that admits it does not know.

use crate::meetings_v2::types::MeetingNotes;

/// A commitment a summary is expected to carry.
#[derive(Debug, Clone)]
pub struct ExpectedAction {
    /// Wording that identifies the action. Matched loosely.
    pub action: &'static str,
    /// The owner label that must appear on it, or `None` where the meeting
    /// established no owner — in which case an owner appearing is a failure.
    pub owner: Option<&'static str>,
    /// The date that must appear, or `None` where none was spoken.
    pub deadline: Option<&'static str>,
}

/// What a good summary of one meeting contains, and what it must never contain.
#[derive(Debug, Clone, Default)]
pub struct Expected {
    /// Decisions the meeting actually reached.
    pub decisions: Vec<&'static str>,
    pub actions: Vec<ExpectedAction>,
    /// Reasoning that must survive alongside its decision.
    pub rationale: Vec<&'static str>,
    pub open_questions: Vec<&'static str>,
    pub risks: Vec<&'static str>,
    /// Concrete specifics — numbers, names, constraints — a reader would want.
    pub details: Vec<&'static str>,
    /// Anything whose presence is a hallucination: a decision nobody made, an
    /// owner nobody accepted, a date nobody said.
    ///
    /// Must be *lexically* distinct from what the meeting did say, not merely
    /// its opposite. The scorer matches content words, so "the data was cleaned"
    /// as the negation of "the data must be cleaned" would fire on the correct
    /// summary — the same polarity blindness the summary rules warn about in
    /// translated transcripts. Write a forbidden claim as the sentence a
    /// hallucinating summary would actually produce.
    pub forbidden: Vec<&'static str>,
    /// Content the meeting genuinely contains that a summary must nonetheless
    /// leave out: greetings, audio checks, small talk, decoder loops.
    ///
    /// Separate from `forbidden` because the failure is a different one.
    /// Repeating a greeting is not an invention — it is the "expand the noise"
    /// failure the summary rules name, where the model treats the first thing
    /// said as the subject of the meeting. It costs score; it does not fail the
    /// case outright the way an invented owner does.
    pub noise: Vec<&'static str>,
}

/// One evaluated meeting.
pub struct EvalCase {
    pub name: &'static str,
    /// What the meeting was about, for the report.
    pub premise: &'static str,
    /// `(text, mic_had_audio, sys_had_audio)` per 30-second chunk.
    pub transcript: Vec<(&'static str, bool, bool)>,
    pub notes: MeetingNotes,
    /// A realistic Stage A answer for this meeting.
    ///
    /// Lets the model path be exercised without a provider. It is a *proposal*
    /// like any model's: sanitization, action qualification, rendering, and
    /// validation all run on it for real, so a case can still fail because the
    /// pipeline mishandled a plausible answer.
    pub model_extraction: &'static str,
    pub expected: Expected,
}

/// How one summary scored.
#[derive(Debug, Clone, Default)]
pub struct Scorecard {
    pub case: String,
    pub decision_recall: f64,
    pub action_recall: f64,
    pub owner_accuracy: f64,
    pub deadline_accuracy: f64,
    pub rationale_preservation: f64,
    pub open_question_recall: f64,
    pub risk_recall: f64,
    pub detail_preservation: f64,
    /// Forbidden claims that appeared. Any entry fails the case.
    pub hallucinations: Vec<String>,
    /// 1.0 when no conversational noise survived into the summary.
    pub noise_suppression: f64,
    /// Fraction of bullets that repeat an earlier one.
    pub repetition: f64,
    /// Whether the output follows the required shape.
    pub structure_ok: bool,
    pub words: usize,
}

impl Scorecard {
    /// A single number for ranking runs. Recall axes averaged, with
    /// hallucination as a hard gate rather than a weighted term — averaging it
    /// in would let a summary buy back an invented owner with extra detail.
    pub fn overall(&self) -> f64 {
        if !self.hallucinations.is_empty() {
            return 0.0;
        }
        let axes = [
            self.decision_recall,
            self.action_recall,
            self.owner_accuracy,
            self.deadline_accuracy,
            self.rationale_preservation,
            self.open_question_recall,
            self.risk_recall,
            self.detail_preservation,
            self.noise_suppression,
        ];
        let mean = axes.iter().sum::<f64>() / axes.len() as f64;
        (mean * (1.0 - self.repetition) * if self.structure_ok { 1.0 } else { 0.8 })
            .clamp(0.0, 1.0)
    }

    pub fn report(&self) -> String {
        format!(
            "{:<28} overall {:.2} | decisions {:.2} actions {:.2} owners {:.2} dates {:.2} \
rationale {:.2} open {:.2} risks {:.2} detail {:.2} noise-free {:.2} | repetition {:.2} | \
{} words | hallucinations: {}",
            self.case,
            self.overall(),
            self.decision_recall,
            self.action_recall,
            self.owner_accuracy,
            self.deadline_accuracy,
            self.rationale_preservation,
            self.open_question_recall,
            self.risk_recall,
            self.detail_preservation,
            self.noise_suppression,
            self.repetition,
            self.words,
            if self.hallucinations.is_empty() {
                "none".to_string()
            } else {
                self.hallucinations.join("; ")
            }
        )
    }
}

/// Scores one summary against one case's expectations.
pub fn score(case_name: &str, summary: &str, expected: &Expected) -> Scorecard {
    let haystack = normalize(summary);

    let recall = |claims: &[&'static str]| -> f64 {
        if claims.is_empty() {
            return 1.0;
        }
        let found = claims.iter().filter(|c| contains_claim(&haystack, c)).count();
        found as f64 / claims.len() as f64
    };

    let mut hallucinations: Vec<String> = expected
        .forbidden
        .iter()
        .filter(|f| contains_claim(&haystack, f))
        .map(|f| (*f).to_string())
        .collect();

    // Owner and deadline accuracy are measured only over the actions that were
    // actually reported: crediting a summary for getting the owner right on a
    // commitment it left out entirely would reward omission.
    let mut owner_checks = 0usize;
    let mut owner_correct = 0usize;
    let mut deadline_checks = 0usize;
    let mut deadline_correct = 0usize;

    for action in &expected.actions {
        if !contains_claim(&haystack, action.action) {
            continue;
        }
        let line = line_containing(summary, action.action).unwrap_or_default();
        let line_norm = normalize(&line);

        owner_checks += 1;
        match action.owner {
            Some(owner) => {
                if line_norm.contains(&normalize(owner)) {
                    owner_correct += 1;
                }
            }
            None => {
                // No owner was established. Either the line says so, or the
                // summary has invented one.
                if line_norm.contains("unassigned") || !line_norm.contains('*') {
                    owner_correct += 1;
                }
            }
        }

        deadline_checks += 1;
        let has_date = line.contains("Due:") || line_norm.contains("due ");
        match action.deadline {
            Some(deadline) => {
                if line.contains(deadline) {
                    deadline_correct += 1;
                }
            }
            None => {
                if has_date {
                    hallucinations.push(format!(
                        "a deadline on \"{}\", which nobody gave one",
                        action.action
                    ));
                } else {
                    deadline_correct += 1;
                }
            }
        }
    }

    let ratio = |correct: usize, total: usize| {
        if total == 0 {
            1.0
        } else {
            correct as f64 / total as f64
        }
    };

    let surviving_noise = expected
        .noise
        .iter()
        .filter(|n| contains_claim(&haystack, n))
        .count();

    Scorecard {
        case: case_name.to_string(),
        decision_recall: recall(&expected.decisions),
        action_recall: recall(
            &expected
                .actions
                .iter()
                .map(|a| a.action)
                .collect::<Vec<_>>(),
        ),
        owner_accuracy: ratio(owner_correct, owner_checks),
        deadline_accuracy: ratio(deadline_correct, deadline_checks),
        rationale_preservation: recall(&expected.rationale),
        open_question_recall: recall(&expected.open_questions),
        risk_recall: recall(&expected.risks),
        detail_preservation: recall(&expected.details),
        noise_suppression: if expected.noise.is_empty() {
            1.0
        } else {
            1.0 - (surviving_noise as f64 / expected.noise.len() as f64)
        },
        hallucinations,
        repetition: repetition(summary),
        structure_ok: summary.trim_start().starts_with("## Overview"),
        words: summary.split_whitespace().count(),
    }
}

/// Whether a summary makes a claim.
///
/// Matched on content words rather than exact wording, because a summary that
/// says the same thing in its own words is doing exactly what it should — the
/// rewrite rule requires it. Three quarters of a claim's content words present
/// counts as the claim being made, and order is deliberately ignored: a
/// paraphrase reorders, and a scorer that demanded the original order would
/// penalise the behaviour the rules ask for.
///
/// Each word in the summary is consumed at most once, so a summary that repeats
/// one word cannot satisfy several of a claim's words with it.
fn contains_claim(haystack: &str, claim: &str) -> bool {
    let words: Vec<String> = normalize(claim)
        .split_whitespace()
        .filter(|w| !is_stopword(w))
        .map(str::to_string)
        .collect();
    if words.is_empty() {
        return false;
    }
    let needed = ((words.len() as f64) * 0.75).ceil() as usize;

    let mut hay: Vec<String> = haystack.split_whitespace().map(str::to_string).collect();
    let mut matched = 0usize;
    for word in &words {
        if let Some(index) = hay.iter().position(|h| same_content_word(h, word)) {
            hay.remove(index);
            matched += 1;
        }
    }
    matched >= needed
}

/// Whether two words carry the same content.
///
/// Prefix matching handles inflection — "review"/"reviewed",
/// "clean"/"cleaned" — but only between words long enough for a shared prefix
/// to mean something. Without the length floor, "off" matches "of", which is
/// how a forbidden claim ("QA signed off") fires on a summary that says the
/// opposite ("QA needs another three days on the payment integration").
fn same_content_word(a: &str, b: &str) -> bool {
    if a == b {
        return true;
    }
    let shorter = a.len().min(b.len());
    shorter >= 4 && (a.starts_with(b) || b.starts_with(a))
}

fn is_stopword(word: &str) -> bool {
    matches!(
        word,
        "the" | "a" | "an" | "to" | "of" | "on" | "in" | "is" | "was" | "be" | "and" | "for"
            | "it" | "that" | "this" | "with" | "as" | "at" | "by" | "will"
    )
}

fn normalize(text: &str) -> String {
    text.chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '-' || c == '*' {
                c.to_ascii_lowercase()
            } else {
                ' '
            }
        })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

/// The first line that makes a claim, so owner and date can be checked in the
/// place they would actually appear.
fn line_containing(summary: &str, claim: &str) -> Option<String> {
    summary
        .lines()
        .find(|line| contains_claim(&normalize(line), claim))
        .map(str::to_string)
}

/// The fraction of bullets that repeat an earlier one.
fn repetition(summary: &str) -> f64 {
    let bullets: Vec<String> = summary
        .lines()
        .map(str::trim)
        .filter(|l| l.starts_with("- ") || l.starts_with("* "))
        .map(|l| {
            normalize(
                l.trim_start_matches(['-', '*', ' '])
                    .trim_start_matches("[ ]")
                    .trim_start_matches("[x]"),
            )
        })
        .filter(|l| !l.is_empty())
        .collect();
    if bullets.len() < 2 {
        return 0.0;
    }
    let mut seen = std::collections::HashSet::new();
    let duplicates = bullets.iter().filter(|b| !seen.insert((*b).clone())).count();
    duplicates as f64 / bullets.len() as f64
}

/// The evaluation set.
///
/// Small on purpose. Each case exists to pin one failure mode that a real
/// meeting produced or that the summary rules explicitly forbid, and a case that
/// does not distinguish a good summary from a bad one is not worth the runtime.
pub fn cases() -> Vec<EvalCase> {
    vec![
        EvalCase {
            name: "decision_with_rationale",
            premise: "A launch slips, and the reason is the part worth keeping.",
            transcript: vec![
                (
                    "right so on the launch, we agreed to move it from Friday to Monday because \
QA needs another three days on the payment integration, there are still three blocking bugs \
in there and shipping on top of them would be worse than a weekend of slip",
                    true,
                    false,
                ),
                (
                    "yes that works for me, I'll update the release calendar today and let \
support know before end of day",
                    false,
                    true,
                ),
            ],
            notes: MeetingNotes::default(),
                        model_extraction: r#"{
  "title": "Launch Slip And QA Runway",
  "meeting_type": "planning",
  "key_points": [
    {"text": "The payment integration still carries three blocking bugs.", "kind": "discussion", "topic": "Launch timing", "source_segment_ids": ["seg_00000"]},
    {"text": "Shipping on top of the open bugs was judged worse than a weekend of slip.", "kind": "tradeoff", "topic": "Launch timing", "source_segment_ids": ["seg_00000"]}
  ],
  "topics": [{"label": "Launch timing", "segment_ids": ["seg_00000", "seg_00001"]}],
  "decisions": [
    {"statement": "Move the launch from Friday to Monday.", "rationale": "QA needs another three days on the payment integration", "decided_by": "speaker_me", "source_segment_ids": ["seg_00000"]}
  ],
  "action_items": [
    {"description": "Update the release calendar", "owner": "speaker_1", "candidate_type": "action", "source_segment_ids": ["seg_00001"]},
    {"description": "Tell support about the new date", "owner": "speaker_1", "candidate_type": "action", "source_segment_ids": ["seg_00001"]}
  ],
  "open_questions": [],
  "risks": [{"statement": "Three blocking bugs remain in the payment integration.", "kind": "blocker", "raised_by": "speaker_me", "source_segment_ids": ["seg_00000"]}],
  "entities": [{"name": "QA", "kind": "other", "segment_ids": ["seg_00000"]}]
}"#,
            expected: Expected {
                decisions: vec!["move the launch from Friday to Monday"],
                rationale: vec!["QA needs another three days on the payment integration"],
                actions: vec![ExpectedAction {
                    action: "update the release calendar",
                    owner: Some("Speaker 1"),
                    deadline: None,
                }],
                details: vec!["three blocking bugs"],
                forbidden: vec![
                    "the launch was cancelled entirely",
                    "QA signed off on the payment integration",
                ],
                ..Default::default()
            },
        },
        EvalCase {
            name: "proposal_is_not_a_decision",
            premise: "Something was floated and never adopted.",
            transcript: vec![
                (
                    "maybe we should launch on Friday instead, it would give marketing a clear \
week, though I'm not sure the build will be ready by then",
                    true,
                    false,
                ),
                (
                    "we could look at that, let's park it and come back once we know where QA is",
                    false,
                    true,
                ),
            ],
            notes: MeetingNotes::default(),
                        model_extraction: r#"{
  "title": "Launch Date Options",
  "meeting_type": "planning",
  "key_points": [
    {"text": "Launching on Friday instead, which would give marketing a clear week.", "kind": "proposal", "topic": "Launch timing", "source_segment_ids": ["seg_00000"]},
    {"text": "The build may not be ready by Friday.", "kind": "discussion", "topic": "Launch timing", "source_segment_ids": ["seg_00000"]}
  ],
  "topics": [{"label": "Launch timing", "segment_ids": ["seg_00000", "seg_00001"]}],
  "decisions": [],
  "action_items": [],
  "open_questions": [{"question": "Whether to launch on Friday, pending where QA is.", "source_segment_ids": ["seg_00001"]}],
  "risks": [],
  "entities": []
}"#,
            expected: Expected {
                decisions: vec![],
                open_questions: vec!["whether to launch on Friday"],
                forbidden: vec![
                    "the team decided to launch on Friday",
                    "Friday was confirmed as the launch date",
                ],
                ..Default::default()
            },
        },
        EvalCase {
            name: "unclear_owner",
            premise: "Work that needs doing, that nobody took.",
            transcript: vec![
                (
                    "someone should really review the migration script before we ship, it has not \
been looked at by anyone yet",
                    true,
                    false,
                ),
                (
                    "yes that needs to happen, we should sort that out",
                    false,
                    true,
                ),
            ],
            notes: MeetingNotes::default(),
                        model_extraction: r#"{
  "title": "Migration Script Review Gap",
  "meeting_type": "general",
  "key_points": [
    {"text": "Nobody has reviewed the migration script yet.", "kind": "discussion", "topic": "Migration readiness", "source_segment_ids": ["seg_00000"]}
  ],
  "topics": [{"label": "Migration readiness", "segment_ids": ["seg_00000", "seg_00001"]}],
  "decisions": [],
  "action_items": [
    {"description": "Review the migration script before shipping", "owner": "unassigned", "candidate_type": "action", "source_segment_ids": ["seg_00000"]}
  ],
  "open_questions": [{"question": "Who reviews the migration script.", "source_segment_ids": ["seg_00001"]}],
  "risks": [{"statement": "The migration script has not been reviewed and ship is close.", "kind": "blocker", "raised_by": "speaker_me", "source_segment_ids": ["seg_00000"]}],
  "entities": []
}"#,
            expected: Expected {
                actions: vec![ExpectedAction {
                    action: "review the migration script",
                    owner: None,
                    deadline: None,
                }],
                risks: vec!["the migration script has not been reviewed"],
                forbidden: vec![
                    "Speaker 1 accepted the migration script review",
                    "the migration script is signed off",
                ],
                ..Default::default()
            },
        },
        EvalCase {
            name: "explicit_commitment_with_date",
            premise: "A commitment with a real owner and a real date.",
            transcript: vec![
                (
                    "I'll send the revised proposal to the team by Friday, I have most of it \
drafted already",
                    true,
                    false,
                ),
                (
                    "great, and I'll get you the pricing numbers before then so you can fold them in",
                    false,
                    true,
                ),
            ],
            notes: MeetingNotes::default(),
                        model_extraction: r#"{
  "title": "Revised Proposal And Pricing",
  "meeting_type": "client_meeting",
  "key_points": [
    {"text": "The revised proposal is mostly drafted already.", "kind": "discussion", "topic": "Proposal", "source_segment_ids": ["seg_00000"]}
  ],
  "topics": [{"label": "Proposal", "segment_ids": ["seg_00000", "seg_00001"]}],
  "decisions": [],
  "action_items": [
    {"description": "Send the revised proposal to the team", "owner": "speaker_me", "deadline": "2026-08-28", "candidate_type": "action", "source_segment_ids": ["seg_00000"]},
    {"description": "Supply the pricing numbers for the proposal", "owner": "speaker_1", "candidate_type": "action", "source_segment_ids": ["seg_00001"]}
  ],
  "open_questions": [],
  "risks": [],
  "entities": []
}"#,
            expected: Expected {
                actions: vec![ExpectedAction {
                    action: "send the revised proposal to the team",
                    owner: Some("Me"),
                    deadline: Some("2026-08-28"),
                }],
                forbidden: vec![
                    "the client approved the revised proposal",
                    "the pricing numbers are final",
                ],
                ..Default::default()
            },
        },
        EvalCase {
            name: "nothing_was_settled",
            premise: "A meeting that reached no conclusion. The honest summary says so.",
            transcript: vec![
                (
                    "so where are we on the vendor question, I have looked at both and honestly \
I cannot tell them apart on the things we care about",
                    true,
                    false,
                ),
                (
                    "same here, I think we need the security review before we can say anything \
useful, let's leave it open",
                    false,
                    true,
                ),
            ],
            notes: MeetingNotes::default(),
                        model_extraction: r#"{
  "title": "Vendor Comparison Stalled",
  "meeting_type": "project_review",
  "key_points": [
    {"text": "Both vendors look equivalent on the criteria that matter here.", "kind": "discussion", "topic": "Vendor selection", "source_segment_ids": ["seg_00000"]}
  ],
  "topics": [{"label": "Vendor selection", "segment_ids": ["seg_00000", "seg_00001"]}],
  "decisions": [],
  "action_items": [],
  "open_questions": [{"question": "Which vendor to use, pending the security review.", "source_segment_ids": ["seg_00001"]}],
  "risks": [{"statement": "The security review is a dependency for any vendor choice.", "kind": "dependency", "raised_by": "speaker_1", "source_segment_ids": ["seg_00001"]}],
  "entities": []
}"#,
            expected: Expected {
                decisions: vec![],
                open_questions: vec!["which vendor to use"],
                forbidden: vec![
                    "the team selected a vendor",
                    "the security review was completed",
                ],
                ..Default::default()
            },
        },
        EvalCase {
            name: "degraded_transcript_with_noise",
            premise: "The meeting Relay actually records: greetings, a decoder loop, and one \
real decision buried in the middle.",
            transcript: vec![
                (
                    "hello hello can you hear me, yes I can hear you, sorry I was late the \
traffic was terrible today, no problem no problem, how are you doing, I am good thank you \
how about you, all good all good, can you see my screen now",
                    true,
                    false,
                ),
                (
                    "so the thing is the reporting is broken. the reporting is broken. the \
reporting is broken. the reporting is broken. anyway what I wanted to say is that we agreed \
to drop the weekly export and move everything to the dashboard instead, because maintaining \
two paths was costing us about a day a week and nobody was reading the export",
                    false,
                    true,
                ),
                (
                    "yes that makes sense. I'll turn off the export job on Monday and put a \
notice on the old link",
                    true,
                    false,
                ),
                (
                    "great, ok I think that is everything, thanks everyone, bye, bye, see you \
next week",
                    false,
                    true,
                ),
            ],
            notes: MeetingNotes::default(),
            model_extraction: r#"{
  "title": "Dropping The Weekly Export",
  "meeting_type": "project_review",
  "key_points": [
    {"text": "Maintaining both the export and the dashboard was costing about a day a week.", "kind": "discussion", "topic": "Reporting", "source_segment_ids": ["seg_00001"]},
    {"text": "Nobody was reading the weekly export.", "kind": "discussion", "topic": "Reporting", "source_segment_ids": ["seg_00001"]}
  ],
  "topics": [{"label": "Reporting", "segment_ids": ["seg_00001", "seg_00002"]}],
  "decisions": [
    {"statement": "Drop the weekly export and move reporting to the dashboard.", "rationale": "maintaining two paths cost about a day a week and nobody read the export", "decided_by": "speaker_1", "source_segment_ids": ["seg_00001"]}
  ],
  "action_items": [
    {"description": "Turn off the weekly export job", "owner": "speaker_me", "deadline": "2026-08-31", "candidate_type": "action", "source_segment_ids": ["seg_00002"]},
    {"description": "Put a notice on the old export link", "owner": "speaker_me", "candidate_type": "action", "source_segment_ids": ["seg_00002"]}
  ],
  "open_questions": [],
  "risks": [],
  "entities": [{"name": "dashboard", "kind": "product", "segment_ids": ["seg_00001"]}]
}"#,
            expected: Expected {
                decisions: vec!["drop the weekly export and move reporting to the dashboard"],
                rationale: vec!["maintaining two paths cost about a day a week"],
                actions: vec![
                    ExpectedAction {
                        action: "turn off the weekly export job",
                        owner: Some("Me"),
                        deadline: Some("2026-08-31"),
                    },
                    ExpectedAction {
                        action: "put a notice on the old export link",
                        owner: Some("Me"),
                        deadline: None,
                    },
                ],
                details: vec!["nobody was reading the export"],
                forbidden: vec!["the dashboard rollout is complete"],
                noise: vec![
                    "sorry I was late the traffic was terrible",
                    "can you see my screen now",
                    "the reporting is broken the reporting is broken",
                ],
                ..Default::default()
            },
        },
        EvalCase {
            name: "notes_carry_what_the_transcript_garbled",
            premise: "The user's own notes name a term the recogniser mangled.",
            transcript: vec![
                (
                    "the alaym placement numbers are the blocker here, we cannot report on them \
until the data is cleaned and right now half the salary figures mix lakhs and thousands",
                    true,
                    false,
                ),
                (
                    "agreed, we decided to clean the response sheet first before anything else \
goes on top of it",
                    false,
                    true,
                ),
            ],
            notes: MeetingNotes {
                during: "alumni placement data is the blocker — salary column mixes lakhs and \
thousands, must be cleaned first"
                    .to_string(),
                ..Default::default()
            },
                        model_extraction: r#"{
  "title": "Alumni Placement Data Cleanup",
  "meeting_type": "project_review",
  "key_points": [
    {"text": "Alumni placement reporting is blocked until the response data is cleaned.", "kind": "discussion", "topic": "Placement reporting", "source_segment_ids": ["seg_00000"]},
    {"text": "Half the salary figures mix lakhs and thousands.", "kind": "discussion", "topic": "Placement reporting", "source_segment_ids": ["seg_00000"]}
  ],
  "topics": [{"label": "Placement reporting", "segment_ids": ["seg_00000", "seg_00001"]}],
  "decisions": [
    {"statement": "Clean the response sheet first, before anything is built on top of it.", "rationale": "the salary column mixes lakhs and thousands and cannot be reported on", "decided_by": "speaker_1", "source_segment_ids": ["seg_00001"]}
  ],
  "action_items": [],
  "open_questions": [],
  "risks": [{"statement": "Placement numbers cannot be reported until the data is cleaned.", "kind": "blocker", "raised_by": "speaker_me", "source_segment_ids": ["seg_00000"]}],
  "entities": [{"name": "alumni", "kind": "other", "segment_ids": ["seg_00000"]}]
}"#,
            expected: Expected {
                decisions: vec!["clean the response sheet first"],
                details: vec!["salary figures mix lakhs and thousands"],
                forbidden: vec![
                    "the salary column has already been corrected",
                    "placement reporting is unblocked",
                ],
                ..Default::default()
            },
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_scorer_credits_a_summary_that_says_the_same_thing_differently() {
        // The rewrite rule requires the summary to be in its own words, so a
        // scorer that only matched verbatim would punish the correct behaviour.
        let expected = Expected {
            decisions: vec!["move the launch from Friday to Monday"],
            ..Default::default()
        };
        let paraphrased =
            "## Overview\n\n## Decisions\n\n- The launch moved from Friday to Monday.\n";
        let card = score("t", paraphrased, &expected);
        assert_eq!(card.decision_recall, 1.0);
    }

    #[test]
    fn a_missing_decision_is_measured_not_forgiven() {
        let expected = Expected {
            decisions: vec![
                "move the launch from Friday to Monday",
                "freeze the schema this sprint",
            ],
            ..Default::default()
        };
        let half = "## Overview\n\n## Decisions\n\n- The launch moved to Monday from Friday.\n";
        let card = score("t", half, &expected);
        assert!((card.decision_recall - 0.5).abs() < 0.01, "{}", card.report());
    }

    #[test]
    fn any_hallucination_takes_the_score_to_zero() {
        let expected = Expected {
            decisions: vec!["move the launch from Friday to Monday"],
            forbidden: vec!["QA signed off"],
            ..Default::default()
        };
        let good = "## Overview\n\n## Decisions\n\n- The launch moved from Friday to Monday.\n";
        let bad = "## Overview\n\n## Decisions\n\n- The launch moved from Friday to Monday.\n\
- QA signed off on the payment integration.\n";

        assert!(score("t", good, &expected).overall() > 0.9);
        let card = score("t", bad, &expected);
        assert_eq!(card.overall(), 0.0);
        assert_eq!(card.hallucinations.len(), 1);
    }

    #[test]
    fn an_invented_owner_is_caught_on_an_unowned_action() {
        let expected = Expected {
            actions: vec![ExpectedAction {
                action: "review the migration script",
                owner: None,
                deadline: None,
            }],
            ..Default::default()
        };

        let honest = "## Action Items\n\n- [ ] Review the migration script — **Unassigned**\n";
        let invented = "## Action Items\n\n- [ ] Review the migration script — **Speaker 1**\n";

        assert_eq!(score("t", honest, &expected).owner_accuracy, 1.0);
        assert_eq!(score("t", invented, &expected).owner_accuracy, 0.0);
    }

    #[test]
    fn an_invented_deadline_is_a_hallucination_not_a_low_score() {
        let expected = Expected {
            actions: vec![ExpectedAction {
                action: "look into the vendor question",
                owner: Some("Me"),
                deadline: None,
            }],
            ..Default::default()
        };
        let invented =
            "## Action Items\n\n- [ ] Look into the vendor question — **Me** · Due: 2026-08-28\n";
        let card = score("t", invented, &expected);
        assert!(!card.hallucinations.is_empty());
        assert_eq!(card.overall(), 0.0);
    }

    #[test]
    fn omitting_a_commitment_is_not_rewarded_by_owner_accuracy() {
        // Owner accuracy is measured over reported actions, so a summary that
        // reports nothing must not score a perfect 1.0 overall.
        let expected = Expected {
            actions: vec![ExpectedAction {
                action: "send the revised proposal to the team",
                owner: Some("Me"),
                deadline: Some("2026-08-28"),
            }],
            ..Default::default()
        };
        let silent = "## Overview\n\nWe talked about the proposal.\n";
        let card = score("t", silent, &expected);
        assert_eq!(card.action_recall, 0.0);
        assert!(card.overall() < 0.9, "{}", card.report());
    }

    #[test]
    fn repetition_is_measured() {
        let repeated = "## Overview\n\n- The launch moved to Monday.\n- The launch moved to Monday.\n";
        let card = score("t", repeated, &Expected::default());
        assert!(card.repetition > 0.4);
    }

    #[test]
    fn a_short_word_never_matches_a_stopword_by_prefix() {
        // "off" matching "of" is how a forbidden claim fired on a summary that
        // said the opposite of it.
        assert!(!same_content_word("off", "of"));
        assert!(!same_content_word("as", "assigned"));
        // Real inflection still matches.
        assert!(same_content_word("reviewed", "review"));
        assert!(same_content_word("clean", "cleaned"));
    }

    #[test]
    fn no_forbidden_claim_can_fire_on_a_correct_summary() {
        // A forbidden claim that is only a polarity flip of something the
        // meeting did say would fail every correct summary. Requiring enough
        // distinct content words is the cheap guard against writing one.
        for case in cases() {
            for claim in &case.expected.forbidden {
                let content = normalize(claim)
                    .split_whitespace()
                    .filter(|w| !is_stopword(w))
                    .count();
                assert!(
                    content >= 3,
                    "forbidden claim \"{}\" in case {} is too short to be distinctive",
                    claim,
                    case.name
                );
            }
        }
    }

    #[test]
    fn no_forbidden_claim_is_a_tense_flip_of_something_the_meeting_did_say() {
        // The trap this catches at authoring time: "the script was reviewed" as
        // the negation of "review the script" shares every content word with it,
        // so it fires on the correct summary and the case can never pass. A
        // forbidden claim has to be a different proposition, not the same one in
        // another tense.
        for case in cases() {
            let mut said: Vec<String> = case.expected.decisions.iter().map(|d| d.to_string()).collect();
            said.extend(case.expected.actions.iter().map(|a| a.action.to_string()));
            said.extend(case.expected.rationale.iter().map(|r| r.to_string()));
            said.extend(case.expected.open_questions.iter().map(|q| q.to_string()));
            said.extend(case.expected.risks.iter().map(|r| r.to_string()));
            said.extend(case.expected.details.iter().map(|d| d.to_string()));

            // Also checked against the transcript itself. The deterministic
            // extractor is openly extractive — it lifts sentences rather than
            // comprehending them — so a forbidden claim built out of the
            // meeting's own vocabulary fires on the honest fallback.
            let transcript = normalize(
                &case
                    .transcript
                    .iter()
                    .map(|(text, _, _)| *text)
                    .collect::<Vec<_>>()
                    .join(" "),
            );

            for claim in &case.expected.forbidden {
                assert!(
                    !contains_claim(&transcript, claim),
                    "case {}: forbidden claim \"{}\" is satisfied by the transcript itself — \
the extractive fallback will echo it. Rewrite it as a different proposition",
                    case.name,
                    claim
                );
                for real in &said {
                    assert!(
                        !contains_claim(&normalize(real), claim),
                        "case {}: forbidden claim \"{}\" is satisfied by \"{}\", which the \
meeting did say — rewrite it as a different proposition",
                        case.name,
                        claim,
                        real
                    );
                }
            }
        }
    }

    #[test]
    fn every_case_states_something_a_summary_must_not_say() {
        // A case with no forbidden claims cannot detect a hallucination, which
        // is the failure mode this set exists to measure.
        for case in cases() {
            assert!(
                !case.expected.forbidden.is_empty(),
                "case {} has nothing it forbids",
                case.name
            );
            assert!(!case.transcript.is_empty(), "case {} has no transcript", case.name);
            assert!(!case.premise.is_empty());
        }
    }

    #[test]
    fn no_cases_expectation_is_satisfied_by_an_empty_summary() {
        // Guards against a case whose bar is so low that any output passes it.
        for case in cases() {
            let card = score(case.name, "## Overview\n\nNothing happened.\n", &case.expected);
            assert!(
                card.overall() < 0.9,
                "case {} is passed by an empty summary: {}",
                case.name,
                card.report()
            );
        }
    }
}
