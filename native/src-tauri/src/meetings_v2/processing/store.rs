//! Persistence for derived meeting data.
//!
//! Writes exactly two files, both siblings of the recorder's artifacts and
//! neither of them read by the recorder:
//!
//! * `processing.json` — the current derived model, replaced atomically.
//! * `processing_log.jsonl` — append-only, one line per stage run.
//!
//! Nothing here opens `session.json`, `transcript.jsonl`, or `audio/` for
//! writing. That is the mechanical guarantee behind raw immutability: the
//! pipeline has no code path that could modify a source artifact, so no future
//! change to it can accidentally acquire one.

use super::model::{MeetingProcessing, ProcessingLogEntry};
use std::fs::{self, File, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

/// How many log lines `read_log` will return, newest last. A long-running
/// meeting that is regenerated repeatedly should not hand the UI an unbounded
/// list.
const MAX_LOG_ENTRIES_RETURNED: usize = 200;

pub struct ProcessingStore {
    /// The `meetings_v2` directory — the same root `SessionStore` uses, so a
    /// meeting's derived data sits beside its source data and is removed with it.
    base_dir: PathBuf,
    /// Serializes read-modify-write cycles on `processing.json`. Regeneration
    /// and a speaker rename can race; without this, whichever finishes last
    /// discards the other's work.
    write_lock: Mutex<()>,
}

impl ProcessingStore {
    pub fn new(meetings_dir: PathBuf) -> Self {
        Self {
            base_dir: meetings_dir,
            write_lock: Mutex::new(()),
        }
    }

    fn session_dir(&self, meeting_id: &str) -> PathBuf {
        self.base_dir.join(meeting_id)
    }

    pub fn processing_path(&self, meeting_id: &str) -> PathBuf {
        self.session_dir(meeting_id).join("processing.json")
    }

    pub fn log_path(&self, meeting_id: &str) -> PathBuf {
        self.session_dir(meeting_id).join("processing_log.jsonl")
    }

    /// Loads the derived model, or `None` if this meeting has never been
    /// processed.
    ///
    /// Unreadable or stale-version content is reported as absent rather than as
    /// an error: derived data is always recomputable, and a meeting must remain
    /// openable regardless of what its `processing.json` contains.
    pub fn load(&self, meeting_id: &str) -> Option<MeetingProcessing> {
        let path = self.processing_path(meeting_id);
        if !path.exists() {
            return None;
        }

        let content = match fs::read_to_string(&path) {
            Ok(content) => content,
            Err(e) => {
                tracing::warn!(
                    meeting_id = %meeting_id,
                    "meeting_processing: processing.json unreadable ({}); treating as unprocessed",
                    e
                );
                return None;
            }
        };

        match serde_json::from_str::<MeetingProcessing>(&content) {
            Ok(processing) => Some(processing),
            Err(e) => {
                tracing::warn!(
                    meeting_id = %meeting_id,
                    "meeting_processing: processing.json could not be deserialized ({}); treating as unprocessed",
                    e
                );
                None
            }
        }
    }

    /// Replaces the derived model.
    ///
    /// Committed by rename, so an interrupted write cannot leave truncated JSON
    /// that would make an otherwise-fine meeting look unprocessed.
    pub fn save(&self, processing: &MeetingProcessing) -> Result<(), String> {
        let _guard = self.write_lock.lock().map_err(|e| e.to_string())?;
        self.write_unlocked(processing)
    }

    /// Applies `mutate` to the stored model under the write lock and persists the
    /// result.
    ///
    /// Every mutation must go through this rather than saving a copy read
    /// earlier — a stale copy would roll back a concurrent rename or
    /// regeneration.
    pub fn update<F>(&self, meeting_id: &str, mutate: F) -> Result<MeetingProcessing, String>
    where
        F: FnOnce(&mut MeetingProcessing),
    {
        let _guard = self.write_lock.lock().map_err(|e| e.to_string())?;
        let mut processing = self
            .load_unlocked(meeting_id)
            .unwrap_or_else(|| MeetingProcessing::new(meeting_id));
        mutate(&mut processing);
        processing.updated_at = chrono::Utc::now().to_rfc3339();
        processing.recompute_status();
        self.write_unlocked(&processing)?;
        Ok(processing)
    }

    fn load_unlocked(&self, meeting_id: &str) -> Option<MeetingProcessing> {
        let path = self.processing_path(meeting_id);
        let content = fs::read_to_string(path).ok()?;
        serde_json::from_str(&content).ok()
    }

    fn write_unlocked(&self, processing: &MeetingProcessing) -> Result<(), String> {
        let dir = self.session_dir(&processing.meeting_id);
        fs::create_dir_all(&dir)
            .map_err(|e| format!("Failed to create meeting directory: {}", e))?;

        let content = serde_json::to_string_pretty(processing)
            .map_err(|e| format!("Failed to serialize processing.json: {}", e))?;

        let final_path = dir.join("processing.json");
        let tmp_path = dir.join("processing.json.tmp");
        fs::write(&tmp_path, content)
            .map_err(|e| format!("Failed to write processing.json.tmp: {}", e))?;
        fs::rename(&tmp_path, &final_path)
            .map_err(|e| format!("Failed to commit processing.json: {}", e))?;
        Ok(())
    }

    /// Appends one stage record to the meeting's processing log.
    ///
    /// Logging is best-effort: a failure to write the log must never fail the
    /// stage it is describing.
    pub fn append_log(&self, entry: &ProcessingLogEntry) {
        let dir = self.session_dir(&entry.meeting_id);
        if let Err(e) = fs::create_dir_all(&dir) {
            tracing::warn!("meeting_processing: cannot create log directory: {}", e);
            return;
        }

        let line = match serde_json::to_string(entry) {
            Ok(line) => line,
            Err(e) => {
                tracing::warn!("meeting_processing: cannot serialize log entry: {}", e);
                return;
            }
        };

        let result = OpenOptions::new()
            .create(true)
            .append(true)
            .open(dir.join("processing_log.jsonl"))
            .and_then(|mut file| writeln!(file, "{}", line));

        if let Err(e) = result {
            tracing::warn!("meeting_processing: cannot append to processing log: {}", e);
        }
    }

    /// Reads the processing log, oldest first, capped at the most recent
    /// `MAX_LOG_ENTRIES_RETURNED` entries.
    pub fn read_log(&self, meeting_id: &str) -> Vec<ProcessingLogEntry> {
        let path = self.log_path(meeting_id);
        if !path.exists() {
            return Vec::new();
        }

        let Ok(file) = File::open(&path) else {
            return Vec::new();
        };

        let mut entries: Vec<ProcessingLogEntry> = BufReader::new(file)
            .lines()
            .map_while(Result::ok)
            .filter(|line| !line.trim().is_empty())
            .filter_map(|line| serde_json::from_str(&line).ok())
            .collect();

        if entries.len() > MAX_LOG_ENTRIES_RETURNED {
            entries.drain(..entries.len() - MAX_LOG_ENTRIES_RETURNED);
        }
        entries
    }

    /// Lists the meeting ids that have derived data, for the related-meetings
    /// search.
    pub fn list_processed_ids(&self) -> Vec<String> {
        let Ok(entries) = fs::read_dir(&self.base_dir) else {
            return Vec::new();
        };

        entries
            .filter_map(Result::ok)
            .filter(|entry| entry.path().is_dir())
            .filter(|entry| entry.path().join("processing.json").exists())
            .filter_map(|entry| entry.file_name().to_str().map(str::to_string))
            .collect()
    }

    pub fn meetings_dir(&self) -> &Path {
        &self.base_dir
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::meetings_v2::processing::model::{
        ProcessingStatus, StageStatus, PROCESSING_VERSION, RULES_VERSION,
    };
    use std::sync::Arc;

    fn temp_store() -> (ProcessingStore, PathBuf) {
        let dir =
            std::env::temp_dir().join(format!("relay_test_proc_store_{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&dir).unwrap();
        (ProcessingStore::new(dir.clone()), dir)
    }

    #[test]
    fn an_unprocessed_meeting_loads_as_none() {
        let (store, dir) = temp_store();
        assert!(store.load("meet_missing").is_none());
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn saving_and_loading_round_trips() {
        let (store, dir) = temp_store();

        let mut processing = MeetingProcessing::new("meet_1");
        processing.stages.normalization.status = StageStatus::Success;
        processing.recompute_status();
        store.save(&processing).unwrap();

        let loaded = store.load("meet_1").unwrap();
        assert_eq!(loaded.meeting_id, "meet_1");
        assert_eq!(loaded.status, ProcessingStatus::Ready);
        assert_eq!(loaded.processing_version, PROCESSING_VERSION);

        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn a_committed_write_leaves_no_temporary_file() {
        let (store, dir) = temp_store();
        store.save(&MeetingProcessing::new("meet_1")).unwrap();

        let session_dir = dir.join("meet_1");
        assert!(session_dir.join("processing.json").exists());
        assert!(
            !session_dir.join("processing.json.tmp").exists(),
            "the temporary file must not survive a committed write"
        );

        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn corrupt_derived_data_reads_as_unprocessed_rather_than_erroring() {
        // Derived data is always recomputable, so a damaged processing.json must
        // never make a meeting unopenable.
        let (store, dir) = temp_store();
        let session_dir = dir.join("meet_1");
        fs::create_dir_all(&session_dir).unwrap();
        fs::write(session_dir.join("processing.json"), "{ this is not json").unwrap();

        assert!(store.load("meet_1").is_none());

        // And it can be overwritten cleanly.
        store.save(&MeetingProcessing::new("meet_1")).unwrap();
        assert!(store.load("meet_1").is_some());

        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn update_recomputes_status_and_stamps_the_time() {
        let (store, dir) = temp_store();
        store.save(&MeetingProcessing::new("meet_1")).unwrap();

        let updated = store
            .update("meet_1", |p| {
                p.stages.normalization.status = StageStatus::Success;
                p.stages.summary.status = StageStatus::Failed;
            })
            .unwrap();

        assert_eq!(updated.status, ProcessingStatus::Partial);
        assert!(!updated.updated_at.is_empty());

        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn update_on_a_never_processed_meeting_creates_the_record() {
        let (store, dir) = temp_store();
        let created = store
            .update("meet_new", |p| {
                p.stages.normalization.status = StageStatus::Success
            })
            .unwrap();
        assert_eq!(created.meeting_id, "meet_new");
        assert_eq!(created.status, ProcessingStatus::Ready);
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn concurrent_updates_do_not_clobber_each_other() {
        let (store, dir) = temp_store();
        let store = Arc::new(store);
        store.save(&MeetingProcessing::new("meet_1")).unwrap();

        let mut handles = Vec::new();
        for _ in 0..4 {
            let store = store.clone();
            handles.push(std::thread::spawn(move || {
                for _ in 0..25 {
                    store
                        .update("meet_1", |p| {
                            p.speakers
                                .push(crate::meetings_v2::processing::model::Speaker {
                                id: format!("speaker_{}", p.speakers.len()),
                                display_name: None,
                                fallback_label: "Speaker".into(),
                                origin:
                                    crate::meetings_v2::processing::model::SpeakerOrigin::Channel,
                                channel:
                                    crate::meetings_v2::processing::model::SegmentChannel::Unknown,
                                is_local_user: false,
                                segment_count: 0,
                            });
                        })
                        .unwrap();
                }
            }));
        }
        for handle in handles {
            handle.join().unwrap();
        }

        assert_eq!(store.load("meet_1").unwrap().speakers.len(), 100);
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn the_log_is_append_only_and_readable() {
        let (store, dir) = temp_store();

        for stage in ["normalization", "extraction", "summary"] {
            store.append_log(&ProcessingLogEntry {
                meeting_id: "meet_1".into(),
                stage: stage.into(),
                status: "success".into(),
                at: chrono::Utc::now().to_rfc3339(),
                duration_ms: Some(12),
                provider: Some("scripted".into()),
                model: Some("scripted-model".into()),
                input_chars: Some(100),
                output_chars: Some(50),
                validator_passed: Some(true),
                validator_issue_codes: Vec::new(),
                error: None,
                action_diagnostics: None,
                provider_output_status: None,
                fallback_used: None,
                processing_version: PROCESSING_VERSION,
                rules_version: RULES_VERSION.into(),
            });
        }

        let entries = store.read_log("meet_1");
        assert_eq!(entries.len(), 3);
        assert_eq!(entries[0].stage, "normalization");
        assert_eq!(entries[2].stage, "summary");

        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn the_log_never_records_transcript_text() {
        // The entry type has no text field at all, which is the enforcement.
        // This test asserts the serialized shape stays that way.
        let entry = ProcessingLogEntry {
            meeting_id: "meet_1".into(),
            stage: "normalization".into(),
            status: "success".into(),
            at: chrono::Utc::now().to_rfc3339(),
            duration_ms: Some(1),
            provider: None,
            model: None,
            input_chars: Some(4096),
            output_chars: Some(2048),
            validator_passed: None,
            validator_issue_codes: Vec::new(),
            error: None,
            // The action-item gate records what it did per candidate, including
            // the candidate's own words. Only the counts may be logged.
            action_diagnostics: Some(crate::meetings_v2::processing::qualify::ActionDiagnostics {
                candidates: 12,
                rejected: 9,
                deduplicated: 1,
                capped: 0,
                retained: 2,
                unassigned: 1,
                with_deadlines: 1,
                owners_downgraded: 1,
            }),
            provider_output_status: Some("rejected".into()),
            fallback_used: Some(true),
            processing_version: PROCESSING_VERSION,
            rules_version: RULES_VERSION.into(),
        };
        let json = serde_json::to_string(&entry).unwrap();
        assert!(json.contains("input_chars"));
        assert!(json.contains("\"candidates\":12"), "counts are loggable");
        assert!(!json.contains("text"));
        assert!(!json.contains("transcript"));
        assert!(
            !json.contains("candidate_text"),
            "a candidate's own words must never reach the log"
        );
    }

    #[test]
    fn only_processed_meetings_are_listed() {
        let (store, dir) = temp_store();
        fs::create_dir_all(dir.join("meet_unprocessed")).unwrap();
        store
            .save(&MeetingProcessing::new("meet_processed"))
            .unwrap();

        let ids = store.list_processed_ids();
        assert_eq!(ids, vec!["meet_processed".to_string()]);

        let _ = fs::remove_dir_all(dir);
    }
}
