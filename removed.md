# Removed Subsystems: Meetings and Talkback Architecture & Functional Specification

> **Archival Notice**
> This document was generated upon the complete removal of the **Meetings** (`meetings_v2`) and **Talkback** (including Piper TTS and Calendar meeting matching) subsystems from Relay.
> It preserves all functional, architectural, mathematical, prompt-engineering, and interface knowledge of these components for future reference or re-implementation.

---

## Table of Contents
1. [Overview & Rationale](#1-overview--rationale)
2. [Meetings Subsystem (`meetings_v2`)](#2-meetings-subsystem-meetings_v2)
   - [2.1 High-Level Architecture & 7-Layer Pipeline](#21-high-level-architecture--7-layer-pipeline)
   - [2.2 Audio Ingestion & Chunking](#22-audio-ingestion--chunking)
   - [2.3 Acoustic Screening & Hallucination Defense](#23-acoustic-screening--hallucination-defense)
   - [2.4 Diarization & Speaker Identification Engines](#24-diarization--speaker-identification-engines)
   - [2.5 Multi-Stage LLM Extraction Pipeline](#25-multi-stage-llm-extraction-pipeline)
   - [2.6 Google Calendar Integration & Candidate Matching](#26-google-calendar-integration--candidate-matching)
   - [2.7 Overlay Windows, Reminders & Conferencing Detection](#27-overlay-windows-reminders--conferencing-detection)
   - [2.8 Session Store, Vault Layout & Recovery](#28-session-store-vault-layout--recovery)
   - [2.9 Frontend Views & Diagnostics](#29-frontend-views--diagnostics)
   - [2.10 Meeting Prompt Rules Specification](#210-meeting-prompt-rules-specification)
3. [Talkback Subsystem (`talkback` & `tts`)](#3-talkback-subsystem-talkback--tts)
   - [3.1 Architecture & Grounded Retrieval Pipeline](#31-architecture--grounded-retrieval-pipeline)
   - [3.2 State Machine & Full-Duplex Barge-in Handling](#32-state-machine--full-duplex-barge-in-handling)
   - [3.3 Context Assembly & Prompt Boundaries](#33-context-assembly--prompt-boundaries)
   - [3.4 Local Piper TTS Engine & Installation Infrastructure](#34-local-piper-tts-engine--installation-infrastructure)
   - [3.5 Frontend UI, Visualizer & Audio Queue](#35-frontend-ui-visualizer--audio-queue)
4. [Comprehensive Catalog of Removed Artifacts](#4-comprehensive-catalog-of-removed-artifacts)
   - [4.1 Backend Modules & Files](#41-backend-modules--files)
   - [4.2 Tauri Commands & IPC Events](#42-tauri-commands--ipc-events)
   - [4.3 Tauri Capabilities, Overlays & Routes](#43-tauri-capabilities-overlays--routes)
   - [4.4 Frontend Components, Hooks & Types](#44-frontend-components-hooks--types)
   - [4.5 Crates & External Dependencies](#45-crates--external-dependencies)

---

## 1. Overview & Rationale

Relay's architecture initially supported three primary capture modalities:
1. **Push-to-talk & Voice Notes**: Local dictation and note-taking powered by Whisper.
2. **Meetings (`meetings_v2`)**: Long-form multi-speaker meeting recording (combining microphone and loopback audio), acoustic chunking, speaker diarization, calendar auto-matching, desktop reminder cards, and a two-stage LLM pipeline (facts extraction + executive summary).
3. **Talkback**: A hands-free, turn-taking conversational voice assistant operating over the user's personal knowledge vault with local TTS via Piper.

To streamline Relay into a focused, low-overhead personal knowledge capture and synthesis application, the long-form **Meetings** system and the voice **Talkback** assistant were retired.

---

## 2. Meetings Subsystem (`meetings_v2`)

### 2.1 High-Level Architecture & 7-Layer Pipeline

Meetings V2 was structured across 7 decoupled processing layers:

```
[Layer 1: Audio Capture] ──> Dual CPAL streams (Mic + System Loopback)
                                   │
[Layer 2: Chunking & Gate] ──> 30s durable WAVs + RMS Speech Gate (discard silence)
                                   │
[Layer 3: Transcription]  ──> Live Whisper transcription + repetition suppression
                                   │
[Layer 4: Diarization]    ──> Acoustic embeddings / GMM / Turn-taking heuristics
                                   │
[Layer 5: Calendar Match] ──> Read-only Google Calendar scoring & attendee roster
                                   │
[Layer 6: LLM Extraction] ──> Stage A (Structured Facts) ──> Stage B (Markdown Summary)
                                   │
[Layer 7: Storage & UI]   ──> Immutable JSON sessions, audio chunks, Trash, UI views
```

### 2.2 Audio Ingestion & Chunking
- **Dual Device Capture (`capture/device.rs`)**: Recorded from the default microphone and WASAPI loopback audio simultaneously (capturing both the user and remote call participants).
- **30-Second Slices**: Audio was continuously chunked into 30-second 16kHz mono 16-bit PCM WAV files (`chunk_000000.wav`, `chunk_000001.wav`, etc.).
- **Durable Index**: Each slice was flushed to disk under `.relay/vault/meetings_v2/<meeting_id>/audio/` immediately upon completion so that unexpected shutdowns or application crashes lost at most 30 seconds of audio.

### 2.3 Acoustic Screening & Hallucination Defense
Whisper tends to hallucinate repetitive subtitles or boilerplate phrases ("Thank you for watching", "Subtitles by...") when decoding silence, air conditioning, or background hiss. Meetings V2 defended against this in two phases:
1. **Pre-Decode RMS Energy Gate**: Chunks with root-mean-square amplitude below `-42 dBFS` were marked as silence and bypassed the Whisper decoder entirely.
2. **Post-Decode Repetition Screen**: Whisper output was screened for repeating n-grams (3+ repeated words or sentences) and compression ratio anomalies. Chunks failing this screen were logged with diagnostic status `Rejected(Hallucination)` and omitted from the transcript sent to the LLM.

### 2.4 Diarization & Speaker Identification Engines
Three interchangeable diarization engines were supported:
1. **Agglomerative Clustering (Voice Embeddings)**:
   - Audio segments were projected into a 512-dimensional embedding space using an ONNX speaker recognition model.
   - Cosine distance matrix was clustered with a tuned threshold (`0.68`).
   - Self-voice anchoring: The user's own voice profile was anchored from their voice note recordings, ensuring "Speaker 0" consistently mapped to the local user.
2. **GMM / Statistical Clustering**: Fast Gaussian Mixture Model clustering over MFCC features for lower-end hardware without GPU acceleration.
3. **Turn-Taking Heuristics (Rule-Based)**: Fallback engine using acoustic silence gaps (>750ms) and linguistic sentence boundaries to separate turns.
4. **Speaker Naming & Suggestions**:
   - Proposed speaker names were extracted from conversational cues ("Thanks Rahul", "Over to you, Sarah") or matched against calendar invitees.
   - Names were displayed in the UI as unconfirmed chips that the user could accept, edit, or merge across the recording.

### 2.5 Multi-Stage LLM Extraction Pipeline
Meeting analysis was strictly separated into two prompt stages to eliminate LLM hallucinations:
- **Stage A: Facts Extraction (`meeting.facts` prompt)**:
  - Input: Verified speaker-tagged transcript and calendar participant metadata.
  - Output: Strict JSON schema containing:
    - Key discussion topics with timestamp references.
    - Explicit decisions made and the rationale behind them.
    - Action items with owner and due date.
    - Unresolved questions and identified project risks.
- **Stage B: Executive Summary (`meeting.summary` prompt)**:
  - Input: Stage A structured facts and verified speaker roster.
  - Output: Clean GitHub-flavored Markdown document with clear sections:
    - Executive Summary
    - Key Decisions
    - Action Items (with checkbox syntax `- [ ]`)
    - Discussion Points per Topic
- **Directives & Custom Extensions**:
  - Users could supply custom instructions ("Format for engineering standup", "Focus on budget numbers").
  - Directives adjusted the Stage B synthesis without invalidating Stage A facts.

### 2.6 Google Calendar Integration & Candidate Matching
- **Scope**: Strictly read-only OAuth 2.0 access (`https://www.googleapis.com/auth/calendar.events.readonly`).
- **Matching Algorithm (`match_recording`)**:
  - Evaluated calendar events within `[-15 min, +30 min]` of meeting start/end times.
  - Scored candidates on overlap duration, participant overlap, and start-time proximity.
  - If two events scored identically within an ambiguity margin, both were presented to the user rather than auto-linking.
- **Candidate Roster**: Attendee emails and display names from the matched event pre-populated the speaker suggestion pool.

### 2.7 Overlay Windows, Reminders & Conferencing Detection
- **`meeting-overlay` Window**:
  - A compact horizontal pill (248x52 resting, 330x52 expanded) anchored to the top-right screen edge.
  - Rendered `MeetingRecordingOverlay.tsx`, showing live recording duration, audio waveform, pause/resume/stop buttons.
- **`meeting-reminder` Window**:
  - Floating 400x120 borderless card displayed 2 minutes prior to scheduled calendar events.
  - Actions:
    - **Record**: Immediately initiates V2 recording, sets title from calendar event, and navigates main app to Meetings.
    - **Join**: Launches the video meeting link (Google Meet, Zoom, MS Teams, Webex) in the default browser via `tauri-plugin-opener` and re-arms the reminder after 60 seconds.
    - **Snooze**: Snoozes notification for 5, 10, or 15 minutes.
  - Set `content_protected = true` in Tauri to prevent the card from appearing in screen shares or recordings.
- **Conferencing Window Detection**:
  - Periodically scanned open window titles on Windows for active meeting clients ("Zoom Meeting", "Google Meet - ", "Microsoft Teams", "Cisco Webex Meetings") to trigger reminders even if the meeting was not in the calendar.

### 2.8 Session Store, Vault Layout & Recovery
- **Disk Structure**:
  ```
  .relay/vault/meetings_v2/<session_id>/
  ├── session.json         # Complete metadata, state, title, duration
  ├── transcript.json      # Structured utterances with speaker IDs and timestamps
  ├── analysis.json        # Stage A facts and Stage B summary
  ├── notes.json           # User typed notes during/after meeting
  ├── calendar_link.json   # Matched Google Calendar event
  └── audio/
      ├── chunk_000000.wav
      └── chunk_000001.wav
  ```
- **Crash Recovery**: At startup, `MeetingsV2Engine::recover_interrupted_sessions()` inspected `<vault>/meetings_v2/` for sessions left in `Recording` or `Paused` state, finalized their chunk index, calculated true duration from audio files, and marked them `Completed`.
- **Trash Lifecycle**: Moving a meeting to Trash moved the folder to `.relay/vault/.trash/<trash_id>/`, allowing full restoration or automatic permanent purge after 30 days.
- **Scribble Promotion**: Meetings could be promoted to permanent knowledge Scribbles (`Scribble::from_meeting`), carrying topics, entities, and summary text into the unified knowledge graph.

### 2.9 Frontend Views & Diagnostics
- `MeetingsV2View.tsx`: Main meeting interface with sidebar session list, live recording state, tabbed meeting detail:
  - **Summary Tab**: Rendered executive markdown summary, decisions, risks, and questions.
  - **Conversation Tab**: Color-coded speaker turns, audio waveform scrub bar, speaker rename dialog.
  - **Raw Transcript Tab**: Non-diarized timestamped Whisper chunks with confidence and RMS metrics.
  - **Notes Tab**: Live rich-text markdown notepad synchronized with meeting timestamps.
  - **Action Items Bar**: Interactive task list with "Push to Kanban" integration.
- `SpeakerEngineComparison.tsx`: Diagnostics view running all three speaker diarization engines concurrently over a selected recording to benchmark clustering purity and speaker confusion.
- `MeetingPipelineDiagnostics.tsx`: Test runner exercising acoustic gates, Whisper hallucination rejection, and prompt pipelines with synthetic audio.

### 2.10 Meeting Prompt Rules Specification
The following core rules governed prompt construction (formerly in `Meeting-rules/`):
- **Evidence-Only Ingestion**: Calendar descriptions and attendee notes were treated as untrusted data, enclosed within `<untrusted_input>` tags to prevent prompt injection.
- **Zero Extrapolation**: Summaries were forbidden from stating facts or decisions not present in the Stage A JSON.
- **Strict Ownership**: Action items had to identify an explicit individual speaker or be marked "Unassigned".

---

## 3. Talkback Subsystem (`talkback` & `tts`)

### 3.1 Architecture & Grounded Retrieval Pipeline
Talkback was Relay's local conversational voice assistant. It answered user questions aloud using only data retrieved from the local vault:

```
[User Utterance] ──> STT Engine (Streaming Whisper)
                          │
                   Intent Classification
                          │
                   Unified Retrieval (Voice Notes, Scribbles, Files, Web Captures)
                          │
                   Context Pack Assembly (Strict Prompt Isolation Boundary)
                          │
                   Local LLM Streaming (`talkback.answer`)
                          │
                   Piper TTS Engine (Local ONNX)
                          │
                   Audio Queue Playback (Speaker Output with Barge-In Guard)
```

### 3.2 State Machine & Full-Duplex Barge-in Handling
Talkback operated as an asynchronous finite state machine:
- `Idle`: Microphone warm, awaiting wake event or push-to-talk.
- `Listening`: Capturing user speech input via VAD (Voice Activity Detection).
- `Transcribing`: Finalizing Whisper STT on the recorded query.
- `Retrieving`: Executing multi-source retrieval across vault notes and files.
- `Generating`: Streaming conversational answer from local LLM (Ollama).
- `Synthesizing`: Streaming Piper TTS synthesis on completed sentences.
- `Speaking`: Playing synthesized WAV audio chunks over speakers.
- `Interrupted`: Triggered when user spoke during `Synthesizing` or `Speaking`.
  - Audio playback halted in `< 20ms`.
  - Pending synthesis tasks cancelled.
  - Context retained; state transitioned directly back to `Listening`.

### 3.3 Context Assembly & Prompt Boundaries
- **Prompt ID**: `PromptId::TalkbackAnswer` (`talkback.answer`).
- **Prompt Structure**:
  - Enforced a concise, conversational tone suitable for speech (1-3 sentences per answer).
  - Explicit instruction: "Answer ONLY using the provided vault facts. If the vault does not contain the answer, say 'I couldn't find that in your vault' and do not speculate."
  - Output formatting: Plain text only, avoiding markdown headers, tables, bullet points, or code blocks that sound awkward when synthesized.

### 3.4 Local Piper TTS Engine & Installation Infrastructure
- **Engine**: Embedded Piper neural text-to-speech runtime (`piper.exe` on Windows).
- **Voice Manifest (`resources/voice-manifest.json`)**:
  - Catalog of bundled and downloadable ONNX voice models (e.g. `en_US-amy-medium`, `en_US-lessac-medium`).
  - Stored model hashes, sample rates, size, and remote download URLs.
- **Installer Infrastructure (`tts/installer.rs`)**:
  - Multi-platform downloader supporting `.zip` (Windows) and `.tar.gz` (Unix/macOS).
  - Multi-stage installation: Download $\rightarrow$ Checksum Verification $\rightarrow$ Staging Extraction $\rightarrow$ Atomic Promotion to `.relay/config/tts/` $\rightarrow$ Self-Test Synthesis.
  - Self-Test phrase: Synthesizing `"Relay speech synthesizer is ready."` and verifying output WAV validity before reporting `TtsStatus::Ready`.

### 3.5 Frontend UI, Visualizer & Audio Queue
- `TalkbackPage.tsx`: Full conversational canvas with conversational history.
- `TalkbackAgent.tsx`: Compact assistant overlay with status pills and latency readouts (TTFT, TTS first-audio latency).
- `TalkbackOrbCanvas.tsx`: Fluid canvas orb visualizer reacting to audio input and output volume.
- `talkbackAudioQueue.ts`: Web Audio API playback pipeline managing queue buffering, sentence sequencing, and instant playback flushing on barge-in.
- `TalkbackSettingsView.tsx` & `VoiceSettings.tsx`: Settings panel for voice selection, volume, rate, and test playback.

---

## 4. Comprehensive Catalog of Removed Artifacts

### 4.1 Backend Modules & Files
- `native/src-tauri/src/meetings_v2/` (24 Rust files: mod, types, worker, session_store, diarize, processing, reminders, tests).
- `native/src-tauri/src/talkback/` (11 Rust files: mod, engine, turn, speech, audio, assemble, retrieval, intent, session, sources, state).
- `native/src-tauri/src/tts/` (6 Rust files: mod, piper, installer, discovery, manifest, types).
- `native/src-tauri/src/calendar/` (5 Rust files: mod, accounts, google, match_event, model).
- `native/src-tauri/resources/voice-manifest.json`.
- `Meeting-rules/` (6 markdown specification files).
- `docs/meetings/` (5 documentation files).
- `docs/talkback/` (3 documentation files).

### 4.2 Tauri Commands & IPC Events
- **Meetings**:
  - `start_meeting_v2`, `stop_meeting_v2`, `pause_meeting_v2`, `resume_meeting_v2`, `get_active_meeting_v2`, `list_meetings_v2`, `get_meeting_v2`, `get_meeting_v2_transcript`, `get_meeting_v2_diagnostics`, `delete_meeting_v2`, `summarize_meeting_v2`, `get_meeting_v2_processing`, `prepare_meeting_v2`, `generate_meeting_v2_summary`, `rename_meeting_v2_speaker`, `identify_meeting_v2_speakers`, `compare_meeting_v2_speaker_engines`, `merge_meeting_v2_speakers`, `get_meeting_v2_audio_chunk_path`, `share_meeting_v2`, `run_meeting_pipeline_selftest`, `get_meeting_v2_transcript_health`, `get_meeting_v2_notes`, `save_meeting_v2_notes`, `add_meeting_v2_directive`, `remove_meeting_v2_directive`, `set_meeting_v2_action_item_status`, `get_meeting_v2_related`, `get_meeting_v2_processing_log`, `get_meeting_v2_extensions`, `list_meeting_v2_processing`, `promote_meeting_v2_to_scribble`, `push_meeting_v2_action_items_to_kanban`, `set_meeting_overlay_expanded`.
- **Meeting Reminders & Calendar**:
  - `get_pending_meeting_reminder`, `meeting_reminder_ready`, `meeting_reminder_hover_changed`, `dismiss_meeting_reminder`, `snooze_meeting_reminder`, `join_meeting_from_reminder`, `start_meeting_from_reminder`, `trigger_mock_meeting_reminder`, `debug_detect_conferencing_windows`, `get_calendar_connection`, `connect_google_calendar`, `sync_google_calendar`, `list_calendar_accounts`, `add_google_calendar_account`, `update_calendar_account`, `disconnect_calendar_account`, `sync_calendar_accounts`, `get_upcoming_calendar_events`, `disconnect_google_calendar`, `link_meeting_v2_to_calendar`, `set_meeting_v2_calendar_event`, `get_meeting_v2_calendar_link`.
- **Talkback & TTS**:
  - `start_talkback`, `stop_talkback`, `get_talkback_state`, `get_talkback_session`, `submit_talkback_turn`, `interrupt_talkback`, `search_talkback_context`, `get_tts_status`, `browse_for_piper_binary`, `browse_for_piper_voice`, `set_tts_configuration`, `test_tts_voice`, `prepare_tts_folders`, `install_local_voice`, `cancel_voice_install`.
- **IPC Events**:
  - `meeting-reminder`, `meeting-v2-updated`, `meeting-v2-chunk-transcribed`, `meeting-v2-state-changed`, `talkback-state-changed`, `talkback-audio-chunk`, `talkback-turn-complete`, `tts-install-progress`.

### 4.3 Tauri Capabilities, Overlays & Routes
- Capability: `native/src-tauri/capabilities/meeting-reminder.json`.
- Windows:
  - `meeting-overlay` (desktop edge recording indicator).
  - `meeting-reminder` (desktop notification window).
- Hash routes in `native/src/main.tsx`:
  - `meeting-overlay`
  - `meeting-reminder`

### 4.4 Frontend Components, Hooks & Types
- `native/src/components/meetings_v2/` (29 files: `MeetingsV2View`, tabs, action items, modals, tests).
- `native/src/components/talkback/` (9 files: `TalkbackPage`, `TalkbackAgent`, `TalkbackOrbCanvas`, `useTalkback`, tests).
- `native/src/components/settings/MeetingsSettings.tsx`.
- `native/src/components/settings/TalkbackSettingsView.tsx`.
- `native/src/components/settings/VoiceSettings.tsx` & `VoiceSettings.test.tsx`.
- `native/src/components/settings/VoiceLibraryModal.tsx`.
- `native/src/components/diagnostics/MeetingPipelineDiagnostics.tsx` & `.test.tsx`.
- `native/src/components/diagnostics/SpeakerEngineComparison.tsx` & `.test.tsx`.

### 4.5 Crates & External Dependencies
- Crate `tar = "0.4"` (removed from `native/src-tauri/Cargo.toml`).
- Crate `flate2 = "1"` (removed from `native/src-tauri/Cargo.toml`).
- Voice models and Piper executables previously staged in `.relay/config/tts/`.
