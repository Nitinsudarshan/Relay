//! Structured conversation context extraction for captured AI conversations.
//!
//! # Architecture: Source vs. Context
//! - The captured conversation (`original/*.json` and `VaultFile.content`) is immutable source evidence.
//! - `ConversationContext` is derived understanding, stored in `context.json` alongside the capture.
//! - Re-analyzing context never overwrites or risks the raw source.
//! - Every derived item carries `source_turn_ordinals` linking back to the message turns it originated from.

use serde::{Deserialize, Serialize};
use std::collections::HashSet;

use crate::pipeline::source_boundary;
use super::WebCapturePayload;
use crate::providers::LLMClient;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum DecisionStatus {
    #[default]
    Current,
    Superseded,
    Modified,
    Rejected,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ContextDecision {
    pub id: String,
    pub decision: String,
    #[serde(default)]
    pub rationale: Option<String>,
    #[serde(default)]
    pub status: DecisionStatus,
    #[serde(default)]
    pub source_turn_ordinals: Vec<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ContextRequirement {
    pub id: String,
    pub statement: String,
    #[serde(default)]
    pub source_turn_ordinals: Vec<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ContextConstraint {
    pub id: String,
    pub statement: String,
    #[serde(default)]
    pub reason: Option<String>,
    #[serde(default)]
    pub source_turn_ordinals: Vec<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RejectedApproach {
    pub approach: String,
    #[serde(default)]
    pub reason_rejected: String,
    #[serde(default)]
    pub source_turn_ordinals: Vec<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ContextActionItem {
    pub id: String,
    pub description: String,
    #[serde(default)]
    pub owner: Option<String>,
    #[serde(default = "default_action_status")]
    pub status: String,
    #[serde(default)]
    pub source_turn_ordinals: Vec<u32>,
}

fn default_action_status() -> String {
    "OPEN".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ContextOpenQuestion {
    pub id: String,
    pub question: String,
    #[serde(default)]
    pub context_note: Option<String>,
    #[serde(default)]
    pub source_turn_ordinals: Vec<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ContextArtifact {
    pub name: String,
    pub kind: String,
    #[serde(default)]
    pub reference_or_path: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
}

/// The canonical derived context representation for a captured AI conversation.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ConversationContext {
    pub capture_id: String,
    pub title: String,
    pub objective: String,
    #[serde(default)]
    pub background: Vec<String>,
    pub current_state: String,
    #[serde(default)]
    pub decisions: Vec<ContextDecision>,
    #[serde(default)]
    pub requirements: Vec<ContextRequirement>,
    #[serde(default)]
    pub constraints: Vec<ContextConstraint>,
    #[serde(default)]
    pub preferences: Vec<String>,
    #[serde(default)]
    pub rejected_approaches: Vec<RejectedApproach>,
    #[serde(default)]
    pub open_questions: Vec<ContextOpenQuestion>,
    #[serde(default)]
    pub action_items: Vec<ContextActionItem>,
    #[serde(default)]
    pub important_facts: Vec<String>,
    #[serde(default)]
    pub key_artifacts: Vec<ContextArtifact>,
    pub generated_at: String,
    #[serde(default)]
    pub model: Option<String>,
    #[serde(default)]
    pub deterministic: bool,
}

/// Canonical derived structured context for any captured source (conversations, repositories, documents).
pub type SourceContext = ConversationContext;

pub const CONTEXT_EXTRACTION_SYSTEM_PROMPT: &str = r#"
You are Relay's Conversation Intelligence Engine.
Your task is to analyze a captured AI conversation and extract structured, high-signal work context to power a Context Handoff.

The receiving AI needs to continue this work seamlessly without having to read hundreds of raw chat messages.
Do NOT output a long essay or generic summary.
Extract specific, concrete, structured elements from the conversation turns.

Return ONLY a valid JSON object with the following fields:
{
  "objective": "A concise, clear statement of what the user is trying to accomplish or solve",
  "background": ["Crucial project or technical context necessary to understand the work"],
  "current_state": "Where the conversation ended and where the work currently stands",
  "decisions": [
    {
      "decision": "Concrete decision settled in the thread",
      "rationale": "Why it was decided this way",
      "source_turn_ordinals": [1, 2]
    }
  ],
  "requirements": [
    {
      "statement": "Must-have condition or specification the solution must meet",
      "source_turn_ordinals": [1]
    }
  ],
  "constraints": [
    {
      "statement": "Non-negotiable constraint or boundary (e.g. technology, performance, security, architecture)",
      "reason": "Why this constraint exists",
      "source_turn_ordinals": [1]
    }
  ],
  "preferences": [
    "User styling, architectural, or workflow preferences expressed"
  ],
  "rejected_approaches": [
    {
      "approach": "Approach or technology considered and dismissed",
      "reason_rejected": "Why it was rejected or failed",
      "source_turn_ordinals": [1]
    }
  ],
  "open_questions": [
    {
      "question": "Unresolved question, open decision, or uncertainty remaining",
      "context_note": "Context surrounding the open question",
      "source_turn_ordinals": [1]
    }
  ],
  "action_items": [
    {
      "description": "Concrete next step that needs to happen",
      "owner": "user, assistant, or specific role if stated",
      "source_turn_ordinals": [1]
    }
  ],
  "important_facts": [
    "Established domain truths, architecture facts, or verified findings"
  ],
  "key_artifacts": [
    {
      "name": "Filename, code module, diagram, or file",
      "kind": "code | file | diagram | document",
      "description": "Brief description of what this artifact contains or does"
    }
  ]
}

Ensure all arrays contain only meaningful items explicitly grounded in the conversation. Do not hallucinate facts.
"#;

#[derive(Debug, Deserialize)]
struct RawLlmContextResponse {
    #[serde(default)]
    objective: Option<String>,
    #[serde(default)]
    background: Vec<String>,
    #[serde(default)]
    current_state: Option<String>,
    #[serde(default)]
    decisions: Vec<RawDecision>,
    #[serde(default)]
    requirements: Vec<RawRequirement>,
    #[serde(default)]
    constraints: Vec<RawConstraint>,
    #[serde(default)]
    preferences: Vec<String>,
    #[serde(default)]
    rejected_approaches: Vec<RawRejectedApproach>,
    #[serde(default)]
    open_questions: Vec<RawOpenQuestion>,
    #[serde(default)]
    action_items: Vec<RawActionItem>,
    #[serde(default)]
    important_facts: Vec<String>,
    #[serde(default)]
    key_artifacts: Vec<RawContextArtifact>,
}

#[derive(Debug, Deserialize)]
struct RawDecision {
    decision: String,
    #[serde(default)]
    rationale: Option<String>,
    #[serde(default)]
    source_turn_ordinals: Vec<u32>,
}

#[derive(Debug, Deserialize)]
struct RawRequirement {
    statement: String,
    #[serde(default)]
    source_turn_ordinals: Vec<u32>,
}

#[derive(Debug, Deserialize)]
struct RawConstraint {
    statement: String,
    #[serde(default)]
    reason: Option<String>,
    #[serde(default)]
    source_turn_ordinals: Vec<u32>,
}

#[derive(Debug, Deserialize)]
struct RawRejectedApproach {
    approach: String,
    #[serde(default)]
    reason_rejected: String,
    #[serde(default)]
    source_turn_ordinals: Vec<u32>,
}

#[derive(Debug, Deserialize)]
struct RawActionItem {
    description: String,
    #[serde(default)]
    owner: Option<String>,
    #[serde(default)]
    source_turn_ordinals: Vec<u32>,
}

#[derive(Debug, Deserialize)]
struct RawOpenQuestion {
    question: String,
    #[serde(default)]
    context_note: Option<String>,
    #[serde(default)]
    source_turn_ordinals: Vec<u32>,
}

#[derive(Debug, Deserialize)]
struct RawContextArtifact {
    name: String,
    #[serde(default = "default_artifact_kind")]
    kind: String,
    #[serde(default)]
    description: Option<String>,
}

fn default_artifact_kind() -> String {
    "code".to_string()
}

/// Extracts structured conversation context from a captured payload.
///
/// Uses the LLM client when available with source-boundary isolation; falls back
/// to deterministic extraction if the LLM is unreachable or disabled.
pub async fn extract_conversation_context(
    llm: Option<&LLMClient>,
    capture_id: &str,
    payload: &WebCapturePayload,
    normalized_markdown: &str,
) -> ConversationContext {
    let now = chrono::Utc::now().to_rfc3339();
    let title = payload
        .title
        .clone()
        .unwrap_or_else(|| "AI Conversation".to_string());

    if let Some(client) = llm {
        let source_desc = format!(
            "Captured AI conversation from {} ({})",
            payload.extractor.id, payload.url
        );
        let wrapped = source_boundary::wrap_external_source(&source_desc, normalized_markdown);
        let system_prompt = format!(
            "{}\n{}",
            CONTEXT_EXTRACTION_SYSTEM_PROMPT,
            source_boundary::EXTERNAL_SOURCE_RULE
        );

        match client.complete(&wrapped.framed, Some(&system_prompt)).await {
            Ok(response) => {
                if let Some(parsed) = parse_llm_context_response(&response.text) {
                    return build_context_from_parsed(
                        capture_id,
                        &title,
                        parsed,
                        now,
                        Some(format!("{:?}", client.provider_type())),
                    );
                } else {
                    tracing::warn!(
                        "Failed to parse LLM context JSON for capture {}. Falling back to deterministic extraction.",
                        capture_id
                    );
                }
            }
            Err(err) => {
                tracing::warn!(
                    "LLM context extraction failed for capture {}: {}. Falling back to deterministic extraction.",
                    capture_id,
                    err
                );
            }
        }
    }

    extract_deterministic_context(capture_id, &title, payload, &now)
}

fn parse_llm_context_response(raw: &str) -> Option<RawLlmContextResponse> {
    let text = raw.trim();
    let json_str = if text.contains("```json") {
        text.split("```json")
            .nth(1)?
            .split("```")
            .next()?
            .trim()
    } else if text.contains("```") {
        text.split("```")
            .nth(1)?
            .split("```")
            .next()?
            .trim()
    } else {
        text
    };

    serde_json::from_str::<RawLlmContextResponse>(json_str).ok()
}

fn build_context_from_parsed(
    capture_id: &str,
    title: &str,
    parsed: RawLlmContextResponse,
    now: String,
    model: Option<String>,
) -> ConversationContext {
    let objective = parsed
        .objective
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| format!("Work on {}", title));

    let current_state = parsed
        .current_state
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| "Discussion captured and ready for handoff.".to_string());

    let decisions = parsed
        .decisions
        .into_iter()
        .enumerate()
        .map(|(idx, d)| ContextDecision {
            id: format!("dec_{}", idx + 1),
            decision: d.decision.trim().to_string(),
            rationale: d.rationale.filter(|r| !r.trim().is_empty()),
            status: DecisionStatus::Current,
            source_turn_ordinals: d.source_turn_ordinals,
        })
        .filter(|d| !d.decision.is_empty())
        .collect();

    let requirements = parsed
        .requirements
        .into_iter()
        .enumerate()
        .map(|(idx, r)| ContextRequirement {
            id: format!("req_{}", idx + 1),
            statement: r.statement.trim().to_string(),
            source_turn_ordinals: r.source_turn_ordinals,
        })
        .filter(|r| !r.statement.is_empty())
        .collect();

    let constraints = parsed
        .constraints
        .into_iter()
        .enumerate()
        .map(|(idx, c)| ContextConstraint {
            id: format!("con_{}", idx + 1),
            statement: c.statement.trim().to_string(),
            reason: c.reason.filter(|r| !r.trim().is_empty()),
            source_turn_ordinals: c.source_turn_ordinals,
        })
        .filter(|c| !c.statement.is_empty())
        .collect();

    let rejected_approaches = parsed
        .rejected_approaches
        .into_iter()
        .map(|r| RejectedApproach {
            approach: r.approach.trim().to_string(),
            reason_rejected: r.reason_rejected.trim().to_string(),
            source_turn_ordinals: r.source_turn_ordinals,
        })
        .filter(|r| !r.approach.is_empty())
        .collect();

    let open_questions = parsed
        .open_questions
        .into_iter()
        .enumerate()
        .map(|(idx, q)| ContextOpenQuestion {
            id: format!("q_{}", idx + 1),
            question: q.question.trim().to_string(),
            context_note: q.context_note.filter(|n| !n.trim().is_empty()),
            source_turn_ordinals: q.source_turn_ordinals,
        })
        .filter(|q| !q.question.is_empty())
        .collect();

    let action_items = parsed
        .action_items
        .into_iter()
        .enumerate()
        .map(|(idx, a)| ContextActionItem {
            id: format!("act_{}", idx + 1),
            description: a.description.trim().to_string(),
            owner: a.owner.filter(|o| !o.trim().is_empty()),
            status: "OPEN".to_string(),
            source_turn_ordinals: a.source_turn_ordinals,
        })
        .filter(|a| !a.description.is_empty())
        .collect();

    let key_artifacts = parsed
        .key_artifacts
        .into_iter()
        .map(|a| ContextArtifact {
            name: a.name.trim().to_string(),
            kind: a.kind.trim().to_string(),
            reference_or_path: None,
            description: a.description.filter(|d| !d.trim().is_empty()),
        })
        .filter(|a| !a.name.is_empty())
        .collect();

    ConversationContext {
        capture_id: capture_id.to_string(),
        title: title.to_string(),
        objective,
        background: parsed.background,
        current_state,
        decisions,
        requirements,
        constraints,
        preferences: parsed.preferences,
        rejected_approaches,
        open_questions,
        action_items,
        important_facts: parsed.important_facts,
        key_artifacts,
        generated_at: now,
        model,
        deterministic: false,
    }
}

/// Deterministically extracts high-value conversation context using turn analysis and cue scanning.
///
/// Ensures Relay always produces an honest, well-formed context model even when completely offline
/// or when no LLM is configured.
#[derive(Default)]
struct ContextAccumulator {
    decisions: Vec<ContextDecision>,
    seen_decisions: HashSet<String>,
    requirements: Vec<ContextRequirement>,
    constraints: Vec<ContextConstraint>,
    open_questions: Vec<ContextOpenQuestion>,
    seen_questions: HashSet<String>,
    action_items: Vec<ContextActionItem>,
    seen_actions: HashSet<String>,
}

/// Deterministically extracts high-value conversation context using turn analysis and cue scanning.
///
/// Ensures Relay always produces an honest, well-formed context model even when completely offline
/// or when no LLM is configured.
pub fn extract_deterministic_context(
    capture_id: &str,
    title: &str,
    payload: &WebCapturePayload,
    generated_at: &str,
) -> ConversationContext {
    let mut acc = ContextAccumulator::default();
    let mut key_artifacts = Vec::new();
    let mut objective = String::new();
    let mut current_state = String::new();

    let messages = &payload.content.messages;

    // Scan the initial user turns to identify the core objective
    for (turn_idx, msg) in messages.iter().enumerate() {
        let ordinal = msg.ordinal.unwrap_or((turn_idx + 1) as u32);
        if msg.role.to_lowercase() == "user" && objective.is_empty() {
            for block in &msg.blocks {
                if let super::ContentBlock::Paragraph { text } = block {
                    let trimmed = text.trim();
                    if !trimmed.is_empty() {
                        objective = trimmed.chars().take(240).collect();
                        break;
                    }
                }
            }
        }

        // Scan turn blocks for decisions, questions, actions, and constraints
        for block in &msg.blocks {
            match block {
                super::ContentBlock::Paragraph { text } | super::ContentBlock::Quote { text } => {
                    scan_text_for_cues(text, ordinal, &mut acc);
                }
                super::ContentBlock::List { items, .. } => {
                    for item in items {
                        scan_text_for_cues(item, ordinal, &mut acc);
                    }
                }
                super::ContentBlock::Attachment { name: Some(n), kind, preview, reference, .. } => {
                    key_artifacts.push(ContextArtifact {
                        name: n.clone(),
                        kind: kind.clone().unwrap_or_else(|| "file".to_string()),
                        reference_or_path: reference.clone(),
                        description: preview.clone(),
                    });
                }
                super::ContentBlock::Code { language, text } if text.lines().count() > 5 => {
                    let lang = language.as_deref().unwrap_or("code");
                    let snippet: String = text.lines().take(2).collect::<Vec<_>>().join(" ");
                    key_artifacts.push(ContextArtifact {
                        name: format!("{} snippet (turn {})", lang, ordinal),
                        kind: "code".to_string(),
                        reference_or_path: None,
                        description: Some(snippet.chars().take(120).collect()),
                    });
                }
                _ => {}
            }
        }
    }

    if objective.is_empty() {
        for block in &payload.content.blocks {
            if let super::ContentBlock::Paragraph { text } = block {
                let trimmed = text.trim();
                if !trimmed.is_empty() {
                    objective = trimmed.chars().take(240).collect();
                    break;
                }
            }
        }
        if objective.is_empty() {
            objective = format!("Explore and work on {}", title);
        }
    }

    if let Some(last_msg) = messages.last() {
        for block in &last_msg.blocks {
            if let super::ContentBlock::Paragraph { text } = block {
                let trimmed = text.trim();
                if !trimmed.is_empty() {
                    current_state = trimmed.chars().take(220).collect();
                    break;
                }
            }
        }
    } else if let Some(last_block) = payload.content.blocks.last() {
        if let super::ContentBlock::Paragraph { text } = last_block {
            current_state = text.trim().chars().take(220).collect();
        } else {
            current_state = format!("Captured document snapshot of {}", title);
        }
    }

    if current_state.is_empty() {
        current_state = format!(
            "Captured {} turns from {}. Ready for handoff.",
            messages.len(),
            payload.extractor.id
        );
    }

    acc.decisions.truncate(10);
    acc.requirements.truncate(8);
    acc.constraints.truncate(6);
    acc.open_questions.truncate(8);
    acc.action_items.truncate(10);
    key_artifacts.truncate(10);

    ConversationContext {
        capture_id: capture_id.to_string(),
        title: title.to_string(),
        objective,
        background: vec![format!("Captured from {} ({})", payload.extractor.id, payload.url)],
        current_state,
        decisions: acc.decisions,
        requirements: acc.requirements,
        constraints: acc.constraints,
        preferences: vec![],
        rejected_approaches: vec![],
        open_questions: acc.open_questions,
        action_items: acc.action_items,
        important_facts: vec![],
        key_artifacts,
        generated_at: generated_at.to_string(),
        model: None,
        deterministic: true,
    }
}

fn scan_text_for_cues(
    text: &str,
    ordinal: u32,
    acc: &mut ContextAccumulator,
) {
    let lower = text.to_lowercase();

    // Decisions
    for cue in &[
        "we decided to",
        "we've decided to",
        "the decision is",
        "let's go with",
        "agreed to",
        "we settled on",
        "the consensus is",
        "we will use",
    ] {
        if let Some(pos) = lower.find(cue) {
            let snippet = extract_sentence_from(&text[pos..]);
            let key = snippet.to_lowercase();
            if !snippet.is_empty() && !acc.seen_decisions.contains(&key) {
                acc.seen_decisions.insert(key);
                acc.decisions.push(ContextDecision {
                    id: format!("dec_{}", acc.decisions.len() + 1),
                    decision: snippet,
                    rationale: None,
                    status: DecisionStatus::Current,
                    source_turn_ordinals: vec![ordinal],
                });
            }
        }
    }

    // Constraints & Requirements
    for cue in &["must not", "cannot", "is required to", "must always", "restricted to", "constraint:"] {
        if let Some(pos) = lower.find(cue) {
            let snippet = extract_sentence_from(&text[pos..]);
            if !snippet.is_empty() {
                acc.constraints.push(ContextConstraint {
                    id: format!("con_{}", acc.constraints.len() + 1),
                    statement: snippet,
                    reason: None,
                    source_turn_ordinals: vec![ordinal],
                });
            }
        }
    }

    for cue in &["must have", "requirement:", "needs to support", "we need to ensure that"] {
        if let Some(pos) = lower.find(cue) {
            let snippet = extract_sentence_from(&text[pos..]);
            if !snippet.is_empty() {
                acc.requirements.push(ContextRequirement {
                    id: format!("req_{}", acc.requirements.len() + 1),
                    statement: snippet,
                    source_turn_ordinals: vec![ordinal],
                });
            }
        }
    }

    // Open Questions
    if text.contains('?') {
        for sentence in text.split(&['.', '\n'][..]) {
            if sentence.contains('?') {
                let clean = sentence.trim();
                let key = clean.to_lowercase();
                if clean.len() >= 12 && !acc.seen_questions.contains(&key) {
                    acc.seen_questions.insert(key);
                    acc.open_questions.push(ContextOpenQuestion {
                        id: format!("q_{}", acc.open_questions.len() + 1),
                        question: clean.to_string(),
                        context_note: None,
                        source_turn_ordinals: vec![ordinal],
                    });
                }
            }
        }
    }

    // Action Items
    for cue in &[
        "next step is",
        "action item:",
        "todo:",
        "i will",
        "we will follow up",
        "please make sure to",
    ] {
        if let Some(pos) = lower.find(cue) {
            let snippet = extract_sentence_from(&text[pos..]);
            let key = snippet.to_lowercase();
            if !snippet.is_empty() && !acc.seen_actions.contains(&key) {
                acc.seen_actions.insert(key);
                acc.action_items.push(ContextActionItem {
                    id: format!("act_{}", acc.action_items.len() + 1),
                    description: snippet,
                    owner: None,
                    status: "OPEN".to_string(),
                    source_turn_ordinals: vec![ordinal],
                });
            }
        }
    }
}

fn extract_sentence_from(slice: &str) -> String {
    let first = slice.split(&['\n', ';'][..]).next().unwrap_or(slice).trim();
    first.trim_matches(&[' ', '-', '*', '•', ':'][..]).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::capture::web::{CaptureContent, CaptureContentKind, CaptureMessage, ContentBlock, ExtractorInfo};

    fn sample_conversation_payload() -> WebCapturePayload {
        WebCapturePayload {
            protocol_version: 1,
            captured_at: Some("2026-02-14T09:30:00Z".to_string()),
            url: "https://chatgpt.com/c/test-123".to_string(),
            title: Some("Architecture Discussion".to_string()),
            browser: Some("Chrome".to_string()),
            extractor: ExtractorInfo {
                id: "chatgpt".to_string(),
                version: 2,
                strategy: "site".to_string(),
            },
            document: Default::default(),
            content: CaptureContent {
                kind: CaptureContentKind::Conversation,
                blocks: vec![],
                messages: vec![
                    CaptureMessage {
                        role: "user".to_string(),
                        blocks: vec![ContentBlock::Paragraph {
                            text: "We need to design a local-first context handoff system for Relay.".to_string(),
                        }],
                        timestamp: None,
                        ordinal: Some(1),
                    },
                    CaptureMessage {
                        role: "assistant".to_string(),
                        blocks: vec![
                            ContentBlock::Paragraph {
                                text: "We decided to store derived context separately in context.json.".to_string(),
                            },
                            ContentBlock::Paragraph {
                                text: "The system must not require cloud processing for basic capture.".to_string(),
                            },
                            ContentBlock::Paragraph {
                                text: "How should we format the Markdown handoff prompt?".to_string(),
                            },
                            ContentBlock::Paragraph {
                                text: "Next step is to implement the handoff compiler in Rust.".to_string(),
                            },
                        ],
                        timestamp: None,
                        ordinal: Some(2),
                    },
                ],
            },
            links: vec![],
            diagnostics: Default::default(),
        }
    }

    #[test]
    fn deterministic_extraction_extracts_decisions_and_actions() {
        let payload = sample_conversation_payload();
        let context = extract_deterministic_context("cap_1", "Architecture Discussion", &payload, "2026-02-14T10:00:00Z");

        assert_eq!(context.capture_id, "cap_1");
        assert!(context.deterministic);
        assert!(context.objective.contains("local-first context handoff"));
        assert!(!context.decisions.is_empty());
        assert!(context.decisions[0].decision.contains("store derived context"));
        assert_eq!(context.decisions[0].source_turn_ordinals, vec![2]);

        assert!(!context.constraints.is_empty());
        assert!(context.constraints[0].statement.contains("must not require cloud"));

        assert!(!context.open_questions.is_empty());
        assert!(context.open_questions[0].question.contains("How should we format"));

        assert!(!context.action_items.is_empty());
        assert!(context.action_items[0].description.contains("implement the handoff compiler"));
    }

    #[test]
    fn parses_llm_json_wrapped_in_markdown_fence() {
        let raw = r#"Here is the extracted context:
```json
{
  "objective": "Build context handoff",
  "background": ["Relay is a desktop app"],
  "current_state": "Architecture drafted",
  "decisions": [
    {
      "decision": "Use SQLite for local storage",
      "rationale": "High reliability and no external daemon",
      "source_turn_ordinals": [2]
    }
  ],
  "requirements": [],
  "constraints": [],
  "preferences": [],
  "rejected_approaches": [],
  "open_questions": [],
  "action_items": [],
  "important_facts": [],
  "key_artifacts": []
}
```
"#;
        let parsed = parse_llm_context_response(raw).expect("should parse json from markdown fence");
        assert_eq!(parsed.objective.as_deref(), Some("Build context handoff"));
        assert_eq!(parsed.decisions.len(), 1);
        assert_eq!(parsed.decisions[0].decision, "Use SQLite for local storage");
    }
}
