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
use crate::meetings_v2::processing::model::{OwnerType, SPEAKER_ID_ME, SPEAKER_ID_REMOTE};
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
            {"text": "Timing was settled after weighing the migration risk.", "topic": "Release Planning", "source_segment_ids": ["seg_00000"]},
            {"text": "Schema stability was prioritized for this sprint.", "topic": "Data Migration Strategy", "source_segment_ids": ["seg_00001"]}
        ],
        "topics": [
            {"label": "Release Planning", "segment_ids": ["seg_00000"]},
            {"label": "Data Migration Strategy", "segment_ids": ["seg_00001"]}
        ],
        "decisions": [
            {"statement": "Ship the release on Friday.", "decided_by": "speaker_me", "source_segment_ids": ["seg_00000"]},
            {"statement": "Freeze the schema for this sprint.", "decided_by": "speaker_1", "source_segment_ids": ["seg_00001"]}
        ],
        "action_items": [
            {"description": "Write the changelog", "owner": "speaker_me", "deadline": "2026-08-28", "source_segment_ids": ["seg_00000"]},
            {"description": "Review the migration script", "owner": "speaker_1", "source_segment_ids": ["seg_00001"]}
        ],
        "open_questions": [
            {"question": "Who signs off on the migration?", "source_segment_ids": ["seg_00001"]}
        ],
        "entities": [
            {"name": "Relay", "kind": "product", "segment_ids": ["seg_00000"]}
        ]
    })
    .to_string()
}

fn prose() -> String {
    "## Summary\n\n- Release timing was settled once the migration risk had been weighed.\n\
- Schema stability was prioritized over new fields for this sprint.\n\n\
## Decisions\n\n- The release ships Friday — Me\n- The schema is frozen for the sprint — Speaker 1\n\n\
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
    assert!(summary.markdown.contains("## Summary"));

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
    let bad_prose = "## Summary\n\n- Work was distributed among the attendees today.\n\n\
## Action Items\n\n- [ ] Send the deck — **Rajesh**\n";
    let llm = ScriptedLlm::new(vec![Ok(facts_json()), Ok(bad_prose.to_string())]);

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
    assert!(summary
        .validation
        .issues
        .iter()
        .any(|i| i.code == "SUMMARY_INVENTED_PARTICIPANT"));
    assert!(summary
        .validation
        .issues
        .iter()
        .any(|i| i.code == "SUMMARY_INVENTED_PARTICIPANT"));
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
        let llm = ScriptedLlm::new(vec![Ok(prose())]);
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
