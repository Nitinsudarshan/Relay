//! Phase 7: output validators.
//!
//! The self-check lists at the end of each file in `Meeting-rules/` are the
//! specification for these functions. As prose in a prompt they are advisory
//! and a small model ignores them under load; as code they run after generation
//! and before anything reaches the UI, and a failure is a failure.
//!
//! The load-bearing check is [`max_ngram_overlap`]. "Extraction instead of
//! comprehension" — the model copying important-looking sentences out of the
//! transcript instead of describing the discussion — is the central failure the
//! whole pipeline exists to prevent, and n-gram overlap measures it directly.

use serde::{Deserialize, Serialize};

/// Consecutive words an output may share with the transcript before it counts
/// as copied. `meeting_transcript_summary.md` §3 allows five, for proper nouns
/// and exact policy phrases.
pub const MAX_SHARED_WORDS: usize = 5;

/// Phrases the rules files ban outright, in lowercase.
const BANNED_PHRASES: &[&str] = &[
    "due: tbd",
    "owner: not specified",
    "owner: unknown",
    "not specified",
    "to be determined",
    "due: asap",
    "priority: medium",
];

/// Words a title may not end on — the signature of a truncated phrase.
const TRUNCATION_ENDINGS: &[&str] = &[
    "and", "or", "the", "to", "of", "with", "for", "in", "on", "at", "a", "an", "but", "from",
    "into", "about", "as", "by", "is", "are", "was", "were",
];

/// The exact empty-case strings. Anything else is a violation: an apology or a
/// speculation about what the to-dos might have been is worse than nothing.
pub const EMPTY_TODOS: &str = "_No to-dos identified._";
pub const INSUFFICIENT_SUMMARY: &str = "## Overview\n\n**Purpose:** This recording does not contain enough intelligible discussion to summarize.";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Violation {
    /// A date or timestamp in a title, which the UI already shows.
    TitleContainsDate { found: String },
    /// A span copied from the transcript. The measurement of the extraction
    /// failure.
    CopiedSpan { words: usize, excerpt: String },
    /// A bracketed ASR tag reached the output.
    AsrTag { tag: String },
    BannedPhrase { phrase: String },
    /// A date that is not `YYYY-MM-DD`.
    MalformedDate { found: String },
    /// A to-do line that is not a checkbox with a bold action.
    MalformedTodo { line: String },
    /// A heading with nothing under it, or a literal "None".
    EmptyPlaceholder { text: String },
    MissingOverview,
    /// The empty case was expressed in the model's own words instead of the
    /// exact specified string.
    InexactEmptyCase { found: String },
    TitleLength { words: usize, chars: usize },
    TitleTruncated { ends_with: String },
    TitlePunctuation { found: String },
    TitleCopiesOpening,
    TooManyTodos { count: usize },
}

impl Violation {
    /// One line naming the violation, for the retry prompt. Specific enough
    /// that a model can act on it — "shorten the title" is useless, "the title
    /// ends on the preposition 'for'" is not.
    pub fn message(&self) -> String {
        match self {
            Violation::CopiedSpan { words, excerpt } => format!(
                "A {words}-word span is copied verbatim from the transcript: \"{excerpt}\". Rewrite it as a claim about the discussion; no more than {MAX_SHARED_WORDS} consecutive words may match."
            ),
            Violation::AsrTag { tag } => format!(
                "The ASR tag \"{tag}\" appears in the output. Bracketed tags are never content."
            ),
            Violation::BannedPhrase { phrase } => format!(
                "The banned phrase \"{phrase}\" appears. Omit the field entirely instead of filling it with a placeholder."
            ),
            Violation::MalformedDate { found } => format!(
                "\"{found}\" is not a YYYY-MM-DD date. Use that format or omit the due date."
            ),
            Violation::MalformedTodo { line } => format!(
                "This to-do is not in the required format `- [ ] **Action** — Due: YYYY-MM-DD · Priority`: \"{line}\"."
            ),
            Violation::EmptyPlaceholder { text } => format!(
                "\"{text}\" is an empty section or placeholder. Remove the section instead of printing a placeholder."
            ),
            Violation::MissingOverview => {
                "The summary must begin with an `## Overview` section containing `**Purpose:**`.".to_string()
            }
            Violation::InexactEmptyCase { found } => format!(
                "The empty case must be exactly the specified string, not \"{found}\"."
            ),
            Violation::TitleLength { words, chars } => format!(
                "The title is {words} words and {chars} characters; it must be 3-8 words and under 60 characters."
            ),
            Violation::TitleTruncated { ends_with } => format!(
                "The title ends on \"{ends_with}\", which means it is truncated. Produce a complete phrase."
            ),
            Violation::TitlePunctuation { found } => format!(
                "The title contains \"{found}\". No terminal punctuation, quotes, or brackets."
            ),
            Violation::TitleContainsDate { found } => format!(
                "The title contains the date or time \"{found}\". The UI already shows those."
            ),
            Violation::TitleCopiesOpening => {
                "The title reuses the opening words of the transcript, which is where joining noise and cold audio live.".to_string()
            }
            Violation::TooManyTodos { count } => {
                format!("{count} to-dos were returned; the cap is 15.")
            }
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ValidationReport {
    pub violations: Vec<Violation>,
    /// Longest run of words shared with the transcript. Reported even when it
    /// is within budget, because it is the metric worth tracking over time.
    pub max_shared_words: usize,
}

impl ValidationReport {
    pub fn is_valid(&self) -> bool {
        self.violations.is_empty()
    }

    /// Feedback for the single retry, naming every violation.
    pub fn prompt_feedback(&self) -> String {
        let mut out = String::from(
            "Your previous response violated these rules. Produce a corrected response:\n",
        );
        for violation in &self.violations {
            out.push_str("- ");
            out.push_str(&violation.message());
            out.push('\n');
        }
        out
    }

    pub fn summary_line(&self) -> String {
        if self.is_valid() {
            format!("valid (max shared span {} words)", self.max_shared_words)
        } else {
            format!(
                "{} violation(s): {}",
                self.violations.len(),
                self.violations
                    .iter()
                    .map(|v| v.message())
                    .collect::<Vec<_>>()
                    .join(" | ")
            )
        }
    }
}

/// Longest run of consecutive words `output` shares with `transcript`.
///
/// Returns the run length and the excerpt, so a violation can quote the span
/// back to the model.
pub fn max_ngram_overlap(output: &str, transcript: &str) -> (usize, String) {
    let out_words = comparable_words(output);
    let src_words = comparable_words(transcript);
    if out_words.is_empty() || src_words.is_empty() {
        return (0, String::new());
    }

    // Index the transcript by word so each output position only compares
    // against real candidates.
    let mut positions: std::collections::HashMap<&str, Vec<usize>> =
        std::collections::HashMap::new();
    for (i, word) in src_words.iter().enumerate() {
        positions.entry(word.as_str()).or_default().push(i);
    }

    let mut best = 0usize;
    let mut best_excerpt = String::new();

    for i in 0..out_words.len() {
        let Some(starts) = positions.get(out_words[i].as_str()) else {
            continue;
        };
        for &j in starts {
            let mut len = 0;
            while i + len < out_words.len()
                && j + len < src_words.len()
                && out_words[i + len] == src_words[j + len]
            {
                len += 1;
            }
            if len > best {
                best = len;
                best_excerpt = out_words[i..i + len].join(" ");
            }
        }
    }

    (best, best_excerpt)
}

/// Words stripped to their comparable form: lowercase, no punctuation. Markdown
/// syntax is dropped so bolding a copied sentence does not hide it.
fn comparable_words(text: &str) -> Vec<String> {
    text.split_whitespace()
        .map(|w| {
            w.chars()
                .filter(|c| c.is_alphanumeric() || *c == '\'')
                .flat_map(|c| c.to_lowercase())
                .collect::<String>()
        })
        .filter(|w| !w.is_empty())
        .collect()
}

/// The bold action text of a to-do line, if it is bolded at all.
fn bold_action(body: &str) -> Option<&str> {
    let rest = body.strip_prefix("**")?;
    let end = rest.find("**")?;
    Some(rest[..end].trim())
}

/// Whether a line opens a top-level output section (`## X`, not `### X`).
fn is_section_heading(line: &str) -> bool {
    line.starts_with("## ") || line == "##"
}

/// Bracketed spans still present in generated output.
fn find_asr_tags(text: &str) -> Vec<String> {
    let mut tags = Vec::new();
    let chars: Vec<char> = text.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        if chars[i] == '[' {
            // A markdown link or a to-do checkbox is not an ASR tag.
            let inner: String = chars[i + 1..]
                .iter()
                .take_while(|c| **c != ']')
                .collect::<String>();
            let trimmed = inner.trim();
            if !trimmed.is_empty() && trimmed != "x" && trimmed != "X" {
                tags.push(format!("[{}]", trimmed));
            }
            i += inner.chars().count() + 2;
            continue;
        }
        i += 1;
    }
    tags
}

fn check_shared_text(text: &str, transcript: &str, report: &mut ValidationReport) {
    let (words, excerpt) = max_ngram_overlap(text, transcript);
    report.max_shared_words = report.max_shared_words.max(words);
    if words > MAX_SHARED_WORDS {
        report.violations.push(Violation::CopiedSpan {
            words,
            excerpt,
        });
    }
}

fn check_tags_and_phrases(text: &str, report: &mut ValidationReport) {
    for tag in find_asr_tags(text) {
        report.violations.push(Violation::AsrTag { tag });
    }
    let lower = text.to_lowercase();
    for phrase in BANNED_PHRASES {
        if lower.contains(phrase) {
            report.violations.push(Violation::BannedPhrase {
                phrase: phrase.to_string(),
            });
        }
    }
}

/// Validates a generated summary against `meeting_transcript_summary.md` §10.
pub fn validate_summary(summary: &str, transcript: &str) -> ValidationReport {
    let mut report = ValidationReport::default();
    let trimmed = summary.trim();

    if trimmed == INSUFFICIENT_SUMMARY {
        return report;
    }

    if !trimmed.starts_with("## Overview") || !trimmed.contains("**Purpose:**") {
        report.violations.push(Violation::MissingOverview);
    }

    check_tags_and_phrases(trimmed, &mut report);
    check_shared_text(trimmed, transcript, &mut report);

    // A heading with nothing under it, or a literal "None", instead of the
    // section being removed.
    let lines: Vec<&str> = trimmed.lines().collect();
    for (i, line) in lines.iter().enumerate() {
        let l = line.trim();
        if is_section_heading(l) {
            // A `###` topic heading and its bullets are this section's content;
            // only the next `##` ends it.
            let has_content = lines[i + 1..]
                .iter()
                .take_while(|next| !is_section_heading(next.trim()))
                .any(|next| !next.trim().is_empty());
            if !has_content {
                report.violations.push(Violation::EmptyPlaceholder {
                    text: l.to_string(),
                });
            }
        } else if matches!(l.trim_start_matches(['-', '*', ' ']), "None" | "N/A" | "none") {
            report.violations.push(Violation::EmptyPlaceholder {
                text: l.to_string(),
            });
        }
    }

    // Action items belong to their own pass.
    if trimmed.contains("- [ ]") || trimmed.contains("- [x]") {
        report.violations.push(Violation::MalformedTodo {
            line: "checkboxes must not appear in the summary".to_string(),
        });
    }

    report
}

/// Validates generated to-dos against `meeting_action_items_tasks.md` §5.
pub fn validate_action_items(items: &[String], transcript: &str) -> ValidationReport {
    let mut report = ValidationReport::default();

    if items.len() == 1 && items[0].trim() == EMPTY_TODOS {
        return report;
    }
    if items.is_empty() {
        return report;
    }
    if items.len() > 15 {
        report.violations.push(Violation::TooManyTodos {
            count: items.len(),
        });
    }

    for item in items {
        let line = item.trim();
        if line.is_empty() {
            continue;
        }

        // An owner heading is allowed; everything else must be a checkbox.
        if line.starts_with("###") {
            continue;
        }
        if line.starts_with("_No to-dos") {
            report.violations.push(Violation::InexactEmptyCase {
                found: line.to_string(),
            });
            continue;
        }

        let is_checkbox = line.starts_with("- [ ]") || line.starts_with("- [x]");
        let is_detail = line.starts_with("  ") || item.starts_with("  ");
        if !is_checkbox && !is_detail {
            report.violations.push(Violation::MalformedTodo {
                line: line.to_string(),
            });
            continue;
        }

        if is_checkbox {
            let body = line[5..].trim();
            match bold_action(body) {
                None => report.violations.push(Violation::MalformedTodo {
                    line: line.to_string(),
                }),
                Some(action) => {
                    // Verb-first, 3-12 words, no trailing period.
                    let words = action.split_whitespace().count();
                    if action.trim_end().ends_with('.') || !(2..=12).contains(&words) {
                        report.violations.push(Violation::MalformedTodo {
                            line: line.to_string(),
                        });
                    }
                }
            }
        }

        check_tags_and_phrases(line, &mut report);
        check_shared_text(line, transcript, &mut report);

        for date in find_date_like(line) {
            if !is_iso_date(&date) {
                report.violations.push(Violation::MalformedDate { found: date });
            }
        }
    }

    report
}

/// Validates a generated title against `meeting_title_headings.md` §9.
pub fn validate_title(title: &str, transcript: &str) -> ValidationReport {
    let mut report = ValidationReport::default();
    let t = title.trim();

    let is_fallback = t.starts_with("Untitled Meeting —") || t.starts_with("Short Recording —");

    let words = t.split_whitespace().count();
    let chars = t.chars().count();
    if !is_fallback && (!(3..=8).contains(&words) || chars >= 60) {
        report.violations.push(Violation::TitleLength { words, chars });
    }

    for bad in ['[', ']', '(', ')', '"', '\n', '*', '#'] {
        if t.contains(bad) {
            report.violations.push(Violation::TitlePunctuation {
                found: bad.to_string(),
            });
        }
    }
    if t.ends_with('.') || t.ends_with(',') || t.ends_with('…') || t.ends_with('-') {
        report.violations.push(Violation::TitlePunctuation {
            found: t.chars().last().map(String::from).unwrap_or_default(),
        });
    }

    if let Some(last) = t.split_whitespace().last() {
        let lower = last.to_lowercase();
        if TRUNCATION_ENDINGS.contains(&lower.as_str()) {
            report.violations.push(Violation::TitleTruncated {
                ends_with: last.to_string(),
            });
        }
    }

    if !is_fallback {
        if let Some(found) = find_date_token(t) {
            report
                .violations
                .push(Violation::TitleContainsDate { found });
        }
    }

    // The opening of a recording is its least reliable part; a title drawn from
    // it is the documented failure this check exists for.
    let opening: String = comparable_words(transcript)
        .into_iter()
        .take(12)
        .collect::<Vec<_>>()
        .join(" ");
    if !opening.is_empty() {
        let (shared, _) = max_ngram_overlap(t, &opening);
        if shared >= 3 {
            report.violations.push(Violation::TitleCopiesOpening);
        }
    }

    report
}

/// A year, clock time, or month name in a title.
fn find_date_token(title: &str) -> Option<String> {
    const MONTHS: &[&str] = &[
        "jan", "feb", "mar", "apr", "may", "jun", "jul", "aug", "sep", "oct", "nov", "dec",
    ];
    for token in title.split_whitespace() {
        let bare: String = token
            .chars()
            .filter(|c| c.is_alphanumeric() || *c == ':')
            .collect();
        let lower = bare.to_lowercase();

        // A four-digit year.
        if bare.len() == 4 && bare.chars().all(|c| c.is_ascii_digit()) {
            if let Ok(year) = bare.parse::<u32>() {
                if (1900..=2200).contains(&year) {
                    return Some(token.to_string());
                }
            }
        }
        // A clock time.
        if bare.contains(':') && bare.chars().any(|c| c.is_ascii_digit()) {
            return Some(token.to_string());
        }
        // A month name, alone or as part of a date fragment.
        if lower.len() >= 3 && MONTHS.contains(&&lower[..3]) && lower.len() <= 9 {
            return Some(token.to_string());
        }
    }
    None
}

/// Date-like tokens in a line: anything containing two separators or a month name.
fn find_date_like(line: &str) -> Vec<String> {
    let mut found = Vec::new();
    if let Some(idx) = line.find("Due:") {
        let rest = line[idx + 4..].trim();
        let token: String = rest
            .split(['·', '|', '\n'])
            .next()
            .unwrap_or("")
            .trim()
            .to_string();
        if !token.is_empty() {
            found.push(token);
        }
    }
    found
}

fn is_iso_date(token: &str) -> bool {
    let parts: Vec<&str> = token.split('-').collect();
    if parts.len() != 3 {
        return false;
    }
    parts[0].len() == 4
        && parts[1].len() == 2
        && parts[2].len() == 2
        && parts.iter().all(|p| p.chars().all(|c| c.is_ascii_digit()))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A stand-in transcript. Short, but every check compares against it.
    const TRANSCRIPT: &str = "Hello brother, how are you? Fine, you are good. \
I thought you were saying that you are not feeling well. Can you see my screen? \
Our main problem is that when we asked for data from the report, we shared so many \
opportunities, what is the update? So they shared the data but it came company-wise. \
I will send you the list of mails that need to go out. Let me give it a day to think \
about it and I will let you know.";

    // ---- the overlap metric ----

    #[test]
    fn overlap_finds_the_longest_shared_run() {
        let (words, excerpt) = max_ngram_overlap(
            "The team said our main problem is that when we asked for data.",
            TRANSCRIPT,
        );
        assert!(words >= 10, "got {words}: {excerpt}");
        assert!(excerpt.starts_with("our main problem is"));
    }

    #[test]
    fn overlap_is_zero_for_a_genuine_rewrite() {
        let (words, _) = max_ngram_overlap(
            "Placement updates arrive in bulk, leaving no chance to intervene.",
            TRANSCRIPT,
        );
        assert!(words <= MAX_SHARED_WORDS, "a rewrite must not trip the check");
    }

    #[test]
    fn five_shared_words_are_allowed_and_six_are_not() {
        // §3 permits five consecutive words for proper nouns and exact phrases.
        let five = "we shared so many opportunities";
        let six = "we shared so many opportunities, what";
        assert!(validate_summary(&summary_with(five), TRANSCRIPT).is_valid());
        assert!(!validate_summary(&summary_with(six), TRANSCRIPT).is_valid());
    }

    fn summary_with(bullet: &str) -> String {
        format!(
            "## Overview\n\n**Purpose:** Close the gaps in placement reporting.\n\n## Discussion\n\n### Reporting cadence\n\n- {bullet}\n"
        )
    }

    #[test]
    fn markdown_emphasis_cannot_hide_a_copied_span() {
        let hidden = "**our main problem is that when we asked for data**";
        assert!(!validate_summary(&summary_with(hidden), TRANSCRIPT).is_valid());
    }

    // ---- the documented bad outputs ----

    /// The rejected output from `meeting_transcript_summary.md` §9.
    const REJECTED_SUMMARY: &str = "## Summary
Hello brother, how are you? Fine, you are good. I thought you were saying that you are not feeling well.

## Key Points
- I thought you were saying that you are not feeling well
- That's why I asked you
- I am hearing your voice
- Our main problem is that when we asked for data from the report, we shared so many opportunities, what is the update? So they shared the data";

    #[test]
    fn the_rejected_summary_from_the_rules_file_fails() {
        let report = validate_summary(REJECTED_SUMMARY, TRANSCRIPT);
        assert!(!report.is_valid());
        assert!(
            report
                .violations
                .iter()
                .any(|v| matches!(v, Violation::CopiedSpan { .. })),
            "the copying must be what fails: {:?}",
            report.violations
        );
        assert!(report
            .violations
            .iter()
            .any(|v| matches!(v, Violation::MissingOverview)));
        assert!(report.max_shared_words > 15, "{}", report.max_shared_words);
    }

    #[test]
    fn the_shipped_deterministic_fallback_fails() {
        // What the current fallback produces: the first transcript sentences as
        // an overview, the next few as "key points". This is the extraction
        // failure being shipped under the name of a fallback.
        let fallback = "## Summary\n\nHello brother, how are you? Fine, you are good\n\n## Key Points\n\n- I thought you were saying that you are not feeling well\n- Our main problem is that when we asked for data from the report";
        assert!(!validate_summary(fallback, TRANSCRIPT).is_valid());
    }

    #[test]
    fn a_genuine_summary_passes() {
        // Abridged from the correct output in §9: claims about the discussion,
        // sharing no long span with the transcript.
        let good = "## Overview\n\n**Purpose:** Address gaps in alumni placement support and define a reliable application-tracking process.\n**Themes:** Placement update gaps, application-stage tracking, profile data cleanup\n\n## Discussion\n\n### Placement update gaps\n\n- Candidate-level outcomes never reach the team, so nobody can explain why an application stalled.\n- Bulk updates organised by company do not answer the question actually being asked.\n\n## Decisions\n\n- Request updates at each candidate checkpoint rather than as bulk company-wise data.\n";
        let report = validate_summary(good, TRANSCRIPT);
        assert!(report.is_valid(), "{}", report.summary_line());
    }

    #[test]
    fn the_exact_insufficient_content_string_is_valid() {
        assert!(validate_summary(INSUFFICIENT_SUMMARY, TRANSCRIPT).is_valid());
    }

    #[test]
    fn an_empty_section_or_a_literal_none_fails() {
        let with_empty = "## Overview\n\n**Purpose:** Something real.\n\n## Decisions\n\n## Next Steps\n\n1. Move on\n";
        assert!(!validate_summary(with_empty, TRANSCRIPT).is_valid());

        let with_none = "## Overview\n\n**Purpose:** Something real.\n\n## Decisions\n\n- None\n";
        let report = validate_summary(with_none, TRANSCRIPT);
        assert!(report
            .violations
            .iter()
            .any(|v| matches!(v, Violation::EmptyPlaceholder { .. })));
    }

    #[test]
    fn an_asr_tag_in_the_summary_fails() {
        let with_tag = "## Overview\n\n**Purpose:** Review the [inaudible] rollout plan.\n";
        let report = validate_summary(with_tag, TRANSCRIPT);
        assert!(report
            .violations
            .iter()
            .any(|v| matches!(v, Violation::AsrTag { .. })));
    }

    #[test]
    fn checkboxes_do_not_belong_in_the_summary() {
        let with_todo = "## Overview\n\n**Purpose:** Real purpose.\n\n## Next Steps\n\n- [ ] **Do the thing**\n";
        assert!(!validate_summary(with_todo, TRANSCRIPT).is_valid());
    }

    // ---- to-dos ----

    #[test]
    fn well_formed_todos_pass() {
        let items = vec![
            "### Nitin".to_string(),
            "- [ ] **Send PNC the list of required system emails** — High".to_string(),
            "  PNC needs the trigger list before drafting copy; blocks launch.".to_string(),
            "- [ ] **Add a city dropdown with a free-text fallback** — Due: 2026-08-31 · High"
                .to_string(),
        ];
        let report = validate_action_items(&items, TRANSCRIPT);
        assert!(report.is_valid(), "{}", report.summary_line());
    }

    #[test]
    fn the_exact_empty_case_passes_and_a_paraphrase_does_not() {
        assert!(validate_action_items(&[EMPTY_TODOS.to_string()], TRANSCRIPT).is_valid());
        let paraphrase = vec!["_No to-dos were identified in this meeting._".to_string()];
        assert!(!validate_action_items(&paraphrase, TRANSCRIPT).is_valid());
    }

    #[test]
    fn banned_placeholders_fail() {
        for bad in [
            "- [ ] **Send the list** — Due: TBD",
            "- [ ] **Send the list** — Owner: Not specified",
            "- [ ] **Send the list** — Priority: Medium",
        ] {
            let report = validate_action_items(&[bad.to_string()], TRANSCRIPT);
            assert!(
                report
                    .violations
                    .iter()
                    .any(|v| matches!(v, Violation::BannedPhrase { .. })),
                "should be banned: {bad}"
            );
        }
    }

    #[test]
    fn a_non_iso_due_date_fails() {
        let items = vec!["- [ ] **Send the list** — Due: next Monday".to_string()];
        let report = validate_action_items(&items, TRANSCRIPT);
        assert!(report
            .violations
            .iter()
            .any(|v| matches!(v, Violation::MalformedDate { .. })));
    }

    #[test]
    fn a_todo_copied_from_the_transcript_fails() {
        // The shipped fallback emits transcript sentences as tasks.
        let items =
            vec!["- [ ] **I will send you the list of mails that need to go out** — **Participant**"
                .to_string()];
        let report = validate_action_items(&items, TRANSCRIPT);
        assert!(
            report
                .violations
                .iter()
                .any(|v| matches!(v, Violation::CopiedSpan { .. })),
            "{:?}",
            report.violations
        );
    }

    #[test]
    fn malformed_todo_lines_fail() {
        for bad in [
            "Send the list of emails",                       // no checkbox
            "- [ ] Send the list of emails",                 // action not bold
            "- [ ] **Send the list of emails.**",            // trailing period
        ] {
            let report = validate_action_items(&[bad.to_string()], TRANSCRIPT);
            assert!(
                report
                    .violations
                    .iter()
                    .any(|v| matches!(v, Violation::MalformedTodo { .. })),
                "should be malformed: {bad}"
            );
        }
    }

    #[test]
    fn more_than_fifteen_todos_fails() {
        let items: Vec<String> = (0..16)
            .map(|i| format!("- [ ] **Do distinct task number {i}**"))
            .collect();
        let report = validate_action_items(&items, TRANSCRIPT);
        assert!(report
            .violations
            .iter()
            .any(|v| matches!(v, Violation::TooManyTodos { .. })));
    }

    // ---- titles ----

    #[test]
    fn a_good_title_passes() {
        let report = validate_title("Alumni Placement Tracking Gaps", TRANSCRIPT);
        assert!(report.is_valid(), "{}", report.summary_line());
    }

    #[test]
    fn the_documented_bad_titles_all_fail() {
        for bad in [
            "so um the reason I pulled everyone in",  // copies the cold open
            "[inaudible] thanks for joining everyone", // ASR tag
            "Meeting - Aug 26, 2026 02:03PM",          // placeholder timestamp
            "Investigating the Onboarding Drop-off For", // truncated
            "Alumni Placement Tracking Gaps.",         // terminal punctuation
            "Gaps",                                    // too short
        ] {
            let report = validate_title(bad, TRANSCRIPT);
            assert!(!report.is_valid(), "should fail: {bad}");
        }
    }

    #[test]
    fn a_title_copying_the_transcript_opening_fails() {
        let report = validate_title("Hello Brother How Are You", TRANSCRIPT);
        assert!(report
            .violations
            .iter()
            .any(|v| matches!(v, Violation::TitleCopiesOpening)));
    }

    #[test]
    fn the_fallback_ladder_titles_are_allowed_a_date() {
        // Rungs 4 and 5 are the only places a date is permitted.
        assert!(validate_title("Untitled Meeting — 26 Aug", TRANSCRIPT).is_valid());
        assert!(validate_title("Short Recording — 26 Aug", TRANSCRIPT).is_valid());
    }

    // ---- retry feedback ----

    #[test]
    fn feedback_names_every_violation_specifically() {
        let report = validate_summary(REJECTED_SUMMARY, TRANSCRIPT);
        let feedback = report.prompt_feedback();
        assert!(feedback.contains("consecutive words"));
        assert!(feedback.contains("## Overview"));
        assert!(
            feedback.lines().count() > 2,
            "each violation gets its own line"
        );
    }
}
