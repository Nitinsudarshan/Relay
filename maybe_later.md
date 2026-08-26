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

- **Status**: Backlog (identified during the Meetings V2 review; deliberately out of scope of the 0.12.0 latency/pill work)
- **Area**: Native App (`native/src-tauri/src/meetings_v2/*`, `native/src-tauri/src/vault/scribble.rs`, `native/src/components/meetings_v2/*`)
- **Original Context**:
  - `meetings_implementation.md` planned a detection and reminder architecture (§4.1, §4.2) that the V2 rebuild does not implement: there is no calendar-proximity, meeting-process, or sustained-audio trigger, and no auto-end. A meeting today is started and stopped by hand.
  - `vault/scribble.rs` already defines `SOURCE_TYPE_MEETING` and handles `meeting_id` / `meeting_title` source metadata in the knowledge graph, but nothing ever produces it — meetings live in `vault/meetings_v2/<id>/`, outside the notes/scribbles/trash model, so they are unsearchable, un-enriched, and cannot be promoted to a Scribble.
  - Microphone and system audio are soft-mixed to one mono track before anything is persisted, so no per-source track exists and diarization is not expressible.
- **Concept & Implementation Blueprint**:
  - **Detection**: one service normalizing calendar, process, sustained-audio, and manual signals into `meeting-detected` / `meeting-ended` events, deduplicated by a fingerprint that is explicitly not identity. Detection must never start recording directly — it prompts, and the session manager decides.
  - **Auto-end**: a state machine over process exit, calendar end time, and audio activity with a cancellable countdown, always fenced by the active `sessionId` (the fencing helper added in 0.12.0 is the hook for this).
  - **Per-source tracks**: persist mic and system audio as separate chunk streams and keep the mix as a derived artifact. This is a storage-format change and gets more expensive the longer mixed-only sessions accumulate, so it should precede any diarization work.
  - **Diarization**: after stop, pick the source by whether system audio was actually heard (`sys_audio_heard`, added in 0.12.0), diarize that one, and store speaker identity separately from transcript text so renaming a speaker never rewrites the transcript.
  - **Vault integration**: give a completed meeting a note of type `meeting` and let a Scribble *reference* the session rather than copying a transcript snapshot, so merges and re-transcription reconcile instead of going stale. Route deletion through the 30-day trash that scribbles and voice notes already use, instead of the current immediate `remove_dir_all`.
  - Edge cases: acoustic echo (remote speech arrives both over loopback and back into the mic, currently counted twice), platform differences in loopback availability, and meetings that start while a dictation recording is already active — nothing currently arbitrates between the two recorders.
