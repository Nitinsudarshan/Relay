//! Speaker attribution and the speaker registry.
//!
//! Only rung 1 of the ladder in `Meeting-rules/meeting_speaker_identification.md`
//! is implemented: microphone input is the local user, system audio is everyone
//! else. It needs no model, no ONNX runtime, and no consent flow, and it
//! resolves every first-person commitment in a solo stretch — which is the
//! majority of the to-dos that matter to the person using the app.
//!
//! Two invariants hold everything else together:
//!
//! 1. **Ids are never display names.** `speaker_1` is the identifier;
//!    "Pranjali" is a label the user can change at any time. Renaming touches
//!    the registry and nothing else, so no transcript is ever rewritten.
//! 2. **Ambiguity is preserved, not resolved.** Where the channel says nothing
//!    (both sources audible in one chunk, or no channel data at all), the
//!    segment keeps `speaker_id = None`. Diarization can fill those in later
//!    without any id changing.

use super::model::{
    NormalizedSegment, SegmentChannel, Speaker, SpeakerOrigin, SPEAKER_ID_ME, SPEAKER_ID_REMOTE,
};

/// Whether speaker attribution should run at all.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpeakerIdentificationMode {
    /// Channel-based attribution (rung 1).
    Automatic,
    /// No attribution; every turn is unattributed.
    Off,
}

/// Assigns `speaker_id` on each segment from its channel, and returns the
/// registry of speakers that actually contributed.
///
/// `existing` carries user-assigned display names forward across regeneration —
/// re-running the pipeline must never silently discard a rename.
pub fn attribute_speakers(
    segments: &mut [NormalizedSegment],
    existing: &[Speaker],
    mode: SpeakerIdentificationMode,
) -> Vec<Speaker> {
    if mode == SpeakerIdentificationMode::Off {
        for segment in segments.iter_mut() {
            segment.speaker_id = None;
        }
        // Existing speakers are kept so their names survive the setting being
        // toggled off and back on.
        return existing.to_vec();
    }

    let mut me_count = 0usize;
    let mut remote_count = 0usize;

    for segment in segments.iter_mut() {
        segment.speaker_id = segment
            .channel
            .implied_speaker_id()
            .map(|id| id.to_string());

        match segment.speaker_id.as_deref() {
            Some(SPEAKER_ID_ME) => me_count += 1,
            Some(SPEAKER_ID_REMOTE) => remote_count += 1,
            _ => {}
        }
    }

    let mut speakers = Vec::new();
    if me_count > 0 {
        speakers.push(build_speaker(
            SPEAKER_ID_ME,
            "Me",
            SegmentChannel::Mic,
            true,
            me_count,
            existing,
        ));
    }
    if remote_count > 0 {
        speakers.push(build_speaker(
            SPEAKER_ID_REMOTE,
            "Speaker 1",
            SegmentChannel::System,
            false,
            remote_count,
            existing,
        ));
    }

    // A speaker the user has named but who did not speak in this run (e.g. the
    // mic was muted) is retained rather than dropped, so the name is not lost.
    for prior in existing {
        if !speakers.iter().any(|s| s.id == prior.id) {
            let mut carried = prior.clone();
            carried.segment_count = 0;
            speakers.push(carried);
        }
    }

    speakers
}

fn build_speaker(
    id: &str,
    fallback_label: &str,
    channel: SegmentChannel,
    is_local_user: bool,
    segment_count: usize,
    existing: &[Speaker],
) -> Speaker {
    let prior = existing.iter().find(|s| s.id == id);
    Speaker {
        id: id.to_string(),
        display_name: prior.and_then(|s| s.display_name.clone()),
        fallback_label: fallback_label.to_string(),
        // A manual name is a stronger claim than the channel that found the
        // speaker, so it is preserved on re-attribution.
        origin: match prior.map(|s| s.origin) {
            Some(SpeakerOrigin::Manual) => SpeakerOrigin::Manual,
            _ => SpeakerOrigin::Channel,
        },
        channel,
        is_local_user,
        segment_count,
    }
}

/// Renames a speaker in the registry.
///
/// Returns `Err` for an unknown id rather than creating a speaker who never
/// spoke. An empty or whitespace name clears the override, restoring the
/// `Speaker N` fallback.
pub fn rename_speaker(
    speakers: &mut [Speaker],
    speaker_id: &str,
    display_name: Option<&str>,
) -> Result<(), String> {
    let speaker = speakers
        .iter_mut()
        .find(|s| s.id == speaker_id)
        .ok_or_else(|| format!("Unknown speaker {}", speaker_id))?;

    match display_name.map(str::trim).filter(|n| !n.is_empty()) {
        Some(name) => {
            speaker.display_name = Some(name.to_string());
            speaker.origin = SpeakerOrigin::Manual;
        }
        None => {
            speaker.display_name = None;
            // Reverting to the fallback also reverts the claim about how this
            // speaker was identified.
            speaker.origin = match speaker.channel {
                SegmentChannel::Mic | SegmentChannel::System => SpeakerOrigin::Channel,
                _ => SpeakerOrigin::Diarization,
            };
        }
    }

    Ok(())
}

/// Resolves a speaker id to the name to display, without inventing one.
/// Unknown or absent ids render as "Unknown speaker".
pub fn resolve_label<'a>(speakers: &'a [Speaker], speaker_id: Option<&str>) -> &'a str {
    match speaker_id {
        Some(id) => speakers
            .iter()
            .find(|s| s.id == id)
            .map(|s| s.label())
            .unwrap_or("Unknown speaker"),
        None => "Unknown speaker",
    }
}

/// Matches a model-supplied owner string to a speaker.
///
/// Accepts a speaker id, a current display name, or a fallback label — case
/// insensitively. Anything else returns `None`, which is what keeps an invented
/// name out of an action item's owner field.
pub fn match_speaker<'a>(speakers: &'a [Speaker], candidate: &str) -> Option<&'a Speaker> {
    let needle = candidate.trim().to_lowercase();
    if needle.is_empty() {
        return None;
    }

    speakers.iter().find(|s| {
        s.id.to_lowercase() == needle
            || s.fallback_label.to_lowercase() == needle
            || s.display_name
                .as_deref()
                .is_some_and(|n| n.trim().to_lowercase() == needle)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::meetings_v2::processing::normalize::{normalize_transcript, RawSegmentInput};

    fn raw(chunk_index: usize, text: &str, mic: bool, sys: bool) -> RawSegmentInput {
        RawSegmentInput {
            chunk_index,
            start_time_s: chunk_index as f64 * 30.0,
            end_time_s: (chunk_index + 1) as f64 * 30.0,
            text: text.to_string(),
            mic_had_audio: mic,
            sys_had_audio: sys,
        }
    }

    fn fixture() -> Vec<NormalizedSegment> {
        let raws = vec![
            raw(0, "I will send the document tomorrow", true, false),
            raw(1, "Agreed I can take care of the deployment", false, true),
            raw(2, "Both of us were talking over each other", true, true),
        ];
        normalize_transcript(&raws, &[]).segments
    }

    #[test]
    fn channels_attribute_only_the_unambiguous_segments() {
        let mut segments = fixture();
        let speakers = attribute_speakers(&mut segments, &[], SpeakerIdentificationMode::Automatic);

        assert_eq!(segments[0].speaker_id.as_deref(), Some(SPEAKER_ID_ME));
        assert_eq!(segments[1].speaker_id.as_deref(), Some(SPEAKER_ID_REMOTE));
        assert_eq!(
            segments[2].speaker_id, None,
            "a chunk with both channels audible must not be attributed"
        );

        assert_eq!(speakers.len(), 2);
        assert!(speakers
            .iter()
            .any(|s| s.id == SPEAKER_ID_ME && s.is_local_user));
        assert_eq!(
            speakers
                .iter()
                .find(|s| s.id == SPEAKER_ID_REMOTE)
                .unwrap()
                .label(),
            "Speaker 1"
        );
    }

    #[test]
    fn unknown_speakers_keep_their_speaker_n_labels() {
        let mut segments = fixture();
        let speakers = attribute_speakers(&mut segments, &[], SpeakerIdentificationMode::Automatic);
        let remote = speakers.iter().find(|s| s.id == SPEAKER_ID_REMOTE).unwrap();
        assert_eq!(remote.display_name, None);
        assert_eq!(remote.label(), "Speaker 1");
    }

    #[test]
    fn renaming_changes_the_label_but_not_the_id() {
        let mut segments = fixture();
        let mut speakers =
            attribute_speakers(&mut segments, &[], SpeakerIdentificationMode::Automatic);

        rename_speaker(&mut speakers, SPEAKER_ID_REMOTE, Some("Pranjali")).unwrap();

        let remote = speakers.iter().find(|s| s.id == SPEAKER_ID_REMOTE).unwrap();
        assert_eq!(remote.id, SPEAKER_ID_REMOTE, "the id must never change");
        assert_eq!(remote.label(), "Pranjali");
        assert_eq!(remote.origin, SpeakerOrigin::Manual);

        // The segments still reference the id, never the name.
        assert_eq!(segments[1].speaker_id.as_deref(), Some(SPEAKER_ID_REMOTE));
    }

    #[test]
    fn a_rename_survives_re_attribution() {
        let mut segments = fixture();
        let mut speakers =
            attribute_speakers(&mut segments, &[], SpeakerIdentificationMode::Automatic);
        rename_speaker(&mut speakers, SPEAKER_ID_REMOTE, Some("Pranjali")).unwrap();

        // Regeneration must not discard the user's work.
        let mut segments2 = fixture();
        let speakers2 = attribute_speakers(
            &mut segments2,
            &speakers,
            SpeakerIdentificationMode::Automatic,
        );
        assert_eq!(
            speakers2
                .iter()
                .find(|s| s.id == SPEAKER_ID_REMOTE)
                .unwrap()
                .label(),
            "Pranjali"
        );
    }

    #[test]
    fn clearing_a_name_restores_the_fallback() {
        let mut segments = fixture();
        let mut speakers =
            attribute_speakers(&mut segments, &[], SpeakerIdentificationMode::Automatic);
        rename_speaker(&mut speakers, SPEAKER_ID_REMOTE, Some("Pranjali")).unwrap();
        rename_speaker(&mut speakers, SPEAKER_ID_REMOTE, Some("   ")).unwrap();

        let remote = speakers.iter().find(|s| s.id == SPEAKER_ID_REMOTE).unwrap();
        assert_eq!(remote.label(), "Speaker 1");
        assert_eq!(remote.origin, SpeakerOrigin::Channel);
    }

    #[test]
    fn renaming_an_unknown_speaker_is_rejected_rather_than_inventing_one() {
        let mut speakers = Vec::new();
        assert!(rename_speaker(&mut speakers, "speaker_9", Some("Ghost")).is_err());
        assert!(speakers.is_empty());
    }

    #[test]
    fn attribution_off_leaves_every_segment_unattributed() {
        let mut segments = fixture();
        attribute_speakers(&mut segments, &[], SpeakerIdentificationMode::Off);
        assert!(segments.iter().all(|s| s.speaker_id.is_none()));
    }

    #[test]
    fn owner_matching_accepts_ids_labels_and_names_but_nothing_invented() {
        let mut segments = fixture();
        let mut speakers =
            attribute_speakers(&mut segments, &[], SpeakerIdentificationMode::Automatic);
        rename_speaker(&mut speakers, SPEAKER_ID_REMOTE, Some("Pranjali")).unwrap();

        assert_eq!(
            match_speaker(&speakers, "speaker_1").unwrap().id,
            SPEAKER_ID_REMOTE
        );
        assert_eq!(
            match_speaker(&speakers, "Pranjali").unwrap().id,
            SPEAKER_ID_REMOTE
        );
        assert_eq!(
            match_speaker(&speakers, "  me  ").unwrap().id,
            SPEAKER_ID_ME
        );
        assert!(
            match_speaker(&speakers, "Someone Who Was Never Here").is_none(),
            "a name the meeting never established must not resolve to a speaker"
        );
    }

    #[test]
    fn an_unresolved_speaker_renders_honestly() {
        let speakers: Vec<Speaker> = Vec::new();
        assert_eq!(resolve_label(&speakers, None), "Unknown speaker");
        assert_eq!(
            resolve_label(&speakers, Some("speaker_4")),
            "Unknown speaker"
        );
    }
}
