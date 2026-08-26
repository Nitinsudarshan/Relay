use super::types::{MeetingSession, MeetingState, TranscriptSegment, TranscriptSegmentStatus};
use hound::{WavReader, WavSpec, WavWriter};
use std::fs::{self, File, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

pub struct SessionStore {
    base_dir: PathBuf,
    /// Serializes read-modify-write cycles on `session.json`. The capture
    /// worker and the engine both mutate session metadata concurrently; without
    /// this, whichever writes last silently discards the other's counters.
    write_lock: Mutex<()>,
}

impl SessionStore {
    pub fn new(vault_dir: PathBuf) -> Self {
        let base_dir = vault_dir.join("meetings_v2");
        Self {
            base_dir,
            write_lock: Mutex::new(()),
        }
    }

    pub fn meetings_dir(&self) -> &Path {
        &self.base_dir
    }

    pub fn session_dir(&self, session_id: &str) -> PathBuf {
        self.base_dir.join(session_id)
    }

    pub fn audio_dir(&self, session_id: &str) -> PathBuf {
        self.session_dir(session_id).join("audio")
    }

    pub fn init_session(&self, session: &MeetingSession) -> Result<(), String> {
        let audio_dir = self.audio_dir(&session.id);
        fs::create_dir_all(&audio_dir)
            .map_err(|e| format!("Failed to create session audio dir: {}", e))?;

        self.save_session(session)?;
        Ok(())
    }

    pub fn save_session(&self, session: &MeetingSession) -> Result<(), String> {
        let _guard = self.write_lock.lock().map_err(|e| e.to_string())?;
        self.write_session_unlocked(session)
    }

    /// Applies `mutate` to the on-disk session under the store's write lock and
    /// persists the result, returning the updated session.
    ///
    /// Every mutation of live session metadata must go through this rather than
    /// saving a `MeetingSession` captured earlier: a stale copy would roll back
    /// whatever another thread has written since it was cloned.
    pub fn update_session<F>(&self, session_id: &str, mutate: F) -> Result<MeetingSession, String>
    where
        F: FnOnce(&mut MeetingSession),
    {
        let _guard = self.write_lock.lock().map_err(|e| e.to_string())?;
        let mut session = self.read_session_raw(session_id)?;
        mutate(&mut session);
        session.updated_at = chrono::Utc::now().to_rfc3339();
        self.write_session_unlocked(&session)?;
        Ok(session)
    }

    fn write_session_unlocked(&self, session: &MeetingSession) -> Result<(), String> {
        let dir = self.session_dir(&session.id);
        fs::create_dir_all(&dir)
            .map_err(|e| format!("Failed to create session dir: {}", e))?;

        let content = serde_json::to_string_pretty(session)
            .map_err(|e| format!("Failed to serialize session metadata: {}", e))?;

        // Write-then-rename: a crash mid-write must never leave a truncated
        // session.json, which would make the meeting unreadable (and so
        // invisible in the UI) even though its audio is intact on disk.
        let meta_path = dir.join("session.json");
        let tmp_path = dir.join("session.json.tmp");
        fs::write(&tmp_path, content)
            .map_err(|e| format!("Failed to write session.json.tmp: {}", e))?;
        fs::rename(&tmp_path, &meta_path)
            .map_err(|e| format!("Failed to commit session.json: {}", e))?;
        Ok(())
    }

    fn read_session_raw(&self, session_id: &str) -> Result<MeetingSession, String> {
        let meta_path = self.session_dir(session_id).join("session.json");
        let content = fs::read_to_string(&meta_path)
            .map_err(|e| format!("Failed to read session.json: {}", e))?;
        serde_json::from_str(&content)
            .map_err(|e| format!("Failed to deserialize session.json: {}", e))
    }

    pub fn get_session(&self, session_id: &str) -> Result<MeetingSession, String> {
        let meta_path = self.session_dir(session_id).join("session.json");
        if !meta_path.exists() {
            return Err(format!("Session {} not found", session_id));
        }

        let content = fs::read_to_string(&meta_path)
            .map_err(|e| format!("Failed to read session.json: {}", e))?;
        let mut session: MeetingSession = serde_json::from_str(&content)
            .map_err(|e| format!("Failed to deserialize session.json: {}", e))?;

        if session.chunk_count == 0 {
            if let Ok(chunks) = self.list_chunk_files(&session.id) {
                session.chunk_count = chunks.len();
            }
        }
        if session.word_count == 0 {
            if let Ok(text) = self.get_full_transcript_text(&session.id) {
                session.word_count = text.split_whitespace().count();
            }
        }
        Ok(session)
    }

    pub fn list_sessions(&self) -> Result<Vec<MeetingSession>, String> {
        if !self.base_dir.exists() {
            return Ok(Vec::new());
        }

        let mut sessions = Vec::new();
        for entry in fs::read_dir(&self.base_dir).map_err(|e| e.to_string())? {
            let entry = entry.map_err(|e| e.to_string())?;
            let path = entry.path();
            if path.is_dir() {
                let meta_file = path.join("session.json");
                if meta_file.exists() {
                    if let Ok(content) = fs::read_to_string(&meta_file) {
                        if let Ok(mut session) = serde_json::from_str::<MeetingSession>(&content) {
                            if session.chunk_count == 0 {
                                if let Ok(chunks) = self.list_chunk_files(&session.id) {
                                    session.chunk_count = chunks.len();
                                }
                            }
                            if session.word_count == 0 {
                                if let Ok(text) = self.get_full_transcript_text(&session.id) {
                                    session.word_count = text.split_whitespace().count();
                                }
                            }
                            sessions.push(session);
                        }
                    }
                }
            }
        }

        sessions.sort_by(|a, b| b.created_at.cmp(&a.created_at));
        Ok(sessions)
    }

    pub fn delete_session(&self, session_id: &str) -> Result<(), String> {
        let dir = self.session_dir(session_id);
        if dir.exists() {
            fs::remove_dir_all(&dir)
                .map_err(|e| format!("Failed to delete meeting session {}: {}", session_id, e))?;
        }
        Ok(())
    }

    pub fn chunk_path(&self, session_id: &str, chunk_index: usize) -> PathBuf {
        self.audio_dir(session_id)
            .join(format!("chunk_{:05}.wav", chunk_index))
    }

    pub fn write_chunk_wav(
        &self,
        session_id: &str,
        chunk_index: usize,
        samples: &[f32],
        sample_rate: u32,
    ) -> Result<PathBuf, String> {
        let path = self.chunk_path(session_id, chunk_index);
        let spec = WavSpec {
            channels: 1,
            sample_rate,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        };

        let mut writer = WavWriter::create(&path, spec)
            .map_err(|e| format!("Failed to create chunk wav: {}", e))?;

        for &sample in samples {
            let sample_clamped = sample.clamp(-1.0, 1.0);
            let sample_i16 = (sample_clamped * i16::MAX as f32) as i16;
            writer
                .write_sample(sample_i16)
                .map_err(|e| format!("Failed to write chunk sample: {}", e))?;
        }

        writer
            .finalize()
            .map_err(|e| format!("Failed to finalize chunk wav: {}", e))?;

        Ok(path)
    }

    pub fn append_transcript_segment(
        &self,
        session_id: &str,
        segment: &TranscriptSegment,
    ) -> Result<(), String> {
        let path = self.session_dir(session_id).join("transcript.jsonl");
        let line = serde_json::to_string(segment)
            .map_err(|e| format!("Failed to serialize transcript segment: {}", e))?;

        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
            .map_err(|e| format!("Failed to open transcript.jsonl: {}", e))?;

        writeln!(file, "{}", line)
            .map_err(|e| format!("Failed to append to transcript.jsonl: {}", e))?;
        Ok(())
    }

    pub fn get_transcript_segments(
        &self,
        session_id: &str,
    ) -> Result<Vec<TranscriptSegment>, String> {
        let path = self.session_dir(session_id).join("transcript.jsonl");
        if !path.exists() {
            return Ok(Vec::new());
        }

        let file = File::open(&path)
            .map_err(|e| format!("Failed to open transcript.jsonl: {}", e))?;
        let reader = BufReader::new(file);
        let mut segments = Vec::new();

        for line in reader.lines() {
            let line = line.map_err(|e| format!("Failed to read line from transcript: {}", e))?;
            if line.trim().is_empty() {
                continue;
            }
            if let Ok(segment) = serde_json::from_str::<TranscriptSegment>(&line) {
                segments.push(segment);
            }
        }

        segments.sort_by_key(|s| s.chunk_index);
        Ok(segments)
    }

    pub fn get_full_transcript_text(&self, session_id: &str) -> Result<String, String> {
        let segments = self.get_transcript_segments(session_id)?;
        let mut full_text = String::new();
        for segment in segments {
            if segment.status == TranscriptSegmentStatus::Success && !segment.text.trim().is_empty() {
                if !full_text.is_empty() {
                    full_text.push(' ');
                }
                full_text.push_str(segment.text.trim());
            }
        }
        Ok(full_text)
    }

    pub fn list_chunk_files(&self, session_id: &str) -> Result<Vec<PathBuf>, String> {
        let audio_dir = self.audio_dir(session_id);
        if !audio_dir.exists() {
            return Ok(Vec::new());
        }

        let mut chunks = Vec::new();
        for entry in fs::read_dir(&audio_dir).map_err(|e| e.to_string())? {
            let entry = entry.map_err(|e| e.to_string())?;
            let path = entry.path();
            if path.extension().is_some_and(|ext| ext == "wav") {
                let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
                if name.starts_with("chunk_") {
                    chunks.push(path);
                }
            }
        }

        chunks.sort();
        Ok(chunks)
    }

    pub fn merge_chunks_to_full_audio(&self, session_id: &str) -> Result<Option<PathBuf>, String> {
        let chunks = self.list_chunk_files(session_id)?;
        if chunks.is_empty() {
            return Ok(None);
        }

        let full_audio_path = self.session_dir(session_id).join("audio_full.wav");
        let spec = WavSpec {
            channels: 1,
            sample_rate: 16_000,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        };

        let mut writer = WavWriter::create(&full_audio_path, spec)
            .map_err(|e| format!("Failed to create audio_full.wav: {}", e))?;

        for chunk_path in chunks {
            if let Ok(mut reader) = WavReader::open(&chunk_path) {
                for sample in reader.samples::<i16>() {
                    if let Ok(s) = sample {
                        let _ = writer.write_sample(s);
                    }
                }
            }
        }

        writer
            .finalize()
            .map_err(|e| format!("Failed to finalize audio_full.wav: {}", e))?;

        Ok(Some(full_audio_path))
    }

    pub fn generate_markdown_note(
        &self,
        session: &MeetingSession,
        transcript: &str,
    ) -> Result<PathBuf, String> {
        let note_path = self.session_dir(&session.id).join("meeting.md");
        let formatted = format!(
            "---\nid: \"{}\"\ntitle: \"{}\"\nstate: \"{:?}\"\ncreated_at: \"{}\"\nduration_seconds: {:.1}\nchunks: {}\n---\n\n# {}\n\n**Duration**: {:.1} seconds | **Recorded Chunks**: {}\n**Created**: {}\n\n## Transcript\n\n{}\n",
            session.id,
            session.title,
            session.state,
            session.created_at,
            session.duration_seconds,
            session.chunk_count,
            session.title,
            session.duration_seconds,
            session.chunk_count,
            session.created_at,
            if transcript.trim().is_empty() {
                "_No speech detected in this recording session._"
            } else {
                transcript.trim()
            }
        );

        fs::write(&note_path, formatted)
            .map_err(|e| format!("Failed to write meeting.md: {}", e))?;
        Ok(note_path)
    }

    /// Scans for unfinalized or interrupted sessions on application launch.
    pub fn scan_interrupted_sessions(&self) -> Result<Vec<MeetingSession>, String> {
        let sessions = self.list_sessions()?;
        let mut interrupted = Vec::new();

        for mut session in sessions {
            if matches!(
                session.state,
                MeetingState::Starting
                    | MeetingState::Recording
                    | MeetingState::Paused
                    | MeetingState::Stopping
                    | MeetingState::Finalizing
            ) {
                session.state = MeetingState::Interrupted;
                let _ = self.save_session(&session);
                interrupted.push(session);
            }
        }

        Ok(interrupted)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::meetings_v2::types::TranscriptSegmentStatus;
    use std::sync::Arc;

    #[test]
    fn test_session_store_lifecycle_and_chunking() {
        let temp_dir = std::env::temp_dir().join(format!("relay_test_meet_store_{}", uuid::Uuid::new_v4()));
        let store = SessionStore::new(temp_dir.clone());

        // 1. Create and init session
        let mut session = MeetingSession::new("test_session_01".to_string(), Some("Sprint Planning".to_string()));
        store.init_session(&session).unwrap();

        let loaded = store.get_session(&session.id).unwrap();
        assert_eq!(loaded.title, "Sprint Planning");
        assert_eq!(loaded.state, MeetingState::Starting);

        // 2. Write 2 chunks
        let chunk1_samples = vec![0.1_f32; 16000]; // 1s
        let chunk2_samples = vec![0.2_f32; 16000]; // 1s
        let c1_path = store.write_chunk_wav(&session.id, 0, &chunk1_samples, 16000).unwrap();
        let c2_path = store.write_chunk_wav(&session.id, 1, &chunk2_samples, 16000).unwrap();

        assert!(c1_path.exists());
        assert!(c2_path.exists());

        let chunk_files = store.list_chunk_files(&session.id).unwrap();
        assert_eq!(chunk_files.len(), 2);

        // 3. Append incremental transcript segments
        let seg1 = TranscriptSegment {
            chunk_index: 0,
            start_time_s: 0.0,
            end_time_s: 30.0,
            text: "Hello everyone.".to_string(),
            created_at: chrono::Utc::now().to_rfc3339(),
            status: TranscriptSegmentStatus::Success,
        };
        let seg2 = TranscriptSegment {
            chunk_index: 1,
            start_time_s: 30.0,
            end_time_s: 60.0,
            text: "Let's review the architecture.".to_string(),
            created_at: chrono::Utc::now().to_rfc3339(),
            status: TranscriptSegmentStatus::Success,
        };

        store.append_transcript_segment(&session.id, &seg1).unwrap();
        store.append_transcript_segment(&session.id, &seg2).unwrap();

        let segments = store.get_transcript_segments(&session.id).unwrap();
        assert_eq!(segments.len(), 2);
        assert_eq!(segments[0].text, "Hello everyone.");
        assert_eq!(segments[1].text, "Let's review the architecture.");

        let full_text = store.get_full_transcript_text(&session.id).unwrap();
        assert_eq!(full_text, "Hello everyone. Let's review the architecture.");

        // 4. Merge chunks into audio_full.wav
        let full_audio = store.merge_chunks_to_full_audio(&session.id).unwrap().unwrap();
        assert!(full_audio.exists());

        // 5. Generate Markdown note
        session.state = MeetingState::Completed;
        session.duration_seconds = 60.0;
        session.chunk_count = 2;
        let note_path = store.generate_markdown_note(&session, &full_text).unwrap();
        assert!(note_path.exists());
        let md_content = fs::read_to_string(note_path).unwrap();
        assert!(md_content.contains("Sprint Planning"));
        assert!(md_content.contains("Hello everyone. Let's review the architecture."));

        let _ = fs::remove_dir_all(temp_dir);
    }

    #[test]
    fn concurrent_updates_never_clobber_each_others_counters() {
        // The capture worker and the engine both mutate session metadata while a
        // meeting runs. Saving a `MeetingSession` cloned earlier would roll back
        // whatever the other thread wrote in the meantime, which is how recorded
        // chunk and transcript counts used to end up back at zero.
        let temp_dir =
            std::env::temp_dir().join(format!("relay_test_meet_concurrent_{}", uuid::Uuid::new_v4()));
        let store = Arc::new(SessionStore::new(temp_dir.clone()));

        let session = MeetingSession::new("concurrent_session".to_string(), None);
        store.init_session(&session).unwrap();

        let mut handles = Vec::new();
        for _ in 0..4 {
            let store = store.clone();
            let id = session.id.clone();
            handles.push(std::thread::spawn(move || {
                for _ in 0..50 {
                    store
                        .update_session(&id, |s| {
                            s.chunk_count += 1;
                            s.transcript_segment_count += 1;
                            s.total_audio_bytes += 10;
                        })
                        .unwrap();
                }
            }));
        }
        for handle in handles {
            handle.join().unwrap();
        }

        let reloaded = store.get_session(&session.id).unwrap();
        assert_eq!(reloaded.chunk_count, 200);
        assert_eq!(reloaded.transcript_segment_count, 200);
        assert_eq!(reloaded.total_audio_bytes, 2_000);

        let _ = fs::remove_dir_all(temp_dir);
    }

    #[test]
    fn saving_leaves_no_partial_file_behind() {
        // session.json is committed by rename, so a crash mid-write cannot leave
        // truncated JSON — which would make the meeting unreadable, and so
        // invisible in the UI, even with its audio intact.
        let temp_dir =
            std::env::temp_dir().join(format!("relay_test_meet_atomic_{}", uuid::Uuid::new_v4()));
        let store = SessionStore::new(temp_dir.clone());

        let session = MeetingSession::new("atomic_session".to_string(), None);
        store.init_session(&session).unwrap();
        store
            .update_session(&session.id, |s| s.state = MeetingState::Recording)
            .unwrap();

        let dir = store.session_dir(&session.id);
        assert!(dir.join("session.json").exists());
        assert!(
            !dir.join("session.json.tmp").exists(),
            "the temporary file must not survive a committed write"
        );
        assert_eq!(
            store.get_session(&session.id).unwrap().state,
            MeetingState::Recording
        );

        let _ = fs::remove_dir_all(temp_dir);
    }

    #[test]
    fn recovery_reclaims_sessions_interrupted_while_finalizing() {
        // Finalization (merging chunks, writing the note) is the slowest part of
        // a stop, so a crash there is likely; such a session used to stay in
        // FINALIZING forever and never be recovered.
        let temp_dir =
            std::env::temp_dir().join(format!("relay_test_meet_finalizing_{}", uuid::Uuid::new_v4()));
        let store = SessionStore::new(temp_dir.clone());

        let mut session = MeetingSession::new("finalizing_session".to_string(), None);
        session.state = MeetingState::Finalizing;
        store.init_session(&session).unwrap();

        let interrupted = store.scan_interrupted_sessions().unwrap();
        assert_eq!(interrupted.len(), 1);
        assert_eq!(interrupted[0].state, MeetingState::Interrupted);

        let _ = fs::remove_dir_all(temp_dir);
    }

    #[test]
    fn test_crash_recovery_scan_and_reconciliation() {
        let temp_dir = std::env::temp_dir().join(format!("relay_test_meet_crash_{}", uuid::Uuid::new_v4()));
        let store = SessionStore::new(temp_dir.clone());

        // Create an unfinalized session in RECORDING state
        let mut session = MeetingSession::new("crashed_session_01".to_string(), Some("Crashed Sync".to_string()));
        session.state = MeetingState::Recording;
        store.init_session(&session).unwrap();

        // Write a chunk
        let samples = vec![0.05_f32; 16000];
        store.write_chunk_wav(&session.id, 0, &samples, 16000).unwrap();

        // Append 1 transcript segment
        let seg = TranscriptSegment {
            chunk_index: 0,
            start_time_s: 0.0,
            end_time_s: 30.0,
            text: "This was preserved before crash.".to_string(),
            created_at: chrono::Utc::now().to_rfc3339(),
            status: TranscriptSegmentStatus::Success,
        };
        store.append_transcript_segment(&session.id, &seg).unwrap();

        // Simulate app restart and scan
        let interrupted = store.scan_interrupted_sessions().unwrap();
        assert_eq!(interrupted.len(), 1);
        assert_eq!(interrupted[0].id, "crashed_session_01");
        assert_eq!(interrupted[0].state, MeetingState::Interrupted);

        let reloaded = store.get_session(&session.id).unwrap();
        assert_eq!(reloaded.state, MeetingState::Interrupted);

        let _ = fs::remove_dir_all(temp_dir);
    }
}

