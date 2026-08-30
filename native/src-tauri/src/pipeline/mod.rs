use crate::providers::{LLMClient, ProviderError};
use crate::vault::{VaultManager, VaultNote};
use serde::{Deserialize, Serialize};
use thiserror::Error;

mod enrichment;
pub use enrichment::{
    enrich_scribble, extract_deterministic_entities, extract_deterministic_knowledge,
    extract_deterministic_questions, extract_deterministic_title,
    extract_deterministic_topics, summarize_scribble, AiEnrichmentResponse,
};

#[derive(Error, Debug)]
pub enum PipelineError {
    #[error("LLM Provider error: {0}")]
    ProviderError(#[from] ProviderError),

    #[error("Failed to parse JSON response: {0}")]
    JsonParseError(#[from] serde_json::Error),

    #[error("Vault operation failed: {0}")]
    VaultError(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessedPipelineResult {
    pub mode: String,
    pub transcript: String,
    pub note_id: Option<String>,
    pub kanban_cards_created: usize,
    pub output_markdown: String,
    /// Vault note titles used as grounding context.
    ///
    /// Retained for the frontend's `ProcessedPipelineResult` contract;
    /// grounded answers now come from Talkback, which carries full
    /// provenance (`talkback::ContextItem`) rather than bare titles.
    #[serde(default)]
    pub sources: Vec<String>,
    /// Base64 WAV of the answer spoken aloud, if a local TTS engine is configured.
    #[serde(default)]
    pub spoken_audio_base64: Option<String>,
}

pub struct PipelineEngine;

impl PipelineEngine {

    pub async fn process_scribble(
        llm: &LLMClient,
        vault: &VaultManager,
        transcript: &str,
    ) -> Result<ProcessedPipelineResult, PipelineError> {
        let system_prompt = r#"
You are Relay's Voice Scribble Structurer.
Transform rambling, raw voice notes into a polished Markdown document.

Include:
# Executive Summary
- Concise bullet points summarizing main ideas

## Key Decisions & Context
- Structured breakdown of thoughts

## Next Steps
- Clear, actionable follow-ups
"#;

        let response = llm.complete(transcript, Some(system_prompt)).await?;
        let now_str = chrono::Utc::now().to_rfc3339();
        let note_id = format!("note_{}", uuid::Uuid::new_v4());

        let note = VaultNote {
            id: note_id.clone(),
            title: "Voice Scribble Note".to_string(),
            note_type: "scribble".to_string(),
            created_at: now_str.clone(),
            updated_at: now_str,
            tags: vec!["scribble".to_string(), "structured".to_string()],
            source_audio: None,
            content: response.text.clone(),
        };

        vault
            .save_note(&note)
            .map_err(|e| PipelineError::VaultError(e.to_string()))?;

        Ok(ProcessedPipelineResult {
            mode: "scribble".to_string(),
            transcript: transcript.to_string(),
            note_id: Some(note_id),
            kanban_cards_created: 0,
            output_markdown: response.text,
            sources: Vec::new(),
            spoken_audio_base64: None,
        })
    }
}
