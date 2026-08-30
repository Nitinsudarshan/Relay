//! The action-item quality gate — deterministic, and shared by both extractors.
//!
//! Stage A proposes candidates. This module decides which of them are work a
//! person would actually want on a task list, and it is the only place that
//! decision is made: the model path and the cue-based path both run through it,
//! so the two can never disagree about what qualifies.
//!
//! The gate exists because a prompt cannot enforce a rule it is free to
//! reinterpret. A real meeting produced forty-nine "action items" — screen
//! shares, demo clicks, and "I'll just be back in a minute" — every one of which
//! contains an action verb and a speaker. Structural validation passed them all.
//! The rules in `Meeting-rules/meeting_action_items_tasks.md` were correct and
//! unenforced.
//!
//! ```text
//! candidates
//!     ↓ evidence          the sentence each candidate actually came from
//!     ↓ gate 1            durable after the call? (mechanics, demo narration)
//!     ↓ gate 2            is there a deliverable?
//!     ↓ gate 3            did anybody undertake it?
//!     ↓ owner resolution  never guessed; ambiguous channels become Unassigned
//!     ↓ scoring           evidence quality, not phrasing
//!     ↓ deduplication     semantic, keeping the richest version
//!     ↓ ranking + cap     at most 15, and never padded to 15
//! retained
//! ```
//!
//! The governing principle is asymmetric: a missed to-do costs one follow-up
//! message, a fabricated one costs trust in the whole feature. Every ambiguous
//! case here resolves to *drop it*.

use super::model::{ActionItem, NormalizedSegment, OwnerType};
use std::collections::{BTreeSet, HashMap};

/// The ceiling from `Meeting-rules/meeting_action_items_tasks.md` §7, enforced
/// here rather than asked for in a prompt.
///
/// It is a ceiling and never a target. A meeting with three real commitments
/// returns three.
pub const MAX_ACTION_ITEMS: usize = 15;

/// Score below which a candidate is not worth showing.
///
/// Calibrated against the weakest thing that should still qualify: an unowned
/// commitment the group made together, with a real deliverable — "we'll set up
/// a Slack channel for travel queries" — which lands exactly here. A
/// first-person undertaking with a named deliverable clears it comfortably; a
/// short or unowned fragment does not.
pub const MIN_ACCEPT_CONFIDENCE: f32 = 0.45;

/// Why a candidate did not become an action item.
///
/// Recorded per candidate so "why is this not in my list?" — and the far more
/// common "why *is* this in my list?" — are both answerable.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RejectionReason {
    /// Fulfilled before anyone hung up: screen shares, joining logistics,
    /// stepping away. Gate 1.
    MeetingMechanic,
    /// Narration of a live product walkthrough. Gate 1.
    DemoNarration,
    /// The work was reported as already done. Gate 3.
    AlreadyCompleted,
    /// "We could", "maybe later", "in version two". Gate 3.
    Hypothetical,
    /// Nothing concrete exists once it is finished. Gate 2.
    NoDeliverable,
    /// Nobody undertook it and the group did not agree to it. Gate 3.
    NoCommitment,
    /// The candidate cites no transcript segment, so nothing supports it.
    NoEvidence,
    /// Not a coherent action — a collided or truncated ASR fragment.
    BrokenFragment,
    /// A phrase the decoder emitted on a loop.
    DecoderLoop,
    /// The same commitment, already kept in a richer form.
    Duplicate,
    /// Survived the gates but the evidence is too thin to show.
    LowConfidence,
    /// Real, but past the hard cap.
    CapExceeded,
}

impl RejectionReason {
    pub fn code(self) -> &'static str {
        match self {
            Self::MeetingMechanic => "MEETING_MECHANIC",
            Self::DemoNarration => "DEMO_NARRATION",
            Self::AlreadyCompleted => "ALREADY_COMPLETED",
            Self::Hypothetical => "HYPOTHETICAL",
            Self::NoDeliverable => "NO_DELIVERABLE",
            Self::NoCommitment => "NO_COMMITMENT",
            Self::NoEvidence => "NO_EVIDENCE",
            Self::BrokenFragment => "BROKEN_FRAGMENT",
            Self::DecoderLoop => "DECODER_LOOP",
            Self::Duplicate => "DUPLICATE",
            Self::LowConfidence => "LOW_CONFIDENCE",
            Self::CapExceeded => "CAP_EXCEEDED",
        }
    }
}

/// What happened to one candidate.
///
/// `candidate_text` is transcript-derived and therefore **never** written to the
/// processing log. It exists for tests and for a developer inspecting a single
/// run in memory.
#[derive(Debug, Clone)]
pub struct CandidateDiagnostic {
    pub candidate_id: String,
    pub source_segment_ids: Vec<String>,
    pub candidate_text: String,
    pub owner: String,
    pub accepted: bool,
    pub rejection_reason: Option<RejectionReason>,
    /// True when a claimed speaker owner could not be verified from the capture
    /// channel and was demoted to `Unassigned` rather than guessed.
    pub owner_downgraded: bool,
    pub confidence: f32,
}

/// Counts only. This is the half of the diagnostics that is safe to persist and
/// to log, because it contains no meeting content.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize,
)]
pub struct ActionDiagnostics {
    pub candidates: usize,
    pub rejected: usize,
    pub deduplicated: usize,
    pub capped: usize,
    pub retained: usize,
    pub unassigned: usize,
    pub with_deadlines: usize,
    pub owners_downgraded: usize,
}

/// The full outcome of one qualification pass.
#[derive(Debug, Clone, Default)]
pub struct QualificationReport {
    pub counts: ActionDiagnostics,
    pub diagnostics: Vec<CandidateDiagnostic>,
}

impl QualificationReport {
    /// Rejection reason codes, for a test or a debug build. Order follows the
    /// candidate order, so a fixture can assert on the whole shape of a pass.
    pub fn rejection_codes(&self) -> Vec<&'static str> {
        self.diagnostics
            .iter()
            .filter_map(|d| d.rejection_reason.map(|r| r.code()))
            .collect()
    }

    pub fn rejected_for(&self, reason: RejectionReason) -> Vec<&CandidateDiagnostic> {
        self.diagnostics
            .iter()
            .filter(|d| d.rejection_reason == Some(reason))
            .collect()
    }
}

// ---------------------------------------------------------------------------
// Cue tables
// ---------------------------------------------------------------------------

/// Gate 1 — commitments discharged before anyone hung up.
///
/// Every phrase here appeared in a real meeting and became a "task". The list is
/// phrase-level rather than verb-level on purpose: "share" is a perfectly good
/// deliverable verb, and only "share my screen" is mechanics.
const MECHANIC_PHRASES: &[&str] = &[
    // Screen and presentation control
    "share my screen",
    "sharing my screen",
    "share the screen",
    "stop sharing",
    "stop the screen",
    "screen share",
    "project my screen",
    "present my screen",
    "put up my screen",
    "can you see my screen",
    "can you see the screen",
    "are you able to see",
    "you can see",
    "as you can see",
    "on this screen",
    "on the screen",
    "on my screen",
    "in this screen",
    // Turn-taking and pacing
    "speak first",
    "go first",
    "take you through",
    "take her through",
    "take him through",
    "walk you through",
    "walk through the screen",
    "move it along",
    "move along",
    "next slide",
    "next section",
    "come back to this later in the call",
    // Presence and logistics
    "be back in a minute",
    "back in a minute",
    "back in two minutes",
    "give me a minute",
    "give me a second",
    "hold on",
    "bear with me",
    "grab some water",
    "get some water",
    "step away",
    "step out",
    "step outside",
    // Joining and inviting into this call
    "join the call",
    "join this call",
    "join the meeting",
    "join us on this",
    "on this call",
    "into the call",
    "pull him in",
    "pull her in",
    "pull them in",
    "add him to the call",
    "add her to the call",
    "check if she can join",
    "check if he can join",
    "check with her to join",
    "check with him to join",
    // Live lookups and audio checks
    "check the id",
    "check the link",
    "audio check",
    "mic check",
    "can you hear me",
    "am i audible",
    "unmute",
    "mute myself",
    "stop the recording",
    "start the recording",
    // Note-taking happening right now
    "taking notes",
    "noting it down",
    "noting this down",
    "putting it in the notes",
];

/// Gate 1 — narration of a live product walkthrough.
///
/// Not a blanket ban on these verbs: "open the port" and "switch the provider"
/// are real work. A verb here only rejects when the sentence is *also*
/// pointing at something on a screen (`DEICTIC_MARKERS`) or narrating the
/// immediate next moment (`IMMEDIACY_MARKERS`).
const UI_INTERACTION_VERBS: &[&str] = &[
    "click",
    "clicking",
    "tap",
    "scroll",
    "zoom",
    "hover",
    "drag",
    "toggle",
    "refresh",
    "navigate",
    "project",
    "present",
    "display",
    "demonstrate",
    "showcase",
    "show",
    "showing",
    "highlight",
    "expand",
    "collapse",
    "select",
    "switch",
    "login",
    "sign",
    "type",
    "paste",
    "move",
    "drop",
];

/// Words that point at something visible right now.
///
/// Deliberately excludes bare "this"/"that"/"there": "I'll update the audit log
/// after this call" is ordinary phrasing, not a demo, and a generic determiner
/// alongside a verb like "switch" is not evidence of one.
const DEICTIC_MARKERS: &[&str] = &[
    "here",
    "screen",
    "tab",
    "page",
    "dashboard",
    "button",
    "field",
    "dropdown",
    "menu",
    "column",
    "row",
    "view",
];

/// Words that put a commitment inside the next few seconds rather than after
/// the meeting.
const IMMEDIACY_MARKERS: &[&str] = &["now", "just", "quickly", "right", "first", "then"];

/// Gate 3 — the work is not undertaken, only entertained.
const HYPOTHETICAL_PHRASES: &[&str] = &[
    "we could",
    "i could",
    "you could",
    "they could",
    "we might",
    "i might",
    "might be",
    "maybe we",
    "maybe i",
    "maybe later",
    "perhaps",
    "would be nice",
    "would be good",
    "nice to have",
    "good to have",
    "at some point",
    "someday",
    "some day",
    "down the line",
    "version two",
    "phase two",
    "not right now",
    "not at the moment",
    "let us park",
    "park it",
    "park this",
    "we can consider",
    "we will consider",
    "think about whether",
    "if we ever",
    "in the future",
    "later on",
    "eventually",
    "ideally",
    "in an ideal world",
];

/// Gate 3 — the work is reported as finished.
///
/// Deliberately paired forms rather than a bare "already", which is a common
/// discourse filler ("we already discussed that, and I'll send the list").
const COMPLETED_PHRASES: &[&str] = &[
    "i already",
    "we already",
    "have already",
    "has already",
    "had already",
    "already done",
    "already sent",
    "already shared",
    "already updated",
    "already configured",
    "already fixed",
    "already created",
    "i have done",
    "i've done",
    "we have done",
    "we've done",
    "i did that",
    "we did that",
    "just did",
    "is done",
    "was done",
    "has been done",
    "have been sent",
    "is already there",
];

/// Gate 2 — phrasings that name no deliverable.
///
/// A candidate matching one of these is dropped unless the description carries a
/// concrete object beyond a pronoun, which is exactly the §3.6 rule: "we will
/// tell them" qualifies only when the passage says what is being sent and to
/// whom.
const VAGUE_PHRASES: &[&str] = &[
    "help with",
    "look into",
    "looking into",
    "handle it",
    "handle this",
    "take it up",
    "take this up",
    "work on it",
    "work on this",
    "jump in",
    "chip in",
    "pitch in",
    "keep an eye",
    "think about it",
    "figure it out",
    "sort it out",
    "deal with it",
    "take care of it",
    "see to it",
    "look at it",
    "check on it",
    "get to it",
    "touch base",
    "sync up on it",
    "maintain the log",
    "maintain that log",
    "keep maintaining",
    "do the needful",
    "take it forward",
    "carry it forward",
];

/// Verbs that produce something outside the meeting. Gate 2's positive test and
/// the main scoring signal.
const DELIVERABLE_VERBS: &[&str] = &[
    "send", "share", "circulate", "forward", "email", "mail", "reply", "respond", "answer",
    "notify", "inform", "announce", "escalate", "reach", "update", "revise", "edit", "amend",
    "review", "check", "verify", "confirm", "validate", "audit", "test", "qa", "investigate",
    "analyze", "analyse", "diagnose", "debug", "create", "build", "make", "write", "draft",
    "prepare", "compile", "collect", "gather", "compose", "document", "add", "implement",
    "configure", "set", "setup", "install", "enable", "disable", "deploy", "release", "ship",
    "publish", "migrate", "integrate", "sync", "fix", "resolve", "close", "remove", "delete",
    "rename", "refactor", "clean", "decide", "finalize", "finalise", "approve", "sign",
    "schedule", "book", "arrange", "organize", "organise", "plan", "define", "clarify",
    "specify", "estimate", "budget", "procure", "order", "invoice", "pay", "renew", "cancel",
    "reschedule", "coordinate", "align", "onboard", "train", "hire", "assign", "delegate",
    "raise", "file", "track", "upload", "download", "export", "import", "provide",
    "deliver", "submit", "complete", "circulateback", "reshare", "resend", "recirculate",
];

/// Modal openings that mark a spoken commitment. Used by the cue-based
/// extractor's candidate detection and by scoring.
const FIRST_PERSON_CUES: &[&str] = &[
    "i'll",
    "i will",
    "i am going to",
    "i'm going to",
    "i can take",
    "i'll take",
    "i need to",
    "i have to",
    "i shall",
    "let me",
];

const COLLECTIVE_CUES: &[&str] = &[
    "we'll",
    "we will",
    "we are going to",
    "we're going to",
    "we need to",
    "we have to",
    "let's",
    "let us",
];

/// Acceptance tokens for §4.2 and §4.3 — assignment plus acceptance, and
/// capability answer plus group acceptance.
const ACCEPTANCE_TOKENS: &[&str] = &[
    "sure", "yes", "yeah", "okay", "ok", "great", "perfect", "aligned", "agreed", "done",
    "absolutely", "certainly", "definitely", "works",
];

/// Openings that propose work without anybody taking it.
///
/// On their own these are thinking aloud (§3.6) and Gate 3 rejects them. Paired
/// with an acceptance somewhere in the same evidence they become the group's
/// commitment — the §4.3 pattern — and the owner stays `Unassigned` unless
/// somebody actually took it.
const PROPOSAL_CUES: &[&str] = &[
    "we should",
    "we need to",
    "someone should",
    "somebody should",
    "someone needs to",
    "we can add",
    "we can do",
    "we can have",
    "can we have",
];

const ASSIGNMENT_CUES: &[&str] = &[
    "can you",
    "could you",
    "would you",
    "will you",
    "please",
    "can we have",
    "can we get",
    "can this be",
    "that can be done",
    "this can be done",
];

/// Tokens that carry no topic. Removed before a candidate is compared with
/// another for deduplication.
const STOPWORDS: &[&str] = &[
    "a", "an", "the", "and", "or", "but", "so", "then", "of", "to", "for", "with", "on", "in",
    "at", "by", "from", "into", "onto", "about", "as", "is", "are", "was", "were", "be", "been",
    "being", "do", "does", "did", "will", "would", "shall", "should", "can", "could", "may",
    "might", "must", "have", "has", "had", "it", "its", "this", "that", "these", "those", "i",
    "me", "my", "we", "us", "our", "you", "your", "he", "him", "his", "she", "her", "they",
    "them", "their", "there", "here", "what", "which", "who", "whom", "when", "where", "how",
    "all", "any", "some", "one", "also", "just", "now", "please", "okay", "ok", "yes", "no",
    "not", "up", "out", "over", "again", "back", "get", "got", "go", "going", "want", "need",
    "let", "make", "made", "thing", "things", "stuff", "really", "very", "much", "more",
    "i'll", "we'll", "let's", "i'm", "we're", "it's", "don't", "doesn't",
];

/// Verb families, so "circulate the MoM" and "send the MoM" are recognized as
/// the same commitment without a model.
const VERB_FAMILIES: &[(&str, &[&str])] = &[
    (
        "communicate",
        &[
            "send", "share", "reshare", "resend", "circulate", "recirculate", "forward",
            "email", "mail", "notify", "inform", "announce", "publish", "distribute",
            "provide", "deliver", "submit", "give", "reply", "respond", "answer",
        ],
    ),
    (
        "review",
        &[
            "review", "check", "verify", "confirm", "validate", "audit", "test", "inspect",
            "examine", "assess", "evaluate", "go", "read",
        ],
    ),
    (
        "author",
        &[
            "write", "draft", "prepare", "compose", "document", "compile", "collect",
            "gather", "create", "build", "make", "produce", "put",
        ],
    ),
    (
        "change",
        &[
            "update", "revise", "edit", "amend", "fix", "resolve", "refactor", "clean",
            "modify", "change", "adjust", "add", "remove", "delete", "rename", "implement",
        ],
    ),
    (
        "configure",
        &[
            "configure", "set", "setup", "install", "enable", "disable", "deploy", "migrate",
            "integrate", "sync", "provision",
        ],
    ),
    (
        "decide",
        &["decide", "finalize", "finalise", "approve", "sign", "confirm", "conclude"],
    ),
    (
        "schedule",
        &["schedule", "book", "arrange", "organize", "organise", "plan", "reschedule"],
    ),
];

// ---------------------------------------------------------------------------
// Entry point
// ---------------------------------------------------------------------------

/// Runs the whole gate over a candidate list.
///
/// Returns the items worth showing plus a full account of what happened to
/// every candidate. Never returns more than [`MAX_ACTION_ITEMS`], and never
/// returns an item it cannot trace to a transcript segment.
pub fn qualify_action_items(
    candidates: Vec<ActionItem>,
    segments: &[NormalizedSegment],
) -> (Vec<ActionItem>, QualificationReport) {
    let mut report = QualificationReport::default();
    report.counts.candidates = candidates.len();

    let segment_text: HashMap<&str, &str> = segments
        .iter()
        .map(|s| (s.id.as_str(), s.text.as_str()))
        .collect();
    // A commitment restated in the closing minutes is the group's own filtered
    // list (rules §4.5), so the tail of the meeting is worth extra weight.
    let closing_threshold = segments.len().saturating_sub(segments.len() / 5).max(1);
    let closing_ids: BTreeSet<&str> = segments
        .iter()
        .skip(closing_threshold)
        .map(|s| s.id.as_str())
        .collect();

    let mut survivors: Vec<Scored> = Vec::new();

    for (index, mut item) in candidates.into_iter().enumerate() {
        let candidate_id = format!("cand_{:03}", index);
        let evidence = evidence_for(&item, &segment_text);
        let mut diagnostic = CandidateDiagnostic {
            candidate_id: candidate_id.clone(),
            source_segment_ids: item.source_segment_ids.clone(),
            candidate_text: item.description.clone(),
            owner: owner_key(&item),
            accepted: false,
            rejection_reason: None,
            owner_downgraded: false,
            confidence: 0.0,
        };

        // Ownership is resolved before the gates so that a downgrade is recorded
        // even for a candidate that is then rejected for another reason.
        if downgrade_unverifiable_owner(&mut item, segments) {
            diagnostic.owner_downgraded = true;
            diagnostic.owner = owner_key(&item);
        }

        if let Err(reason) = gate(&item, &evidence) {
            diagnostic.rejection_reason = Some(reason);
            report.diagnostics.push(diagnostic);
            continue;
        }

        let confidence = score(&item, &evidence, &closing_ids);
        diagnostic.confidence = confidence;

        if confidence < MIN_ACCEPT_CONFIDENCE {
            diagnostic.rejection_reason = Some(RejectionReason::LowConfidence);
            report.diagnostics.push(diagnostic);
            continue;
        }

        item.confidence = confidence;
        survivors.push(Scored {
            item,
            diagnostic,
            order: index,
        });
    }

    // Everything discarded before this point failed a gate. Duplicates and
    // overflow are counted separately, because "we found nine and kept three"
    // and "we found nine, three were the same, and one was over the cap" are
    // different stories about the same meeting.
    report.counts.rejected = report.diagnostics.len();

    let survivors = deduplicate(survivors, &mut report);
    let retained = apply_cap(survivors, &mut report);

    report.counts.retained = retained.len();
    report.counts.unassigned = retained
        .iter()
        .filter(|i| i.owner_type == OwnerType::Unassigned)
        .count();
    report.counts.with_deadlines = retained.iter().filter(|i| i.deadline.is_some()).count();
    report.counts.owners_downgraded =
        report.diagnostics.iter().filter(|d| d.owner_downgraded).count();

    debug_assert!(retained.len() <= MAX_ACTION_ITEMS);
    (retained, report)
}

struct Scored {
    item: ActionItem,
    diagnostic: CandidateDiagnostic,
    /// Position in the candidate list, i.e. meeting order. Kept so the retained
    /// list can be restored to the order the commitments were made in after
    /// being ranked by evidence.
    order: usize,
}

// ---------------------------------------------------------------------------
// Evidence
// ---------------------------------------------------------------------------

/// What the transcript actually says behind a candidate.
struct Evidence {
    /// The sentence in the cited segments that best matches the description.
    /// Empty when the candidate cites nothing.
    sentence: String,
    /// Everything in the cited segments, for loop and acceptance detection.
    full: String,
    has_source: bool,
}

/// Picks the sentence a candidate most plausibly came from.
///
/// A 30-second chunk routinely holds a screen-share aside and a real commitment
/// in the same breath. Judging the whole chunk would reject the commitment;
/// judging the best-matching sentence judges the right thing.
fn evidence_for(item: &ActionItem, segment_text: &HashMap<&str, &str>) -> Evidence {
    let mut full = String::new();
    for id in &item.source_segment_ids {
        if let Some(text) = segment_text.get(id.as_str()) {
            full.push_str(text);
            full.push(' ');
        }
    }

    let description_words: BTreeSet<String> = content_words(&item.description);
    let mut best: Option<(usize, &str)> = None;
    for sentence in split_sentences(&full) {
        let overlap = content_words(sentence)
            .iter()
            .filter(|w| description_words.contains(*w))
            .count();
        match best {
            Some((best_overlap, _)) if best_overlap >= overlap => {}
            _ => best = Some((overlap, sentence)),
        }
    }

    Evidence {
        sentence: best.map(|(_, s)| s.to_string()).unwrap_or_default(),
        has_source: !full.trim().is_empty(),
        full,
    }
}

// ---------------------------------------------------------------------------
// The gates
// ---------------------------------------------------------------------------

/// Runs every gate in order, returning the first reason a candidate fails.
///
/// Order matters for the diagnostic, not the verdict: a screen share that is
/// also a broken fragment is reported as the fragment because that is the more
/// specific fact about it.
fn gate(item: &ActionItem, evidence: &Evidence) -> Result<(), RejectionReason> {
    if !evidence.has_source {
        return Err(RejectionReason::NoEvidence);
    }
    if is_decoder_loop(&evidence.full) || is_decoder_loop(&item.description) {
        return Err(RejectionReason::DecoderLoop);
    }
    if is_broken_fragment(&item.description) {
        return Err(RejectionReason::BrokenFragment);
    }

    // Gate 1 — durability. Applied before the verb is even noticed, which is
    // the whole point: almost every mechanic sentence contains "I'll".
    if is_meeting_mechanic(&item.description) || is_meeting_mechanic(&evidence.sentence) {
        return Err(RejectionReason::MeetingMechanic);
    }
    if is_demo_narration(&item.description) || is_demo_narration(&evidence.sentence) {
        return Err(RejectionReason::DemoNarration);
    }

    // Gate 3 — intent. Checked before Gate 2 because "we could maybe send the
    // list" has a perfectly good deliverable and still is not a commitment.
    if matches_any(&evidence.sentence, COMPLETED_PHRASES)
        || matches_any(&item.description, COMPLETED_PHRASES)
    {
        return Err(RejectionReason::AlreadyCompleted);
    }
    if matches_any(&evidence.sentence, HYPOTHETICAL_PHRASES)
        || matches_any(&item.description, HYPOTHETICAL_PHRASES)
    {
        return Err(RejectionReason::Hypothetical);
    }

    // Gate 2 — deliverable.
    if !has_deliverable(&item.description) {
        return Err(RejectionReason::NoDeliverable);
    }

    // Gate 3 — undertaking. A candidate with no first-person commitment, no
    // collective agreement, and no assignment-plus-acceptance in its evidence
    // is somebody thinking aloud.
    if !has_commitment(evidence) {
        return Err(RejectionReason::NoCommitment);
    }

    Ok(())
}

/// True when a sentence is meeting procedure rather than meeting content.
///
/// Exported because the same judgement decides what belongs in a summary: a
/// reader who missed the meeting does not need to know that somebody shared
/// their screen. Used by the extractors to keep procedural narration out of the
/// key points, and by the gate as Gate 1.
pub fn is_procedural(text: &str) -> bool {
    is_meeting_mechanic(text) || is_demo_narration(text)
}

/// True when the sentence is a commitment discharged during the call.
fn is_meeting_mechanic(text: &str) -> bool {
    matches_any(text, MECHANIC_PHRASES)
}

/// True when the sentence narrates a live product walkthrough.
///
/// Requires a UI verb *and* either something being pointed at or an immediacy
/// marker, so "switch the mail provider to SES" survives while "now I'll switch
/// to the reports tab" does not.
fn is_demo_narration(text: &str) -> bool {
    let words = words(text);
    let hay = haystack(&words);
    let has_ui_verb = UI_INTERACTION_VERBS
        .iter()
        .any(|v| has_phrase(&hay, v) || has_phrase(&hay, &format!("{}s", v)));
    if !has_ui_verb {
        return false;
    }
    let deictic = DEICTIC_MARKERS.iter().any(|m| has_phrase(&hay, m));
    let immediate = IMMEDIACY_MARKERS.iter().any(|m| has_phrase(&hay, m));
    deictic || immediate
}

/// Gate 2. True when finishing this leaves something behind.
fn has_deliverable(description: &str) -> bool {
    let words = words(description);
    let hay = haystack(&words);

    let concrete = content_words(description);
    // A vague phrasing survives only when the description also names the thing.
    if matches_any(description, VAGUE_PHRASES) {
        let named: BTreeSet<&String> = concrete
            .iter()
            .filter(|w| !VAGUE_PHRASES.iter().any(|p| p.contains(w.as_str())))
            .collect();
        if named.len() < 2 {
            return false;
        }
    }

    let has_verb = DELIVERABLE_VERBS
        .iter()
        .any(|v| has_phrase(&hay, v) || has_phrase(&hay, &format!("{}s", v)) || has_phrase(&hay, &format!("{}ing", v)));

    // A named object is required in every case; a verb with nothing to act on
    // ("follow up", "circulate") is not a task anyone can complete.
    has_verb && !concrete.is_empty()
}

/// Gate 3. True when somebody undertook the work or the group agreed to it.
fn has_commitment(evidence: &Evidence) -> bool {
    let sentence_words = words(&evidence.sentence);
    let sentence = haystack(&sentence_words);
    let full_words = words(&evidence.full);
    let full = haystack(&full_words);

    if FIRST_PERSON_CUES.iter().any(|c| has_phrase(&sentence, c)) {
        return true;
    }
    if COLLECTIVE_CUES.iter().any(|c| has_phrase(&sentence, c)) {
        return true;
    }
    // §4.2 / §4.3 — a request or a proposal somewhere in the evidence, answered
    // with an acceptance. Neither half qualifies alone: a proposal nobody
    // answered is thinking aloud, and an acceptance token with nothing to
    // accept is just somebody saying "okay".
    let proposed = ASSIGNMENT_CUES.iter().any(|c| has_phrase(&full, c))
        || PROPOSAL_CUES.iter().any(|c| has_phrase(&full, c));
    let accepted = ACCEPTANCE_TOKENS.iter().any(|t| has_phrase(&full, t));
    proposed && accepted
}

/// A phrase repeated three or more times in immediate succession is a decoder
/// artifact, however task-like it reads.
fn is_decoder_loop(text: &str) -> bool {
    let words = words(text);
    if words.len() < 6 {
        return false;
    }
    // Whisper's loops repeat whole clauses, not just short phrases: "I will pay
    // the firm to fill the form" is nine words and repeated nine times.
    for length in 1..=12usize {
        if words.len() < length * 3 {
            break;
        }
        for start in 0..=(words.len() - length * 3) {
            let first = &words[start..start + length];
            let second = &words[start + length..start + length * 2];
            let third = &words[start + length * 2..start + length * 3];
            if first == second && second == third {
                return true;
            }
        }
    }
    false
}

/// True when the text does not parse as a coherent action.
///
/// Never repairs one — §3.4 is explicit that a fragment is discarded rather than
/// reconstructed.
fn is_broken_fragment(description: &str) -> bool {
    let words = words(description);
    if words.len() < 3 {
        return true;
    }
    // Trailing function words mean the sentence was cut off.
    const DANGLING: &[&str] = &[
        "and", "or", "the", "a", "an", "to", "for", "with", "that", "of", "in", "on", "but",
        "so", "if", "as", "at", "by", "from", "is", "are", "was", "were", "will", "we", "i",
    ];
    if words
        .last()
        .is_some_and(|w| DANGLING.contains(&w.as_str()))
    {
        return true;
    }
    // A modal with no verb behind it — "we will the specialty has also joined
    // in" — is two collided fragments, not an action.
    let hay = haystack(&words);
    for cue in ["i'll", "we'll", "i will", "we will"] {
        if let Some(position) = hay.find(&format!(" {} ", cue)) {
            let tail: Vec<&str> = hay[position + cue.len() + 2..]
                .split_whitespace()
                .take(3)
                .collect();
            if !tail.is_empty() && !tail.iter().any(|w| looks_like_a_verb(w)) {
                return true;
            }
        }
    }
    false
}

// ---------------------------------------------------------------------------
// Owner resolution
// ---------------------------------------------------------------------------

/// True when a token could plausibly be a verb.
///
/// Deliberately generous. This backs the fragment check, whose job is to catch
/// "we will the specialty has also joined in" — a modal with no verb at all —
/// not to decide whether the verb names real work. Gate 2 does that.
fn looks_like_a_verb(word: &str) -> bool {
    const AUXILIARIES: &[&str] = &[
        "be", "been", "being", "have", "has", "get", "got", "go", "do", "keep", "make",
        "take", "put", "give", "let", "start", "stop", "try", "need", "want", "come",
        "bring", "work", "look", "help", "run", "move", "call", "meet", "talk", "speak",
        "discuss", "follow", "continue", "finish", "handle", "maintain", "ensure", "use",
    ];
    let stemmed = stem(word);
    DELIVERABLE_VERBS.contains(&word)
        || DELIVERABLE_VERBS.contains(&stemmed.as_str())
        || UI_INTERACTION_VERBS.contains(&word)
        || AUXILIARIES.contains(&word)
        || AUXILIARIES.contains(&stemmed.as_str())
        || VERB_FAMILIES
            .iter()
            .any(|(_, verbs)| verbs.contains(&word) || verbs.contains(&stemmed.as_str()))
        || word.ends_with("ing")
        || word.ends_with("ate")
        || word.ends_with("ize")
        || word.ends_with("ise")
        || word.ends_with("ify")
}

/// Demotes a speaker owner that the capture channel cannot support.
///
/// Attribution here is channel-level, not diarization. When every segment a
/// candidate cites had both the microphone and system audio live, nothing in the
/// data says who spoke, and `Unassigned` is the honest answer — §12 of the
/// task brief and §7 of `meeting_speaker_identification.md`.
fn downgrade_unverifiable_owner(item: &mut ActionItem, segments: &[NormalizedSegment]) -> bool {
    if !matches!(item.owner_type, OwnerType::Me | OwnerType::Speaker) {
        return false;
    }
    if item.source_segment_ids.is_empty() {
        return false;
    }
    let any_attributed = segments
        .iter()
        .filter(|s| item.source_segment_ids.contains(&s.id))
        .any(|s| s.speaker_id.is_some());
    if any_attributed {
        return false;
    }

    item.owner_type = OwnerType::Unassigned;
    item.owner_speaker_id = None;
    item.owner_label = None;
    true
}

// ---------------------------------------------------------------------------
// Scoring
// ---------------------------------------------------------------------------

/// How much the evidence supports this candidate, in `0.0..=1.0`.
///
/// Never shown to the user. It exists to rank candidates against each other and
/// to draw the line under [`MIN_ACCEPT_CONFIDENCE`].
fn score(item: &ActionItem, evidence: &Evidence, closing_ids: &BTreeSet<&str>) -> f32 {
    let sentence_words = words(&evidence.sentence);
    let sentence = haystack(&sentence_words);
    let full_words = words(&evidence.full);
    let full = haystack(&full_words);
    let description_words = words(&item.description);
    let description = haystack(&description_words);

    let mut score = 0.20f32;

    // How firmly somebody took this on. The strongest form counts; a second
    // form adds a little on top, because being both undertaken and accepted is
    // better evidence than either alone.
    let first_person = FIRST_PERSON_CUES.iter().any(|c| has_phrase(&sentence, c));
    let collective = COLLECTIVE_CUES.iter().any(|c| has_phrase(&sentence, c));
    let accepted_assignment = (ASSIGNMENT_CUES.iter().any(|c| has_phrase(&full, c))
        || PROPOSAL_CUES.iter().any(|c| has_phrase(&full, c)))
        && ACCEPTANCE_TOKENS.iter().any(|t| has_phrase(&full, t));

    let strongest = [
        first_person.then_some(0.25),
        accepted_assignment.then_some(0.20),
        collective.then_some(0.15),
    ]
    .into_iter()
    .flatten()
    .fold(0.0f32, f32::max);
    score += strongest;
    if [first_person, collective, accepted_assignment]
        .iter()
        .filter(|present| **present)
        .count()
        > 1
    {
        score += 0.05;
    }

    // Who owns it. Never a reason to invent one — this only rewards an owner
    // the evidence already supported.
    score += match item.owner_type {
        OwnerType::Me | OwnerType::Speaker => 0.15,
        OwnerType::External => 0.05,
        OwnerType::Group => 0.0,
        OwnerType::Unassigned => -0.05,
    };

    // What is produced.
    if DELIVERABLE_VERBS.iter().any(|v| {
        has_phrase(&description, v) || has_phrase(&description, &format!("{}s", v))
    }) {
        score += 0.15;
    }
    if item.deadline.is_some() {
        score += 0.10;
    }

    // Where it was said. The closing recap is the group's own filtered list.
    if item
        .source_segment_ids
        .iter()
        .any(|id| closing_ids.contains(id.as_str()))
    {
        score += 0.05;
    }
    // Committed to more than once.
    if item.source_segment_ids.len() > 1 {
        score += 0.05;
    }
    if description_words.len() < 4 {
        score -= 0.10;
    }

    score.clamp(0.0, 1.0)
}

// ---------------------------------------------------------------------------
// Deduplication
// ---------------------------------------------------------------------------

/// Collapses restatements of the same commitment, keeping the richest version.
///
/// Semantic rather than string-equal: "I'll send the mail list" and "I'll share
/// the required email list" are one to-do. Two commitments that merely share a
/// word are not.
fn deduplicate(candidates: Vec<Scored>, report: &mut QualificationReport) -> Vec<Scored> {
    let mut kept: Vec<Scored> = Vec::new();

    for candidate in candidates {
        let signature = content_words(&candidate.item.description);
        let families = verb_families(&candidate.item.description);

        let duplicate_of = kept.iter().position(|existing| {
            if !owners_compatible(&existing.item, &candidate.item) {
                return false;
            }
            let existing_signature = content_words(&existing.item.description);
            if jaccard(&signature, &existing_signature) >= 0.55 {
                return true;
            }
            // The same kind of act on the same distinctive object — "circulate
            // the MoM" and "reshare the link and send the MoM". Requiring both
            // the family and a shared object keeps two unrelated commitments
            // that happen to share a word apart.
            let existing_families = verb_families(&existing.item.description);
            if families.is_disjoint(&existing_families) {
                return false;
            }
            // The shared token has to be the *object*. Two candidates that share
            // only the verb — "update the migration plan" and "update the
            // rollback script" — are two tasks, not one.
            let objects = object_words(&candidate.item.description);
            let existing_objects = object_words(&existing.item.description);
            objects
                .intersection(&existing_objects)
                .any(|word| word.len() >= 3)
        });

        let Some(index) = duplicate_of else {
            kept.push(candidate);
            continue;
        };

        // Merge first, decide which text to keep second. Every merged field is
        // read from both sides before either is moved.
        let mut sources = kept[index].item.source_segment_ids.clone();
        for id in &candidate.item.source_segment_ids {
            if !sources.contains(id) {
                sources.push(id.clone());
            }
        }
        // A commitment restated later in the meeting is stronger evidence for
        // it, not weaker.
        let confidence =
            (kept[index].item.confidence.max(candidate.item.confidence) + 0.05).min(1.0);
        let deadline = kept[index]
            .item
            .deadline
            .clone()
            .or_else(|| candidate.item.deadline.clone());
        let owner = if kept[index].item.owner_type != OwnerType::Unassigned {
            &kept[index].item
        } else {
            &candidate.item
        };
        let (owner_type, owner_speaker_id, owner_label) = (
            owner.owner_type,
            owner.owner_speaker_id.clone(),
            owner.owner_label.clone(),
        );
        let order = kept[index].order.min(candidate.order);

        // Detail wins: an owner plus a date beats an owner alone (rules §7).
        let dropped = if is_richer(&candidate, &kept[index]) {
            std::mem::replace(&mut kept[index], candidate).diagnostic
        } else {
            candidate.diagnostic
        };

        let winner = &mut kept[index];
        winner.item.source_segment_ids = sources;
        winner.item.confidence = confidence;
        winner.item.deadline = deadline;
        winner.item.owner_type = owner_type;
        winner.item.owner_speaker_id = owner_speaker_id;
        winner.item.owner_label = owner_label;
        winner.order = order;

        let mut dropped = dropped;
        dropped.rejection_reason = Some(RejectionReason::Duplicate);
        report.diagnostics.push(dropped);
        report.counts.deduplicated += 1;
    }

    kept
}

/// Two candidates may be the same to-do only if their owners do not contradict.
/// An unassigned item can merge into an owned one; two different owners cannot.
fn owners_compatible(a: &ActionItem, b: &ActionItem) -> bool {
    if a.owner_type == OwnerType::Unassigned || b.owner_type == OwnerType::Unassigned {
        return true;
    }
    owner_key(a) == owner_key(b)
}

/// Which of two restatements to keep. Detail wins: an owner plus a date beats an
/// owner alone (§7).
fn is_richer(candidate: &Scored, existing: &Scored) -> bool {
    let rank = |s: &Scored| {
        (
            s.item.deadline.is_some() as u8,
            (s.item.owner_type != OwnerType::Unassigned) as u8,
            s.item.source_segment_ids.len(),
            s.item.description.split_whitespace().count(),
        )
    };
    rank(candidate) > rank(existing)
}

// ---------------------------------------------------------------------------
// Ranking and the cap
// ---------------------------------------------------------------------------

/// Keeps at most [`MAX_ACTION_ITEMS`], preferring the best-evidenced.
///
/// The list is then restored to meeting order, because a reader follows the
/// meeting, not the ranking.
fn apply_cap(mut candidates: Vec<Scored>, report: &mut QualificationReport) -> Vec<ActionItem> {
    candidates.sort_by(|a, b| {
        b.item
            .confidence
            .partial_cmp(&a.item.confidence)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| {
                (b.item.owner_type != OwnerType::Unassigned)
                    .cmp(&(a.item.owner_type != OwnerType::Unassigned))
            })
            .then_with(|| b.item.deadline.is_some().cmp(&a.item.deadline.is_some()))
            .then_with(|| a.order.cmp(&b.order))
    });

    let overflow = candidates.split_off(candidates.len().min(MAX_ACTION_ITEMS));
    report.counts.capped = overflow.len();
    for dropped in overflow {
        let mut diagnostic = dropped.diagnostic;
        diagnostic.rejection_reason = Some(RejectionReason::CapExceeded);
        report.diagnostics.push(diagnostic);
    }

    candidates.sort_by_key(|c| c.order);
    candidates
        .into_iter()
        .enumerate()
        .map(|(index, mut scored)| {
            scored.item.id = format!("action_{}", index);
            scored.diagnostic.accepted = true;
            scored.diagnostic.confidence = scored.item.confidence;
            report.diagnostics.push(scored.diagnostic);
            scored.item
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Text helpers
// ---------------------------------------------------------------------------

/// Splits on sentence terminators. Shared with the cue-based extractor so both
/// see the same units.
pub fn split_sentences(text: &str) -> Vec<&str> {
    text.split_inclusive(['.', '!', '?'])
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .collect()
}

/// Lowercased word tokens. Apostrophes are kept so "i'll" stays one token and
/// can be matched as a whole word rather than as a substring of "will".
fn words(text: &str) -> Vec<String> {
    text.split(|c: char| !(c.is_alphanumeric() || c == '\'' || c == '\u{2019}'))
        .filter(|w| !w.is_empty())
        .map(|w| w.replace('\u{2019}', "'").to_lowercase())
        .collect()
}

/// Space-delimited on both ends, so a phrase search matches whole words only.
fn haystack(words: &[String]) -> String {
    format!(" {} ", words.join(" "))
}

fn has_phrase(haystack: &str, phrase: &str) -> bool {
    haystack.contains(&format!(" {} ", phrase))
}

fn matches_any(text: &str, phrases: &[&str]) -> bool {
    let words = words(text);
    let hay = haystack(&words);
    phrases.iter().any(|p| has_phrase(&hay, p))
}

/// Topic-bearing words, lightly stemmed so "mails" and "mail" compare equal.
fn content_words(text: &str) -> BTreeSet<String> {
    words(text)
        .into_iter()
        .filter(|w| !STOPWORDS.contains(&w.as_str()) && w.len() > 2)
        .map(|w| stem(&w))
        .collect()
}

/// Conservative suffix stripping. Enough to match plurals and gerunds, not
/// enough to collide unrelated words.
fn stem(word: &str) -> String {
    for suffix in ["ings", "ing", "ies", "es", "ed", "s"] {
        if word.len() > suffix.len() + 3 && word.ends_with(suffix) {
            let base = &word[..word.len() - suffix.len()];
            return if suffix == "ies" {
                format!("{}y", base)
            } else {
                base.to_string()
            };
        }
    }
    word.to_string()
}

/// Content words with the verbs removed — what the action is *about*.
fn object_words(text: &str) -> BTreeSet<String> {
    content_words(text)
        .into_iter()
        .filter(|word| {
            !DELIVERABLE_VERBS.contains(&word.as_str())
                && !UI_INTERACTION_VERBS.contains(&word.as_str())
                && !VERB_FAMILIES
                    .iter()
                    .any(|(_, verbs)| verbs.contains(&word.as_str()))
        })
        .collect()
}

fn jaccard(a: &BTreeSet<String>, b: &BTreeSet<String>) -> f32 {
    if a.is_empty() || b.is_empty() {
        return 0.0;
    }
    let intersection = a.intersection(b).count() as f32;
    let union = a.union(b).count() as f32;
    intersection / union
}

/// Every verb family present in the text.
///
/// All of them, not the first one found: a restatement like "just to confirm,
/// I'll send you the list" leads with a verb from one family and carries the
/// real one later, and first-match-wins would call that a different kind of act
/// from "I'll send the list".
fn verb_families(text: &str) -> BTreeSet<&'static str> {
    let mut found = BTreeSet::new();
    for word in words(text) {
        let stemmed = stem(&word);
        for (family, verbs) in VERB_FAMILIES {
            if verbs.contains(&word.as_str()) || verbs.contains(&stemmed.as_str()) {
                found.insert(*family);
            }
        }
    }
    found
}

/// A stable key for "the same owner", used by deduplication.
pub fn owner_key(item: &ActionItem) -> String {
    match item.owner_type {
        OwnerType::Me | OwnerType::Speaker => item
            .owner_speaker_id
            .clone()
            .unwrap_or_else(|| "unassigned".to_string()),
        OwnerType::External => item
            .owner_label
            .as_deref()
            .unwrap_or("unassigned")
            .to_lowercase(),
        OwnerType::Group => "group".to_string(),
        OwnerType::Unassigned => "unassigned".to_string(),
    }
}

#[cfg(test)]
mod tests;
