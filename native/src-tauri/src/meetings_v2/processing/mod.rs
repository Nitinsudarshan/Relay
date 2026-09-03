//! The meeting processing pipeline.
//!
//! One pipeline, one canonical representation, many projections:
//!
//! ```text
//! transcript.jsonl (raw, immutable)   notes.json (raw, user-authored)
//!         ↓  normalize        deterministic, no model
//! NormalizedTranscript        the canonical human-readable transcript
//!         ↓  attribute        channel-based speaker ids
//!         ↓  converse         speaker-grouped turns
//!         ↓  context          canonical meeting context (+ notes, metadata)
//!         ↓  extract          Stage A → MeetingFacts (JSON), in passes if long
//!         ↓  summarize        Stage B → Markdown (mode + extension + budget)
//!         ↓  validate         ──► repair once with targeted feedback, then
//!                                 re-render deterministically rather than
//!                                 show prose that failed
//!    processing.json
//!         ↓
//! Summary · Conversation · Action items · Topics · Entities · Type
//!         · Related meetings · Scribble
//! ```
//!
//! Summary, action items, topics, and the Scribble are **not** separate
//! pipelines. Each is a projection of the same `MeetingFacts`, which is why two
//! of them can never disagree about what the meeting decided.
//!
//! Three properties this module is responsible for holding:
//!
//! * **The raw transcript is never written.** `SessionStore` is used read-only
//!   and derived data goes to `processing.json`. Normalization, summarization,
//!   a speaker rename, and regeneration all leave `transcript.jsonl` byte-identical.
//! * **The meeting survives failure.** Every stage records its own outcome; a
//!   failed stage leaves the ones before it intact and retryable.
//! * **Nothing here blocks recording.** The pipeline runs after finalization,
//!   holds no capture resources, and shares no state with either audio clock.

// TODO(context): migrate this pipeline onto `pipeline::analysis`.
//
// The foundation added in the 01-10 convergence (source contract, analysis
// contract, prompt registry, derived data) covers captures, files and
// scribbles. Meetings deliberately did not move in that pass: this pipeline
// has staged extraction, validation and a repair loop that the shared service
// does not model yet, and destabilising it for architectural symmetry is a bad
// trade.
//
// What it needs before it can migrate: multi-stage requests in
// `AnalysisRequest`, and prompt-registry entries for the extraction and
// summary builders (which are computed per call, not constant). What is
// already shared: the provider layer, and the heuristic-filler marker, which
// now comes from `providers::HEURISTIC_FALLBACK_MODEL` for both.


pub mod context;
pub mod conversation;
/// The summary quality evaluation set — fixtures, expectations, and a scorer.
pub mod eval;
pub mod extract;
pub mod length;
pub mod llm;
pub mod model;
pub mod modes;
pub mod normalize;
pub mod qualify;
pub mod related;
pub mod speakers;
pub mod store;
pub mod summarize;
/// Meeting action items as tasks, and the mapping that gets them onto a board.
pub mod tasks;
pub mod validate;

use super::session_store::SessionStore;
use super::types::{MeetingNotes, TranscriptSegment, TranscriptSegmentStatus};
use llm::MeetingLlm;
pub use model::MeetingProcessing;
use model::{
    ActionItemStatus, MeetingExtension, ProcessingLogEntry, ProcessingStatus, ProviderOutputStatus,
    ScribbleRef, StageState, StageStatus, SummaryArtifact, SummaryMode, SummarySource,
    ValidationReport, PROCESSING_VERSION, RULES_VERSION,
};
use context::MeetingContext;
use length::summary_budget;
use normalize::RawSegmentInput;
use related::{find_related, MeetingIndexEntry, RelatedMeeting};
use speakers::SpeakerIdentificationMode;
use std::sync::Arc;
use std::time::Instant;
use store::ProcessingStore;

/// Settings that shape one processing run. Assembled by the command layer from
/// `AppSettings` so this module never reads configuration itself.
#[derive(Debug, Clone)]
pub struct ProcessingOptions {
    /// Canonical spellings for normalization, from the user's dictionary.
    pub glossary: Vec<String>,
    pub generate_conversation: bool,
    pub speaker_identification: SpeakerIdentificationMode,
    pub summary_mode: SummaryMode,
    pub extension_id: String,
    pub user_extensions: Vec<MeetingExtension>,
    /// Standing instructions the user gave for how their summaries should read.
    /// Presentation only — see the summary contract, which subordinates them to
    /// the accuracy rules.
    pub user_instructions: Option<String>,
}

impl Default for ProcessingOptions {
    fn default() -> Self {
        Self {
            glossary: Vec::new(),
            generate_conversation: true,
            speaker_identification: SpeakerIdentificationMode::Automatic,
            summary_mode: SummaryMode::Standard,
            extension_id: modes::DEFAULT_EXTENSION_ID.to_string(),
            user_extensions: Vec::new(),
            user_instructions: None,
        }
    }
}

pub struct MeetingProcessor {
    /// Read-only. The pipeline calls no mutating method on this.
    sessions: Arc<SessionStore>,
    store: ProcessingStore,
}

impl MeetingProcessor {
    pub fn new(sessions: Arc<SessionStore>) -> Self {
        let store = ProcessingStore::new(sessions.meetings_dir().to_path_buf());
        Self { sessions, store }
    }

    pub fn get(&self, meeting_id: &str) -> Option<MeetingProcessing> {
        self.store.load(meeting_id)
    }

    pub fn log(&self, meeting_id: &str) -> Vec<ProcessingLogEntry> {
        self.store.read_log(meeting_id)
    }

    /// Runs the deterministic stages: normalize, attribute speakers, build the
    /// conversation.
    ///
    /// No model, no network, no meaningful cost — which is why this is what runs
    /// automatically once a recording is finalized, leaving the expensive stages
    /// to an explicit request. Idempotent.
    pub fn prepare(
        &self,
        meeting_id: &str,
        options: &ProcessingOptions,
    ) -> Result<MeetingProcessing, String> {
        let raw = self.read_raw_segments(meeting_id)?;
        let existing_speakers = self
            .store
            .load(meeting_id)
            .map(|p| p.speakers)
            .unwrap_or_default();

        let started = Instant::now();
        let mut normalized = normalize::normalize_transcript(&raw, &options.glossary);
        let normalize_ms = started.elapsed().as_millis() as u64;

        let speaker_started = Instant::now();
        let roster = speakers::attribute_speakers(
            &mut normalized.segments,
            &existing_speakers,
            options.speaker_identification,
        );
        let speaker_report = validate::validate_speakers(&roster);
        let speaker_ms = speaker_started.elapsed().as_millis() as u64;

        let conversation_started = Instant::now();
        let conversation = options
            .generate_conversation
            .then(|| conversation::build_conversation(&normalized.segments));
        let conversation_ms = conversation_started.elapsed().as_millis() as u64;

        let source_chars = normalized.source_char_count;
        let output_chars = normalized.output_char_count;
        let segment_count = normalized.segments.len();
        let turn_count = conversation.as_ref().map(|c| c.turns.len()).unwrap_or(0);
        let roster_len = roster.len();

        let updated = self.store.update(meeting_id, |processing| {
            processing.processing_version = PROCESSING_VERSION;
            processing.rules_version = RULES_VERSION.to_string();

            processing.stages.normalization = StageState {
                status: if segment_count > 0 {
                    StageStatus::Success
                } else {
                    StageStatus::Failed
                },
                started_at: Some(chrono::Utc::now().to_rfc3339()),
                finished_at: Some(chrono::Utc::now().to_rfc3339()),
                duration_ms: Some(normalize_ms),
                error: (segment_count == 0)
                    .then(|| "no transcribed speech to normalize".to_string()),
                provider: None,
                model: None,
                input_chars: Some(source_chars),
                output_chars: Some(output_chars),
                validation: None,
                action_diagnostics: None,
            };

            processing.stages.speakers = StageState {
                status: match options.speaker_identification {
                    SpeakerIdentificationMode::Off => StageStatus::Skipped,
                    SpeakerIdentificationMode::Automatic => StageStatus::Success,
                },
                started_at: Some(chrono::Utc::now().to_rfc3339()),
                finished_at: Some(chrono::Utc::now().to_rfc3339()),
                duration_ms: Some(speaker_ms),
                error: match options.speaker_identification {
                    SpeakerIdentificationMode::Off => {
                        Some("speaker identification is off in settings".to_string())
                    }
                    SpeakerIdentificationMode::Automatic => None,
                },
                provider: None,
                model: None,
                input_chars: None,
                output_chars: None,
                validation: Some(speaker_report.clone()),
                action_diagnostics: None,
            };

            processing.stages.conversation = match &conversation {
                Some(_) => StageState {
                    status: StageStatus::Success,
                    started_at: Some(chrono::Utc::now().to_rfc3339()),
                    finished_at: Some(chrono::Utc::now().to_rfc3339()),
                    duration_ms: Some(conversation_ms),
                    error: None,
                    provider: None,
                    model: None,
                    input_chars: Some(output_chars),
                    output_chars: None,
                    validation: None,
                    action_diagnostics: None,
                },
                None => StageState::skipped("conversation transcript is off in settings"),
            };

            processing.normalized = Some(normalized.clone());
            // Compared before the roster is replaced: existing prose is only
            // stale if the labels it was written against have actually changed.
            // Flagging it on every prepare would put a "regenerate" banner on
            // meetings where nothing moved.
            let labels_changed = {
                let before: Vec<&str> = processing
                    .speakers
                    .iter()
                    .map(|s| s.label())
                    .collect();
                let after: Vec<&str> = roster.iter().map(|s| s.label()).collect();
                before != after
            };

            processing.speakers = roster.clone();
            processing.conversation = conversation.clone();

            if labels_changed {
                if let Some(summary) = processing.summary.as_mut() {
                    summary.speaker_names_stale = true;
                }
            }
        })?;

        self.record(meeting_id, "normalization", &updated.stages.normalization);
        self.record(meeting_id, "speakers", &updated.stages.speakers);
        self.record(meeting_id, "conversation", &updated.stages.conversation);

        tracing::info!(
            meeting_id = %meeting_id,
            stage = "prepare",
            segments = segment_count,
            speakers = roster_len,
            turns = turn_count,
            duration_ms = normalize_ms + speaker_ms + conversation_ms,
            "meeting_processing: deterministic stages complete"
        );

        Ok(updated)
    }

    /// The full canonical pipeline behind "Generate Summary".
    ///
    /// Follows the required sequence: confirm the raw transcript exists, ensure a
    /// normalized transcript (creating it if absent), confirm speaker data, build
    /// the canonical representation, run the mode, apply the extension, validate,
    /// persist, and return the updated model for the UI.
    ///
    /// `force_extraction` re-runs Stage A even when usable facts already exist.
    /// Without it, changing only the mode or extension re-runs Stage B alone —
    /// the whole point of having a canonical intermediate.
    pub async fn generate_summary(
        &self,
        meeting_id: &str,
        llm: &dyn MeetingLlm,
        options: &ProcessingOptions,
        force_extraction: bool,
    ) -> Result<MeetingProcessing, String> {
        // Step 1 — the raw transcript is the precondition for everything.
        let session = self
            .sessions
            .get_session(meeting_id)
            .map_err(|e| format!("Meeting not found: {}", e))?;

        // Step 2 — a normalized transcript must exist. Re-preparing is cheap and
        // guarantees the glossary and speaker settings in force now are applied.
        let processing = self.prepare(meeting_id, options)?;

        let normalized = processing
            .normalized
            .clone()
            .filter(|n| !n.segments.is_empty())
            .ok_or_else(|| {
                "This meeting has no transcribed speech to summarize. The raw transcript and \
audio are unaffected."
                    .to_string()
            })?;

        // Step 3 — speaker data, however incomplete. An empty roster is valid.
        let roster = processing.speakers.clone();

        // Step 4 — the canonical meeting context: everything a model is told
        // about this meeting, assembled in one place.
        //
        // The user's notes are a *source* artifact and are read here, never
        // written. A meeting with no notes is the common case and produces an
        // identical pipeline — nothing branches on their absence.
        let notes = self
            .sessions
            .get_notes(meeting_id)
            .unwrap_or_else(|e| {
                tracing::warn!(
                    meeting_id = %meeting_id,
                    "meeting_processing: notes unreadable ({}); summarizing without them",
                    e
                );
                MeetingNotes::default()
            });

        let meeting_date = session
            .started_at
            .as_deref()
            .or(Some(session.created_at.as_str()))
            .and_then(|d| d.split('T').next())
            .unwrap_or("")
            .to_string();

        let context = MeetingContext {
            title: &session.title,
            date_iso: &meeting_date,
            duration_minutes: (session.duration_seconds > 0.0)
                .then(|| (session.duration_seconds / 60.0).round() as u64),
            speakers: &roster,
            segments: &normalized.segments,
            notes: &notes,
            glossary: &options.glossary,
        };

        let reuse_facts = !force_extraction
            && processing.facts.as_ref().is_some_and(|f| !f.deterministic)
            && processing.stages.extraction.status == StageStatus::Success;

        let facts = if reuse_facts {
            tracing::info!(
                meeting_id = %meeting_id,
                stage = "extraction",
                "meeting_processing: reusing existing facts; only prose is regenerated"
            );
            processing
                .facts
                .clone()
                .expect("reuse_facts implies facts are present")
        } else {
            let started = Instant::now();
            let output = extract::extract_facts(llm, &context, &session.title).await;
            let duration_ms = started.elapsed().as_millis() as u64;

            let mut facts = output.facts;
            let qualification = output.action_qualification;
            let mut action_report = validate::validate_action_items(&facts.action_items, &roster);
            let dropped = validate::drop_invalid_action_items(&mut facts.action_items, &roster);
            if !dropped.is_empty() {
                tracing::warn!(
                    meeting_id = %meeting_id,
                    stage = "extraction",
                    dropped = dropped.len(),
                    "meeting_processing: dropped action items that failed validation"
                );
                // Re-report against what survived, keeping the removals visible.
                action_report = ValidationReport::from_issues(
                    validate::validate_action_items(&facts.action_items, &roster)
                        .issues
                        .into_iter()
                        .chain(dropped)
                        .collect(),
                );
            }

            let extraction_stage = StageState {
                // Extraction succeeds whenever it produced facts. The
                // deterministic path is a degraded success, not a failure — the
                // meeting is still usable — and `llm_error` records why.
                status: StageStatus::Success,
                started_at: Some(chrono::Utc::now().to_rfc3339()),
                finished_at: Some(chrono::Utc::now().to_rfc3339()),
                duration_ms: Some(duration_ms),
                error: output.llm_error.clone(),
                provider: output.provider.clone(),
                model: output.model.clone(),
                input_chars: Some(output.input_chars),
                output_chars: serde_json::to_string(&facts).ok().map(|s| s.len()),
                validation: Some(action_report),
                action_diagnostics: Some(qualification.counts),
            };

            tracing::info!(
                meeting_id = %meeting_id,
                stage = "extraction",
                candidates = qualification.counts.candidates,
                rejected = qualification.counts.rejected,
                deduplicated = qualification.counts.deduplicated,
                capped = qualification.counts.capped,
                retained = qualification.counts.retained,
                unassigned = qualification.counts.unassigned,
                with_deadlines = qualification.counts.with_deadlines,
                owners_downgraded = qualification.counts.owners_downgraded,
                "meeting_processing: action items qualified"
            );

            self.store.update(meeting_id, |p| {
                p.facts = Some(facts.clone());
                p.stages.extraction = extraction_stage.clone();
            })?;
            self.record(meeting_id, "extraction", &extraction_stage);
            facts
        };

        // Steps 5 and 6 — mode, then extension, over the same facts.
        let extension = modes::find_extension(&options.user_extensions, &options.extension_id);
        // The size the summary is judged against is derived from this meeting,
        // not from a constant. Stage B is told it, so the model is aiming at the
        // same target the validator measures.
        let budget = summary_budget(normalized.word_count(), options.summary_mode);
        let summary_input = summarize::SummaryInput {
            facts: &facts,
            speakers: &roster,
            budget,
            extension: &extension,
            notes: &notes,
            user_instructions: options.user_instructions.as_deref(),
        };

        let started = Instant::now();
        let summary_output = summarize::generate_summary(llm, &summary_input).await;
        let transcript_text = normalized.plain_text();

        // Step 7 — validate, and act on the verdict.
        //
        // Four outcomes, kept distinct because the UI has to tell them apart:
        //
        // ```text
        // model answered ─► validate ─┬─ pass ────────────────► show the model's prose
        //                             └─ fail ─► targeted repair ─► validate ─┬─ pass ─► show it
        //                                                                     └─ fail ─┐
        //                                                                              ↓
        //                                                            deterministic render
        //                                                                     ↓
        //                                                                 validate ─┬─ pass ─► show
        //                                                                           └─ fail ─► failed
        // ```
        //
        // The repair step is what changed. Rejected prose used to go straight to
        // the deterministic renderer, so a single fixable slip — a code fence, an
        // opening "Here's the summary", forty words over — cost the user the
        // whole model-written summary and downgraded them to a fact dump. The
        // retry is *not* the same prompt again: `repair_feedback` names the rule
        // that was broken, so the second attempt has a reason to differ from the
        // first.
        //
        // "The model failed" and "the summary stage failed" remain different
        // facts. Merging a rejected draft's issues into what replaced it made
        // every rejected draft read as a failed stage, and the user was told
        // "Summary unavailable" over a perfectly good summary.
        let mut markdown = summary_output.markdown;
        let mut provider_output_status = if summary_output.deterministic {
            ProviderOutputStatus::Unavailable
        } else {
            ProviderOutputStatus::Accepted
        };
        let mut fallback_used = summary_output.deterministic;
        let mut llm_error = summary_output.llm_error;
        let mut rejected_issues: Vec<model::ValidationIssue> = Vec::new();
        let mut repair_attempted = false;

        let mut validation = validate::validate_summary(
            &markdown,
            &facts,
            &roster,
            &budget,
            &transcript_text,
            fallback_used,
        );

        if validation.has_errors() && !fallback_used {
            let codes: Vec<String> = validation
                .issues
                .iter()
                .map(|i| i.code.clone())
                .collect();

            match validate::repair_feedback(&validation, &budget) {
                Some(feedback) => {
                    repair_attempted = true;
                    tracing::info!(
                        meeting_id = %meeting_id,
                        stage = "summary",
                        issues = ?codes,
                        "meeting_processing: model prose failed validation; asking for a repair"
                    );

                    let repaired = summarize::repair_summary(llm, &summary_input, &feedback).await;
                    if repaired.deterministic {
                        // The repair call itself could not be made. Keep the
                        // first draft's diagnosis rather than the retry's.
                        rejected_issues = std::mem::take(&mut validation.issues);
                        llm_error = Some(format!(
                            "model output rejected ({}); repair unavailable: {}",
                            codes.join(", "),
                            repaired.llm_error.unwrap_or_default()
                        ));
                        provider_output_status = ProviderOutputStatus::Rejected;
                        fallback_used = true;
                        markdown = repaired.markdown;
                    } else {
                        let retry_validation = validate::validate_summary(
                            &repaired.markdown,
                            &facts,
                            &roster,
                            &budget,
                            &transcript_text,
                            false,
                        );
                        if retry_validation.has_errors() {
                            let retry_codes: Vec<&str> = retry_validation
                                .issues
                                .iter()
                                .map(|i| i.code.as_str())
                                .collect();
                            tracing::warn!(
                                meeting_id = %meeting_id,
                                stage = "summary",
                                first_issues = ?codes,
                                repair_issues = ?retry_codes,
                                "meeting_processing: repair also failed validation; rendering deterministically"
                            );
                            llm_error = Some(format!(
                                "model output rejected twice ({} then {})",
                                codes.join(", "),
                                retry_codes.join(", ")
                            ));
                            // Both drafts' findings are kept as diagnostics; only
                            // the second is what the repair actually produced.
                            rejected_issues = std::mem::take(&mut validation.issues);
                            rejected_issues.extend(retry_validation.issues);
                            provider_output_status = ProviderOutputStatus::Rejected;
                            fallback_used = true;
                            markdown =
                                summarize::render_markdown(&facts, &roster, options.summary_mode);
                        } else {
                            tracing::info!(
                                meeting_id = %meeting_id,
                                stage = "summary",
                                issues = ?codes,
                                "meeting_processing: repair accepted"
                            );
                            rejected_issues = std::mem::take(&mut validation.issues);
                            llm_error = Some(format!(
                                "first draft rejected ({}); repaired draft accepted",
                                codes.join(", ")
                            ));
                            markdown = repaired.markdown;
                            validation = retry_validation;
                        }
                    }
                }
                None => {
                    // Nothing a corrective instruction could address. Re-asking
                    // would only cost the user another wait.
                    tracing::warn!(
                        meeting_id = %meeting_id,
                        stage = "summary",
                        issues = ?codes,
                        "meeting_processing: model prose failed validation with no repairable cause"
                    );
                    llm_error = Some(format!(
                        "model output rejected by validation ({})",
                        codes.join(", ")
                    ));
                    rejected_issues = std::mem::take(&mut validation.issues);
                    provider_output_status = ProviderOutputStatus::Rejected;
                    fallback_used = true;
                    markdown = summarize::render_markdown(&facts, &roster, options.summary_mode);
                }
            }

            if fallback_used {
                validation = validate::validate_summary(
                    &markdown,
                    &facts,
                    &roster,
                    &budget,
                    &transcript_text,
                    true,
                );
            }
        }

        let duration_ms = started.elapsed().as_millis() as u64;

        // Provenance the user can act on: deterministic *presentation* of
        // model-understood facts is a different thing from deterministic
        // extraction, and neither is an AI summary.
        let source = match (fallback_used, facts.deterministic) {
            (false, _) => SummarySource::Model,
            (true, false) => SummarySource::DeterministicPresentation,
            (true, true) => SummarySource::DeterministicExtraction,
        };

        let artifact = SummaryArtifact {
            markdown,
            mode: options.summary_mode,
            extension_id: extension.id.clone(),
            generated_at: chrono::Utc::now().to_rfc3339(),
            provider: summary_output.provider.clone(),
            model: summary_output.model.clone(),
            processing_version: PROCESSING_VERSION,
            rules_version: RULES_VERSION.to_string(),
            deterministic: fallback_used,
            source,
            provider_output_status,
            fallback_used,
            rejected_issues: rejected_issues.clone(),
            repair_attempted,
            length_budget_words: Some(budget.max_words),
            speaker_names_stale: false,
            validation: validation.clone(),
        };

        // The stage's verdict is on the prose actually shown. A rejected draft
        // that the fallback replaced is recorded, not counted as a failure.
        let summary_stage = StageState {
            status: if validation.has_errors() {
                StageStatus::Failed
            } else {
                StageStatus::Success
            },
            started_at: Some(chrono::Utc::now().to_rfc3339()),
            finished_at: Some(chrono::Utc::now().to_rfc3339()),
            duration_ms: Some(duration_ms),
            error: llm_error,
            provider: Some(summary_output.provider),
            model: Some(summary_output.model),
            input_chars: Some(summary_output.input_chars),
            output_chars: Some(artifact.markdown.len()),
            validation: Some(validation),
            action_diagnostics: None,
        };

        // Step 8 — persist. Only derived state is written.
        let title = facts.title.clone();
        let updated = self.store.update(meeting_id, |p| {
            p.facts = Some(facts.clone());
            p.summary = Some(artifact.clone());
            p.stages.summary = summary_stage.clone();
        })?;

        self.record_summary(
            meeting_id,
            &summary_stage,
            provider_output_status,
            fallback_used,
        );
        tracing::info!(
            meeting_id = %meeting_id,
            stage = "summary",
            mode = options.summary_mode.label(),
            extension = %extension.id,
            provider_output = provider_output_status.label(),
            fallback_used,
            repair_attempted,
            budget_words = budget.max_words,
            transcript_words = normalized.word_count(),
            rejected_issue_codes = ?rejected_issues.iter().map(|i| i.code.as_str()).collect::<Vec<_>>(),
            title = %title,
            duration_ms,
            "meeting_processing: summary generated"
        );

        // Step 9 — the caller emits the UI event.
        Ok(updated)
    }

    /// Renames a speaker.
    ///
    /// Touches the registry only. The conversation and action items reference
    /// speaker ids and resolve names at read time, so they update immediately;
    /// existing prose still carries the old label and is marked stale rather than
    /// silently rewritten or regenerated behind the user's back.
    pub fn rename_speaker(
        &self,
        meeting_id: &str,
        speaker_id: &str,
        display_name: Option<&str>,
    ) -> Result<MeetingProcessing, String> {
        let mut rename_error = None;
        let updated =
            self.store.update(meeting_id, |processing| {
                match speakers::rename_speaker(&mut processing.speakers, speaker_id, display_name) {
                    Ok(()) => {
                        if let Some(summary) = processing.summary.as_mut() {
                            summary.speaker_names_stale = true;
                        }
                        let report = validate::validate_speakers(&processing.speakers);
                        processing.stages.speakers.validation = Some(report);
                    }
                    Err(e) => rename_error = Some(e),
                }
            })?;

        if let Some(e) = rename_error {
            return Err(e);
        }

        tracing::info!(
            meeting_id = %meeting_id,
            speaker_id = %speaker_id,
            named = display_name.is_some(),
            "meeting_processing: speaker renamed"
        );
        Ok(updated)
    }

    /// Persists an action item's checked state.
    ///
    /// Action items are first-class objects, so ticking one off is durable rather
    /// than component state lost on unmount.
    pub fn set_action_item_status(
        &self,
        meeting_id: &str,
        action_item_id: &str,
        status: ActionItemStatus,
    ) -> Result<MeetingProcessing, String> {
        let mut found = false;
        let updated = self.store.update(meeting_id, |processing| {
            if let Some(facts) = processing.facts.as_mut() {
                if let Some(item) = facts
                    .action_items
                    .iter_mut()
                    .find(|i| i.id == action_item_id)
                {
                    item.status = status;
                    found = true;
                }
            }
        })?;

        if !found {
            return Err(format!("Unknown action item {}", action_item_id));
        }
        Ok(updated)
    }

    /// Records that an action item has been pushed onto a Kanban board.
    ///
    /// Kept on the action item rather than in a separate ledger, so "already a
    /// task" travels with the to-do it describes and survives a regeneration
    /// that reuses existing facts.
    pub fn record_action_item_task(
        &self,
        meeting_id: &str,
        action_item_id: &str,
        kanban_card_id: &str,
    ) -> Result<MeetingProcessing, String> {
        let mut found = false;
        let updated = self.store.update(meeting_id, |processing| {
            if let Some(facts) = processing.facts.as_mut() {
                if let Some(item) = facts
                    .action_items
                    .iter_mut()
                    .find(|i| i.id == action_item_id)
                {
                    item.kanban_card_id = Some(kanban_card_id.to_string());
                    found = true;
                }
            }
        })?;

        if !found {
            return Err(format!("Unknown action item {}", action_item_id));
        }
        Ok(updated)
    }

    /// Records that this meeting has been exported as a Scribble.
    pub fn record_scribble(
        &self,
        meeting_id: &str,
        scribble: ScribbleRef,
    ) -> Result<MeetingProcessing, String> {
        self.store.update(meeting_id, |processing| {
            processing.scribble_ref = Some(scribble.clone());
        })
    }

    /// Finds meetings related to this one, from extracted metadata.
    pub fn related(&self, meeting_id: &str, limit: usize) -> Result<Vec<RelatedMeeting>, String> {
        let subject = self
            .index_entry(meeting_id)
            .ok_or_else(|| "This meeting has not been processed yet".to_string())?;

        let candidates: Vec<MeetingIndexEntry> = self
            .store
            .list_processed_ids()
            .into_iter()
            .filter(|id| id != meeting_id)
            .filter_map(|id| self.index_entry(&id))
            .collect();

        Ok(find_related(&subject, &candidates, limit))
    }

    fn index_entry(&self, meeting_id: &str) -> Option<MeetingIndexEntry> {
        let processing = self.store.load(meeting_id)?;
        let facts = processing.facts.as_ref()?;
        let session = self.sessions.get_session(meeting_id).ok()?;
        let labels = processing
            .speakers
            .iter()
            // Only named speakers are useful across meetings: "Speaker 1" in two
            // different meetings is not the same person.
            .filter(|s| s.display_name.is_some() || s.is_local_user)
            .map(|s| s.label().to_string())
            .collect();

        Some(MeetingIndexEntry::from_facts(
            meeting_id,
            &facts.title,
            &session.created_at,
            facts,
            labels,
        ))
    }

    /// Reads the raw transcript. The only source-artifact access the pipeline
    /// makes, and it is read-only.
    fn read_raw_segments(&self, meeting_id: &str) -> Result<Vec<RawSegmentInput>, String> {
        let segments = self
            .sessions
            .get_transcript_segments(meeting_id)
            .map_err(|e| format!("Failed to read the raw transcript: {}", e))?;

        Ok(segments
            .into_iter()
            .filter(|s| s.status == TranscriptSegmentStatus::Success)
            .filter(|s| !s.text.trim().is_empty())
            .flat_map(raw_inputs_from_segment)
            .collect())
    }

    /// Writes one stage record to the processing log.
    ///
    /// Sizes, counts and outcomes only — never transcript content, and never a
    /// candidate's text. That guarantee is what lets this log be read freely
    /// while diagnosing a meeting nobody is allowed to read.
    fn record(&self, meeting_id: &str, stage: &str, state: &StageState) {
        let validation = state.validation.as_ref();
        self.store.append_log(&ProcessingLogEntry {
            meeting_id: meeting_id.to_string(),
            stage: stage.to_string(),
            status: format!("{:?}", state.status).to_lowercase(),
            at: chrono::Utc::now().to_rfc3339(),
            duration_ms: state.duration_ms,
            provider: state.provider.clone(),
            model: state.model.clone(),
            input_chars: state.input_chars,
            output_chars: state.output_chars,
            validator_passed: validation.map(|v| v.passed),
            validator_issue_codes: validation
                .map(|v| v.issues.iter().map(|i| i.code.clone()).collect())
                .unwrap_or_default(),
            error: state.error.clone(),
            action_diagnostics: state.action_diagnostics,
            provider_output_status: None,
            fallback_used: None,
            processing_version: PROCESSING_VERSION,
            rules_version: RULES_VERSION.to_string(),
        });
    }

    /// Writes the summary stage's record, including what became of the model's
    /// draft. A separate entry point because that distinction exists only here.
    fn record_summary(
        &self,
        meeting_id: &str,
        state: &StageState,
        provider_output_status: ProviderOutputStatus,
        fallback_used: bool,
    ) {
        let validation = state.validation.as_ref();
        self.store.append_log(&ProcessingLogEntry {
            meeting_id: meeting_id.to_string(),
            stage: "summary".to_string(),
            status: format!("{:?}", state.status).to_lowercase(),
            at: chrono::Utc::now().to_rfc3339(),
            duration_ms: state.duration_ms,
            provider: state.provider.clone(),
            model: state.model.clone(),
            input_chars: state.input_chars,
            output_chars: state.output_chars,
            validator_passed: validation.map(|v| v.passed),
            validator_issue_codes: validation
                .map(|v| v.issues.iter().map(|i| i.code.clone()).collect())
                .unwrap_or_default(),
            error: state.error.clone(),
            action_diagnostics: None,
            provider_output_status: Some(provider_output_status.label().to_string()),
            fallback_used: Some(fallback_used),
            processing_version: PROCESSING_VERSION,
            rules_version: RULES_VERSION.to_string(),
        });
    }
}

/// Renders a processed meeting as Markdown for a Scribble.
///
/// Composed from the same derived artifacts the UI shows, so the exported
/// Scribble cannot say something the meeting view does not.
pub fn render_scribble_markdown(
    processing: &MeetingProcessing,
    meeting_title: &str,
    include_conversation: bool,
) -> String {
    let mut out = String::new();

    if let Some(facts) = processing.facts.as_ref() {
        out.push_str(&format!("# {}\n\n", facts.title));
        out.push_str(&format!(
            "**Meeting type:** {}\n",
            facts.meeting_type.label()
        ));
        if !processing.speakers.is_empty() {
            let labels: Vec<&str> = processing.speakers.iter().map(|s| s.label()).collect();
            out.push_str(&format!("**Participants:** {}\n", labels.join(", ")));
        }
        out.push('\n');
    } else {
        out.push_str(&format!("# {}\n\n", meeting_title));
    }

    match processing.summary.as_ref() {
        Some(summary) => {
            out.push_str(&summary.markdown);
            out.push('\n');
        }
        None => out.push_str("_No summary has been generated for this meeting yet._\n"),
    }

    if include_conversation {
        if let Some(conv) = processing.conversation.as_ref() {
            if !conv.turns.is_empty() {
                out.push_str("\n## Conversation\n\n");
                out.push_str(&conversation::render_conversation_markdown(
                    conv,
                    &processing.speakers,
                ));
                out.push('\n');
            }
        }
    }

    out.trim_end().to_string()
}

/// A short, user-facing description of where processing stands.
///
/// Deliberately does not expose every internal stage: the user needs to know
/// whether the meeting is usable and what to retry, not the pipeline's topology.
pub fn processing_headline(processing: Option<&MeetingProcessing>) -> &'static str {
    match processing.map(|p| p.status) {
        None | Some(ProcessingStatus::NotStarted) => "Not processed yet",
        Some(ProcessingStatus::Running) => "Processing meeting…",
        Some(ProcessingStatus::Ready) => "Ready",
        Some(ProcessingStatus::Partial) => "Partly processed",
        Some(ProcessingStatus::Failed) => "Processing failed",
    }
}

#[cfg(test)]
mod tests;

/// Fans one raw transcript record out into the inputs the normalizer sees.
///
/// A chunk that carries utterances becomes one input per utterance, each with
/// the channel measured over that utterance's own span. That is the whole point
/// of the v2.5 capture change: at chunk granularity a two-way conversation
/// resolves to `Mixed` almost everywhere, and `Mixed` means no speaker.
///
/// A chunk with no utterances — recorded before v2.5, or one Whisper returned no
/// timed spans for — becomes a single whole-chunk input, exactly as before.
fn raw_inputs_from_segment(segment: TranscriptSegment) -> Vec<RawSegmentInput> {
    if segment.utterances.is_empty() {
        return vec![RawSegmentInput {
            chunk_index: segment.chunk_index,
            utterance_index: None,
            start_time_s: segment.start_time_s,
            end_time_s: segment.end_time_s,
            text: segment.text,
            mic_had_audio: segment.mic_had_audio,
            sys_had_audio: segment.sys_had_audio,
        }];
    }

    segment
        .utterances
        .into_iter()
        .filter(|u| !u.text.trim().is_empty())
        .map(|u| RawSegmentInput {
            chunk_index: segment.chunk_index,
            utterance_index: Some(u.index),
            start_time_s: u.start_time_s,
            end_time_s: u.end_time_s,
            text: u.text,
            mic_had_audio: u.mic_had_audio,
            sys_had_audio: u.sys_had_audio,
        })
        .collect()
}
