# Maybe Later (Deferred Features & Architecture Backlog)

This document tracks deferred features, rejected/postponed UI patterns, and architecture concepts that have been set aside for future evaluation. 

> [!NOTE]
> When a feature, visual affordance, or architectural idea is identified as speculative, half-implemented, or deferred to keep the current surface clean and reliable, it **must** be documented here rather than left as ghost UI or commented-out code. See [Rules/maybe-later.md](file:///d:/Projects/Relay/Rules/maybe-later.md).

---

## Backlog Items

### 1. In-App Navigation Keyboard Shortcuts (`Alt + 1..4` / `Ctrl + 1..4`)

- **Status**: Deferred (UI indicators removed from Sidenav)
- **Area**: Native App (`native/src/components/common/NativeSidebar.tsx`, `native/src/App.tsx`)
- **Original Context**:
  - The Sidenav previously displayed `⌥1`, `⌥2`, `⌥3`, and `⌥4` next to navigation items ("Voice Note", "Meetings", "Scribbles", "Settings").
  - However, no corresponding `keydown` listeners or Tauri global shortcuts were attached, meaning pressing `Alt + Number` or `Option + Number` did nothing.
  - Furthermore, using the Mac Option symbol (`⌥`) in a Windows-first desktop app created platform inconsistency.
- **Concept & Implementation Blueprint**:
  - Implement a dedicated hook (e.g., `useNavigationShortcuts`) or a `keydown` listener in `App.tsx`.
  - Listen for `Alt + 1` (Voice Note), `Alt + 2` (Meetings), `Alt + 3` (Scribbles), `Alt + 4` (Settings).
  - Ensure shortcuts are bypassed when user focus is inside text fields, textareas, code editors, or modals (`event.target.tagName !== 'INPUT' && event.target.tagName !== 'TEXTAREA'`).
  - Provide platform-accurate visual shortcut badges (`Alt+1` on Windows / Linux, `⌥1` on macOS) or make them configurable in Settings > Hotkeys.

### 2. Post-Migration Legal & Governance Hardening
- **Status**: Deferred (Post-AGPL Migration)
- **Area**: Governance & Legal (`TRADEMARKS.md`, `CONTRIBUTING.md`, `NOTICE`)
- **Concept & Items**:
  - **Formal Trademark / Branding Policy (`TRADEMARKS.md`)**: Establish explicit branding guidelines distinguishing AGPL software licensing from official Relay trademark rights for forks and derivatives.
  - **Contributor License / CLA Policy (`CONTRIBUTING.md`)**: Define contributor guidelines and CLA / DCO expectations before accepting external pull requests.
  - **Dual-Licensing Evaluation**: Evaluate whether to maintain AGPL-only core or establish an AGPL + commercial dual-licensing structure for official cloud/enterprise offerings.
  - **Third-Party Attribution Notice (`NOTICE`)**: Consolidate upstream open-source attribution notices if the asset/crate footprint expands.


### 3. Meeting Detection, Diarization & Vault Integration

- **Status**: Partly delivered in 0.13.0 — the remainder is Backlog

> [!NOTE]
> **Delivered in 0.13.0** (see `docs/meetings/MEETINGS_INTELLIGENCE_AUDIT.md`):
> the vault-integration half of this item. A completed meeting can now become a
> Scribble that *references* the meeting (`source_type = "meeting"`,
> `source_id`) rather than copying a snapshot, reusing the existing Scribble
> type and vault; speaker identity is stored separately from transcript text, so
> renaming a speaker rewrites nothing; and deletion already routed through the
> 30-day trash. Channel-based attribution (mic = the local user, system audio =
> everyone else) is live, using the per-chunk flags the recorder already
> measured.
>
> **Still outstanding**: meeting detection and auto-end; per-source audio
> tracks; real diarization; and the acoustic-echo and dual-recorder edge cases
> below. The per-source-track work is now the gating item — see item 4.
- **Area**: Native App (`native/src-tauri/src/meetings_v2/*`, `native/src-tauri/src/vault/scribble.rs`, `native/src/components/meetings_v2/*`)
- **Original Context**:
  - The pre-V2 meetings plan (removed once V2 shipped; see CHANGELOG v0.14.0) specified a detection and reminder architecture that the V2 rebuild does not implement: there is no calendar-proximity, meeting-process, or sustained-audio trigger, and no auto-end. A meeting today is started and stopped by hand.
  - `vault/scribble.rs` already defines `SOURCE_TYPE_MEETING` and handles `meeting_id` / `meeting_title` source metadata in the knowledge graph, but nothing ever produces it — meetings live in `vault/meetings_v2/<id>/`, outside the notes/scribbles/trash model, so they are unsearchable, un-enriched, and cannot be promoted to a Scribble.
  - Microphone and system audio are soft-mixed to one mono track before anything is persisted, so no per-source track exists and diarization is not expressible.
- **Concept & Implementation Blueprint**:
  - **Detection**: one service normalizing calendar, process, sustained-audio, and manual signals into `meeting-detected` / `meeting-ended` events, deduplicated by a fingerprint that is explicitly not identity. Detection must never start recording directly — it prompts, and the session manager decides.
  - **Auto-end**: a state machine over process exit, calendar end time, and audio activity with a cancellable countdown, always fenced by the active `sessionId` (the fencing helper added in 0.12.0 is the hook for this).
  - **Per-source tracks**: persist mic and system audio as separate chunk streams and keep the mix as a derived artifact. This is a storage-format change and gets more expensive the longer mixed-only sessions accumulate, so it should precede any diarization work.
  - **Diarization**: after stop, pick the source by whether system audio was actually heard (`sys_audio_heard`, added in 0.12.0), diarize that one, and store speaker identity separately from transcript text so renaming a speaker never rewrites the transcript.
  - **Vault integration**: give a completed meeting a note of type `meeting` and let a Scribble *reference* the session rather than copying a transcript snapshot, so merges and re-transcription reconcile instead of going stale. Route deletion through the 30-day trash that scribbles and voice notes already use, instead of the current immediate `remove_dir_all`.
  - Edge cases: acoustic echo (remote speech arrives both over loopback and back into the mic, currently counted twice), platform differences in loopback availability, and meetings that start while a dictation recording is already active — nothing currently arbitrates between the two recorders.

### 4. Per-Second Channel Provenance for Turn-Level Speaker Attribution

- **Status**: Backlog (documented as out-of-scope issue 1 during the 0.13.0 meetings work)
- **Area**: Native Rust Backend (`native/src-tauri/src/meetings_v2/capture.rs`)
- **Original Context**:
  - Mic and system audio are soft-mixed into one 16 kHz mono stream, and the only channel information that survives is `AudioChunk::mic_had_audio` / `sys_had_audio` — two booleans covering a whole 30-second chunk. 0.13.0 persists these onto each `TranscriptSegment` and uses them for rung-1 speaker attribution.
  - At chunk granularity that only fully resolves solo stretches. In a real two-way conversation most 30-second windows contain both sources, so those segments are left deliberately unattributed rather than guessed, and the Conversation tab says so.
  - Rung 1 of `Meeting-rules/meeting_speaker_identification.md` is described as mandatory and always-on, and it is the cheapest reliable owner signal for action items — so its resolution matters more than the other rungs.
- **Concept & Implementation Blueprint**:
  - Have each chunk carry a per-second channel energy track (e.g. `Vec<(f32, f32)>`, ~30 pairs per chunk) alongside the existing booleans, and persist it beside the raw transcript segment.
  - This changes neither the WAV format, the chunk duration, nor the recording clock — it only stops discarding a measurement `capture.rs` already takes in its metering loop.
  - Attribution would then resolve at roughly sentence granularity: split a normalized segment at channel-dominance boundaries and assign each part. `meetings_v2::processing::speakers` is the only consumer and already models `speaker_id` as per-segment and optional, so nothing downstream changes shape.
  - Deliberately kept separate from item 3's full per-source *tracks*, which are a storage-format change. This is a metadata change and is the cheaper first step.

### 5. Honest Failure Reporting from `LLMClient::complete`

- **Status**: Backlog (documented as out-of-scope issue 2 during the 0.13.0 meetings work)
- **Area**: Native Rust Backend (`native/src-tauri/src/providers/mod.rs`)
- **Original Context**:
  - `LLMClient::complete` never returns `Err`. On any provider failure it logs a warning and returns `heuristic_fallback(...)` — canned filler tagged `model: "heuristic-fallback"` — as `Ok`. A caller cannot distinguish a model answer from an outage except by inspecting the model string.
  - That is defensible for dictation, where some output beats none, but it means a caller cannot report honestly whether a model ran, and a validator can end up judging text no model wrote.
  - 0.13.0 works around it for meetings only: `meetings_v2::processing::llm::ProviderLlm` treats the marker as the failure it is, so the meeting pipeline records the truth and chooses its own deterministic path. `providers/mod.rs` was left untouched because the dictation and scribble pipelines depend on the current behavior.
- **Concept & Implementation Blueprint**:
  - Either return `Result<LLMResponse, ProviderError>` from `complete` and let each caller decide whether to fall back, or add a `degraded: bool` to `LLMResponse` so the substitution is visible without changing the signature.
  - Then remove the marker check in `ProviderLlm` and have callers that want filler ask for it explicitly.
  - Touches the dictation and scribble paths, so it wants its own change and its own test pass rather than riding along with a meetings change.

### 6. Frontend Test Runner (Vitest + React Testing Library)

- **Status**: Backlog (documented as out-of-scope issue 3 during the 0.13.0 meetings work)
- **Area**: Native App (`native/package.json`, `native/src/**`)
- **Original Context**:
  - `rules/testing.md` mandates Vitest + React Testing Library for both frontends, but neither is installed, there is no `test` script, and `native/src/` contains no test file.
  - The 0.13.0 meetings work therefore put all of its behavior tests in Rust (`cargo test`), where the pipeline logic lives, and kept the new frontend code presentational.
- **Concept & Implementation Blueprint**:
  - Add Vitest + RTL + `jsdom` to `native/` as a standalone change, so the dependency addition is reviewable on its own rather than buried in a feature diff.
  - First targets, per `rules/testing.md`'s priority order: `native/src/components/meetings_v2/meetingProcessing.ts` (speaker-name and owner resolution — pure functions, already isolated for this), then the trigger-phrase config form's validation logic.
  - Do not retrofit tests onto presentational components with no branching logic.
