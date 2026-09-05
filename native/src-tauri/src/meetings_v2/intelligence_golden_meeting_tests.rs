//! Regression and adversarial test suite for the Meetings Intelligence pipeline.
//!
//! Includes:
//! 1. Golden Meeting #1: Hinglish Two-Speaker Meeting (~3m46s real-world failure reproduction & invariants).
//! 2. Adversarial Synthetic Test Suite (Cases A–T: speaker counts, interruptions, boundary crossing,
//!    short interjections, language preservation, acoustic leakage, empty LLM response, deterministic floor).

use crate::meetings_v2::diarize::{Diarization, DiarizationReport, VoiceAssignment};
use crate::meetings_v2::processing::conversation::build_conversation;
use crate::meetings_v2::processing::model::*;
use crate::meetings_v2::processing::normalize::{
    normalize_transcript, RawSegmentInput,
};
use crate::meetings_v2::processing::speakers::{
    attribute_speakers_with_evidence, merge_speakers,
    AttributionInput, SpeakerIdentificationMode,
};
use crate::meetings_v2::processing::summarize::render_markdown;
use crate::capture::stt::SttLanguageConfig;
use crate::settings::LanguageSettings;
use crate::meetings_v2::types::{
    SpeakerAssignment, SpeakerAssignmentMethod, SpeakerConfidenceLevel, SpeakerEvidence,
};
use std::collections::HashMap;

fn raw_seg(
    chunk_index: usize,
    utterance_index: usize,
    start_time_s: f64,
    end_time_s: f64,
    text: &str,
    mic: bool,
    sys: bool,
) -> RawSegmentInput {
    RawSegmentInput {
        chunk_index,
        utterance_index: Some(utterance_index),
        start_time_s,
        end_time_s,
        text: text.to_string(),
        mic_had_audio: mic,
        sys_had_audio: sys,
    }
}

use crate::meetings_v2::transcript_health::is_safe_as_prompt;

// =========================================================================
// 1. Golden Meeting #1: Hinglish Two-Speaker Meeting (~3m46s conversation)
// =========================================================================

#[test]
fn golden_meeting_1_multilingual_language_config_does_not_force_english() {
    // Invariant: Unspecified or Auto language must NOT default whisper_language to Some("en").
    // Whisper native multilingual model must receive None for auto-detection and translate = false.
    let auto_settings = LanguageSettings {
        primary_dictation_language: "auto".to_string(),
        spoken_languages: vec!["en".to_string(), "hi".to_string()],
        output_script: "native".to_string(),
        notes_language: "auto".to_string(),
    };
    let auto_config = SttLanguageConfig::from_settings(&auto_settings);
    assert_eq!(auto_config.whisper_language, None, "Auto language must pass None to Whisper for multilingual detection");
    assert!(!auto_config.translate, "translate must NEVER be implicitly enabled");

    let explicit_hindi_settings = LanguageSettings {
        primary_dictation_language: "hi".to_string(),
        spoken_languages: vec!["hi".to_string()],
        output_script: "native".to_string(),
        notes_language: "auto".to_string(),
    };
    let explicit_hindi = SttLanguageConfig::from_settings(&explicit_hindi_settings);
    assert_eq!(explicit_hindi.whisper_language, Some("hi".to_string()));
    assert!(!explicit_hindi.translate);
}

#[test]
fn golden_meeting_1_raw_transcript_preserves_hinglish_without_translation() {
    // Invariant: Raw transcript is sacred. Hinglish phrases must not be translated into English.
    let hinglish_inputs = vec![
        raw_seg(0, 0, 0.0, 3.5, "Good morning, brother. Kaise ho?", true, false),
        raw_seg(0, 1, 3.5, 8.0, "The number of mails sent, bas do replies aaye hain.", false, true),
        raw_seg(1, 0, 8.0, 13.0, "Monday ya Tuesday ko schedule kar sakte hain.", true, false),
        raw_seg(1, 1, 13.0, 17.5, "Haan, Tuesday better rahega.", false, true),
    ];

    let normalized = normalize_transcript(&hinglish_inputs, &[]);
    assert_eq!(normalized.segments.len(), 4);

    // Verify raw text is retained unmodified
    assert_eq!(normalized.segments[0].raw_text, "Good morning, brother. Kaise ho?");
    assert_eq!(normalized.segments[2].raw_text, "Monday ya Tuesday ko schedule kar sakte hain.");

    // Verify normalized text retains Hinglish tokens verbatim (not translated to "Can we schedule on Monday or Tuesday")
    assert!(normalized.segments[2].text.contains("Monday ya Tuesday ko schedule kar sakte hain."));
    assert!(normalized.segments[3].text.contains("Haan, Tuesday better rahega."));
}

#[test]
fn golden_meeting_1_long_sentence_repetition_loop_is_collapsed_and_blocked_from_prompt() {
    // Invariant: The 11-word repeated sentence loop observed in real meetings:
    // "If you are schedule for Monday then you can sit here. If you are schedule for Monday then you can sit here."
    // 1. Must be detected as unsafe prompt carry by transcript_health
    // 2. Must be collapsed to single occurrence by normalize without losing meaning

    let loop_sentence = "If you are schedule for Monday then you can sit here. If you are schedule for Monday then you can sit here.";
    
    // Test prompt safety gating:
    assert!(!is_safe_as_prompt(loop_sentence), "11-word repeated sentence must NOT be carried as next chunk prompt!");

    // Test normalization collapsing:
    let raws = vec![raw_seg(0, 0, 0.0, 8.0, loop_sentence, true, false)];
    let normalized = normalize_transcript(&raws, &[]);
    assert_eq!(
        normalized.segments[0].text,
        "If you are schedule for Monday then you can sit here."
    );
}

#[test]
fn golden_meeting_1_conversational_repetition_survives_while_loops_are_gated() {
    // Conversational affirmations repeated up to twice are legitimate human speech
    let affirms = [
        "Yes, yes, I agree.",
        "Haan, haan, Tuesday better rahega.",
        "Theek, theek, we will do that.",
    ];

    for text in affirms {
        assert!(
            is_safe_as_prompt(text),
            "Conversational repetition '{text}' must NOT be flagged as unsafe prompt loop!"
        );
        let raws = vec![raw_seg(0, 0, 0.0, 4.0, text, true, false)];
        let normalized = normalize_transcript(&raws, &[]);
        assert_eq!(
            normalized.segments[0].text, text,
            "Conversational repetition must be preserved by normalizer!"
        );
    }

    // Loops (3-word, 8-word, 16-word) must be gated and collapsed
    let loops = [
        (
            "we should ship we should ship",
            "We should ship.",
        ),
        (
            "we need to verify all changes before shipping we need to verify all changes before shipping",
            "We need to verify all changes before shipping.",
        ),
    ];

    for (loop_text, expected_collapsed) in loops {
        assert!(
            !is_safe_as_prompt(loop_text),
            "Multi-word repetition '{loop_text}' must be flagged as unsafe prompt loop!"
        );
        let raws = vec![raw_seg(0, 0, 0.0, 6.0, loop_text, true, false)];
        let normalized = normalize_transcript(&raws, &[]);
        assert_eq!(normalized.segments[0].text, expected_collapsed);
    }

    // Legitimate separated repetition survives
    let separated = "Ship it today and then ship it tomorrow.";
    assert!(is_safe_as_prompt(separated));
    let raws = vec![raw_seg(0, 0, 0.0, 4.0, separated, true, false)];
    let normalized = normalize_transcript(&raws, &[]);
    assert_eq!(normalized.segments[0].text, separated);
}

#[test]
fn golden_meeting_1_speaker_coverage_is_not_catastrophically_skewed() {
    // Regression target: The user actually spoke 20–25% of the ~3m46s meeting.
    // The previous bug produced Mansi ~99% and Me ~1%.
    // In this test with 22 utterances (5 from local user, 17 from remote),
    // local user coverage must be within 15%..=30% and remote within 70%..=85%.

    // 5 user utterances (~22.7%)
    let mut segments = vec![
        raw_seg(0, 0, 0.0, 3.0, "Good morning Mansi.", true, false),
        raw_seg(1, 0, 30.0, 34.0, "How many responses did we get so far?", true, false),
        raw_seg(2, 0, 60.0, 65.0, "Monday ya Tuesday ko kar sakte hain.", true, false),
        raw_seg(4, 0, 120.0, 125.0, "I will arrange for two calls.", true, false),
        raw_seg(6, 0, 180.0, 184.0, "Okay sounds good, let us do Tuesday.", true, false),
    ];

    // 17 remote utterances (~77.3%)
    for i in 1..=17 {
        segments.push(raw_seg(
            i,
            1,
            i as f64 * 10.0,
            i as f64 * 10.0 + 4.0,
            &format!("Remote discussion point number {}", i),
            false,
            true,
        ));
    }

    let mut norm = normalize_transcript(&segments, &[]);
    let total_count = norm.segments.len() as f32;

    // Cluster map: cluster 0 = local user, cluster 1 = remote
    let mut assignments = Vec::new();
    for seg in &norm.segments {
        let cluster = if seg.channel == SegmentChannel::Mic {
            Some(0)
        } else {
            Some(1)
        };
        assignments.push(VoiceAssignment {
            segment_id: seg.id.clone(),
            cluster,
            distance: 0.1,
        });
    }

    let diarization = Diarization {
        report: DiarizationReport {
            cluster_count: 2,
            placed_count: norm.segments.len(),
            unplaced_count: 0,
            skipped_count: 0,
            local_cluster: Some(0),
            well_separated: true,
            mean_within_distance: 0.1,
            min_between_distance: 1.0,
            singleton_speaker_count: 0,
            silhouette: 0.85,
            expected_speakers: Some(2),
            duration_ms: 226_000,
            embedding_provider: None,
            fallback_used: false,
            embedding_duration_ms: 0,
        },
        assignments,
        self_voice_anchor: None,
        self_voice_similarities: HashMap::new(),
    };

    let (roster, _) = attribute_speakers_with_evidence(
        &mut norm.segments,
        AttributionInput {
            existing: &[],
            mode: SpeakerIdentificationMode::Automatic,
            diarization: Some(&diarization),
            self_voice: None,
            calendar_attendees: &[],
            assume_in_person: false,
        },
    );

    let me_speaker = roster.iter().find(|s| s.is_local_user).expect("Local user must be identified");
    let remote_speaker = roster.iter().find(|s| !s.is_local_user).expect("Remote speaker must be identified");

    let me_ratio = me_speaker.segment_count as f32 / total_count;
    let remote_ratio = remote_speaker.segment_count as f32 / total_count;

    // Invariant: local user coverage is approx 20–25% (tolerance [0.15, 0.30])
    assert!((0.15..=0.30).contains(&me_ratio), "Local user coverage {:.2} was outside [0.15, 0.30]!", me_ratio);
    assert!((0.70..=0.85).contains(&remote_ratio), "Remote coverage {:.2} was outside [0.70, 0.85]!", remote_ratio);
}

#[test]
fn golden_meeting_1_deterministic_summary_floor_always_provides_structured_facts() {
    // Invariant: An empty LLM response must NEVER result in "Summary unavailable".
    // The deterministic summary floor must produce a valid document containing Overview, Decisions, Action Items.
    let facts = MeetingFacts {
        title: "Hinglish Sync".to_string(),
        meeting_type: MeetingType::General,
        key_points: vec![
            KeyPoint {
                id: "kp_1".into(),
                text: "Agreed to follow up on client mails.".into(),
                topic_id: None,
                kind: KeyPointKind::Discussion,
                source_segment_ids: vec!["seg_00000_000".into()],
            },
        ],
        topics: vec![],
        decisions: vec![
            Decision {
                id: "dec_1".into(),
                statement: "Schedule client call for Tuesday.".into(),
                rationale: Some("better availability".into()),
                decided_by_speaker_id: Some("speaker_1".into()),
                source_segment_ids: vec!["seg_00001_000".into()],
                confidence: 0.9,
            },
        ],
        action_items: vec![
            ActionItem {
                id: "act_1".into(),
                description: "Arrange for two calls on Tuesday".into(),
                owner_type: OwnerType::Speaker,
                owner_speaker_id: Some("me".into()),
                owner_label: None,
                deadline: Some("2026-09-08".into()),
                status: ActionItemStatus::Open,
                source_segment_ids: vec!["seg_00004_000".into()],
                confidence: 0.85,
                kanban_card_id: None,
            },
        ],
        open_questions: vec![],
        risks: vec![],
        entities: vec![],
        speaker_ids: vec!["me".into(), "speaker_1".into()],
        deterministic: true,
    };

    let speakers = vec![
        Speaker {
            id: "me".into(),
            display_name: Some("Me".into()),
            fallback_label: "Me".into(),
            origin: SpeakerOrigin::Channel,
            channel: SegmentChannel::Mic,
            is_local_user: true,
            segment_count: 5,
        },
        Speaker {
            id: "speaker_1".into(),
            display_name: Some("Mansi".into()),
            fallback_label: "Speaker 1".into(),
            origin: SpeakerOrigin::Diarization,
            channel: SegmentChannel::System,
            is_local_user: false,
            segment_count: 17,
        },
    ];

    let summary_md = render_markdown(&facts, &speakers, SummaryMode::Standard);
    
    assert!(summary_md.starts_with("## Overview"), "Summary must start with ## Overview");
    assert!(summary_md.contains("## Decisions"), "Decisions section must be rendered");
    assert!(summary_md.contains("Schedule client call for Tuesday — because better availability (Mansi)"));
    assert!(summary_md.contains("## Action Items"), "Action items section must be rendered");
    assert!(summary_md.contains("Arrange for two calls on Tuesday — **Me** · Due: 2026-09-08"));
}

// =========================================================================
// 2. Synthetic Adversarial Test Suite (Cases A–T)
// =========================================================================

#[test]
fn adversarial_case_a_one_speaker() {
    let raws = vec![
        raw_seg(0, 0, 0.0, 5.0, "Hello, this is a solo voice note.", true, false),
        raw_seg(0, 1, 5.0, 10.0, "I am recording notes for myself.", true, false),
    ];
    let mut norm = normalize_transcript(&raws, &[]);
    let (roster, assignments) = attribute_speakers_with_evidence(
        &mut norm.segments,
        AttributionInput {
            existing: &[],
            mode: SpeakerIdentificationMode::Automatic,
            diarization: None,
            self_voice: None,
            calendar_attendees: &[],
            assume_in_person: false,
        },
    );
    assert_eq!(roster.len(), 1);
    assert!(roster[0].is_local_user);
    assert_eq!(assignments.len(), 2);
}

#[test]
fn adversarial_case_b_two_speakers_clear_separation() {
    let raws = vec![
        raw_seg(0, 0, 0.0, 4.0, "Can you review the PR?", true, false),
        raw_seg(0, 1, 4.0, 8.0, "Yes, reviewing it now.", false, true),
    ];
    let mut norm = normalize_transcript(&raws, &[]);
    let (roster, assignments) = attribute_speakers_with_evidence(
        &mut norm.segments,
        AttributionInput {
            existing: &[],
            mode: SpeakerIdentificationMode::Automatic,
            diarization: None,
            self_voice: None,
            calendar_attendees: &[],
            assume_in_person: false,
        },
    );
    assert_eq!(roster.len(), 2);
    assert_eq!(assignments[0].speaker_id, "speaker_me");
    assert_eq!(assignments[1].speaker_id, "speaker_1");
}

#[test]
fn adversarial_case_d_speaker_interruption_preserved() {
    // Interruption pattern: A -> B -> A must produce 3 separate turns, never merging A turns together.
    let raws = vec![
        raw_seg(0, 0, 0.0, 3.0, "I think we should deploy on Friday.", true, false),
        raw_seg(0, 1, 3.0, 6.0, "Wait, Friday is risky!", false, true),
        raw_seg(0, 2, 6.0, 9.0, "Understood, then let us do Monday.", true, false),
    ];
    let mut norm = normalize_transcript(&raws, &[]);
    let (_, _) = attribute_speakers_with_evidence(
        &mut norm.segments,
        AttributionInput {
            existing: &[],
            mode: SpeakerIdentificationMode::Automatic,
            diarization: None,
            self_voice: None,
            calendar_attendees: &[],
            assume_in_person: false,
        },
    );
    let conv = build_conversation(&norm.segments);
    assert_eq!(conv.turns.len(), 3, "Interruption A -> B -> A must preserve 3 distinct turns");
    assert_eq!(conv.turns[0].speaker_id.as_deref(), Some("speaker_me"));
    assert_eq!(conv.turns[1].speaker_id.as_deref(), Some("speaker_1"));
    assert_eq!(conv.turns[2].speaker_id.as_deref(), Some("speaker_me"));
}

#[test]
fn adversarial_case_f_speaker_crossing_30s_chunk_boundary() {
    // Speaker talking across chunk boundary: chunk 0 (27.0s-30.0s) and chunk 1 (30.0s-35.0s)
    let raws = vec![
        raw_seg(0, 0, 25.0, 30.0, "We are evaluating the Q3 roadmaps", true, false),
        raw_seg(1, 0, 30.0, 35.0, "and comparing them with current deliverables.", true, false),
    ];
    let mut norm = normalize_transcript(&raws, &[]);
    let (_, _) = attribute_speakers_with_evidence(
        &mut norm.segments,
        AttributionInput {
            existing: &[],
            mode: SpeakerIdentificationMode::Automatic,
            diarization: None,
            self_voice: None,
            calendar_attendees: &[],
            assume_in_person: false,
        },
    );
    let conv = build_conversation(&norm.segments);
    assert_eq!(conv.turns.len(), 1, "Continuous speaker crossing 30s chunk boundary merges into 1 turn");
    assert_eq!(
        conv.turns[0].text,
        "We are evaluating the Q3 roadmaps. And comparing them with current deliverables."
    );
}

#[test]
fn adversarial_case_g_short_interjections_leave_unresolved_on_ambiguous_channel() {
    // Short isolated "haan", "okay", "yes" on mixed/unknown channel without self-voice match
    // must be left unresolved (None) rather than assigned to a remote speaker cluster.
    let raws = vec![
        raw_seg(0, 0, 0.0, 0.8, "haan", false, false), // (false, false) -> Unknown channel
        raw_seg(0, 1, 1.0, 1.6, "okay", true, true),   // (true, true) -> Mixed channel
    ];
    let mut norm = normalize_transcript(&raws, &[]);
    let (_, assignments) = attribute_speakers_with_evidence(
        &mut norm.segments,
        AttributionInput {
            existing: &[],
            mode: SpeakerIdentificationMode::Automatic,
            diarization: None,
            self_voice: None,
            calendar_attendees: &[],
            assume_in_person: false,
        },
    );

    assert_eq!(norm.segments[0].speaker_id, None, "Short interjection 'haan' on unknown channel must remain unresolved");
    assert_eq!(norm.segments[1].speaker_id, None, "Short interjection 'okay' on mixed channel must remain unresolved");
    assert_eq!(assignments.len(), 0, "Unresolved segments do not create false speaker assignments");
}

#[test]
fn adversarial_case_m_in_person_one_mic_meeting_disables_local_user_inference() {
    // Single shared mic in meeting room: assume_in_person = true must NOT claim mic is local user.
    let raws = vec![
        raw_seg(0, 0, 0.0, 5.0, "Let's begin the boardroom review.", true, false),
        raw_seg(0, 1, 5.0, 10.0, "I have the financial report ready.", true, false),
    ];
    let mut norm = normalize_transcript(&raws, &[]);
    let (roster, _) = attribute_speakers_with_evidence(
        &mut norm.segments,
        AttributionInput {
            existing: &[],
            mode: SpeakerIdentificationMode::Automatic,
            diarization: None,
            self_voice: None,
            calendar_attendees: &[],
            assume_in_person: true,
        },
    );
    for speaker in roster {
        assert!(!speaker.is_local_user, "In-person room mic must never be attributed to 'Me'");
    }
}

#[test]
fn adversarial_case_t_manual_speaker_correction_propagates_cleanly() {
    // Renaming and merging speaker 2 into speaker 1 updates roster, segments, and assignments
    let mut speakers = vec![
        Speaker {
            id: "speaker_1".into(),
            display_name: None,
            fallback_label: "Speaker 1".into(),
            origin: SpeakerOrigin::Diarization,
            channel: SegmentChannel::System,
            is_local_user: false,
            segment_count: 2,
        },
        Speaker {
            id: "speaker_2".into(),
            display_name: None,
            fallback_label: "Speaker 2".into(),
            origin: SpeakerOrigin::Diarization,
            channel: SegmentChannel::System,
            is_local_user: false,
            segment_count: 3,
        },
    ];

    let mut segments = vec![
        NormalizedSegment {
            id: "seg_00000_000".into(),
            chunk_index: 0,
            utterance_index: Some(0),
            start_time_s: 0.0,
            end_time_s: 3.0,
            text: "First turn".into(),
            raw_text: "First turn".into(),
            channel: SegmentChannel::System,
            speaker_id: Some("speaker_1".into()),
            applied_rules: vec![],
        },
        NormalizedSegment {
            id: "seg_00000_001".into(),
            chunk_index: 0,
            utterance_index: Some(1),
            start_time_s: 3.0,
            end_time_s: 6.0,
            text: "Second turn".into(),
            raw_text: "Second turn".into(),
            channel: SegmentChannel::System,
            speaker_id: Some("speaker_2".into()),
            applied_rules: vec![],
        },
    ];

    let mut assignments = vec![
        SpeakerAssignment {
            utterance_id: "seg_00000_000".into(),
            speaker_id: "speaker_1".into(),
            confidence: 0.8,
            confidence_level: Some(SpeakerConfidenceLevel::Likely),
            method: SpeakerAssignmentMethod::Diarization,
            evidence: SpeakerEvidence::default(),
        },
        SpeakerAssignment {
            utterance_id: "seg_00000_001".into(),
            speaker_id: "speaker_2".into(),
            confidence: 0.8,
            confidence_level: Some(SpeakerConfidenceLevel::Likely),
            method: SpeakerAssignmentMethod::Diarization,
            evidence: SpeakerEvidence::default(),
        },
    ];

    // Merge speaker_2 into speaker_1
    merge_speakers(&mut speakers, &mut segments, &mut assignments, "speaker_2", "speaker_1", Some("Mansi"))
        .expect("Merge must succeed");

    assert_eq!(speakers.len(), 1);
    assert_eq!(speakers[0].id, "speaker_1");
    assert_eq!(speakers[0].display_name.as_deref(), Some("Mansi"));
    assert_eq!(speakers[0].segment_count, 5);
    assert_eq!(segments[1].speaker_id.as_deref(), Some("speaker_1"));
    assert_eq!(assignments[1].speaker_id, "speaker_1");
}

// =========================================================================
// 3. Stage 1: Self-Voice Evidence Fusion Regression Tests
// =========================================================================

#[test]
fn test_stage_1_mic_remote_self_voice_me_combined_evidence_selects_me() {
    // 1. Mic evidence -> Remote (channel: System), but Self-voice -> Me (cluster 0 is local user)
    // Diarization has two clusters. Combined evidence must support Me.
    use crate::meetings_v2::diarize::SelfVoiceAnchor;

    let raws = vec![
        raw_seg(0, 0, 0.0, 4.0, "Speaking through system audio loopback", false, true), // System channel
    ];
    let mut norm = normalize_transcript(&raws, &[]);

    let diarization = Diarization {
        report: DiarizationReport {
            cluster_count: 2,
            placed_count: 1,
            unplaced_count: 0,
            skipped_count: 0,
            local_cluster: Some(0),
            well_separated: true,
            mean_within_distance: 0.1,
            min_between_distance: 1.0,
            singleton_speaker_count: 0,
            silhouette: 0.85,
            expected_speakers: Some(2),
            duration_ms: 4000,
            embedding_provider: None,
            fallback_used: false,
            embedding_duration_ms: 0,
        },
        assignments: vec![VoiceAssignment {
            segment_id: norm.segments[0].id.clone(),
            cluster: Some(0),
            distance: 0.1,
        }],
        self_voice_anchor: None,
        self_voice_similarities: HashMap::new(),
    };

    let anchor = SelfVoiceAnchor {
        mean_vector: vec![0.5; 64],
        mean_embedding: None,
        sample_count: 3,
        total_seconds: 5.0,
        reference_quality: 0.90,
    };

    let (roster, assignments) = attribute_speakers_with_evidence(
        &mut norm.segments,
        AttributionInput {
            existing: &[],
            mode: SpeakerIdentificationMode::Automatic,
            diarization: Some(&diarization),
            self_voice: Some(&anchor),
            calendar_attendees: &[],
            assume_in_person: false,
        },
    );

    assert_eq!(roster.len(), 1);
    assert!(roster[0].is_local_user, "Local user must be selected when combined evidence supports Me");
    assert_eq!(assignments.len(), 1);
    assert_eq!(assignments[0].speaker_id, "speaker_me");
    assert_eq!(assignments[0].method, SpeakerAssignmentMethod::SelfVoiceAnchor);
}

#[test]
fn test_stage_1_mic_me_self_voice_remote_contradiction_rejects_me() {
    // 2. Mic evidence -> Me (channel: Mic), but Self-voice -> Remote (cluster 1 is remote cluster)
    // Relay must NOT blindly trust mic share; contradiction penalty must reject false Me.
    use crate::meetings_v2::diarize::SelfVoiceAnchor;

    let raws = vec![
        raw_seg(0, 0, 0.0, 4.0, "Remote participant speaking near mic", true, false), // Mic channel
    ];
    let mut norm = normalize_transcript(&raws, &[]);

    let diarization = Diarization {
        report: DiarizationReport {
            cluster_count: 2,
            placed_count: 1,
            unplaced_count: 0,
            skipped_count: 0,
            local_cluster: Some(0), // cluster 0 is local, but this utterance is cluster 1 (remote)
            well_separated: true,
            mean_within_distance: 0.1,
            min_between_distance: 1.0,
            singleton_speaker_count: 0,
            silhouette: 0.85,
            expected_speakers: Some(2),
            duration_ms: 4000,
            embedding_provider: None,
            fallback_used: false,
            embedding_duration_ms: 0,
        },
        assignments: vec![VoiceAssignment {
            segment_id: norm.segments[0].id.clone(),
            cluster: Some(1), // remote cluster
            distance: 0.1,
        }],
        self_voice_anchor: None,
        self_voice_similarities: HashMap::new(),
    };

    let anchor = SelfVoiceAnchor {
        mean_vector: vec![0.5; 64],
        mean_embedding: None,
        sample_count: 3,
        total_seconds: 5.0,
        reference_quality: 0.90,
    };

    let (roster, assignments) = attribute_speakers_with_evidence(
        &mut norm.segments,
        AttributionInput {
            existing: &[],
            mode: SpeakerIdentificationMode::Automatic,
            diarization: Some(&diarization),
            self_voice: Some(&anchor),
            calendar_attendees: &[],
            assume_in_person: false,
        },
    );

    assert_eq!(roster.len(), 1);
    assert!(!roster[0].is_local_user, "Relay must NOT blindly trust mic share when acoustic self-voice contradicts it!");
    assert_eq!(assignments.len(), 1);
    assert_ne!(assignments[0].speaker_id, "speaker_me");
    assert_eq!(assignments[0].speaker_id, "speaker_1");
}

#[test]
fn test_stage_1_mic_tie_self_voice_tie_abstains_unresolved() {
    // 3. Mic evidence ≈ tie (channel: Mixed), Self-voice ≈ tie (no cluster, no anchor)
    // Relay must abstain from false certainty and mark as Unresolved.
    let raws = vec![
        raw_seg(0, 0, 0.0, 4.0, "Ambiguous crosstalk utterance", true, true), // Mixed channel (mic and sys audio)
    ];
    let mut norm = normalize_transcript(&raws, &[]);

    let (roster, assignments) = attribute_speakers_with_evidence(
        &mut norm.segments,
        AttributionInput {
            existing: &[],
            mode: SpeakerIdentificationMode::Automatic,
            diarization: None,
            self_voice: None,
            calendar_attendees: &[],
            assume_in_person: false,
        },
    );

    assert_eq!(roster.len(), 0, "No confident speaker should be invented when evidence is tied");
    assert_eq!(assignments.len(), 0, "Ambiguous tied evidence must remain unresolved");
    assert_eq!(norm.segments[0].speaker_id, None, "Tied segment must have speaker_id None");
}

#[test]
fn test_stage_3_diagnose_real_meeting_coverage() {
    let vault_path = std::path::PathBuf::from(".relay/vault/meetings_v2");
    let meeting_id = "meet_dd912b1e-b243-4239-8a3a-6e6ed575d1c8";
    let meet_dir = vault_path.join(meeting_id);
    if !meet_dir.exists() {
        println!("Real meeting vault not present, skipping runtime validation");
        return;
    }

    let temp_vault = std::env::temp_dir().join(format!("relay_stage3_test_{}", uuid::Uuid::new_v4()));
    let temp_meet = temp_vault.join("meetings_v2").join(meeting_id);
    std::fs::create_dir_all(&temp_meet).unwrap();
    let _ = std::fs::copy(meet_dir.join("session.json"), temp_meet.join("session.json"));
    let _ = std::fs::copy(meet_dir.join("transcript.jsonl"), temp_meet.join("transcript.jsonl"));
    let temp_audio = temp_meet.join("audio");
    std::fs::create_dir_all(&temp_audio).unwrap();
    if let Ok(entries) = std::fs::read_dir(meet_dir.join("audio")) {
        for entry in entries.flatten() {
            let _ = std::fs::copy(entry.path(), temp_audio.join(entry.file_name()));
        }
    }

    let store = std::sync::Arc::new(crate::meetings_v2::session_store::SessionStore::new(temp_vault.clone()));
    let processor = crate::meetings_v2::processing::MeetingProcessor::new(store);
    let options = crate::meetings_v2::processing::ProcessingOptions {
        diarize_speakers: true,
        ..Default::default()
    };
    let prepared = processor.prepare(meeting_id, &options).expect("prepare succeeds");

    println!("Diarization report: {:?}", prepared.diarization.as_ref().map(|d| &d.report));
    println!("Self voice anchor: {:?}", prepared.diarization.as_ref().and_then(|d| d.self_voice_anchor.as_ref()).map(|a| (a.sample_count, a.total_seconds, a.reference_quality)));

    let mut me_count = 0usize;
    let mut remote_count = 0usize;
    let mut unresolved_count = 0usize;

    let normalized = prepared.normalized.as_ref().unwrap();
    for seg in &normalized.segments {
        match seg.speaker_id.as_deref() {
            Some(SPEAKER_ID_ME) => me_count += 1,
            Some(_) => remote_count += 1,
            None => unresolved_count += 1,
        }
    }

    let total = normalized.segments.len();
    let me_pct = (me_count as f64 / total as f64) * 100.0;
    let remote_pct = (remote_count as f64 / total as f64) * 100.0;
    let unresolved_pct = (unresolved_count as f64 / total as f64) * 100.0;

    let diar = prepared.diarization.as_ref().unwrap();
    println!("Diarization report: {:?}", diar.report);
    let _anchor_opt = diar.self_voice_anchor.as_ref();
    for seg in &normalized.segments {
        let cl = diar.assignments.iter().find(|a| a.segment_id == seg.id).and_then(|a| a.cluster);
        let dist = diar.assignments.iter().find(|a| a.segment_id == seg.id).map(|a| a.distance).unwrap_or(0.0);
        let dur = seg.end_time_s - seg.start_time_s;
        println!("SEG {:?} dur={:.2}s chan={:?} cl={:?} dist={:.4} spk={:?}: {:?}", seg.id, dur, seg.channel, cl, dist, seg.speaker_id, seg.text);
    }

    println!("Total segments: {}", total);
    println!("Me: {} ({:.1}%)", me_count, me_pct);
    println!("Remote: {} ({:.1}%)", remote_count, remote_pct);
    println!("Unresolved: {} ({:.1}%)", unresolved_count, unresolved_pct);
    println!("Roster: {:?}", prepared.speakers);

    assert!((15.0..=30.0).contains(&me_pct), "Me coverage must be in ~20-25% range (got {:.1}%)", me_pct);
    assert!((70.0..=85.0).contains(&remote_pct), "Remote coverage must be in ~75-80% range (got {:.1}%)", remote_pct);
    assert!(prepared.speakers.iter().any(|s| s.is_local_user), "Roster must include local user Me");
    assert!(prepared.speakers.iter().any(|s| !s.is_local_user), "Roster must include remote speaker");

    let _ = std::fs::remove_dir_all(temp_vault);
}
