use super::{PipelineError, ProcessedPipelineResult};
use crate::providers::LLMClient;
use crate::settings::TtsSettings;
use crate::tts::TtsEngine;
use crate::vault::VaultManager;

const MAX_GROUNDING_NOTES: usize = 5;

/// Voice chat: record -> transcribe -> retrieve grounding notes from the
/// vault -> ask the LLM for an answer with sources -> optionally speak it
/// back via local TTS. This is the "voice input inside the app" flow,
/// distinct from the meeting/scribble PTT capture modes.
pub async fn process_chat(
    llm: &LLMClient,
    vault: &VaultManager,
    tts_settings: &TtsSettings,
    question: &str,
) -> Result<ProcessedPipelineResult, PipelineError> {
    let notes = vault
        .search_notes(question, MAX_GROUNDING_NOTES)
        .map_err(|e| PipelineError::VaultError(e.to_string()))?;

    let system_prompt = if notes.is_empty() {
        "You are Relay's voice assistant. The user's vault has no notes matching this question. \
         Say so honestly and answer from general knowledge only if it's safe to do so; do not \
         fabricate specifics that would need to come from their notes."
            .to_string()
    } else {
        let context = notes
            .iter()
            .map(|n| format!("### {}\n{}", n.title, n.content))
            .collect::<Vec<_>>()
            .join("\n\n");
        format!(
            "You are Relay's voice assistant. Answer the user's question using ONLY the following \
             notes from their vault as grounding context. If the notes don't contain the answer, \
             say so honestly rather than guessing.\n\n{}",
            context
        )
    };

    let response = llm.complete(question, Some(&system_prompt)).await?;
    let spoken_audio_base64 =
        TtsEngine::synthesize(tts_settings, &response.text).unwrap_or_else(|e| {
            tracing::warn!("TTS synthesis failed, falling back to text-only: {}", e);
            None
        });

    Ok(ProcessedPipelineResult {
        mode: "chat".to_string(),
        transcript: question.to_string(),
        note_id: None,
        kanban_cards_created: 0,
        output_markdown: response.text,
        sources: notes.into_iter().map(|n| n.title).collect(),
        spoken_audio_base64,
    })
}
