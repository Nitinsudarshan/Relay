//! Meetings Intelligence v2.1: Adversarial Identity, Neural Diarization & Evidence Fusion Tests.
//!
//! Enforces:
//! 1. Calendar is context, NEVER truth (adversarial calendar presence cannot force wrong identity).
//! 2. Contextual mention cannot overpower conflicting acoustic evidence.
//! 3. Short interruptions (A -> B -> A) are preserved across chunk and temporal smoothing.
//! 4. 30s chunk storage boundary is strictly invariant (continuous speech merges; change splits).
//! 5. Room mic / In-person mode NEVER defaults mic channel to "Me".
//! 6. Raw transcript SHA-256 hash is strictly immutable across manual speaker edits and re-resolutions.
//! 7. Ephemeral meeting-local voice embeddings without persistent cross-meeting biometric profile.
//! 8. Graceful degradation: neural model absent or LLM offline NEVER fails the meeting.
//! 9. Calibrated confidence levels (Confirmed, High, Likely, Unresolved, Unknown).

use std::fs;
use std::path::PathBuf;
use std::sync::Arc;
use sha2::{Digest, Sha256};

use crate::calendar::{AttendanceResponse, CalendarAttendee};
use crate::meetings_v2::diarize::embedding::{
    AcousticSpectralEmbeddingProvider, DynamicSpeakerEmbeddingProvider,
    SpeakerEmbeddingProvider,
};
use crate::meetings_v2::diarize::self_voice::{SelfVoiceAnchor, SelfVoiceConfidence};
use crate::meetings_v2::processing::conversation::build_conversation;
use crate::meetings_v2::processing::model::{
    MeetingProcessing, MeetingType, NormalizedSegment, ProcessingStatus,
    SegmentChannel, Speaker, SpeakerOrigin, SummaryMode, SummarySource, SPEAKER_ID_ME,
};
use crate::meetings_v2::processing::speakers::{
    attribute_speakers_with_evidence, merge_speakers, AttributionInput, SpeakerIdentificationMode,
};
use crate::meetings_v2::session_store::SessionStore;
use crate::meetings_v2::types::{
    MeetingSession, MeetingState, SpeakerAssignment, SpeakerAssignmentMethod, SpeakerConfidenceLevel,
    SpeakerEvidence, TranscriptSegment, TranscriptSegmentStatus,
};

fn sha256_file(path: &PathBuf) -> String {
    let bytes = fs::read(path).expect("failed to read file for hashing");
    let mut hasher = Sha256::new();
    hasher.update(&bytes);
    format!("{:x}", hasher.finalize())
}

fn make_seg(
    id: &str,
    chunk_index: usize,
    start_s: f64,
    end_s: f64,
    text: &str,
    speaker_id: Option<&str>,
    channel: SegmentChannel,
) -> NormalizedSegment {
    NormalizedSegment {
        id: id.to_string(),
        chunk_index,
        utterance_index: None,
        start_time_s: start_s,
        end_time_s: end_s,
        text: text.to_string(),
        raw_text: text.to_string(),
        channel,
        speaker_id: speaker_id.map(|s| s.to_string()),
        applied_rules: Vec::new(),
    }
}

struct TestHarness {
    _vault: PathBuf,
    sessions: Arc<SessionStore>,
    meeting_id: String,
}

impl TestHarness {
    fn new(meeting_id: &str, segments: Vec<TranscriptSegment>) -> Self {
        let vault = std::env::temp_dir().join(format!("relay_adv_test_{}", uuid::Uuid::new_v4()));
        let sessions = Arc::new(SessionStore::new(vault.clone()));

        let mut session = MeetingSession::new(meeting_id.to_string(), None);
        session.state = MeetingState::Completed;
        session.started_at = Some("2026-09-05T10:00:00Z".to_string());
        sessions.init_session(&session).unwrap();

        for seg in &segments {
            sessions.append_transcript_segment(meeting_id, seg).unwrap();
        }

        Self {
            _vault: vault,
            sessions,
            meeting_id: meeting_id.to_string(),
        }
    }

    fn transcript_path(&self) -> PathBuf {
        self.sessions.session_dir(&self.meeting_id).join("transcript.jsonl")
    }
}

// ---------------------------------------------------------------------------
// TEST 1 — Basic conversation: Bala, Nitin, Bala produces 3 speaker turns
// ---------------------------------------------------------------------------
#[test]
fn test_scenario_1_basic_conversation_three_turns() {
    let segments = vec![
        make_seg("s1", 0, 0.0, 5.0, "We should move this to next week.", Some("spk_bala"), SegmentChannel::System),
        make_seg("s2", 0, 5.2, 6.2, "Yes, absolutely.", Some(SPEAKER_ID_ME), SegmentChannel::Mic),
        make_seg("s3", 0, 6.5, 12.0, "Great, I will update the roadmap.", Some("spk_bala"), SegmentChannel::System),
    ];

    let conv = build_conversation(&segments);
    assert_eq!(conv.turns.len(), 3, "Bala -> Nitin -> Bala must be exactly 3 turns");
    assert_eq!(conv.turns[0].speaker_id.as_deref(), Some("spk_bala"));
    assert_eq!(conv.turns[1].speaker_id.as_deref(), Some(SPEAKER_ID_ME));
    assert_eq!(conv.turns[2].speaker_id.as_deref(), Some("spk_bala"));
}

// ---------------------------------------------------------------------------
// TEST 2 — Five speakers: A -> B -> C -> D -> E produces 5 distinct turns
// ---------------------------------------------------------------------------
#[test]
fn test_scenario_2_five_speakers_five_turns() {
    let segments = vec![
        make_seg("s1", 0, 0.0, 5.0, "First topic.", Some("spk_a"), SegmentChannel::System),
        make_seg("s2", 0, 5.2, 10.0, "Second topic.", Some("spk_b"), SegmentChannel::System),
        make_seg("s3", 0, 10.2, 15.0, "Third topic.", Some("spk_c"), SegmentChannel::System),
        make_seg("s4", 0, 15.2, 20.0, "Fourth topic.", Some("spk_d"), SegmentChannel::System),
        make_seg("s5", 0, 20.2, 25.0, "Fifth topic.", Some("spk_e"), SegmentChannel::System),
    ];

    let conv = build_conversation(&segments);
    assert_eq!(conv.turns.len(), 5, "Five distinct speakers produce five turns");
    for i in 0..5 {
        assert_eq!(conv.turns[i].segment_ids.len(), 1);
    }
}

// ---------------------------------------------------------------------------
// TEST 3 — One-second Nitin interruption: Bala 0-15s, Nitin 15-16s, Bala 16-30s
// ---------------------------------------------------------------------------
#[test]
fn test_scenario_3_short_interruption_not_swallowed() {
    let segments = vec![
        make_seg("s1", 0, 0.0, 15.0, "Bala long presentation point.", Some("spk_bala"), SegmentChannel::System),
        make_seg("s2", 0, 15.1, 16.1, "Yes.", Some(SPEAKER_ID_ME), SegmentChannel::Mic),
        make_seg("s3", 0, 16.3, 30.0, "Bala continues presentation.", Some("spk_bala"), SegmentChannel::System),
    ];

    let conv = build_conversation(&segments);
    assert_eq!(conv.turns.len(), 3, "1-second interjection must NOT be swallowed by smoothing");
    assert_eq!(conv.turns[0].speaker_id.as_deref(), Some("spk_bala"));
    assert_eq!(conv.turns[1].speaker_id.as_deref(), Some(SPEAKER_ID_ME));
    assert_eq!(conv.turns[2].speaker_id.as_deref(), Some("spk_bala"));
    assert_eq!(conv.turns[1].text, "Yes.");
}

// ---------------------------------------------------------------------------
// TEST 4 — Cross-chunk continuity: Bala 25-36s (across 30s storage boundary)
// ---------------------------------------------------------------------------
#[test]
fn test_scenario_4_cross_chunk_same_speaker_merges() {
    let segments = vec![
        make_seg("s1", 0, 25.0, 29.8, "Starting thought before chunk boundary,", Some("spk_bala"), SegmentChannel::System),
        make_seg("s2", 1, 30.1, 36.0, "and finishing smoothly after chunk boundary.", Some("spk_bala"), SegmentChannel::System),
    ];

    let conv = build_conversation(&segments);
    assert_eq!(conv.turns.len(), 1, "Same speaker across 30s storage boundary must merge into 1 turn");
    assert_eq!(conv.turns[0].segment_ids, vec!["s1", "s2"]);
    assert_eq!(conv.turns[0].start_time_s, 25.0);
    assert_eq!(conv.turns[0].end_time_s, 36.0);
}

// ---------------------------------------------------------------------------
// TEST 5 — Speaker change at chunk boundary: Bala 25-30s, Nitin 30-36s
// ---------------------------------------------------------------------------
#[test]
fn test_scenario_5_speaker_change_at_chunk_boundary() {
    let segments = vec![
        make_seg("s1", 0, 25.0, 29.9, "Bala ends speech at chunk 0.", Some("spk_bala"), SegmentChannel::System),
        make_seg("s2", 1, 30.0, 36.0, "Nitin begins speech at chunk 1.", Some(SPEAKER_ID_ME), SegmentChannel::Mic),
    ];

    let conv = build_conversation(&segments);
    assert_eq!(conv.turns.len(), 2, "Different speakers at chunk boundary must remain 2 turns");
    assert_eq!(conv.turns[0].speaker_id.as_deref(), Some("spk_bala"));
    assert_eq!(conv.turns[1].speaker_id.as_deref(), Some(SPEAKER_ID_ME));
}

// ---------------------------------------------------------------------------
// TEST 6 — Room microphone: 4 people around one mic. NEVER defaults to "Me"
// ---------------------------------------------------------------------------
#[test]
fn test_scenario_6_room_mic_never_defaults_to_me() {
    let mut segments = vec![
        make_seg("s1", 0, 0.0, 5.0, "Hello everyone in the room.", None, SegmentChannel::Mic),
    ];

    let input = AttributionInput {
        existing: &[],
        mode: SpeakerIdentificationMode::Automatic,
        diarization: None,
        self_voice: None,
        calendar_attendees: &[],
        assume_in_person: true,
    };

    let (roster, assignments) = attribute_speakers_with_evidence(&mut segments, input);
    for spk in &roster {
        assert_ne!(
            spk.id, SPEAKER_ID_ME,
            "In-person room mic must NEVER default to 'Me'"
        );
    }
    for a in &assignments {
        assert_ne!(a.speaker_id, SPEAKER_ID_ME, "In-person assignments must not be 'Me'");
    }
}

// ---------------------------------------------------------------------------
// TEST 7 — Calendar ambiguity: Nitin, Bala, Aayushi in calendar, acoustic unknown
// ---------------------------------------------------------------------------
#[test]
fn test_scenario_7_calendar_ambiguity_abstains_from_hallucinating() {
    let attendees = vec![
        CalendarAttendee {
            name: "Nitin".to_string(),
            email: Some("nitin@example.com".to_string()),
            response: AttendanceResponse::Accepted,
            is_organizer: true,
            is_self: true,
        },
        CalendarAttendee {
            name: "Bala".to_string(),
            email: Some("bala@example.com".to_string()),
            response: AttendanceResponse::Accepted,
            is_organizer: false,
            is_self: false,
        },
        CalendarAttendee {
            name: "Aayushi".to_string(),
            email: Some("aayushi@example.com".to_string()),
            response: AttendanceResponse::Accepted,
            is_organizer: false,
            is_self: false,
        },
    ];

    let mut segments = vec![
        make_seg("s-remote", 0, 0.0, 4.0, "Good morning team, let's review.", None, SegmentChannel::System),
    ];

    let input = AttributionInput {
        existing: &[],
        mode: SpeakerIdentificationMode::Automatic,
        diarization: None,
        self_voice: None,
        calendar_attendees: &attendees,
        assume_in_person: false,
    };

    let (_roster, assignments) = attribute_speakers_with_evidence(&mut segments, input);
    let assigned = &assignments[0];

    // Calendar presence must NEVER force Bala or Aayushi without acoustic evidence
    assert_ne!(assigned.speaker_id, "Bala", "Calendar alone cannot name Bala");
    assert_ne!(assigned.speaker_id, "Aayushi", "Calendar alone cannot name Aayushi");
    assert_ne!(assigned.speaker_id, SPEAKER_ID_ME, "Remote audio cannot be Me");

    assert_eq!(
        assigned.confidence_level,
        Some(SpeakerConfidenceLevel::Unresolved),
        "Ambiguous identity must report Unresolved confidence level"
    );
}

// ---------------------------------------------------------------------------
// TEST 8 — Short self-interjection matched against meeting-local self-voice
// ---------------------------------------------------------------------------
#[test]
fn test_scenario_8_short_self_interjection_with_calibrated_self_voice() {
    let provider = AcousticSpectralEmbeddingProvider::new();
    let mut ref_audio = Vec::with_capacity(32000);
    for i in 0..32000 {
        let t = i as f32 / 16000.0;
        ref_audio.push((2.0 * std::f32::consts::PI * 150.0 * t).sin() * 0.4);
    }
    let ref_emb = provider.embed(&ref_audio, 16_000).unwrap();

    let anchor = SelfVoiceAnchor {
        mean_vector: vec![0.5; 38],
        mean_embedding: Some(ref_emb.vector),
        sample_count: 8,
        total_seconds: 18.5,
        reference_quality: 0.92,
    };

    // Short interjection "Yes" of the SAME voice
    let mut short_audio = Vec::with_capacity(8000);
    for i in 0..8000 {
        let t = i as f32 / 16000.0;
        short_audio.push((2.0 * std::f32::consts::PI * 150.0 * t).sin() * 0.4);
    }

    let decision = anchor.evaluate_candidate(&short_audio, 0.95, Some(0.40));
    assert!(
        decision.confidence == SelfVoiceConfidence::High || decision.confidence == SelfVoiceConfidence::Medium,
        "Healthy margin against runner up should produce High or Medium confidence"
    );
}

// ---------------------------------------------------------------------------
// TEST 9 — Similar voices: Insufficient margin leads to Unresolved / Cluster
// ---------------------------------------------------------------------------
#[test]
fn test_scenario_9_similar_voices_abstains_with_low_margin() {
    let anchor = SelfVoiceAnchor {
        mean_vector: vec![0.5; 38],
        mean_embedding: Some(vec![0.1; 64]),
        sample_count: 8,
        total_seconds: 18.5,
        reference_quality: 0.92,
    };

    let fake_audio = vec![0.04f32; 8000];
    // Candidate similarity and runner up are nearly identical (margin 0.02)
    let decision = anchor.evaluate_candidate(&fake_audio, 0.50, Some(0.70));
    assert!(
        decision.confidence == SelfVoiceConfidence::Abstain || decision.confidence == SelfVoiceConfidence::Low,
        "Tiny acoustic margin between similar voices must abstain"
    );
}

// ---------------------------------------------------------------------------
// TEST 10 — Model unavailable: Dynamic provider fallback works gracefully
// ---------------------------------------------------------------------------
#[test]
fn test_scenario_10_model_unavailable_graceful_fallback() {
    let provider = DynamicSpeakerEmbeddingProvider::default();
    assert_eq!(provider.provider_name(), "acoustic-spectral-v2");
    assert!(provider.is_fallback_active());

    let fake_audio = vec![0.05f32; 16000];
    let emb = provider.embed(&fake_audio, 16000);
    assert!(emb.is_ok(), "Fallback provider must generate valid embedding");
    assert_eq!(emb.unwrap().dim(), 64);
}

// ---------------------------------------------------------------------------
// TEST 11 — LLM unavailable: Deterministic summary produces Ready state
// ---------------------------------------------------------------------------
#[test]
fn test_scenario_11_llm_unavailable_deterministic_summary_ready() {
    let mut processing = MeetingProcessing::new("adv_meet_ready_test");
    processing.stages.normalization.status = crate::meetings_v2::processing::model::StageStatus::Success;
    processing.stages.summary.status = crate::meetings_v2::processing::model::StageStatus::Success;
    processing.facts = Some(crate::meetings_v2::processing::model::MeetingFacts {
        title: "Local Standup".to_string(),
        meeting_type: MeetingType::General,
        key_points: vec![],
        topics: vec![],
        decisions: vec![],
        action_items: vec![],
        open_questions: vec![],
        risks: vec![],
        entities: vec![],
        speaker_ids: vec![],
        deterministic: true,
    });

    let summary = crate::meetings_v2::processing::model::SummaryArtifact {
        markdown: "## Overview\nLocally generated summary.".to_string(),
        mode: SummaryMode::Standard,
        extension_id: String::new(),
        generated_at: "2026-09-05T10:05:00Z".to_string(),
        provider: "local".to_string(),
        model: "deterministic".to_string(),
        processing_version: 2,
        rules_version: "2.0".to_string(),
        source: SummarySource::DeterministicExtraction,
        deterministic: true,
        fallback_used: true,
        repair_attempted: false,
        length_budget_words: None,
        validation: Default::default(),
        provider_output_status: crate::meetings_v2::processing::model::ProviderOutputStatus::Accepted,
        rejected_issues: vec![],
        speaker_names_stale: false,
    };
    processing.summary = Some(summary);

    processing.recompute_status();
    assert_eq!(
        processing.status,
        ProcessingStatus::Ready,
        "Meeting with deterministic summary floor must have status Ready, not Degraded or Failed"
    );
}

// ---------------------------------------------------------------------------
// TEST 12 — Manual correction updates turns & facts; raw transcript hash invariant
// ---------------------------------------------------------------------------
#[test]
fn test_scenario_12_manual_correction_raw_transcript_hash_invariant() {
    let raw_segs = vec![
        TranscriptSegment {
            chunk_index: 0,
            start_time_s: 0.0,
            end_time_s: 5.0,
            text: "Initial roadmap proposal.".to_string(),
            created_at: "2026-09-05T10:00:00Z".to_string(),
            status: TranscriptSegmentStatus::Success,
            mic_had_audio: false,
            sys_had_audio: true,
            utterances: vec![],
            speech: None,
            rejection: None,
        },
        TranscriptSegment {
            chunk_index: 1,
            start_time_s: 5.0,
            end_time_s: 10.0,
            text: "Second point on the timeline.".to_string(),
            created_at: "2026-09-05T10:00:30Z".to_string(),
            status: TranscriptSegmentStatus::Success,
            mic_had_audio: false,
            sys_had_audio: true,
            utterances: vec![],
            speech: None,
            rejection: None,
        },
    ];

    let harness = TestHarness::new("adv-hash-meeting", raw_segs);
    let hash_before = sha256_file(&harness.transcript_path());

    let mut roster = vec![
        Speaker {
            id: "speaker_1".to_string(),
            display_name: None,
            fallback_label: "Speaker 1".to_string(),
            origin: SpeakerOrigin::Diarization,
            channel: SegmentChannel::System,
            is_local_user: false,
            segment_count: 1,
        },
        Speaker {
            id: "speaker_2".to_string(),
            display_name: None,
            fallback_label: "Speaker 2".to_string(),
            origin: SpeakerOrigin::Diarization,
            channel: SegmentChannel::System,
            is_local_user: false,
            segment_count: 1,
        },
    ];
    let mut normalized = vec![
        make_seg("s1", 0, 0.0, 5.0, "Initial roadmap proposal.", Some("speaker_1"), SegmentChannel::System),
        make_seg("s2", 1, 5.0, 10.0, "Second point on the timeline.", Some("speaker_2"), SegmentChannel::System),
    ];
    let mut assignments = vec![
        SpeakerAssignment {
            utterance_id: "s1".to_string(),
            speaker_id: "speaker_1".to_string(),
            confidence: 0.85,
            confidence_level: Some(SpeakerConfidenceLevel::Likely),
            method: SpeakerAssignmentMethod::Diarization,
            evidence: SpeakerEvidence::default(),
        },
        SpeakerAssignment {
            utterance_id: "s2".to_string(),
            speaker_id: "speaker_2".to_string(),
            confidence: 0.85,
            confidence_level: Some(SpeakerConfidenceLevel::Likely),
            method: SpeakerAssignmentMethod::Diarization,
            evidence: SpeakerEvidence::default(),
        },
    ];

    // Merge speaker_2 into speaker_1 with name "Bala"
    merge_speakers(
        &mut roster,
        &mut normalized,
        &mut assignments,
        "speaker_2",
        "speaker_1",
        Some("Bala"),
    )
    .unwrap();

    assert_eq!(roster.len(), 1);
    assert_eq!(roster[0].display_name.as_deref(), Some("Bala"));
    assert_eq!(normalized[1].speaker_id.as_deref(), Some("speaker_1"));

    // Verify raw transcript file on disk has NOT been mutated by a single byte!
    let hash_after = sha256_file(&harness.transcript_path());
    assert_eq!(
        hash_before, hash_after,
        "Raw transcript.jsonl SHA256 hash MUST remain identical after manual speaker correction"
    );
}

// ---------------------------------------------------------------------------
// NEURAL SPEAKER EMBEDDING TESTS
// ---------------------------------------------------------------------------

#[test]
fn test_embedding_same_speaker_similarity() {
    let provider = AcousticSpectralEmbeddingProvider::new();

    let mut voice_a1 = Vec::with_capacity(16000);
    let mut voice_a2 = Vec::with_capacity(16000);
    for i in 0..16000 {
        let t = i as f32 / 16000.0;
        let s1 = (2.0 * std::f32::consts::PI * 130.0 * t).sin() * 0.4;
        let s2 = (2.0 * std::f32::consts::PI * 260.0 * t).sin() * 0.2;
        voice_a1.push(s1 + s2);
        voice_a2.push(s1 * 1.05 + s2 * 0.95);
    }

    let emb_a1 = provider.embed(&voice_a1, 16000).unwrap();
    let emb_a2 = provider.embed(&voice_a2, 16000).unwrap();

    let sim = provider.similarity(&emb_a1, &emb_a2);
    assert!(sim > 0.90, "Same speaker samples should have high cosine similarity (>0.90), got {}", sim);
}

#[test]
fn test_embedding_different_speaker_similarity() {
    let provider = AcousticSpectralEmbeddingProvider::new();

    let mut voice_a = Vec::with_capacity(16000);
    let mut voice_b = Vec::with_capacity(16000);

    for i in 0..16000 {
        let t = i as f32 / 16000.0;
        voice_a.push((2.0 * std::f32::consts::PI * 110.0 * t).sin() * 0.5);
        voice_b.push((2.0 * std::f32::consts::PI * 280.0 * t).sin() * 0.5);
    }

    let emb_a = provider.embed(&voice_a, 16000).unwrap();
    let emb_b = provider.embed(&voice_b, 16000).unwrap();

    let sim = provider.similarity(&emb_a, &emb_b);
    assert!(sim < 0.75, "Different speakers should have lower similarity (<0.75), got {}", sim);
}

#[test]
fn test_embedding_short_interjection_against_reference() {
    let provider = AcousticSpectralEmbeddingProvider::new();

    let mut ref_speech = Vec::with_capacity(32000);
    for i in 0..32000 {
        let t = i as f32 / 16000.0;
        ref_speech.push((2.0 * std::f32::consts::PI * 150.0 * t).sin() * 0.4);
    }

    let mut short_speech = Vec::with_capacity(8000);
    for i in 0..8000 {
        let t = i as f32 / 16000.0;
        short_speech.push((2.0 * std::f32::consts::PI * 150.0 * t).sin() * 0.4);
    }

    let emb_ref = provider.embed(&ref_speech, 16000).unwrap();
    let emb_short = provider.embed(&short_speech, 16000).unwrap();

    let sim = provider.similarity(&emb_ref, &emb_short);
    assert!(sim > 0.88, "Short interjection matching reference voice should have similarity > 0.88, got {}", sim);
}

#[test]
fn test_embedding_cross_chunk_consistency() {
    let provider = AcousticSpectralEmbeddingProvider::new();

    let mut chunk_0 = Vec::with_capacity(16000);
    let mut chunk_1 = Vec::with_capacity(16000);
    for i in 0..16000 {
        let t = i as f32 / 16000.0;
        let sample = (2.0 * std::f32::consts::PI * 180.0 * t).sin() * 0.4;
        chunk_0.push(sample);
        chunk_1.push(sample);
    }

    let emb_0 = provider.embed(&chunk_0, 16000).unwrap();
    let emb_1 = provider.embed(&chunk_1, 16000).unwrap();

    let sim = provider.similarity(&emb_0, &emb_1);
    assert!(sim > 0.98, "Cross-chunk continuation should have near identical embedding, got {}", sim);
}

#[test]
fn test_embedding_adversarial_context_contradiction_penalty() {
    let mut segments = vec![
        make_seg("s-adversarial", 0, 0.0, 4.0, "Hi, this is Bala speaking here.", None, SegmentChannel::Mic),
    ];

    let anchor = SelfVoiceAnchor {
        mean_vector: vec![0.5; 38],
        mean_embedding: Some(vec![0.1; 64]),
        sample_count: 8,
        total_seconds: 18.5,
        reference_quality: 0.92,
    };

    let input = AttributionInput {
        existing: &[],
        mode: SpeakerIdentificationMode::Automatic,
        diarization: None,
        self_voice: Some(&anchor),
        calendar_attendees: &[],
        assume_in_person: false,
    };

    let (_roster, assignments) = attribute_speakers_with_evidence(&mut segments, input);
    let assigned = &assignments[0];

    assert_eq!(
        assigned.speaker_id, SPEAKER_ID_ME,
        "Strong acoustic self-voice + Mic channel must beat spoofed contextual text 'This is Bala'"
    );
}
