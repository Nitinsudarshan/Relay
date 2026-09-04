//! Structured conversation context extraction for captured AI conversations.
//!
//! # Architecture: Source vs. Context
//! - The captured conversation (`original/*.json` and `VaultFile.content`) is immutable source evidence.
//! - `ConversationContext` is derived understanding, stored in `context.json` alongside the capture.
//! - Re-analyzing context never overwrites or risks the raw source.
//! - Every derived item carries `source_turn_ordinals` linking back to the message turns it originated from.

use serde::{Deserialize, Serialize};
use std::collections::HashSet;

use crate::pipeline::analysis::{
    context_request, AnalysisFailure, AnalysisService, AnalysisType, MetadataBuilder, PromptId,
    SourceDescriptor,
};
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

/// The canonical derived context representation for a captured software repository.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RepositoryContext {
    pub capture_id: String,
    pub repository_name: String,
    pub objective: String,
    #[serde(default)]
    pub stack: Vec<String>,
    #[serde(default)]
    pub features: Vec<String>,
    #[serde(default)]
    pub user_base: Vec<String>,
    #[serde(default)]
    pub licensing: Option<String>,
    pub generated_at: String,
    #[serde(default)]
    pub model: Option<String>,
    #[serde(default)]
    pub deterministic: bool,
}

/// What a context says when the evidence did not support a value.
///
/// Kept as constants, and phrased as statements about the *evidence* rather
/// than about the source, because these strings are the difference between
/// "Relay read this and could not tell" and a fabricated finding. Tests assert
/// on them, and the frontend can recognise them.
pub const INSUFFICIENT_OBJECTIVE_EVIDENCE: &str =
    "The captured conversation does not state an objective clearly enough to extract one.";
pub const INSUFFICIENT_STATE_EVIDENCE: &str =
    "The captured conversation does not establish where the work currently stands.";
pub const INSUFFICIENT_REPOSITORY_OBJECTIVE_EVIDENCE: &str =
    "Insufficient evidence to determine the repository's primary objective.";

/// Distinguishes the specific kind of source context.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "kind", content = "data", rename_all = "snake_case")]
pub enum SourceContextKind {
    Conversation(ConversationContext),
    Repository(RepositoryContext),
}

/// Canonical derived structured context for any captured source (conversations, repositories, documents).
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SourceContext {
    pub capture_id: String,
    pub generated_at: String,
    #[serde(default)]
    pub model: Option<String>,
    #[serde(default)]
    pub deterministic: bool,
    /// Full analysis provenance: status, prompt id and version, the model that
    /// actually answered, and why a fallback ran when one did.
    ///
    /// `Option` because every context written before the analysis contract
    /// existed has none, and a vault must stay readable. `model` and
    /// `deterministic` above are kept as the flattened summary the frontend
    /// already reads, so this is additive on the wire.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub analysis: Option<crate::pipeline::analysis::AnalysisMetadata>,
    #[serde(flatten)]
    pub kind: SourceContextKind,
}

impl SourceContext {
    pub fn conversation(&self) -> Option<&ConversationContext> {
        match &self.kind {
            SourceContextKind::Conversation(c) => Some(c),
            _ => None,
        }
    }

    pub fn repository(&self) -> Option<&RepositoryContext> {
        match &self.kind {
            SourceContextKind::Repository(r) => Some(r),
            _ => None,
        }
    }
}

/// Deserialization-only shim that accepts both the current envelope and the
/// pre-v0.28.4 bare-`ConversationContext` shape.
///
/// `large_enum_variant` is allowed rather than fixed: the `Envelope` variant is
/// large because it inlines a whole context, boxing it would change the public
/// `SourceContext` shape, and a value of this type exists only for the duration
/// of one `from_str` call.
#[allow(clippy::large_enum_variant)]
#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum RawSourceContextHelper {
    Envelope {
        capture_id: String,
        generated_at: String,
        #[serde(default)]
        model: Option<String>,
        #[serde(default)]
        deterministic: bool,
        #[serde(default)]
        analysis: Option<crate::pipeline::analysis::AnalysisMetadata>,
        #[serde(flatten)]
        kind: SourceContextKind,
    },
    LegacyConversation(ConversationContext),
}

impl<'de> Deserialize<'de> for SourceContext {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let helper = RawSourceContextHelper::deserialize(deserializer)?;
        match helper {
            RawSourceContextHelper::Envelope {
                capture_id,
                generated_at,
                model,
                deterministic,
                analysis,
                kind,
            } => Ok(SourceContext {
                capture_id,
                generated_at,
                model,
                deterministic,
                analysis,
                kind,
            }),
            RawSourceContextHelper::LegacyConversation(conv) => Ok(SourceContext {
                capture_id: conv.capture_id.clone(),
                generated_at: conv.generated_at.clone(),
                model: conv.model.clone(),
                deterministic: conv.deterministic,
                // A context written before the analysis contract existed says
                // nothing about how it was produced, and inventing metadata for
                // it would be a claim Relay cannot support.
                analysis: None,
                kind: SourceContextKind::Conversation(conv),
            }),
        }
    }
}

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

#[derive(Debug, Default, Deserialize)]
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

/// Extracts structured context for a captured source.
///
/// One entry point for every source type. Which analysis runs is decided by the
/// prompt registry from the source's own classification — the one capture
/// already derived from the URL and stored on the artifact — rather than by a
/// conditional here. Adding a `DocumentContext` later means adding a prompt and
/// a builder, not another branch.
pub async fn extract_source_context(
    llm: Option<&LLMClient>,
    file: &crate::vault::VaultFile,
    payload: &WebCapturePayload,
) -> SourceContext {
    let source = SourceDescriptor::from_vault_file(file);
    let content = super::canonical::to_canonical(payload, &file.content);
    let service = match llm {
        Some(client) => AnalysisService::new(client),
        None => AnalysisService::offline(),
    };

    let Some(request) = context_request(&source) else {
        // No context schema for this source type. Producing a conversation
        // context for a plain web page because that branch happened to be last
        // is exactly what this dispatch exists to stop.
        tracing::info!(
            "No structured context schema for {} source {}",
            source.source_type.as_str(),
            source.id
        );
        return deterministic_source_context(file, payload);
    };

    match request.prompt_id {
        PromptId::RepositoryContext => {
            let default_name = derive_repository_name(payload);
            let result = service
                .run_structured(
                    &request,
                    &source,
                    &content,
                    |json| {
                        serde_json::from_value::<RawLlmRepositoryResponse>(json.clone())
                            .ok()
                            .map(|parsed| (parsed, default_name.clone()))
                    },
                    || (RawLlmRepositoryResponse::default(), default_name.clone()),
                )
                .await;

            let metadata = result.metadata;
            let repo = match result.payload {
                Some((parsed, name)) if !metadata.deterministic => build_repository_context_from_parsed(
                    &file.id,
                    &name,
                    parsed,
                    &metadata.generated_at,
                    metadata.model.clone(),
                ),
                _ => extract_deterministic_repository_context(
                    &file.id,
                    payload,
                    &file.content,
                    &metadata.generated_at,
                ),
            };
            SourceContext {
                capture_id: file.id.clone(),
                generated_at: metadata.generated_at.clone(),
                model: metadata.model.clone(),
                deterministic: metadata.deterministic,
                kind: SourceContextKind::Repository(repo),
                analysis: Some(metadata),
            }
        }
        _ => {
            let title = payload
                .title
                .clone()
                .unwrap_or_else(|| "AI Conversation".to_string());
            let result = service
                .run_structured(
                    &request,
                    &source,
                    &content,
                    |json| serde_json::from_value::<RawLlmContextResponse>(json.clone()).ok(),
                    RawLlmContextResponse::default,
                )
                .await;

            let metadata = result.metadata;
            let conv = match result.payload {
                Some(parsed) if !metadata.deterministic => build_context_from_parsed(
                    &file.id,
                    &title,
                    parsed,
                    metadata.generated_at.clone(),
                    metadata.model.clone(),
                ),
                _ => extract_deterministic_context(&file.id, &title, payload, &metadata.generated_at),
            };
            SourceContext {
                capture_id: file.id.clone(),
                generated_at: metadata.generated_at.clone(),
                model: metadata.model.clone(),
                deterministic: metadata.deterministic,
                kind: SourceContextKind::Conversation(conv),
                analysis: Some(metadata),
            }
        }
    }
}

/// The context a source gets when no model was involved at all.
fn deterministic_source_context(
    file: &crate::vault::VaultFile,
    payload: &WebCapturePayload,
) -> SourceContext {
    let now = chrono::Utc::now().to_rfc3339();
    let source = SourceDescriptor::from_vault_file(file);
    let metadata = MetadataBuilder::new(AnalysisType::Context, PromptId::ConversationContext, 0)
        .with_coverage(source.coverage)
        .deterministic(AnalysisFailure::PromptNotApplicable(format!(
            "no context schema for a {} source",
            source.source_type.as_str()
        )));

    let title = payload
        .title
        .clone()
        .unwrap_or_else(|| "Captured Source".to_string());
    let conv = extract_deterministic_context(&file.id, &title, payload, &now);

    SourceContext {
        capture_id: file.id.clone(),
        generated_at: now,
        model: None,
        deterministic: true,
        kind: SourceContextKind::Conversation(conv),
        analysis: Some(metadata),
    }
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
        .unwrap_or_else(|| INSUFFICIENT_OBJECTIVE_EVIDENCE.to_string());

    let current_state = parsed
        .current_state
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| INSUFFICIENT_STATE_EVIDENCE.to_string());

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

pub const REPOSITORY_CONTEXT_SYSTEM_PROMPT: &str = r#"
You are Relay's Repository Intelligence Engine.
Your task is to analyze captured software repository documentation and evidence, and extract a concise, structured repository briefing.

The user needs to quickly understand:
1. What this repository is (Objective)
2. What technical signals are grounded in captured evidence (Stack)
3. What product capabilities and ecosystem it offers (Features / Ecosystem)
4. Who it is for (User Base)
5. How it is licensed (Licensing)

CRITICAL EXTRACTION RULES:
1. OBJECTIVE: Produce a concise product-level objective explaining what the project is and what it accomplishes. Do NOT simply copy the README or dump installation steps. Preserve key repository terminology.
2. STACK: Only list technical details directly supported by the captured evidence (e.g. Git / Git worktrees, WebGL terminal rendering, Chromium/browser integration, SSH / remote development, CLI-based coding-agent ecosystem). Do NOT infer or claim an unverified complete stack (such as React/Rust/Postgres) unless explicit evidence is present in the capture.
3. FEATURES / ECOSYSTEM: Summarize concise product-level capabilities rather than reproducing every README bullet verbatim. If the repository supports a broader tool or agent ecosystem (such as CLI coding agents), explicitly include that ecosystem capability (e.g. "Supports a broad range of CLI coding agents").
4. USER BASE: Identify the intended user groups from stated use cases, workflows, and positioning (e.g. "Software developers", "AI-assisted developers", "Developers using coding agents"). Do NOT invent demographic personas, company sizes, or job titles.
5. LICENSING: Extract the license if explicitly stated in repository metadata, badges, or documentation (e.g. "MIT License"). If absent, return null.
6. NO ISSUES / NO TRIVIA: Do NOT extract, invent, or create sections for GitHub issues or historical bugs. Do NOT include installation instructions, package managers, community links (Discord/X), or exact URLs.

Return ONLY a valid JSON object matching this structure:
{
  "repository_name": "Name of the repository (e.g. stablyai/orca)",
  "objective": "Orca is an AI orchestration/development environment for running multiple coding agents in parallel, using isolated Git worktrees and providing tools for managing, monitoring and comparing agent work.",
  "stack": [
    "Git / Git worktrees",
    "WebGL terminal rendering",
    "Chromium/browser integration",
    "SSH / remote development",
    "CLI-based coding-agent ecosystem"
  ],
  "features": [
    "Parallel coding-agent orchestration",
    "Isolated Git worktrees",
    "Multi-agent development workflows",
    "Integrated terminal splits",
    "GitHub and Linear integration",
    "Remote SSH worktrees",
    "AI diff annotation/review",
    "File/image handoff to agents",
    "Supports a broad range of CLI coding agents"
  ],
  "user_base": [
    "Software developers",
    "AI-assisted developers",
    "Developers using coding agents"
  ],
  "licensing": "MIT License"
}
"#;

#[derive(Debug, Default, Deserialize)]
struct RawLlmRepositoryResponse {
    repository_name: Option<String>,
    objective: Option<String>,
    #[serde(default)]
    stack: Vec<String>,
    #[serde(default)]
    features: Vec<String>,
    #[serde(default)]
    user_base: Vec<String>,
    #[serde(default)]
    licensing: Option<String>,
}


fn build_repository_context_from_parsed(
    capture_id: &str,
    default_name: &str,
    parsed: RawLlmRepositoryResponse,
    generated_at: &str,
    model: Option<String>,
) -> RepositoryContext {
    let repository_name = parsed
        .repository_name
        .filter(|n| !n.trim().is_empty())
        .unwrap_or_else(|| default_name.to_string());

    let objective = parsed
        .objective
        .filter(|o| !o.trim().is_empty())
        .unwrap_or_else(|| INSUFFICIENT_REPOSITORY_OBJECTIVE_EVIDENCE.to_string());

    let stack = parsed.stack.into_iter().filter(|x| !x.trim().is_empty()).collect();
    let features = parsed.features.into_iter().filter(|x| !x.trim().is_empty()).collect();
    let user_base = parsed.user_base.into_iter().filter(|x| !x.trim().is_empty()).collect();
    let licensing = parsed.licensing.filter(|l| !l.trim().is_empty());

    RepositoryContext {
        capture_id: capture_id.to_string(),
        repository_name,
        objective,
        stack,
        features,
        user_base,
        licensing,
        generated_at: generated_at.to_string(),
        model,
        deterministic: false,
    }
}

pub fn derive_repository_name(payload: &WebCapturePayload) -> String {
    if let Some(ref title) = payload.title {
        let trimmed = title.trim();
        if trimmed.contains('/') && !trimmed.contains("://") {
            let name = trimmed.split(':').next().unwrap_or(trimmed).trim();
            return name.to_string();
        }
    }
    if let Some(parsed) = super::source::parse_url(&payload.url) {
        let parts: Vec<&str> = parsed.path.trim_matches('/').split('/').collect();
        if parts.len() >= 2 {
            return format!("{}/{}", parts[0], parts[1]);
        }
        if !parts.is_empty() && !parts[0].is_empty() {
            return parts[0].to_string();
        }
    }
    payload.title.clone().unwrap_or_else(|| "Software Repository".to_string())
}

fn derive_repository_objective(payload: &WebCapturePayload, normalized_markdown: &str) -> String {
    for block in &payload.content.blocks {
        if let super::ContentBlock::Paragraph { text } = block {
            let trimmed = text.trim();
            if !trimmed.is_empty() && trimmed.len() > 15 {
                return trimmed.chars().take(280).collect();
            }
        }
    }
    for line in normalized_markdown.lines() {
        let trimmed = line.trim();
        if !trimmed.is_empty() && !trimmed.starts_with('#') && !trimmed.starts_with('>') && trimmed.len() > 20 {
            return trimmed.chars().take(280).collect();
        }
    }
    format!("Explore and reference {}", derive_repository_name(payload))
}

fn derive_repository_stack(markdown: &str) -> Vec<String> {
    let mut stack = Vec::new();
    let lower = markdown.to_lowercase();

    if lower.contains("worktree") || lower.contains("git ") || lower.contains("git\n") {
        stack.push("Git / Git worktrees".to_string());
    }
    if lower.contains("webgl") {
        stack.push("WebGL terminal rendering".to_string());
    }
    if lower.contains("chromium") || (lower.contains("browser") && lower.contains("integration")) {
        stack.push("Chromium/browser integration".to_string());
    }
    if lower.contains("ssh") || lower.contains("remote development") {
        stack.push("SSH / remote development".to_string());
    }
    if lower.contains("terminal") && !stack.iter().any(|s| s.contains("terminal")) {
        stack.push("Integrated terminal splits".to_string());
    }
    if lower.contains("cli") || (lower.contains("agent") && lower.contains("coding")) {
        stack.push("CLI-based coding-agent ecosystem".to_string());
    }

    let explicit_techs = [
        ("rust", "Rust"),
        ("typescript", "TypeScript"),
        ("javascript", "JavaScript"),
        ("python", "Python"),
        ("golang", "Go"),
        ("sqlite", "SQLite"),
        ("docker", "Docker"),
    ];
    for (kw, name) in explicit_techs {
        if lower.contains(kw) && !stack.iter().any(|s| s == name) {
            stack.push(name.to_string());
        }
    }

    stack
}

fn derive_repository_features(markdown: &str) -> Vec<String> {
    let mut features = Vec::new();
    let lower = markdown.to_lowercase();

    if lower.contains("parallel") && (lower.contains("agent") || lower.contains("orchestration")) {
        features.push("Parallel coding-agent orchestration".to_string());
    }
    if lower.contains("worktree") {
        features.push("Isolated Git worktrees".to_string());
    }
    if lower.contains("multi-agent") || (lower.contains("multiple") && lower.contains("agent")) {
        features.push("Multi-agent development workflows".to_string());
    }
    if lower.contains("terminal") {
        features.push("Integrated terminal splits".to_string());
    }
    if lower.contains("github") && lower.contains("linear") {
        features.push("GitHub and Linear integration".to_string());
    }
    if lower.contains("ssh") || lower.contains("remote") {
        features.push("Remote SSH worktrees".to_string());
    }
    if lower.contains("diff") || lower.contains("review") {
        features.push("AI diff annotation/review".to_string());
    }
    if lower.contains("handoff") || (lower.contains("image") && lower.contains("file")) {
        features.push("File/image handoff to agents".to_string());
    }
    if lower.contains("mobile") {
        features.push("Mobile companion".to_string());
    }
    if lower.contains("design mode") {
        features.push("Design Mode".to_string());
    }
    if lower.contains("computer use") {
        features.push("Computer Use".to_string());
    }

    if lower.contains("agent") || lower.contains("claude") || lower.contains("aider") || lower.contains("cline") {
        features.push("Supports a broad range of CLI coding agents".to_string());
    }

    if features.is_empty() {
        let mut in_features_section = false;
        for line in markdown.lines() {
            let trimmed = line.trim();
            if trimmed.starts_with('#') {
                let heading_text = trimmed.trim_start_matches('#').trim().to_lowercase();
                in_features_section = heading_text.contains("feature")
                    || heading_text.contains("capabilities")
                    || heading_text.contains("what it does")
                    || heading_text.contains("highlights");
                continue;
            }

            if in_features_section && (trimmed.starts_with('-') || trimmed.starts_with('*')) {
                let item_text = trimmed.trim_start_matches(['-', '*']).trim();
                if !item_text.is_empty() && item_text.len() > 5 {
                    features.push(item_text.to_string());
                    if features.len() >= 10 {
                        break;
                    }
                }
            }
        }
    }

    features
}

fn derive_repository_user_base(markdown: &str) -> Vec<String> {
    let lower = markdown.to_lowercase();
    if lower.contains("agent") || lower.contains("developer") || lower.contains("code") {
        vec![
            "Software developers".to_string(),
            "AI-assisted developers".to_string(),
            "Developers using coding agents".to_string(),
        ]
    } else {
        vec!["Software developers".to_string()]
    }
}

fn derive_repository_licensing(markdown: &str) -> Option<String> {
    let lower = markdown.to_lowercase();
    if lower.contains("mit license") || lower.contains("license: mit") || lower.contains("licensed under the mit") {
        Some("MIT License".to_string())
    } else if lower.contains("apache 2.0") || lower.contains("apache-2.0") {
        Some("Apache-2.0 License".to_string())
    } else if lower.contains("bsd") {
        Some("BSD License".to_string())
    } else if lower.contains("gpl") {
        Some("GPL License".to_string())
    } else {
        None
    }
}

pub fn extract_deterministic_repository_context(
    capture_id: &str,
    payload: &WebCapturePayload,
    normalized_markdown: &str,
    generated_at: &str,
) -> RepositoryContext {
    let repo_name = derive_repository_name(payload);
    let objective = derive_repository_objective(payload, normalized_markdown);
    let stack = derive_repository_stack(normalized_markdown);
    let features = derive_repository_features(normalized_markdown);
    let user_base = derive_repository_user_base(normalized_markdown);
    let licensing = derive_repository_licensing(normalized_markdown);

    RepositoryContext {
        capture_id: capture_id.to_string(),
        repository_name: repo_name,
        objective,
        stack,
        features,
        user_base,
        licensing,
        generated_at: generated_at.to_string(),
        model: None,
        deterministic: true,
    }
}

/// Extracts structured repository context from a captured software repository.
#[cfg(test)]
mod tests {
    use super::*;
    use crate::capture::web::{CaptureContent, CaptureContentKind, CaptureMessage, ContentBlock, ExtractorInfo};

    fn capture_file_for(capture_type: &str, url: &str, markdown: &str) -> crate::vault::VaultFile {
        crate::vault::VaultFile::new_capture(
            "cap_dispatch".to_string(),
            "capture.json".to_string(),
            "captures/cap_dispatch".to_string(),
            markdown.to_string(),
            "hash".to_string(),
            crate::capture::web::CaptureProvenance {
                source_type: "web".to_string(),
                capture_type: capture_type.to_string(),
                application: "GitHub".to_string(),
                domain: "github.com".to_string(),
                url: url.to_string(),
                page_title: "owner/repo".to_string(),
                captured_at: "2026-09-03T10:00:00Z".to_string(),
                browser_captured_at: None,
                browser: None,
                extractor_id: "github".to_string(),
                extractor_version: 1,
                trust: "external_untrusted".to_string(),
                fidelity: "structured".to_string(),
                coverage: crate::capture::web::CaptureCoverage::FullDocument,
                notes: Vec::new(),
                message_count: None,
                block_count: 3,
                skipped_block_count: 0,
                truncated: false,
                canonical_url: None,
                author: None,
                published_at: None,
                language: None,
                version: 1,
                previous_capture_id: None,
                recapture_count: 0,
                traversal: None,
            },
        )
    }

    /// The dispatch, end to end and offline. A repository capture gets the
    /// repository schema, and the result says which prompt produced it and that
    /// no model did.
    #[tokio::test]
    async fn a_repository_capture_gets_repository_context_and_honest_metadata() {
        let file = capture_file_for(
            "repository",
            "https://github.com/owner/repo",
            "# owner/repo\n\nA local-first tool built with Rust and TypeScript.",
        );
        let payload = WebCapturePayload {
            url: "https://github.com/owner/repo".to_string(),
            title: Some("owner/repo".to_string()),
            ..Default::default()
        };

        let context = extract_source_context(None, &file, &payload).await;

        assert!(context.repository().is_some(), "a repository must not get conversation semantics");
        assert!(context.conversation().is_none());
        assert!(context.deterministic, "no model answered, so this is not a model's work");
        assert!(context.model.is_none(), "no model answered, so none may be named");

        let analysis = context.analysis.expect("analysis metadata is recorded");
        assert_eq!(analysis.prompt_id, "repository.context");
        assert_eq!(analysis.prompt_version, 1);
        assert_eq!(
            analysis.status,
            crate::pipeline::analysis::AnalysisStatus::InsufficientEvidence
        );
        assert!(analysis.failure.is_some(), "why no model answered is recorded");
    }

    /// The distinction the substring router could not make: an issue page is on
    /// github.com and is not a repository. Analysing it with the repository
    /// prompt would produce a tech stack for a bug report.
    #[tokio::test]
    async fn a_github_issue_capture_is_not_analysed_as_a_repository() {
        let file = capture_file_for(
            "issue",
            "https://github.com/owner/repo/issues/42",
            "# Crash on startup\n\nRelay exits immediately on launch.",
        );
        let payload = WebCapturePayload {
            url: "https://github.com/owner/repo/issues/42".to_string(),
            title: Some("Crash on startup".to_string()),
            ..Default::default()
        };

        let context = extract_source_context(None, &file, &payload).await;

        assert!(
            context.repository().is_none(),
            "an issue is not a repository, whatever host it lives on"
        );
        assert!(context.conversation().is_some());
        let analysis = context.analysis.expect("analysis metadata is recorded");
        assert_eq!(analysis.prompt_id, "conversation.context");
    }

    /// §45 — a missing objective reads as a missing objective, not as
    /// "Work on <title>", which was indistinguishable from an extracted one.
    #[test]
    fn an_unextractable_objective_says_so_rather_than_restating_the_title() {
        let conv = build_context_from_parsed(
            "cap_1",
            "Some Page Title",
            RawLlmContextResponse::default(),
            "2026-09-04T00:00:00Z".to_string(),
            None,
        );
        assert_eq!(conv.objective, INSUFFICIENT_OBJECTIVE_EVIDENCE);
        assert_eq!(conv.current_state, INSUFFICIENT_STATE_EVIDENCE);
        assert!(!conv.objective.contains("Some Page Title"));
    }

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
        let json = crate::pipeline::analysis::parse_json_response(raw)
            .expect("the shared parser should read json out of a markdown fence");
        let parsed: RawLlmContextResponse =
            serde_json::from_value(json).expect("should deserialize into the context contract");
        assert_eq!(parsed.objective.as_deref(), Some("Build context handoff"));
        assert_eq!(parsed.decisions.len(), 1);
        assert_eq!(parsed.decisions[0].decision, "Use SQLite for local storage");
    }

    #[test]
    fn deterministic_repository_extraction_extracts_stack_features_and_licensing() {
        let payload = WebCapturePayload {
            protocol_version: 1,
            captured_at: Some("2026-09-03T10:00:00Z".to_string()),
            url: "https://github.com/stablyai/orca".to_string(),
            title: Some("stablyai/orca: Agentic workspace for coding agents".to_string()),
            browser: Some("Chrome".to_string()),
            extractor: ExtractorInfo {
                id: "github".to_string(),
                version: 2,
                strategy: "site".to_string(),
            },
            document: Default::default(),
            content: CaptureContent {
                kind: CaptureContentKind::Repository,
                messages: vec![],
                blocks: vec![ContentBlock::Paragraph {
                    text: "Orca is an AI orchestration/development environment for running multiple coding agents in parallel, using isolated Git worktrees and providing tools for managing, monitoring and comparing agent work.".to_string(),
                }],
            },
            links: vec![],
            diagnostics: Default::default(),
        };

        let markdown = r#"# stablyai/orca
Orca is an AI orchestration/development environment for running multiple coding agents in parallel.

## Features
- Parallel coding-agent orchestration
- Isolated Git worktrees
- Supports multiple coding agents like Claude Code, Aider, and Cline

## Architecture
Built with WebGL terminal rendering, Chromium integration, and remote SSH worktrees.

## License
MIT License
"#;

        let repo = extract_deterministic_repository_context(
            "cap_repo_1",
            &payload,
            markdown,
            "2026-09-03T10:00:00Z",
        );

        assert_eq!(repo.capture_id, "cap_repo_1");
        assert_eq!(repo.repository_name, "stablyai/orca");
        assert!(repo.objective.contains("AI orchestration"));
        assert!(repo.stack.iter().any(|s| s.contains("Git worktrees")));
        assert!(repo.stack.iter().any(|s| s.contains("WebGL")));
        assert!(repo.features.iter().any(|f| f.contains("coding agents")));
        assert!(repo.user_base.contains(&"Software developers".to_string()));
        assert_eq!(repo.licensing, Some("MIT License".to_string()));
    }

    #[test]
    fn repository_llm_json_parsing_and_context_construction() {
        let raw = r#"```json
{
  "repository_name": "stablyai/orca",
  "objective": "Orca is an AI orchestration/development environment for running multiple coding agents in parallel.",
  "stack": [
    "Git / Git worktrees",
    "WebGL terminal rendering",
    "Chromium/browser integration",
    "SSH / remote development",
    "CLI-based coding-agent ecosystem"
  ],
  "features": [
    "Parallel coding-agent orchestration",
    "Isolated Git worktrees",
    "Supports a broad range of CLI coding agents"
  ],
  "user_base": [
    "Software developers",
    "AI-assisted developers",
    "Developers using coding agents"
  ],
  "licensing": "MIT License"
}
```"#;

        let json = crate::pipeline::analysis::parse_json_response(raw)
            .expect("the shared parser should read json out of a markdown fence");
        let parsed: RawLlmRepositoryResponse =
            serde_json::from_value(json).expect("should deserialize into the repository contract");
        let repo = build_repository_context_from_parsed(
            "cap_123",
            "stablyai/orca",
            parsed,
            "2026-09-04T00:00:00Z",
            Some("Anthropic".to_string()),
        );

        assert_eq!(repo.repository_name, "stablyai/orca");
        assert_eq!(repo.objective, "Orca is an AI orchestration/development environment for running multiple coding agents in parallel.");
        assert_eq!(repo.stack.len(), 5);
        assert_eq!(repo.features.len(), 3);
        assert!(repo.features.contains(&"Supports a broad range of CLI coding agents".to_string()));
        assert_eq!(repo.licensing, Some("MIT License".to_string()));
    }

    #[test]
    fn source_context_deserialization_is_backward_compatible_with_legacy_conversation() {
        let legacy_json = r#"{
  "capture_id": "legacy_cap_1",
  "title": "ChatGPT Chat",
  "objective": "Discuss Relay Architecture",
  "background": [],
  "current_state": "Completed discussion",
  "decisions": [
    {
      "id": "dec_1",
      "decision": "Use SQLite",
      "rationale": "Reliable local DB",
      "status": "CURRENT",
      "source_turn_ordinals": [1]
    }
  ],
  "requirements": [],
  "constraints": [],
  "preferences": [],
  "rejected_approaches": [],
  "open_questions": [],
  "action_items": [],
  "important_facts": [],
  "key_artifacts": [],
  "generated_at": "2026-09-01T12:00:00Z",
  "model": null,
  "deterministic": true
}"#;

        let source_ctx: SourceContext = serde_json::from_str(legacy_json)
            .expect("should deserialize legacy conversation context into SourceContext");

        assert_eq!(source_ctx.capture_id, "legacy_cap_1");
        match source_ctx.kind {
            SourceContextKind::Conversation(conv) => {
                assert_eq!(conv.objective, "Discuss Relay Architecture");
                assert_eq!(conv.decisions.len(), 1);
                assert_eq!(conv.decisions[0].decision, "Use SQLite");
            }
            SourceContextKind::Repository(_) => panic!("Expected Conversation kind"),
        }
    }

    #[test]
    fn source_context_envelope_roundtrips_repository_context() {
        let repo = RepositoryContext {
            capture_id: "cap_roundtrip".to_string(),
            repository_name: "stablyai/orca".to_string(),
            objective: "AI workspace".to_string(),
            stack: vec!["Git / Git worktrees".to_string()],
            features: vec!["Supports a broad range of CLI coding agents".to_string()],
            user_base: vec!["Software developers".to_string()],
            licensing: Some("MIT License".to_string()),
            generated_at: "2026-09-04T00:00:00Z".to_string(),
            model: None,
            deterministic: true,
        };

        let src = SourceContext {
            capture_id: "cap_roundtrip".to_string(),
            generated_at: "2026-09-04T00:00:00Z".to_string(),
            model: None,
            deterministic: true,
            analysis: None,
            kind: SourceContextKind::Repository(repo),
        };

        let serialized = serde_json::to_string(&src).expect("should serialize");
        assert!(serialized.contains(r#""kind":"repository""#));

        let deserialized: SourceContext = serde_json::from_str(&serialized).expect("should deserialize");
        assert_eq!(src, deserialized);
    }
}
