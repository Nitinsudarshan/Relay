# Relay — Meetings Intelligence v2 Architecture

**Status**: Implemented (v0.36.0)  
**Scope**: `native/src-tauri/src/meetings_v2/`, `native/src-tauri/src/calendar/`, `native/src/components/meetings_v2/`

---

## 1. Core Architectural Separation

Relay Meetings v2 enforces a strict separation between physical storage boundaries, transcription units, identity evidence, conversational structures, and intelligence outputs:

```
GOOGLE CALENDAR
      │ contextual evidence (roster, times, title)
      ▼
┌──────────────┐
│ Audio Capture│ ─── 30-second storage / recovery chunks (WAV)
└──────┬───────┘
       ▼
┌──────────────┐
│ Transcript   │ ─── Whisper utterances (meeting-relative timestamps)
│ Utterances   │
└──────┬───────┘
       ▼
┌──────────────┐
│ Speaker      │ ─── Identity evidence (channel, self-voice anchor,
│ Assignment   │     diarization cluster, calendar candidate, manual)
└──────┬───────┘
       ▼
┌──────────────┐
│ Speaker      │ ─── Conversational units (cross-chunk continuity,
│ Turns        │     interjection preservation, same-speaker merging)
└──────┬───────┘
       ▼
┌──────────────┐
│ Speaker      │ ─── Multi-signal evidence hierarchy resolution
│ Resolution   │
└──────┬───────┘
       ▼
┌──────────────┐
│ Meeting      │ ─── Extracted facts (topics, decisions, actions)
│ Facts        │
└──────┬───────┘
       │
  ┌────┴────────────┐
  ▼                 ▼
Deterministic    Local LLM
Summary Floor    Enhancement (Optional)
  │                 │
  └────┬────────────┘
       ▼
┌──────────────┐
│ Meeting      │ ─── Truthful meeting record, ready status,
│ Record       │     provenance-preserving, clickable audio seeking
└──────────────┘
```

---

## 2. Audio Chunks Are Implementation Units, Not Transcript Units

- **30-Second Chunks**: Used strictly for capture streaming (`cpal`), audio persistence (`chunk_NNNN.wav`), incremental decoding, crash resilience, and bounded memory usage.
- **Transcript Utterances**: Whisper emits word/phrase segments with millisecond offsets within each chunk. Relay normalizes every utterance timestamp to **meeting-relative milliseconds** (`start_ms`, `end_ms`).
- **Semantic Continuity**: If a speaker speaks from `00:25` to `00:42`, the 30-second storage boundary at `00:30` does **not** split the conversational turn into artificial pieces. The turn engine bridges adjacent chunks.

---

## 3. The Seven-Layer Meeting Pipeline

### Layer 1: Audio Capture & Persistence
- Microphones and loopback system audio streams are captured at 16 kHz mono.
- Chunks are written to disk as standard WAV files under `<meeting_dir>/audio/chunk_NNNN.wav`.
- The raw audio remains permanently inspectable and auditable.

### Layer 2: Canonical Transcript Utterances
- Each utterance carries:
  ```rust
  pub struct TranscriptUtterance {
      pub id: String,
      pub meeting_id: String,
      pub audio_chunk_id: String,
      pub start_ms: u64,
      pub end_ms: u64,
      pub text: String,
      pub confidence: Option<f32>,
      pub source: TranscriptSource,
  }
  ```
- **Immutability Guarantee**: Raw transcript files (`transcript.jsonl`) are strictly append-only and immutable. Downstream identity reassignments or speaker merges **never** rewrite raw transcript text.

### Layer 3: Speaker Assignment & Evidence
- Speaker assignment is an explicit, verifiable provenance layer:
  ```rust
  pub struct SpeakerAssignment {
      pub utterance_id: String,
      pub speaker_id: String,
      pub confidence: f32,
      pub method: SpeakerAssignmentMethod, // Channel, SelfVoiceAnchor, Diarization, Calendar, ContextualInference, Manual
      pub evidence: SpeakerEvidence,
  }
  ```
- Every assignment explains *why* Relay attributed the speech to that speaker (e.g. `channel: mic`, `similarity: 0.88`, `cluster: 2`).

### Layer 4: Conversational Speaker Turns
- Consecutive utterances by the same speaker are merged into coherent conversational turns.
- **Short Interjections Preserved**: If Speaker A speaks for 15s, Speaker B says "Yes." (1s), and Speaker A continues, the system produces **three turns** (`A -> B -> A`). Short utterances are never discarded or absorbed into surrounding speakers.
- **Cross-Chunk Invariance**: Same-speaker utterances bridging chunk boundaries (`25-30s` and `30-36s`) merge into a single continuous logical turn (`25-36s`).

### Layer 5: Centralized Speaker Resolution & Evidence Hierarchy
Relay applies a strict, testable hierarchy where strong acoustic evidence cannot be overridden by weak contextual guesses:
1. **Manual User Override**: Definitive correction by the user.
2. **Channel Identity**: Dedicated microphone input (`mic` channel) vs system loopback (`system` channel).
3. **Self-Voice Anchor**: Meeting-local acoustic reference extracted from high-quality local microphone samples (>1.0s), used to accurately identify short local interruptions and interjections.
4. **Diarization Clustering**: Acoustic feature clustering (MFCC, pitch, energy) grouping utterances into distinct voice clusters without inventing names.
5. **Contextual Name Inference**: Conservative spoken self-introductions (e.g., "I'm Nitin", "This is Bala speaking").
6. **Calendar Candidate Roster**: Candidate attendee names mapped to clusters only when corroborating evidence exists.
7. **Unnamed Speaker Floor**: If evidence is insufficient, the system labels the turn honestly as `Speaker N` rather than guessing a participant name.

### Layer 6: Google Calendar Context & Attendance Reconciliation
- **Read-Only Integration**: Connects via OAuth (`calendar.events.readonly`) with token refresh and zero event-mutation permissions.
- **Multi-Signal Matching**: Matches recordings to calendar events using temporal overlap, duration proximity, conference URLs, and title/attendee hints.
- **Attendance vs. Heard vs. Identified**:
  - **Invited**: Present on the calendar attendee list.
  - **Heard**: Acoustic evidence detected matching this participant's voice or self-introduction.
  - **No Voice Evidence**: Attendee was invited and may have been quietly present, but no distinguishable voice was isolated. Never falsely labelled as "absent".
  - **Identity**: Marked as `confirmed` (manual/channel/anchor), `inferred` (calendar candidate/context), or `unresolved` (`Speaker N`).
- **Prompt Injection Boundary**: Calendar titles, descriptions, and attendee names are treated as untrusted external text. They are never interpreted as LLM instructions.

### Layer 7: Meeting Facts & Dual-Track Intelligence
- **Structured Evidence Floor**: Meeting facts (agreed decisions, assigned action items with owners and deadlines, key topics, risks, open questions) are extracted directly from the transcript turns.
- **Deterministic Summary Floor**: If no LLM is configured or if the local model fails/returns empty output, Relay renders a structured deterministic summary grounded in the extracted meeting facts. The meeting status remains **Ready** (`✓ Summary generated locally`).
- **Optional Local LLM Enhancement**: When an LLM is active, it enhances the prose fluency and thematic synthesis. If the model fails or times out, internal retry diagnostics are logged to the **Diagnostics Hub**, and the meeting never displays alarming error banners.

---

## 4. User Interaction & Provenance Seeking

- **Interactive Speaker Merging & Renaming**: Users can rename any `Speaker N` or merge duplicate clusters (`Merge Speaker 3 into Bala`) with one click. Assignments update instantly across turns, facts, and action item ownership while leaving raw audio and transcripts 100% immutable.
- **Direct Audio Seeking**: Every conversational turn displays a meeting-relative timestamp (e.g., `14:32`). Clicking the seek button resolves the turn to its underlying audio chunk (`chunk_0029.wav`) and exact millisecond offset (`2.0s`), streaming the audio locally via Tauri's secure asset protocol.
- **Upcoming Calendar Schedule**: The Meetings sidebar displays upcoming meetings for the next 24 hours with start times and participant counts, allowing instant context attachment.

---

## 5. Non-Negotiable Privacy & Trust Boundaries

1. **No Meeting Bots**: Relay records audio through local Windows audio endpoints (CPAL microphone and WASAPI loopback); no bots join video calls.
2. **No Cloud Audio or Diarization**: Whisper runs on-device; speaker clustering and self-voice anchoring run locally.
3. **No Default Persistent Voiceprints**: Voice anchors are strictly meeting-local and ephemeral. No cross-meeting biometric voice database is created without explicit user opt-in.
4. **Untrusted Calendar Content**: Calendar descriptions cannot alter LLM system prompts or hijack meeting facts.
