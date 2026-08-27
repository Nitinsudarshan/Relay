//! Validators, wired into the pipeline rather than sitting beside it.
//!
//! Model output is a proposal. These checks are what decides whether it is fit
//! to show, and their verdict has a consequence: prose with an `Error` is
//! discarded and re-rendered deterministically from the same facts, so the user
//! sees something correct instead of something plausible.
//!
//! Warnings are recorded and kept. They are the signal that summary quality is
//! drifting — visible in the processing log without changing what the user sees.

use super::model::{
    ActionItem, IssueSeverity, MeetingFacts, OwnerType, Speaker, SummaryMode, ValidationIssue,
    ValidationReport,
};
use std::collections::HashSet;

/// Overlap length, in words, at which prose counts as copied from the source.
/// Set by `Meeting-rules/meeting_transcript_summary.md`, which allows at most
/// five consecutive words in common.
const MAX_TRANSCRIPT_OVERLAP_WORDS: usize = 6;

/// A summary shorter than this is not a summary.
const MIN_SUMMARY_WORDS: usize = 8;

fn error(code: &str, message: String) -> ValidationIssue {
    ValidationIssue {
        code: code.to_string(),
        severity: IssueSeverity::Error,
        message,
    }
}

fn warning(code: &str, message: String) -> ValidationIssue {
    ValidationIssue {
        code: code.to_string(),
        severity: IssueSeverity::Warning,
        message,
    }
}

/// Validates generated prose against the facts it was supposed to come from.
///
/// `transcript_text` is the normalized transcript, used only to detect copying.
pub fn validate_summary(
    markdown: &str,
    facts: &MeetingFacts,
    speakers: &[Speaker],
    mode: SummaryMode,
    transcript_text: &str,
) -> ValidationReport {
    let mut issues = Vec::new();
    let body = markdown.trim();

    if body.is_empty() {
        return ValidationReport::from_issues(vec![error(
            "SUMMARY_EMPTY",
            "The generated summary is empty.".to_string(),
        )]);
    }

    let word_count = body.split_whitespace().count();
    if word_count < MIN_SUMMARY_WORDS {
        issues.push(error(
            "SUMMARY_TOO_SHORT",
            format!("The summary is only {} words long.", word_count),
        ));
    }
    if word_count > mode.max_words() {
        issues.push(error(
            "SUMMARY_TOO_LONG",
            format!(
                "The summary is {} words, over the {} allowed for {} mode.",
                word_count,
                mode.max_words(),
                mode.label()
            ),
        ));
    }

    if looks_like_json(body) {
        issues.push(error(
            "SUMMARY_JSON_LEAKED",
            "The summary contains raw JSON rather than prose.".to_string(),
        ));
    }

    if let Some(overlap) = longest_shared_phrase(body, transcript_text) {
        if overlap.split_whitespace().count() >= MAX_TRANSCRIPT_OVERLAP_WORDS {
            let message = format!(
                "The summary reuses {} consecutive words from the transcript: \"{}\".",
                overlap.split_whitespace().count(),
                overlap
            );
            // The rule exists to catch a model that extracted instead of
            // comprehending. The deterministic fallback is openly extractive —
            // it has no way to comprehend — and labels itself as such, so
            // copying there is a known limitation to record, not grounds for
            // rejecting the only summary available.
            issues.push(if facts.deterministic {
                warning("SUMMARY_COPIES_TRANSCRIPT", message)
            } else {
                error("SUMMARY_COPIES_TRANSCRIPT", message)
            });
        }
    }

    let duplicates = duplicate_bullets(body);
    if !duplicates.is_empty() {
        issues.push(warning(
            "SUMMARY_DUPLICATE_BULLETS",
            format!("{} bullet(s) repeat the same point.", duplicates.len()),
        ));
    }

    for invented in invented_participants(body, facts, speakers) {
        issues.push(error(
            "SUMMARY_INVENTED_PARTICIPANT",
            format!(
                "The summary names \"{}\", who is not a known speaker or entity.",
                invented
            ),
        ));
    }

    for unsupported in unsupported_decisions(body, facts) {
        issues.push(warning(
            "SUMMARY_UNSUPPORTED_DECISION",
            format!(
                "A Decisions line does not correspond to any extracted decision: \"{}\".",
                unsupported
            ),
        ));
    }

    if body.contains("· Due: ") && facts.action_items.iter().all(|a| a.deadline.is_none()) {
        issues.push(error(
            "SUMMARY_INVENTED_DEADLINE",
            "The summary shows a due date, but no action item has one.".to_string(),
        ));
    }

    ValidationReport::from_issues(issues)
}

/// Validates the structured action items themselves.
pub fn validate_action_items(items: &[ActionItem], speakers: &[Speaker]) -> ValidationReport {
    let mut issues = Vec::new();
    let known_ids: HashSet<&str> = speakers.iter().map(|s| s.id.as_str()).collect();
    let mut seen: HashSet<String> = HashSet::new();

    for item in items {
        if item.description.trim().is_empty() {
            issues.push(error(
                "ACTION_EMPTY_DESCRIPTION",
                format!("Action item {} has no description.", item.id),
            ));
        }

        let key = item.description.trim().to_lowercase();
        if !key.is_empty() && !seen.insert(key) {
            issues.push(warning(
                "ACTION_DUPLICATE",
                format!("Action item {} duplicates an earlier one.", item.id),
            ));
        }

        match item.owner_type {
            OwnerType::Me | OwnerType::Speaker => match item.owner_speaker_id.as_deref() {
                Some(id) if known_ids.contains(id) => {}
                Some(id) => issues.push(error(
                    "ACTION_UNKNOWN_OWNER",
                    format!(
                        "Action item {} is owned by unknown speaker {}.",
                        item.id, id
                    ),
                )),
                None => issues.push(error(
                    "ACTION_MISSING_OWNER_ID",
                    format!(
                        "Action item {} claims a speaker owner but carries no speaker id.",
                        item.id
                    ),
                )),
            },
            OwnerType::External => {
                if item
                    .owner_label
                    .as_deref()
                    .map(str::trim)
                    .unwrap_or("")
                    .is_empty()
                {
                    issues.push(error(
                        "ACTION_MISSING_OWNER_LABEL",
                        format!(
                            "Action item {} claims an external owner with no name.",
                            item.id
                        ),
                    ));
                }
                if item.owner_speaker_id.is_some() {
                    issues.push(error(
                        "ACTION_EXTERNAL_OWNER_HAS_SPEAKER_ID",
                        format!(
                            "Action item {} names an external owner but also claims a speaker id.",
                            item.id
                        ),
                    ));
                }
            }
            OwnerType::Group | OwnerType::Unassigned => {
                if item.owner_speaker_id.is_some() {
                    issues.push(error(
                        "ACTION_UNOWNED_HAS_SPEAKER_ID",
                        format!(
                            "Action item {} is unowned but carries a speaker id.",
                            item.id
                        ),
                    ));
                }
            }
        }

        if let Some(deadline) = item.deadline.as_deref() {
            if chrono::NaiveDate::parse_from_str(deadline, "%Y-%m-%d").is_err() {
                issues.push(error(
                    "ACTION_MALFORMED_DEADLINE",
                    format!(
                        "Action item {} has deadline \"{}\", which is not an ISO date.",
                        item.id, deadline
                    ),
                ));
            }
            if item.source_segment_ids.is_empty() {
                issues.push(error(
                    "ACTION_UNSOURCED_DEADLINE",
                    format!(
                        "Action item {} has a deadline but cites no transcript segment.",
                        item.id
                    ),
                ));
            }
        }
    }

    ValidationReport::from_issues(issues)
}

/// Removes action items that fail validation, returning the issues that caused
/// each removal.
///
/// An item whose owner cannot be resolved is not shown to the user at all: an
/// action attributed to a speaker who was never in the meeting is worse than no
/// action item. Warnings (duplicates) do not cause removal.
pub fn drop_invalid_action_items(
    items: &mut Vec<ActionItem>,
    speakers: &[Speaker],
) -> Vec<ValidationIssue> {
    let mut removed_issues = Vec::new();
    items.retain(|item| {
        let report = validate_action_items(std::slice::from_ref(item), speakers);
        if report.has_errors() {
            removed_issues.extend(
                report
                    .issues
                    .into_iter()
                    .filter(|i| i.severity == IssueSeverity::Error),
            );
            false
        } else {
            true
        }
    });
    removed_issues
}

/// Validates the speaker registry.
pub fn validate_speakers(speakers: &[Speaker]) -> ValidationReport {
    let mut issues = Vec::new();
    let mut seen_ids: HashSet<&str> = HashSet::new();
    let mut seen_names: HashSet<String> = HashSet::new();

    for speaker in speakers {
        if speaker.id.trim().is_empty() {
            issues.push(error(
                "SPEAKER_EMPTY_ID",
                "A speaker has an empty id.".to_string(),
            ));
            continue;
        }
        if !seen_ids.insert(speaker.id.as_str()) {
            issues.push(error(
                "SPEAKER_DUPLICATE_ID",
                format!("Speaker id {} appears more than once.", speaker.id),
            ));
        }
        if speaker.fallback_label.trim().is_empty() {
            issues.push(error(
                "SPEAKER_NO_LABEL",
                format!("Speaker {} has no fallback label.", speaker.id),
            ));
        }

        // Two ids sharing one name is the accidental-merge failure: the UI would
        // show one person where the data holds two.
        if let Some(name) = speaker.display_name.as_deref().map(str::trim) {
            if !name.is_empty() && !seen_names.insert(name.to_lowercase()) {
                issues.push(error(
                    "SPEAKER_NAME_COLLISION",
                    format!(
                        "More than one speaker is named \"{}\"; renaming has merged two identities.",
                        name
                    ),
                ));
            }
        }
    }

    ValidationReport::from_issues(issues)
}

/// True when the text is a JSON document rather than Markdown. Deliberately
/// narrow: a Markdown summary may legitimately contain braces in a code span.
fn looks_like_json(text: &str) -> bool {
    let trimmed = text.trim();
    if (trimmed.starts_with('{') && trimmed.ends_with('}'))
        || (trimmed.starts_with('[') && trimmed.ends_with(']'))
    {
        return true;
    }
    // A quoted-key/value pair outside a code fence is the other common leak.
    !trimmed.contains("```") && trimmed.contains("\": \"") && trimmed.matches("\": ").count() >= 2
}

/// The longest run of consecutive words shared between the summary and the
/// transcript. Detects extraction-instead-of-comprehension without needing an
/// alignment algorithm.
fn longest_shared_phrase(summary: &str, transcript: &str) -> Option<String> {
    let summary_words = comparable_words(summary);
    if summary_words.len() < MAX_TRANSCRIPT_OVERLAP_WORDS {
        return None;
    }
    let transcript_joined = format!(" {} ", comparable_words(transcript).join(" "));

    let mut longest: Option<String> = None;
    for start in 0..summary_words.len() {
        // Only phrases at or beyond the threshold matter, so start there and
        // grow; anything shorter cannot trip the check.
        let mut length = MAX_TRANSCRIPT_OVERLAP_WORDS;
        while start + length <= summary_words.len() {
            let phrase = summary_words[start..start + length].join(" ");
            if !transcript_joined.contains(&format!(" {} ", phrase)) {
                break;
            }
            longest = match &longest {
                Some(current) if current.split_whitespace().count() >= length => longest,
                _ => Some(phrase),
            };
            length += 1;
        }
    }

    longest
}

/// Lowercased, punctuation-free words, with Markdown structure removed so
/// headings and bullet markers do not affect the comparison.
fn comparable_words(text: &str) -> Vec<String> {
    text.split_whitespace()
        .map(|w| {
            w.chars()
                .filter(|c| c.is_alphanumeric() || *c == '\'')
                .collect::<String>()
                .to_lowercase()
        })
        .filter(|w| !w.is_empty())
        .collect()
}

fn bullet_lines(markdown: &str) -> Vec<String> {
    markdown
        .lines()
        .map(str::trim)
        .filter(|line| line.starts_with("- ") || line.starts_with("* "))
        .map(|line| {
            line.trim_start_matches(['-', '*', ' '])
                .trim_start_matches("[ ]")
                .trim_start_matches("[x]")
                .trim()
                .to_lowercase()
        })
        .filter(|line| !line.is_empty())
        .collect()
}

fn duplicate_bullets(markdown: &str) -> Vec<String> {
    let mut seen = HashSet::new();
    let mut duplicates = Vec::new();
    for bullet in bullet_lines(markdown) {
        if !seen.insert(bullet.clone()) {
            duplicates.push(bullet);
        }
    }
    duplicates
}

/// Capitalized names in the prose that are neither a known speaker label nor a
/// known entity.
///
/// Deliberately conservative — it only inspects owner slots (`**Name**`) and the
/// "— Name" attribution suffix, the two places the summary format puts a person.
/// Scanning all prose for capitalized words would flag every product name and
/// sentence opening.
fn invented_participants(
    markdown: &str,
    facts: &MeetingFacts,
    speakers: &[Speaker],
) -> Vec<String> {
    let mut allowed: HashSet<String> = HashSet::new();
    for speaker in speakers {
        allowed.insert(speaker.label().to_lowercase());
        allowed.insert(speaker.fallback_label.to_lowercase());
        allowed.insert(speaker.id.to_lowercase());
    }
    for entity in &facts.entities {
        allowed.insert(entity.name.to_lowercase());
    }
    for item in &facts.action_items {
        if let Some(label) = item.owner_label.as_deref() {
            allowed.insert(label.to_lowercase());
        }
    }
    // Generic owner words the format itself produces.
    for generic in [
        "unassigned",
        "the group",
        "the team",
        "everyone",
        "unknown speaker",
        "me",
    ] {
        allowed.insert(generic.to_string());
    }

    let mut found = Vec::new();
    let mut seen = HashSet::new();
    for candidate in bold_spans(markdown) {
        let key = candidate.to_lowercase();
        // Bold is also used for labels like "Topics discussed:"; only treat a
        // span as a name when it looks like one.
        if candidate.ends_with(':') || candidate.split_whitespace().count() > 3 {
            continue;
        }
        if allowed.contains(&key) || !seen.insert(key.clone()) {
            continue;
        }
        if candidate.chars().next().is_some_and(|c| c.is_uppercase()) {
            found.push(candidate);
        }
    }

    found
}

fn bold_spans(markdown: &str) -> Vec<String> {
    let mut spans = Vec::new();
    let mut rest = markdown;
    while let Some(open) = rest.find("**") {
        let after_open = &rest[open + 2..];
        match after_open.find("**") {
            Some(close) => {
                let span = after_open[..close].trim();
                if !span.is_empty() {
                    spans.push(span.to_string());
                }
                rest = &after_open[close + 2..];
            }
            None => break,
        }
    }
    spans
}

/// Decisions-section lines with no matching extracted decision.
fn unsupported_decisions(markdown: &str, facts: &MeetingFacts) -> Vec<String> {
    let mut in_decisions = false;
    let mut unsupported = Vec::new();

    for line in markdown.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("##") {
            in_decisions = trimmed.to_lowercase().contains("decision");
            continue;
        }
        if !in_decisions || !(trimmed.starts_with("- ") || trimmed.starts_with("* ")) {
            continue;
        }

        let claim = trimmed.trim_start_matches(['-', '*', ' ']).trim();
        let claim_words = comparable_words(claim);
        if claim_words.is_empty() {
            continue;
        }

        // A rewritten decision will not match verbatim, so require only that a
        // reasonable share of the claim's content words appear in some extracted
        // decision. This catches fabrication, not paraphrase.
        let supported = facts.decisions.iter().any(|decision| {
            let decision_words: HashSet<String> =
                comparable_words(&decision.statement).into_iter().collect();
            let shared = claim_words
                .iter()
                .filter(|w| w.len() > 3 && decision_words.contains(*w))
                .count();
            let significant = claim_words.iter().filter(|w| w.len() > 3).count().max(1);
            shared * 2 >= significant
        });

        if !supported {
            unsupported.push(claim.to_string());
        }
    }

    unsupported
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::meetings_v2::processing::model::{
        ActionItemStatus, Decision, Entity, EntityKind, KeyPoint, MeetingType, SegmentChannel,
        SpeakerOrigin, SPEAKER_ID_ME, SPEAKER_ID_REMOTE,
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
            segment_count: 1,
        }
    }

    fn roster() -> Vec<Speaker> {
        vec![
            speaker(SPEAKER_ID_ME, "Me", None, true),
            speaker(SPEAKER_ID_REMOTE, "Speaker 1", Some("Pranjali"), false),
        ]
    }

    fn facts() -> MeetingFacts {
        MeetingFacts {
            title: "Release Planning".into(),
            meeting_type: MeetingType::Planning,
            key_points: vec![KeyPoint {
                id: "point_0".into(),
                text: "The release date was settled.".into(),
                topic_id: None,
                source_segment_ids: vec!["seg_00000".into()],
            }],
            topics: Vec::new(),
            decisions: vec![Decision {
                id: "decision_0".into(),
                statement: "Ship the release on Friday.".into(),
                decided_by_speaker_id: Some(SPEAKER_ID_ME.into()),
                source_segment_ids: vec!["seg_00000".into()],
                confidence: 0.8,
            }],
            action_items: Vec::new(),
            open_questions: Vec::new(),
            entities: vec![Entity {
                id: "entity_0".into(),
                name: "Relay".into(),
                kind: EntityKind::Product,
                segment_ids: Vec::new(),
            }],
            speaker_ids: vec![SPEAKER_ID_ME.into()],
            deterministic: false,
        }
    }

    #[test]
    fn a_reasonable_summary_passes() {
        let markdown = "## Summary\n\n- The team settled the release date after weighing migration risk.\n\n## Decisions\n\n- Ship the release on Friday — Me\n";
        let report = validate_summary(
            markdown,
            &facts(),
            &roster(),
            SummaryMode::Standard,
            "we decided to ship the release on friday",
        );
        assert!(report.passed, "unexpected issues: {:?}", report.issues);
    }

    #[test]
    fn an_empty_summary_is_an_error() {
        let report = validate_summary("   ", &facts(), &roster(), SummaryMode::Standard, "");
        assert!(!report.passed);
        assert_eq!(report.issues[0].code, "SUMMARY_EMPTY");
    }

    #[test]
    fn an_over_long_summary_is_an_error() {
        let padding = "The team discussed the architecture at some length. ".repeat(40);
        let markdown = format!("## Summary\n\n{}", padding);
        let report = validate_summary(
            &markdown,
            &facts(),
            &roster(),
            SummaryMode::Concise,
            "unrelated transcript",
        );
        assert!(!report.passed);
        assert!(report.issues.iter().any(|i| i.code == "SUMMARY_TOO_LONG"));
    }

    #[test]
    fn leaked_json_is_an_error() {
        let markdown = r#"{"title": "Release", "summary": "we shipped"}"#;
        let report = validate_summary(markdown, &facts(), &roster(), SummaryMode::Standard, "");
        assert!(report
            .issues
            .iter()
            .any(|i| i.code == "SUMMARY_JSON_LEAKED"));
    }

    #[test]
    fn a_markdown_summary_containing_braces_in_code_is_not_flagged_as_json() {
        let markdown = "## Summary\n\n- The config change was agreed: ```{\"a\": \"b\", \"c\": \"d\"}``` stays as is for now.";
        let report = validate_summary(markdown, &facts(), &roster(), SummaryMode::Standard, "");
        assert!(!report
            .issues
            .iter()
            .any(|i| i.code == "SUMMARY_JSON_LEAKED"));
    }

    #[test]
    fn copying_the_transcript_is_an_error() {
        let transcript = "so we really need to move the migration script review to next week because nobody has looked at it";
        let markdown = format!("## Summary\n\n- {}", transcript);
        let report = validate_summary(
            &markdown,
            &facts(),
            &roster(),
            SummaryMode::Standard,
            transcript,
        );
        assert!(!report.passed);
        assert!(report
            .issues
            .iter()
            .any(|i| i.code == "SUMMARY_COPIES_TRANSCRIPT"));
    }

    #[test]
    fn a_short_incidental_overlap_is_allowed() {
        // Proper nouns and short policy phrases may legitimately overlap.
        let transcript = "we should ship the release on friday because the client expects it";
        let markdown = "## Summary\n\n- The team committed to a Friday release, driven by client expectations rather than readiness.";
        let report = validate_summary(
            markdown,
            &facts(),
            &roster(),
            SummaryMode::Standard,
            transcript,
        );
        assert!(
            !report
                .issues
                .iter()
                .any(|i| i.code == "SUMMARY_COPIES_TRANSCRIPT"),
            "issues: {:?}",
            report.issues
        );
    }

    #[test]
    fn an_invented_participant_is_an_error() {
        let markdown = "## Summary\n\n- Work was assigned.\n\n## Action Items\n\n- [ ] Send the deck — **Rajesh**\n";
        let report = validate_summary(markdown, &facts(), &roster(), SummaryMode::Standard, "");
        assert!(!report.passed);
        assert!(report
            .issues
            .iter()
            .any(|i| i.code == "SUMMARY_INVENTED_PARTICIPANT"));
    }

    #[test]
    fn a_renamed_speaker_is_not_treated_as_invented() {
        let markdown = "## Summary\n\n- Work was assigned to the reviewer.\n\n## Action Items\n\n- [ ] Send the deck — **Pranjali**\n";
        let report = validate_summary(markdown, &facts(), &roster(), SummaryMode::Standard, "");
        assert!(
            !report
                .issues
                .iter()
                .any(|i| i.code == "SUMMARY_INVENTED_PARTICIPANT"),
            "issues: {:?}",
            report.issues
        );
    }

    #[test]
    fn a_bold_section_label_is_not_mistaken_for_a_person() {
        let markdown = "## Summary\n\n**Topics discussed:** Release Planning, Schema\n\n- The release date was settled.";
        let report = validate_summary(markdown, &facts(), &roster(), SummaryMode::Standard, "");
        assert!(!report
            .issues
            .iter()
            .any(|i| i.code == "SUMMARY_INVENTED_PARTICIPANT"));
    }

    #[test]
    fn duplicate_bullets_are_a_warning_not_a_failure() {
        let markdown =
            "## Summary\n\n- The release date was settled.\n- The release date was settled.\n";
        let report = validate_summary(markdown, &facts(), &roster(), SummaryMode::Standard, "");
        assert!(report.passed, "duplicates should not block a summary");
        assert!(report
            .issues
            .iter()
            .any(|i| i.code == "SUMMARY_DUPLICATE_BULLETS"));
    }

    #[test]
    fn a_fabricated_decision_is_flagged() {
        let markdown = "## Summary\n\n- Things were discussed at length.\n\n## Decisions\n\n- Acquire a competitor in the fourth quarter\n";
        let report = validate_summary(markdown, &facts(), &roster(), SummaryMode::Standard, "");
        assert!(report
            .issues
            .iter()
            .any(|i| i.code == "SUMMARY_UNSUPPORTED_DECISION"));
    }

    #[test]
    fn a_paraphrased_decision_is_accepted() {
        let markdown = "## Summary\n\n- Timing was agreed.\n\n## Decisions\n\n- The release will ship on Friday\n";
        let report = validate_summary(markdown, &facts(), &roster(), SummaryMode::Standard, "");
        assert!(
            !report
                .issues
                .iter()
                .any(|i| i.code == "SUMMARY_UNSUPPORTED_DECISION"),
            "issues: {:?}",
            report.issues
        );
    }

    #[test]
    fn a_due_date_with_no_backing_action_item_is_an_error() {
        let markdown = "## Summary\n\n- Work was assigned to people.\n\n## Action Items\n\n- [ ] Send the deck — **Me** · Due: 2026-09-01\n";
        let report = validate_summary(markdown, &facts(), &roster(), SummaryMode::Standard, "");
        assert!(report
            .issues
            .iter()
            .any(|i| i.code == "SUMMARY_INVENTED_DEADLINE"));
    }

    fn action(id: &str, owner_type: OwnerType) -> ActionItem {
        ActionItem {
            id: id.into(),
            description: "Do the thing".into(),
            owner_type,
            owner_speaker_id: None,
            owner_label: None,
            deadline: None,
            status: ActionItemStatus::Open,
            source_segment_ids: vec!["seg_00000".into()],
            confidence: 0.8,
        }
    }

    #[test]
    fn action_items_with_valid_owners_pass() {
        let mut mine = action("action_0", OwnerType::Me);
        mine.owner_speaker_id = Some(SPEAKER_ID_ME.into());
        let unassigned = action("action_1", OwnerType::Unassigned);

        let report = validate_action_items(&[mine, unassigned], &roster());
        assert!(report.passed, "issues: {:?}", report.issues);
    }

    #[test]
    fn an_owner_who_is_not_a_known_speaker_is_an_error() {
        let mut ghost = action("action_0", OwnerType::Speaker);
        ghost.owner_speaker_id = Some("speaker_99".into());

        let report = validate_action_items(&[ghost], &roster());
        assert!(!report.passed);
        assert_eq!(report.issues[0].code, "ACTION_UNKNOWN_OWNER");
    }

    #[test]
    fn an_unassigned_item_carrying_a_speaker_id_is_an_error() {
        let mut contradictory = action("action_0", OwnerType::Unassigned);
        contradictory.owner_speaker_id = Some(SPEAKER_ID_ME.into());

        let report = validate_action_items(&[contradictory], &roster());
        assert!(report
            .issues
            .iter()
            .any(|i| i.code == "ACTION_UNOWNED_HAS_SPEAKER_ID"));
    }

    #[test]
    fn an_empty_description_is_an_error() {
        let mut blank = action("action_0", OwnerType::Unassigned);
        blank.description = "  ".into();

        let report = validate_action_items(&[blank], &roster());
        assert!(report
            .issues
            .iter()
            .any(|i| i.code == "ACTION_EMPTY_DESCRIPTION"));
    }

    #[test]
    fn a_malformed_or_unsourced_deadline_is_an_error() {
        let mut malformed = action("action_0", OwnerType::Unassigned);
        malformed.deadline = Some("next Friday".into());

        let mut unsourced = action("action_1", OwnerType::Unassigned);
        unsourced.deadline = Some("2026-09-01".into());
        unsourced.source_segment_ids.clear();

        let report = validate_action_items(&[malformed, unsourced], &roster());
        assert!(report
            .issues
            .iter()
            .any(|i| i.code == "ACTION_MALFORMED_DEADLINE"));
        assert!(report
            .issues
            .iter()
            .any(|i| i.code == "ACTION_UNSOURCED_DEADLINE"));
    }

    #[test]
    fn duplicate_action_items_are_a_warning() {
        let report = validate_action_items(
            &[
                action("action_0", OwnerType::Unassigned),
                action("action_1", OwnerType::Unassigned),
            ],
            &roster(),
        );
        assert!(report.passed);
        assert!(report.issues.iter().any(|i| i.code == "ACTION_DUPLICATE"));
    }

    #[test]
    fn a_valid_speaker_registry_passes() {
        assert!(validate_speakers(&roster()).passed);
    }

    #[test]
    fn renaming_two_speakers_to_the_same_name_is_caught_as_an_identity_merge() {
        let merged = vec![
            speaker(SPEAKER_ID_ME, "Me", Some("Nitin"), true),
            speaker(SPEAKER_ID_REMOTE, "Speaker 1", Some("Nitin"), false),
        ];
        let report = validate_speakers(&merged);
        assert!(!report.passed);
        assert_eq!(report.issues[0].code, "SPEAKER_NAME_COLLISION");
    }

    #[test]
    fn duplicate_speaker_ids_are_an_error() {
        let dupes = vec![
            speaker(SPEAKER_ID_ME, "Me", None, true),
            speaker(SPEAKER_ID_ME, "Me", None, true),
        ];
        let report = validate_speakers(&dupes);
        assert!(report
            .issues
            .iter()
            .any(|i| i.code == "SPEAKER_DUPLICATE_ID"));
    }

    #[test]
    fn an_empty_registry_is_valid_because_unknown_speakers_are_allowed() {
        assert!(validate_speakers(&[]).passed);
    }
}
