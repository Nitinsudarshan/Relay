# Meetings Intelligence — Architecture Audit (Phase A)

Traced from the checked-out code, not from prior design documents. Every claim
below cites the file that actually implements it. Written before any code in
this round was changed.

---

## 1. Stage-by-stage trace

### 1.1 Meeting detection

| | |
|---|---|
| **Input** | User click, or `start_meeting_v2` invoked from the recording pill |
| **Function** | `commands::start_meeting_v2` → `MeetingsV2Engine::start_session` |
| **Storage** | — |
| **Output** | `MeetingSession { state: STARTING }` |

**CURRENT** There is no automatic meeting detection. No calendar hook, no
conferencing-app detection, no audio-based "a meeting seems to be happening"
heuristic. Recording is entirely manual.

**INTENDED** Manual start is fine for this round.

**GAP** None in scope. Automatic detection is not part of this work.

### 1.2 Lifecycle / state machine

| | |
|---|---|
| **Input** | Engine calls: start / pause / resume / stop, plus startup recovery |
| **Function** | `meetings_v2/engine.rs` — `MeetingsV2Engine` owns `ActiveSessionContext` |
| **Storage** | `session.json` (`state` field) |
| **Output** | `meeting-session-state-changed` Tauri event |

`MeetingState` (`meetings_v2/types.rs`): `Idle, Starting, Recording, Paused,
Stopping, Finalizing, Completed, Interrupted, Recovered, Error`.

**CURRENT** This is a **recording** state machine only. It says nothing about
AI processing. `Error` is overloaded: a capture failure and a finalization
failure both land there.

**INTENDED** Recording state stays exactly as it is. Processing state is
tracked separately, per stage.

**GAP** No processing state machine at all. A meeting whose summary failed is
indistinguishable from one that was never summarized.

### 1.3 Session creation

| | |
|---|---|
| **Input** | Optional title |
| **Function** | `MeetingsV2Engine::start_session` → `SessionStore::init_session` |
| **Storage** | `<vault>/meetings_v2/meet_<uuid>/{session.json, audio/}` |
| **Output** | Persisted `MeetingSession` |

Session ids are `meet_<uuid>`. Default title is `Meeting — <local date/time>`.
`session.json` is committed write-then-rename, so a crash cannot truncate it
(`session_store.rs::write_session_unlocked`). Concurrent mutation goes through
`update_session`, which holds a `Mutex` and re-reads before mutating.

**GAP** None. This layer is sound and is left alone.

### 1.4 Audio recording

| | |
|---|---|
| **Input** | cpal mic stream + cpal system-loopback stream |
| **Function** | `meetings_v2/capture.rs` — `DualAudioCapture` |
| **Storage** | in-memory ring, then handed to two channels |
| **Output** | `AudioChunk` (Clock A, unbounded) and `LiveAudioFrame` (Clock B, bounded depth 8) |

Both streams are resampled to 16 kHz mono and **soft-mixed into a single
stream** (`soft_mix`). Per-chunk energy is measured per source, producing
`AudioChunk::mic_had_audio` / `sys_had_audio`.

**GAP (relevant to speakers)** The mixed stream is the only audio retained.
Channel provenance survives only as **two booleans per 30-second chunk**.
There is no per-second or per-utterance channel track. This bounds how good
channel-based speaker attribution can be without touching capture — see §4.

### 1.5 30-second chunk creation

| | |
|---|---|
| **Input** | `AudioChunk` from Clock A |
| **Function** | `meetings_v2/worker.rs::TranscriptionWorker` step 1 |
| **Storage** | `audio/chunk_%05d.wav` (16 kHz, mono, 16-bit PCM) |
| **Output** | Path on disk |

`CHUNK_DURATION_SECS = 30.0`. Audio is written **before** transcription is
attempted, so audio never depends on STT succeeding. The worker is
deliberately not interruptible: everything queued at stop time is audio the
user already recorded.

**GAP** None. Format and clock are untouched by this work.

### 1.6 Live STT

| | |
|---|---|
| **Input** | `LiveAudioFrame` (1 s frames) from Clock B |
| **Function** | `meetings_v2/live_stt.rs::LiveSttWorker` — rolling utterance buffer, re-decoded each tick |
| **Storage** | **none — ephemeral** |
| **Output** | `meeting-live-transcript` event (`LiveTranscriptUpdate`, keyed by `segment_id`) |

**CURRENT** Live text is display-only. It is never persisted and never feeds
the summary. Live frames are dropped rather than stalling capture.

**GAP** None. This is the correct separation and stays as-is.

### 1.7 Transcript persistence — the raw transcript

| | |
|---|---|
| **Input** | Whisper output per 30-second chunk |
| **Function** | `worker.rs` step 3 → `SessionStore::append_transcript_segment` |
| **Storage** | `transcript.jsonl`, append-only, one JSON object per line |
| **Output** | `TranscriptSegment { chunk_index, start_time_s, end_time_s, text, created_at, status }` |

`status` is `SUCCESS | EMPTY | FAILED`. Chunks below `SILENCE_RMS_THRESHOLD`
are marked `EMPTY` without a decode.

**CURRENT** This file **is** the raw transcript, and it is already
append-only. Nothing overwrites it. Good foundation.

**GAP** `mic_had_audio` / `sys_had_audio` are computed on the chunk and then
**thrown away** — they are not persisted on the segment. Rung 1 of the
attribution ladder in `Meeting-rules/meeting_speaker_identification.md`
("mandatory and always on") is therefore unavailable to any later stage.

### 1.8 Transcript UI

| | |
|---|---|
| **Input** | `get_meeting_v2_transcript` + live events |
| **Function** | `native/src/components/meetings_v2/MeetingsV2View.tsx` (907 lines) |
| **Storage** | React state |
| **Output** | Two tabs: `Summary`, `Transcripts` |

`const [activeMeetingTab, setActiveMeetingTab] = useState<'summary' | 'transcript'>('transcript')`

**CURRENT** **The default tab is the raw transcript.** Opening a meeting shows
a wall of 30-second STT chunks. Action-item checkboxes exist but their state
lives in a local `Set<string>` and is lost on unmount.

**INTENDED** Three tabs — `Summary`, `Conversation`, `Raw Transcript` — with
Summary as the default.

**GAP** Default tab, missing Conversation tab, "Transcripts" naming, no
processing-status surface, no speaker UI, unpersisted checkboxes, monolithic
component (rules cap page components at ~150 lines of JSX).

### 1.9 Summary generation

| | |
|---|---|
| **Input** | `SessionStore::get_full_transcript_text` — all `SUCCESS` segments concatenated with single spaces |
| **Function** | `pipeline/enrichment.rs::summarize_meeting` — **one** LLM call |
| **Storage** | `session.json` fields `summary: Option<String>`, `action_items: Vec<String>` |
| **Output** | Updated `MeetingSession` |

The one call asks the model to simultaneously comprehend the transcript,
title it, structure it, extract tasks, resolve relative dates, and emit JSON.
Pre-processing is `strip_asr_artifacts` (removes bracketed/parenthesized spans,
collapses whitespace) and nothing else.

On unparseable JSON or an LLM error it falls back to
`extract_deterministic_meeting_enrichment` — a cue-phrase scan that emits
Markdown strings.

**CURRENT** `raw STT → LLM → Markdown blob stored on the source record`.

**GAP** This is the central gap:
- Derived output is written **into the source record** (`session.json`).
- No normalization stage.
- No structured intermediate representation.
- Action items are opaque Markdown strings — no owner field, no deadline
  field, no source reference, no confidence, no status.
- No topics, entities, decisions, or meeting type as data.
- One mode only; no extensions.
- No validation of what came back.
- No record of which model/prompt/version produced it.

### 1.10 Current LLM calls

| | |
|---|---|
| **Function** | `providers/mod.rs::LLMClient::complete(prompt, system_prompt)` |
| **Output** | `LLMResponse { text, model, prompt_tokens, completion_tokens }` |

Providers: Ollama (`/api/generate`) and an OpenAI-compatible cloud path.

**Important behavior:** `complete()` **never returns an error**. On any
provider failure it logs a warning and returns
`LLMClient::heuristic_fallback(...)` with `model: "heuristic-fallback"`. A
caller cannot distinguish "the model answered" from "the provider was down"
except by inspecting `model`.

**GAP** The meeting pipeline has no way to know an LLM call failed, so it can
neither report it nor choose its own deterministic path deliberately.

### 1.11 Prompt / rules files

`Meeting-rules/` contains five detailed, well-written specs:
`meeting_transcript_summary.md`, `meeting_action_items_tasks.md`,
`meeting_speaker_identification.md`, `meeting_title_headings.md`,
`meeting_notes_competitive_teardown.md`.

**CONFIRMED CONFLICT** `meeting_transcript_summary.md` states *"Output:
Markdown only"* and specifies a two-stage procedure (Stage A comprehend,
Stage B write). The system prompt in `summarize_meeting` states *"Output ONLY
valid JSON"* and collapses both stages into one call. The rules file is
closer to correct; the code is closer to convenient.

**INTENDED** Structured JSON internally between stages; Markdown at the
presentation boundary. The rules files' Stage A / Stage B split maps directly
onto extraction / summarization.

### 1.12 Validators

**CURRENT** None in the meeting path. `strip_asr_artifacts` and
`should_regenerate_meeting_title` are input sanitizers, not output
validators. Nothing checks the model's output before it is persisted and
shown.

**GAP** Everything in §24 of the brief.

### 1.13 Speaker / diarization code

**CURRENT** None. No speaker type, no speaker id, no diarization, no
`sherpa-onnx`. The only speaker-adjacent data in the system is the pair of
per-chunk channel booleans described in §1.7, which are discarded.

### 1.14 Action-item extraction

Part of the single summary call. Output shape: `Vec<String>` of pre-rendered
Markdown such as `- [ ] Send the deck — **Nitin** · Due: 2026-08-28`.

**GAP** Owner, deadline, status, and provenance are baked into a display
string and cannot be queried, validated, re-rendered after a speaker rename,
or checked off durably.

### 1.15 Topic / entity extraction

**CURRENT** Exists for **Scribbles** only — `extract_deterministic_topics`,
`extract_deterministic_entities` in `pipeline/enrichment.rs`, backed by
`DOMAIN_TOPIC_PATTERNS` and `KNOWN_ENTITIES` keyword tables. Meetings do not
use them and store no topics or entities.

### 1.16 Meeting → Scribble

**CURRENT** Not implemented. Two pieces of scaffolding already exist:
- `vault/scribble.rs`: `pub const SOURCE_TYPE_MEETING: &str = "meeting";`
- `KnowledgeGraphData::from_scribbles` has a `source_type == SOURCE_TYPE_MEETING` branch.

No command creates a meeting-sourced Scribble. The Voice Note equivalent,
`commands::promote_voice_note_to_scribble` → `Scribble::from_voice_note`, is
the pattern to mirror.

**GAP** The command, the `Scribble::from_meeting` constructor, and provenance
metadata.

### 1.17 Meeting database / schema / storage

Filesystem only — no SQLite, no LanceDB for meetings.

```
<vault>/meetings_v2/<meeting_id>/
├── session.json        # metadata AND (today) derived summary + action items
├── transcript.jsonl    # raw STT, append-only
├── audio/chunk_*.wav   # durable 30 s chunks
├── audio_full.wav      # merged at finalize
└── meeting.md          # frontmatter + raw transcript
```

**GAP** There is no separation between source and derived data. `summary` and
`action_items` are derived, yet they live in `session.json` alongside
recording facts, so every summary regeneration rewrites the source record.

### 1.18 Settings related to meetings

**CURRENT** **None.** `AppSettings` (`settings/mod.rs`) has `provider, stt,
tts, hotkeys, ui, vault, language, diagnostics, cloud, sound, clipboard,
startup, audio_input, dictionary, snippets` — no meetings section.

`dictionary: Vec<String>` (default: Relay, Whisper, Tauri, Rust, Supabase,
LanceDB, Ollama) is a dictation vocabulary list that doubles usefully as a
**normalization glossary**.

### 1.19 Meeting tabs / components / routes

- `MeetingsV2View.tsx` — 907 lines: list, recorder controls, timer, tabs,
  summary card, action-item checklist, transcript list, delete flow.
- `MeetingRecordingOverlay.tsx` — 315 lines, the floating pill.
- Navigation: `App.tsx` tab `'meetings'`.

### 1.20 Existing meeting metadata

On `MeetingSession`: `duration_seconds, paused_seconds, chunk_count,
transcript_segment_count, word_count, total_audio_bytes, mic_active,
sys_audio_active, mic_heard, sys_audio_heard, capture_warning,
pending_transcription_chunks, error_message`.

Recording metadata is rich and trustworthy. Processing metadata does not
exist: no version, no model, no rules version, no timestamps per stage.

---

## 2. Consolidated gap list

| # | Gap | Phase |
|---|---|---|
| 1 | Derived data written into the source record (`session.json`) | B |
| 2 | Default meeting view is the raw transcript | C |
| 3 | No Conversation view; "Transcripts" not named as raw/debug | C |
| 4 | No normalization stage — raw STT goes straight to the summary model | D |
| 5 | No structured intermediate representation; one prompt does everything | E |
| 6 | Summary is one mode, long, unvalidated, unversioned | F |
| 7 | Action items are display strings, not objects | G |
| 8 | No speakers, no stable speaker ids, no rename | H |
| 9 | No extensions | I |
| 10 | No topics / entities / meeting type for meetings | J |
| 11 | No related-meeting surface | K |
| 12 | No meeting → Scribble | L |
| 13 | No meeting settings | C/F |
| 14 | No processing status, no per-stage observability, no versioning | B |
| 15 | LLM failure is invisible to callers (`heuristic_fallback` masks it) | D/E |
| 16 | Per-chunk channel provenance computed then discarded | H |

---

## 3. Target architecture

```
                    ┌─────────────────── SOURCE (immutable) ───────────────────┐
 mic + system ──▶ DualAudioCapture ──▶ chunk_*.wav ──▶ Whisper ──▶ transcript.jsonl
      │                    │                                              │
      │                    └──▶ LiveAudioFrame ──▶ LiveSttWorker ──▶ (ephemeral UI)
      └── recording clock — never blocked by anything below ──────────────┘
                                                                          │
                    ┌────────────── DERIVED (processing.json) ────────────▼────┐
                    │  normalize (deterministic)                               │
                    │      ↓                                                   │
                    │  speaker attribution (channel-based; diarization later)   │
                    │      ↓                                                   │
                    │  conversation (speaker-grouped, chronological)            │
                    │      ↓                                                   │
                    │  extraction — Stage A → MeetingFacts (JSON)               │
                    │      ↓                                                   │
                    │  summarization — Stage B → Markdown (mode + extension)    │
                    │      ↓                                                   │
                    │  validation → persist                                     │
                    └──────────────────────────┬───────────────────────────────┘
                                               ▼
              Summary · Conversation · Action Items · Topics · Entities
              · Meeting type · Related meetings · Scribble
```

Two artifacts per meeting, and the boundary between them is the whole point:

- `session.json` + `transcript.jsonl` + `audio/` — **source.** Written only by
  the recorder. The processing pipeline opens them read-only.
- `processing.json` + `processing_log.jsonl` — **derived.** Written only by the
  pipeline. Deleting it loses nothing that cannot be recomputed.

One canonical pipeline. Summary, decisions, action items, topics, entities,
meeting type, related meetings, and the Scribble are all projections of the
same `MeetingFacts`, not separate pipelines.

---

## 4. OUT-OF-SCOPE ISSUES

Documented, not fixed, per §2 of the brief.

### OUT-OF-SCOPE ISSUE 1 — channel provenance is too coarse for turn-level speakers

**Location** `native/src-tauri/src/meetings_v2/capture.rs` (`soft_mix`, chunk
assembly around L620–640)

**Problem** Mic and system audio are soft-mixed into one stream, and the only
surviving channel information is `mic_had_audio` / `sys_had_audio` — two
booleans covering a whole 30-second chunk. In a real two-way conversation most
30-second windows contain both sources, so chunk-level channel attribution
resolves to "unknown" for the majority of segments.

**Why it matters** Rung 1 of
`Meeting-rules/meeting_speaker_identification.md` is described as mandatory
and always-on, and it is the cheapest reliable owner signal for action items.
At chunk granularity it only fully resolves solo stretches.

**Recommended follow-up** Have the chunk carry a per-second channel energy
track (e.g. `Vec<(f32, f32)>`, ~30 pairs per chunk) alongside the existing
booleans. This changes neither the WAV format, the chunk duration, nor the
recording clock — it only stops discarding a measurement capture already
takes. Attribution would then resolve at roughly sentence granularity.

**Handled in this round by** attributing at chunk granularity where the
channel is unambiguous, leaving `speaker_id = None` where it is not, and
saying so in the Conversation tab rather than guessing.

### OUT-OF-SCOPE ISSUE 2 — `LLMClient::complete` cannot report failure

**Location** `native/src-tauri/src/providers/mod.rs::LLMClient::complete`

**Problem** Every provider error is swallowed and replaced with
`heuristic_fallback`, returned as `Ok`. Callers cannot detect an outage.

**Why it matters** §25/§43/§55 of the brief require the pipeline to report
honestly whether a model ran, and validators must not treat filler output as
a model answer.

**Recommended follow-up** Return `Result` and let callers decide, or add a
`degraded: bool` field to `LLMResponse`. This affects the dictation and
scribble pipelines too, hence out of scope here.

**Handled in this round by** the meeting pipeline's provider adapter treating
`model == "heuristic-fallback"` as a failed LLM call, recording it as such,
and using the pipeline's own deterministic path instead. `providers/mod.rs` is
not modified.

### OUT-OF-SCOPE ISSUE 3 — no frontend test runner

**Location** `native/package.json`

**Problem** `rules/testing.md` mandates Vitest + React Testing Library, but
neither is installed and there is no `test` script or a single test file in
`native/src/`.

**Why it matters** §40–44 of the brief ask for fixture and behavior tests.

**Recommended follow-up** Add Vitest + RTL as a standalone change so the
dependency addition is reviewable on its own.

**Handled in this round by** putting all pipeline behavior tests in Rust
(`cargo test`), where the logic lives, and keeping new frontend code
presentational with the one piece of real logic (speaker-name resolution)
isolated in a pure, exported helper ready for a Vitest test once a runner
exists.

---

## 5. Stop conditions — checked, none triggered

| Condition | Verdict |
|---|---|
| Schema cannot represent raw + normalized | **No.** `transcript.jsonl` is already append-only; derived data goes to a new sibling file. No migration of existing meetings needed. |
| Meeting lifecycle assumes UI owns processing | **No.** `MeetingsV2Engine` owns recording; the UI only invokes commands and reconciles. Processing attaches as a separate post-recording concern. |
| LLM interface cannot support structured output | **No**, with a caveat. `complete(prompt, system)` returns free text, which is enough for a JSON contract plus tolerant parsing. It cannot report failure — see out-of-scope issue 2, worked around without modifying the provider. |
| Scribble architecture requires duplicated content | **No.** `Scribble` has `source_type` + `source_metadata: serde_json::Value`, and `SOURCE_TYPE_MEETING` already exists. Provenance by reference works. |
| Would require touching unrelated functionality | **No.** Voice notes, dictation, global STT, capture, the chunk format, and crash recovery are all left unmodified. The one additive change to a recording-path file is documented in §6. |

---

## 6. Files this round modifies in the recording path

Kept to the minimum, and additive only.

**`meetings_v2/types.rs` — `TranscriptSegment` gains two `#[serde(default)]`
fields** (`mic_had_audio`, `sys_had_audio`), and **`meetings_v2/worker.rs`
copies them from the `AudioChunk` it already holds.** No format change, no
clock change, no new work per chunk — the values are already computed and
currently discarded. Old `transcript.jsonl` lines deserialize unchanged
(defaulting to `false`, correctly read as "channel unknown"). This is the
minimum needed for any speaker attribution at all; without it the
pipeline has no channel signal whatsoever.

Everything else in `capture.rs`, `live_stt.rs`, and the recording clock is
untouched.
