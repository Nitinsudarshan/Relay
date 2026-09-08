# Maybe Later (Deferred Features & Architecture Backlog)

This document tracks deferred features, rejected/postponed UI patterns, and architecture concepts that have been set aside for future evaluation. 

> [!NOTE]
> When a feature, visual affordance, or architectural idea is identified as speculative, half-implemented, or deferred to keep the current surface clean and reliable, it **must** be documented here rather than left as ghost UI or commented-out code. See [`rules/maybe-later.md`](rules/maybe-later.md).

---

## Backlog Items

### 1. Neural Turn Detection and Wake Word for Talkback

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
- **Blocked on**: the `ort`-on-Windows packaging proof. That spike is now written — `docs/spikes/onnx-windows.md`, behind the `onnx-spike` cargo feature — and has never been run; it needs a Windows machine and half an hour. One run unblocks this item and G6 denoise together.

### 2. A Second Local TTS Provider for Talkback (Kokoro)

- **Status**: Deferred (V2) — trait shipped, provider not
- **Area**: Native backend (`native/src-tauri/src/tts/`)
- **Original Context**:
  - `tts::TtsProvider` exists with Piper behind it, so a second engine is an addition rather than a refactor. What has *not* happened is the benchmark: Piper, Kokoro and Chatterbox were researched but not measured, because measuring them in a Linux CI container says nothing about a Windows laptop (`docs/talkback/BENCHMARKS.md`).
  - Piper also needs attention independently: `rhasspy/piper` was archived read-only in October 2025 and active work moved to `OHF-Voice/piper1-gpl` (GPL-3.0 — compatible with Relay's AGPL-3.0, and irrelevant anyway since Relay shells out to a separate process). The settings copy and doc comments should point at the maintained binary.
- **Concept & Implementation Blueprint**:
  - **Kokoro-82M** is the leading candidate: Apache-2.0 for both code *and* weights (so it is redistributable with a desktop installer), ~350 MB, 8 languages including Hindi, and several Rust ONNX ports (`Kokoros`, `kokoroxide`, `kokoro-en`, `tts-rs`) with chunked streaming.
  - **Chatterbox is not a candidate** despite being MIT and sounding better: it needs a PyTorch runtime, which disqualifies it as a Tauri dependency regardless of quality.
  - Benchmark on real Windows hardware before adopting: time-to-first-audio, total synthesis, CPU, RAM, English, Hindi, mixed-language speech, startup time, packaging size, and interruptibility. Piper stays the default until a measurement says otherwise — being newer is not evidence.

### 3. Acoustic Echo Cancellation for Talkback Barge-In

- **Status**: Deferred — mitigated, not solved
- **Area**: Native backend (`native/src-tauri/src/talkback/turn.rs`)
- **Original Context**:
  - Talkback plays its answer through the speakers while its microphone is open, so on a laptop without headphones the microphone hears the agent and can interrupt it with its own voice.
  - The current mitigation is an echo guard: while the agent speaks, the speech threshold is multiplied (2.5×), so a barge-in must be clearly louder than the agent. This works well enough at normal volumes and is honest about its limits — the Settings copy recommends headphones.
- **Concept & Implementation Blueprint**:
  - Real AEC needs the playback signal as a reference, which means playback would have to move into Rust — currently blocked by the `rodio`/`cpal 0.17` conflict recorded in `docs/decisions.md` Decision 49.
  - `webrtc-audio-processing` bindings are the usual answer, at the cost of a C++ dependency and a Windows build story.
  - A cheaper intermediate step: have the frontend report playback amplitude back to the engine so the echo guard scales with actual output level rather than using a fixed multiplier.

### 4. Move Relay's Config and Vault Root Off `current_dir()`

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

### 5. Offline and ARM64 Voice Installation

- **Status**: Deferred — a packaging decision, not missing code
- **Area**: Native backend (`native/src-tauri/src/tts/{manifest,installer}.rs`, `native/src-tauri/resources/voice-manifest.json`)
- **Original Context**:
  - v0.19.0 installs the local voice by downloading it. Two users are therefore still stuck: one on a machine with no internet access, and one on Windows on ARM, for which the manifest carries no runtime and the installer reports "Automatic voice setup isn't available for aarch64 yet" instead of guessing.
  - Both are deliberate. The engine and a voice are roughly 80–100 MB per architecture; bundling them into the installer for every user so that a minority never has to download would triple the download for everyone, and shipping an x86-64 binary to an ARM machine would fail *after* setup claimed success rather than before it started.
- **Concept & Implementation Blueprint**:
  - **ARM64** is the smaller job: add a `piper-windows-aarch64` entry to the manifest once an upstream release asset exists for it. `runtime_for()` already distinguishes an unsupported architecture from an unsupported platform, and nothing else changes — the manifest is the only place a runtime is named.
  - **Offline** is a packaging change, not a code change: `discovery` already looks in Tauri's resource directory (the `Bundled` origin) before falling back to `PATH`, so an offline or enterprise build can ship the engine and the recommended voice as bundled resources and the installer will find them already present and skip the download. What is missing is the build variant and the release pipeline that produces it, not the lookup.
  - A third option, worth considering before either: let the user point setup at a folder they copied from another machine, verified against the same manifest checksums. Cheap, and it covers the air-gapped case without a second build.

### 6. Capturing a Web Page From Relay's Own Hotkey

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

### 7. Screenshot and OCR Fallback for Unreadable Pages

- **Status**: Deferred — deliberately not a rung on the capture ladder
- **Area**: Native backend (`native/src-tauri/src/capture/web/`), browser extension
- **Original Context**:
  - The capture ladder ends at `text_only` (the page's visible text) and then refuses. A canvas-rendered app, a scanned document in a viewer, or a page that renders entirely into shadow roots produces nothing, and Relay says so rather than saving an empty artifact.
  - A screenshot plus OCR was considered as a fifth rung and rejected for v1: the product principle is structured acquisition, and the cases it would serve are already labelled honestly rather than silently mis-captured.
- **Concept & Implementation Blueprint**:
  - `chrome.tabs.captureVisibleTab` needs the `activeTab` grant Relay already has, so the browser side is small — but it captures the *viewport*, not the page, which means the artifact would be less complete than the fallback it sits below unless scroll-and-stitch is built too.
  - OCR is the real cost: a Rust OCR engine (Tesseract bindings or an ONNX model) is a large native dependency with a Windows build story, for output that is lower fidelity than every rung above it.
  - If it is built, it must be a *supplementary* artifact attached to a capture — `ScribbleAttachment` already exists — and never a substitute that lets `coverage` claim more than the text extraction earned.

### 8. Firefox Support for the Capture Extension

- **Status**: Deferred — structurally compatible, not validated
- **Area**: `native/browser-extension/manifest.json`, `native/src/webcapture/background.ts`
- **Original Context**:
  - The extension targets Chrome and Edge. Firefox supports `scripting.executeScript` under MV3 from Firefox 101 and grants `activeTab` on the same gestures, so nothing in the extraction layer is Chrome-specific — but the manifest ships Chrome shapes and no Firefox testing was done, so it is not claimed as supported (`docs/capture.md` §13).
- **Concept & Implementation Blueprint**:
  - Add `browser_specific_settings.gecko.id` and, on older Firefox, a `background.scripts` event page instead of `background.service_worker`; the build already emits both an ES module and an IIFE, so no new bundling is required.
  - Firefox treats `host_permissions` as optional by default, so pairing must handle "granted later" — the options page's **Save and test** is the natural place to call `permissions.request`.
  - The bridge already accepts `moz-extension://` origins, and there is a test asserting it.

### 9. Capturing File Bytes and Image Data

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

### 10. A Configurable Traversal Budget

- **Status**: Deferred — the budget is a constant per source, and hitting it is reported
- **Area**: `native/src/webcapture/traversal/budget.ts`, `native/src-tauri/src/settings/`, `native/src/components/settings/CaptureSettingsView.tsx`
- **Original Context**:
  - The reveal pass stops after 10 seconds on a conversation (6 on a document) and reports `time_budget` with `partial` coverage. Measured against the validation fixtures, that covers roughly 450–500 turns (`docs/capture/BENCHMARKS.md`), so a genuinely enormous thread is captured incompletely — honestly, but incompletely — and the user has no way to say "take longer, I'll wait".
  - It was left constant because the extension cannot read Relay's settings without another round-trip to the bridge, and adding one to the capture path for a value that is right in almost every case is a poor trade.
- **Concept & Implementation Blueprint**:
  - The bridge's `/v1/health` response is the natural carrier: the service worker already calls it during pairing, so the budget could ride along and be cached in `chrome.storage.local` rather than fetched per capture.
  - Expose it as a choice rather than a number — "quick", "thorough", "as long as it takes" — mapping to `maxMs`/`maxSteps` pairs, because a millisecond field invites a value that makes capture feel broken.
  - Whatever is chosen, `termination` and the completeness verdict must keep working the same way: a longer budget may not change what a capture is *allowed to claim*, only how much it manages to read.

### 11. Neural Speaker Embeddings and a Voice Library

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
  - Per-source audio tracks would help independently: diarizing the
    system-audio stream alone removes the local user's voice from the clustering
    problem entirely.

### 12. Calendar Sync for Meeting Attendees and Titles

- **Status**: Partly shipped (0.41.0) — calendar integration exists; the
  expected-speaker hint does not
- **Area**: Native backend (`native/src-tauri/src/meetings_v2/processing/metadata.rs`, `native/src-tauri/src/oauth/*`)
- **Original Context**:
  - Rung 3 of `Meeting-rules/meeting_speaker_identification.md` is "calendar
    attendees + conferencing display names", and §2.2 specifies that a
    recording's expected-speaker count should default to the calendar attendee
    count.
  - **The first half shipped and this entry did not get updated.** Google
    Calendar accounts, event matching and `ParticipantOrigin::Invited` all
    exist, and `processing/metadata.rs` populates participants from the matched
    event's likely attendees — ordered after the voices actually heard and
    before names merely mentioned, and never marked confirmed, because the
    calendar is a record rather than evidence that somebody spoke.
  - What is still missing is §2.2: the attendee count is not used as the
    expected-speaker hint for diarization, which is the parameter that would
    let separation know how many voices to look for.
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

### 13. Auto-Learning Dictionary Words From Corrections

- **Status**: Removed — the setting existed, the mechanism could not
- **Area**: Native backend (`native/src-tauri/src/settings/mod.rs`), Settings › Engine
- **Original Context**:
  - `AudioInputSettings::auto_learn_words` shipped as a persisted setting with a
    rendered toggle, defaulting on, and was read by nothing. Its own copy
    promised: "When you correct a transcription in the target app, the
    corrected word is automatically added to your dictionary."
  - That is not a feature that was left unfinished. It describes observing what
    the user edits **in another application**, which needs accessibility text
    APIs — reading the contents of somebody else's window. Relay does not do
    that anywhere, and the capture surfaces are deliberately built the other
    way round: `hotkeys::injection` can put text into a focused field and has
    no way to read one back.
  - Left in place it was worse than absent: a toggle that is on by default and
    silently does nothing teaches the user that the setting does not matter.
- **What would have to be true first**:
  - A decision to take an accessibility-API dependency (UI Automation on
    Windows, AX on macOS) and the privacy posture to go with it — Relay would
    be reading the user's other applications, which is a different product
    promise from recording their microphone when they press a key.
  - That is the same class of decision as the ONNX runtime in item 1: a
    dependency and a stance, not an afternoon.
- **The cheaper thing that is actually available**:
  - Relay already has a user dictionary and a glossary that primes the
    recognizer, and it now has a rewrite layer (`capture::rewrite`) where the
    user explicitly accepts or rejects a proposed change to their own dictated
    text. A word the user *accepts* there is a correction Relay can observe
    without reading anybody's window. That is a real feature and a different
    one, and it should be designed as itself rather than smuggled in under this
    setting's name.
