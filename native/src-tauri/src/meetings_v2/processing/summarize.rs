//! Stage B — prose generation.
//!
//! Reads `MeetingFacts` and nothing else. The transcript is not in scope at this
//! stage, which is what stops the "summary" from becoming a reshuffled
//! transcript: a model cannot copy out sentences it was never shown.
//!
//! Stage B's input is small — a few hundred tokens of structured facts rather
//! than a whole meeting — so the largest practical local model can run here even
//! when a smaller one did the extraction.
//!
//! There is always a summary. If no model is reachable, or if the model's prose
//! fails validation, the same facts are rendered deterministically. That
//! renderer is the floor under the whole feature: a meeting is never left with
//! nothing to show.

use super::llm::MeetingLlm;
use super::model::{
    ActionItem, MeetingExtension, MeetingFacts, OwnerType, Speaker, SummaryMode, SummarySource,
};
use super::modes::mode_instructions;
use super::speakers::resolve_label;

/// What Stage B produced, and how.
pub struct SummaryOutput {
    pub markdown: String,
    pub provider: String,
    pub model: String,
    /// True when the prose came from the deterministic renderer rather than a
    /// model — either because none was reachable or because the model's output
    /// did not survive validation.
    pub deterministic: bool,
    pub llm_error: Option<String>,
    pub input_chars: usize,
}

/// Runs Stage B.
///
/// Never fails: a model error falls through to the deterministic renderer.
/// Validation happens in the caller, which may also discard model prose and
/// re-render deterministically.
pub async fn generate_summary(
    llm: &dyn MeetingLlm,
    facts: &MeetingFacts,
    speakers: &[Speaker],
    mode: SummaryMode,
    extension: &MeetingExtension,
) -> SummaryOutput {
    let facts_json = facts_for_prompt(facts, speakers);
    let system_prompt = build_summary_prompt(mode, extension);

    match llm.complete(&system_prompt, &facts_json).await {
        Ok(outcome) => {
            let cleaned = strip_code_fence(&outcome.text);
            if cleaned.trim().is_empty() {
                SummaryOutput {
                    markdown: render_markdown(facts, speakers, mode),
                    provider: outcome.provider,
                    model: outcome.model,
                    deterministic: true,
                    llm_error: Some("model returned empty prose".to_string()),
                    input_chars: facts_json.len(),
                }
            } else {
                SummaryOutput {
                    markdown: cleaned,
                    provider: outcome.provider,
                    model: outcome.model,
                    deterministic: false,
                    llm_error: None,
                    input_chars: facts_json.len(),
                }
            }
        }
        Err(err) => SummaryOutput {
            markdown: render_markdown(facts, speakers, mode),
            provider: llm.provider_name(),
            model: llm.model_name(),
            deterministic: true,
            llm_error: Some(err.to_string()),
            input_chars: facts_json.len(),
        },
    }
}

/// The Stage B instructions.
///
/// Markdown out, JSON in — the resolution of the JSON-only/Markdown-only
/// conflict noted in the audit. The model is told what *not* to write at least
/// as firmly as what to write, because the failure mode here is length, not
/// absence.
fn build_summary_prompt(mode: SummaryMode, extension: &MeetingExtension) -> String {
    let extension_block = if extension.instructions.trim().is_empty() {
        String::new()
    } else {
        format!(
            "\nPRESENTATION — {} \n{}\n",
            extension.name,
            extension.instructions.trim()
        )
    };

    format!(
        r#"You are Relay's meeting summary writer.

You are given structured facts already extracted from a meeting. You did not see
the transcript and you must not act as though you did. Write only from these
facts. If a fact is not present, it did not happen — do not fill the gap.

Write GitHub-flavored Markdown. No preamble, no closing remarks, no description
of your process, no JSON.

STRUCTURE — include a section only if it has content:

## Summary
{mode_shape}

## Decisions
- One line per decision, naming who settled it when the facts say so.

## Action Items
- [ ] Action, verb first — **Owner** · Due: YYYY-MM-DD
  Omit " · Due: ..." when the facts carry no deadline. Use the owner exactly as
  the facts give it, including "Unassigned".

## Open Questions
- One line per unresolved item.

WHAT MATTERS
A summary answers five questions: what was this meeting about, what actually
mattered, what was decided, what is still unresolved, and what happens next. It
does not answer "which sentences appeared in the transcript".

Before a point goes in, ask whether somebody who missed the meeting would
consider it important enough to know. Leave out greetings, introductions,
screen-share mechanics, "let me show you", "I'll just check", logistics, filler,
small talk, and demo narration. Keep substantive topics, decisions, risks,
blockers, changes, commitments, unresolved questions, and conclusions.

Lead with what actually mattered and what was decided. Do not narrate the
meeting in order ("The meeting started with...", "Then Sarah said...") unless the
chronology is itself the point. Never pad. A short summary that a reader trusts
beats a complete one they skim.

FORBIDDEN
- Naming a participant who is not in the facts.
- Stating a decision, owner, or deadline the facts do not contain.
- Repeating the same point in two bullets.
- Quoting the transcript. You do not have it.
{extension_block}"#,
        mode_shape = mode_instructions(mode),
        extension_block = extension_block,
    )
}

/// The facts, as the model sees them: speaker ids already resolved to labels, so
/// the model never has to reason about identifiers, and never has to guess a name.
fn facts_for_prompt(facts: &MeetingFacts, speakers: &[Speaker]) -> String {
    let payload = serde_json::json!({
        "title": facts.title,
        "meeting_type": facts.meeting_type.label(),
        "participants": speakers.iter().map(|s| s.label()).collect::<Vec<_>>(),
        "topics": facts.topics.iter().map(|t| &t.label).collect::<Vec<_>>(),
        "key_points": facts.key_points.iter().map(|p| &p.text).collect::<Vec<_>>(),
        "decisions": facts
            .decisions
            .iter()
            .map(|d| serde_json::json!({
                "statement": d.statement,
                "decided_by": d
                    .decided_by_speaker_id
                    .as_deref()
                    .map(|id| resolve_label(speakers, Some(id))),
            }))
            .collect::<Vec<_>>(),
        "action_items": facts
            .action_items
            .iter()
            .map(|a| serde_json::json!({
                "description": a.description,
                "owner": owner_label(a, speakers),
                "deadline": a.deadline,
            }))
            .collect::<Vec<_>>(),
        "open_questions": facts
            .open_questions
            .iter()
            .map(|q| &q.question)
            .collect::<Vec<_>>(),
        "entities": facts.entities.iter().map(|e| &e.name).collect::<Vec<_>>(),
    });

    serde_json::to_string_pretty(&payload).unwrap_or_else(|_| "{}".to_string())
}

/// The display name for an action item's owner. Resolved at render time from the
/// speaker registry, so a rename is reflected without re-extracting anything.
pub fn owner_label(item: &ActionItem, speakers: &[Speaker]) -> String {
    match item.owner_type {
        OwnerType::Me | OwnerType::Speaker => item
            .owner_speaker_id
            .as_deref()
            .map(|id| resolve_label(speakers, Some(id)).to_string())
            .unwrap_or_else(|| "Unassigned".to_string()),
        OwnerType::External => item
            .owner_label
            .clone()
            .unwrap_or_else(|| "Unassigned".to_string()),
        OwnerType::Group => "The group".to_string(),
        OwnerType::Unassigned => "Unassigned".to_string(),
    }
}

/// Renders facts to Markdown with no model involved.
///
/// This is both the fallback and the reference implementation of the required
/// output shape. It cannot hallucinate — every line is a field — so it passes
/// validation by construction.
pub fn render_markdown(facts: &MeetingFacts, speakers: &[Speaker], mode: SummaryMode) -> String {
    let mut out = String::new();

    out.push_str("## Summary\n\n");
    // Honesty about provenance. Reaching this renderer at all means no model
    // wrote the prose; whether a model *understood* the meeting is a separate
    // question, and the two produce noticeably different text.
    out.push_str(&format!(
        "_{}_\n\n",
        if facts.deterministic {
            SummarySource::DeterministicExtraction.provenance()
        } else {
            SummarySource::DeterministicPresentation.provenance()
        }
    ));
    let point_budget = match mode {
        SummaryMode::Concise => 4,
        SummaryMode::Standard => 8,
        SummaryMode::Detailed => 14,
    };

    if facts.key_points.is_empty() && facts.topics.is_empty() {
        out.push_str(
            "_No summary could be derived from this recording. The raw transcript is available._\n",
        );
    } else {
        if !facts.topics.is_empty() {
            let labels: Vec<&str> = facts
                .topics
                .iter()
                .map(|t| t.label.as_str())
                .take(6)
                .collect();
            out.push_str(&format!("**Topics discussed:** {}\n\n", labels.join(", ")));
        }
        for point in facts.key_points.iter().take(point_budget) {
            out.push_str(&format!("- {}\n", point.text));
        }
    }

    if !facts.decisions.is_empty() {
        out.push_str("\n## Decisions\n\n");
        for decision in &facts.decisions {
            match decision.decided_by_speaker_id.as_deref() {
                Some(id) => out.push_str(&format!(
                    "- {} — {}\n",
                    decision.statement,
                    resolve_label(speakers, Some(id))
                )),
                None => out.push_str(&format!("- {}\n", decision.statement)),
            }
        }
    }

    if !facts.action_items.is_empty() {
        out.push_str("\n## Action Items\n\n");
        for item in &facts.action_items {
            let due = item
                .deadline
                .as_deref()
                .map(|d| format!(" · Due: {}", d))
                .unwrap_or_default();
            out.push_str(&format!(
                "- [ ] {} — **{}**{}\n",
                item.description,
                owner_label(item, speakers),
                due
            ));
        }
    }

    if !facts.open_questions.is_empty() {
        out.push_str("\n## Open Questions\n\n");
        for question in &facts.open_questions {
            out.push_str(&format!("- {}\n", question.question));
        }
    }

    out.trim_end().to_string()
}

/// Removes a wrapping code fence, which models add to Markdown about as often as
/// to JSON.
fn strip_code_fence(text: &str) -> String {
    let trimmed = text.trim();
    if !trimmed.starts_with("```") {
        return trimmed.to_string();
    }

    let without_open = trimmed
        .trim_start_matches("```")
        .trim_start_matches("markdown")
        .trim_start_matches("md")
        .trim_start();

    match without_open.rfind("```") {
        Some(idx) => without_open[..idx].trim_end().to_string(),
        None => without_open.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::meetings_v2::processing::llm::test_support::ScriptedLlm;
    use crate::meetings_v2::processing::model::{
        ActionItemStatus, Decision, EntityKind, KeyPoint, MeetingType, OpenQuestion,
        SegmentChannel, SpeakerOrigin, Topic, SPEAKER_ID_ME, SPEAKER_ID_REMOTE,
    };
    use crate::meetings_v2::processing::modes::builtin_extensions;

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

    fn facts() -> MeetingFacts {
        MeetingFacts {
            title: "Release Cut And Schema Freeze".to_string(),
            meeting_type: MeetingType::Planning,
            key_points: vec![KeyPoint {
                id: "point_0".into(),
                text: "The release date was settled after weighing the migration risk.".into(),
                topic_id: None,
                source_segment_ids: vec!["seg_00000".into()],
            }],
            topics: vec![Topic {
                id: "topic_0".into(),
                label: "Release Planning".into(),
                segment_ids: vec!["seg_00000".into()],
            }],
            decisions: vec![Decision {
                id: "decision_0".into(),
                statement: "Ship the release on Friday.".into(),
                decided_by_speaker_id: Some(SPEAKER_ID_ME.into()),
                source_segment_ids: vec!["seg_00000".into()],
                confidence: 0.8,
            }],
            action_items: vec![
                ActionItem {
                    id: "action_0".into(),
                    description: "Write the changelog".into(),
                    owner_type: OwnerType::Speaker,
                    owner_speaker_id: Some(SPEAKER_ID_REMOTE.into()),
                    owner_label: None,
                    deadline: Some("2026-08-28".into()),
                    status: ActionItemStatus::Open,
                    source_segment_ids: vec!["seg_00000".into()],
                    confidence: 0.8,
                },
                ActionItem {
                    id: "action_1".into(),
                    description: "Decide who owns the migration".into(),
                    owner_type: OwnerType::Unassigned,
                    owner_speaker_id: None,
                    owner_label: None,
                    deadline: None,
                    status: ActionItemStatus::Open,
                    source_segment_ids: vec!["seg_00001".into()],
                    confidence: 0.5,
                },
            ],
            open_questions: vec![OpenQuestion {
                id: "question_0".into(),
                question: "Who reviews the migration script?".into(),
                source_segment_ids: vec!["seg_00001".into()],
            }],
            entities: vec![crate::meetings_v2::processing::model::Entity {
                id: "entity_0".into(),
                name: "Relay".into(),
                kind: EntityKind::Product,
                segment_ids: vec!["seg_00000".into()],
            }],
            speaker_ids: vec![SPEAKER_ID_ME.into(), SPEAKER_ID_REMOTE.into()],
            deterministic: false,
        }
    }

    fn roster() -> Vec<Speaker> {
        vec![
            speaker(SPEAKER_ID_ME, "Me", None, true),
            speaker(SPEAKER_ID_REMOTE, "Speaker 1", None, false),
        ]
    }

    #[test]
    fn the_deterministic_renderer_produces_the_required_shape() {
        let rendered = render_markdown(&facts(), &roster(), SummaryMode::Standard);

        assert!(rendered.starts_with("## Summary"));
        assert!(rendered.contains("## Decisions"));
        assert!(rendered.contains("## Action Items"));
        assert!(rendered.contains("## Open Questions"));
        assert!(rendered.contains("- [ ] Write the changelog — **Speaker 1** · Due: 2026-08-28"));
        assert!(
            rendered.contains("- [ ] Decide who owns the migration — **Unassigned**"),
            "an unowned item must render as Unassigned, not as a guess"
        );
        assert!(
            !rendered.contains("Due: \n") && !rendered.contains("· Due: none"),
            "no empty deadline may be rendered"
        );
    }

    #[test]
    fn empty_sections_are_omitted_entirely() {
        let mut sparse = facts();
        sparse.decisions.clear();
        sparse.action_items.clear();
        sparse.open_questions.clear();

        let rendered = render_markdown(&sparse, &roster(), SummaryMode::Concise);
        assert!(!rendered.contains("## Decisions"));
        assert!(!rendered.contains("## Action Items"));
        assert!(!rendered.contains("## Open Questions"));
        assert!(rendered.contains("## Summary"));
    }

    #[test]
    fn a_meeting_with_nothing_extractable_still_renders_something_honest() {
        let empty = MeetingFacts {
            title: "Untitled".into(),
            meeting_type: MeetingType::General,
            key_points: Vec::new(),
            topics: Vec::new(),
            decisions: Vec::new(),
            action_items: Vec::new(),
            open_questions: Vec::new(),
            entities: Vec::new(),
            speaker_ids: Vec::new(),
            deterministic: true,
        };
        let rendered = render_markdown(&empty, &[], SummaryMode::Standard);
        assert!(rendered.contains("raw transcript is available"));
    }

    #[test]
    fn renaming_a_speaker_changes_the_rendered_owner_without_touching_the_facts() {
        let facts = facts();
        let mut speakers = roster();

        let before = render_markdown(&facts, &speakers, SummaryMode::Standard);
        assert!(before.contains("**Speaker 1**"));

        speakers[1].display_name = Some("Pranjali".to_string());
        let after = render_markdown(&facts, &speakers, SummaryMode::Standard);

        assert!(after.contains("**Pranjali**"));
        assert!(!after.contains("**Speaker 1**"));
        assert_eq!(
            facts.action_items[0].owner_speaker_id.as_deref(),
            Some(SPEAKER_ID_REMOTE),
            "the action item still references the id"
        );
    }

    #[test]
    fn concise_renders_fewer_points_than_detailed() {
        let mut many = facts();
        many.key_points = (0..20)
            .map(|i| KeyPoint {
                id: format!("point_{}", i),
                text: format!("Discussion point number {}.", i),
                topic_id: None,
                source_segment_ids: Vec::new(),
            })
            .collect();

        let concise = render_markdown(&many, &roster(), SummaryMode::Concise);
        let detailed = render_markdown(&many, &roster(), SummaryMode::Detailed);
        assert!(concise.len() < detailed.len());
        assert_eq!(concise.matches("- Discussion point").count(), 4);
        assert_eq!(detailed.matches("- Discussion point").count(), 14);
    }

    #[tokio::test]
    async fn model_prose_is_used_when_the_model_answers() {
        let llm = ScriptedLlm::new(vec![Ok("## Summary\n\nWe cut the release.".to_string())]);
        let default = builtin_extensions()[0].clone();

        let out =
            generate_summary(&llm, &facts(), &roster(), SummaryMode::Standard, &default).await;
        assert!(!out.deterministic);
        assert_eq!(out.markdown, "## Summary\n\nWe cut the release.");
        assert!(out.llm_error.is_none());
    }

    #[tokio::test]
    async fn the_model_is_never_shown_the_transcript_only_the_facts() {
        let llm = ScriptedLlm::new(vec![Ok("## Summary\n\nDone.".to_string())]);
        let default = builtin_extensions()[0].clone();
        generate_summary(&llm, &facts(), &roster(), SummaryMode::Standard, &default).await;

        let calls = llm.calls.lock().unwrap();
        let (system, user) = &calls[0];
        assert!(
            user.contains("\"key_points\""),
            "Stage B receives JSON facts"
        );
        assert!(
            !user.contains("seg_00000"),
            "segment ids are not part of Stage B's input"
        );
        assert!(system.contains("You did not see"));
    }

    #[tokio::test]
    async fn an_unavailable_model_still_yields_a_summary() {
        let llm = ScriptedLlm::always_unavailable();
        let default = builtin_extensions()[0].clone();

        let out =
            generate_summary(&llm, &facts(), &roster(), SummaryMode::Standard, &default).await;
        assert!(out.deterministic);
        assert!(out.llm_error.is_some());
        assert!(out.markdown.contains("## Action Items"));
    }

    #[tokio::test]
    async fn empty_model_prose_falls_back_rather_than_showing_a_blank_summary() {
        let llm = ScriptedLlm::new(vec![Ok("   \n  ".to_string())]);
        let default = builtin_extensions()[0].clone();

        let out =
            generate_summary(&llm, &facts(), &roster(), SummaryMode::Standard, &default).await;
        assert!(out.deterministic);
        assert!(out.markdown.contains("## Summary"));
    }

    #[tokio::test]
    async fn an_extension_changes_the_instructions_not_the_facts() {
        let brief = builtin_extensions()
            .into_iter()
            .find(|e| e.id == "executive_brief")
            .unwrap();
        let llm = ScriptedLlm::new(vec![Ok("## Summary\n\nShort.".to_string())]);

        generate_summary(&llm, &facts(), &roster(), SummaryMode::Standard, &brief).await;

        let calls = llm.calls.lock().unwrap();
        let (system, user) = &calls[0];
        assert!(system.contains("Executive Brief"));
        assert!(system.contains("two minutes"));
        // The facts payload is identical regardless of extension.
        assert!(user.contains("Ship the release on Friday."));
    }

    #[test]
    fn a_fenced_markdown_response_is_unwrapped() {
        assert_eq!(
            strip_code_fence("```markdown\n## Summary\n\nText.\n```"),
            "## Summary\n\nText."
        );
        assert_eq!(strip_code_fence("## Summary"), "## Summary");
        // An unterminated fence must not swallow the content.
        assert_eq!(strip_code_fence("```\n## Summary"), "## Summary");
    }
}
