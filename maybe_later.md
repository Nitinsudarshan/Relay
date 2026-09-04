# Maybe Later (Deferred Features & Architecture Backlog)

This document tracks deferred features, rejected/postponed UI patterns, and architecture concepts that have been set aside for future evaluation. 

> [!NOTE]
> When a feature, visual affordance, or architectural idea is identified as speculative, half-implemented, or deferred to keep the current surface clean and reliable, it **must** be documented here rather than left as ghost UI or commented-out code. See [`rules/maybe-later.md`](rules/maybe-later.md).

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
> **Delivered in 0.31.0**: diarization. `meetings_v2::diarize` clusters the
> stored chunk WAVs into distinct voices (rung 4 of
> `Meeting-rules/meeting_speaker_identification.md`), so a call with several
> remote participants no longer reports one `Speaker 1`. It runs on the mixed
> track rather than per-source tracks, using MFCC statistics and a pitch
> estimate rather than a neural embedding — see item 18 for what that costs.
>
> **Still outstanding**: meeting detection and auto-end; per-source audio
> tracks; a voice library that matches a speaker across meetings; and the
> acoustic-echo and dual-recorder edge cases below.
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

### 7. Semantic Retrieval for Talkback (Embeddings + Hybrid Scoring)

- **Status**: Deferred (V2) — the seam exists, the dependency does not
- **Area**: Native backend (`native/src-tauri/src/talkback/retrieval.rs::score_candidate`)
- **Original Context**:
  - `docs/decisions.md` Decision 6 committed Relay to embedded LanceDB vector search in 2024. It was never built: there is no LanceDB dependency in `Cargo.toml` and no embedding code anywhere in the repository. The README claimed it until v0.18.0.
  - Talkback ships lexical retrieval instead — IDF-weighted scoring with source weighting, recency decay and one-hop expansion — which is honest and, measured at 2.66 ms per query over 1,000 documents, not the latency bottleneck. What it cannot do is match "what did we decide about cost" against a note that only ever says "pricing".
- **Why it is deferred rather than done**:
  - `fastembed`/`ort` is the natural Rust path, but `ort` has an unresolved Windows packaging risk: Windows ships an older `onnxruntime.dll` in System32 that shadows the crate's, and the documented `copy-dylibs` workaround targets **binary** Cargo targets while Relay's crate is `staticlib`/`cdylib`/`rlib`. That has to be proven on a real Windows build before it gates a shipped feature.
  - Ollama's `/api/embed` needs no new native dependency at all and is the cheaper first experiment — but it needs an embedding model pulled and an index that is invalidated whenever any of four capture surfaces writes.
- **Concept & Implementation Blueprint**:
  - Replace the body of `retrieval::score_candidate` with `alpha * lexical + beta * cosine(embedding)`. Nothing else in the pipeline changes — `rank`, expansion, dedup and budgeting all operate on the score.
  - Start with Ollama `/api/embed` behind a feature flag so the Windows ONNX question does not block the experiment. Persist vectors beside the notes rather than in a new store, per the "no second knowledge base" rule.
  - Judge it with `search_talkback_context`, which already returns retrieval results with no model involved — that command exists precisely so retrieval quality can be measured rather than guessed.

### 8. Neural Turn Detection and Wake Word for Talkback

- **Status**: Deferred (V2) — architecture ready, models not shipped
- **Area**: Native backend (`native/src-tauri/src/talkback/turn.rs::TurnDetector::push`, `talkback::ActivationMode`)
- **Original Context**:
  - Talkback V1 detects turns with energy plus an adaptive noise floor and a 700 ms hangover. It works, and it is the same shape as the meeting live clock's speech flag, but it cannot tell a thinking pause from the end of a thought — so the hangover is a compromise between cutting people off and feeling sluggish.
  - `ActivationMode::WakeWord` is a real settings value that the engine refuses with a clear message. No always-on listener ships, deliberately.
- **Concept & Implementation Blueprint**:
  - **Silero VAD** (MIT, code and weights; ONNX; ~1 ms per 30 ms frame on one CPU thread; mature Rust ports in `silero-vad-rs` and `voice_activity_detector`) replaces the energy gate for speech/non-speech.
  - **Pipecat Smart Turn v3** (open weights, open training data, ~8M parameters, <60 ms CPU) answers the harder question — has this person *finished* — from the waveform rather than the transcript. That is what removes the hangover tradeoff.
  - **openWakeWord** / **microWakeWord** (both Apache-2.0) for `wake_word` activation. Both are Python/MCU-targeted today, so the realistic path is their ONNX exports, not the frameworks.
  - All three land through the same seam: `TurnDetector::push` takes a frame and returns a `TurnEvent`. Same signature, better decision.
- **Blocked on**: the same `ort`-on-Windows proof as item 7. One packaging spike unblocks both.

### 9. A Second Local TTS Provider for Talkback (Kokoro)

- **Status**: Deferred (V2) — trait shipped, provider not
- **Area**: Native backend (`native/src-tauri/src/tts/`)
- **Original Context**:
  - `tts::TtsProvider` exists with Piper behind it, so a second engine is an addition rather than a refactor. What has *not* happened is the benchmark: Piper, Kokoro and Chatterbox were researched but not measured, because measuring them in a Linux CI container says nothing about a Windows laptop (`docs/talkback/BENCHMARKS.md`).
  - Piper also needs attention independently: `rhasspy/piper` was archived read-only in October 2025 and active work moved to `OHF-Voice/piper1-gpl` (GPL-3.0 — compatible with Relay's AGPL-3.0, and irrelevant anyway since Relay shells out to a separate process). The settings copy and doc comments should point at the maintained binary.
- **Concept & Implementation Blueprint**:
  - **Kokoro-82M** is the leading candidate: Apache-2.0 for both code *and* weights (so it is redistributable with a desktop installer), ~350 MB, 8 languages including Hindi, and several Rust ONNX ports (`Kokoros`, `kokoroxide`, `kokoro-en`, `tts-rs`) with chunked streaming.
  - **Chatterbox is not a candidate** despite being MIT and sounding better: it needs a PyTorch runtime, which disqualifies it as a Tauri dependency regardless of quality.
  - Benchmark on real Windows hardware before adopting: time-to-first-audio, total synthesis, CPU, RAM, English, Hindi, mixed-language speech, startup time, packaging size, and interruptibility. Piper stays the default until a measurement says otherwise — being newer is not evidence.

### 10. Acoustic Echo Cancellation for Talkback Barge-In

- **Status**: Deferred — mitigated, not solved
- **Area**: Native backend (`native/src-tauri/src/talkback/turn.rs`)
- **Original Context**:
  - Talkback plays its answer through the speakers while its microphone is open, so on a laptop without headphones the microphone hears the agent and can interrupt it with its own voice.
  - The current mitigation is an echo guard: while the agent speaks, the speech threshold is multiplied (2.5×), so a barge-in must be clearly louder than the agent. This works well enough at normal volumes and is honest about its limits — the Settings copy recommends headphones.
- **Concept & Implementation Blueprint**:
  - Real AEC needs the playback signal as a reference, which means playback would have to move into Rust — currently blocked by the `rodio`/`cpal 0.17` conflict recorded in `docs/decisions.md` Decision 49.
  - `webrtc-audio-processing` bindings are the usual answer, at the cost of a C++ dependency and a Windows build story.
  - A cheaper intermediate step: have the frontend report playback amplitude back to the engine so the echo guard scales with actual output level rather than using a fixed multiplier.

### 11. Move Relay's Config and Vault Root Off `current_dir()`

- **Status**: Deferred — a migration, not a feature
- **Area**: Native backend (`native/src-tauri/src/lib.rs`)
- **Original Context**:
  - `base_dir` is `std::env::current_dir()/.relay`, and the vault, `settings.json` and the Whisper models directory all hang off it.
  - Beside a development checkout this is fine and convenient. In a packaged Windows app it is not: launched from a Start Menu shortcut `current_dir()` is typically `C:\Windows\System32`; launched from its install directory it is under `Program Files`. Neither is writable by a standard user, and the location changes depending on how Relay was started — so settings can silently fail to save, or save somewhere that is not found next launch.
  - v0.18.1 fixed this for the *voice installation only* (`docs/decisions.md` Decision 52), because that subsystem prints its folder to the user as an instruction and is new state with nothing to migrate.
- **Why it is deferred rather than done**:
  - Changing `base_dir` relocates every existing user's vault, notes, Kanban cards, meetings and settings. Getting that wrong loses data, and getting it right means a detection-and-migration path with its own testing.
  - It is not a Talkback problem, and bundling it into a Talkback change would hide a data migration inside a feature diff.
- **Concept & Implementation Blueprint**:
  - Resolve `base_dir` from the OS application-data directory, as `tts::discovery::default_tts_root` already does.
  - On startup, if the new location is empty and a process-relative `.relay` exists beside the executable, offer to move it — explicitly, with the old copy left in place, matching the "never move, migrate or delete" promise the Vault Directory Location setting already makes (`docs/decisions.md` Decision 38).
  - The configurable Vault Directory Location setting already overrides this for the vault, so the migration mainly concerns `config/` — settings, models, and the STT cache.

### 12. Offline and ARM64 Voice Installation

- **Status**: Deferred — a packaging decision, not missing code
- **Area**: Native backend (`native/src-tauri/src/tts/{manifest,installer}.rs`, `native/src-tauri/resources/voice-manifest.json`)
- **Original Context**:
  - v0.19.0 installs the local voice by downloading it. Two users are therefore still stuck: one on a machine with no internet access, and one on Windows on ARM, for which the manifest carries no runtime and the installer reports "Automatic voice setup isn't available for aarch64 yet" instead of guessing.
  - Both are deliberate. The engine and a voice are roughly 80–100 MB per architecture; bundling them into the installer for every user so that a minority never has to download would triple the download for everyone, and shipping an x86-64 binary to an ARM machine would fail *after* setup claimed success rather than before it started.
- **Concept & Implementation Blueprint**:
  - **ARM64** is the smaller job: add a `piper-windows-aarch64` entry to the manifest once an upstream release asset exists for it. `runtime_for()` already distinguishes an unsupported architecture from an unsupported platform, and nothing else changes — the manifest is the only place a runtime is named.
  - **Offline** is a packaging change, not a code change: `discovery` already looks in Tauri's resource directory (the `Bundled` origin) before falling back to `PATH`, so an offline or enterprise build can ship the engine and the recommended voice as bundled resources and the installer will find them already present and skip the download. What is missing is the build variant and the release pipeline that produces it, not the lookup.
  - A third option, worth considering before either: let the user point setup at a folder they copied from another machine, verified against the same manifest checksums. Cheap, and it covers the air-gapped case without a second build.

### 13. Capturing a Web Page From Relay's Own Hotkey

- **Status**: Deferred — blocked by the browser permission model, not by missing code
- **Area**: Native backend (`native/src-tauri/src/capture/web/bridge.rs`, `hotkeys/mod.rs`), browser extension (`native/src/webcapture/background.ts`)
- **Original Context**:
  - The capture feature was asked for as an OS-level Relay hotkey that reads whatever page the user is looking at. It ships the other way round: the browser owns the trigger, and Relay's `capture_hotkey` opens the Captures surface (`docs/decisions.md` Decision 57).
  - The blocker is `activeTab`. Chrome grants it only in response to a gesture made inside the browser — the extension's action, a context-menu item, a `commands` shortcut, or an omnibox suggestion. A global desktop hotkey is none of those, so an extension asked to capture "from outside" has no permission to read the tab.
- **Why it is deferred rather than done**:
  - The only way to make it work today is `<all_urls>` (or `optional_host_permissions` the user grants once), which trades the entire least-privilege story — the thing that makes this feature safe to install — for the convenience of one shortcut.
  - Even with the permission, the desktop→browser direction is unreliable: an MV3 service worker is terminated when idle, so Relay cannot simply push a request to it. Keeping one alive means holding a port open, which in practice means native messaging.
- **Concept & Implementation Blueprint**:
  - Switch the transport to native messaging (`runtime.connectNative`), which keeps the service worker alive for the life of the port and gives Relay a channel it can *initiate* on. That is the same migration Decision 58 already names as the exit from the loopback bridge, so the two would land together.
  - Add `optional_host_permissions: ["<all_urls>"]` to the manifest and request it from the options page, with the trade stated plainly: "Relay can capture from its own shortcut, but the extension will be able to read any page you visit."
  - Keep the current behaviour as the default and the fallback: if the permission is not granted, the desktop hotkey opens Captures and shows the browser shortcut, exactly as it does now.

### 14. Screenshot and OCR Fallback for Unreadable Pages

- **Status**: Deferred — deliberately not a rung on the capture ladder
- **Area**: Native backend (`native/src-tauri/src/capture/web/`), browser extension
- **Original Context**:
  - The capture ladder ends at `text_only` (the page's visible text) and then refuses. A canvas-rendered app, a scanned document in a viewer, or a page that renders entirely into shadow roots produces nothing, and Relay says so rather than saving an empty artifact.
  - A screenshot plus OCR was considered as a fifth rung and rejected for v1: the product principle is structured acquisition, and the cases it would serve are already labelled honestly rather than silently mis-captured.
- **Concept & Implementation Blueprint**:
  - `chrome.tabs.captureVisibleTab` needs the `activeTab` grant Relay already has, so the browser side is small — but it captures the *viewport*, not the page, which means the artifact would be less complete than the fallback it sits below unless scroll-and-stitch is built too.
  - OCR is the real cost: a Rust OCR engine (Tesseract bindings or an ONNX model) is a large native dependency with a Windows build story, for output that is lower fidelity than every rung above it.
  - If it is built, it must be a *supplementary* artifact attached to a capture — `ScribbleAttachment` already exists — and never a substitute that lets `coverage` claim more than the text extraction earned.

### 15. Firefox Support for the Capture Extension

- **Status**: Deferred — structurally compatible, not validated
- **Area**: `native/browser-extension/manifest.json`, `native/src/webcapture/background.ts`
- **Original Context**:
  - The extension targets Chrome and Edge. Firefox supports `scripting.executeScript` under MV3 from Firefox 101 and grants `activeTab` on the same gestures, so nothing in the extraction layer is Chrome-specific — but the manifest ships Chrome shapes and no Firefox testing was done, so it is not claimed as supported (`docs/capture.md` §13).
- **Concept & Implementation Blueprint**:
  - Add `browser_specific_settings.gecko.id` and, on older Firefox, a `background.scripts` event page instead of `background.service_worker`; the build already emits both an ES module and an IIFE, so no new bundling is required.
  - Firefox treats `host_permissions` as optional by default, so pairing must handle "granted later" — the options page's **Save and test** is the natural place to call `permissions.request`.
  - The bridge already accepts `moz-extension://` origins, and there is a test asserting it.

### 16. Capturing File Bytes and Image Data

- **Status**: Deferred — technically possible, deliberately refused for v2
- **Area**: `native/src/webcapture/dom.ts`, `native/src/webcapture/types.ts`, `native/src-tauri/src/capture/web/`
- **Original Context**:
  - Capture v2 records files and images as metadata plus a reference, with `content_captured: false` and a note saying the file itself was not retrieved. It is not a limitation of the browser: a content script's `fetch` runs with the page's cookies, so an authenticated asset URL (ChatGPT's `backend-api/estuary/content`, a GitHub attachment) *would* resolve, and at least one published exporter does exactly that.
  - Four reasons it was refused rather than postponed by accident, all recorded in `docs/capture/RESEARCH.md` §6.1: the gesture was "capture this page", not "download every file it references"; the payload contract is text-only, which is what lets normalization be a total function over untrusted input, and the 8 MiB body limit would be spent by two screenshots; fetching authenticated asset URLs makes Relay a client of the site's API, a materially larger claim for a least-privilege feature; and metadata-now/bytes-later loses nothing because the reference is preserved, while shipping downloads and retracting them is not available.
- **Concept & Implementation Blueprint**:
  - Gate it on an explicit, off-by-default setting with its own consent copy — "download files referenced by captured pages" — rather than folding it into "enable capture".
  - Do it in the **service worker**, not the content script: the worker already holds the only network permission the extension has, and keeping fetches out of the page context keeps the isolated-world guarantee intact. The content script would hand over a list of references, and the worker would decide what to fetch.
  - Store bytes beside the artifact (`vault/captures/<id>/files/`) rather than in the payload, so the text-only contract survives; the block gains a `stored_path` and `content_captured` becomes a real measurement instead of a constant.
  - Needs its own limits (per-file and per-capture ceilings, an allowlist of types, a same-origin-as-the-page rule) and its own honesty: a file that was fetched and rejected by a limit must say so, exactly as `content_note` does now.
  - `sandbox:/mnt/data/…` references are not fetchable at all and would stay metadata-only regardless.

### 17. A Configurable Traversal Budget

- **Status**: Deferred — the budget is a constant per source, and hitting it is reported
- **Area**: `native/src/webcapture/traversal/budget.ts`, `native/src-tauri/src/settings/`, `native/src/components/settings/CaptureSettingsView.tsx`
- **Original Context**:
  - The reveal pass stops after 10 seconds on a conversation (6 on a document) and reports `time_budget` with `partial` coverage. Measured against the validation fixtures, that covers roughly 450–500 turns (`docs/capture/BENCHMARKS.md`), so a genuinely enormous thread is captured incompletely — honestly, but incompletely — and the user has no way to say "take longer, I'll wait".
  - It was left constant because the extension cannot read Relay's settings without another round-trip to the bridge, and adding one to the capture path for a value that is right in almost every case is a poor trade.
- **Concept & Implementation Blueprint**:
  - The bridge's `/v1/health` response is the natural carrier: the service worker already calls it during pairing, so the budget could ride along and be cached in `chrome.storage.local` rather than fetched per capture.
  - Expose it as a choice rather than a number — "quick", "thorough", "as long as it takes" — mapping to `maxMs`/`maxSteps` pairs, because a millisecond field invites a value that makes capture feel broken.
  - Whatever is chosen, `termination` and the completeness verdict must keep working the same way: a longer budget may not change what a capture is *allowed to claim*, only how much it manages to read.

### 18. Neural Speaker Embeddings and a Voice Library

- **Status**: Backlog (diarization shipped in 0.31.0 without them)
- **Area**: Native backend (`native/src-tauri/src/meetings_v2/diarize/*`)
- **Original Context**:
  - 0.31.0 shipped rung 4 of `Meeting-rules/meeting_speaker_identification.md`
    using classical features: MFCC mean and standard deviation over each
    utterance, plus a median pitch estimate, clustered agglomeratively with the
    speaker count read off the merge sequence.
  - That was a deliberate trade. A neural speaker embedding (x-vector, ECAPA)
    is materially better, and it means shipping an ONNX runtime, a model
    download with its own licence question, and the consent flow §6 of the
    speaker rules requires — because an embedding stored across meetings *is*
    biometric data, which the classical path never creates.
  - What the classical features do not buy is written into the module's own
    docs: telling two similar voices apart on one channel, and matching a voice
    across meetings. `DiarizationReport::well_separated` exists so the UI can
    say which of those situations it is in rather than presenting a guess.
- **Concept & Implementation Blueprint**:
  - Add an ONNX runtime behind a feature flag and a settings toggle that is
    **off** by default, alongside the existing "Separate individual speakers".
  - Replace `features::VoiceFeatures::vector` with the embedding, keeping the
    same `cluster::Utterance` shape so the clusterer and the merge-sequence
    stopping rule are unchanged. Recalibrate `MIN_SPLIT_DISTANCE` against the
    embedding's own scale — the constant's doc comment records how the current
    value was measured, and the same method applies.
  - Only then build the voice library (rung 2): enrolment, a "Remember voices
    across meetings" toggle that must never default on, and the management UI
    §6 requires — list, rename, merge, delete. Nothing in 0.31.0 persists a
    voice, so there is no migration to do first.
  - Per-source audio tracks (item 3) would help independently: diarizing the
    system-audio stream alone removes the local user's voice from the clustering
    problem entirely.

### 19. Calendar Sync for Meeting Attendees and Titles

- **Status**: Deferred — no calendar integration exists
- **Area**: Native backend (`native/src-tauri/src/meetings_v2/processing/metadata.rs`, `native/src-tauri/src/oauth/*`)
- **Original Context**:
  - Rung 3 of `Meeting-rules/meeting_speaker_identification.md` is "calendar
    attendees + conferencing display names", and §2.2 specifies that a
    recording's expected-speaker count should default to the calendar attendee
    count. Neither is built.
  - 0.31.0 built the surface those would fill: `MeetingMetadata` carries a
    participant list with a `ParticipantOrigin`, and `Stated` already covers "a
    name a person supplied". A calendar attendee is the same shape with a
    different origin, and the expected-speaker hint is already a parameter on
    `diarize_session`.
  - Google OAuth exists in `oauth/` for Drive, so the authorization half is not
    starting from nothing.
- **Concept & Implementation Blueprint**:
  - Add a `ParticipantOrigin::Calendar` and populate participants from the
    event whose window contains the recording's start time. Match an attendee
    name to a diarization cluster only where exactly one candidate fits, which
    is what rung 3 requires — several candidates means no claim.
  - Default `expected_speakers` to the attendee count when an event matched,
    leaving the user's explicit setting to win over it.
  - Use the event title as the meeting title only when the user has not typed
    one, and never overwrite a title the extraction stage produced.
  - Treat the calendar as external source material, per `rules/security.md`: an
    event description is data, never an instruction to Relay's AI.
