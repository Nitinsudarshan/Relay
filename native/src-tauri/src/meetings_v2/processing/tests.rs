//! End-to-end tests for the meeting processing pipeline.
//!
//! These run against real files in a temporary vault, because the properties
//! being tested are about what does and does not get written to disk. The
//! central one is raw immutability: `transcript.jsonl` is hashed before the
//! pipeline runs and compared afterwards, so any future change that starts
//! writing to it fails here rather than in a user's vault.

use super::*;
use crate::meetings_v2::processing::llm::test_support::ScriptedLlm;
use crate::meetings_v2::processing::llm::LlmError;
use crate::meetings_v2::processing::model::{
    OwnerType, ProviderOutputStatus, SummarySource, SPEAKER_ID_ME, SPEAKER_ID_REMOTE,
};
use crate::meetings_v2::session_store::SessionStore;
use crate::meetings_v2::types::{MeetingSession, MeetingState, TranscriptSegment};
use std::fs;
use std::path::PathBuf;

struct Harness {
    vault: PathBuf,
    sessions: Arc<SessionStore>,
    processor: MeetingProcessor,
    meeting_id: String,
}

impl Harness {
    /// Builds a meeting on disk from `(text, mic_had_audio, sys_had_audio)`
    /// triples, one per 30-second chunk.
    fn new(fixture: &[(&str, bool, bool)]) -> Self {
        let vault =
            std::env::temp_dir().join(format!("relay_test_pipeline_{}", uuid::Uuid::new_v4()));
        let sessions = Arc::new(SessionStore::new(vault.clone()));

        let meeting_id = "meet_fixture".to_string();
        let mut session = MeetingSession::new(meeting_id.clone(), None);
        session.state = MeetingState::Completed;
        session.started_at = Some("2026-08-27T10:00:00Z".to_string());
        sessions.init_session(&session).unwrap();

        for (idx, (text, mic, sys)) in fixture.iter().enumerate() {
            sessions
                .append_transcript_segment(
                    &meeting_id,
                    &TranscriptSegment {
                        chunk_index: idx,
                        start_time_s: idx as f64 * 30.0,
                        end_time_s: (idx + 1) as f64 * 30.0,
                        text: text.to_string(),
                        created_at: "2026-08-27T10:00:00Z".to_string(),
                        status: TranscriptSegmentStatus::Success,
                        mic_had_audio: *mic,
                        sys_had_audio: *sys,
                        utterances: Vec::new(),
            speech: None,
            rejection: None,
                    },
                )
                .unwrap();
        }

        let processor = MeetingProcessor::new(sessions.clone());
        Self {
            vault,
            sessions,
            processor,
            meeting_id,
        }
    }

    fn transcript_path(&self) -> PathBuf {
        self.sessions
            .session_dir(&self.meeting_id)
            .join("transcript.jsonl")
    }

    /// A fingerprint of the raw transcript file, byte for byte.
    fn raw_fingerprint(&self) -> String {
        let bytes = fs::read(self.transcript_path()).unwrap();
        use sha2::{Digest, Sha256};
        format!("{:x}", Sha256::digest(&bytes))
    }

    fn session_json(&self) -> String {
        fs::read_to_string(
            self.sessions
                .session_dir(&self.meeting_id)
                .join("session.json"),
        )
        .unwrap()
    }
}

impl Drop for Harness {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.vault);
    }
}

/// Fixture A — two speakers, clear decisions.
fn fixture_a() -> Vec<(&'static str, bool, bool)> {
    vec![
        (
            "so um we decided to ship the release on Friday and I will write the changelog tonight \
before the freeze because the client is expecting it",
            true,
            false,
        ),
        (
            "agreed we agreed to freeze the schema this sprint and I'll handle the migration script \
review with the platform team",
            false,
            true,
        ),
    ]
}

/// Fixture I — a long meeting spanning many chunks and several topics.
fn fixture_long() -> Vec<(&'static str, bool, bool)> {
    let mut fixture = Vec::new();
    for _ in 0..12 {
        fixture.push((
            "we walked through the audio pipeline and the whisper decoding settings and the \
tradeoffs around chunk size and latency in some detail",
            true,
            false,
        ));
        fixture.push((
            "then we looked at supabase sync and the migration strategy and what it means for the \
local vault and the knowledge layer",
            false,
            true,
        ));
    }
    fixture
}

fn facts_json() -> String {
    serde_json::json!({
        "title": "Release Cut And Schema Freeze",
        "meeting_type": "planning",
        "key_points": [
            {"text": "Timing was settled after weighing the migration risk.", "kind": "discussion", "topic": "Release Planning", "source_segment_ids": ["seg_00000"]},
            {"text": "Schema stability was prioritized for this sprint.", "kind": "discussion", "topic": "Data Migration Strategy", "source_segment_ids": ["seg_00001"]},
            {"text": "Cutting the release a day early instead.", "kind": "proposal", "topic": "Release Planning", "source_segment_ids": ["seg_00000"]}
        ],
        "topics": [
            {"label": "Release Planning", "segment_ids": ["seg_00000"]},
            {"label": "Data Migration Strategy", "segment_ids": ["seg_00001"]}
        ],
        "decisions": [
            {"statement": "Ship the release on Friday.", "rationale": "downstream teams have been waiting on it", "decided_by": "speaker_me", "source_segment_ids": ["seg_00000"]},
            {"statement": "Freeze the schema for this sprint.", "decided_by": "speaker_1", "source_segment_ids": ["seg_00001"]}
        ],
        "action_items": [
            {"description": "Write the changelog", "owner": "speaker_me", "deadline": "2026-08-28", "source_segment_ids": ["seg_00000"]},
            {"description": "Review the migration script", "owner": "speaker_1", "source_segment_ids": ["seg_00001"]}
        ],
        "open_questions": [
            {"question": "Who signs off on the migration?", "source_segment_ids": ["seg_00001"]}
        ],
        "risks": [
            {"statement": "The migration script has had no review.", "kind": "blocker", "raised_by": "speaker_1", "source_segment_ids": ["seg_00001"]}
        ],
        "entities": [
            {"name": "Relay", "kind": "product", "segment_ids": ["seg_00000"]}
        ]
    })
    .to_string()
}

/// Prose in the shape the output contract requires.
fn prose() -> String {
    "## Overview\n\nRelease timing and schema stability were settled for this sprint.\n\n\
## Discussion\n\n### Release Planning\n\n\
- Release timing was settled once the migration risk had been weighed.\n\n\
### Data Migration Strategy\n\n\
- Schema stability was prioritized over new fields for this sprint.\n\n\
## Decisions\n\n- The release ships Friday, because downstream teams have been waiting — Me\n\
- The schema is frozen for the sprint — Speaker 1\n\n\
## Action Items\n\n- [ ] Write the changelog — **Me** · Due: 2026-08-28\n\
- [ ] Review the migration script — **Speaker 1**\n\n\
## Risks & Blockers\n\n- **Blocker:** The migration script has had no review.\n\n\
## Open Questions\n\n- Who signs off on the migration?\n"
        .to_string()
}

/// The same structure, short enough to fit even Concise's budget for this
/// two-chunk fixture. The fixture transcript is 46 words, so the budget it earns
/// is genuinely small — which is the point of deriving it from the meeting.
fn short_prose() -> String {
    "## Overview\n\nRelease timing and the schema freeze were settled.\n\n\
## Decisions\n\n- The release ships Friday — Me\n- The schema is frozen — Speaker 1\n\n\
## Action Items\n\n- [ ] Write the changelog — **Me** · Due: 2026-08-28\n\
- [ ] Review the migration script — **Speaker 1**\n\n\
## Open Questions\n\n- Who signs off on the migration?\n"
        .to_string()
}

fn options() -> ProcessingOptions {
    ProcessingOptions {
        glossary: vec!["Relay".to_string(), "Supabase".to_string()],
        ..Default::default()
    }
}

// ---------------------------------------------------------------------------
// Raw / derived separation
// ---------------------------------------------------------------------------

#[tokio::test]
async fn no_stage_of_the_pipeline_modifies_the_raw_transcript() {
    let harness = Harness::new(&fixture_a());
    let before = harness.raw_fingerprint();
    let session_before = harness.session_json();

    // Normalization.
    harness
        .processor
        .prepare(&harness.meeting_id, &options())
        .unwrap();
    assert_eq!(
        harness.raw_fingerprint(),
        before,
        "normalization mutated the raw transcript"
    );

    // Summary generation.
    let llm = ScriptedLlm::new(vec![Ok(facts_json()), Ok(prose())]);
    harness
        .processor
        .generate_summary(&harness.meeting_id, &llm, &options(), false)
        .await
        .unwrap();
    assert_eq!(
        harness.raw_fingerprint(),
        before,
        "summarization mutated the raw transcript"
    );

    // Speaker rename.
    harness
        .processor
        .rename_speaker(&harness.meeting_id, SPEAKER_ID_REMOTE, Some("Pranjali"))
        .unwrap();
    assert_eq!(
        harness.raw_fingerprint(),
        before,
        "a rename mutated the raw transcript"
    );

    // Regeneration in a different mode.
    let llm = ScriptedLlm::new(vec![Ok(prose())]);
    let mut concise = options();
    concise.summary_mode = SummaryMode::Concise;
    harness
        .processor
        .generate_summary(&harness.meeting_id, &llm, &concise, false)
        .await
        .unwrap();
    assert_eq!(
        harness.raw_fingerprint(),
        before,
        "regeneration mutated the raw transcript"
    );

    // Extension change.
    let llm = ScriptedLlm::new(vec![Ok(prose())]);
    let mut extended = options();
    extended.extension_id = "executive_brief".to_string();
    harness
        .processor
        .generate_summary(&harness.meeting_id, &llm, &extended, false)
        .await
        .unwrap();

    assert_eq!(
        harness.raw_fingerprint(),
        before,
        "an extension change mutated the raw transcript"
    );
    assert_eq!(
        harness.session_json(),
        session_before,
        "the pipeline must not write to session.json either"
    );
}

#[tokio::test]
async fn derived_data_lands_in_its_own_file_beside_the_source_artifacts() {
    let harness = Harness::new(&fixture_a());
    let llm = ScriptedLlm::new(vec![Ok(facts_json()), Ok(prose())]);
    harness
        .processor
        .generate_summary(&harness.meeting_id, &llm, &options(), false)
        .await
        .unwrap();

    let dir = harness.sessions.session_dir(&harness.meeting_id);
    assert!(dir.join("processing.json").exists());
    assert!(dir.join("processing_log.jsonl").exists());
    assert!(dir.join("transcript.jsonl").exists());

    // The legacy derived fields on the source record stay untouched.
    let session = harness.sessions.get_session(&harness.meeting_id).unwrap();
    assert_eq!(session.summary, None);
    assert!(session.action_items.is_empty());
}

#[tokio::test]
async fn the_normalized_transcript_differs_from_the_raw_one_but_preserves_it_per_segment() {
    let harness = Harness::new(&fixture_a());
    let processing = harness
        .processor
        .prepare(&harness.meeting_id, &options())
        .unwrap();
    let normalized = processing.normalized.unwrap();

    let first = &normalized.segments[0];
    assert!(first.raw_text.starts_with("so um we decided"));
    assert!(
        !first.text.contains(" um "),
        "fillers are gone from the derived text"
    );
    assert!(
        first.text.ends_with('.'),
        "sentence boundaries are repaired"
    );
    assert!(
        first.raw_text != first.text,
        "normalization should have changed something here"
    );
}

// ---------------------------------------------------------------------------
// Speakers
// ---------------------------------------------------------------------------

#[tokio::test]
async fn renaming_a_speaker_updates_every_derived_view_but_not_the_ids() {
    let harness = Harness::new(&fixture_a());
    let llm = ScriptedLlm::new(vec![Ok(facts_json()), Ok(prose())]);
    let before = harness
        .processor
        .generate_summary(&harness.meeting_id, &llm, &options(), false)
        .await
        .unwrap();

    // Before: the remote speaker has no name.
    let conversation_before = conversation::render_conversation_markdown(
        before.conversation.as_ref().unwrap(),
        &before.speakers,
    );
    assert!(conversation_before.contains("**Speaker 1**"));
    let action_before = before
        .facts
        .as_ref()
        .unwrap()
        .action_items
        .iter()
        .find(|a| a.owner_speaker_id.as_deref() == Some(SPEAKER_ID_REMOTE))
        .unwrap();
    assert_eq!(
        summarize::owner_label(action_before, &before.speakers),
        "Speaker 1"
    );

    let after = harness
        .processor
        .rename_speaker(&harness.meeting_id, SPEAKER_ID_REMOTE, Some("Pranjali"))
        .unwrap();

    // The conversation view resolves the new name.
    let conversation_after = conversation::render_conversation_markdown(
        after.conversation.as_ref().unwrap(),
        &after.speakers,
    );
    assert!(conversation_after.contains("**Pranjali**"));
    assert!(!conversation_after.contains("**Speaker 1**"));

    // So does the action item's owner.
    let action_after = after
        .facts
        .as_ref()
        .unwrap()
        .action_items
        .iter()
        .find(|a| a.owner_speaker_id.as_deref() == Some(SPEAKER_ID_REMOTE))
        .unwrap();
    assert_eq!(
        summarize::owner_label(action_after, &after.speakers),
        "Pranjali"
    );

    // The id is unchanged everywhere.
    assert_eq!(
        after
            .speakers
            .iter()
            .find(|s| s.id == SPEAKER_ID_REMOTE)
            .unwrap()
            .id,
        SPEAKER_ID_REMOTE
    );
    assert_eq!(
        after.conversation.as_ref().unwrap().turns[1]
            .speaker_id
            .as_deref(),
        Some(SPEAKER_ID_REMOTE)
    );

    // Existing prose still says "Speaker 1", and says so honestly.
    assert!(after.summary.as_ref().unwrap().speaker_names_stale);
    assert!(after
        .summary
        .as_ref()
        .unwrap()
        .markdown
        .contains("Speaker 1"));
}

#[tokio::test]
async fn a_rename_survives_regeneration() {
    let harness = Harness::new(&fixture_a());
    let llm = ScriptedLlm::new(vec![Ok(facts_json()), Ok(prose())]);
    harness
        .processor
        .generate_summary(&harness.meeting_id, &llm, &options(), false)
        .await
        .unwrap();
    harness
        .processor
        .rename_speaker(&harness.meeting_id, SPEAKER_ID_REMOTE, Some("Pranjali"))
        .unwrap();

    let llm = ScriptedLlm::new(vec![Ok(prose())]);
    let regenerated = harness
        .processor
        .generate_summary(&harness.meeting_id, &llm, &options(), false)
        .await
        .unwrap();

    assert_eq!(
        regenerated
            .speakers
            .iter()
            .find(|s| s.id == SPEAKER_ID_REMOTE)
            .unwrap()
            .label(),
        "Pranjali",
        "regeneration must not discard the user's rename"
    );
}

#[tokio::test]
async fn re_preparing_an_unchanged_meeting_does_not_flag_its_summary_stale() {
    // Opening a meeting re-runs the deterministic stages. That must not put a
    // "names changed, regenerate" banner on prose that is still accurate.
    let harness = Harness::new(&fixture_a());
    let llm = ScriptedLlm::new(vec![Ok(facts_json()), Ok(prose())]);
    let generated = harness
        .processor
        .generate_summary(&harness.meeting_id, &llm, &options(), false)
        .await
        .unwrap();
    assert!(!generated.summary.as_ref().unwrap().speaker_names_stale);

    let reprepared = harness
        .processor
        .prepare(&harness.meeting_id, &options())
        .unwrap();
    assert!(
        !reprepared.summary.as_ref().unwrap().speaker_names_stale,
        "nothing changed, so the prose is not stale"
    );

    // A rename does change the labels, and now it is stale.
    let renamed = harness
        .processor
        .rename_speaker(&harness.meeting_id, SPEAKER_ID_REMOTE, Some("Pranjali"))
        .unwrap();
    assert!(renamed.summary.as_ref().unwrap().speaker_names_stale);

    // And re-preparing after the rename keeps it stale rather than clearing it.
    let after = harness
        .processor
        .prepare(&harness.meeting_id, &options())
        .unwrap();
    assert!(after.summary.as_ref().unwrap().speaker_names_stale);
}

#[tokio::test]
async fn renaming_an_unknown_speaker_is_rejected() {
    let harness = Harness::new(&fixture_a());
    harness
        .processor
        .prepare(&harness.meeting_id, &options())
        .unwrap();

    let result = harness
        .processor
        .rename_speaker(&harness.meeting_id, "speaker_99", Some("Ghost"));
    assert!(result.is_err());

    let processing = harness.processor.get(&harness.meeting_id).unwrap();
    assert!(processing.speakers.iter().all(|s| s.display_name.is_none()));
}

#[tokio::test]
async fn a_transcript_with_no_channel_data_leaves_speakers_unknown() {
    // Fixture H, and also every meeting recorded before channel flags existed.
    let harness = Harness::new(&[(
        "I will send the notes round after this call ends",
        false,
        false,
    )]);
    let processing = harness
        .processor
        .prepare(&harness.meeting_id, &options())
        .unwrap();

    assert!(processing.speakers.is_empty());
    let conversation = processing.conversation.unwrap();
    assert_eq!(conversation.unattributed_turn_count, 1);
    assert!(conversation.turns[0].speaker_id.is_none());
}

// ---------------------------------------------------------------------------
// Failure behavior
// ---------------------------------------------------------------------------

#[tokio::test]
async fn an_unavailable_model_leaves_the_meeting_fully_usable() {
    let harness = Harness::new(&fixture_a());
    let before = harness.raw_fingerprint();

    let llm = ScriptedLlm::always_unavailable();
    let processing = harness
        .processor
        .generate_summary(&harness.meeting_id, &llm, &options(), false)
        .await
        .unwrap();

    // The meeting still exists, with everything the deterministic path can give.
    assert_eq!(harness.raw_fingerprint(), before);
    assert!(processing.normalized.is_some());
    assert!(processing.conversation.is_some());
    let facts = processing.facts.as_ref().unwrap();
    assert!(facts.deterministic);
    let summary = processing.summary.as_ref().unwrap();
    assert!(summary.deterministic);
    assert!(!summary.markdown.is_empty());
    assert!(summary.markdown.contains("## Overview"));

    // And the failure is on the record.
    assert!(processing.stages.extraction.error.is_some());
    assert!(processing.stages.summary.error.is_some());
}

#[tokio::test]
async fn invalid_model_json_does_not_lose_the_meeting() {
    let harness = Harness::new(&fixture_a());
    let llm = ScriptedLlm::new(vec![
        Ok("Sorry, I can't produce that.".to_string()),
        Ok(prose()),
    ]);

    let processing = harness
        .processor
        .generate_summary(&harness.meeting_id, &llm, &options(), false)
        .await
        .unwrap();

    assert!(processing.facts.as_ref().unwrap().deterministic);
    assert!(processing
        .stages
        .extraction
        .error
        .as_deref()
        .unwrap()
        .contains("parseable"));
    // A summary is still produced from the deterministic facts.
    assert!(processing.summary.is_some());
}

#[tokio::test]
async fn prose_that_fails_validation_is_replaced_rather_than_shown() {
    let harness = Harness::new(&fixture_a());

    // Prose naming someone who was never in the meeting.
    let bad_prose = "## Overview\n\nWork was distributed among the attendees today.\n\n\
## Action Items\n\n- [ ] Send the deck — **Rajesh**\n";
    // Twice, because a rejected draft now gets one corrected attempt before the
    // deterministic renderer takes over. A model that keeps inventing the same
    // participant is what this test is about.
    let llm = ScriptedLlm::new(vec![
        Ok(facts_json()),
        Ok(bad_prose.to_string()),
        Ok(bad_prose.to_string()),
    ]);

    let processing = harness
        .processor
        .generate_summary(&harness.meeting_id, &llm, &options(), false)
        .await
        .unwrap();

    let summary = processing.summary.as_ref().unwrap();
    assert!(
        !summary.markdown.contains("Rajesh"),
        "an invented participant must not reach the user"
    );
    assert!(
        summary.deterministic,
        "the deterministic renderer took over"
    );
    // The rejection is recorded against the model's draft, not against the
    // summary the user is reading. `validation` describes only what is shown.
    assert!(summary
        .rejected_issues
        .iter()
        .any(|i| i.code == "SUMMARY_INVENTED_PARTICIPANT"));
    assert_eq!(summary.provider_output_status, ProviderOutputStatus::Rejected);
    assert!(summary.fallback_used);
    assert!(
        summary.validation.passed,
        "the fallback that replaced it is valid: {:?}",
        summary.validation.issues
    );
    assert_eq!(
        processing.stages.summary.status,
        StageStatus::Success,
        "a rejected draft plus a valid fallback is a successful summary stage"
    );
    // The facts survived — only the prose was rejected.
    assert!(!processing.facts.as_ref().unwrap().deterministic);
}

#[tokio::test]
async fn an_empty_transcript_fails_clearly_without_destroying_anything() {
    let harness = Harness::new(&[]);
    let llm = ScriptedLlm::new(vec![Ok(facts_json())]);

    let result = harness
        .processor
        .generate_summary(&harness.meeting_id, &llm, &options(), false)
        .await;

    let error = result.unwrap_err();
    assert!(error.contains("no transcribed speech"));
    assert!(error.contains("audio are unaffected"));

    // The meeting is still readable and its state is honest.
    let processing = harness.processor.get(&harness.meeting_id).unwrap();
    assert_eq!(processing.stages.normalization.status, StageStatus::Failed);
    assert!(harness.sessions.get_session(&harness.meeting_id).is_ok());
}

#[tokio::test]
async fn retrying_after_a_failure_succeeds() {
    let harness = Harness::new(&fixture_a());

    // First attempt: no model.
    let llm = ScriptedLlm::always_unavailable();
    let failed = harness
        .processor
        .generate_summary(&harness.meeting_id, &llm, &options(), false)
        .await
        .unwrap();
    assert!(failed.facts.as_ref().unwrap().deterministic);

    // Retry: the model is back. Deterministic facts are not treated as reusable,
    // so extraction runs again without needing `force`.
    let llm = ScriptedLlm::new(vec![Ok(facts_json()), Ok(prose())]);
    let retried = harness
        .processor
        .generate_summary(&harness.meeting_id, &llm, &options(), false)
        .await
        .unwrap();

    assert!(!retried.facts.as_ref().unwrap().deterministic);
    assert!(!retried.summary.as_ref().unwrap().deterministic);
    assert_eq!(retried.status, ProcessingStatus::Ready);
}

#[tokio::test]
async fn a_processing_interruption_leaves_the_earlier_stages_intact() {
    let harness = Harness::new(&fixture_a());
    harness
        .processor
        .prepare(&harness.meeting_id, &options())
        .unwrap();

    // Extraction answers; prose generation then dies.
    let llm = ScriptedLlm::new(vec![
        Ok(facts_json()),
        Err(LlmError::Unavailable("connection reset".to_string())),
    ]);
    let processing = harness
        .processor
        .generate_summary(&harness.meeting_id, &llm, &options(), false)
        .await
        .unwrap();

    assert_eq!(processing.stages.normalization.status, StageStatus::Success);
    assert_eq!(processing.stages.extraction.status, StageStatus::Success);
    assert!(
        !processing.facts.as_ref().unwrap().deterministic,
        "the facts survived"
    );
    assert!(processing.summary.as_ref().unwrap().deterministic);
}

// ---------------------------------------------------------------------------
// Regeneration and modes
// ---------------------------------------------------------------------------

#[tokio::test]
async fn each_mode_replaces_the_summary_without_touching_anything_upstream() {
    let harness = Harness::new(&fixture_a());
    let raw_before = harness.raw_fingerprint();

    let llm = ScriptedLlm::new(vec![Ok(facts_json()), Ok(prose())]);
    let standard = harness
        .processor
        .generate_summary(&harness.meeting_id, &llm, &options(), false)
        .await
        .unwrap();

    let normalized_before = standard.normalized.clone();
    let speakers_before = standard.speakers.clone();
    let action_sources: Vec<Vec<String>> = standard
        .facts
        .as_ref()
        .unwrap()
        .action_items
        .iter()
        .map(|a| a.source_segment_ids.clone())
        .collect();

    for mode in [
        SummaryMode::Concise,
        SummaryMode::Standard,
        SummaryMode::Detailed,
    ] {
        let llm = ScriptedLlm::new(vec![Ok(short_prose())]);
        let mut opts = options();
        opts.summary_mode = mode;

        let regenerated = harness
            .processor
            .generate_summary(&harness.meeting_id, &llm, &opts, false)
            .await
            .unwrap();

        assert_eq!(regenerated.summary.as_ref().unwrap().mode, mode);
        assert_eq!(
            llm.call_count(),
            1,
            "changing only the mode must not re-run extraction"
        );

        assert_eq!(harness.raw_fingerprint(), raw_before);
        assert_eq!(regenerated.normalized, normalized_before);
        assert_eq!(regenerated.speakers, speakers_before);
        assert_eq!(
            regenerated
                .facts
                .as_ref()
                .unwrap()
                .action_items
                .iter()
                .map(|a| a.source_segment_ids.clone())
                .collect::<Vec<_>>(),
            action_sources,
            "action-item provenance must survive regeneration"
        );
    }
}

#[tokio::test]
async fn regeneration_records_the_metadata_needed_to_explain_a_quality_change() {
    let harness = Harness::new(&fixture_a());
    let llm = ScriptedLlm::new(vec![Ok(facts_json()), Ok(prose())]);
    let mut opts = options();
    opts.extension_id = "project_update".to_string();

    let processing = harness
        .processor
        .generate_summary(&harness.meeting_id, &llm, &opts, false)
        .await
        .unwrap();

    let summary = processing.summary.as_ref().unwrap();
    assert_eq!(summary.mode, SummaryMode::Standard);
    assert_eq!(summary.extension_id, "project_update");
    assert_eq!(summary.processing_version, PROCESSING_VERSION);
    assert_eq!(summary.rules_version, RULES_VERSION);
    assert_eq!(summary.provider, "scripted");
    assert_eq!(summary.model, "scripted-model");
    assert!(!summary.generated_at.is_empty());
}

#[tokio::test]
async fn forcing_extraction_re_runs_stage_a() {
    let harness = Harness::new(&fixture_a());
    let llm = ScriptedLlm::new(vec![Ok(facts_json()), Ok(prose())]);
    harness
        .processor
        .generate_summary(&harness.meeting_id, &llm, &options(), false)
        .await
        .unwrap();

    let llm = ScriptedLlm::new(vec![Ok(facts_json()), Ok(prose())]);
    harness
        .processor
        .generate_summary(&harness.meeting_id, &llm, &options(), true)
        .await
        .unwrap();
    assert_eq!(llm.call_count(), 2, "force must re-run both stages");
}

// ---------------------------------------------------------------------------
// Action items, topics, entities, classification
// ---------------------------------------------------------------------------

#[tokio::test]
async fn action_items_are_structured_with_owners_and_provenance() {
    let harness = Harness::new(&fixture_a());
    let llm = ScriptedLlm::new(vec![Ok(facts_json()), Ok(prose())]);
    let processing = harness
        .processor
        .generate_summary(&harness.meeting_id, &llm, &options(), false)
        .await
        .unwrap();

    let facts = processing.facts.as_ref().unwrap();
    assert_eq!(facts.action_items.len(), 2);

    let changelog = &facts.action_items[0];
    assert_eq!(changelog.owner_type, OwnerType::Me);
    assert_eq!(changelog.owner_speaker_id.as_deref(), Some(SPEAKER_ID_ME));
    assert_eq!(changelog.deadline.as_deref(), Some("2026-08-28"));
    assert_eq!(changelog.source_segment_ids, vec!["seg_00000"]);

    let review = &facts.action_items[1];
    assert_eq!(review.owner_type, OwnerType::Speaker);
    assert_eq!(review.owner_speaker_id.as_deref(), Some(SPEAKER_ID_REMOTE));
    assert_eq!(review.deadline, None, "no date was spoken for this one");
}

#[tokio::test]
async fn checking_off_an_action_item_persists() {
    let harness = Harness::new(&fixture_a());
    let llm = ScriptedLlm::new(vec![Ok(facts_json()), Ok(prose())]);
    harness
        .processor
        .generate_summary(&harness.meeting_id, &llm, &options(), false)
        .await
        .unwrap();

    let updated = harness
        .processor
        .set_action_item_status(&harness.meeting_id, "action_0", ActionItemStatus::Done)
        .unwrap();
    assert_eq!(
        updated.facts.as_ref().unwrap().action_items[0].status,
        ActionItemStatus::Done
    );

    // Survives a reload — which is what "restart the application" means here.
    let reloaded = MeetingProcessor::new(harness.sessions.clone())
        .get(&harness.meeting_id)
        .unwrap();
    assert_eq!(
        reloaded.facts.as_ref().unwrap().action_items[0].status,
        ActionItemStatus::Done
    );

    assert!(harness
        .processor
        .set_action_item_status(&harness.meeting_id, "action_nope", ActionItemStatus::Done)
        .is_err());
}

#[tokio::test]
async fn topics_entities_and_meeting_type_are_extracted() {
    let harness = Harness::new(&fixture_a());
    let llm = ScriptedLlm::new(vec![Ok(facts_json()), Ok(prose())]);
    let processing = harness
        .processor
        .generate_summary(&harness.meeting_id, &llm, &options(), false)
        .await
        .unwrap();

    let facts = processing.facts.as_ref().unwrap();
    assert_eq!(facts.meeting_type, model::MeetingType::Planning);
    assert!(facts.topics.iter().any(|t| t.label == "Release Planning"));
    assert!(facts.entities.iter().any(|e| e.name == "Relay"));
    assert_eq!(facts.decisions.len(), 2);
    assert_eq!(facts.open_questions.len(), 1);
}

#[tokio::test]
async fn a_standup_is_classified_without_a_model() {
    // Fixture J, deterministic path.
    let harness = Harness::new(&[(
        "daily scrum yesterday I finished the parser today I start the writer and I have no \
blockers to report at the moment",
        true,
        false,
    )]);
    let llm = ScriptedLlm::always_unavailable();
    let processing = harness
        .processor
        .generate_summary(&harness.meeting_id, &llm, &options(), false)
        .await
        .unwrap();

    assert_eq!(
        processing.facts.as_ref().unwrap().meeting_type,
        model::MeetingType::Scrum
    );
}

// ---------------------------------------------------------------------------
// Settings-driven behavior
// ---------------------------------------------------------------------------

#[tokio::test]
async fn turning_off_the_conversation_transcript_skips_it_without_failing() {
    let harness = Harness::new(&fixture_a());
    let mut opts = options();
    opts.generate_conversation = false;

    let processing = harness
        .processor
        .prepare(&harness.meeting_id, &opts)
        .unwrap();
    assert!(processing.conversation.is_none());
    assert_eq!(processing.stages.conversation.status, StageStatus::Skipped);
    assert_eq!(
        processing.status,
        ProcessingStatus::Ready,
        "a skipped stage is not a failure"
    );
}

#[tokio::test]
async fn turning_off_speaker_identification_leaves_the_meeting_usable() {
    let harness = Harness::new(&fixture_a());
    let mut opts = options();
    opts.speaker_identification = SpeakerIdentificationMode::Off;

    let processing = harness
        .processor
        .prepare(&harness.meeting_id, &opts)
        .unwrap();
    assert!(processing
        .normalized
        .as_ref()
        .unwrap()
        .segments
        .iter()
        .all(|s| s.speaker_id.is_none()));
    assert_eq!(processing.stages.speakers.status, StageStatus::Skipped);
    assert!(processing.conversation.is_some());
}

#[tokio::test]
async fn the_glossary_corrects_known_terms_in_the_derived_transcript() {
    let harness = Harness::new(&[(
        "we should move relay onto supabass before the next release cycle begins in earnest",
        true,
        false,
    )]);
    let processing = harness
        .processor
        .prepare(&harness.meeting_id, &options())
        .unwrap();
    let text = &processing.normalized.as_ref().unwrap().segments[0].text;

    assert!(text.contains("Relay"), "got: {}", text);
    assert!(text.contains("Supabase"), "got: {}", text);
    // The raw line is untouched.
    let raw = &processing.normalized.as_ref().unwrap().segments[0].raw_text;
    assert!(raw.contains("supabass"));
}

// ---------------------------------------------------------------------------
// Observability
// ---------------------------------------------------------------------------

#[tokio::test]
async fn the_processing_log_answers_what_happened_without_reading_source() {
    let harness = Harness::new(&fixture_a());
    let llm = ScriptedLlm::new(vec![Ok(facts_json()), Ok(prose())]);
    harness
        .processor
        .generate_summary(&harness.meeting_id, &llm, &options(), false)
        .await
        .unwrap();

    let log = harness.processor.log(&harness.meeting_id);
    let stages: Vec<&str> = log.iter().map(|e| e.stage.as_str()).collect();
    for expected in [
        "normalization",
        "speakers",
        "conversation",
        "extraction",
        "summary",
    ] {
        assert!(
            stages.contains(&expected),
            "missing {} in {:?}",
            expected,
            stages
        );
    }

    let summary_entry = log.iter().rev().find(|e| e.stage == "summary").unwrap();
    assert_eq!(summary_entry.status, "success");
    assert_eq!(summary_entry.model.as_deref(), Some("scripted-model"));
    assert_eq!(summary_entry.provider.as_deref(), Some("scripted"));
    assert_eq!(summary_entry.validator_passed, Some(true));
    assert_eq!(summary_entry.processing_version, PROCESSING_VERSION);
    assert_eq!(summary_entry.rules_version, RULES_VERSION);
    assert!(summary_entry.duration_ms.is_some());

    // No transcript content is ever written to the log.
    let raw_log = fs::read_to_string(
        harness
            .sessions
            .session_dir(&harness.meeting_id)
            .join("processing_log.jsonl"),
    )
    .unwrap();
    assert!(!raw_log.contains("changelog"));
    assert!(!raw_log.contains("schema"));
}

#[tokio::test]
async fn a_failed_run_is_diagnosable_from_the_log_alone() {
    let harness = Harness::new(&fixture_a());
    let llm = ScriptedLlm::always_unavailable();
    harness
        .processor
        .generate_summary(&harness.meeting_id, &llm, &options(), false)
        .await
        .unwrap();

    let log = harness.processor.log(&harness.meeting_id);
    let extraction = log.iter().rev().find(|e| e.stage == "extraction").unwrap();
    assert!(
        extraction.error.is_some(),
        "the reason the model was not used is recorded"
    );
    assert!(
        extraction.provider.is_some(),
        "which provider was tried is recorded"
    );
}

// ---------------------------------------------------------------------------
// Related meetings and Scribble export
// ---------------------------------------------------------------------------

#[tokio::test]
async fn related_meetings_are_found_through_shared_metadata() {
    let vault = std::env::temp_dir().join(format!("relay_test_related_{}", uuid::Uuid::new_v4()));
    let sessions = Arc::new(SessionStore::new(vault.clone()));
    let processor = MeetingProcessor::new(sessions.clone());

    for id in ["meet_a", "meet_b"] {
        let mut session = MeetingSession::new(id.to_string(), None);
        session.state = MeetingState::Completed;
        session.started_at = Some("2026-08-27T10:00:00Z".to_string());
        sessions.init_session(&session).unwrap();
        for (idx, (text, mic, sys)) in fixture_a().iter().enumerate() {
            sessions
                .append_transcript_segment(
                    id,
                    &TranscriptSegment {
                        chunk_index: idx,
                        start_time_s: idx as f64 * 30.0,
                        end_time_s: (idx + 1) as f64 * 30.0,
                        text: text.to_string(),
                        created_at: "2026-08-27T10:00:00Z".to_string(),
                        status: TranscriptSegmentStatus::Success,
                        mic_had_audio: *mic,
                        sys_had_audio: *sys,
                        utterances: Vec::new(),
            speech: None,
            rejection: None,
                    },
                )
                .unwrap();
        }

        let llm = ScriptedLlm::new(vec![Ok(facts_json()), Ok(prose())]);
        processor
            .generate_summary(id, &llm, &options(), false)
            .await
            .unwrap();
    }

    let related = processor.related("meet_a", 5).unwrap();
    assert_eq!(related.len(), 1);
    assert_eq!(related[0].meeting_id, "meet_b");
    assert!(!related[0].signals.shared_topics.is_empty());

    // An unprocessed meeting reports honestly rather than returning nothing.
    assert!(processor.related("meet_missing", 5).is_err());

    let _ = fs::remove_dir_all(vault);
}

#[tokio::test]
async fn the_scribble_markdown_is_built_from_the_derived_artifacts() {
    let harness = Harness::new(&fixture_a());
    let llm = ScriptedLlm::new(vec![Ok(facts_json()), Ok(prose())]);
    let processing = harness
        .processor
        .generate_summary(&harness.meeting_id, &llm, &options(), false)
        .await
        .unwrap();

    let markdown = render_scribble_markdown(&processing, "ignored", true);
    assert!(markdown.starts_with("# Release Cut And Schema Freeze"));
    assert!(markdown.contains("**Meeting type:** Planning"));
    assert!(markdown.contains("**Participants:** Me, Speaker 1"));
    assert!(markdown.contains("## Action Items"));
    assert!(markdown.contains("## Conversation"));

    let without = render_scribble_markdown(&processing, "ignored", false);
    assert!(!without.contains("## Conversation"));
}

#[tokio::test]
async fn a_scribble_reference_is_recorded_on_the_meeting() {
    let harness = Harness::new(&fixture_a());
    harness
        .processor
        .prepare(&harness.meeting_id, &options())
        .unwrap();

    let updated = harness
        .processor
        .record_scribble(
            &harness.meeting_id,
            ScribbleRef {
                scribble_id: "scribble_1".to_string(),
                created_at: "2026-08-27T11:00:00Z".to_string(),
                title: "Release Cut And Schema Freeze".to_string(),
            },
        )
        .unwrap();

    assert_eq!(
        updated.scribble_ref.as_ref().unwrap().scribble_id,
        "scribble_1"
    );
}

#[tokio::test]
async fn an_unprocessed_meeting_renders_a_scribble_that_admits_it() {
    let harness = Harness::new(&fixture_a());
    let processing = harness
        .processor
        .prepare(&harness.meeting_id, &options())
        .unwrap();
    let markdown = render_scribble_markdown(&processing, "Meeting — Aug 27", false);
    assert!(markdown.contains("No summary has been generated"));
}

// ---------------------------------------------------------------------------
// Scale
// ---------------------------------------------------------------------------

#[tokio::test]
async fn a_long_meeting_processes_and_stays_summary_shaped() {
    // Fixture I — 24 chunks, twelve minutes of speech, several topics.
    let harness = Harness::new(&fixture_long());
    let llm = ScriptedLlm::always_unavailable();

    let mut opts = options();
    opts.summary_mode = SummaryMode::Concise;

    let processing = harness
        .processor
        .generate_summary(&harness.meeting_id, &llm, &opts, false)
        .await
        .unwrap();

    let normalized = processing.normalized.as_ref().unwrap();
    assert_eq!(normalized.segments.len(), 24);

    // Repeated identical chunks must not collapse into one another: dedup is
    // within a segment, not across the transcript.
    assert!(normalized.word_count() > 400);

    let summary = processing.summary.as_ref().unwrap();
    let words = summary.markdown.split_whitespace().count();
    assert!(
        words <= SummaryMode::Concise.max_words(),
        "a concise summary of a long meeting is still concise, got {} words",
        words
    );
    assert!(
        summary.validation.passed,
        "issues: {:?}",
        summary.validation.issues
    );

    // The conversation alternates between the two channels.
    let conversation = processing.conversation.as_ref().unwrap();
    assert_eq!(conversation.turns.len(), 24);
    assert_eq!(conversation.unattributed_turn_count, 0);
}

#[tokio::test]
async fn repeated_stt_fragments_are_cleaned_from_the_derived_transcript_only() {
    // Fixture C.
    let harness = Harness::new(&[(
        "we we we should ship it we should ship it we should ship it tomorrow morning at the latest",
        true,
        false,
    )]);
    let before = harness.raw_fingerprint();
    let processing = harness
        .processor
        .prepare(&harness.meeting_id, &options())
        .unwrap();

    let segment = &processing.normalized.as_ref().unwrap().segments[0];
    assert_eq!(
        segment.text.matches("should ship it").count(),
        1,
        "got: {}",
        segment.text
    );
    assert_eq!(segment.raw_text.matches("should ship it").count(), 3);
    assert_eq!(harness.raw_fingerprint(), before);
}


// ---------------------------------------------------------------------------
// Quality regression fixtures
//
// Each of these is built from a real meeting that produced bad output. They are
// end-to-end on purpose: the failures they cover were not in any one function,
// they were in how the stages handed work to each other.
// ---------------------------------------------------------------------------

/// Fixture A — a demo-heavy meeting.
///
/// Every line has an action verb and a first-person future, and not one of them
/// is work that outlives the call. This transcript is what produced forty-nine
/// "action items".
fn fixture_demo_heavy() -> Vec<(&'static str, bool, bool)> {
    vec![
        ("okay so I'll project my screen now and I will show you the list of pointers that we have for today", true, false),
        ("let me just check the ID for this one and I'll pull up the dashboard so we can look at it", true, false),
        ("I'll move it to approved on this screen and then I will move it to processing so you can see the flow", true, false),
        ("I'll click here and now I will change the role to member for this user account", true, false),
        ("I'll just be back in a minute, let me grab some water before we continue", true, false),
        ("yes I'll quickly check with Ayush to join him on this call in a moment", false, true),
        ("I'll stop sharing now and let me take you through the next section of the deck", true, false),
        ("but still we'll be maintaining that log and some of the things I will jump in wherever needed", false, true),
        ("I'll upload a ticket here so let me show you on the dashboard how that actually looks", true, false),
        ("let me show you the other tab and I'll switch to the reports view for a second", true, false),
        ("I'll share my screen again because I will need to show you the filter that we added", true, false),
        ("we're taking notes in the meeting right now so we will update her afterwards about it", false, true),
    ]
}

/// Fixture B — a genuine requirements meeting.
///
/// Real commitments, a real assignment-and-acceptance, a spoken deadline, and a
/// decision that is *not* a task.
fn fixture_requirements() -> Vec<(&'static str, bool, bool)> {
    vec![
        ("so the main thing pending from our side is the trigger list, I'll send the list of mails that need to go out tomorrow", true, false),
        ("can you review the employee guide and the FAQs for any discrepancies before we publish them", false, true),
        ("sure, I'll go through the employee guide and send the corrections across", false, true),
        ("on the mail service, let's use Gmail SMTP as the fallback since delivery is around eighty five percent", true, false),
        ("agreed, cancellation will stay PNC only, that is the decision for now", false, true),
        ("I'll circulate the MoM after this and I'll reshare the query tracker link as well", true, false),
    ]
}

/// Fixture C — a degraded ASR transcript: decoder loops, collided fragments,
/// bracketed tags, and a mangled name.
fn fixture_noisy_asr() -> Vec<(&'static str, bool, bool)> {
    vec![
        ("[BLANK_AUDIO]", true, false),
        ("I will pay the firm to fill the form. I will pay the firm to fill the form. I will pay the firm to fill the form. I will pay the firm to fill the form.", true, false),
        ("there are few features that we will the specialty IUC has also joined in", false, true),
        ("(speaking in foreign language) um uh so so so the the the thing is", true, false),
        ("we will we will we will send a form to them at some point", false, true),
        ("[NON-ENGLISH SPEECH]", false, true),
    ]
}

/// Fixture D — an agreed piece of work that nobody took.
fn fixture_ambiguous_owner() -> Vec<(&'static str, bool, bool)> {
    // Both channels live for the whole chunk, so the capture data says nothing
    // about who spoke — and the commitment itself is real.
    vec![
        // Mic-only, so the local user exists in the roster and could be named.
        (
            "so the last thing on the list is the employee document, it has been sitting there \
for a while now and nobody on either side has actually picked it up yet",
            true,
            false,
        ),
        // Both channels live, so nothing in the capture data says who committed.
        (
            "right we'll update the employee document and get it circulated after this call, \
somebody still needs to own that piece",
            true,
            true,
        ),
    ]
}

fn options_with_mode(mode: SummaryMode) -> ProcessingOptions {
    ProcessingOptions {
        summary_mode: mode,
        ..options()
    }
}

#[tokio::test]
async fn fixture_a_a_demo_heavy_meeting_produces_almost_no_action_items() {
    let harness = Harness::new(&fixture_demo_heavy());
    // No model: the cue-based extractor runs, which is the harsher test — it
    // proposes a candidate for every "I'll".
    let llm = ScriptedLlm::always_unavailable();

    let processing = harness
        .processor
        .generate_summary(&harness.meeting_id, &llm, &options(), false)
        .await
        .unwrap();

    let items = &processing.facts.as_ref().unwrap().action_items;
    assert!(
        items.len() <= 1,
        "demo narration is not post-meeting work, got {}: {:?}",
        items.len(),
        items.iter().map(|i| &i.description).collect::<Vec<_>>()
    );

    // The candidates were found and then rejected, not simply never noticed.
    let diagnostics = processing
        .stages
        .extraction
        .action_diagnostics
        .expect("the extraction stage records what the gate did");
    assert!(diagnostics.candidates >= 8, "counts: {:?}", diagnostics);
    assert!(diagnostics.rejected >= diagnostics.candidates - 1);
}

#[tokio::test]
async fn fixture_b_a_requirements_meeting_produces_the_right_small_set() {
    let harness = Harness::new(&fixture_requirements());
    let llm = ScriptedLlm::always_unavailable();

    let processing = harness
        .processor
        .generate_summary(&harness.meeting_id, &llm, &options(), false)
        .await
        .unwrap();

    let facts = processing.facts.as_ref().unwrap();
    let descriptions: Vec<String> = facts
        .action_items
        .iter()
        .map(|i| i.description.to_lowercase())
        .collect();

    assert!(
        (1..=6).contains(&facts.action_items.len()),
        "an ordinary meeting yields a handful of tasks, got {}: {:?}",
        facts.action_items.len(),
        descriptions
    );
    assert!(
        descriptions.iter().any(|d| d.contains("list of mails")),
        "the trigger-list commitment is the clearest task in the meeting: {:?}",
        descriptions
    );

    // The decision is recorded as a decision, and does not silently become a
    // task of its own.
    assert!(
        !descriptions
            .iter()
            .any(|d| d.contains("cancellation will stay")),
        "a decision is not automatically an action item: {:?}",
        descriptions
    );

    // Every item is traceable and none carries an invented date.
    for item in &facts.action_items {
        assert!(
            !item.source_segment_ids.is_empty(),
            "{:?} has no provenance",
            item.description
        );
        assert!(
            item.deadline.is_none() || item.deadline.as_deref() == Some("2026-08-28"),
            "only a spoken date may become a deadline: {:?}",
            item.deadline
        );
    }
}

#[tokio::test]
async fn fixture_b_restated_commitments_collapse_into_one_task() {
    let mut fixture = fixture_requirements();
    // The same commitment, restated twice more — once mid-meeting and once in
    // the closing recap, which is how real meetings say things.
    fixture.push((
        "just to confirm, I'll send you the required email list tomorrow morning",
        true,
        false,
    ));
    fixture.push((
        "right, so to recap, I'll share the mail list and Pranjal reviews the guide",
        true,
        false,
    ));

    let harness = Harness::new(&fixture);
    let llm = ScriptedLlm::always_unavailable();
    let processing = harness
        .processor
        .generate_summary(&harness.meeting_id, &llm, &options(), false)
        .await
        .unwrap();

    let facts = processing.facts.as_ref().unwrap();
    let mail_items: Vec<&str> = facts
        .action_items
        .iter()
        .filter(|i| {
            let d = i.description.to_lowercase();
            d.contains("mail") || d.contains("email")
        })
        .map(|i| i.description.as_str())
        .collect();
    assert_eq!(
        mail_items.len(),
        1,
        "three restatements of one commitment are one task: {:?}",
        mail_items
    );
    assert!(processing
        .stages
        .extraction
        .action_diagnostics
        .is_some_and(|d| d.deduplicated > 0));
}

#[tokio::test]
async fn fixture_c_a_noisy_transcript_invents_nothing() {
    let harness = Harness::new(&fixture_noisy_asr());
    let before = harness.raw_fingerprint();
    let llm = ScriptedLlm::always_unavailable();

    let processing = harness
        .processor
        .generate_summary(&harness.meeting_id, &llm, &options(), false)
        .await
        .unwrap();

    let normalized = processing.normalized.as_ref().unwrap();
    let derived = normalized.plain_text();
    assert!(!derived.contains("[BLANK_AUDIO]"));
    assert!(!derived.contains("NON-ENGLISH"));
    assert!(!derived.contains("speaking in foreign language"));

    let facts = processing.facts.as_ref().unwrap();
    for item in &facts.action_items {
        let lower = item.description.to_lowercase();
        assert!(
            !lower.contains("pay the firm"),
            "a decoder loop is never a task: {:?}",
            item.description
        );
        assert!(
            !lower.contains("specialty"),
            "a collided fragment is discarded, never repaired: {:?}",
            item.description
        );
    }
    assert_eq!(
        harness.raw_fingerprint(),
        before,
        "the raw transcript is untouched by any of this"
    );
}

#[tokio::test]
async fn fixture_d_an_unowned_task_is_never_attributed_to_a_guess() {
    let harness = Harness::new(&fixture_ambiguous_owner());
    let llm = ScriptedLlm::always_unavailable();

    let processing = harness
        .processor
        .generate_summary(&harness.meeting_id, &llm, &options(), false)
        .await
        .unwrap();

    for item in &processing.facts.as_ref().unwrap().action_items {
        assert!(
            matches!(item.owner_type, OwnerType::Unassigned | OwnerType::Group),
            "both channels were live, so nobody may be named: {:?} owns {:?}",
            item.owner_type,
            item.description
        );
        assert!(item.owner_speaker_id.is_none());
    }
}

#[tokio::test]
async fn fixture_d_a_model_owner_the_channel_cannot_support_is_demoted() {
    let harness = Harness::new(&fixture_ambiguous_owner());
    let draft = serde_json::json!({
        "title": "Employee Document Update",
        "meeting_type": "general",
        "action_items": [{
            "description": "Update and circulate the employee document",
            "owner": "speaker_me",
            "source_segment_ids": ["seg_00001"]
        }]
    })
    .to_string();
    let llm = ScriptedLlm::new(vec![Ok(draft), Ok(prose())]);

    let processing = harness
        .processor
        .generate_summary(&harness.meeting_id, &llm, &options(), false)
        .await
        .unwrap();

    let item = &processing.facts.as_ref().unwrap().action_items[0];
    assert_eq!(
        item.owner_type,
        OwnerType::Unassigned,
        "a model may not name an owner the capture channel cannot support"
    );
    assert!(processing
        .stages
        .extraction
        .action_diagnostics
        .is_some_and(|d| d.owners_downgraded == 1));
}

#[tokio::test]
async fn fixture_e_a_long_meeting_stays_bounded_in_every_mode() {
    let mut fixture = fixture_long();
    // Salt a long meeting with a dozen genuine, distinct commitments, so the
    // cap has something real to bite on.
    let objects = [
        "migration plan",
        "rollback script",
        "release notes",
        "cancellation logic",
        "city dropdown",
        "analytics filter",
        "mail service",
        "employee guide",
        "query tracker",
        "slack channel",
        "email templates",
        "ticket workflow",
        "billing report",
        "vendor contract",
        "status dashboard",
        "onboarding checklist",
        "audit trail",
        "support rota",
    ];
    let owned: Vec<String> = objects
        .iter()
        .map(|object| format!("I'll update the {} once we are done here", object))
        .collect();
    for line in &owned {
        fixture.push((Box::leak(line.clone().into_boxed_str()), true, false));
    }

    let harness = Harness::new(&fixture);

    for mode in [
        SummaryMode::Concise,
        SummaryMode::Standard,
        SummaryMode::Detailed,
    ] {
        let llm = ScriptedLlm::always_unavailable();
        let processing = harness
            .processor
            .generate_summary(&harness.meeting_id, &llm, &options_with_mode(mode), true)
            .await
            .unwrap();

        let summary = processing.summary.as_ref().unwrap();
        let words = summary.markdown.split_whitespace().count();
        assert!(
            words <= mode.max_words(),
            "{} summary ran to {} words",
            mode.label(),
            words
        );

        let items = &processing.facts.as_ref().unwrap().action_items;
        assert!(
            items.len() <= qualify::MAX_ACTION_ITEMS,
            "{} mode produced {} action items",
            mode.label(),
            items.len()
        );

        let mut seen = std::collections::HashSet::new();
        for item in items {
            assert!(
                seen.insert(item.description.to_lowercase()),
                "duplicate action item: {:?}",
                item.description
            );
        }

        // A summary, not a transcript: the prose is a fraction of the source.
        let transcript_words = processing.normalized.as_ref().unwrap().word_count();
        assert!(
            words * 2 < transcript_words,
            "{} summary is {} words against a {}-word transcript",
            mode.label(),
            words,
            transcript_words
        );
    }
}

#[tokio::test]
async fn fixture_f_a_rejected_model_summary_still_shows_a_summary() {
    let harness = Harness::new(&fixture_a());

    // Prose that both copies the transcript verbatim and runs far over the cap —
    // the exact pair of codes a real meeting produced.
    let mut bad = String::from("## Overview\n\n- so um we decided to ship the release on Friday \
and I will write the changelog tonight before the freeze because the client is expecting it\n");
    for i in 0..200 {
        bad.push_str(&format!(
            "- padding line {} carrying enough words to run the summary well past its cap\n",
            i
        ));
    }
    let llm = ScriptedLlm::new(vec![Ok(facts_json()), Ok(bad)]);

    let processing = harness
        .processor
        .generate_summary(&harness.meeting_id, &llm, &options(), false)
        .await
        .unwrap();

    let summary = processing.summary.as_ref().unwrap();

    // The model's draft was rejected, and says why.
    assert_eq!(
        summary.provider_output_status,
        ProviderOutputStatus::Rejected
    );
    let rejected: Vec<&str> = summary
        .rejected_issues
        .iter()
        .map(|i| i.code.as_str())
        .collect();
    assert!(rejected.contains(&"SUMMARY_TOO_LONG"), "{:?}", rejected);
    assert!(
        rejected.contains(&"SUMMARY_COPIES_TRANSCRIPT"),
        "{:?}",
        rejected
    );

    // The fallback rendered, and it is what the user sees.
    assert!(summary.fallback_used);
    assert!(!summary.markdown.contains("padding line"));
    assert!(summary.markdown.contains("## Overview"));

    // And the stage is a success, because a summary exists.
    assert!(
        summary.validation.passed,
        "the fallback is valid: {:?}",
        summary.validation.issues
    );
    assert_eq!(processing.stages.summary.status, StageStatus::Success);
    assert_eq!(processing.status, ProcessingStatus::Ready);

    // Provenance is honest: a model understood the meeting, but no model wrote
    // this text.
    assert_eq!(summary.source, SummarySource::DeterministicPresentation);
    assert!(!summary.markdown.to_lowercase().contains("ai summary"));
}

#[tokio::test]
async fn a_summary_only_fails_when_the_fallback_itself_fails() {
    // No transcribed speech at all: there is nothing for either path to render,
    // and the meeting must say so rather than pretend.
    let harness = Harness::new(&[("[BLANK_AUDIO]", true, false)]);
    let llm = ScriptedLlm::always_unavailable();

    let result = harness
        .processor
        .generate_summary(&harness.meeting_id, &llm, &options(), false)
        .await;

    assert!(result.is_err(), "an empty meeting cannot be summarized");
    let processing = harness.processor.get(&harness.meeting_id).unwrap();
    assert_eq!(processing.stages.normalization.status, StageStatus::Failed);
    assert!(processing.summary.is_none());
}

#[tokio::test]
async fn the_processing_log_explains_a_fallback_without_quoting_the_meeting() {
    let harness = Harness::new(&fixture_demo_heavy());
    let llm = ScriptedLlm::always_unavailable();
    harness
        .processor
        .generate_summary(&harness.meeting_id, &llm, &options(), false)
        .await
        .unwrap();

    let log = harness.processor.log(&harness.meeting_id);
    let extraction = log.iter().rev().find(|e| e.stage == "extraction").unwrap();
    let counts = extraction
        .action_diagnostics
        .expect("the log carries the gate's counts");
    assert!(counts.candidates > 0);
    assert!(counts.rejected > 0);

    let summary = log.iter().rev().find(|e| e.stage == "summary").unwrap();
    assert_eq!(summary.fallback_used, Some(true));
    assert_eq!(summary.provider_output_status.as_deref(), Some("unavailable"));

    // The privacy guarantee: the log explains the run without reproducing it.
    let serialized = serde_json::to_string(&log).unwrap();
    for phrase in ["project my screen", "grab some water", "Ayush"] {
        assert!(
            !serialized.contains(phrase),
            "the processing log must never carry transcript text: {}",
            phrase
        );
    }
}

// ---------------------------------------------------------------------------
// One whole meeting, end to end
// ---------------------------------------------------------------------------

/// A full UAT review, the way one actually sounds: joining noise, a demo
/// stretch, a requirements discussion, a decision, and a closing recap.
///
/// This is the shape of transcript that produced forty-nine action items.
fn fixture_real_meeting() -> Vec<(&'static str, bool, bool)> {
    vec![
        ("hi everyone good morning, sorry I was a bit late joining, can you hear me okay now", true, false),
        ("yes we can hear you, let me just check if Pranjal is able to join us as well", false, true),
        ("okay so I'll project my screen now and I will take you through the pointers we have", true, false),
        ("I'll click here and now I will move it to approved so you can see what happens on this screen", true, false),
        ("let me show you the reports tab, I'll switch over and pull up the dashboard for you", true, false),
        ("I'll just be back in a minute, give me a second to get some water", true, false),
        ("so the main pending item from our side is the trigger list, I'll send the list of mails that need to go out tomorrow", true, false),
        ("can you review the employee guide and the FAQs and flag anything that looks inconsistent", false, true),
        ("sure, I'll go through the employee guide and send the corrections back to you", false, true),
        ("on the city field, can we have a dropdown of cities instead of people typing it in freely", false, true),
        ("that can be done, we can add a dropdown with an other option for anything unlisted", true, false),
        ("great, that works for us, team are we aligned on that one", false, true),
        ("for the mail service, let's go with Gmail SMTP as the fallback, delivery is around eighty five percent", true, false),
        ("agreed, and cancellation will stay PNC only, that is the decision for now", false, true),
        ("let me give it a day to think about the cancellation coordination and I'll let you know tomorrow", false, true),
        ("we could maybe look at a self service option in version two if there is appetite", false, true),
        ("I already bumped the staging config this morning so that part is done", true, false),
        ("we should probably set up a dedicated Slack channel for travel queries at launch", false, true),
        ("yes that works, let's have it ready alongside the dashboard, we are aligned on that", true, false),
        ("some of the things I will jump in wherever needed, but still we'll be maintaining that log", true, false),
        ("I'll stop sharing now, and just to recap, I'll send the required email list and circulate the MoM", true, false),
        ("and I'll reshare the query tracker link along with the MOM after this call", true, false),
        ("perfect, thanks everyone, talk tomorrow, have a good day", false, true),
    ]
}

#[tokio::test]
async fn a_real_meeting_runs_end_to_end_and_stays_trustworthy() {
    let harness = Harness::new(&fixture_real_meeting());
    let before = harness.raw_fingerprint();
    let raw_segments = harness
        .sessions
        .get_transcript_segments(&harness.meeting_id)
        .unwrap()
        .len();

    // No model, which is the pessimistic case: the cue-based extractor proposes
    // a candidate for every "I'll" in the transcript, and the gate is all that
    // stands between those and the user's task list.
    let llm = ScriptedLlm::always_unavailable();
    let processing = harness
        .processor
        .generate_summary(&harness.meeting_id, &llm, &options(), false)
        .await
        .unwrap();

    let normalized = processing.normalized.as_ref().unwrap();
    let facts = processing.facts.as_ref().unwrap();
    let summary = processing.summary.as_ref().unwrap();
    let counts = processing.stages.extraction.action_diagnostics.unwrap();

    eprintln!("\n===== end-to-end run =====");
    eprintln!("raw segments:        {}", raw_segments);
    eprintln!("normalized segments: {}", normalized.segments.len());
    eprintln!(
        "facts:               {} key points, {} topics, {} decisions, {} open questions, {} entities",
        facts.key_points.len(),
        facts.topics.len(),
        facts.decisions.len(),
        facts.open_questions.len(),
        facts.entities.len()
    );
    eprintln!("action candidates:   {}", counts.candidates);
    eprintln!("actions rejected:    {}", counts.rejected);
    eprintln!("actions deduplicated:{}", counts.deduplicated);
    eprintln!("actions capped:      {}", counts.capped);
    eprintln!("actions retained:    {}", counts.retained);
    eprintln!("  unassigned:        {}", counts.unassigned);
    eprintln!("  with deadlines:    {}", counts.with_deadlines);
    eprintln!("summary mode:        {}", summary.mode.label());
    eprintln!(
        "summary word count:  {} (cap {})",
        summary.markdown.split_whitespace().count(),
        summary.mode.max_words()
    );
    eprintln!("fallback used:       {}", summary.fallback_used);
    eprintln!("provider output:     {}", summary.provider_output_status.label());
    eprintln!("final validation:    passed={}", summary.validation.passed);
    for item in &facts.action_items {
        eprintln!("  [ ] {} — {:?}", item.description, item.owner_type);
    }
    eprintln!("==========================\n");

    // What a person opening this meeting is entitled to assume.
    assert!(
        (1..=6).contains(&facts.action_items.len()),
        "a normal meeting yields a handful of tasks, got {}",
        facts.action_items.len()
    );
    assert!(facts.action_items.len() <= qualify::MAX_ACTION_ITEMS);

    let descriptions: Vec<String> = facts
        .action_items
        .iter()
        .map(|i| i.description.to_lowercase())
        .collect();
    for banned in [
        "project my screen",
        "back in a minute",
        "stop sharing",
        "click here",
        "move it to approved",
        "switch over",
        "jump in wherever",
        "maintaining that log",
        "version two",
        "already bumped",
    ] {
        assert!(
            !descriptions.iter().any(|d| d.contains(banned)),
            "{:?} reached the task list: {:?}",
            banned,
            descriptions
        );
    }

    // Provenance holds for everything derived.
    for item in &facts.action_items {
        assert!(!item.source_segment_ids.is_empty());
    }
    for decision in &facts.decisions {
        assert!(!decision.source_segment_ids.is_empty());
    }
    for point in &facts.key_points {
        assert!(!point.source_segment_ids.is_empty());
    }

    // The summary is a summary.
    assert!(summary.validation.passed);
    assert_eq!(processing.stages.summary.status, StageStatus::Success);
    assert!(summary.markdown.split_whitespace().count() <= summary.mode.max_words());
    assert_eq!(summary.source, SummarySource::DeterministicExtraction);

    // And none of it touched the recording.
    assert_eq!(harness.raw_fingerprint(), before);
    assert_eq!(
        harness
            .sessions
            .get_transcript_segments(&harness.meeting_id)
            .unwrap()
            .len(),
        raw_segments
    );
}

#[tokio::test]
async fn the_gate_keeps_the_hard_patterns_a_model_gets_right() {
    // The gate's job is to remove what a model over-extracts, not to undo what
    // it correctly understood. These are the patterns the cue-based path cannot
    // reach — capability-plus-group-acceptance, a deferred decision, and an
    // agreed task nobody took — and all of them must survive it.
    let harness = Harness::new(&fixture_real_meeting());

    let draft = serde_json::json!({
        "title": "Travel Dashboard UAT Review",
        "meeting_type": "client_meeting",
        "key_points": [
            {"text": "The trigger list is the last blocker on the mail templates.", "source_segment_ids": ["seg_00006"]},
            {"text": "Free-text city entry was producing inconsistent data.", "source_segment_ids": ["seg_00009"]}
        ],
        "topics": [{"label": "Mail Service", "segment_ids": ["seg_00012"]}],
        "decisions": [
            {"statement": "Cancellation stays PNC-only.", "source_segment_ids": ["seg_00013"]},
            {"statement": "Gmail SMTP is the mail fallback.", "source_segment_ids": ["seg_00012"]}
        ],
        "action_items": [
            // §4.1 — direct undertaking.
            {"description": "Send PNC the list of required system emails",
             "owner": "speaker_me", "candidate_type": "action",
             "source_segment_ids": ["seg_00006"]},
            // §4.3 — capability answer plus group acceptance, across turns.
            {"description": "Add a city dropdown with a free-text fallback",
             "owner": "speaker_me", "candidate_type": "action",
             "source_segment_ids": ["seg_00009", "seg_00010", "seg_00011"]},
            // §4.4 — a deferred decision, with a spoken date.
            {"description": "Decide whether cancellations stay PNC-only",
             "owner": "speaker_1", "deadline": "2026-08-28", "candidate_type": "action",
             "source_segment_ids": ["seg_00014"]},
            // Agreed, but nobody took it.
            // The proposal and the group's acceptance of it, both cited.
            {"description": "Set up a dedicated Slack channel for travel queries",
             "owner": "unassigned", "candidate_type": "action",
             "source_segment_ids": ["seg_00017", "seg_00018"]},
            // The model over-extracting, which is what the gate is for.
            {"description": "Project the screen and walk through the pointers",
             "owner": "speaker_me", "candidate_type": "action",
             "source_segment_ids": ["seg_00002"]},
            {"description": "Look at a self service option in version two",
             "owner": "unassigned", "candidate_type": "action",
             "source_segment_ids": ["seg_00015"]},
            {"description": "Bump the staging config",
             "owner": "speaker_me", "candidate_type": "action",
             "source_segment_ids": ["seg_00016"]}
        ],
        "open_questions": [],
        "entities": [{"name": "PNC", "kind": "organization", "segment_ids": ["seg_00013"]}]
    })
    .to_string();

    let llm = ScriptedLlm::new(vec![Ok(draft), Ok(prose())]);
    let processing = harness
        .processor
        .generate_summary(&harness.meeting_id, &llm, &options(), false)
        .await
        .unwrap();

    let facts = processing.facts.as_ref().unwrap();
    let kept: Vec<&str> = facts
        .action_items
        .iter()
        .map(|i| i.description.as_str())
        .collect();

    for expected in [
        "Send PNC the list of required system emails",
        "Add a city dropdown with a free-text fallback",
        "Decide whether cancellations stay PNC-only",
        "Set up a dedicated Slack channel for travel queries",
    ] {
        assert!(
            kept.contains(&expected),
            "the gate removed a correctly extracted item: {:?} — kept {:?}",
            expected,
            kept
        );
    }
    for rejected in [
        "Project the screen and walk through the pointers",
        "Look at a self service option in version two",
        "Bump the staging config",
    ] {
        assert!(
            !kept.contains(&rejected),
            "{:?} should not have survived: {:?}",
            rejected,
            kept
        );
    }

    // The unowned item stays unowned rather than being attached to whoever
    // happened to be speaking.
    let slack = facts
        .action_items
        .iter()
        .find(|i| i.description.contains("Slack"))
        .unwrap();
    assert_eq!(slack.owner_type, OwnerType::Unassigned);

    // The deferred decision keeps the date that was actually spoken.
    let decision = facts
        .action_items
        .iter()
        .find(|i| i.description.starts_with("Decide"))
        .unwrap();
    assert_eq!(decision.deadline.as_deref(), Some("2026-08-28"));

    // The two decisions stay decisions and are not duplicated as tasks.
    assert_eq!(facts.decisions.len(), 2);
    assert!(!kept.iter().any(|d| d.contains("Gmail SMTP is")));
}

// ---------------------------------------------------------------------------
// The summary quality evaluation set
//
// These run every case in `processing::eval` through the real pipeline and
// measure the result. They are the answer to "is the output actually any
// better?", which no amount of behaviour testing answers on its own.
// ---------------------------------------------------------------------------

/// Runs one evaluation case end to end and returns the summary the user would
/// see, along with the derived data behind it.
///
/// `extraction` is the Stage A answer to replay. `None` runs with no model at
/// all, which measures the deterministic floor: the summary a user gets when
/// Ollama is not running.
async fn run_eval_case(
    case: &crate::meetings_v2::processing::eval::EvalCase,
    extraction: Option<&str>,
) -> MeetingProcessing {
    let harness = Harness::new(&case.transcript);
    if !case.notes.is_empty() {
        harness
            .sessions
            .save_notes(&harness.meeting_id, &case.notes)
            .unwrap();
    }

    // Stage A replays the fixture; Stage B has no model, so the deterministic
    // renderer presents the facts. That isolates the measurement to what the
    // pipeline does with a plausible model answer, rather than to prose a test
    // author wrote to pass its own assertions.
    let llm = match extraction {
        Some(json) => ScriptedLlm::new(vec![Ok(json.to_string())]),
        None => ScriptedLlm::always_unavailable(),
    };

    harness
        .processor
        .generate_summary(&harness.meeting_id, &llm, &options(), false)
        .await
        .unwrap()
}

#[tokio::test]
async fn the_deterministic_floor_never_invents_anything() {
    // With no model reachable at all, every case must still come out clean.
    // A summary that hallucinates when the provider is down is worse than no
    // summary, because nothing on screen says the provider was down.
    use crate::meetings_v2::processing::eval;

    for case in eval::cases() {
        let processing = run_eval_case(&case, None).await;
        let summary = processing.summary.as_ref().unwrap();
        let card = eval::score(case.name, &summary.markdown, &case.expected);

        assert!(
            card.hallucinations.is_empty(),
            "the deterministic floor invented something on {}: {}",
            case.name,
            card.report()
        );
        assert!(
            summary.source.is_deterministic(),
            "{} should have taken the deterministic path",
            case.name
        );
    }
}

#[tokio::test]
async fn every_case_survives_the_model_path_without_a_hallucination() {
    use crate::meetings_v2::processing::eval;

    for case in eval::cases() {
        let processing = run_eval_case(&case, Some(case.model_extraction)).await;
        let summary = processing.summary.as_ref().unwrap();
        let card = eval::score(case.name, &summary.markdown, &case.expected);

        assert!(
            card.hallucinations.is_empty(),
            "{} — {}\n{}",
            case.name,
            case.premise,
            card.report()
        );
        assert_eq!(
            card.owner_accuracy, 1.0,
            "{} reported an owner the meeting did not establish\n{}",
            case.name,
            card.report()
        );
        assert_eq!(
            card.deadline_accuracy, 1.0,
            "{} reported a date the meeting did not give\n{}",
            case.name,
            card.report()
        );
    }
}

#[tokio::test]
async fn understanding_the_meeting_scores_better_than_reading_it_for_cues() {
    // The measurement that says the two-stage pipeline earns its second model
    // call: the same meetings, scored with and without comprehension.
    use crate::meetings_v2::processing::eval;

    let mut with_model = 0.0;
    let mut without_model = 0.0;
    let mut lines = Vec::new();

    for case in eval::cases() {
        let floor = run_eval_case(&case, None).await;
        let understood = run_eval_case(&case, Some(case.model_extraction)).await;

        let floor_card = eval::score(
            case.name,
            &floor.summary.as_ref().unwrap().markdown,
            &case.expected,
        );
        let understood_card = eval::score(
            case.name,
            &understood.summary.as_ref().unwrap().markdown,
            &case.expected,
        );

        lines.push(format!(
            "  {:<40} floor {:.2} → understood {:.2}",
            case.name,
            floor_card.overall(),
            understood_card.overall()
        ));
        without_model += floor_card.overall();
        with_model += understood_card.overall();

        assert!(
            understood_card.overall() >= floor_card.overall(),
            "comprehension made {} worse:\n  floor:      {}\n  understood: {}",
            case.name,
            floor_card.report(),
            understood_card.report()
        );
    }

    let cases = eval::cases().len() as f64;
    assert!(
        with_model / cases > without_model / cases,
        "comprehension must beat cue matching across the set:\n{}",
        lines.join("\n")
    );
}

#[tokio::test]
async fn the_decision_rationale_reaches_the_summary() {
    // The single most valuable field, and the one the schema had no room for
    // before: "move the launch to Monday" is a note, "move it because QA needs
    // three more days on the payment integration" is a memory.
    use crate::meetings_v2::processing::eval;

    let case = eval::cases()
        .into_iter()
        .find(|c| c.name == "decision_with_rationale")
        .unwrap();
    let processing = run_eval_case(&case, Some(case.model_extraction)).await;
    let markdown = &processing.summary.as_ref().unwrap().markdown;

    let card = eval::score(case.name, markdown, &case.expected);
    assert_eq!(
        card.decision_recall, 1.0,
        "the decision was lost: {}",
        card.report()
    );
    assert_eq!(
        card.rationale_preservation, 1.0,
        "the reason was lost: {}",
        card.report()
    );
    assert!(markdown.contains("because"));
}

#[tokio::test]
async fn a_proposal_never_becomes_a_decision_end_to_end() {
    use crate::meetings_v2::processing::eval;

    let case = eval::cases()
        .into_iter()
        .find(|c| c.name == "proposal_is_not_a_decision")
        .unwrap();
    let processing = run_eval_case(&case, Some(case.model_extraction)).await;
    let facts = processing.facts.as_ref().unwrap();
    let markdown = &processing.summary.as_ref().unwrap().markdown;

    assert!(
        facts.decisions.is_empty(),
        "nothing was settled, so nothing may be recorded as settled"
    );
    assert!(
        !markdown.contains("## Decisions"),
        "an empty section is omitted, never printed with a placeholder:\n{}",
        markdown
    );
    assert!(
        markdown.contains("Proposed:"),
        "the proposal is kept, and kept as a proposal:\n{}",
        markdown
    );
    assert!(eval::score(case.name, markdown, &case.expected)
        .hallucinations
        .is_empty());
}

#[tokio::test]
async fn work_nobody_took_on_reaches_the_user_without_an_owner() {
    use crate::meetings_v2::processing::eval;

    let case = eval::cases()
        .into_iter()
        .find(|c| c.name == "unclear_owner")
        .unwrap();
    let processing = run_eval_case(&case, Some(case.model_extraction)).await;
    let markdown = &processing.summary.as_ref().unwrap().markdown;

    for item in &processing.facts.as_ref().unwrap().action_items {
        assert!(
            matches!(item.owner_type, OwnerType::Unassigned),
            "\"{}\" acquired an owner nobody agreed to be",
            item.description
        );
    }
    assert!(!markdown.contains("**Speaker 1**"));
    assert!(!markdown.contains("**Me**"));
}

#[tokio::test]
async fn a_meeting_that_settled_nothing_says_so_rather_than_inventing_a_decision() {
    use crate::meetings_v2::processing::eval;

    let case = eval::cases()
        .into_iter()
        .find(|c| c.name == "nothing_was_settled")
        .unwrap();
    let processing = run_eval_case(&case, Some(case.model_extraction)).await;
    let facts = processing.facts.as_ref().unwrap();
    let markdown = &processing.summary.as_ref().unwrap().markdown;

    assert!(facts.decisions.is_empty());
    assert!(facts.action_items.is_empty());
    assert!(!markdown.contains("## Decisions"));
    assert!(!markdown.contains("## Action Items"));
    // What it does carry is the thing a reader needs: the question still open.
    assert!(markdown.contains("## Open Questions"));
    assert_eq!(
        eval::score(case.name, markdown, &case.expected).open_question_recall,
        1.0
    );
}

#[tokio::test]
async fn user_notes_reach_stage_a_and_absent_notes_change_nothing() {
    use crate::meetings_v2::processing::eval;

    let case = eval::cases()
        .into_iter()
        .find(|c| c.name == "notes_carry_what_the_transcript_garbled")
        .unwrap();

    let harness = Harness::new(&case.transcript);
    harness
        .sessions
        .save_notes(&harness.meeting_id, &case.notes)
        .unwrap();
    let llm = ScriptedLlm::new(vec![Ok(case.model_extraction.to_string())]);
    harness
        .processor
        .generate_summary(&harness.meeting_id, &llm, &options(), false)
        .await
        .unwrap();

    {
        let calls = llm.calls.lock().unwrap();
        let (system, user) = &calls[0];
        assert!(
            user.contains("alumni placement data is the blocker"),
            "the user's notes must reach the extraction stage"
        );
        assert!(system.contains("SOURCES — how to weigh what you are given"));
        assert!(system.contains("A user's to-do list is not the meeting's action items"));
    }

    // The same meeting with no notes: the prompt loses the notes block and says
    // nothing at all about their absence.
    let bare = Harness::new(&case.transcript);
    let llm = ScriptedLlm::new(vec![Ok(case.model_extraction.to_string())]);
    bare.processor
        .generate_summary(&bare.meeting_id, &llm, &options(), false)
        .await
        .unwrap();

    let calls = llm.calls.lock().unwrap();
    let (system, user) = &calls[0];
    assert!(!user.contains("USER NOTES"));
    assert!(!system.contains("SOURCES — how to weigh"));
    assert!(
        !user.to_lowercase().contains("no notes")
            && !user.to_lowercase().contains("pre-meeting notes: none"),
        "the absence of notes is never mentioned"
    );
}

#[tokio::test]
async fn pre_meeting_notes_are_optional_enrichment_and_never_a_section() {
    // The 1-in-100 case. Present, they add intent; absent, nothing changes.
    let harness = Harness::new(&fixture_a());
    harness
        .sessions
        .save_notes(
            &harness.meeting_id,
            &crate::meetings_v2::types::MeetingNotes {
                before: "agenda: release date, schema freeze".to_string(),
                ..Default::default()
            },
        )
        .unwrap();

    let llm = ScriptedLlm::new(vec![Ok(facts_json()), Ok(short_prose())]);
    let processing = harness
        .processor
        .generate_summary(&harness.meeting_id, &llm, &options(), false)
        .await
        .unwrap();

    {
        let calls = llm.calls.lock().unwrap();
        assert!(calls[0].1.contains("agenda: release date, schema freeze"));
        assert!(calls[0].0.contains("Notes written before the meeting describe intent"));
        // Never handed to the writer as something to reproduce.
        assert!(!calls[1].1.contains("agenda: release date"));
    }

    let markdown = &processing.summary.as_ref().unwrap().markdown;
    assert!(!markdown.contains("Pre-Meeting"));
    assert!(!markdown.contains("Agenda"));
}

// ---------------------------------------------------------------------------
// Validation, repair, long meetings, and regeneration
// ---------------------------------------------------------------------------

#[tokio::test]
async fn a_formatting_failure_is_caught_and_repaired_rather_than_costing_the_summary() {
    // The behaviour this replaces: one fixable slip — an opening line addressed
    // to the user — threw away the whole model-written summary and handed the
    // reader a deterministic fact dump instead.
    let harness = Harness::new(&fixture_a());

    let with_preamble = format!("Sure! Here is the summary you asked for.\n\n{}", short_prose());
    let llm = ScriptedLlm::new(vec![
        Ok(facts_json()),
        Ok(with_preamble),
        Ok(short_prose()),
    ]);

    let processing = harness
        .processor
        .generate_summary(&harness.meeting_id, &llm, &options(), false)
        .await
        .unwrap();

    let summary = processing.summary.as_ref().unwrap();
    assert!(summary.repair_attempted);
    assert!(!summary.deterministic, "the repaired draft is the model's own prose");
    assert_eq!(summary.provider_output_status, ProviderOutputStatus::Accepted);
    assert!(!summary.markdown.contains("Here is the summary"));
    assert!(summary.markdown.starts_with("## Overview"));
    assert!(summary
        .rejected_issues
        .iter()
        .any(|i| i.code == "SUMMARY_HAS_PREAMBLE"));

    // The correction named the rule, rather than re-rolling the same prompt.
    let calls = llm.calls.lock().unwrap();
    assert_eq!(calls.len(), 3);
    let repair_prompt = &calls[2].1;
    assert!(repair_prompt.starts_with("CORRECTION"));
    assert!(repair_prompt.contains("opened with commentary"));
    assert_ne!(
        calls[1].1, calls[2].1,
        "a repair must differ from the attempt it repairs"
    );
}

#[tokio::test]
async fn a_second_failure_falls_back_rather_than_looping() {
    let harness = Harness::new(&fixture_a());
    let bad = format!("Here is the summary.\n\n{}", short_prose());
    let llm = ScriptedLlm::new(vec![Ok(facts_json()), Ok(bad.clone()), Ok(bad)]);

    let processing = harness
        .processor
        .generate_summary(&harness.meeting_id, &llm, &options(), false)
        .await
        .unwrap();

    let summary = processing.summary.as_ref().unwrap();
    assert!(summary.repair_attempted);
    assert!(summary.fallback_used);
    assert_eq!(summary.provider_output_status, ProviderOutputStatus::Rejected);
    assert!(summary.validation.passed, "what is shown is still valid");
    assert_eq!(
        llm.call_count(),
        3,
        "one extraction, one draft, one repair — never a loop"
    );
}

#[tokio::test]
async fn a_long_meeting_is_read_in_passes_and_its_opening_survives() {
    // The failure this replaces was silent and in the provider: a transcript
    // longer than the model's window was handed over whole and the front of it
    // — where the agenda and the framing decisions are — was discarded with
    // nothing in the response to say so.
    let mut fixture: Vec<(&'static str, bool, bool)> = vec![(
        "right, first thing, we decided to move the launch to Monday because QA needs \
another three days on the payment integration",
        true,
        false,
    )];
    for _ in 0..30 {
        fixture.push((
            "then we went through the audio pipeline and the whisper decoding settings and the \
tradeoffs around chunk size and latency in a good deal of detail",
            false,
            true,
        ));
    }
    fixture.push((
        "and last thing before we close, I'll write up the migration plan",
        true,
        false,
    ));

    let harness = Harness::new(&fixture);

    let opening_facts = serde_json::json!({
        "title": "Launch Slip",
        "meeting_type": "planning",
        "key_points": [{"text": "QA needs three more days on the payment integration.", "kind": "discussion", "source_segment_ids": ["seg_00000"]}],
        "topics": [{"label": "Launch timing", "segment_ids": ["seg_00000"]}],
        "decisions": [{"statement": "Move the launch to Monday.", "rationale": "QA needs another three days on the payment integration", "decided_by": "speaker_me", "source_segment_ids": ["seg_00000"]}],
        "action_items": [],
        "open_questions": [],
        "risks": [],
        "entities": []
    })
    .to_string();
    let middle_facts = serde_json::json!({
        "title": "Audio Pipeline Review",
        "meeting_type": "project_review",
        "key_points": [{"text": "Chunk size trades latency against decoding accuracy.", "kind": "tradeoff", "source_segment_ids": []}],
        "topics": [{"label": "Audio pipeline", "segment_ids": []}],
        "decisions": [], "action_items": [], "open_questions": [], "risks": [], "entities": []
    })
    .to_string();

    // A window small enough that this fixture needs several passes.
    let llm = ScriptedLlm::new(vec![
        Ok(opening_facts),
        Ok(middle_facts.clone()),
        Ok(middle_facts.clone()),
        Ok(middle_facts.clone()),
        Ok(middle_facts.clone()),
        Ok(middle_facts.clone()),
        Ok(middle_facts.clone()),
        Ok(middle_facts.clone()),
        Ok(middle_facts.clone()),
        Ok(middle_facts),
    ])
    .with_prompt_budget(3_000);

    let processing = harness
        .processor
        .generate_summary(&harness.meeting_id, &llm, &options(), false)
        .await
        .unwrap();

    let facts = processing.facts.as_ref().unwrap();
    assert!(!facts.deterministic);

    // Extraction passes are the calls that carry the meeting itself; the
    // trailing calls are Stage B, which is given facts.
    let extraction_passes = llm
        .calls
        .lock()
        .unwrap()
        .iter()
        .filter(|(_, user)| user.starts_with("MEETING"))
        .count();
    assert!(
        extraction_passes > 2,
        "a long meeting must be read in more than one pass, got {}",
        extraction_passes
    );

    // The first pass's decision is still there after the later passes ran.
    assert!(
        facts
            .decisions
            .iter()
            .any(|d| d.statement.contains("Move the launch to Monday")),
        "the opening decision was lost: {:?}",
        facts.decisions
    );
    assert!(facts.decisions[0]
        .rationale
        .as_deref()
        .is_some_and(|r| r.contains("three days")));

    // Every pass saw the participants and the meeting's framing, not just a
    // slice of transcript.
    let calls = llm.calls.lock().unwrap();
    let extractions: Vec<&(String, String)> = calls
        .iter()
        .filter(|(_, user)| user.starts_with("MEETING"))
        .collect();
    for (system, user) in &extractions {
        assert!(user.contains("PARTICIPANTS"));
        assert!(
            system.contains("one stretch of a longer meeting"),
            "each pass must know it is reading part of something larger"
        );
    }

    // And ids survive the merge without colliding.
    let ids: std::collections::HashSet<&str> =
        facts.decisions.iter().map(|d| d.id.as_str()).collect();
    assert_eq!(ids.len(), facts.decisions.len());
    let point_ids: std::collections::HashSet<&str> =
        facts.key_points.iter().map(|p| p.id.as_str()).collect();
    assert_eq!(point_ids.len(), facts.key_points.len());
}

#[tokio::test]
async fn a_failed_pass_does_not_cost_the_passes_that_worked() {
    let mut fixture: Vec<(&'static str, bool, bool)> = Vec::new();
    for _ in 0..20 {
        fixture.push((
            "we walked through the migration plan and the rollout sequencing and what it means \
for the teams downstream in a fair amount of detail",
            true,
            false,
        ));
    }
    let harness = Harness::new(&fixture);

    let good = serde_json::json!({
        "title": "Migration Rollout",
        "meeting_type": "planning",
        "key_points": [{"text": "Rollout sequencing affects the downstream teams.", "kind": "discussion", "source_segment_ids": []}],
        "topics": [], "decisions": [], "action_items": [], "open_questions": [], "risks": [], "entities": []
    })
    .to_string();

    // The second pass fails outright; the rest answer.
    let llm = ScriptedLlm::new(vec![
        Ok(good.clone()),
        Err(crate::meetings_v2::processing::llm::LlmError::Unavailable("timeout".into())),
        Ok(good.clone()),
        Ok(good.clone()),
        Ok(good.clone()),
        Ok(good.clone()),
        Ok(good),
    ])
    .with_prompt_budget(3_000);

    let processing = harness
        .processor
        .generate_summary(&harness.meeting_id, &llm, &options(), false)
        .await
        .unwrap();

    let facts = processing.facts.as_ref().unwrap();
    assert!(
        !facts.deterministic,
        "one failed pass must not discard the passes that worked"
    );
    assert!(!facts.key_points.is_empty());
    assert!(processing
        .stages
        .extraction
        .error
        .as_deref()
        .unwrap()
        .contains("part 2"));
}

#[tokio::test]
async fn regeneration_starts_from_the_meeting_not_from_the_previous_summary() {
    // Summarizing a summary loses information every time. Each regeneration
    // must read the same canonical facts the first one did.
    let harness = Harness::new(&fixture_a());

    let llm = ScriptedLlm::new(vec![Ok(facts_json()), Ok(short_prose())]);
    let first = harness
        .processor
        .generate_summary(&harness.meeting_id, &llm, &options(), false)
        .await
        .unwrap();
    let first_markdown = first.summary.as_ref().unwrap().markdown.clone();

    let second_prose = short_prose().replace("Release timing", "Timing");
    let llm = ScriptedLlm::new(vec![Ok(second_prose)]);
    harness
        .processor
        .generate_summary(&harness.meeting_id, &llm, &options(), false)
        .await
        .unwrap();

    let calls = llm.calls.lock().unwrap();
    let user_prompt = &calls[0].1;
    assert!(
        user_prompt.contains("Ship the release on Friday."),
        "the regeneration reads the original facts"
    );
    assert!(
        !user_prompt.contains(&first_markdown),
        "the previous summary must never be the input to the next one"
    );
    for marker in ["## Overview", "## Action Items", "- [ ]"] {
        assert!(
            !user_prompt.contains(marker),
            "rendered prose from the previous run leaked into the input: {}",
            marker
        );
    }
}

#[tokio::test]
async fn conflicting_notes_and_transcript_are_left_conflicting() {
    // Neither source is declared the winner. The rules tell the model to record
    // what the transcript supports and leave the disagreement visible; nothing
    // in the pipeline manufactures a reconciliation.
    let harness = Harness::new(&fixture_a());
    harness
        .sessions
        .save_notes(
            &harness.meeting_id,
            &crate::meetings_v2::types::MeetingNotes {
                during: "launch is Wednesday, not Friday".to_string(),
                ..Default::default()
            },
        )
        .unwrap();

    let llm = ScriptedLlm::new(vec![Ok(facts_json()), Ok(short_prose())]);
    let processing = harness
        .processor
        .generate_summary(&harness.meeting_id, &llm, &options(), false)
        .await
        .unwrap();

    {
        let calls = llm.calls.lock().unwrap();
        let system = &calls[0].0;
        assert!(system.contains("not automatically right and not"));
        assert!(system.contains("leave the disagreement visible rather than picking a"));
    }

    // The note did not become a decision on its own.
    let facts = processing.facts.as_ref().unwrap();
    assert!(
        !facts
            .decisions
            .iter()
            .any(|d| d.statement.to_lowercase().contains("wednesday")),
        "a note is not evidence that something was decided"
    );
}

#[tokio::test]
async fn a_meetings_notes_are_never_written_by_the_pipeline() {
    let harness = Harness::new(&fixture_a());
    let notes = crate::meetings_v2::types::MeetingNotes {
        during: "budget is the blocker".to_string(),
        before: "agenda: budget".to_string(),
        updated_at: None,
    };
    harness
        .sessions
        .save_notes(&harness.meeting_id, &notes)
        .unwrap();

    let notes_path = harness
        .sessions
        .session_dir(&harness.meeting_id)
        .join("notes.json");
    let before = fs::read(&notes_path).unwrap();

    let llm = ScriptedLlm::new(vec![Ok(facts_json()), Ok(short_prose())]);
    harness
        .processor
        .generate_summary(&harness.meeting_id, &llm, &options(), false)
        .await
        .unwrap();

    assert_eq!(
        fs::read(&notes_path).unwrap(),
        before,
        "notes are a source artifact; summarizing must leave them byte-identical"
    );
    let reloaded = harness.sessions.get_notes(&harness.meeting_id).unwrap();
    assert_eq!(reloaded.during, "budget is the blocker");
    assert_eq!(reloaded.before, "agenda: budget");
}

// ---------------------------------------------------------------------------
// Before / after
// ---------------------------------------------------------------------------

/// What the previous pipeline produced for the same meeting.
///
/// Reconstructed from the code it replaced, not invented to lose: the old
/// `MeetingFacts` had no field for a decision's reason, no risks collection, and
/// no way to mark a point as a proposal, so no prompt could have recovered them
/// — and `render_markdown` emitted exactly these four sections in this order.
/// Everything the old schema *could* carry is present here.
fn previous_pipeline_output() -> &'static str {
    "## Summary\n\n\
**Topics discussed:** Launch timing\n\n\
- The payment integration still carries three blocking bugs.\n\
- Shipping on top of the open bugs was judged worse than a weekend of slip.\n\n\
## Decisions\n\n\
- Move the launch from Friday to Monday. — Me\n\n\
## Action Items\n\n\
- [ ] Update the release calendar — **Speaker 1**\n\
- [ ] Tell support about the new date — **Speaker 1**\n"
}

#[tokio::test]
async fn the_new_pipeline_scores_better_on_the_same_meeting() {
    use crate::meetings_v2::processing::eval;

    let case = eval::cases()
        .into_iter()
        .find(|c| c.name == "decision_with_rationale")
        .unwrap();

    let before = eval::score("before", previous_pipeline_output(), &case.expected);

    let processing = run_eval_case(&case, Some(case.model_extraction)).await;
    let after_markdown = processing.summary.as_ref().unwrap().markdown.clone();
    let after = eval::score("after", &after_markdown, &case.expected);

    println!("\nBEFORE  {}", before.report());
    println!("AFTER   {}", after.report());
    println!("\n--- before ---\n{}\n--- after ---\n{}", previous_pipeline_output(), after_markdown);

    // The specific improvements, named rather than summed:
    assert_eq!(
        before.rationale_preservation, 0.0,
        "the old schema had nowhere to put a reason"
    );
    assert_eq!(
        after.rationale_preservation, 1.0,
        "the reason must survive now: {}",
        after.report()
    );
    assert!(
        after.risk_recall >= before.risk_recall,
        "risks were unrepresentable before and must not regress"
    );
    assert!(
        after.structure_ok && !before.structure_ok,
        "the output contract is now enforced"
    );
    assert!(
        before.hallucinations.is_empty() && after.hallucinations.is_empty(),
        "neither may invent anything"
    );

    // And it is not merely longer: decisions and actions were already right,
    // so the gain has to come from information the old output could not carry.
    assert_eq!(before.decision_recall, after.decision_recall);
    assert_eq!(before.action_recall, after.action_recall);
    assert!(
        after.overall() > before.overall(),
        "no measured improvement:\n  before {}\n  after  {}",
        before.report(),
        after.report()
    );
}

#[tokio::test]
async fn the_whole_evaluation_set_improves_and_the_report_is_printable() {
    use crate::meetings_v2::processing::eval;

    let mut floor_total = 0.0;
    let mut model_total = 0.0;
    println!("\nRelay summary quality — evaluation set\n");

    for case in eval::cases() {
        let floor = run_eval_case(&case, None).await;
        let understood = run_eval_case(&case, Some(case.model_extraction)).await;

        let floor_card = eval::score(
            case.name,
            &floor.summary.as_ref().unwrap().markdown,
            &case.expected,
        );
        let model_card = eval::score(
            case.name,
            &understood.summary.as_ref().unwrap().markdown,
            &case.expected,
        );
        println!("  no model     {}", floor_card.report());
        println!("  with model   {}", model_card.report());
        println!();

        floor_total += floor_card.overall();
        model_total += model_card.overall();
    }

    let n = eval::cases().len() as f64;
    println!(
        "  mean overall: no model {:.2}, with model {:.2}",
        floor_total / n,
        model_total / n
    );
    assert!(model_total > floor_total);
}
