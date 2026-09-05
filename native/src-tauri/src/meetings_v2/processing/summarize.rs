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

use super::length::{budget_guidance, SummaryBudget};
use super::llm::{LlmError, LlmRequest, MeetingLlm};
use super::model::{
    ActionItem, KeyPointKind, MeetingExtension, MeetingFacts, OwnerType, Speaker, SummaryMode,
    SummarySource,
};
use super::modes::mode_instructions;
use super::speakers::resolve_label;
use crate::meetings_v2::types::MeetingNotes;

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

/// Everything Stage B is given about one meeting.
///
/// Grouped rather than passed as six arguments because they travel together
/// through generation, repair, and the deterministic renderer, and because a
/// repair attempt must be built from *exactly* the same inputs as the draft it
/// is repairing.
pub struct SummaryInput<'a> {
    pub facts: &'a MeetingFacts,
    pub speakers: &'a [Speaker],
    pub budget: SummaryBudget,
    pub extension: &'a MeetingExtension,
    /// Notes reach Stage B as emphasis only. Stage A already reconciled them
    /// against the transcript; what survives here is the fact that a person
    /// thought something mattered.
    pub notes: &'a MeetingNotes,
    /// The user's own instructions for this summary, if they gave any. Always
    /// subordinate to accuracy.
    pub user_instructions: Option<&'a str>,
}

/// Runs Stage B.
///
/// Never fails: a model error falls through to the deterministic renderer.
/// Validation happens in the caller, which may also ask for a repair and, if
/// that fails too, re-render deterministically.
pub async fn generate_summary(llm: &dyn MeetingLlm, input: &SummaryInput<'_>) -> SummaryOutput {
    run(llm, input, None).await
}

/// Regenerates after a validation failure, telling the model what was wrong.
///
/// Deliberately not "call the same prompt again": an identical request has no
/// reason to produce a different answer, and re-rolling the dice is not a
/// repair. The corrective text names the specific rule that was broken and what
/// to do about it, and the original request is otherwise unchanged so a fix for
/// one problem cannot quietly change everything else.
pub async fn repair_summary(
    llm: &dyn MeetingLlm,
    input: &SummaryInput<'_>,
    feedback: &str,
) -> SummaryOutput {
    run(llm, input, Some(feedback)).await
}

async fn run(
    llm: &dyn MeetingLlm,
    input: &SummaryInput<'_>,
    feedback: Option<&str>,
) -> SummaryOutput {
    let user_prompt = build_user_prompt(input, feedback);
    // Words to tokens, with room for Markdown structure. Generous enough that
    // the cap never truncates a summary the budget itself allows.
    let max_output_tokens = ((input.budget.max_words as f64 * 2.0) as u32 + 400).max(600);

    let full = build_summary_prompt(input);
    let first = attempt(llm, &full, &user_prompt, max_output_tokens).await;
    if let Some(output) = first.as_summary(&user_prompt) {
        return output;
    }

    // The full contract is sixteen numbered sections, and a small local model
    // handed all of it sometimes answers with nothing at all — which is the
    // "model returned an empty response" a user sees on a short meeting. An
    // empty answer is not a refusal and not an opinion about the facts, so the
    // same facts are put again behind a contract short enough to follow.
    //
    // Only an *empty* answer earns this. A provider that could not be reached
    // will not be reached by a shorter prompt either, and retrying there just
    // doubles the wait before the fallback the user was always going to get.
    let last = if matches!(first, Attempt::Empty { .. }) {
        tracing::warn!(
            "meeting_summary: empty prose from {}; retrying with the compact contract",
            llm.model_name()
        );
        let compact = build_compact_summary_prompt(input);
        let second = attempt(llm, &compact, &user_prompt, max_output_tokens).await;
        if let Some(output) = second.as_summary(&user_prompt) {
            return output;
        }
        second
    } else {
        first
    };

    SummaryOutput {
        markdown: render_markdown(input.facts, input.speakers, input.budget.mode),
        provider: last.provider(llm),
        model: last.model(llm),
        deterministic: true,
        llm_error: Some(last.reason()),
        input_chars: user_prompt.len(),
    }
}

/// One model call and what came back, before the pipeline decides what it means.
enum Attempt {
    /// Prose that survived fence-stripping and is not blank.
    Prose {
        markdown: String,
        provider: String,
        model: String,
    },
    /// The model answered with nothing usable. Worth trying again differently.
    Empty { provider: String, model: String },
    /// No answer at all. Trying again differently would not help.
    Failed(String),
}

impl Attempt {
    /// The finished summary, when this attempt produced one.
    fn as_summary(&self, user_prompt: &str) -> Option<SummaryOutput> {
        match self {
            Self::Prose {
                markdown,
                provider,
                model,
            } => Some(SummaryOutput {
                markdown: markdown.clone(),
                provider: provider.clone(),
                model: model.clone(),
                deterministic: false,
                llm_error: None,
                input_chars: user_prompt.len(),
            }),
            _ => None,
        }
    }

    /// The provider that answered, or the one that was asked.
    fn provider(&self, llm: &dyn MeetingLlm) -> String {
        match self {
            Self::Prose { provider, .. } | Self::Empty { provider, .. } => provider.clone(),
            Self::Failed(_) => llm.provider_name(),
        }
    }

    fn model(&self, llm: &dyn MeetingLlm) -> String {
        match self {
            Self::Prose { model, .. } | Self::Empty { model, .. } => model.clone(),
            Self::Failed(_) => llm.model_name(),
        }
    }

    /// What to record about why no model wrote this summary.
    fn reason(&self) -> String {
        match self {
            // Said in full, because "empty response" alone reads as a bug in
            // Relay. It is a model declining to write, twice.
            Self::Empty { .. } | Self::Prose { .. } => {
                "model returned empty prose, twice — once from the full contract and once from \
a shortened one"
                    .to_string()
            }
            Self::Failed(reason) => reason.clone(),
        }
    }
}

async fn attempt(
    llm: &dyn MeetingLlm,
    system_prompt: &str,
    user_prompt: &str,
    max_output_tokens: u32,
) -> Attempt {
    match llm
        .complete_request(LlmRequest::prose(
            system_prompt,
            user_prompt,
            max_output_tokens,
        ))
        .await
    {
        Ok(outcome) => {
            let cleaned = strip_code_fence(&outcome.text);
            if cleaned.trim().is_empty() {
                Attempt::Empty {
                    provider: outcome.provider,
                    model: outcome.model,
                }
            } else {
                Attempt::Prose {
                    markdown: cleaned,
                    provider: outcome.provider,
                    model: outcome.model,
                }
            }
        }
        Err(LlmError::Empty) => Attempt::Empty {
            provider: llm.provider_name(),
            model: llm.model_name(),
        },
        Err(err) => Attempt::Failed(err.to_string()),
    }
}

/// The summary contract, reduced to what cannot be dropped.
///
/// The second attempt after an empty first one. It keeps the accuracy rules and
/// the output shape — those are why the summary is trustworthy — and drops the
/// taste, the rationale, and the worked examples, which are why it is long. The
/// user's own instructions and the chosen extension are kept too: dropping them
/// would silently produce a summary shaped differently from the one asked for,
/// and the user has no way of knowing a retry happened.
fn build_compact_summary_prompt(input: &SummaryInput<'_>) -> String {
    let mut out = String::from(
        "You write the summary a person reads tomorrow, from structured facts already \
extracted from their meeting.\n\n\
RULES\n\
- Write only what the facts contain. Never invent a decision, owner, deadline, date, \
number, or name.\n\
- A point marked \"proposal\" or \"recommendation\" was floated, not adopted. Never write \
it as a decision.\n\
- Where the facts say an owner is \"Unassigned\", write \"Unassigned\".\n\
- Be specific. \"The launch moved to Monday because QA needs three more days\" is the \
summary; \"the team discussed the timeline\" is a wasted line.\n\
- Leave out greetings, logistics, audio checks, and small talk.\n\n\
OUTPUT — GitHub-flavored Markdown, and nothing else. No preamble, no code fence.\n\
Begin with `## Overview` — one short paragraph on why they met and what came of it.\n\
Then, only where the facts carry content: `## Discussion` (a `###` per topic), \
`## Decisions`, `## Action Items` (as `- [ ] Action — **Owner** · Due: YYYY-MM-DD`, \
omitting the due part when there is no date), `## Risks & Blockers`, \
`## Open Questions`.\n\
Omit a section that would be empty. Never write \"None\" under a heading.\n\
Write something for `## Overview` even when the facts are thin — a two-line meeting \
gets a two-line summary, not a blank one.\n",
    );

    if !input.extension.instructions.trim().is_empty() {
        out.push_str(&format!(
            "\nPRESENTATION — {}\n{}\nArrangement and tone only; never a claim the facts \
do not carry.\n",
            input.extension.name,
            input.extension.instructions.trim()
        ));
    }

    if let Some(instructions) = input.user_instructions.map(str::trim).filter(|i| !i.is_empty()) {
        out.push_str(&format!(
            "\nTHE USER'S OWN INSTRUCTIONS\n{}\nFollow these for structure, tone, emphasis and \
length. They never permit inventing something the facts do not contain.\n",
            instructions
        ));
    }

    out
}

/// The summary contract.
///
/// Markdown out, JSON in — the resolution of the JSON-only/Markdown-only
/// conflict noted in the audit. Written as a numbered hierarchy rather than as
/// prose because a small local model follows a structure it can see; the
/// previous version put role, scope, structure, taste, and prohibitions into one
/// run of paragraphs and the prohibitions were the part that got lost.
fn build_summary_prompt(input: &SummaryInput<'_>) -> String {
    let extension_block = if input.extension.instructions.trim().is_empty() {
        String::new()
    } else {
        format!(
            "\n15. PRESENTATION — {}\n{}\nThis affects arrangement and tone only. It never permits \
a claim the facts do not carry.\n",
            input.extension.name,
            input.extension.instructions.trim()
        )
    };

    let user_block = match input.user_instructions.map(str::trim).filter(|i| !i.is_empty()) {
        Some(instructions) => format!(
            "\n16. THE USER'S OWN INSTRUCTIONS\n{}\nFollow these where they concern structure, \
tone, emphasis, or length. They are subordinate to sections 3 to 6: no instruction \
makes it acceptable to state a decision, owner, deadline, or commitment the facts \
do not contain. If an instruction cannot be followed without inventing something, \
follow the part that can and drop the part that cannot.\n",
            instructions
        ),
        None => String::new(),
    };

    let notes_block = if input.notes.has_during() {
        "\n14. THE USER'S NOTES\nA participant's own notes are included. They are not a second \
transcript and must not be copied out or listed. They tell you what a person in the \
room thought was worth writing down: where they emphasise something the facts also \
carry, that thing belongs near the top. A note about something the facts do not \
carry is not evidence that it happened.\n"
    } else {
        ""
    };

    format!(
        r#"1. ROLE
You are Relay's meeting summary writer. You write the record a person reads
tomorrow when they have forgotten the meeting happened.

2. OBJECTIVE
Your summary must let someone answer, without the transcript: what mattered, what
changed, what was decided and why, what they have to do, what others have to do,
and what is still unresolved. A summary that reads well but leaves those
unanswered has failed.

3. YOUR SOURCE
You are given structured facts already extracted from this meeting. You did not
see the transcript and must not act as though you did. Write only from these
facts. If something is not in them, it did not happen — do not fill the gap, and
do not hedge about what might have been discussed.

4. ACCURACY — the rules that outrank every other rule here
  a. Never invent a decision, fact, deadline, owner, commitment, conclusion,
     risk, or agreement.
  b. Never name a person who is not in the facts.
  c. Never promote a proposal into a decision. A key point marked "proposal" or
     "recommendation" was floated or argued for — not adopted. Write it as such:
     "X was proposed", never "the team decided X".
  d. Never turn a discussion into an action item. An action item exists in the
     facts or it does not exist at all.
  e. Never invent an owner. Where the facts say "Unassigned", write "Unassigned".
  f. Never invent a deadline. No date in the facts means no date in the summary.
  g. Copy numbers exactly or leave them out.

5. WHAT TO INCLUDE
Decisions and the reasoning behind them. Commitments, with their owner and date.
Substantive discussion — what was proposed, what constraint drove it, what
changed. Disagreements that affected the outcome, and trade-offs that were
knowingly accepted. Risks and blockers. Questions left open.

6. WHAT TO LEAVE OUT
Greetings, introductions, small talk, audio checks, screen-share mechanics,
"let me show you", logistics, filler, demo narration, and repetition. Generic
observations that would be true of any meeting ("several topics were covered").
The same point in two bullets. Speaker-by-speaker chronology — "Alice said X,
then Bob said Y" — unless the sequence is itself what mattered.

7. BE CONCRETE
"The team discussed the launch timeline" is a wasted line. "The launch moved from
Friday to Monday because QA needs three more days" is the summary. Wherever the
facts carry a specific — a number, a constraint, a reason, a name — use it. The
specific is the part that is worth remembering.

8. RATIONALE
Where a decision carries a reason, the reason goes in the summary with it. Do not
reduce "moving the launch to Monday because the payment integration still has
three blocking bugs" to "launch moved to Monday". Where a decision carries no
reason, state the decision plainly and say nothing about why.

9. UNCERTAINTY
Where the facts leave something unresolved, the summary leaves it unresolved.
Do not manufacture closure, and do not write around an open question as though it
had been answered.

10. ATTRIBUTION
Use names where the facts give them, and where they matter: who owns a
commitment, who settled a decision, who disagreed. Do not attach a name to
anything the facts leave unattributed.

11. DEPTH
{mode_shape}

12. STRUCTURE
GitHub-flavored Markdown. Include a section only when it has content; an empty
section is never written out, and never filled with "None" or a placeholder.

## Overview
One short paragraph: why these people met and what came of it. Never opens by
narrating the recording ("The meeting began with...").

## Discussion
One `###` heading per topic, each with the points that matter under it. Capture
the reasoning, not just the position.

## Decisions
- One line per decision, with its reason where the facts give one, and who
  settled it where the facts say.

## Action Items
- [ ] Action, verb first — **Owner** · Due: YYYY-MM-DD
  Omit " · Due: ..." when the facts carry no deadline. Use the owner exactly as
  the facts give it, including "Unassigned".

## Risks & Blockers
- One line per risk, blocker, dependency, or constraint the facts carry.

## Open Questions
- One line per unresolved item.

13. OUTPUT
Return the summary and nothing else. No preamble, no "here is the summary", no
closing remark, no note about what you did, no JSON, no code fence. Begin with
`## Overview`.
{notes_block}{extension_block}{user_block}"#,
        mode_shape = mode_instructions(input.budget.mode),
        notes_block = notes_block,
        extension_block = extension_block,
        user_block = user_block,
    )
}

/// The user message: the facts, the meeting's own size budget, and — on a second
/// attempt — what was wrong with the first.
fn build_user_prompt(input: &SummaryInput<'_>, feedback: Option<&str>) -> String {
    let mut out = String::new();
    if let Some(feedback) = feedback {
        out.push_str(feedback);
        out.push_str("\n\n");
    }
    out.push_str(&budget_guidance(&input.budget));
    out.push_str("\n\n");
    if input.notes.has_during() {
        out.push_str("WHAT A PARTICIPANT WROTE DOWN (emphasis, not a transcript)\n");
        out.push_str(input.notes.during.trim());
        out.push_str("\n\n");
    }
    out.push_str("FACTS\n");
    out.push_str(&facts_for_prompt(input.facts, input.speakers));
    out
}

/// The facts, as the model sees them: speaker ids already resolved to labels, so
/// the model never has to reason about identifiers, and never has to guess a name.
///
/// Key points are grouped under their topic rather than handed over as a flat
/// list. Stage B is asked to organize the Discussion section by topic, and it
/// cannot do that from a list that has already lost the grouping.
fn facts_for_prompt(facts: &MeetingFacts, speakers: &[Speaker]) -> String {
    let point_json = |point: &super::model::KeyPoint| {
        // Plain discussion is the default and does not need saying; the other
        // kinds are exactly the ones the model must not flatten.
        if point.kind == KeyPointKind::Discussion {
            serde_json::json!({ "point": point.text })
        } else {
            serde_json::json!({ "point": point.text, "this_was_only_a": point.kind.label() })
        }
    };

    let topics: Vec<serde_json::Value> = facts
        .topics
        .iter()
        .map(|topic| {
            serde_json::json!({
                "topic": topic.label,
                "points": facts
                    .key_points
                    .iter()
                    .filter(|p| p.topic_id.as_deref() == Some(topic.id.as_str()))
                    .map(point_json)
                    .collect::<Vec<_>>(),
            })
        })
        .collect();

    let ungrouped: Vec<serde_json::Value> = facts
        .key_points
        .iter()
        .filter(|p| {
            p.topic_id
                .as_deref()
                .is_none_or(|id| !facts.topics.iter().any(|t| t.id == id))
        })
        .map(point_json)
        .collect();

    let payload = serde_json::json!({
        "title": facts.title,
        "meeting_type": facts.meeting_type.label(),
        "participants": speakers.iter().map(|s| s.label()).collect::<Vec<_>>(),
        "discussion_by_topic": topics,
        "other_points": ungrouped,
        "decisions": facts
            .decisions
            .iter()
            .map(|d| {
                let mut value = serde_json::json!({ "decided": d.statement });
                if let Some(rationale) = d.rationale.as_deref() {
                    value["because"] = serde_json::Value::String(rationale.to_string());
                }
                if let Some(id) = d.decided_by_speaker_id.as_deref() {
                    value["settled_by"] =
                        serde_json::Value::String(resolve_label(speakers, Some(id)).to_string());
                }
                value
            })
            .collect::<Vec<_>>(),
        "action_items": facts
            .action_items
            .iter()
            .map(|a| {
                let mut value = serde_json::json!({
                    "action": a.description,
                    "owner": owner_label(a, speakers),
                });
                if let Some(deadline) = a.deadline.as_deref() {
                    value["due"] = serde_json::Value::String(deadline.to_string());
                }
                value
            })
            .collect::<Vec<_>>(),
        "open_questions": facts
            .open_questions
            .iter()
            .map(|q| &q.question)
            .collect::<Vec<_>>(),
        "risks": facts
            .risks
            .iter()
            .map(|r| serde_json::json!({ "kind": r.kind.label(), "statement": r.statement }))
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
/// Both the fallback and the reference implementation of the required output
/// shape. It cannot hallucinate — every line is a field — so it passes
/// validation by construction, and it is the floor under the whole feature.
///
/// It carries rationale and risks for the same reason the model's version does:
/// a fallback that silently drops the reason behind a decision would make every
/// provider outage quietly cost the most valuable part of the record.
pub fn render_markdown(facts: &MeetingFacts, speakers: &[Speaker], mode: SummaryMode) -> String {
    let mut out = String::new();

    out.push_str("## Overview\n\n");
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

    if facts.key_points.is_empty()
        && facts.topics.is_empty()
        && facts.decisions.is_empty()
        && facts.action_items.is_empty()
        && facts.risks.is_empty()
        && facts.open_questions.is_empty()
    {
        out.push_str(
            "_No structured summary could be derived from this recording. The raw transcript is available._\n",
        );
        return out.trim_end().to_string();
    }

    if facts.key_points.is_empty() && facts.topics.is_empty() {
        out.push_str(
            "_No main discussion points could be extracted. The verified meeting facts and raw transcript are available below._\n\n",
        );
    }

    if !facts.topics.is_empty() {
        let labels: Vec<&str> = facts
            .topics
            .iter()
            .map(|t| t.label.as_str())
            .take(6)
            .collect();
        out.push_str(&format!("**Topics discussed:** {}\n", labels.join(", ")));
    }

    let point_budget = match mode {
        SummaryMode::Concise => 4,
        SummaryMode::Standard => 8,
        SummaryMode::Detailed => 14,
    };

    // Grouped under their topics where the facts carry the grouping, so the
    // fallback is scannable rather than one undifferentiated list.
    let mut rendered = 0usize;
    let mut discussion = String::new();
    for topic in &facts.topics {
        let points: Vec<&super::model::KeyPoint> = facts
            .key_points
            .iter()
            .filter(|p| p.topic_id.as_deref() == Some(topic.id.as_str()))
            .collect();
        if points.is_empty() {
            continue;
        }
        discussion.push_str(&format!("\n### {}\n\n", topic.label));
        for point in points {
            if rendered >= point_budget {
                break;
            }
            discussion.push_str(&format!("- {}\n", render_point(point)));
            rendered += 1;
        }
    }

    let ungrouped: Vec<&super::model::KeyPoint> = facts
        .key_points
        .iter()
        .filter(|p| {
            p.topic_id
                .as_deref()
                .is_none_or(|id| !facts.topics.iter().any(|t| t.id == id))
        })
        .collect();
    if !ungrouped.is_empty() && rendered < point_budget {
        if !discussion.is_empty() {
            discussion.push_str("\n### Other points\n\n");
        } else {
            discussion.push('\n');
        }
        for point in ungrouped {
            if rendered >= point_budget {
                break;
            }
            discussion.push_str(&format!("- {}\n", render_point(point)));
            rendered += 1;
        }
    }

    if !discussion.trim().is_empty() {
        out.push_str("\n## Discussion\n");
        out.push_str(&discussion);
    }

    if !facts.decisions.is_empty() {
        out.push_str("\n## Decisions\n\n");
        for decision in &facts.decisions {
            let mut line = decision.statement.trim_end_matches('.').to_string();
            if let Some(rationale) = decision.rationale.as_deref() {
                line.push_str(&format!(" — because {}", rationale.trim_end_matches('.')));
            }
            if let Some(id) = decision.decided_by_speaker_id.as_deref() {
                line.push_str(&format!(" ({})", resolve_label(speakers, Some(id))));
            }
            out.push_str(&format!("- {}\n", line));
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

    if !facts.risks.is_empty() {
        out.push_str("\n## Risks & Blockers\n\n");
        for risk in &facts.risks {
            out.push_str(&format!("- **{}:** {}\n", risk.kind.label(), risk.statement));
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

/// A key point, with its kind made explicit when it is anything but plain
/// discussion.
///
/// Without this the fallback reads a proposal and a settled position
/// identically, which is the exact confusion the `kind` field exists to prevent.
fn render_point(point: &super::model::KeyPoint) -> String {
    match point.kind {
        KeyPointKind::Discussion => point.text.clone(),
        KeyPointKind::Proposal => format!("Proposed: {}", point.text),
        KeyPointKind::Recommendation => format!("Recommended: {}", point.text),
        KeyPointKind::Disagreement => format!("Disagreement: {}", point.text),
        KeyPointKind::Tradeoff => format!("Trade-off: {}", point.text),
    }
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
    use crate::meetings_v2::processing::length::summary_budget;
    use crate::meetings_v2::processing::llm::test_support::ScriptedLlm;
    use crate::meetings_v2::processing::model::{
        ActionItemStatus, Decision, Entity, EntityKind, KeyPoint, MeetingType, OpenQuestion, Risk,
        RiskKind, SegmentChannel, SpeakerOrigin, Topic, SPEAKER_ID_ME, SPEAKER_ID_REMOTE,
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
            key_points: vec![
                KeyPoint {
                    id: "point_0".into(),
                    text: "The release date was settled after weighing the migration risk.".into(),
                    kind: KeyPointKind::Discussion,
                    topic_id: Some("topic_0".into()),
                    source_segment_ids: vec!["seg_00000".into()],
                },
                KeyPoint {
                    id: "point_1".into(),
                    text: "Cutting the release on Thursday instead.".into(),
                    kind: KeyPointKind::Proposal,
                    topic_id: Some("topic_0".into()),
                    source_segment_ids: vec!["seg_00001".into()],
                },
            ],
            topics: vec![Topic {
                id: "topic_0".into(),
                label: "Release Planning".into(),
                segment_ids: vec!["seg_00000".into()],
            }],
            decisions: vec![Decision {
                id: "decision_0".into(),
                statement: "Ship the release on Friday.".into(),
                rationale: Some("the migration script has not been reviewed".into()),
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
                    kanban_card_id: None,
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
                    kanban_card_id: None,
                },
            ],
            open_questions: vec![OpenQuestion {
                id: "question_0".into(),
                question: "Who reviews the migration script?".into(),
                source_segment_ids: vec!["seg_00001".into()],
            }],
            risks: vec![Risk {
                id: "risk_0".into(),
                statement: "The migration script is unreviewed with two days to go.".into(),
                kind: RiskKind::Blocker,
                raised_by_speaker_id: Some(SPEAKER_ID_REMOTE.into()),
                source_segment_ids: vec!["seg_00001".into()],
            }],
            entities: vec![Entity {
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

    fn input<'a>(
        facts: &'a MeetingFacts,
        speakers: &'a [Speaker],
        extension: &'a MeetingExtension,
        notes: &'a MeetingNotes,
        mode: SummaryMode,
    ) -> SummaryInput<'a> {
        SummaryInput {
            facts,
            speakers,
            budget: summary_budget(1_400, mode),
            extension,
            notes,
            user_instructions: None,
        }
    }

    #[test]
    fn the_deterministic_renderer_produces_the_required_shape() {
        let rendered = render_markdown(&facts(), &roster(), SummaryMode::Standard);

        assert!(rendered.starts_with("## Overview"));
        assert!(rendered.contains("## Discussion"));
        assert!(rendered.contains("### Release Planning"));
        assert!(rendered.contains("## Decisions"));
        assert!(rendered.contains("## Action Items"));
        assert!(rendered.contains("## Risks & Blockers"));
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
    fn the_fallback_keeps_the_reason_behind_a_decision() {
        // A provider outage must not quietly cost the most valuable part of the
        // record. "Ship Friday" is a note; "Ship Friday because the migration
        // script has not been reviewed" is the memory.
        let rendered = render_markdown(&facts(), &roster(), SummaryMode::Standard);
        assert!(rendered.contains("because the migration script has not been reviewed"));
    }

    #[test]
    fn the_fallback_never_reads_a_proposal_as_a_settled_position() {
        let rendered = render_markdown(&facts(), &roster(), SummaryMode::Standard);
        let proposal_line = rendered
            .lines()
            .find(|l| l.contains("Cutting the release on Thursday"))
            .expect("the proposal is rendered");
        assert!(proposal_line.starts_with("- Proposed:"));
        // And it never migrates into the Decisions section.
        let decisions = rendered.split("## Decisions").nth(1).unwrap();
        assert!(!decisions.contains("Thursday"));
    }

    #[test]
    fn empty_sections_are_omitted_entirely() {
        let mut sparse = facts();
        sparse.decisions.clear();
        sparse.action_items.clear();
        sparse.open_questions.clear();
        sparse.risks.clear();

        let rendered = render_markdown(&sparse, &roster(), SummaryMode::Concise);
        assert!(!rendered.contains("## Decisions"));
        assert!(!rendered.contains("## Action Items"));
        assert!(!rendered.contains("## Risks"));
        assert!(!rendered.contains("## Open Questions"));
        assert!(
            !rendered.contains("None"),
            "an absent section is omitted, never filled with a placeholder"
        );
        assert!(rendered.contains("## Overview"));
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
            risks: Vec::new(),
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
                kind: KeyPointKind::Discussion,
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
        let llm = ScriptedLlm::new(vec![Ok("## Overview\n\nWe cut the release.".to_string())]);
        let default = builtin_extensions()[0].clone();
        let facts = facts();
        let roster = roster();
        let notes = MeetingNotes::default();

        let out = generate_summary(
            &llm,
            &input(&facts, &roster, &default, &notes, SummaryMode::Standard),
        )
        .await;
        assert!(!out.deterministic);
        assert_eq!(out.markdown, "## Overview\n\nWe cut the release.");
        assert!(out.llm_error.is_none());
    }

    #[tokio::test]
    async fn the_model_is_never_shown_the_transcript_only_the_facts() {
        let llm = ScriptedLlm::new(vec![Ok("## Overview\n\nDone.".to_string())]);
        let default = builtin_extensions()[0].clone();
        let facts = facts();
        let roster = roster();
        let notes = MeetingNotes::default();
        generate_summary(
            &llm,
            &input(&facts, &roster, &default, &notes, SummaryMode::Standard),
        )
        .await;

        let calls = llm.calls.lock().unwrap();
        let (system, user) = &calls[0];
        assert!(
            user.contains("\"discussion_by_topic\""),
            "Stage B receives structured facts"
        );
        assert!(
            !user.contains("seg_00000"),
            "segment ids are not part of Stage B's input"
        );
        assert!(system.contains("You did not"));
    }

    #[tokio::test]
    async fn the_model_is_told_this_meetings_size_budget() {
        // The regression this pins: a fixed per-mode cap the model was judged
        // against but never shown.
        let llm = ScriptedLlm::new(vec![Ok("## Overview\n\nDone.".to_string())]);
        let default = builtin_extensions()[0].clone();
        let facts = facts();
        let roster = roster();
        let notes = MeetingNotes::default();
        let request = input(&facts, &roster, &default, &notes, SummaryMode::Standard);
        let expected = request.budget.max_words.to_string();

        generate_summary(&llm, &request).await;

        let calls = llm.calls.lock().unwrap();
        assert!(calls[0].1.contains(&expected), "the budget must be stated");
    }

    #[tokio::test]
    async fn a_proposal_reaches_the_model_labelled_as_one() {
        let llm = ScriptedLlm::new(vec![Ok("## Overview\n\nDone.".to_string())]);
        let default = builtin_extensions()[0].clone();
        let facts = facts();
        let roster = roster();
        let notes = MeetingNotes::default();
        generate_summary(
            &llm,
            &input(&facts, &roster, &default, &notes, SummaryMode::Standard),
        )
        .await;

        let calls = llm.calls.lock().unwrap();
        assert!(calls[0].1.contains("\"this_was_only_a\": \"proposal\""));
    }

    #[tokio::test]
    async fn notes_reach_the_model_as_emphasis_and_are_absent_when_there_are_none() {
        let default = builtin_extensions()[0].clone();
        let facts = facts();
        let roster = roster();

        let with_notes = MeetingNotes {
            during: "the migration review is the real blocker".to_string(),
            ..Default::default()
        };
        let llm = ScriptedLlm::new(vec![Ok("## Overview\n\nDone.".to_string())]);
        generate_summary(
            &llm,
            &input(&facts, &roster, &default, &with_notes, SummaryMode::Standard),
        )
        .await;
        {
            let calls = llm.calls.lock().unwrap();
            assert!(calls[0].1.contains("the migration review is the real blocker"));
            assert!(calls[0].0.contains("THE USER'S NOTES"));
        }

        let without = MeetingNotes::default();
        let llm = ScriptedLlm::new(vec![Ok("## Overview\n\nDone.".to_string())]);
        generate_summary(
            &llm,
            &input(&facts, &roster, &default, &without, SummaryMode::Standard),
        )
        .await;
        let calls = llm.calls.lock().unwrap();
        assert!(
            !calls[0].0.contains("THE USER'S NOTES"),
            "no notes means no notes section — never an empty one"
        );
        assert!(!calls[0].1.to_lowercase().contains("no notes"));
    }

    #[tokio::test]
    async fn a_users_own_instructions_are_included_and_subordinated_to_accuracy() {
        let llm = ScriptedLlm::new(vec![Ok("## Overview\n\nDone.".to_string())]);
        let default = builtin_extensions()[0].clone();
        let facts = facts();
        let roster = roster();
        let notes = MeetingNotes::default();
        let mut request = input(&facts, &roster, &default, &notes, SummaryMode::Standard);
        request.user_instructions = Some("Always assign every task to someone.");

        generate_summary(&llm, &request).await;

        let calls = llm.calls.lock().unwrap();
        let system = &calls[0].0;
        assert!(system.contains("Always assign every task to someone."));
        assert!(
            system.contains("subordinate to sections"),
            "a user instruction must never be able to license an invented owner"
        );
    }

    #[tokio::test]
    async fn a_repair_tells_the_model_what_was_wrong_and_changes_the_prompt() {
        let llm = ScriptedLlm::new(vec![
            Ok("Here is your summary!\n\n## Overview\n\nFirst try.".to_string()),
            Ok("## Overview\n\nSecond try.".to_string()),
        ]);
        let default = builtin_extensions()[0].clone();
        let facts = facts();
        let roster = roster();
        let notes = MeetingNotes::default();
        let request = input(&facts, &roster, &default, &notes, SummaryMode::Standard);

        let first = generate_summary(&llm, &request).await;
        let second = repair_summary(&llm, &request, "CORRECTION — remove the preamble").await;

        assert!(second.markdown.starts_with("## Overview"));
        let calls = llm.calls.lock().unwrap();
        assert_ne!(
            calls[0].1, calls[1].1,
            "a repair must not re-send an identical prompt"
        );
        assert!(calls[1].1.starts_with("CORRECTION"));
        assert!(first.markdown.contains("Here is your summary"));
    }

    #[tokio::test]
    async fn an_unavailable_model_still_yields_a_summary() {
        let llm = ScriptedLlm::always_unavailable();
        let default = builtin_extensions()[0].clone();
        let facts = facts();
        let roster = roster();
        let notes = MeetingNotes::default();

        let out = generate_summary(
            &llm,
            &input(&facts, &roster, &default, &notes, SummaryMode::Standard),
        )
        .await;
        assert!(out.deterministic);
        assert!(out.llm_error.is_some());
        assert!(out.markdown.contains("## Action Items"));
    }

    #[tokio::test]
    async fn empty_model_prose_falls_back_rather_than_showing_a_blank_summary() {
        // Empty twice: once from the full contract, once from the compact one.
        let llm = ScriptedLlm::new(vec![Ok("   \n  ".to_string()), Ok(String::new())]);
        let default = builtin_extensions()[0].clone();
        let facts = facts();
        let roster = roster();
        let notes = MeetingNotes::default();

        let out = generate_summary(
            &llm,
            &input(&facts, &roster, &default, &notes, SummaryMode::Standard),
        )
        .await;
        assert!(out.deterministic);
        assert!(out.markdown.contains("## Overview"));
        // The recorded reason has to say a model declined to write, twice —
        // "empty response" alone reads to a user as a fault in Relay.
        let reason = out.llm_error.expect("the fallback records why it happened");
        assert!(reason.contains("twice"), "{reason}");
    }

    #[tokio::test]
    async fn an_empty_answer_is_retried_against_a_shorter_contract() {
        // The reported failure: a short meeting, a 3B local model, and
        // "Summary unavailable — model returned an empty response". An empty
        // answer is not an opinion about the facts, so the same facts go back
        // behind a contract short enough for a small model to follow.
        let llm = ScriptedLlm::new(vec![
            Ok(String::new()),
            Ok("## Overview\n\nThe launch moved to Monday.".to_string()),
        ]);
        let default = builtin_extensions()[0].clone();
        let facts = facts();
        let roster = roster();
        let notes = MeetingNotes::default();

        let out = generate_summary(
            &llm,
            &input(&facts, &roster, &default, &notes, SummaryMode::Standard),
        )
        .await;

        assert!(
            !out.deterministic,
            "the retry answered, so this is a model's summary: {}",
            out.markdown
        );
        assert!(out.markdown.contains("The launch moved to Monday"));
        assert_eq!(out.llm_error, None);

        let calls = llm.calls.lock().unwrap();
        assert_eq!(calls.len(), 2, "one full attempt and one compact retry");
        assert!(
            calls[1].0.len() < calls[0].0.len() / 2,
            "the retry contract must actually be shorter: {} vs {}",
            calls[1].0.len(),
            calls[0].0.len()
        );
        // Same facts, both times. A retry that changed the input would be a
        // different summary, not a second attempt at this one.
        assert_eq!(calls[0].1, calls[1].1);
    }

    #[tokio::test]
    async fn the_compact_retry_keeps_the_accuracy_rules_and_the_users_instructions() {
        let llm = ScriptedLlm::new(vec![
            Ok(String::new()),
            Ok("## Overview\n\nShort.".to_string()),
        ]);
        let default = builtin_extensions()[0].clone();
        let facts = facts();
        let roster = roster();
        let notes = MeetingNotes::default();
        let mut summary_input = input(&facts, &roster, &default, &notes, SummaryMode::Standard);
        summary_input.user_instructions = Some("Lead with anything affecting the release date.");

        generate_summary(&llm, &summary_input).await;

        let calls = llm.calls.lock().unwrap();
        let compact = &calls[1].0;
        // Shorter, not laxer. Dropping the invention rules to save tokens would
        // trade a blank summary for a wrong one.
        assert!(compact.contains("Never invent"), "{compact}");
        assert!(compact.contains("Unassigned"), "{compact}");
        assert!(compact.contains("proposal"), "{compact}");
        assert!(compact.contains("## Overview"), "{compact}");
        // The user asked for a shape; a silent retry must not quietly drop it.
        assert!(compact.contains("release date"), "{compact}");
    }

    #[tokio::test]
    async fn an_unreachable_provider_is_not_retried() {
        // A shorter prompt does not make a provider reachable. Retrying there
        // only doubles the wait before the fallback the user was always getting.
        let llm = ScriptedLlm::new(vec![
            Err(LlmError::Unavailable("connection refused".to_string())),
            Ok("## Overview\n\nShould never be reached.".to_string()),
        ]);
        let default = builtin_extensions()[0].clone();
        let facts = facts();
        let roster = roster();
        let notes = MeetingNotes::default();

        let out = generate_summary(
            &llm,
            &input(&facts, &roster, &default, &notes, SummaryMode::Standard),
        )
        .await;

        assert!(out.deterministic);
        assert_eq!(llm.calls.lock().unwrap().len(), 1);
        assert!(out.llm_error.unwrap().contains("connection refused"));
    }

    #[tokio::test]
    async fn an_extension_changes_the_instructions_not_the_facts() {
        let brief = builtin_extensions()
            .into_iter()
            .find(|e| e.id == "executive_brief")
            .unwrap();
        let llm = ScriptedLlm::new(vec![Ok("## Overview\n\nShort.".to_string())]);
        let facts = facts();
        let roster = roster();
        let notes = MeetingNotes::default();

        generate_summary(
            &llm,
            &input(&facts, &roster, &brief, &notes, SummaryMode::Standard),
        )
        .await;

        let calls = llm.calls.lock().unwrap();
        let (system, user) = &calls[0];
        assert!(system.contains("Executive Brief"));
        assert!(system.contains("two minutes"));
        assert!(
            system.contains("It never permits"),
            "presentation must never license a claim the facts do not carry"
        );
        // The facts payload is identical regardless of extension.
        assert!(user.contains("Ship the release on Friday."));
    }

    #[test]
    fn a_fenced_markdown_response_is_unwrapped() {
        assert_eq!(
            strip_code_fence("```markdown\n## Overview\n\nText.\n```"),
            "## Overview\n\nText."
        );
        assert_eq!(strip_code_fence("## Overview"), "## Overview");
        // An unterminated fence must not swallow the content.
        assert_eq!(strip_code_fence("```\n## Overview"), "## Overview");
    }
}
