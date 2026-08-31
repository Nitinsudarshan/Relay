# Relay — Changelog

## [0.21.0] - 2026-09-01

### Fully Reversible Voice Note Merging

**Type**: minor — end-to-end reversible voice note merging with stack-based persistence, deterministic nested unmerging, and UI confirmation (`native/src-tauri/src/vault/mod.rs`, `native/src-tauri/src/commands.rs`, `native/src-tauri/src/lib.rs`, `native/src/types/index.ts`, `native/src/components/voicenotes/VoiceNotePage.tsx`, `native/src/components/voicenotes/VoiceNotePage.test.tsx (new)`).

#### Features

- **Stack-Based Pre-Merge Snapshot Persistence (`vault/mod.rs`)**: Saved pre-merge note snapshots (`primary_source` and `secondary_source`) to `{vault_dir}/merged_sources/{primary_id}.json` as a stack of `MergeRecord` objects. Preserves exact note ID, title, created_at, updated_at, tags, source_audio, and content string-for-string without destructive note deletion.
- **Frontmatter Metadata Integration (`vault/mod.rs`, `types/index.ts`)**: Added `merged_from: ["id1", "id2"]` frontmatter field and optional TS type property `merged_from?: string[]`, enabling the vault to track component notes and unmerge capability across Relay application restarts.
- **Deterministic Step-by-Step Unmerge Operation (`vault/mod.rs`, `commands.rs`, `lib.rs`)**: Exposed `unmerge_voice_note` Tauri command wrapping `VaultManager::unmerge_notes`. Supports nested merges (`A + B -> AB`, `AB + C -> ABC`), where unmerging `ABC` restores `AB` (which retains its own merge history) and `C`, and subsequent unmerge on `AB` restores `A` and `B`.
- **UI Merged Indicator & Confirmation Dialog (`VoiceNotePage.tsx`, `VoiceNotePage.test.tsx`)**:
  - Rendered a `Merged · N Voice Notes` badge and an `Unmerge` action button for merged notes.
  - Added an inline confirmation banner (*"Unmerge this Voice Note? This will restore the original Voice Notes..."*) with `Cancel` and `Unmerge` buttons.
  - Surfaced user-visible error alert banners on operation failures and guarded against duplicate action triggers.
- **Scribble & Provenance Synchronization (`vault/mod.rs`)**: Automatically updated derived Scribble metadata and restored any trashed secondary scribbles upon unmerging.

## [0.20.0] - 2026-08-31

### Talkback Immersive Active Mode, Speech Synchronization & Calmer Idle Animation

**Type**: minor — conversational active mode, TTS text/audio synchronization, and directional liquid core animation (`native/src/components/talkback/TalkbackImmersiveView.tsx (new)`, `native/src/components/talkback/TalkbackOrbCanvas.tsx`, `native/src/components/talkback/TalkbackPage.tsx`, `native/src/components/talkback/TalkbackAgent.tsx`, `native/src/components/talkback/talkbackAudioQueue.ts`, `native/src/components/talkback/useTalkback.ts`, `native/src-tauri/src/talkback/speech.rs`, `native/src-tauri/src/talkback/engine.rs`).

#### Features

- **Full-Page Immersive Conversational Mode (`TalkbackImmersiveView.tsx`, `TalkbackPage.tsx`)**: When Talkback is activated, the entire page seamlessly transitions into a distraction-free, full-screen conversational interface centered on the living Relay entity. It removes distracting page banners, duplicated state badges, and latency stats, providing a spacious, enlarged dialog box for natural progressive speech reading. Includes `Escape` shortcut support with a clean top-left exit affordance, top-right history and view-mode switchers, and minimal floating speech controls.
- **Phrase-Level Text / Audio Synchronization (`speech.rs`, `engine.rs`, `talkbackAudioQueue.ts`, `useTalkback.ts`)**: Embedded synthesized phrase text directly into backend `SpeechChunk` and emitted with `talkback-audio` events. `TalkbackAudioQueue` triggers playback start notifications per chunk (`onChunkStart`), enabling synchronized progressive text reveal where words appear precisely as Relay speaks rather than dumping the full response at once.
- **Calm, Restrained Idle State (`TalkbackOrbCanvas.tsx`)**: Refined the idle presence to feel quietly present and non-distracting: very subtle core breathing (~4.5s cycle, ±2.5% scale), low-intensity ambient glow, minimal rotation drift, and reduction to subtle floating dust motes with soft opacity.
- **Directional Liquid Core + Wave Conduit Metaphor (`TalkbackOrbCanvas.tsx`)**:
  - *Listening*: User voice ripples move inward toward the core, dynamically deformed by live microphone amplitude (`micLevel`).
  - *Thinking*: Energy gathers and swirls inward in a computational vortex.
  - *Speaking*: Acoustic harmonic waves radiate outward in sync with real-time TTS audio frequency and intensity (`outputLevel`).
  - *Interruption*: Immediate transition back to listening upon barge-in or stop speaking.

## [0.19.3] - 2026-08-31

### Talkback Living Voice-Reactive Agent & Curated Voice Library

**Type**: minor — conversational agent animation, real-time audio reactivity, and neural voice catalogue (`native/src/components/talkback/TalkbackOrbCanvas.tsx (new)`, `native/src/components/talkback/TalkbackAgent.tsx`, `native/src/components/talkback/talkbackAudioQueue.ts`, `native/src/components/talkback/useTalkback.ts`, `native/src/components/settings/VoiceLibraryModal.tsx (new)`, `native/src/components/settings/VoiceSettings.tsx`, `native/src-tauri/resources/voice-manifest.json`, `native/src-tauri/src/commands.rs`).

#### Features

- **Living, voice-reactive Talkback agent presence (`TalkbackOrbCanvas.tsx`, `TalkbackAgent.tsx`)**: Replaced static graphic rings with a continuous, organic Canvas presence. Ambient breathing during idle, real-time radial wave reactivity to microphone volume during listening, accelerated rotational vortex energy during thinking, and voice acoustic wave pulsations reacting to synthesized output audio during speaking. Smooth spring interpolation ensures seamless transitions between states without abrupt jumps, pausing automatically when hidden and respecting reduced-motion preferences.
- **Real-time output audio metering (`talkbackAudioQueue.ts`, `useTalkback.ts`)**: Integrated real-time Web Audio API frequency analysis directly into the Talkback playback sink, streaming output audio amplitude into the agent so Talkback visually speaks through the orb.
- **Curated Multi-Language Voice Library (`VoiceLibraryModal.tsx`, `VoiceSettings.tsx`, `voice-manifest.json`)**: Expanded the voice catalogue from 3 to 10 curated neural voices across US English (Amy, Ryan, Lessac), UK English (Alba, Alan, Cori), Hindi (Pratham), Spanish (Sharvard), French (Siwis), and German (Thorsten). Added an interactive Voice Catalogue modal with language/gender filtering, search, quality tier badges ("Balanced", "Fast", "High Quality"), on-demand pinned SHA-256 downloads, and test playback.
- **Settings tab navigation fix (`App.tsx`, `commands.rs`, `ProviderSettings.tsx`)**: Wired `navigate-tab` and DOM event listeners to enable direct navigation to settings sub-sections (such as Talkback voice setup) from banners and floating windows.

## [0.19.2] - 2026-08-31

### Event-Aware Dictation Watchdog, Long-Recording Support & Explicit Stop Diagnostics

**Type**: patch — native backend (`native/src-tauri/src/hotkeys/mod.rs`).

#### Fixes

- **Unconditional 60-second dictation cutoff removed (`native/src-tauri/src/hotkeys/mod.rs`)**: Resolved the product bug where hold-to-talk dictation recordings were forcefully terminated after 60 seconds by an unconditional sleep timer. Continuous recordings of arbitrary length (e.g. 2m, 5m, 10m+) now continue as long as the user holds the hotkey.
- **Physical key-state aware safety recovery (`native/src-tauri/src/hotkeys/mod.rs`)**: Separated normal recording duration from lost-key safety recovery. In hold-to-talk mode, the safety watchdog queries physical key states (`GetAsyncKeyState` on Windows) to detect lost OS key-up events within ~1.0s and safely recovers stuck recordings without prematurely terminating active user dictation.
- **Explicit stop reasons & generation protection (`native/src-tauri/src/hotkeys/mod.rs`)**: Introduced `DictationStopReason` (`NormalRelease`, `TogglePress`, `WatchdogLostRelease`, `WatchdogEmergencyCeiling`) to clearly distinguish user actions from safety recoveries in diagnostic logs. Re-verified generation guards so delayed watchdogs never stop subsequent recording sessions.

## [0.19.1] - 2026-08-30

### The Voice Installer Was Pointed at a Python Package

**Type**: patch — native backend and release tooling (`native/src-tauri/resources/voice-manifest.json`, `native/src-tauri/src/tts/manifest.rs`, `native/src-tauri/src/tts/installer.rs`, `native/src-tauri/Cargo.toml`, `scripts/build-voice-manifest.mjs`, `scripts/lib/archive-index.mjs (new)`, `scripts/lib/fake-archives.mjs (new)`, `scripts/build-voice-manifest.test.mjs (new)`, `package.json`, `.github/workflows/ci.yml`, `docs/talkback/{ARCHITECTURE,RESEARCH}.md`, `docs/decisions.md`).

v0.19.0 shipped one-click voice setup with the manifest deliberately
unprovisioned, to be filled in by a release step on a networked machine.
Run on Windows, that step failed:

```text
✗ Release v1.7.0 has no asset for piper-windows-x86_64. Assets:
piper_tts-1.7.0-cp39-abi3-win_amd64.whl  …
```

It reads like a naming drift. It was not. `OHF-Voice/piper1-gpl` publishes
Python wheels and an sdist and nothing else — its release workflow uploads
`dist/*`, in every release from v1.3.0 to v1.7.0 — and Relay's installer
downloads an archive and spawns an executable out of it. The catalogue had
been pointing at a project that could not satisfy the architecture since
the day it was written. Nobody found out, because the manifest was never
provisioned.

The dangerous fix was one line away. `piper_tts-1.7.0-cp39-abi3-win_amd64.whl`
is a zip: it downloads, it hashes, it extracts. Relaxing the asset matcher
would have pinned it, shipped it, and surfaced as *"the voice installed but
couldn't speak"* on a user's machine after a 34 MB download.

#### Fixes

- **The engine is pinned to `rhasspy/piper` `2023.11.14-2`**, the last upstream release that publishes standalone binaries — `piper_windows_amd64.zip` containing `piper/piper.exe`, `piper_linux_x86_64.tar.gz` containing `piper/piper`, both with onnxruntime and espeak-ng beside them, MIT-licensed. Verified by downloading both, listing their contents, installing through the real installer and synthesizing through the production `PiperProvider`. Upstream is archived; that is a real cost, and the alternative was not a newer engine but no engine. See `docs/decisions.md` Decision 56 and `docs/talkback/RESEARCH.md` §B.1.
- **Linux could never have installed either (`native/src-tauri/src/tts/installer.rs`)**: upstream ships every non-Windows platform as a `.tar.gz`, and `ArchiveKind` knew only `zip` and `raw`, so the manifest's Linux entry claimed a packaging its artifact does not use. `tar_gz` is now a first-class archive kind. Its reader is hand-rolled rather than a library's `unpack()`: entry paths are checked lexically against the target (`..`, absolute and drive-letter paths refused), symlinks are validated for containment and created after the regular files — Piper's Unix build loads its libraries through `libonnxruntime.so → libonnxruntime.so.1.14.1`, and both dropping those links and letting one point outside the folder are real failures — tar modes are masked to the ownership bits so setuid cannot survive extraction, and the whole expansion is capped. Zip extraction gained the same cap: a pinned checksum proves what the bytes are and says nothing about what they unfold into.
- **The generator no longer guesses which asset it wants (`scripts/build-voice-manifest.mjs`)**: each runtime names its provenance — `release: { repo, tag, asset }` — and the script resolves that exact name. It then **opens the artifact and asserts the executable the manifest promises is really inside it**, non-empty and (for a tarball) marked runnable, using a dependency-free reader of the zip central directory and of tar headers (`scripts/lib/archive-index.mjs`). A release that publishes only Python distributions is now diagnosed in those words, with an explicit instruction not to rename the expected asset to match it.
- **A wheel can no longer reach the catalogue (`native/src-tauri/src/tts/manifest.rs`)**: `validate()` refuses a `.whl` runtime asset outright, refuses any asset whose extension disagrees with its declared `archive`, refuses an `executable_path` that could escape the engine folder, and re-derives the download URL from the release pin so the artifact, the manifest and the generator cannot disagree. Provenance is therefore checked twice — once at release time, once at load time on the user's machine. The schema is bumped to **2**, so a version-1 file (whose Linux entry claims `zip` and points at a tarball) fails loudly instead of half-working.
- **The generator no longer crashes while reporting a failure (`scripts/build-voice-manifest.mjs`)**: the Windows run also printed `Assertion failed: !(handle->flags & UV_HANDLE_CLOSING), file src\win\async.c, line 94`. That was ours — `fail()` called `process.exit()` from inside the async flow, tearing the event loop down while `fetch` still held handles. Failures now unwind to a single handler that sets `process.exitCode` and lets the process end on its own; error paths cancel any response body they are not going to read; and a half-written progress line is closed before the message. Same shell result, after the message has actually been written.

#### Improvements

- **The release step is tested (`scripts/build-voice-manifest.test.mjs`, `npm run test:scripts`, CI)**: 27 cases over hand-built zip and tar fixtures — including a wheel-shaped archive — covering what the generator accepts, what it refuses, and whether the refusal explains itself. No network required. Wired into the repository-rules job, because a script that decides what Relay is willing to download and execute should not be the untested part of the repository.
- **A network-gated install-and-speak test (`native/src-tauri/src/tts/installer.rs`)**: `cargo test -- --ignored the_pinned_engine_installs_and_speaks` installs the catalogue's own pinned engine into a temporary root and synthesizes through `PiperProvider`. Every other test proves a property of the machinery against a fixture; this one proves the artifact is a real speech engine. It skips, rather than fails, while the manifest is unprovisioned.

#### Unchanged

- **The user experience**: still one button, *Make Relay speak*. No new setting, no new prompt, no mention of Piper, wheels or tarballs anywhere in the flow.
- **The security model**: HTTPS-only, pinned SHA-256, expected size, safe extraction, atomic installation, self-test through the production `PiperProvider`, cancellation and rollback. Nothing was relaxed to make the release step pass; the checks above were added to it. **The shipped manifest remains unprovisioned** — `huggingface.co`, which hosts the voice models, is blocked at this environment's egress gateway — so `node scripts/build-voice-manifest.mjs` must still be run once on a networked machine before setup is available in a build. The engine half of that run was executed here against the real GitHub artifacts and both digests verified.

## [0.19.0] - 2026-08-30

### One-Click Local Voice — Relay Installs Its Own Speech Engine

**Type**: minor — both surfaces (`native/src-tauri/src/tts/manifest.rs (new)`, `native/src-tauri/src/tts/installer.rs (new)`, `native/src-tauri/resources/voice-manifest.json (new)`, `native/src-tauri/src/tts/mod.rs`, `native/src-tauri/src/{lib,commands}.rs`, `native/src-tauri/Cargo.toml`, `native/src/components/settings/VoiceSettings.tsx`, `native/src/components/talkback/TalkbackPage.tsx`, `native/src/types/index.ts`, `scripts/build-voice-manifest.mjs (new)`, `docs/talkback/{ARCHITECTURE,RESEARCH,BENCHMARKS}.md`, `docs/decisions.md`, `README.md`, `maybe_later.md`).

v0.18.1 gave Talkback a voice setup screen. What that screen actually asked
of a user was: find the Piper project on GitHub, work out which of its
release archives matches your CPU, unzip it into a folder under `AppData`
whose path we print for you, then go to a different site, download an
`.onnx` file *and* its `.onnx.json` sidecar, and put those somewhere else.
Six manual steps, three of which are a coin flip if you don't already know
what ONNX is.

That is an implementation detail wearing a product's clothes. This release
deletes it. `Settings › Talkback` now shows one button — **Make Relay
speak** — and Relay does the rest: picks the build for your machine,
downloads it, verifies it against a checksum it shipped with, installs it,
loads the voice, and speaks a sentence to prove it works before it says
Ready.

#### Features

- **One-click voice setup (`native/src/components/settings/VoiceSettings.tsx`)**: Not installed → Downloading → Installing → Validating → Testing → Ready, with a live per-file and overall progress bar, an accurate byte count, and a Cancel that actually stops the transfer. A recommended voice is chosen for you and named, so setup never blocks on a decision the user has no basis to make; the full voice list appears only *after* Relay can speak, and only lists voices from its own catalogue. Words like Piper, ONNX, GitHub and AppData appear nowhere in the default flow — a test asserts it, and the paths and engine version live behind an `Advanced` disclosure that renders nothing until it is opened.
- **A Relay-owned voice catalogue (`native/src-tauri/src/tts/manifest.rs`, `native/src-tauri/resources/voice-manifest.json`)**: compiled into the binary with `include_str!` and the single source of every download URL. The UI names a *voice id* and can no more construct a URL than it can choose a directory to write to. Each entry carries its version, platform, architecture, SHA-256, expected size, the executable's path inside the archive, its licence and its upstream project — so what Relay will fetch is reviewable in a diff.
- **A download/install lifecycle layer (`native/src-tauri/src/tts/installer.rs`)**: streamed to disk in 64 KB chunks with progress and cancellation between each, hashed as it reads rather than re-read afterwards, extracted with zip-slip protection, and moved into the live location by an atomic `rename` from a staging directory on the same filesystem. `install_local_voice` and `cancel_voice_install` are single-flight; a second click cannot start a second download.

#### Improvements

- **Installed now means "it speaks"**: before reporting Ready, Relay synthesizes a sentence through the **production** `PiperProvider` — not a demo path — and additionally cross-checks each `.onnx` against its `.onnx.json` sidecar (sample rate present, dataset matching the voice id), which catches a model and config that each pass their own checksum but do not belong together. A voice that downloads perfectly and cannot load is now caught during setup rather than three sentences into a conversation. A voice that fails that test is removed again — readiness is decided from what is on disk, so leaving it there would turn a caught failure into a silent one — unless it was already installed and working before the attempt, in which case it stays.
- **A failed install cannot cost you a working one**: everything lands in `<app-data>/Relay/tts/.staging` first, and the config file is moved before the model so a torn install never leaves a model without its sidecar. A failure, a cancellation or a crash leaves the previous installation exactly as it was; staging is also swept at startup, because a crash cannot run `Drop`.
- **`TtsProvider`, `PiperProvider` and `NullProvider` are unchanged.** Networking lives in a layer beside the provider, not inside it — the installer only puts files where `discovery` already looks, and an explicit setting still overrides both, so a developer pointing Relay at their own Piper build is not overridden by the installer.
- **Errors are told, not dumped**: `InstallError` carries a stable code, a retryable flag and a plain-English message. "The download was interrupted. Check your connection and try again." is what a user sees; the URL, the path and the Rust error go to the log. A test asserts that no user-facing string contains `http`, a drive letter or a `\`.

#### Fixes

- **The manual setup instructions are gone from the product**, along with the folder paths they printed. They survive only in `docs/talkback/ARCHITECTURE.md`, where an implementation detail belongs.
- **An unverifiable catalogue is treated as no catalogue**: checksums cannot be written by hand, so `scripts/build-voice-manifest.mjs` downloads each artifact, hashes it and rewrites the manifest as a release step. Until it has run, `validate()` rejects the shipped file and Relay reports *"automatic voice setup isn't available in this build"* rather than downloading something it cannot verify. **The manifest in this commit is unprovisioned** — the environment this was built in blocks the artifact hosts at its egress gateway — so the flow is complete in code and inert until that one command runs on a networked machine. See `docs/decisions.md` Decision 54.

#### Tests

- **60 new Rust tests** (654 → 714 passing, plus the 3 `#[ignore]`d benchmarks). Manifest validation (23) rejects duplicate ids, unpinned or non-HTTPS artifacts, a zip with no executable path, zero sizes, and anything other than exactly one recommended voice; it also asserts the *shipped* manifest is HTTPS-only and that a lookalike host like `http://127.0.0.1.evil.example` does not pass the loopback carve-out. The installer (31) covers a refusal to contact the server at all for an unpinned artifact, a corrupt download deleting its temp file and leaving an existing install untouched, cancellation mid-stream, an oversized artifact, zip-slip entries, sidecar/dataset mismatch, staging cleanup on drop and at startup, and that no error message leaks a path or a URL. Four more cover the single-flight guard, so a second click cannot start a second download and a cancel never poisons the retry, and a pair covers the self-test rollback in both directions: a voice that downloads cleanly and cannot speak is removed rather than left looking installed, while a *reinstall* that fails its self-test leaves the voice that was already working exactly where it was.
- **2 new frontend tests** (115 → 117), with the `VoiceSettings` suite rewritten around the new flow: one button and no filesystem path before setup, the recommended voice shown without a prompt to choose, both progress bars and a working cancel during setup, a picker of validated voices only afterwards, and a failure that stays retryable without exposing internals.

## [0.18.1] - 2026-08-30

### Talkback Production Readiness — Real Streaming Speech, a Local Voice Setup Flow, and No Dead Ends

**Type**: patch — both surfaces (`native/src-tauri/src/talkback/speech.rs (new)`, `native/src-tauri/src/talkback/{engine,state,mod}.rs`, `native/src-tauri/src/tts/discovery.rs (new)`, `native/src-tauri/src/tts/{mod,piper}.rs`, `native/src-tauri/src/{lib,commands}.rs`, `native/src/components/settings/VoiceSettings.tsx (new)`, `native/src/components/settings/TalkbackSettingsView.tsx`, `native/src/components/talkback/TalkbackPage.tsx`, `native/src/types/index.ts`, `docs/talkback/{ARCHITECTURE,BENCHMARKS}.md`, `docs/decisions.md`, `maybe_later.md`).

v0.18.0 was architecturally complete and not shippable. An audit against
`docs/talkback/` found three gaps between what the documents claimed and what
the code did, and each of them was the kind that only shows up in use.

The headline one: Talkback did not stream speech. It collected sentences during
generation and synthesized them all afterwards — which looks like streaming
because the LLM API streams, and isn't. Second: there was no way to configure a
local voice without hand-editing `settings.json`, because the only UI that ever
wrote those keys was removed in v0.17.x. Third: several failure paths left the
conversation in a state only an application restart could clear.

#### Features

- **Local voice setup (`Settings › Talkback`, `native/src/components/settings/VoiceSettings.tsx`)**: Readiness at a glance, the resolved Piper program and *how Relay found it*, a voice picker, browse buttons for both, the exact folders to drop files in, and a **Test voice** button that drives the real provider — so a configuration that passes there cannot fail differently mid-conversation. Setup instructions appear only while it isn't working. The Talkback page carries a matching "Voice unavailable" banner rather than silently not speaking.
- **Piper discovery (`native/src-tauri/src/tts/discovery.rs`)**: Relay finds a Piper installation in a location it owns without being told — `<app-data>/Relay/tts/piper` first, then beside its own executable, then Tauri's resource directory (so bundling later is a packaging change, not a code change), then `PATH`. Voice models are enumerated with their `.onnx.json` sidecars, and a missing sidecar — the most common Piper setup mistake — is named as such instead of surfacing as an unreadable parse error.

#### Improvements

- **Speech now overlaps generation (`native/src-tauri/src/talkback/speech.rs`)**: A bounded producer/consumer pipeline synthesizes each sentence *while the model writes the next*. Measured on a simulated five-sentence answer: **600 ms to first audio versus 1,750 ms** for the batched path it replaces, with the gap widening as answers get longer. One consumer thread makes ordering structural, the bounded queue applies backpressure instead of growing, and synthesis no longer parks a Tokio worker on a blocking child process.
- **`tts_first_audio_ms` measures what its name says**: turn start → audio available, rather than the duration of one synthesis call. The old metric looked excellent while the user waited seconds — it was measuring the wrong thing well. `tts_first_synthesis_ms`, `tts_total_synthesis_ms`, `tts_phrases` and `tts_disabled` were added alongside, so a slow voice model and a slow language model stay distinguishable.
- **Cancellation reaches the process (`native/src-tauri/src/tts/piper.rs`)**: A barge-in now kills the Piper child rather than waiting for it and discarding the audio, checked every 15 ms. Cancellation is a distinct `TtsError::Cancelled` variant so talking over the agent is never reported to the user as a failure, and a synthesis that wedges is bounded by a 30-second timeout.
- **A broken voice stops retrying**: an unloadable model previously produced one failed process spawn per sentence, for every sentence, forever. Permanent failures now latch the voice off for the turn and are reported once. A successful Piper exit that produced no audio is classified as permanent rather than as a transient I/O error.
- **The state machine owns Talkback's state again**: the voice worker was emitting `talkback-state` events directly to the frontend, so the UI could show a state the backend did not hold. Every transition now goes through the engine.
- **Talkback settings are self-contained**: the voice card lives in `Settings › Talkback`, so a user asking "why isn't it speaking?" doesn't have to know the answer was filed under a different section.

#### Fixes

- **No console window per sentence on Windows (`native/src-tauri/src/tts/piper.rs`)**: Piper is a console-subsystem program, so every spoken sentence flashed a window. Now spawned with `CREATE_NO_WINDOW`.
- **The voice folder is somewhere that exists (`native/src-tauri/src/tts/discovery.rs`)**: the installation root is anchored to the OS application-data directory instead of `current_dir()/.relay/config`. In a packaged Windows app launched from a Start Menu shortcut, `current_dir()` is typically `C:\Windows\System32` — so "put piper.exe in this folder" was printing a path that was neither writable nor stable between launches. The vault, settings and Whisper models are deliberately **not** moved; that migration is logged in `maybe_later.md`.
- **No dead-end conversation states**: `TRANSCRIBING` had no exit when a decode produced nothing — a cough or an unavailable model stopped the conversation until Relay was restarted. A `TurnGuard` now returns the machine to a resting state on every exit from a turn, including the error and panic paths, and "stop speaking" returns to listening rather than waiting in `INTERRUPTED` for words that are not coming.
- **Scratch audio is always cleaned up**: an RAII guard removes each synthesized WAV on every exit path — success, failure, cancellation, panic — and orphaned files from a previous crash are cleared at startup. At one file per sentence this was a leak measured in files per conversation.

#### Tests

- **73 new Rust tests** (581 → 654 passing, plus a third `#[ignore]`d benchmark), covering the speech pipeline (ordering, no duplicates, no drops, backpressure, cancellation before/during/after synthesis, stale audio suppression, the permanent-failure latch, first-audio signalling, worker join on drop), Piper's process lifecycle against a shell stub (cancellation kills the child, every failure path cleans up, paths with spaces, repeated synthesis leaks nothing), discovery and validation (managed/bundled/PATH precedence, sidecar detection, non-ASCII and spaced paths), and state-machine recovery from every state.
- **19 new frontend tests** (96 → 115), covering the voice setup card (not-configured messaging, problem surfacing, install paths, voice selection and persistence, browse and cancel, test playback and its failure, re-check) and the Talkback page's voice banner.
- **A third benchmark**, `overlap_saving`, quantifying the streaming gain — the 600 ms vs 1,750 ms figure above is its output. `#[ignore]`d like the others so CI has no timing tests.

## [0.18.0] - 2026-08-30

### Talkback — the Conversational Layer over Everything Relay Has Captured

**Type**: minor — both surfaces (`native/src-tauri/src/talkback/** (new)`, `native/src-tauri/src/tts/**`, `native/src-tauri/src/providers/mod.rs`, `native/src-tauri/src/{lib,commands}.rs`, `native/src-tauri/src/settings/mod.rs`, `native/src-tauri/src/pipeline/**`, `native/src-tauri/.cargo/config.toml`, `native/src/components/talkback/** (new)`, `native/src/components/settings/TalkbackSettingsView.tsx (new)`, `native/src/components/settings/ProviderSettings.tsx`, `native/src/components/common/NativeSidebar.tsx`, `native/src/App.tsx`, `native/src/types/index.ts`, `docs/talkback/** (new)`, `docs/{README,decisions}.md`, `README.md`, `maybe_later.md`).

Relay captured, understood, and remembered. It could not converse. Talkback is
that fourth layer, and the thing it deliberately is **not** is another AI stack:
it owns no database, no note type, and no memory of its own. Voice Notes,
Scribbles, Meetings and MeetingFacts *are* the memory.

Ask what you decided and the answer comes from your own capture, with the
sources shown. Ask something Relay has no record of and it says so — without
calling a model at all, because the cheapest way never to invent a memory is
not to give a model the chance. Talk over it and it stops mid-sentence.

#### Features

- **Talkback surface (`native/src/components/talkback/`, `native/src-tauri/src/talkback/`)**: A first-class navigation surface with an animated conversational agent, a live transcript, per-answer source chips, an explicit ON/OFF toggle, and a text box. Voice and text enter the *same* engine — the text box is not a fallback chatbot, it is the same turn without a microphone. **No new global hotkey**: Relay has two and a third would be one too many.
- **Unified context retriever (`talkback/retrieval.rs`, `talkback/sources.rs`)**: One canonical way to ask what Relay knows, across Voice Notes, Scribbles, meeting summaries and MeetingFacts. IDF-weighted lexical scoring with title/tag boosts, exact-phrase bonus, per-source weighting (derived meeting intelligence outranks raw dictation), recency decay, one-hop expansion along Scribble relationships and shared topics, deduplication (a meeting and its facts collapse to one item), and a character budget derived from the provider's configured context window rather than a fixed constant. Every retrieved item keeps `source_type`, `source_id`, `title`, `timestamp` and `relevance` — so "where did you get that?" has a real answer, produced deterministically rather than by the model.
- **Personal-memory policy (`talkback/engine.rs::plan_turn`)**: A recall question ("what did we decide", "do you remember", "what happened in") with no retrieved evidence is answered *"I couldn't find that in your Relay data."* without an LLM call. A recall question *with* evidence gets a prompt that forbids answering from general knowledge.
- **Streaming responses (`providers::LLMClient::complete_streaming`, `talkback/chunk.rs`)**: The LLM streams; a phrase buffer releases whole sentences; each sentence is synthesized and played while the next is still being written. Time-to-first-audio becomes the cost of the *first sentence*, not the whole answer — which is what makes a batch engine like Piper behave like a streaming one. The buffer knows decimals, ellipses, abbreviations, closing quotes, and the Devanagari danda.
- **Barge-in (`talkback/turn.rs`, `talkbackAudioQueue.ts`)**: Speaking over Relay cancels the in-flight generation, abandons pending synthesis, clears the audio queue, and starts a new turn — one cancellation token threaded through all three, so audio synthesized for a superseded turn can never play late.
- **Voice Note and Scribble creation by voice (`talkback/tools.rs`)**: "Start recording this as a voice note" opens a capture that swallows turns until "stop voice note", then persists through `VaultNote::new_voice_note` — the same constructor the dictation hotkey uses, storing the transcript verbatim. "Turn this into a Scribble" saves the conversation through `Scribble::new_text` with `source_metadata.source = "talkback"`, so the existing enrichment, graph and merge pipelines pick it up unchanged. No new schema for either. No destructive or outbound tools ship.
- **TTS provider abstraction (`tts::TtsProvider`)**: Piper moves behind a trait with declared capabilities, so a second local engine can be added and measured without touching a call site. `resolve_provider` is the single place a future provider is registered.
- **Backend-owned state machine (`talkback/state.rs`)**: `OFF → STARTING → LISTENING ⇄ USER_SPEAKING → TRANSCRIBING → THINKING → SPEAKING → LISTENING`, plus `INTERRUPTED` and `ERROR`. A total function over an event enum, so an illegal transition is a failing test rather than a UI that claims to be listening with the microphone closed. The frontend renders it and never invents it.
- **Talkback settings (`Settings › Talkback`)**: Speak-aloud, allow-interruption, end-of-turn pause, and which memory sources may be read. The activation-mode seam accepts `wake_word` and the engine refuses it with a clear message — the architecture is ready, no always-on listener ships.
- **Per-turn observability (`TurnMetrics`)**: STT, retrieval, LLM first-token, LLM total, TTS first-audio and total latency, plus interruption and provider, emitted per turn and shown in the Talkback sidebar. Ids, counts and durations only — never transcript text, retrieved content, or audio.

#### Improvements

- **Streaming for every provider (`native/src-tauri/src/providers/mod.rs`)**: `complete_streaming` handles Ollama's NDJSON and OpenAI/Anthropic/Gemini server-sent events through one pure `parse_stream_line`, so all four are unit-tested against captured frames without a network. Uses `Response::chunk` rather than `bytes_stream`, so streaming costs zero new crates. Chunk boundaries are decoded from bytes, not per-chunk strings, so a boundary falling inside a multi-byte character cannot corrupt Devanagari mid-word.
- **Microphone arbitration (`commands.rs`)**: Dictation and Talkback share one device and now refuse each other with specific error codes instead of racing for it. Talkback's stream is created on enable and *dropped* on disable — "off" means the OS-level capture does not exist.
- **README corrected (`README.md`)**: The Features list and architecture diagram claimed a LanceDB vector store. There is no LanceDB dependency in `Cargo.toml` and no embedding code in the repository; the claim is removed rather than left to mislead the next reader. `docs/decisions.md` Decision 48 records why retrieval is lexical and where embeddings plug in.

#### Fixes

- **Linux and CI builds restored (`native/src-tauri/.cargo/config.toml`)**: v0.17.3 replaced `WHISPER_DONT_GENERATE_BINDINGS` with hardcoded `C:\Program Files\...` paths for `CMAKE` and `LIBCLANG_PATH`. Cargo's `[env]` is global with no per-platform form, so those paths applied on Linux and macOS too and broke every non-Windows build including CI — the exact regression the file's own comment and `AGENTS.md` forbid. Restored the binding-skip flag, which removes the need for libclang on *any* platform; a developer whose CMake is off PATH can still export `CMAKE` themselves, as the comment already documented.

#### Removals

- **Dead voice-chat path deleted (`native/src-tauri/src/pipeline/chat.rs`)**: `process_chat` had no reachable caller — `ChatPanel.tsx` was deleted under Decision 34 and no command exposed it. Talkback replaces it rather than sitting beside it, so it is removed instead of left to rot next to its successor.

#### Tests

- **169 new Rust tests** (412 → 581), covering the state machine's legal and illegal transitions, intent routing across memory/action/provenance phrasings, retrieval ranking (source weighting, recency-versus-relevance, IDF, expansion, budget, determinism), excerpt selection, the phrase buffer under character-by-character streaming, turn detection (mid-sentence pauses, keystroke rejection, echo guard, runaway caps), the Voice Note and Scribble tools against a real vault, provider stream parsing for all four providers, and the no-evidence guarantee.
- **25 new frontend tests** (71 → 96), covering audio-queue ordering, interruption, late audio from a superseded turn, decode failure, the agent's state mapping, and that typing never silently opens the microphone.
- **Two benchmarks**, `#[ignore]`d so CI has no timing tests: retrieval scaling and phrase-buffer throughput. Results and — explicitly — what was *not* measured are in `docs/talkback/BENCHMARKS.md`.

#### Documentation

- **`docs/talkback/RESEARCH.md`**: The research pass. Competitive matrix (ChatGPT Voice, Claude voice, Gemini Live, Perplexity, Granola, Limitless, Mem, Notion AI — all reference-only), open-source technology matrix with licences verified upstream, and the architectures rejected with reasons. Notable findings: Piper was archived in October 2025 and its maintained successor is GPL-3.0; Parakeet has no Hindi, which disqualifies it for Relay; `rodio` requires `cpal ^0.17` against Relay's `0.15`.
- **`docs/talkback/ARCHITECTURE.md`**: The pipeline, state machine, retrieval stages, privacy contract, and what Talkback deliberately does not do.
- **`docs/decisions.md` Decisions 47–49**: Why native rather than Pipecat, why not speech-to-speech, why retrieval is lexical, why playback lives in the WebView, and why turn detection is Talkback's own.

## [0.17.3] - 2026-08-30

### Whisper MSVC Build Environment & Toolchain Path Resolution

**Type**: patch — native backend (`native/src-tauri/.cargo/config.toml`).

#### Fixes

- **Whisper C bindings & CMake discovery (`native/src-tauri/.cargo/config.toml`)**: Resolved Windows MSVC build failures during `whisper-rs-sys` compilation. Explicitly configured `CMAKE` and `LIBCLANG_PATH` toolchain environment pointers so `bindgen` successfully emits MSVC-compatible C bindings and CMake compiles `whisper.cpp` without requiring manual system environment PATH exports. All 412 backend tests and 71 frontend tests pass cleanly.

## [0.17.2] - 2026-08-30

### Continuous Integration, a Frontend Test Suite, Poison-Tolerant Locking & a Repo That Builds Off One Machine

**Type**: minor — both surfaces (`.github/workflows/ci.yml (new)`, `native/src-tauri/.cargo/config.toml`, `native/src-tauri/src/sync.rs (new)`, `native/src-tauri/src/{lib,commands}.rs`, `native/src-tauri/src/{capture,hotkeys,meetings_v2,settings,vault,pipeline,developer}/**`, `native/src/components/meetings_v2/{meetingsViewState.ts (new),MeetingsV2View.tsx}`, `native/src/test/** (new)`, `native/vite.config.ts`, `web/src/app/(dashboard)/layout.tsx`, `AGENTS.md`, `README.md`, `docs/**`, `rules/ui-components.md`).

Nothing in this release changes what the app does. It changes what the repo can
prove about itself: 482 tests now run automatically, the Rust crate compiles on
a machine other than its author's, and one panicking thread no longer takes the
whole app with it.

#### Fixes

- **The crate could not build anywhere but one Windows install (`native/src-tauri/.cargo/config.toml`)**: `CMAKE` and `LIBCLANG_PATH` were pinned to absolute paths inside a specific Visual Studio and LLVM installation. Cargo's `[env]` is global — it has no per-platform form — so those applied on Linux and macOS too, where the build failed at `cmake::fail` with "No such file or directory". This is why 407 existing tests had never run in CI: they could not be built there. Replaced with `WHISPER_DONT_GENERATE_BINDINGS=1`, which is what actually achieves the file's stated goal (use whisper-rs-sys's committed bindings and skip bindgen/libclang entirely) and does so on every platform. A developer whose CMake is off `PATH` can still export `CMAKE` themselves, since Cargo does not override an already-set variable.
- **One panicking thread killed the whole app (`native/src-tauri/src/sync.rs (new)`, 10 modules)**: all 83 non-test lock sites used `lock().unwrap()`. `std::sync::Mutex` poisons on a panic-while-held, and every later `unwrap()` on that mutex then panics too — so a single recoverable fault on a capture thread took down settings reads, the pill, and the in-progress recording with it. In a long-running recorder that is a lost meeting, mid-meeting. `MutexExt::lock_or_recover()` takes the guard through the poison and logs each recovery at `warn` with the caller's file and line, so it stays diagnosable rather than silent. The trade is stated at the module: recovering means reading a value that may be mid-update, which is right for what these mutexes hold — settings replaced wholesale, an `Option<Session>` handle, sample buffers, cached diagnostics — and wrong for anything holding an invariant across fields.
- **A `FAILED` transcript chunk's partial text counted toward the word count (`native/src/components/meetings_v2/meetingsViewState.ts (new)`)**: found while extracting the logic to test it. The counter now takes only `SUCCESS` segments, so the UI stops reporting progress the user does not have.
- **A dead `auth()` stub reported every visitor as an Admin (`web/src/lib/auth.ts`)**: a "dummy auth" module returned hardcoded `Admin` session claims. Nothing consumed it except `lib/roles.ts`, itself unreferenced; the dashboard layout awaited `currentUser()` and discarded the result while hardcoding a user beside it. All removed. The layout now names its placeholder identity instead of appearing to resolve one.

#### Features

- **A pinned Rust toolchain (`native/src-tauri/rust-toolchain.toml (new)`)**: CI runs `clippy -D warnings`, and clippy gains lints between Rust releases — so a floating `stable` means a new compiler can redden CI on code nobody touched, and a contributor on an older toolchain cannot reproduce it. This is not hypothetical: two `unnecessary_sort_by` sites in `vault/mod.rs` passed clippy 1.94 locally and failed CI on 1.98. Both are fixed, and the version is pinned so a local check and CI use the same lint set. The workflow deliberately installs no toolchain of its own; the file is the single source of truth.
- **Continuous integration (`.github/workflows/ci.yml (new)`)**: four jobs on every push and pull request — repository rules, Rust backend, native frontend, web dashboard. Runs `cargo clippy --all-targets -- -D warnings`, `cargo test`, `tsc --noEmit`, `vitest run`, and a production build of both frontends. The Rust job installs the exact system dependencies the crate needs on Linux (GTK/WebKit for Tauri, ALSA for cpal, `libxdo` for enigo, a C/C++ toolchain and CMake for whisper.cpp) and caches the build, which is dominated by whisper.cpp on a cold run.
- **`scripts/verify-commit-rules.js` finally runs (`.github/workflows/ci.yml`)**: the script already existed and enforces `rules/version-and-changelog.md` and `rules/readme.md` — `VERSION` agreeing with all four manifests, a changelog entry for it, README structure. It is a CI job now instead of a command nobody invoked.
- **A frontend test suite (`native/src/test/** (new)`, `native/vite.config.ts`)**: Vitest + React Testing Library + jsdom, the stack `rules/testing.md` already specified, with 71 tests where there had been none across 18k lines. `src/test/setup.ts` stubs the Tauri API modules globally, since they only resolve inside a Tauri webview; `src/test/factories.ts` builds complete typed domain objects so a test states only what it is about. Scripts: `npm test`, `test:watch`, `test:coverage`, `typecheck`.

#### Improvements

- **`MeetingsV2View`'s derived state is testable (`native/src/components/meetings_v2/meetingsViewState.ts (new)`)**: active-state classification, word counting across the durable and live streams, selected-session resolution, and duration formatting were inline in a 1,124-line component and unreachable without mounting it and its whole Tauri surface. Extracted unchanged in behaviour, covered by 22 tests, and the component is shorter for it.
- **Clippy is clean at `-D warnings` (11 modules)**: 52 pre-existing findings fixed — `manual_strip`, `is_some_and`, `needless_range_loop`, `bool_assert_comparison`, `derivable_impls`, `field_reassign_with_default`, `manual_range_contains`, `useless_format`, `while_let_loop`, `single_match`, `iter_last`. The five `too_many_arguments` sites get a scoped allow with a reason each, rather than a crate-level allow that would also silence future code.
- **"No fake controls" is a repository rule (`rules/ui-components.md`)**: previously stated only in a push-to-talk spec that has since been deleted. It now binds every surface, with the review-time form added: a setting that exists in the schema and the UI but that no code path reads is a fake control that happens to survive a restart.
- **`AGENTS.md` describes this repository (`AGENTS.md`)**: all 22 of its rule links pointed at `Rules/` rather than `rules/`, so none resolved on a case-sensitive filesystem; it cited two files that have never existed here; and it told readers the repo was from-scratch with no implementation, at v0.16.0 with 35k lines of Rust. Rewritten with a surface table, working links, the commands CI runs, and where to leave a marker for a known gap.

#### Removals

- **1,160 lines of dead code**: `PTTWidget.tsx` (an 8-line component whose body was `return null`), `ChatPanel.tsx`, `KanbanBoard.tsx`, `ScribbleComposer.tsx`, `ui/popover.tsx`, and the `stageSucceeded` helper in `native/`; `Input.tsx`, `mini-loader.tsx`, `nav-secondary.tsx`, `search-form.tsx`, `ui/field.tsx`, `lib/roles.ts`, and `lib/auth.ts` in `web/`. All verified zero-reference by import search across both frontends and the Rust command surface.
- **1.6MB of generated output untracked**: `graphify-out/` was committed despite `.gitignore` already declaring it regenerated-per-run, and the snapshot was ten days stale.
- **Six stale documents**: `meetings_implementation.md` and `Relay_STT_Implementation_Plan.md` (plans for work shipped versions ago, the former still asserting "no source has been changed yet"), `docs/ptt-redesign-spec.md` (an agent task prompt), `docs/inspect/push-to-talk-pill.md` (a pre-work file map), `update.md` (a v0.4.1 task report), and `unused-components.md` (an audit of the components removed above). Plus `rules/code-standards.md`, an unindexed duplicate. Root markdown went from 9 files to 4. What was load-bearing was kept: `remove_prompt.md` moved to `docs/archive/prompt-mode.md`, and `docs/README.md` is now the index of what exists and what does not warrant a new file.

#### Tests

- 482 total: 411 Rust (407 existing, plus 4 covering mutex poison recovery — including one that pins the failure mode the change exists to prevent, and one asserting the partial write is visible after recovery so the caveat is demonstrated rather than only documented) and 71 native frontend.
- Frontend coverage: label resolution and title preference (23), view-derived state (22), `MeetingsV2View` behaviour driven entirely through `invoke` responses (6), knowledge-graph layout invariants including that coincident nodes separate instead of turning every coordinate into `NaN` (8), and the dictation sound effects that are the only code path behind the `dictation_sounds` setting (12).
- `docs/testing.md` rewritten: it listed `PTTWidget` and the Kanban board as coverage targets — both now deleted as dead code — and a `web` test script that does not exist. It now records what is covered and names what is not.

#### Known gaps

`cargo fmt --check` is deliberately not a CI gate. The crate predates any
formatting pass and differs from rustfmt in 45 files; running `cargo fmt` once,
as its own commit, is what unblocks adding it. `ProviderSettings.tsx` (1,874
lines) and `DictationPill.tsx` remain untested, and no end-to-end test drives a
real recording through capture, STT, and processing.
## [0.17.1] - 2026-08-30

### Design Polish, Meetings Layout Separation & Universal `rounded-lg` Normalization

**Type**: patch — cross-surface (`native/`, `web/`).

#### Improvements & Polish

- **Meetings Architecture & Banner Alignment**:
  - `native/src/App.tsx`: Added canonical `PageHeader` hero banner to the Meetings surface with purple glow, crash-resilience highlight badge, and dual-audio description matching Voice Notes and Scribbles.
  - `native/src/components/meetings_v2/MeetingsV2View.tsx`: Refactored layout to separate LHS (Recorded Sessions list + top recording controls) and RHS (Selected meeting detail + tabs) into distinct cards (`bg-card rounded-lg border border-border shadow-xs`) with `gap-4`, matching `ScribbleViewer.tsx`.
- **Scribble Detail Viewer Light & Dark Mode Parity**:
  - `native/src/components/scribble/ScribbleDetailEditor.tsx`: Resolved light-mode spacing and contrast issues by eliminating nested double-margin card boxes in `AI Summary` and `Thought Content`. Both now render single-layer crisp cards (`bg-muted/30 border border-border p-4 rounded-lg`) with aligned copy buttons and clean header toolbars.
- **Universal `rounded-lg` Normalization**:
  - Audited and updated all non-avatar/non-indicator containers and buttons across native and web from `rounded-xl` / `rounded-2xl` to standard `rounded-lg` (`dropdown-menu`, `MarkdownView`, `NativeSidebar`, `WelcomeModal`, `AccountExplanationModal`, `AccountSettings`, `MeetingsSettings`, `ProviderSettings`, `DictionarySnippetsSettings`, `DeveloperSettingsView`, `web/src/components/ui/{card,sidebar}`, `web/src/components/{page-header,empty-state,nav-user,login-form,changelog-dialog}`, and all dashboard pages).

## [0.17.0] - 2026-08-30

### Complete Repo-Wide UI/UX Audit, Correction & Design-System Consolidation

**Type**: minor — cross-surface (`native/`, `web/`, `Rules/`).

A comprehensive repo-wide design audit, correction, and token consolidation establishing full Light & Dark mode parity, standard roundedness (`rounded-lg` / `rounded-xl`), and centralized reusable components.

#### Improvements & Consolidation

- **Centralized Shared Components**:
  - `PageHeader` (`native/src/components/common/PageHeader.tsx`, `web/src/components/page-header.tsx`): Standardized hero headers across native (Voice Notes, Scribbles, Settings) and web (Kanban Dashboard, Notes, Settings, Components Showcase) with support for kickers, semantic badges, glow accents, and responsive layout.
  - `EmptyState` (`native/src/components/common/EmptyState.tsx`, `web/src/components/empty-state.tsx`): Canonical empty state container with dashed borders, icon wrapper, title, description, and action button slot across native sessions/notes/scribbles and web columns/notes.
- **UI Primitives Token Normalization**:
  - `Card` (`native/src/components/ui/card.tsx`): Replaced hardcoded `slate-` colors with semantic `bg-card`, `border-border`, `text-card-foreground`, `text-muted-foreground`.
  - `Badge` (`native/src/components/ui/badge.tsx`): Standardized semantic variant palettes (`default`, `secondary`, `destructive`, `outline`, `amber`, `emerald`, `purple`) ensuring crisp contrast in both Light and Dark themes.
  - `Input` (`native/src/components/ui/input.tsx`, `web/src/components/Input.tsx`): Normalized border, focus rings, and placeholder tokens; sanitized legacy web `Input.tsx` into a clean re-export of shadcn `Input`.
- **Meetings V2 Full Token Refactor**:
  - `MeetingsV2View.tsx`, `MeetingSummaryTab.tsx`, `MeetingActionItems.tsx`, `MeetingConversationTab.tsx`, `MeetingNotesTab.tsx`, `MeetingRawTranscriptTab.tsx`, `MeetingProcessingStatus.tsx`, `MeetingRelatedList.tsx`: Replaced hardcoded `#0a0a0c`, `zinc-*`, `lime-*`, and custom dark-only opacities with semantic design tokens (`bg-background`, `bg-card`, `border-border`, `text-foreground`, `text-muted-foreground`, `bg-accent`, `text-primary`).
- **Voice Notes, Scribble, Chat & Web Surfaces**:
  - `VoiceNotePage.tsx`, `ScribbleViewer.tsx`, `ChatPanel.tsx`: Integrated `EmptyState`, standardized border radius (`rounded-lg`), and replaced hardcoded slate classes.
  - `web/src/components/loading-view.tsx` & `mini-loader.tsx`: Removed legacy template branding and replaced with sleek Relay branded loading indicators and semantic styling.
  - `web/src/app/(dashboard)/{page,notes,settings,components}/page.tsx`: Standardized with `PageHeader`, `EmptyState`, and responsive grid layouts.

## [0.16.1] - 2026-08-30

### Native Sidebar Navigation Cleanup

**Type**: patch — `native/` only (`native/src/components/common/NativeSidebar.tsx`).

Clean up unused Quick Vault shortcut buttons and unused icon imports from the native sidebar.

#### Improvements

- **Sidebar cleanup (`native/src/components/common/NativeSidebar.tsx`)**: Removed deprecated "Quick Vault" section (voice capture and knowledge graph shortcuts) and unused Lucide icon imports (`Radio`, `FileText`, `Bell`).

## [0.16.0] - 2026-08-27

### Meeting Summaries as Memory: Canonical Context, a Real Summary Contract, Per-Meeting Length, Targeted Repair & an Evaluation Set

**Type**: minor — `native/` only (`native/src-tauri/src/providers/mod.rs`, `native/src-tauri/src/meetings_v2/{types,session_store}.rs`, `native/src-tauri/src/meetings_v2/processing/{context,length,eval}.rs (new)`, `native/src-tauri/src/meetings_v2/processing/{mod,model,llm,extract,summarize,validate,modes,conversation,tasks,tests}.rs`, `native/src-tauri/src/{commands,lib}.rs`, `native/src-tauri/src/settings/mod.rs`, `native/src/components/meetings_v2/{MeetingNotesTab.tsx (new),MeetingsV2View.tsx}`, `native/src/components/settings/MeetingsSettings.tsx`, `native/src/types/index.ts`, `docs/meetings/SUMMARY_QUALITY_REBUILD.md (new)`).

An end-to-end audit of the meeting pipeline and the changes it found. Closes gaps
8 and 10 of `Meeting-rules/meeting_pipeline_gap_analysis.md` and implements §6,
§8 and §11 of `Meeting-rules/meeting_transcript_summary.md`, which were specified
but never built. Recording, chunking, the two audio clocks, pause/resume, crash
recovery, dictation, and voice notes are untouched, and `qualify.rs` — already
the strictest part of the pipeline — is unchanged. Full write-up in
`docs/meetings/SUMMARY_QUALITY_REBUILD.md`.

#### Fixes

- **Long meetings were silently half-read (`providers/mod.rs`)**: `complete_ollama` posted `{model, prompt, stream: false}` with no `options` object, so Ollama applied its own 4096-token window (2048 on older builds) and discarded the overflow — from the *front* of the prompt. Stage A's user message is the whole transcript, so on any meeting past roughly a quarter of an hour the model never saw the beginning, which is where the agenda and the framing decisions are. Nothing logged it, and `input_chars` recorded the full size, so the processing log asserted the whole transcript had been sent. The request now states `num_ctx` from a configurable `ProviderConfig.context_tokens` (default 8192), and a prompt that fills the window is logged as the truncation risk it is.
- **Extraction ran at a creative-writing temperature (`providers/mod.rs`, `processing/llm.rs`)**: no temperature was ever sent, so Stage A — a strict-JSON read of a transcript whose failure mode is confidently invented ownership — ran at Ollama's default of 0.8. `MeetingLlm` had no parameter through which a stage could say otherwise. `LlmRequest::extraction` now runs at 0.1 and `LlmRequest::prose` at 0.3, per `meeting_transcript_summary.md` §11.
- **A fixed word cap rejected correct summaries (`processing/{length,validate,model}.rs`)**: `SummaryMode::max_words()` was a constant (220/550/1100) enforced as a validation **error**, and an error discarded the model's prose in favour of the deterministic renderer. A ninety-minute meeting legitimately needs more than 550 words in Standard mode, so a good summary became a bullet dump; the same number left a four-minute call room to pad, and nothing flagged that. Length is now derived per meeting, stated to the model, and only a runaway (1.4× the budget) is an error.
- **Gemini and Anthropic were sent to OpenAI (`providers/mod.rs`)**: `complete_cloud` hardcoded `api.openai.com` with an OpenAI body and an OpenAI auth header for all three cloud providers. Selecting Gemini or Anthropic sent the meeting to a service the user had not chosen, failed authentication, and fell through to canned filler — which is why those providers looked like they summarized badly rather than not at all. Each now has its own endpoint, body shape, auth header, and response parser, with the routing split into a pure function so it is testable.
- **A stalled provider hung the UI (`providers/mod.rs`)**: no request carried a timeout, so an unresponsive local model left "Generating…" on screen indefinitely. Every request now carries one (300 s by default).
- **A provider outage could be presented as a model's work (`processing/llm.rs`)**: `LLMClient::complete` masks failures with canned filler, which suits dictation and is wrong for a summary. `complete_with` reports the failure, and the pipeline chooses its deterministic path deliberately rather than by accident.
- **The system prompt was a prefix on the user prompt (`providers/mod.rs`)**: Ollama requests folded instructions into the prompt as `[System Instructions: …]`, putting Relay's rules and the meeting's own words into one undifferentiated string. It is a `system` field now, which is what makes "the transcript is evidence, not instructions" enforceable.

#### Features

- **Meeting notes (`meetings_v2/types.rs`, `session_store.rs`, `commands.rs`, `MeetingNotesTab.tsx`)**: a Notes tab, second in the row so it is reachable while a meeting is running. Notes are a **source** artifact — `notes.json` beside `session.json`, committed by rename, written only by `SessionStore` — so saving one regenerates nothing and generating a summary never edits one. Two fields: what you wrote during or after the meeting (the point of the feature), and, behind a disclosure, an agenda written beforehand. Absent notes change nothing about the pipeline and are never mentioned in a prompt or a summary.
- **The canonical meeting context (`processing/context.rs`)**: one place decides what a model is told about a meeting — metadata, participants, glossary, notes, transcript. Optional blocks with no content are not rendered at all, because a model shown `# Pre-Meeting Notes\n\nNone.` writes a summary that mentions their absence.
- **Long meetings are read in passes rather than cut off (`processing/{context,extract}.rs`)**: `MeetingContext::windows` splits the transcript into stretches that fit the model's actual window, with a two-segment overlap; Stage A runs once per window and the facts are merged deterministically, deduplicated on normalized text, with ids reissued. Every segment appears in some window, so a decision taken in the first ten minutes cannot vanish because the meeting ran for ninety. A pass that fails does not fail the meeting — the windows that answered are still merged.
- **Decisions carry their reason (`processing/model.rs`)**: `Decision.rationale`. "Move the launch to Monday" is a note; "move it because the payment integration still has three blocking bugs" is a memory, and six weeks later it is the reason, not the date, that someone needs. Kept only when the meeting stated one; hollow answers ("not stated", a restatement of the decision) are dropped, because a hollow "because" is worse than none.
- **Proposals have somewhere honest to live (`processing/model.rs`)**: `KeyPoint.kind` — discussion, proposal, recommendation, disagreement, tradeoff. A schema with one slot for "we could launch Friday" and "let's launch Monday" is an invitation to file the first as the second; the renderer now prefixes a proposal as proposed, and the validator rejects a Decisions line that reproduces one.
- **Risks and blockers are first class (`processing/model.rs`)**: `MeetingFacts.risks`, typed risk / blocker / dependency / constraint, with provenance. A separate collection rather than a flavour of key point, because a blocker is what a reader scans for first. Never inferred from discussion that merely sounds serious.
- **Targeted repair on a validation failure (`processing/{validate,summarize,mod}.rs`)**: rejected prose used to go straight to the deterministic renderer, so one fixable slip — a code fence, an opening "Here is the summary", forty words over — cost the whole model-written summary. `repair_feedback` now maps failed issue codes to corrective instructions and one corrected attempt is made with the request otherwise unchanged. Not the same prompt again: an identical request has no reason to produce a different answer. Limited to one retry; `SummaryArtifact.repair_attempted` records it.
- **Summary instructions (`settings/mod.rs`, `MeetingsSettings.tsx`)**: standing instructions for how your summaries should read. Explicitly subordinate to the accuracy rules in the contract, so nothing written there can make Relay record an owner or a deadline the meeting did not establish.
- **A summary quality evaluation set (`processing/eval.rs`)**: seven meetings with hand-checked expectations and a model-free scorer over decision recall, action recall, owner and deadline accuracy, rationale preservation, open-question and risk recall, detail preservation, noise suppression, repetition, structure, and length. Hallucination has no threshold — one invented owner, deadline, or decision takes a case to zero. Closes gap 10: a prompt or threshold change can now be measured rather than spot-checked.

#### Improvements

- **Length adapts to the meeting (`processing/length.rs`, `modes.rs`)**: the budget is proportional to the transcript, floored so a short meeting still gets a usable record, capped by the mode, with a topic band from `meeting_transcript_summary.md` §6 judged on surviving words rather than wall-clock. A mode now decides *depth*, never an absolute length — which is what keeps "Detailed" from meaning "long" — and every mode is bound by the same floor: brevity comes out of the explanation, never out of a decision, commitment, owner, deadline, or open question.
- **The summary contract is a hierarchy, not a paragraph (`processing/summarize.rs`)**: sixteen numbered sections — role, objective, source, accuracy, include, exclude, be concrete, rationale, uncertainty, attribution, depth, structure, output-only, plus notes, presentation, and user instructions when they apply. The previous version put the prohibitions in the middle of prose and they were the part a small local model lost.
- **Stage B can finally organize by topic (`processing/summarize.rs`)**: key points reach it grouped under their topics instead of as a flat list that had already discarded the grouping, and any point that is not plain discussion is labelled.
- **The output structure matches the shipped rules (`processing/summarize.rs`)**: `## Overview` · `## Discussion` with a `###` per topic · `## Decisions` · `## Action Items` · `## Risks & Blockers` · `## Open Questions`, per `meeting_transcript_summary.md` §5. The deterministic renderer produces the same shape, carrying rationale and risks, so a provider outage does not quietly cost the most valuable part of the record.
- **More is validated deterministically (`processing/validate.rs`)**: required first heading, no preamble addressed to the user, no empty or placeholder-filled section, no risks section when the facts hold no risks, no Decisions line reproducing a recorded proposal, and length against the meeting's own budget.
- **Stage A states its source rules only when they apply (`processing/extract.rs`)**: how to weigh notes against the transcript, and how to read notes written beforehand, appear only when such notes exist. Conflicting sources are left conflicting — neither is declared the winner.

#### Tests

- 407 pass. `native/src-tauri/src/meetings_v2/processing/tests.rs` grows the pipeline suite: repair accepted and repair exhausted, chunked extraction with the opening decision surviving, a failed pass not costing the passes that worked, regeneration reading the original facts rather than the previous summary, notes reaching Stage A, absent notes changing nothing, pre-meeting notes never becoming a section, conflicting notes left conflicting, and notes staying byte-identical across a summary run.
- `processing/eval.rs` adds the scorer's own tests — paraphrase credited, omission not rewarded, an invented owner or deadline caught — plus two guards that keep the cases honest: a forbidden claim may not be satisfied by the transcript itself, nor be a tense flip of something the meeting did say.
- `processing/length.rs` pins the regressions in both directions: a 250-word transcript no longer gets a 550-word allowance, a 9,000-word one is no longer capped at 550, "detailed" on a tiny meeting stays smaller than "concise" on a long one, and a slight overrun is recorded rather than costing the summary.
- `providers/mod.rs` pins the window, the sampling, the system-prompt field, and that no cloud provider is routed to another's endpoint.

#### Notes for existing meetings

`PROCESSING_VERSION` is 4. Facts extracted under v1–v3 load unchanged but carry
no rationale, classify every point as plain discussion, and hold no risks; a
summary regenerated from them is thinner than one regenerated after a forced
re-extraction. Nothing needs migrating — derived data is always recomputable, and
the raw transcript is untouched.

## [0.15.1] - 2026-08-28

### Meetings: A Working Pill Waveform & a Self-Contained Recording Pill

**Type**: patch — `native/` only (`native/src-tauri/src/meetings_v2/capture.rs`, `native/src-tauri/src/overlay.rs`, `native/src/components/meetings_v2/{MeetingRecordingOverlay,MeetingPillMark}.tsx`).

#### Fixes

- **The pill's waveform now moves (`meetings_v2/capture.rs`)**: `meter_level` scaled RMS linearly (`rms * 5.0`), so a normal speaking voice — around -34 dBFS RMS — reached 0.1–0.4 of full scale and drew bars two or three pixels tall. The levels were arriving the whole time; they were unreadable. The meter now maps a decibel window (-55 dB to -15 dB) onto bar height, which is what a level meter is, and gates anything at or below the pipeline's existing audibility threshold to zero so silence is genuinely flat rather than animated room tone. Display only — every audibility decision already used `raw_rms` directly, so what the recorder considers audible is unchanged.

#### Improvements

- **The pill is one object (`MeetingRecordingOverlay.tsx`)**: the status dot, elapsed time, and waveform sit inside a single `rounded-lg` surface, and hovering opens pause and stop **inside that same surface** rather than floating them alongside it. Nothing is attached to the pill; the pill is the whole object.
- **The waveform shows both channels again, readably (`MeetingPillMark.tsx`)**: one mirrored strip on a shared timeline — the microphone above the centreline in indigo, the meeting's audio below it in sky. The previous pair of separate opposing meters needed MIC and SYS labels to be read at all, and a single combined meter could not say who was talking; a mirrored strip says it without a legend. Twenty bars, just under a second of history.
- **The spiral mark is gone (`MeetingPillMark.tsx`)**: the pill's identity is now the waveform itself, which is the part that carries information. `MeetingPillSpiral` is removed rather than left unused.
- **The window still grows only while the controls are open (`overlay.rs`)**: resting 248×52, hovered 330×52, right-edge anchored. The window is transparent but still takes clicks, so unused width is an invisible dead zone rather than free space. Measured against every state, including an hour-long timer with a capture warning showing: 189–238 px resting, 261–310 px hovered.

#### Tests

- `the_meter_puts_conversational_speech_in_the_visible_middle` pins the regression: silence and room tone read zero, speech at -34 dBFS lands between 0.35 and 0.75 of full scale, and the curve is monotonic across the audible range.

## [0.15.0] - 2026-08-28

### Prompt Mode Removal & Architectural Archiving

**Type**: minor — `native/` only (`native/src-tauri/src/{settings/mod,commands,lib,hotkeys/mod}.rs`, `native/src/{App,types/index}.ts`, `native/src/components/{common/NativeSidebar,voicenotes/VoiceNotePage,scribble/ScribbleDetailEditor,settings/ProviderSettings,capture/{DictationPill,PillSettingsPopover,PillTypes}}.tsx`, `remove_prompt.md`).

Prompt Mode was completely removed as a standalone product mode from Relay, and its architectural context and product philosophy were preserved in `remove_prompt.md`.

#### Removals

- **Prompt Mode UI & components (`components/prompts/*`)**: Deleted `PromptsPage.tsx` and `PromptTransformModal.tsx`. Removed the Prompts navigation tab, Wand actions in Voice Notes and Scribbles, and Prompt hero headers.
- **Prompt Mode settings & hotkeys (`settings/mod.rs`, `ProviderSettings.tsx`, `hotkeys/mod.rs`)**: Removed `PromptSettings`, `PromptItem`, default prompts, prompt hotkey configuration, and dynamic prompt hotkey registration hooks.
- **Prompt command (`commands.rs`, `lib.rs`)**: Removed the `execute_prompt` command and its invoke handler registration.

#### Preserved

- Shared LLM infrastructure (`LLMClient`, `ProviderConfig`, Ollama / cloud integration).
- Meetings processing, intelligence extraction, action items to Kanban tasks, summaries.
- Universal Dictation, Voice Notes capture, Scribble markdown management & enrichment.
- Universal `HotkeyRecorder` component and OS global shortcut management.

## [0.14.0] - 2026-08-27

### Meetings v2.5: Utterance-Level Speaker Attribution, Vocabulary at the Recognizer, To-dos as Tasks & a Minimal Right-Edge Pill

**Type**: minor — `native/` only (`native/src-tauri/src/meetings_v2/{capture,worker,types}.rs`, `native/src-tauri/src/meetings_v2/processing/{tasks.rs (new),mod,model,normalize}.rs`, `native/src-tauri/src/capture/stt.rs`, `native/src-tauri/src/{commands,lib,overlay}.rs`, `native/src/components/meetings_v2/*`, `native/src/types/index.ts`, `Meeting-rules/meeting_pipeline_gap_analysis.md`).

Closes gaps 1, 3 and 5 of `Meeting-rules/meeting_pipeline_gap_analysis.md` and
partly closes gap 4. Recording, chunking, the two clocks, pause/resume, crash
recovery, dictation, and voice notes are untouched.

#### Features

- **Speaker attribution resolves per utterance instead of per 30-second chunk (`meetings_v2/capture.rs`, `capture/stt.rs`, `meetings_v2/worker.rs`)**: the mixer already measured per-source RMS sample by sample in order to set `mic_had_audio` / `sys_had_audio`, then summed it across the whole chunk and kept one boolean per source. In a real two-way conversation almost every 30-second window contains both sources, so attribution resolved to `Mixed` — meaning no speaker — for most segments, and `qualify.rs` demoted every action-item owner it had carefully computed to `Unassigned`. Three changes together fix that:
  - `SliceAccumulator` now buckets channel energy per second and each `AudioChunk` carries a `channel_track: Vec<ChannelEnergy>`. Both slice sizes are exact multiples of the bucket, so the steady state never splits one. The chunk-wide booleans are recovered by summing the buckets and are arithmetically identical to the previous single running total, so no existing consumer changes behaviour.
  - `SttEngine::transcribe_utterances_with_config` returns Whisper's own timed segment spans (`SttUtterance`, with `no_speech_prob` kept for diagnostics) rather than one concatenated string. `transcribe_with_config` now joins those utterances, so the text path and the utterance path cannot disagree about what was said.
  - `attribute_utterances` matches each span against the chunk's track, weighting overlapping buckets by how much of each falls inside the span, so a bucket straddling a speaker change contributes to both neighbours in proportion instead of marking both ambiguous.
- **Microphone bleed is rejected without guessing (`meetings_v2/capture.rs`)**: device-level capture has no acoustic isolation, so with speakers rather than headphones the microphone hears the remote party and every utterance would register both channels. `resolve_utterance_channel` requires roughly a 10 dB margin (`CHANNEL_DOMINANCE_RATIO`) before the quieter source is discarded as leakage — a physical property of the room, not an inference about content. Genuine crosstalk fails the margin in both directions and is still left unresolved, and a short utterance mostly inside a straddling second stays unresolved too; both are asserted by test rather than hidden.
- **Normalized segments are one per utterance (`meetings_v2/processing/{normalize,mod,model}.rs`)**: `RawSegmentInput` and `NormalizedSegment` carry an `utterance_index`, segment ids gained an utterance suffix (`seg_00002_001`), and `raw_inputs_from_segment` fans a chunk out into one input per utterance. A chunk with no utterances — a transcript recorded before v2.5, or one Whisper returned no timed spans for — still becomes a single whole-chunk input, so old transcripts read exactly as they did. `PROCESSING_VERSION` is 3.
- **The recognizer gets the vocabulary before it guesses (`commands.rs`)**: `AppSettings::build_stt_prompt` already folded the user's dictionary and the STT custom prompt into one string and was called from nowhere. `start_meeting_v2` now seeds `initial_prompt` from it. `normalize::apply_glossary` can only repair a near-miss; it cannot recover a project or participant name Whisper never produced anything close to. An explicitly configured STT prompt still wins.
- **Chunk boundaries keep their context without sharing decoder state (`meetings_v2/worker.rs`)**: each chunk still gets a fresh `WhisperState`, so a decoder loop cannot propagate — but the tail of the previous chunk's text (`CONTEXT_CARRY_CHARS`, cut on a word boundary and multibyte-safe) is now carried into the next chunk's prompt, so a sentence spanning a boundary is no longer decoded as two blind halves. Silence, an empty decode, and a failed decode all clear the carry, so context never splices across a gap.
- **Meeting to-dos become Kanban tasks (`meetings_v2/processing/tasks.rs` (new), `commands.rs`)**: the only exit from a meeting was a Scribble, which produces a note rather than a to-do — while a Kanban board sat in the same application. `processing::tasks` maps an action item to a `MeetingTaskDraft`: title trimmed to a board-sized line on a word boundary with the full text kept in the description, assignee resolved through the speaker registry (an id that matches no speaker becomes `Unassigned` rather than leaking onto a board), due date only from a spoken deadline, priority from deadline plus extraction confidence, and a description carrying the meeting, its date, and the transcript segments the commitment was read out of. The draft is deliberately not a `KanbanCard`: `push_meeting_v2_action_items_to_kanban` does that conversion, so the derived layer stays free of the vault's storage types.
- **Adding to-dos twice is safe (`meetings_v2/processing/{model,mod}.rs`)**: `ActionItem` carries a `kanban_card_id`, set by `record_action_item_task` after the card is saved. `pending_drafts` skips anything already pushed, so "Add N to tasks" adds only what is new. The card is saved before the to-do is marked — a card without provenance is recoverable, a to-do marked as pushed with no card behind it is not.

#### Improvements

- **The recording pill is a minimal right-edge capsule (`overlay.rs`, `MeetingRecordingOverlay.tsx`, `MeetingPillMark.tsx` (new))**: a meeting runs for an hour, and for that hour the indicator sits on top of whatever the user is actually working in. At rest it is now a 44×72 vertical capsule anchored to the right edge and centred vertically, showing two things — the mark and one live level meter — and nothing else. The old 640×56 top-centre bar with its status badge, timer, two opposing waveforms, mic/sys labels, and pause and stop buttons is gone. Controls are deferred rather than removed: hovering grows the window and reveals the timer, pause and stop, because a recording you cannot stop from the indicator representing it would be a worse pill, not a more minimal one. The window is resized on hover rather than left permanently large, since a transparent margin would still swallow clicks for the whole meeting.
- **The meetings UI is flat (`MeetingSummaryTab.tsx`, `MeetingsV2View.tsx`, `MeetingActionItems.tsx`, `MeetingConversationTab.tsx`, `MeetingProcessingStatus.tsx`, `MeetingRelatedList.tsx`)**: the summary now reads as a document rather than sitting in a tinted gradient card, to-dos are a divided list rather than an amber panel, topics and mentions are labelled rows rather than a boxed panel, and every indigo/violet/emerald accent is gone in favour of neutral surfaces with a single lime accent shared with the pill. `rounded-xl` and drop shadows are reduced to hairline borders throughout.

#### Fixes

- **`emit_chunk` no longer needs a `too_many_arguments` exemption (`meetings_v2/capture.rs`)**: passing the energy buckets as one slice replaced two float parameters.

#### Tests

- 330 tests pass. New coverage: per-second bucket resolution and offsets, chunk-sized slices never splitting a bucket, utterances resolving to the source that was speaking, the case chunk-wide flags cannot distinguish, bleed rejection, crosstalk staying unresolved, a straddling bucket being outweighed by an utterance's own seconds, a short utterance inside crosstalk staying unresolved, silent and out-of-range spans, timing rebased onto the session clock, Whisper spans clamped to the audio, pre-v2.5 chunks falling back to chunk flags, prompt composition, the carried tail being bounded and multibyte-safe, and the full action-item-to-task mapping including the push-twice guard.

## [0.13.3] - 2026-08-27

### Docs: Narrow the Meeting Gap Analysis to Bot-Free Architectures Only

**Type**: patch — documentation only (`Meeting-rules/meeting_pipeline_gap_analysis.md`). No source in `native/` or `web/` is touched.

Relay is not building a meeting bot, so comparing its pipeline against products that
join the call as a participant compared it against a pipeline that is handed the
participant roster by the conferencing platform. That comparison could not produce an
actionable finding, and the roster asymmetry was doing too much of the document's
framing work.

#### Documentation

- **Bot-based products removed entirely (`meeting_pipeline_gap_analysis.md`)**: the
  five bot-based teardowns are gone, along with every reference to them in the gap
  ledger, the recommended sequence, and the sources. Nine bot-free products remain —
  Granola, Circleback, Littlebird, Jamie, Meetily, anarlog, OpenWhispr, Whisper
  Notes/Vowen — plus the raw engines. Circleback and Littlebird are restored from the
  original teardown, since they carry the exit-path and pre-meeting-context arguments
  first-hand rather than by analogy to a bot-based tool.
- **§2 reframed from "bot vs bot-free" to the four ways a name gets in**: calendar +
  contacts, channel provenance, transcript context, and diarization + enrollment —
  tabulated against who uses each and what it costs. Relay uses source 2 only, and at
  the coarsest granularity its capture layer permits. This states the constraint from
  inside the bot-free world instead of deriving it from a contrast with bots, and it
  surfaces a gap the previous framing had folded into gap 2: **transcript context is
  nearly free**, because Stage A already reads the whole transcript and could name a
  speaker from "thanks, Pranjali" for the cost of one extra field in a prompt Relay
  already sends.
- **Gap 3 re-grounded on Whisper's own `initial_prompt`** rather than on two cloud
  vendors' custom-vocabulary features. The mechanism is already wired in `stt.rs:239`
  and `:256` and fires only from a manually typed setting — so the gap is wiring, not
  a missing capability.
- **Scope note added** explaining why bot-based tools are excluded, without naming
  them, so the exclusion survives future edits.
- **The teardown's legal note is flagged as architecture-independent**: diarizing
  within a single recording is not the regulatory trigger, persisting an identity
  template across recordings is. That still governs gap 11 regardless of anyone's
  capture architecture.

## [0.13.2] - 2026-08-27

### Docs: Meeting Pipeline Gap Analysis Re-Grounded Against the v0.13.1 Code

**Type**: patch — documentation only (`Meeting-rules/meeting_pipeline_gap_analysis.md` (new), `Meeting-rules/meeting_notes_competitive_teardown.md`). No source in `native/` or `web/` is touched.

The competitive teardown's gap table was written before the 0.13.0 pipeline and the
0.13.1 quality pass shipped, so it now understates what exists and misprices what is
missing. This replaces the analysis half of it with one traced from the checked-out
tree.

#### Documentation

- **`Meeting-rules/meeting_pipeline_gap_analysis.md` (new)**: a stage-by-stage
  comparison of eleven meeting-notes products against Relay's pipeline as actually
  built, on the seven stages Relay's own `processing` module is organized around
  (trigger, capture, transcription, attribution, comprehension, generation,
  correction/exit). Covers Fathom, Fireflies, Otter, Granola, Jamie, tl;dv, MeetGeek,
  Meetily, anarlog, OpenWhispr, Whisper Notes/Vowen, and the raw engines. Every claim
  about Relay cites the file and line that implements it; the app-side mechanisms cite
  their source.
  - Identifies bot-vs-bot-free as the structural fork that determines where speaker
    *names* come from, and therefore why Relay inherited the hard version of
    attribution: a bot gets the platform roster free, a device-level tap gets one
    mixed stream and no roster.
  - Ranks thirteen gaps by what they cost a real meeting rather than by feature
    parity, with the mechanism and the code location for each.
  - Records what Relay already does better than this field — the in-code action-item
    quality gate, Stage B's structural blindness to the transcript, enforced
    source/derived separation, the deterministic summary floor, honest provider-failure
    reporting, and durable audio that never left the machine.
  - Revises the recommended sequence: per-second channel energy in `capture.rs` first
    (it unlocks the owner resolution `qualify.rs` already computes and demotes for lack
    of signal), then calendar attendees, then the action-item exit path.
- **`Meeting-rules/meeting_notes_competitive_teardown.md`**: §4's gap table is marked
  superseded with a pointer to the new document. The per-app research in §2 and the
  biometric-privacy note in §2.9 are unchanged and remain current.
### Prompt Mode Capability Layer & Cross-Object Prompt Funnel

**Type**: minor — `native/` only (`native/src-tauri/src/settings/mod.rs`, `native/src-tauri/src/commands.rs`, `native/src-tauri/src/lib.rs`, `native/src/types/index.ts`, `native/src/components/prompts/PromptTransformModal.tsx` (new), `native/src/components/prompts/PromptsPage.tsx`, `native/src/components/common/NativeSidebar.tsx`, `native/src/App.tsx`, `native/src/components/settings/ProviderSettings.tsx`, `native/src/components/voicenotes/VoiceNotePage.tsx`, `native/src/components/scribble/ScribbleDetailEditor.tsx`, `native/src/components/capture/DictationPill.tsx`).

#### Features

- **Prompt Mode Capability Layer (`settings/mod.rs`, `ProviderSettings.tsx`, `App.tsx`, `NativeSidebar.tsx`)**:
  - Added `PromptSettings` with `enabled: bool` (default `false`) and `prompt_hotkey: String` (default `"Ctrl+Alt+Space"`).
  - When disabled, Prompt-related UI, sidebar tab, and Wand actions are hidden; existing Voice Notes, Dictation, Scribbles, and Meetings remain unchanged.
  - When enabled, unlocks Prompts sidenav tab, Prompt Hotkey configuration in General & Dictation settings, and Wand transformation actions across Voice Notes and Scribbles.
- **Cross-Object Prompt Transformation Funnel (`PromptTransformModal.tsx`, `VoiceNotePage.tsx`, `ScribbleDetailEditor.tsx`, `PromptsPage.tsx`)**:
  - **Voice Note $\xrightarrow{\text{Wand}}$ Prompt**: Wand action on Voice Note cards opens Prompt selection modal to transform speech transcript with AI. Original Voice Note remains intact.
  - **Scribble $\xrightarrow{\text{Wand}}$ Prompt**: Wand action in Scribble detail toolbar allows transforming note content with prompt templates while preserving the source Scribble.
  - **Prompt $\xrightarrow{\text{Scribble}}$ Scribble**: Added "Save as Scribble" (`Sparkles` icon) on prompt transformation outputs to save structured results into the Obsidian-compatible Knowledge Layer.
- **Dedicated Prompt Execution Command (`commands.rs`, `lib.rs`)**:
  - Added `execute_prompt` command leveraging existing unified `LLMClient` (Ollama + Cloud OpenAI/Gemini/Anthropic).

#### Improvements & Alignment

- **Capture Terminology & Execution Path Alignment (`commands.rs`, `DictationPill.tsx`)**:
  - Decoupled Voice Note capture (`mode: "voice_note"`) from Scribble/LLM execution: saves directly to Voice Notes history without triggering unnecessary LLM summaries or creating duplicate scribble files.
  - Pill Voice Note capture and Universal Dictation now share the same optimized Capture STT Profile (`Fast` / `ggml-base.bin` vs `Accurate` / `ggml-small.bin`, 12-thread clamped execution).
  - Pill status copy strictly adheres to: `Listening...` $\rightarrow$ `Transcribing...` $\rightarrow$ `Voice note saved` (or `Inserted into document` for Universal Dictation).

## [0.13.2] - 2026-08-27

### Dictation STT Optimization, Truthful Status Copy & Prompts Library Funnel

**Type**: patch — `native/` only (`native/src-tauri/src/settings/mod.rs`, `native/src-tauri/src/hotkeys/mod.rs`, `native/src/components/capture/DictationPill.tsx`, `native/src/components/prompts/PromptsPage.tsx`, `native/src/components/common/NativeSidebar.tsx`, `native/src/App.tsx`, `native/src/types/index.ts`).

#### Features & Improvements

- **Dictation STT Performance Isolation (`settings/mod.rs`, `hotkeys/mod.rs`)**:
  - Universal Dictation (`Ctrl+Space`) defaults to the fast performance profile (`ggml-base.bin`, 12 clamped CPU worker threads), reducing utterance latency from ~2.4–3.2s down to ~0.8–0.9s.
  - Meeting STT, Voice Notes, and audio recording architectures remain completely isolated and unchanged.
- **Truthful Dictation Pill Status Copy (`DictationPill.tsx`)**:
  - Eliminated the frontend decorative dummy rotation timer (`PROCESSING_CAPTIONS`) that previously cycled through misleading states ("extracting kanban tasks...", "summarizing voice note...", "running pipeline triggers...").
  - Status copy is now path-aware and reflects actual pipeline stages: `"Listening..."` $\rightarrow$ `"Transcribing..."` / `"Saving voice note..."` $\rightarrow$ `"Inserted into document"` / `"Voice note saved"`.
- **Prompts Sidenav & Management Funnel (`PromptsPage.tsx`, `NativeSidebar.tsx`, `App.tsx`)**:
  - Added dedicated **Prompts** tab to the navigation sidebar.
  - Built comprehensive Prompt Library management interface supporting Create, Edit, Duplicate, and Delete prompt templates, text placeholder tags (`{{text}}`), active status toggling, and instant local settings persistence.
  - Dictation Pill remains focused on voice-capture and Voice Notes; Prompt execution is deferred to a future pass.

## [0.13.1] - 2026-08-27

### Meetings: Action-Item Quality Gate, Honest Summary Fallback State & Quality Regression Fixtures

**Type**: patch — `native/` only (`native/src-tauri/src/meetings_v2/processing/{qualify.rs,qualify/tests.rs}` (new), `native/src-tauri/src/meetings_v2/processing/{mod,model,extract,summarize,validate,conversation,modes,store,tests}.rs`, `native/src/components/meetings_v2/MeetingProcessingStatus.tsx`, `native/src/types/index.ts`, `Meeting-rules/meeting_action_items_tasks.md`, `docs/meetings/MEETINGS_INTELLIGENCE_AUDIT.md`).

The Meetings V2 pipeline from 0.13.0 is unchanged in shape. This makes what it
produces trustworthy: real meetings were yielding ~49 action items and a
"Summary unavailable" banner over a summary that existed. Recording, chunking,
the clocks, pause/resume, crash recovery, voice notes, and dictation are
untouched.

#### Fixes

- **A rejected model summary no longer reads as a failed summary (`processing/mod.rs`, `model.rs`)**: when model prose failed validation, its `Error` issues were merged into the *fallback's* validation report before recomputing `passed`, so a valid deterministic summary always ended up with `stages.summary.status = FAILED` and the UI said "Summary unavailable". The two facts are now recorded separately — `provider_output_status` (`ACCEPTED` / `REJECTED` / `UNAVAILABLE` / `NOT_ATTEMPTED`) and `rejected_issues` describe the model's draft; `validation` describes only the prose actually shown; and the stage fails only when the fallback itself fails.
- **The deterministic renderer is no longer judged by a rule written for models (`processing/validate.rs`)**: `validate_summary` decided whether transcript copying was an error by reading `facts.deterministic` — a fact about the facts, not about the prose being validated. It now takes `prose_is_deterministic` explicitly.
- **An id-shaped owner that matches no speaker is unassigned, not a person (`processing/extract.rs`)**: a model citing `speaker_me` against an empty roster produced an action item owned by an external person literally named "speaker_me".
- **"We'll" is no longer treated as a group commitment (`processing/extract.rs`)**: one person saying "we" for the room is not the group taking work on. `Group` now requires the speaker to have said so ("as a team", "between us"); otherwise the work is `Unassigned`.

#### Features

- **Action-item quality gate (`processing/qualify.rs`, new)**: a deterministic gate with exactly one call site, in `extract_facts`, so the model path and the cue-based path both run through it and cannot disagree about what qualifies as work. Candidates pass through evidence selection, the three gates from `Meeting-rules/meeting_action_items_tasks.md` §2, owner resolution, scoring, semantic deduplication, ranking, and the cap:
  - **Gate 1 — durability**: screen sharing, presenting, joining logistics, stepping away, turn-taking, live lookups, note-taking-right-now, and demo narration are rejected before the verb is noticed, because nearly every one of them contains "I'll".
  - **Gate 2 — deliverable**: a candidate must name something that exists once it is done. "Help with this", "look into it", "we'll handle it", and "maintain the log" are rejected unless the description carries a concrete object.
  - **Gate 3 — commitment**: hypotheticals ("we could", "maybe", "in version two"), already-completed work, and observations are rejected. A proposal plus the group's acceptance (§4.3) and an assignment plus acceptance (§4.2) both qualify.
  - **Structural**: decoder loops, collided ASR fragments, and candidates citing no transcript segment are discarded rather than repaired.
  - **The cap of 15 is enforced in code, and is a ceiling rather than a target** — it never adds. Three qualifying candidates return three.
  - **Deduplication is semantic**: "I'll send the mail list", "I'll send you the list of mails", and "I'll share the required email list" become one to-do, keeping the version with an owner and a date and merging the provenance of all three. Two different acts on the same object stay two to-dos.
- **Ownership is demoted, never guessed (`processing/qualify.rs`)**: an action item whose every cited segment had both the microphone and system audio live cannot be attributed from channel data, so it becomes `Unassigned` — attribution here is channel-level, not diarization, and this makes the code say so.
- **Honest summary provenance (`processing/model.rs`, `summarize.rs`)**: `SummarySource` distinguishes deterministic *presentation* (a model understood the meeting; no model wrote this text) from deterministic *extraction* (no model at any stage, points lifted from the transcript). Neither is presented as an AI summary, and the status bar shows "no model" rather than naming a model that wrote nothing.
- **Action-item diagnostics (`processing/qualify.rs`, `model.rs`)**: every candidate gets an in-memory record of its text, owner, verdict, and rejection reason (`MEETING_MECHANIC`, `DEMO_NARRATION`, `ALREADY_COMPLETED`, `HYPOTHETICAL`, `NO_DELIVERABLE`, `NO_COMMITMENT`, `NO_EVIDENCE`, `BROKEN_FRAGMENT`, `DECODER_LOOP`, `DUPLICATE`, `LOW_CONFIDENCE`, `CAP_EXCEEDED`). Only eight counters reach `processing.json` and `processing_log.jsonl` — the log still explains a run without reproducing the meeting, now asserted by test.

#### Improvements

- **Stage A prompt selects rather than collects (`processing/extract.rs`)**: two explicit passes — list candidates, then classify and reject — with the rejection classes named, "15 is a ceiling, never a target", and "a decision is not automatically an action item" stated outright. An optional `candidate_type` field lets the model record its own verdict; only `action` is kept, and it is never persisted.
- **Summary relevance filter (`processing/extract.rs`, `summarize.rs`)**: procedural narration is kept out of key points on both extraction paths, and Stage B is told the five questions a summary answers and which categories never belong in one. Concise is now 3–5 bullets.
- **Readable fallback action items (`processing/extract.rs`)**: the cue-based extractor trims a candidate back to where the commitment starts, so a task reads "I'll send the list of mails that need to go out tomorrow" rather than the whole sentence it was embedded in. Pure substring selection — nothing is rewritten, and the full sentence stays reachable through the cited segment.
- **Long turns no longer become one wall of text (`processing/conversation.rs`)**: attribution is per 30-second chunk, so an uninterrupted speaker previously produced one turn per meeting. A same-speaker run is now broken at a segment boundary past 180 words, keeping the same speaker id — no turn boundary implies a speaker change the data does not support.
- **The UI tells the truth about fallbacks (`MeetingProcessingStatus.tsx`)**: "Generated from fallback because the model output failed validation (…)" with the rejection codes, instead of "Summary unavailable".

#### Tests

- 304 backend tests (up from 292), including 21 unit tests for the gate and end-to-end regression fixtures A–F built from the meetings that produced the bad output: a demo-heavy meeting (expects ~0 action items), a genuine requirements meeting (expects the right small set with owners, a deadline, and a decision kept separate), a noisy ASR transcript (decoder loops and collided fragments invent nothing), an ambiguous owner (never guessed), a long meeting (bounded in every mode, ≤15 items, no duplicates), and a rejected model summary (fallback shown, stage successful, rejection preserved).
- A full 23-chunk meeting run end to end asserts the whole shape at once: 10 candidates → 7 rejected → 3 retained, with every mechanic and demo phrase named in the assertions, and `transcript.jsonl` hashed before and after.
- A companion test drives the model path with a draft containing the hard patterns the cue-based path cannot reach — capability-plus-group-acceptance, a deferred decision, an agreed task nobody took — and asserts the gate keeps all of them while dropping the over-extraction alongside.
- `PROCESSING_VERSION` is bumped to 2: facts extracted under v1 carry action items that never passed the gate.

## [0.13.0] - 2026-08-27

### Meetings: Transcript Separation, Normalization Pipeline, Structured Extraction, Speakers, Extensions & Meeting → Scribble

**Type**: minor — `native/` only (`native/src-tauri/src/meetings_v2/processing/*` (new), `native/src-tauri/src/meetings_v2/{mod,types,worker}.rs`, `native/src-tauri/src/pipeline/{mod,enrichment}.rs`, `native/src-tauri/src/settings/mod.rs`, `native/src-tauri/src/vault/scribble.rs`, `native/src-tauri/src/{commands,lib}.rs`, `native/src/components/meetings_v2/*`, `native/src/components/settings/{MeetingsSettings.tsx,ProviderSettings.tsx}`, `native/src/types/index.ts`, `Meeting-rules/*.md`, `docs/meetings/MEETINGS_INTELLIGENCE_AUDIT.md` (new)).

Turns the meeting feature into a pipeline with a hard boundary between source and
derived data. Recording, the durable 30-second WAV chunking, live STT, crash
recovery, voice notes, and dictation are unchanged.

#### Features

- **Raw / derived separation (`meetings_v2/processing/store.rs`, `model.rs`)**:
  - Derived meeting data now lives in a new `processing.json` beside the recorder's artifacts; `session.json`, `transcript.jsonl`, and `audio/` are opened read-only by the pipeline and never written.
  - `transcript.jsonl` is the immutable raw transcript. Normalization, summarization, a speaker rename, regeneration, and an extension change all leave it byte-identical — asserted by hashing the file across a full pipeline run.
  - Derived data is always recomputable: a corrupt or stale `processing.json` reads as "unprocessed" rather than making a meeting unopenable.
- **Deterministic normalization stage (`processing/normalize.rs`)**: ASR-tag stripping, whitespace collapsing, repeated-word and repeated-phrase (decoder loop) collapsing, isolated-filler removal, glossary correction from the user's dictionary (exact plus one-edit for long terms), and sentence-boundary repair. Every rule is meaning-preserving, records itself per segment for debugging, and runs before any model is invoked.
- **Structured extraction, then prose (`processing/extract.rs`, `summarize.rs`)**: replaces the single LLM call that did comprehension, extraction, and writing at once. Stage A emits a canonical `MeetingFacts` (title, meeting type, topics, key points, decisions, action items, open questions, entities) as JSON; Stage B writes Markdown from those facts and is never shown the transcript. A cue-based deterministic extractor and a deterministic Markdown renderer produce the same shapes when no model is reachable, so a meeting always has structured output and a summary.
- **Summary modes and extensions (`processing/modes.rs`)**: Concise / Standard / Detailed, plus four shipped extensions (Default, Executive Brief, Project Update, Decision Log) and user-defined extensions from settings. Both are presentation layers over the same facts, so changing either re-runs prose generation only — not extraction.
- **First-class action items (`processing/model.rs`)**: structured objects with owner type, owner speaker id, optional ISO deadline, status, confidence, and the transcript segment ids they came from. Checked state is persisted rather than lost on unmount. An owner only resolves to a speaker present in the meeting's registry; a deadline is kept only when a cited segment contains a real temporal expression.
- **Speaker attribution and renaming (`processing/speakers.rs`, `conversation.rs`)**: rung 1 of `Meeting-rules/meeting_speaker_identification.md` — microphone is the local user, system audio is everyone else, and an ambiguous chunk stays unattributed rather than guessed. Ids (`speaker_me`, `speaker_1`) are stable and separate from display names, so renaming updates the conversation and every action-item owner without regenerating anything or touching the raw transcript. Renames survive regeneration.
- **Conversation transcript**: a chronological, speaker-labelled, sentence-grouped projection of the normalized transcript. Turns store speaker ids; names resolve at render time.
- **Validators wired into the pipeline (`processing/validate.rs`)**: summary emptiness and length caps per mode, JSON leakage, transcript copying (six-word overlap, per the rules file), duplicate bullets, invented participants, unsupported decisions, and invented deadlines; action-item owner/description/deadline/duplicate checks; speaker id and identity-merge checks. Model prose that fails with an error is discarded and re-rendered deterministically from the same facts, so the user is never shown a plausible-but-wrong summary.
- **Meeting classification and related meetings (`processing/related.rs`)**: `meeting_type` from a small closed set, plus relational scoring over shared topics, entities, participants, and type. Title similarity alone cannot produce a match — two unrelated "Daily Standup" meetings are correctly not related. No graph infrastructure added.
- **Meeting → Scribble (`vault/scribble.rs`, `commands.rs`)**: `Scribble::from_meeting` mirrors `from_voice_note` and reuses the existing Scribble type, vault, and saved event. The Scribble references the meeting (`source_type = "meeting"`, `source_id`) instead of duplicating it, carries the meeting's already-extracted topics and entities, and the meeting records the Scribble it produced.
- **Per-stage observability (`processing/store.rs`, `mod.rs`)**: an append-only `processing_log.jsonl` per meeting recording stage, status, duration, provider, model, input/output sizes, validator verdict, issue codes, `processing_version`, and `rules_version`. Transcript text and audio are never logged.
- **Settings › Meetings (`settings/mod.rs`, `MeetingsSettings.tsx`)**: Show raw transcript, Generate conversation transcript, Summarize automatically, Default summary mode, Speaker identification, and extension management. Six controls, not one per pipeline stage. Hiding the raw transcript affects visibility only and never deletes it.

#### Improvements

- **Meeting UI opens to Summary (`MeetingsV2View.tsx`)**: three tabs — Summary, Conversation, Raw Transcript — with Summary as the default. "Transcripts" is renamed Raw Transcript and presented as the diagnostic view it is, showing segment ids and per-chunk channel provenance.
- **Honest partial states (`MeetingProcessingStatus.tsx`)**: per-stage outcomes with a targeted retry instead of a generic failure. A meeting whose summary failed still shows its transcript and conversation, and a summary written without a model says so. A skipped stage is distinguished from a failed one.
- **Non-blocking post-recording processing (`commands.rs`)**: the deterministic stages, and optionally a summary, are spawned after a recording is safely persisted. The meeting is openable immediately; nothing in the pipeline can affect the recording clock, live STT, or the chunk queue.
- **Extracted titles surfaced without mutating the source (`commands.rs`, `MeetingsV2View.tsx`)**: the meetings list and detail header prefer the extracted title, with meeting type and open action-item counts, via a single index call. The recorder's own `session.title` is never overwritten.
- **Channel provenance persisted (`meetings_v2/types.rs`, `worker.rs`)**: `TranscriptSegment` gains `mic_had_audio` / `sys_had_audio`, copied from the `AudioChunk` the recorder already measures them on. Additive and `serde(default)`, so existing transcripts deserialize unchanged and read correctly as "channel unknown". No change to the WAV format, chunk duration, or recording clock.
- **Rules files annotated (`Meeting-rules/*.md`)**: resolved the JSON-only vs Markdown-only conflict by documenting where each rule runs — structured JSON between stages, Markdown at the presentation boundary — and recorded which rungs of the speaker-identification ladder are actually implemented.
- **Architecture audit (`docs/meetings/MEETINGS_INTELLIGENCE_AUDIT.md`)**: end-to-end trace of the meeting implementation as it stood, with current/intended/gap per stage, the target architecture, and three documented out-of-scope issues (coarse channel provenance in `capture.rs`, `LLMClient::complete` masking provider failures, no frontend test runner) left unfixed by design.

#### Fixes

- **Retired the single-call meeting summary path (`pipeline/enrichment.rs`, `pipeline/mod.rs`)**: removed `summarize_meeting` and its helpers, which wrote derived `summary` and `action_items` into the source `session.json` and shipped with a failing test (`test_meeting_enrichment_and_title_synthesis` — the prompt contained the word "topics", so `LLMClient`'s heuristic fallback returned scribble-shaped JSON and no meeting summary was produced). `summarize_meeting_v2` now routes to the canonical pipeline. The legacy `session.summary` / `action_items` fields are read-only, so meetings summarized before this change remain readable.
- **Deterministic extraction no longer loses a commitment to a decision**: a sentence carrying both ("we decided to ship Friday and I'll write the changelog") now yields both, rather than whichever cue matched first. Key points are deduplicated so a long meeting with repeated stretches does not fill its summary with one repeated line.

#### Tests

- 141 tests covering the pipeline, including the ten transcript fixtures called for by the spec (clear decisions, ambiguous ownership, repeated STT fragments, poor punctuation, multiple topics, no action items, no decisions, unknown speakers, a long meeting, an obvious meeting type).
- Raw immutability asserted by hashing `transcript.jsonl` before and after normalization, summarization, a speaker rename, regeneration in each mode, and an extension change — and `session.json` compared byte-for-byte across the same run.
- Failure paths driven by a scripted model stand-in: provider unavailable, unparseable JSON, empty prose, mid-pipeline failure, empty transcript, validator rejection, and retry-after-failure.

## [0.12.3] - 2026-08-27

### Settings: Streamlined Navigation, OpenWhispr Clipboard, Startup, Microphone & Dictionary/Snippets Engine

**Type**: minor — `native/` only (`native/src-tauri/src/settings/mod.rs`, `native/src-tauri/src/commands.rs`, `native/src-tauri/src/hotkeys/mod.rs`, `native/src/components/settings/*`, `native/src/components/capture/*`, `native/src/types/index.ts`).

#### Features

- **OpenWhispr-Inspired Dictionary & Snippets Engine (`DictionarySnippetsSettings.tsx`, `settings/mod.rs`, `hotkeys/mod.rs`, `commands.rs`)**:
  - **Dictionary Tab**: Add custom vocabulary words via comma-separated quick input (`Relay, Supabase, John Snow, ARR`) with default tagging, search filtering, delete chips, and Import/Export capabilities.
  - **Custom STT Vocabulary Injection**: User dictionary words are directly compiled into Whisper STT's decoding initial prompt, significantly boosting speech recognition accuracy for technical acronyms and names.
  - **Snippets Tab ("The stuff you shouldn't have to say twice")**: Create spoken trigger phrases that expand into saved text templates, intros, URLs, and complex prompt instructions. Includes full CRUD, active toggles, and pre-populated templates.
  - **Runtime Snippet Expansion**: Transcriptions containing trigger phrases automatically expand before note persistence and field text injection.
- **Clipboard & Text Injection Controls (`settings/mod.rs`, `ProviderSettings.tsx`, `PillSettingsPopover.tsx`, `DictationPill.tsx`)**:
  - Added **Automatic Pasting** setting (`clipboard.auto_paste: bool`, default `true`) to control whether transcribed text is automatically typed into the focused application.
  - Added **Keep Transcription in Clipboard** setting (`clipboard.copy_to_clipboard: bool`, default `true`) to preserve transcriptions in the OS clipboard for manual pasting.
  - Bidirectionally synchronized between Desktop Settings and the floating Dictation Pill popover.
- **Startup Launch Options (`settings/mod.rs`, `ProviderSettings.tsx`)**:
  - Added **Launch at Login** (`startup.launch_at_login: bool`, default `false`) to start Relay automatically in the background on OS startup.
  - Added **Start Minimized** (`startup.start_minimized: bool`, default `false`) to launch Relay silently without displaying the main control panel window.
- **Microphone Hardware & Warm-up Optimization (`settings/mod.rs`, `commands.rs`, `ProviderSettings.tsx`)**:
  - Added system audio input query command (`get_audio_devices`) and rendered an active green microphone status card (`Using: Default - Microphone Array...`).
  - Added **Prefer Built-in Microphone** toggle to prioritize onboard inputs for lower latency.
  - Added **Keep Microphone Warm** selection (`Off`, `15s`, `30s`, `1m`, `5m`) to keep audio streams primed and eliminate warm-up latency.
  - Added **Auto-learn from Corrections** toggle to automatically capture corrected transcriptions into the custom dictionary.

#### Improvements

- **Streamlined Settings Navigation & Deduplication (`ProviderSettings.tsx`)**:
  - Reorganized into 9 clear, dedicated sections: *Account & Identity*, *General*, *Dictation & Audio*, *Dictionary & Snippets*, *Languages & Script*, *AI Models & STT*, *Privacy & Vault*, *Trash & Deleted*, and *Developer*.
  - Removed duplicate, unpersisted mock account card from General settings.
  - Consolidated language and orthography preferences into their own dedicated sub-page.

## [0.12.2] - 2026-08-27

### Dictation: Start & Stop Sound Effects & Settings Toggle

**Type**: patch — `native/` only (`native/src-tauri/src/settings/mod.rs`, `native/src/lib/soundEffects.ts`, `native/src/components/capture/*`, `native/src/components/settings/ProviderSettings.tsx`).

#### Features

- **Dictation Sound Effects Feedback (`soundEffects.ts`, `DictationPill.tsx`)**:
  - Implemented zero-dependency Web Audio API harmonic sound synthesizers for instant acoustic feedback when dictation starts and stops.
  - Ascending two-tone chime (`C5` -> `G5`) with exponential envelope plays smoothly when recording starts.
  - Resolving two-tone chime (`G5` -> `C5`) plays smoothly when recording stops and speech processing begins.
- **Sound Effects Toggle in Settings & Pill Popover (`settings/mod.rs`, `ProviderSettings.tsx`, `PillSettingsPopover.tsx`)**:
  - Added `SoundSettings` schema in Rust (`sound.dictation_sounds: bool`, default `true`).
  - Added **Sound Effects** section to Settings under Universal Dictation with a "Dictation sounds" toggle ("Play a tone when recording starts and stops").
  - Added quick toggle switch in the Dictation Pill popover for instantaneous toggling directly from the desktop overlay.

## [0.12.1] - 2026-08-26

### Meetings: AI Summarization, Action Items (TODOs), Intelligent Titles, Word Count & 30-Day Trash

**Type**: patch — `native/` only (`native/src-tauri/src/meetings_v2/*`, `native/src-tauri/src/pipeline/*`, `native/src-tauri/src/vault/*`, `native/src/components/meetings_v2/*`, `native/src/components/settings/TrashSettings.tsx`).

#### Features

- **AI Meeting Summarization & Action Items (`pipeline/enrichment.rs`, `commands.rs`, `MeetingsV2View.tsx`)**:
  - Added `summarize_meeting_v2` command that analyzes full meeting transcripts using the active LLM provider (or local deterministic heuristics).
  - Produces structured markdown meeting summaries with key takeaways and decisions.
  - Automatically extracts action items / TODOs when concrete tasks or follow-ups were discussed, rendered in a checkable task list.
  - Automatically renames generic default titles (e.g. `Meeting — Aug 26, 2026...`) to intelligent 3–7 word meeting titles while preserving custom titles and displaying date/time in the sub-heading.
- **Transcribed Word Count Tracking (`meetings_v2/types.rs`, `meetings_v2/session_store.rs`, `meetings_v2/worker.rs`, `MeetingsV2View.tsx`)**:
  - Tracked and rendered transcribed word count on sidebar meeting cards (`X words`) and in the meeting details header metadata.
- **Move to 30-Day Trash on Single Confirmation (`vault/mod.rs`, `TrashSettings.tsx`, `MeetingsV2View.tsx`)**:
  - Replaced hard-deletion with a single-confirmation "Move to Trash" flow.
  - Retains all meeting audio chunks and transcripts in `trash/` for 30 days, recoverable or purgeable from Settings.

#### Improvements

- **Clean Meeting Header**: Removed redundant `Mic: Captured` and `Sys: Captured` source pills from the meeting overview header.
- **Tabbed Meeting Details Layout (`MeetingsV2View.tsx`)**:
  - Structured the meeting content into two distinct left-aligned tabs: **Summary** (holding the AI meeting summary & action points) and **Transcript** (holding the raw incremental speech segments with live STT stream support).
  - Automatically activates the Summary tab upon summary generation and provides an integrated CTA in the empty state.

## [0.12.0] - 2026-08-26

### Meetings V2: Low-Latency Live STT, Recording Pill Timer Correctness & Pause/Resume

**Type**: minor — `native/` only (`native/src-tauri/src/meetings_v2/*`, `native/src-tauri/src/capture/stt.rs`, `native/src/components/meetings_v2/*`).

#### Features

- **Pause and resume a meeting recording (`meetings_v2/engine.rs`, `meetings_v2/capture.rs`, `MeetingRecordingOverlay.tsx`, `MeetingsV2View.tsx`)**:
  - New `MeetingState::PAUSED` plus `pause_meeting_v2` / `resume_meeting_v2` commands, exposed as a Pause/Resume control on both the recording pill and the Meetings page.
  - Audio arriving while paused is discarded rather than queued, so the recording resumes contiguously instead of containing the paused interval, and the live transcriber is told about the discontinuity so an utterance is never spliced across the gap.
  - Paused time is tracked separately (`paused_seconds`) and excluded from the recorded duration.
- **Live transcription is now partial-then-final per utterance (`meetings_v2/live_stt.rs`, `types.rs`)**:
  - `LiveTranscriptUpdate` gained `utterance_index`; updates sharing a `segment_id` replace one another as an utterance grows and the last one is `is_final`, so the UI refines text in place instead of appending overlapping fragments.

#### Improvements

- **Live STT latency: a dedicated Whisper context per clock (`capture/stt.rs`, `meetings_v2/engine.rs`)**:
  - Added `StreamingTranscriber`, which owns its own `WhisperContext` and reuses one `WhisperState` for the session. The live clock previously shared `SttEngine`'s single model behind a mutex held for the whole of `whisper_full`, so every live window queued behind the durable clock's 30-second decodes (and behind dictation), and every window re-allocated whisper's ~330 MB of per-state buffers.
  - Live decoding now uses `single_segment` with a clamped encoder context (`audio_ctx = 768`), roughly halving encoder cost per window.
  - `WhisperDecodingConfig` gained `n_threads`; the two clocks now split the available cores instead of each claiming all of them.
- **Live windows grow instead of overlapping (`meetings_v2/live_stt.rs`, `meetings_v2/capture.rs`)**:
  - Frames are 1 s and contiguous; the worker keeps a rolling utterance buffer, re-decodes it each tick, and commits on a silence boundary or a 12 s cap. This replaces 1.5 s windows with 250 ms overlap and word-level prefix de-duplication.
  - Queued frames are drained to the newest before decoding, so latency equals one inference rather than growing with queue depth.
  - Silence never reaches Whisper, and the backlog of chunks awaiting transcription is now surfaced in the Meetings page.
- **Whisper's own logging no longer floods the terminal (`lib.rs`)**: installs whisper-rs's logging hooks, so whisper.cpp/GGML stop writing a token-by-token decode trace to stderr for every window.
- **Honest capture reporting (`meetings_v2/capture.rs`, `types.rs`, `MeetingsV2View.tsx`)**:
  - Failing to open a device is now an error at start rather than a silent recording, and a session recorded with only one source carries a `capture_warning` shown in the UI.
  - New `mic_heard` / `sys_audio_heard` distinguish "a device was bound" from "audio was actually audible"; per-chunk audibility is measured from the chunk's own samples. The Meetings page no longer labels a source that was never captured as "Captured".
  - Level metering and event emission moved off the realtime audio callback onto the mixer thread, and each meter now tracks its own source.
- **Deleting a meeting asks first (`MeetingsV2View.tsx`)**: deletion is permanent and immediate, so it now goes through `ConfirmationModal` instead of firing on a single click of a hover-revealed icon.

#### Fixes

- **The recording pill's timer no longer counts the time between meetings (`MeetingRecordingOverlay.tsx`, `MeetingsV2View.tsx`, `meetings_v2/engine.rs`, `commands.rs`)**:
  - Elapsed time is interpolated from the backend's pause-aware recorded duration instead of the wall clock since `started_at`, and both surfaces reconcile against `get_active_meeting_v2` on an interval. A pill that missed a session's start or terminal event previously kept a stale session on screen and its timer running across meetings; every such divergence is now self-healing.
  - The overlay is raised after recording actually starts and the session is re-announced, so a freshly created overlay webview cannot miss the start event and show a zeroed timer.
- **Stop no longer discards the tail of the transcript (`meetings_v2/worker.rs`, `meetings_v2/engine.rs`)**: the durable worker's stop flag applied a harsher silence gate to every chunk still queued when Stop was pressed, dropping quiet-but-real speech from the end of a meeting. The worker is no longer interruptible and drains fully; capture is stopped first so the tail chunk is flushed.
- **Stop no longer zeroes a session's counters (`meetings_v2/session_store.rs`, `meetings_v2/engine.rs`, `meetings_v2/worker.rs`)**: `session.json` mutations go through a new `update_session` under a store-wide lock instead of saving a `MeetingSession` cloned at start, which had been rolling back the chunk, transcript-segment, and byte counts written during the meeting. `pending_transcription_chunks` is now a real value rather than always zero.
- **`session.json` is written atomically (`meetings_v2/session_store.rs`)**: write-then-rename, so a crash mid-write cannot leave truncated JSON — which silently hid the meeting from the sessions list even with its audio intact on disk.
- **Concurrent and stale stops are handled (`meetings_v2/engine.rs`, `commands.rs`)**: `stop`, `pause`, and `resume` take a session id and reject a request naming a session that is no longer current, so a surface that outlived its meeting cannot act on a newer recording. A second stop returns the in-flight session instead of a spurious "nothing to stop" error, and the pill stays up showing "Finalizing" until the session is genuinely finished.
- **Teardown no longer blocks the async runtime (`meetings_v2/engine.rs`)**: stop joins audio threads and drains the transcription queue inside `spawn_blocking` rather than on an executor thread.
- **The audio mixer no longer drifts out of alignment (`meetings_v2/capture.rs`)**: with both streams live, samples are consumed in exact lockstep instead of consuming the faster stream and zero-padding the slower one, which had mixed real samples against padding and then against future audio, diverging further for the rest of the meeting. A stream that goes quiet at the device level (loopback with nothing playing) still cannot stall the recording: misalignment is capped at 250 ms.
- **Sessions interrupted while finalizing are recovered (`meetings_v2/session_store.rs`)**: `FINALIZING` is included in the startup interrupted scan, so a crash during the merge/note step no longer leaves a session stuck in that state forever.

#### Tests

- Added 14 tests across `meetings_v2` covering the mixer's lockstep and lag-allowance behaviour, per-source audibility, utterance commit boundaries, session-id fencing, clock thread budgets, concurrent session-metadata updates, atomic writes, and recovery of sessions interrupted while finalizing.

## [0.11.0] - 2026-08-26

### Meetings V2: Dual-Source Capture, Synchronized Temporal Mixing & Low-Latency Live STT (~1.5s)

- **Complete Legacy Meetings Deletion & Archaeological Archive (`docs/meetings/MEETINGS_LEGACY_REMOVAL.md`)**:
  - Fully purged all legacy Meetings V1 debris, unblocking a clean and reliable rebuild from scratch.
- **Two-Clock Decoupled STT & Recording Architecture (`meetings_v2/engine.rs`, `live_stt.rs`, `capture.rs`)**:
  - **Clock A (Durable Recording Clock)**: Continuous 30-second chunk slicing (`audio/chunk_NNNN.wav`), full audio reconciliation (`audio_full.wav`), incremental JSONL logging (`transcript.jsonl`), and crash recovery remain 100% authoritative and uninterrupted.
  - **Clock B (Live STT Clock)**: Broadcasts 1.5s rolling audio frames (with 250ms overlap) to a dedicated `LiveSttWorker` over a bounded sync channel (`sync_channel(8)`), reducing transcript latency from ~30.7s down to ~1.6s.
  - **Backpressure & Fault Isolation**: Dropping live frames under heavy CPU load never interrupts or pauses recorded audio persistence.
- **Synchronized Temporal Audio Mixer (`meetings_v2/capture.rs`)**:
  - Replaced sequential chunk concatenation with independent sample-by-sample lockstep mixing of Microphone and System Loopback streams (`soft_mix(mic[t], sys[t])`) with soft-saturation limiting and zero-padding for silent streams.
- **DictationPill Waveform Alignment (`MeetingRecordingOverlay.tsx`)**:
  - Directly adopted DictationPill waveform geometry (12 bars per stream, `w-[2.5px]`, `gap-[2.5px]`, `rounded-sm`, `h-[22px]`, `duration-75` transition).
  - Left wave (Emerald `MIC`) scrolling left-to-right from speaker's voice; Right wave (Blue `SYS`) scrolling right-to-left from meeting participants.
- **Repetition Loop & Language Hallucination Elimination (`meetings_v2/worker.rs`, `live_stt.rs`)**:
  - Enforced default fallback language `"en"` when auto-detect was unconstrained, eliminating Whisper repetition loops on background noise.
  - Added low-energy silence gating (`rms < 0.005`) to instantly bypass non-speech frames in `< 1ms`.
  - Implemented prefix overlap deduplication to eliminate repeated words across consecutive live STT windows.

## [0.10.1] - 2026-08-25

### Scribble Lifecycle, Voice Note Merge Synchronization & AI Knowledge Enrichment

- **Voice Note Merge Scribble Synchronization & Consolidation (`vault/mod.rs`, `commands.rs`)**:
  - Implemented `sync_scribbles_for_voice_note_merge`: when Voice Note A is merged with Voice Note B, linked Scribbles are automatically updated with the full merged content, provenance metadata is updated (`source_voice_note_id`, `source_voice_note_ids`, `is_merged: true`, `merged_at`, `source_modality: "VOICE"`), and async re-enrichment is dispatched.
  - Handled dual conversion consolidation: if both Voice Note A and B were independently promoted to Scribbles prior to merging, the primary Scribble is updated with merged content and provenance, while the redundant secondary Scribble is retired to Trash.
  - Emits `SCRIBBLE_SAVED_EVENT` and `SCRIBBLE_ENRICHED_EVENT` on merge, invalidating stale client state immediately.
- **Transcript-Prefix Title Elimination & Conceptual Title Synthesis (`pipeline/enrichment.rs`)**:
  - Built `extract_deterministic_title`: strips conversational filler prefixes (`"Yes — this makes a lot of sense..."`, `"So basically..."`, `"I think..."`), scans markdown headings and core insight markers, and synthesizes a concise 3–8 word conceptual title (e.g. *Local Knowledge Layer & Cloud Integration Strategy* instead of *Yes — this makes a lot*).
- **AI Enrichment Derived State Replacement vs. Accumulation (`pipeline/enrichment.rs`)**:
  - Refactored enrichment pipeline to treat `topics`, `entities`, and `questions` as derived state replacing previous metadata on current content rather than appending indefinitely.
  - Extracts and ranks top 5–7 most relevant domain topics and 5–7 named entities (technologies, tools, organizations).
  - Dynamically synthesizes 3–4 thoughtful AI exploration questions tailored to content and extracted concepts.
- **Robust Deterministic Knowledge Extractor & Heuristic Fallback (`providers/mod.rs`, `pipeline/enrichment.rs`)**:
  - Fixed `LLMClient::heuristic_fallback`: differentiated between Meeting task extraction and Scribble knowledge metadata requests, eliminating empty topics/entities when LLM is offline or unconfigured.
  - Added comprehensive multi-format JSON parsing with fallback to deterministic knowledge extraction on LLM timeout or invalid response.
- **Scribble Merge Knowledge Object Synthesis (`vault/mod.rs`)**:
  - Refactored `merge_scribbles` to generate a fresh unified knowledge object from combined content without compounding raw metadata bags; sets initial semantic title, clean 5–7 topics/entities, and records `source_scribble_ids` provenance.
- **Rich Technical Provenance & Event Sync (`ScribbleDetailEditor.tsx`, `VoiceNotePage.tsx`)**:
  - Enhanced technical provenance inspector in `ScribbleDetailEditor` displaying contributing Voice Note IDs, merged status badge, modality, and last enriched timestamp.
  - Added live `scribble-saved` and `scribble-enriched` listeners to `VoiceNotePage` to maintain accurate `promotedNoteIds` across merge operations.

## [0.10.0] - 2026-08-25

### App-Owned Overlay Meeting Reminder Window & Native Toast Demotion (Reversal of Decision 46)

- **Reversal of Decision 46 & Desktop Toast Limitation Root Cause**:
  - Reverses Decision 46. On Windows desktop, `tauri-plugin-notification` 2.x desktop implementation maps only `title`, `body`, and `icon`, silently discarding `.action_type_id()` and `.id()`. `register_action_types` and interactive action callbacks are mobile-only in the desktop plugin and cannot execute on Windows.
  - Demoted the native Windows OS toast notification to a display-only fallback signal and restored the app-owned desktop floating overlay window (`meeting-reminder`) as the primary interactive surface.
- **Create-Once-and-Hide Overlay Lifecycle (`overlay.rs`, `lib.rs`)**:
  - Created the `meeting-reminder` window at startup in a hidden state (`visible: false`), reusing it across reminder cycles to eliminate creation-race loops, re-show loops, and initial DWM flash.
  - Configured window with `decorations(false)`, `transparent(true)`, `shadow(false)`, `always_on_top(true)`, `skip_taskbar(true)`, `resizable(false)`, `focused(false)`, and `content_protected(true)`.
  - Added screen-share and recording protection (`content_protected(true)`) to prevent reminder cards from appearing in meeting recordings or active screen shares.
- **Show Protocol & Readiness Handshake (`notification_service.rs`, `MeetingReminderWindow.tsx`)**:
  - Implemented two-way show protocol: Rust stages pending sanitized reminder data, frontend overlay mounts and subscribes to events while simultaneously pulling pending data via `get_pending_meeting_reminder`, then signals readiness via `meeting_reminder_ready`.
  - Added a 3000ms safety fallback timer in Rust: if the frontend fails to signal ready within 3 seconds, the overlay is force-shown and logged.
- **Auto-Dismiss Countdown with Hover Pause (`notification_service.rs`)**:
  - Auto-dismiss countdown calibrated per source (30s for detected meetings, 60s for calendar meetings).
  - Hovering pauses countdown; mouse leave resumes with `MIN_RESUME_MS` (5000ms) safety floor to prevent expiration mid-action.
- **Security, Title Clamping & Permission Isolation (`capabilities/meeting-reminder.json`, `notification_service.rs`)**:
  - Created isolated capability `meeting-reminder.json` strictly granting `["core:default", "core:event:default"]` to the `meeting-reminder` window with zero filesystem, shell, or HTTP access.
  - Added text sanitization and clamping (`sanitize_and_clamp_text`) stripping control characters, bidirectional override Unicode codepoints, and capping title/body lengths.
  - Guaranteed all frontend meeting title rendering uses text nodes (zero `dangerouslySetInnerHTML`).
- **Cleaned Dead OS Action Code & Fixed Toast Title Normalization (`App.tsx`, `detection.rs`, `engine.rs`)**:
  - Removed dead `registerActionTypes` and `onAction` action parsing in `App.tsx`.
  - Normalized generic window titles (e.g. `Google Meet` -> `Google Meet Session`) and fixed toast formatting to eliminate duplicate provider strings.

## [0.9.5] - 2026-08-24

### Native OS Notification Delivery, Payload Symmetry & Manifest Version Alignment

- **Tauri Notification Capability Grant (`capabilities/default.json`)**:
  - Added `"notification:default"` permission to `capabilities/default.json` for windows `["main", "dictation-pill"]`, granting the frontend window access to `@tauri-apps/plugin-notification` APIs (`isPermissionGranted()`, `requestPermission()`, `registerActionTypes()`, and `onAction()`).
- **Notification Payload Symmetry & Robust Action Routing (`engine.rs`, `App.tsx`)**:
  - Maintained `i32` numeric hashing for OS toast deduplication and toast replacement in `engine.rs` (`.id(notif_id)`).
  - Attached structured `extra` payload fields (`meeting_id` and `kind`) in Rust reminder notifications.
  - Updated `App.tsx` `onAction` handler to symmetrically inspect `notification.extra` as well as direct properties and delimited strings.
  - Linked counterpart file documentation across `native/src-tauri/src/meetings/engine.rs` and `native/src/App.tsx`.
- **Silent Failure Elimination & In-App Notification Permission Guard (`App.tsx`, `ProviderSettings.tsx`)**:
  - Replaced silent warning suppression in `setupNotifications()` with structured console diagnostics logging permission status and registration outcomes.
  - Added notification permission status check and persistent warning banner under **Settings → Meetings & Calendar** if OS notifications are denied or disabled.
- **End-to-End Diagnostic Tracing (`engine.rs`, `reminders.rs`, `App.tsx`)**:
  - Added `tracing` log events in `reminders.rs` and `engine.rs` when reminders are suppressed (snoozed, already fired, expired past timeout, or actively recording).
  - Added logging of raw notification payloads upon `onAction` events in `App.tsx`.
- **Repository Manifest Version Synchronization & Automated Validation (`VERSION`, `tauri.conf.json`, `package.json`, `Cargo.toml`, `verify-commit-rules.js`)**:
  - Synchronized version string `0.9.5` across `VERSION`, `native/src-tauri/tauri.conf.json`, `native/src-tauri/Cargo.toml`, `native/package.json`, and root `package.json`.
  - Extended `scripts/verify-commit-rules.js` to strictly enforce version consistency across all project manifests on pre-commit and pre-push.

## [0.9.4] - 2026-08-24

### Permanent Removal of Tauri Meeting Reminder Container & Single-Surface Native OS Notifications

- **Permanent Removal of `meeting-reminder` Tauri Window (`overlay.rs`, `MeetingReminderWindow.tsx`, `main.tsx`, `capabilities/default.json`)**:
  - Permanently removed `REMINDER_WINDOW_LABEL`, `REMINDER_SIZE`, `REMINDER_MARGIN`, `ensure_reminder_window()`, `reposition_reminder_window()`, and `compute_reminder_anchor()` from `overlay.rs`.
  - Deleted `MeetingReminderWindow.tsx` and removed all meeting-reminder window routing from `main.tsx`.
  - Removed `"meeting-reminder"` from Tauri capability permissions (`capabilities/default.json`).
  - Preserved the Dictation Pill (`dictation-pill`) window intact.
- **Single-Surface Native Windows OS Notification Architecture (`engine.rs`, `commands.rs`, `App.tsx`)**:
  - Configured Rust meeting engine (`engine.rs`) and Developer Settings simulation (`trigger_mock_meeting_reminder`) to dispatch single, native Windows OS notifications via `dispatch_native_reminder_notification()`.
  - Added native OS notification action category (`meeting-reminder`) with `▶ Record`, `◷ Snooze 5m`, `◷ Snooze 15m`, and `Dismiss` actions.
  - Wired `@tauri-apps/plugin-notification` `onAction` listener in `App.tsx` to handle actions (`start_meeting_recording`, `snooze_meeting_reminder`, `dismiss_meeting_reminder`) and notification body clicks (focuses Relay main window and switches to Meetings tab).
- **Design Exploration Gallery Boundary (`MeetingNotificationGallery.tsx`)**:
  - Retained the design exploration gallery for visual previewing while guaranteeing zero production notification side-effects or overlay windows.

## [0.9.3] - 2026-08-23

### Meeting Notification Popups & Components Sidenav Route

- **Components Sidenav Navigation (`native/src/components/common/NativeSidebar.tsx`, `App.tsx`)**:
  - Added new navigation tab `components-meeting-notifications` and breadcrumb routing (`Components > Meeting > Notifications`).
  - Added dedicated `Components` navigation section in the collapsible sidebar with direct access to meeting notification design options.
- **10 Sleek Interactive System Popup Design Options (`rounded-lg`) (`native/src/components/meetings/MeetingNotificationsDesignGallery.tsx`)**:
  - Expanded the interactive gallery to feature 10 compact, non-intrusive meeting notification popup designs, all formatted with `rounded-lg` (8px border-radius) corners and prominent CTAs:
    1. *Compact HUD Bar*: Top-anchored HUD bar (`rounded-lg`) with live mic activity pulse and "Record Now" button.
    2. *Compact Quick Dock Widget*: Streamlined dock card (`rounded-lg`) with input mode toggles and REC CTA.
    3. *Animated Gradient Border Card*: Shimmering gradient border card (`rounded-lg`) with participant avatar cluster and "Start Recording Now" CTA.
    4. *Stealth Mini Floating Bar*: Ultra-compact 34px height horizontal bar (`rounded-lg`) for zero screen clutter.
    5. *Left-Accent Banner*: Left-border accent card (`rounded-lg`) with speaker badge and glowing red "Start Recording" button.
    6. *Waveform Control Bar*: Dark tech widget (`rounded-lg`) with animated audio frequency visualizer simulation and "Initiate Capture" CTA.
    7. *AI Copilot Quick Toast*: Smart assist toast (`rounded-lg`) with output preset selector chip and "Record & Transcribe" button.
    8. *Corner Action Tray*: 2-row action tray (`rounded-lg`) with participant count badge and "Capture Audio" button.
    9. *Edge-Anchored Mini HUD*: High-contrast HUD card (`rounded-lg`) with live mic input status indicator and "Start STT" CTA.
    10. *Micro Pre-Flight Command Card*: Compact pre-meeting prep card (`rounded-lg`) with mic signal check meter and "Launch Recording" CTA.
  - **Clean Native OS Notification Architecture (`overlay.rs`, `engine.rs`, `commands.rs`, `main.tsx`)**:
    - Completely removed the Tauri WebView `"meeting-reminder"` window to eliminate duplicate notifications and white container rectangle artifacts (Decision 46).
    - Windows native OS Toast Notifications (`tauri_plugin_notification`) handle OS-level desktop meeting alerts cleanly.
    - Floating in-app toast listener (`MeetingReminderToastListener.tsx`) handles clean in-app alerts inside Relay with live Record, Snooze, and Dismiss CTAs.
    - Zero webview container artifacts, zero ghost windows, and zero duplicate notifications.

## [0.9.2] - 2026-08-23

### Repository Licensing & Governance Migration (AGPL-3.0-only)

- **Open-Source Licensing Migration (`LICENSE`, `package.json`, `native/src-tauri/Cargo.toml`, `native/package.json`, `web/package.json`)**:
  - Migrated Relay's top-level open-source license from MIT to GNU Affero General Public License Version 3 (`AGPL-3.0-only`).
  - Added complete official AGPLv3 legal text with copyright `Copyright (C) 2026 Relay Maintainers`.
  - Updated package manifest license metadata across all project surfaces (`native/src-tauri/Cargo.toml`, root `package.json`, `native/package.json`, `web/package.json`).
- **README Authoring Rules & Pre-Commit Verification (`rules/readme.md`, `scripts/verify-commit-rules.js`, `.git/hooks/`)**:
  - Created `rules/readme.md` detailing machine-readable rules for creating, auditing, and maintaining `README.md`, including AGPLv3 strategy, pre-production status rules, open-source core vs. commercial services boundaries, and trademark guidelines.
  - Implemented `scripts/verify-commit-rules.js` and installed Git `.git/hooks/pre-commit` and `.git/hooks/pre-push` to automatically enforce versioning, changelog, and README compliance on commits/pushes.
  - Updated `README.md` to reference `AGPL-3.0-only` and `rules/global.md` & `AGENTS.md` to integrate `rules/readme.md`.

## [0.9.1] - 2026-08-23

### Meetings Directory UI Restructure & Calendar View

- **Simplified Meeting Filters (`native/src/components/meetings/MeetingPage.tsx`)**:
  - Replaced the nested Standalone/Series/Calendar filter system with a flat, unified view.
  - Implemented intuitive top-level tabs: `All`, `Scheduled`, and `Completed/Recorded`.
- **Custom Agenda Calendar View (`native/src/components/meetings/CalendarView.tsx`)**:
  - Implemented a custom `CalendarView` component that renders on the right-hand side when no specific meeting is selected.
  - Replaced the blank empty state with a lightweight, visual schedule of upcoming meetings grouped by day.
- **Streamlined Capture Actions**:
  - Simplified the calendar import button text from `+ Add & start in Relay` to a cleaner `Start Capturing`.
  - Refined the "Calendar Sync" modal trigger button to dynamically display a green `Calendar Connected` state.
### Navigation & Layout Modernization (shadcn sidebar-07)

- **shadcn `sidebar-07` Sidenav Pattern (`web/`, `native/`)**:
  - Implemented collapsible icon sidebar (`w-64` expanded, `w-12` collapsed) with custom cubic bezier smooth transitions.
  - Added workspace / team switcher header with local vault vs hybrid cloud sync options.
  - Implemented core navigation with tooltips, active tab indicators, and quick vault shortcuts.
  - Added user profile footer popover menu with avatar, plan status, changelog, and settings triggers.
  - Created Radix UI primitives (`dropdown-menu`, `tooltip`) for native frontend.

## [0.9.0] - 2026-08-22

### Phase 11D.1: Relay Security & Foundation Hardening Pass

- **Supabase RLS & Ingestion Hardening (`supabase/migrations/20260822_relay_account_schema.sql`)**:
  - Replaced open RLS write policies with secure `SECURITY DEFINER` PostgreSQL RPC functions: `register_installation_heartbeat` and `ingest_diagnostic_event` with strict input validation.
  - Locked down `SELECT` on `installations` and `diagnostics_events` tables strictly to `service_role` (or authenticated installation owner).
- **Google Calendar OAuth Tokens Migrated to OS Keyring (`meetings/calendar.rs`)**:
  - Relocated all Calendar OAuth token storage from `vault/google_calendar_token.json` to the OS Keyring / Credential Manager (`keyring` crate) with an encrypted fallback in `.relay/config/`.
  - Added automated migration that purges any legacy token files in `vault/` to ensure zero secrets ever reside in the user's markdown vault.
- **Account Deletion $\ne$ Local Vault Deletion (`identity/mod.rs`, `commands.rs`, `AccountSettings.tsx`)**:
  - Implemented `delete_relay_account` Tauri command and Settings UI modal that deletes cloud profile state while leaving local markdown notes, recordings, scribbles, and meetings 100% untouched.
- **First-Run Onboarding Vocabulary Refinement (`WelcomeModal.tsx`)**:
  - Refined first-run choices to clearly present **"Continue with Google"** (Primary CTA) vs. **"Continue Locally"** (Secondary CTA: *"No account required. Your data remains entirely on this device."*), with zero "Skip" terminology.
- **Opt-In Telemetry by Default (`settings/mod.rs`)**:
  - Set `allow_anonymous_diagnostics` to default to `false` (opt-in privacy by default).

### Phase 11D: Relay Identity, Product Account & Local→Hybrid Foundation

- **Relay Account $\neq$ Local Vault Invariant (`native/src-tauri/src/identity/`)**:
  - Implemented the `RelayAccount` domain model (`identity/models.rs`) clearly separated from the local markdown vault. Signing in identifies the user and installation but strictly never uploads or moves any local notes, recordings, audio, scribbles, or meetings to the cloud.
  - Added support for `AccountMode::Local` and `AccountMode::Hybrid` with `SubscriptionInfo` scaffolding (`SubscriptionPlan::Free`, `SubscriptionPlan::Hybrid`).
- **Desktop Google Sign-In & Secure Token Storage (`identity/oauth.rs`, `identity/tokens.rs`)**:
  - Implemented real desktop Google OAuth 2.0 PKCE loopback server on `127.0.0.1:{port}/oauth/callback` with system browser launch and responsive HTML success/failure pages.
  - Secured OAuth tokens (`access_token`, `refresh_token`) inside the OS Keyring / Credential Manager (`keyring` crate) with an encrypted local fallback file, strictly outside `localStorage` and outside the vault.
  - Sign-out cleanly purges tokens from secure storage and reverts to Local mode while keeping the user's local vault completely untouched.
- **Stable Anonymous Installation Identity (`identity/installation.rs`)**:
  - Implemented UUID v4-based anonymous installation ID generated once and persisted in `.relay/config/installation.json`.
  - Survives app restarts and updates without invasive hardware fingerprinting.
  - Added UI masking (`••••••••-••••-XXXX`) with instant click-to-copy for diagnostics and support.
- **Privacy-First Diagnostics & Telemetry Firewalled Abstraction (`diagnostics/mod.rs`)**:
  - Built `DiagnosticsService` with an absolute privacy firewall: payloads strictly contain anonymous system metadata (`installation_id`, `account_id`, `relay_version`, `platform`, `os_version`, `event_type`, `timestamp`).
  - Strict Guarantee: Zero note contents, transcripts, audio recordings, or knowledge graph data can ever be collected or transmitted.
  - Controlled by an explicit user consent toggle in Settings.
- **Update Service Abstraction (`updates/mod.rs`)**:
  - Created `UpdateService` with semver comparison and offline resilience, gracefully handling network disconnections without interrupting local app usage.
- **Supabase Cloud Backend, Auth & Database Schema (`supabase/`, `native/src-tauri/src/identity/supabase.rs`)**:
  - Implemented complete PostgreSQL migration schema ([`supabase/migrations/20260822_relay_account_schema.sql`](file:///d:/Projects/Relay/supabase/migrations/20260822_relay_account_schema.sql)) with Row Level Security (RLS) for `relay_accounts`, `installations`, `diagnostics_events`, and `app_releases`.
  - Added Rust backend `SupabaseClient` for async profile upserts, installation tracking, privacy-safe telemetry dispatch, and release update checks.
  - Added `.env` configuration support with `dotenvy` in Rust backend and template in `.env.example`.
  - Upgraded Google OAuth flow to support both Supabase Auth broker (`/auth/v1/authorize?provider=google`) and direct Google OAuth with loopback hash-fragment bridge.

### Phase 11C: First-Class Meetings Capture Surface, Recurring Series & Meeting-to-Knowledge Promotion

- **First-Class Persistent Meetings Domain Model (`native/src-tauri/src/vault/meeting.rs`, `vault/mod.rs`)**:
  - Implemented `Meeting`, `MeetingSeries`, and `MeetingActionItem` structures stored in `vault/meetings/` and `vault/meeting_series/` with YAML frontmatter + Markdown body formatting.
  - Enforced architectural separation: *Meeting $\ne$ Scribble*. Moving or extracting notes into Scribbles never deletes, mutates, or collapses the persistent Meeting source record.
  - Added discrete Standalone Meeting support alongside Recurring Meeting Series groupings (individual occurrences are independently addressable, series views display the newest occurrence first).
- **Real Google Calendar OAuth 2.0 & Event Synchronization (`meetings/calendar.rs`, `commands.rs`)**:
  - Completely purged all mock/dummy calendar events, fake attendees, and hardcoded mock data.
  - Implemented real Google OAuth 2.0 PKCE / Loopback authorization on `127.0.0.1` using the minimum read-only scope (`https://www.googleapis.com/auth/calendar.events.readonly`).
  - Added secure token persistence in `vault/google_calendar_token.json` with automated token refresh before expiry.
  - Added real Google Calendar events synchronization querying primary calendar with singleEvents expansion for recurring series.
  - Built full connection lifecycle in `CalendarSyncModal.tsx`: disconnected state with `[Connect Google Calendar]`, optional custom Client ID/Secret configuration, connected state displaying real authenticated account email, `[Sync Now]`, and `[Disconnect]`.
- **Real Browser & Native Conferencing Window Detection (`meetings/mod.rs`, `commands.rs`)**:
  - Implemented active video conferencing detection via Windows Win32 `EnumWindows` and `GetWindowTextW` for Google Meet (Chrome, Edge, Firefox, Brave, Opera), Zoom, Microsoft Teams, and Cisco Webex.
  - Added window title sanitization (`clean_meeting_window_title`) to extract clean meeting topics from browser window frames.
  - Maintained strict consent-first model: `MeetingDetectionPopup.tsx` prompts the user (`[Start Recording]`, `[Not this meeting]`) and **never** records automatically without explicit consent.
- **Visual Graph Distinction & Candidate Suggestion Isolation (`graphRenderer.ts`, `GraphSettingsPanel.tsx`, `vault/mod.rs`)**:
  - Rendered `DERIVED_FROM` provenance edges with distinct dashed strokes (`ctx.setLineDash([4, 4])`) and purple/slate tones, distinguishing them from solid semantic knowledge edges.
  - Styled Meeting source nodes with distinct purple color and concentric double-ring accents.
  - Added `Meeting Sources & Provenance` filter toggle in graph settings.
  - Enforced that candidate scribbles from AI enrichment remain strictly in `meeting.candidate_scribbles` as suggestions until the user explicitly accepts them.
- **Meeting Audio Recording Pipeline & AI Enrichment (`native/src-tauri/src/pipeline/enrichment.rs`, `commands.rs`)**:
  - Integrated meeting capture with Relay's local Whisper STT engine and local LLM pipeline.
  - Added `enrich_meeting` to asynchronously extract executive summaries ($\ge 100$-word threshold rule), explicit decisions, structured action items (with assignees & priorities), open questions, and candidate scribble suggestions.
- **Full Meeting Management Dashboard & Detail View (`native/src/components/meetings/`)**:
  - `MeetingPage.tsx`: Master-detail navigation supporting Standalone and Recurring Series groupings, search across transcripts/action items/participants, and 1-click Google Calendar sync.
  - `MeetingDetailView.tsx`: Complete control surface featuring audio recording triggers, progressive disclosure tabs (*Notes & Summary*, *Decisions & Tasks*, *Open Questions*, *Audio Transcript*, *Derived Scribbles*), editable meeting metadata, and markdown export.
  - `MeetingModal.tsx` & `CalendarSyncModal.tsx`: Creation modals for standalone meetings, recurring series cadences, and calendar event imports.
- **Knowledge Graph Provenance & 30-Day Trash (`native/src-tauri/src/vault/scribble.rs`, `vault/mod.rs`)**:
  - Scribbles created from meetings (`create_scribble_from_meeting`) automatically carry `source_type: "meeting"`, `meeting_id`, and `meeting_series_id` provenance metadata.
  - The Obsidian-inspired Knowledge Graph automatically connects derived Scribbles to a virtual `source` node via `DERIVED_FROM` edges.
  - Deleting a meeting moves it to the 30-day Trash without affecting or orphaning any derived Scribbles.

## [0.8.2] - 2026-08-22

### Scribbles AI Enrichment Polish, Summary Thresholds, Exploration Fallbacks & Collapsible Content

- **Structured AI Summarization & Threshold Enforcement (`native/src-tauri/src/pipeline/enrichment.rs`, `ScribbleDetailEditor.tsx`)**:
  - Added strict $\ge 100$-word threshold: the "Summarise" action button only appears and triggers for long thoughts ($\ge 100$ words).
  - Short notes under 100 words bypass automatic summarization during enrichment.
  - Summaries render concise takeaways with bold lead-ins, numbered badges, structured bullets, and dark-theme Mermaid flowcharts via the custom `MarkdownView` component.
- **Synchronous Full-Refresh on Re-Enrichment (`native/src-tauri/src/commands.rs`, `ScribbleDetailEditor.tsx`)**:
  - `trigger_enrich_scribble` returns the enriched `Scribble` directly, enabling immediate state synchronization for title, topics, entities, summary, and questions without waiting for async event listeners.
- **Merged Note Title Derivation & Section Header Sanitization (`native/src-tauri/src/vault/mod.rs`, `enrichment.rs`)**:
  - Sanitized internal merge section headers to prevent placeholder markers (e.g. `### [Synthesis: Generating title… + 2 more]`) from leaking into synthesized markdown content.
  - Hardened LLM title derivation to strictly reject echoed placeholder phrases and derive clean concept titles directly from content.
- **Guaranteed AI Exploration Questions (`enrichment.rs`, `ScribbleDetailEditor.tsx`)**:
  - Added flexible serde deserialization aliases (`exploration_questions`, `suggested_questions`, `open_questions`) and dynamic topic-based fallback generation so exploration questions always populate and persist.
- **Collapsible Long Thought Content (`ScribbleDetailEditor.tsx`)**:
  - Added a collapsible **"Read More" / "Show Less"** toggle with bottom gradient fade for thoughts exceeding 200 words, preventing infinite scroll while preserving full detail on demand.
- **Dangling Knowledge Connections Cleanup (`vault/mod.rs`, `ScribbleDetailEditor.tsx`)**:
  - Cleaned up dangling relationship pointers when source notes are merged or moved to Trash.
  - Filtered rendered knowledge connections to strictly display active, non-trashed notes in the vault.

## [0.8.1] - 2026-08-22

### Obsidian-Fidelity Knowledge Graph Rework, Organic Physics & Stable Coordinate Persistence

- **Force-Directed Physics & Organic Equilibrium (`native/src/components/scribble/graph/graphPhysics.ts`)**:
  - Implemented Coulomb electrostatic repulsion with distance clamping, Hooke spring link forces with configurable link distance, center gravity, and velocity damping (`0.88`).
  - Simulation energy decays smoothly to a stable equilibrium without persistent jitter or continuous resource consumption.
  - Interactive force sliders dynamically reheat the live simulation with immediate physical adaptation.
- **Persistent Node Coordinates & Graph Stability (`native/src/components/scribble/graph/graphStorage.ts`)**:
  - Persists node `(x, y)` positions in `localStorage` (`relay_knowledge_graph_positions_v1`), preserving relative structure across sessions.
  - New nodes are placed organically near connected neighbors without scrambling existing graph structure.
  - Added dedicated **Reset Layout** action with confirmation modal to re-simulate positions without wiping graph settings.
- **Independent Camera Architecture (`native/src/components/scribble/KnowledgeGraphView.tsx`)**:
  - Decoupled camera transforms `{ x, y, k }` from node coordinates with zero auto-fitting after loading, dragging, or settling.
  - Added cursor-centered mouse-wheel zoom, drag panning, on-screen zoom buttons, and keyboard controls (Arrow keys, `Shift + Arrow`, `+`/`-`, `0` reset view, `Space` reheat).
- **Interactive Node Dragging & Normalized High-DPI Canvas (`graphRenderer.ts`, `KnowledgeGraphView.tsx`)**:
  - Aligned high-DPI `devicePixelRatio` scaling and CSS world-coordinate hit testing for precision cursor grabbing.
  - Node dragging pins the active node, applies live spring reactions to neighbors, and relaxes smoothly upon release.
- **Obsidian Quieting Hover & Dynamic Label Fading (`graphRenderer.ts`)**:
  - Hovering a node brightly highlights 1-hop connected neighbors/edges while quieting unrelated content to low opacity.
  - Non-linear degree-based node sizing with upper radius limits.
  - Dynamic zoom-dependent text fading with priority rendering (Hovered > Selected > High-degree > Normal) and clean ellipsis truncation.
- **Filters, Dynamic Search-Driven Groups & Local Graph (`GraphSettingsPanel.tsx`, `GraphToolbar.tsx`)**:
  - Configurable filters for Scribbles, Voice Notes, Topics, Entities, Attachments, and Orphans.
  - Search-driven custom color groups updating matching nodes in real time.
  - Scoped **Local Graph** exploration mode with configurable depth (`1`, `2`, `3`).
  - Chronological **Time-lapse** animation revealing nodes by creation timestamp.
- **Relay Context Actions & Inspector (`GraphNodeInspector.tsx`, `ScribbleViewer.tsx`)**:
  - Contextual inspector drawer displaying node metadata, summary, and clickable 1-hop connections.
  - Integrated direct actions: **Open in Editor**, **Connect Scribble**, **Merge Scribbles**, and **Move to Trash**.
  - Streamlined Scribbles navigation to dedicated **Capture**, **Workspace**, and full-screen **Knowledge Graph** tabs.

## [0.8.0] - 2026-08-21

### Phase 11B: First-Class Scribbles Knowledge Layer, Provenance Model & Obsidian-Inspired Knowledge Graph

- **First-Class Persistent Scribbles (`native/src-tauri/src/vault/scribble.rs`, `vault/mod.rs`)**:
  - Implemented the `Scribble` data model with full frontmatter Markdown serialization in `.relay/vault/scribbles/<id>.md`, compatible with Obsidian.
  - Implemented CRUD persistence (`save_scribble`, `get_scribble`, `list_scribbles`, `update_scribble`, `delete_scribble`).
  - Supports multiple capture source types (`voice`, `text`, `file`, `clipboard`, `browser_selection`, `browser_page`, `screenshot`, `meeting`) while preserving complete provenance in `source_metadata`.
- **Zero Impact on Voice Note / Capture UX (`VoiceNotePage.tsx`)**:
  - The `Ctrl + Space` hotkey and Dictation Pill flow remain 100% unchanged with zero modal prompts during recording.
  - Added dedicated **"Save as Scribble"** promotion action to Voice Note cards in `VoiceNotePage.tsx`, creating a new linked Scribble while keeping the original Voice Note and raw audio intact.
- **Asynchronous AI Enrichment (`native/src-tauri/src/pipeline/enrichment.rs`, `commands.rs`)**:
  - Non-blocking background AI enrichment extracts concise titles, summaries, topics, named entities, and suggested concept connections via `LLMClient`.
  - All AI metadata is fully editable by the user (AI suggests, user decides) and resilient to offline/error states.
- **Explicit Relationships Model (`vault/scribble.rs`, `commands.rs`)**:
  - Supported relationship types: `RELATED_TO`, `MENTIONS`, `SAME_TOPIC`, `SAME_PROJECT`, `CONTRADICTS`, `EXTENDS`, `DERIVED_FROM` with origin and confidence tracking without merging independent knowledge objects.
- **Obsidian-Inspired Knowledge Graph View (`native/src/components/scribble/KnowledgeGraphView.tsx`)**:
  - High-performance 2D Canvas graph renderer with real-time force simulation physics.
  - Circular nodes with connectivity-driven sizing (degree scaling), subtle low-weight edges, restrained color-coded node categories, and zoom-dependent intelligent labels.
  - Interactive 1-hop neighborhood highlighting, dragging, zoom/pan controls, and side inspector drawer.
  - Specialized filtering including node types and a dedicated **Orphans** filter for discovering isolated thoughts.
- **Knowledge Workspace Surface (`native/src/components/scribble/ScribbleViewer.tsx`, `ScribbleComposer.tsx`, `ScribbleDetailEditor.tsx`)**:
  - Rebuilt the Scribbles view into a complete Knowledge Workspace with 3 view modes: **Workspace** (List + Detail Editor), **Knowledge Graph** (full-screen Obsidian canvas), and **Split View** (Canvas + Editor).
  - Quick-Create bar supporting instant typed thoughts and file uploads.
  - Real-time search across thoughts, topics, and entities.

## [0.7.3] - 2026-08-21

### STT Reliability: Language Wiring Sync, Audio Telemetry & Frame-Based Voice Activity Detection (VAD)

- **Language Settings Synchronization (`commands.rs`, `stt.rs`, `DictationPill.tsx`, `ProviderSettings.tsx`)**:
  - Connected Dictation Pill language popover to Tauri IPC `save_settings` backend commands and event broadcast (`settings-changed`), ensuring settings persist to `settings.json` and sync live across all windows.
  - Hardened Whisper language configuration in `SttLanguageConfig::from_settings`: auto/empty maps to Whisper auto-detection (`None`), single language maps to locked language tag (e.g. `Some("hi")`), and multilingual pairs (e.g. Hinglish `["en", "hi"]`) safely default to multilingual auto-detection without passing invalid token tags.
- **Audio Measurement & Telemetry (`capture/mod.rs`)**:
  - Implemented `AudioStats` telemetry calculating sample count, duration, RMS, peak amplitude, near-zero percentage, and near-clipping percentage on captured and analyzed audio buffers.
  - Sanitized audio buffers against non-finite values (`NaN`/`Inf`) and clamped samples to `[-1.0, 1.0]`.
  - Analyzed 233 real microphone recordings on disk to verify dynamic range (average RMS 0.0254, max peak 0.7477, 0.00% clipping).
- **Speech Boundary Detection / VAD (`capture/mod.rs`)**:
  - Implemented deterministic frame-based energy VAD (`VadConfig`, `VadResult`) with 20ms frames, adaptive background noise floor estimation (bottom 20% frames), onset requirement (80ms), hangover duration (300ms), and safe pre/post-speech padding (250ms).
  - Validated with an offline batch experiment over 233 real user recordings, achieving an average of 24.1% dead air reduction (~1.77s saved per recording) with $<1\text{ ms}$ processing overhead and zero speech truncation.
  - Short-circuits accidental empty/click recordings (50 files identified) to prevent Whisper hallucinations on silence.

## [0.7.2] - 2026-08-20

### Voice Note Actions (Edit, Delete, Merge), High-Contrast Destructive Tokens & Geometry Polish

- **Backend (`native/src-tauri/src/vault/mod.rs`, `native/src-tauri/src/commands.rs`)**: Added
  `update_voice_note`, `delete_voice_note`, and `merge_voice_notes` Tauri commands and unit
  tests for persistent Markdown vault note editing, deletion, and chronological adjacent note merging.
- **Frontend (`native/src/components/voicenotes/VoiceNotePage.tsx`)**:
  - Removed redundant `"Voice Note"` text labels from each note card in favor of clean timestamps
    and word count chips.
  - Added full interactive action toolbar: **Edit** (inline textarea with <kbd>Ctrl+Enter</kbd>
    save / <kbd>Esc</kbd> cancel), **Delete** (safety confirmation banner), **Adjacent Merge**
    (combines split transcripts into one), and **Copy** (animated 1-click clipboard copy).
  - Enhanced delete confirmation banner with high-contrast, accessible red accents across both
    Light and Dark themes.
- **Design System (`native/src/index.css`, `web/src/app/globals.css`, `native/src/components/ui/button.tsx`)**:
  - Fixed `--destructive` and `--destructive-foreground` design tokens to vivid crimson (`#EF4444`)
    and crisp white (`#FFFFFF`) in both Light and Dark modes.
  - Standardized all button variants (`destructive`, `outline`, `secondary`, `ghost`) to consume
    theme tokens rather than hardcoded slate classes.
  - Standardized UI components across all modules and floating widgets to `rounded-lg` geometry.
  - Implemented custom minimal theme-aware scrollbars (`4px` width, transparent track, `40%` opacity `--muted-foreground` thumb) and `.no-scrollbar` utility across both native and web frontends.
- **Pill Positioning & Alignment (`ProviderSettings.tsx`, `DictationPill.tsx`, `PillSettingsPopover.tsx`, `overlay.rs`)**:
  - Replaced legacy position options with **Bottom Left**, **Bottom Center** (default), and **Bottom Right**.
  - Pushed Bottom Left and Bottom Right anchor positions flush to the monitor's work area edges.
  - Aligned internal pill components (main pill body, keyboard hint bar, notch, and settings popover) to the left on Bottom Left and to the right on Bottom Right.

## [0.7.1] - 2026-08-20

### Dynamic Changelog, 1-Click Theme Toggle & UI Streamlining

- **Backend (`native/src-tauri/src/commands.rs`, `native/src-tauri/src/lib.rs`)**: Added
  `get_app_version` and `get_changelog` Tauri commands that dynamically parse `VERSION` and
  `CHANGELOG.md` at runtime so the release notes modal and footer stay completely up to date
  without hardcoded lists.
- **Frontend (`native/src/components/ThemeToggle.tsx`)**: Streamlined the theme toggle into a
  direct 1-click toggle between light and dark mode with mode-appropriate container styling
  and icons representing the target switch mode (Moon in Light mode, Sun in Dark mode).
- **Frontend (`native/src/components/common/ChangelogModal.tsx`)**: Converted the release notes
  modal to load dynamically from the backend registry with categorized tags and domain pills.
- **Frontend (`native/src/App.tsx`, `native/src/components/capture/PTTWidget.tsx`)**: Removed
  the static `Local Mode` header badge, cleaned the `Local Vault` label, and removed obsolete
  placeholder cards.

## [0.7.0] - 2026-08-20

### Voice Note — Universal Dictation History & Configurable Vault Directory Location

Voice Note is now the persistent history of every successful Relay
transcription, stored in the existing local Vault — regardless of which
capture path produced it or whether text injection succeeded. See
`docs/decisions.md` (Decision 38) for the full design.

- **Backend**: Added a `voice_note` note type to the existing Vault
  (`native/src-tauri/src/vault/mod.rs`) — reuses the existing Markdown/
  frontmatter format and `VaultManager::save_note`, no new database. A new
  `commands::save_voice_note` funnel is called from both the global
  dictation hotkey (`hotkeys::stop_dictation_session`) and click-to-talk
  (`commands::process_captured_audio`) right after each already has a
  successful, non-empty transcript, so one recording always produces
  exactly one Voice Note, independent of injection's outcome.
- **Backend**: "Vault Directory Location" is now a real, persisted setting
  (`AppSettings.vault.directory`) instead of decorative UI text. Added
  `get_vault_location`, `choose_vault_folder` (native OS folder picker via
  `tauri-plugin-dialog`, now registered), and `set_vault_location`
  commands. `VaultManager` can repoint its root at runtime with no
  restart, so a freshly chosen folder is usable immediately.
- **Frontend**: The "Voice Capture" sidebar tab is renamed "Voice Note" and
  rebuilt as the dictation history/review surface — a banner, three stat
  cards (Total Voice Notes, Total Words, Notes Today), and a live-updating
  Transcript History list. First visit with no configured location shows
  a setup prompt (native folder picker or "Use Default Relay Vault"); an
  inaccessible location shows a recovery prompt instead of crashing.
  Settings → Vault & LanceDB now shows the real resolved path and can
  change it via the same commands. Removed the redundant "Menu" label
  above the sidebar's navigation items.
- **Preserved**: Global PTT, click-to-talk, the Dictation Pill, Scribble
  Notes, Kanban, and existing settings are all unmodified.

## [0.6.0] - 2026-08-20

### Toggle-to-Talk — Optional Press-Once Dictation Mode

Requested directly as a new capability (not part of the desktop-first
scope reduction in the entries below): holding the dictation hotkey down
for a long recording is tedious, so this adds an opt-in mode where one
press starts recording and a second press stops it.

- **Backend (`native/src-tauri/src/settings/mod.rs`)**: Added
  `HotkeySettings.toggle_to_talk: bool` (default `false` — hold-to-talk
  remains the default for everyone who doesn't opt in).
- **Backend (`native/src-tauri/src/hotkeys/mod.rs`)**: Reworked the
  dictation hotkey's press/release state machine. Added a `key_down` flag
  to `DictationState` so a genuine second press (toggle mode's "stop"
  signal) can be told apart from the OS re-firing "pressed" while a key
  stays physically held — both modes already had to filter out the
  latter; only toggle mode needed the former. Extracted the actual
  stop/transcribe/inject logic (previously inline in
  `on_dictation_released`) into a new `stop_dictation_session` function so
  both "release stops it" (hold-to-talk) and "a second press stops it"
  (toggle-to-talk) call the same path. Added a separate 10-minute
  stuck-session watchdog timeout for toggle mode
  (`MAX_PERSISTENT_RECORDING`) — the existing 60-second one
  (`MAX_DICTATION_HOLD`, sized for hold-to-talk's short-recording
  assumption) would have silently cut off the exact long recordings this
  feature exists to make easier.
- **Frontend (`native/src/components/settings/ProviderSettings.tsx`)**:
  Added a "Toggle-to-Talk" switch in Settings → General, next to the
  existing hotkey recorders. Applies immediately via `save_settings` (no
  hotkey re-registration needed, since only the interpretation of
  press/release changes, not the key combination itself). Reworded the
  "Universal Dictation" hotkey label, since "(hold to talk...)" is no
  longer always true.
- **Frontend (`native/src/components/capture/DictationPill.tsx`)**: The
  floating hint text now reads "Tap to start/stop" instead of "Hold to
  record" when toggle-to-talk is active, so the pill never states the
  wrong interaction model for how the hotkey actually behaves right now.
- **Frontend (`native/src/types/index.ts`)**: Added `toggle_to_talk` to
  the `HotkeySettings` TypeScript interface, mirroring the Rust struct.
- **What was NOT changed**: Click-to-talk (already toggle-based — each
  click flips start/stop, unaffected by this change), STT, text injection,
  the `AudioRecorder` capture primitive, `overlay.rs`, and every decision
  from the desktop-first scope reduction (Decisions 32–36).
- **Verification**: `native/` — `npm run build` (`tsc && vite build`)
  passes clean with zero errors. Rust — `cargo check` passes cleanly
  (2.4s on the warm build cache). The full press/release/repeat/watchdog
  state machine was traced by hand through every case (hold-to-talk
  unchanged; toggle-to-talk's press→press cycle; OS key-repeat in both
  modes; the watchdog firing in both modes) before considering this done
  — see Decision 37 for the reasoning and one accepted, narrow edge case.
- **Not independently tested end-to-end**: same environment limitation as
  every other change in this session — this is a headless Linux container
  with no display server or audio hardware, and Relay targets Windows.
  Compiler-level correctness is verified; live hotkey/microphone behavior
  is not.

## [0.5.0] - 2026-08-20

### Scope-Reduction Set 5 — Single Dictation Pill (Docked/Floating Mode Removed)

The one set where real architectural consolidation was expected. The
Set 0 audit found `DictationPill.tsx` was already the sole canonical pill
implementation — `FloatingPill.tsx` and `PTTWidget.tsx` were two render
*sites* for it (a separate always-on-top window vs. inline in the main
window), not competing implementations, selected by one boolean setting
(`ui.show_floating_pill`, the "Docked vs Floating" product mode this
release removes per the task's own scope table — unlike Kanban/Voice
Chat/Triggers, which are *deferred*, this is a genuine *removal*).

- **Backend (`native/src-tauri/src/settings/mod.rs`)**: Removed
  `UiSettings.show_floating_pill`. `ui.pill_position` (work-area-aware
  anchor edge) is unchanged.
- **Backend (`native/src-tauri/src/commands.rs`)**: Removed the
  `set_pill_visible` command — its only purpose was toggling the
  now-removed setting.
- **Backend (`native/src-tauri/src/lib.rs`)**: `overlay::ensure_pill_window`
  is now always called with `visible: true` at startup — the floating pill
  window is the one, permanent PTT surface. Removed `set_pill_visible` from
  the `invoke_handler!` registration.
- **Backend (`native/src-tauri/src/hotkeys/mod.rs`)**: Removed a
  docked-mode-specific compensation in `on_dictation_pressed` that showed
  the main window and switched it to the Capture tab so a docked pill's
  reaction to the hotkey would be visible. Unnecessary now — the
  always-on-top floating pill is always present and already reacts to the
  same `capture-state-changed` event regardless of the main window's state.
  Caught by a full rebuild, not by inspection alone: this was a second,
  easy-to-miss reference to the removed setting.
- **Frontend (`native/src/components/capture/PTTWidget.tsx`)**: Removed
  the `showFloatingPill` state, its settings-load effect, its
  `pill-visibility-changed` listener, and the conditional that rendered
  `DictationPill` inline as the "docked" alternative. Removed the
  `onProcessComplete` prop entirely — it was only ever needed by the
  now-removed inline render path; the floating pill (which never received
  this prop) already relies solely on the `capture-processed` backend
  event for the main window to learn about completions, so nothing was
  lost. The informational badge is now static text (no more "toggle it
  off in Settings" — there's no such toggle anymore).
- **Frontend (`native/src/App.tsx`)**: `<PTTWidget />` no longer takes a
  prop; `handleProcessComplete` is unchanged (still used directly by the
  `capture-processed` event listener).
- **Frontend (`native/src/components/settings/ProviderSettings.tsx`)**:
  Removed the "Floating Dictation Pill" toggle switch and the stale
  `show_floating_pill` field from `DEFAULT_SETTINGS`.
- **Frontend (`native/src/types/index.ts`)**: Removed `show_floating_pill`
  from the `UiSettings` TypeScript interface, mirroring the Rust struct.
- **What was NOT changed**: `DictationPill.tsx`, `FloatingPill.tsx`,
  `PillSettingsPopover.tsx`, `PillTypes.ts`, and `overlay.rs` — all
  completely untouched. The capture state machine, STT, text injection,
  and `ui.pill_position`/`set_pill_position`/`set_pill_expanded`/
  `set_pill_window_mode` (work-area/monitor/DPI-aware positioning) are
  unmodified. No settings.json migration needed — `AppSettings` has no
  `deny_unknown_fields`, so a stale `show_floating_pill` key in an existing
  user's file is silently ignored on load and simply not written back.
- **Still open, not addressed by this release**: the click-to-talk vs.
  global-hotkey text-injection divergence flagged since Set 0 — raised
  again here for visibility, deliberately left unresolved per explicit
  direction each time it came up.
- **Verification**: `native/` — `npm run build` (`tsc && vite build`)
  passes clean with zero errors (module count unchanged at 1606 — this
  set trimmed code within existing files rather than removing whole
  modules). A first build attempt caught the `ProviderSettings.tsx`
  `DEFAULT_SETTINGS` reference to the removed field as a real `tsc` type
  error, and a repo-wide grep after fixing it confirms zero remaining
  references to `show_floating_pill`, `set_pill_visible`, or
  `pill-visibility-changed` anywhere in `native/`. Rust — `cargo check`
  (with `LIBCLANG_PATH`/`CMAKE` overridden for this Linux container, same
  as Set 4) passes clean in 3.4s with the warm build cache, confirming
  `settings/mod.rs`, `commands.rs`, `lib.rs`, and `hotkeys/mod.rs` all
  compile correctly together.

## [0.4.8] - 2026-08-20

### Scope-Reduction Set 4 — Triggers/MCP Active Surface Removed

- **Frontend (`native/src/App.tsx`)**: Removed the sidebar "Trigger Phrases"
  nav button, the `triggers` branch of the tab switcher and hero header,
  and `triggers` from the `navigate-tab` allowlist. Removed the now-unused
  `TriggerSettings` and `Zap` icon imports.
- **Frontend (`native/src/components/settings/ProviderSettings.tsx`)**:
  Removed a *second*, easy-to-miss entry point to the same component — the
  "Triggers & MCP" sub-section inside the Settings screen's own sub-nav
  (`activeSection === 'triggers'`), separate from the App-level tab.
  Removed the now-unused `TriggerSettings` and `Zap` icon imports.
- **Backend (`native/src-tauri/src/commands.rs`)**: Removed the inline
  trigger-match-and-MCP-dispatch block from `process_captured_audio` — the
  function every click-to-talk ("scribble" mode) capture runs through.
  Before this change, a spoken phrase matching one of the two
  *enabled-by-default* triggers ("schedule meeting", "remind me to")
  silently short-circuited into a canned MCP-stub reply instead of the
  normal cleanup pipeline, on every fresh install — not only for users who
  had configured their own triggers. This ran automatically on the core
  capture path with no UI left to see or control it once the settings
  entries above are gone, so leaving it in place would not have actually
  deferred the feature. Removed the now-unused `use crate::mcp::McpRouter;`
  import and updated a comment that referenced "trigger-matching" as a
  downstream step it no longer is.
- **What was NOT changed**: `native/src-tauri/src/triggers/mod.rs`
  (`TriggerEngine`, including its unit tests), `native/src-tauri/src/mcp/mod.rs`
  (`McpRouter`), the `get_triggers`/`save_triggers` Tauri commands, and
  `native/src/components/settings/TriggerSettings.tsx` itself — all
  unmodified, just no longer invoked or reachable from the UI. PTT,
  click-to-talk's capture/transcription steps, hotkeys, STT, text
  injection, Kanban, Voice Chat, and Scribble Notes are unmodified.
- **Verification**: `native/` — `npm run build` (`tsc && vite build`)
  passes clean with zero errors; bundle module count dropped from 1608 to
  1606 (`TriggerSettings.tsx` and one component it exclusively imports no
  longer bundled). Rust — this container initially couldn't build past
  `gdk-sys` (Set 1) at all; installing the missing Tauri Linux
  prerequisites (`libgtk-3-dev`, `libwebkit2gtk-4.1-dev`,
  `libappindicator3-dev`, `librsvg2-dev`, `libasound2-dev`) plus overriding
  `LIBCLANG_PATH`/`CMAKE` for this invocation only (the repo's
  `.cargo/config.toml` hardcodes Windows-only paths for both, left
  untouched since it's an intentional, platform-specific config file, not
  something this change needed to touch) let `cargo check` reach and fully
  compile `relay-native-backend` (Relay's own crate, including the edited
  `commands.rs`) for the first time this session — `Finished \`dev\`
  profile [unoptimized + debuginfo] target(s)`, zero errors.

## [0.4.7] - 2026-08-20

### Scope-Reduction Set 3 — Voice Chat & TTS Active Surface Removed

- **Frontend (`native/src/App.tsx`)**:
  - Removed the sidebar "Voice Chat" nav button, the `chat` branch of the
    tab switcher and hero header, `chat` from the `navigate-tab` event's
    allowed payload list, and the `{activeTab === 'chat' && <ChatPanel />}`
    render line. `activeTab` can no longer become `'chat'`.
  - Removed the now-unused `ChatPanel` import and `Bot` icon import.
- **Frontend (`native/src/components/settings/ProviderSettings.tsx`)**:
  - Removed the "Local Text-to-Speech (Piper) — optional" settings block
    from the General section — its own caption stated its sole purpose was
    to "skip 'speak back' in voice chat," so it is Voice Chat's settings
    surface, not an independent one. Removed the now-unused `Volume2` icon
    import. `DEFAULT_SETTINGS.tts` and the `settings.tts` round-trip through
    `get_settings`/`save_settings` are unchanged — the fields simply have no
    editable UI anymore, exactly like Kanban's backend command in Set 2.
- **What was NOT changed**: `native/src/components/chat/ChatPanel.tsx` —
  untouched. `native/src-tauri/src/pipeline/chat.rs` (`process_chat`),
  `native/src-tauri/src/tts/mod.rs` (`TtsEngine`), and
  `native/src-tauri/src/providers/mod.rs` (the shared LLM client also used
  by the in-scope Kanban/meeting and Scribble pipelines) — all untouched;
  `git diff --stat native/src-tauri/` is empty for this release. The
  `AppSettings`/`ProcessedPipelineResult` TypeScript types in
  `native/src/types/index.ts` are unchanged. No settings schema changed.
  Kanban, PTT, click-to-talk, hotkeys, STT, and text injection unmodified.
- **Verification**: `native/` — `npm run build` (`tsc && vite build`)
  passes clean with zero errors; the production bundle now transforms 1608
  modules (down from 1609 after Set 2), confirming `ChatPanel.tsx` is no
  longer bundled. Repo-wide grep confirms the only remaining
  `'chat'`/`piper_*` references in `native/src` are inside `ChatPanel.tsx`
  itself (untouched) and the preserved `AppSettings`/`DEFAULT_SETTINGS`
  type shape (needed for settings round-tripping) — no orphaned
  references. `git status` confirms only the two files above changed.

## [0.4.6] - 2026-08-20

### Scope-Reduction Set 2 — Kanban Active Navigation Removed

- **Frontend (`native/src/App.tsx`)**:
  - Removed the sidebar "Kanban Board" nav button, the `kanban` branch of
    the tab switcher and hero header, and `kanban` from the `navigate-tab`
    event's allowed payload list — `activeTab` can no longer become
    `'kanban'`.
  - Removed the unconditional `get_kanban_cards` fetch that previously ran
    on every app launch (`useEffect` with an empty dependency array) and
    the `fetchKanbanCards` call inside `handleProcessComplete` — both fired
    regardless of whether the user ever opened the Kanban tab. This was the
    one "required at startup" concern flagged by the Set 0 audit.
  - Removed now-unused imports (`KanbanBoard`, `KanbanCard`, `invoke`, the
    `Kanban` icon) and the dead `cards` state. Cleaned up a stale comment
    that referenced "refreshes the board" after the fetch call it described
    was removed.
- **What was NOT changed**: `native/src/components/kanban/KanbanBoard.tsx`
  — untouched, still on disk. The `KanbanCard` type in
  `native/src/types/index.ts` — untouched (still used by `KanbanBoard.tsx`).
  The backend `get_kanban_cards` Tauri command and every `VaultManager`
  Kanban read/write method in `native/src-tauri` — untouched; `git diff
  --stat native/src-tauri/` is empty for this release. No settings changed
  (Kanban had none). Scribble Notes, Voice Chat, PTT, click-to-talk,
  hotkeys, STT, and text injection are unmodified.
- **Verification**: `native/` — `npm run build` (`tsc && vite build`)
  passes clean with zero errors; the production bundle now transforms 1609
  modules (down from 1610), confirming `KanbanBoard.tsx` is no longer
  bundled into the app. Repo-wide grep confirms zero remaining references
  to `KanbanBoard`/`get_kanban_cards` in `native/src` outside
  `KanbanBoard.tsx` and `types/index.ts` themselves. `git status` confirms
  only `native/src/App.tsx` changed under `native/`. No test suite covers
  this path (only Rust-side `#[cfg(test)]` unit tests exist, in
  `vault/mod.rs`, `capture/mod.rs`, `triggers/mod.rs` — none reference
  Kanban or App.tsx, and none were touched).

## [0.4.5] - 2026-08-20

### Scope-Reduction Set 0 (Audit) & Set 1 — Web Surface Marked Deferred

Relay is being reduced to a stable, focused desktop-first universal dictation
app (global PTT + click-to-talk + local STT + text injection through one
Dictation Pill). This is a documentation-only release: no application code
changed. Kanban, Voice Chat, TTS, Triggers/MCP, and the Dictation Pill
consolidation are explicitly **not** part of this release — each remains
gated behind its own future approval (see `docs/decisions.md` Decision 32).

- **Repository audit (Set 0, no changes)**: Read-only audit across Web,
  Kanban, Voice Chat, TTS, Triggers/MCP, the three pill components
  (`DictationPill.tsx`/`FloatingPill.tsx`/`PTTWidget.tsx`), Settings, and the
  core capture/hotkey/STT/injection path. Notable findings recorded for
  future sets rather than acted on now: Voice Chat is fully implemented and
  active (contradicting an assumption that it might not exist); the
  standalone "Dictation Indicator" window was already removed in a prior
  commit (Decision 30/PTT-013) and no longer exists in code; click-to-talk
  and the global PTT hotkey share the same backend capture session but
  diverge after transcription (only the hotkey path calls real OS text
  injection today; click-to-talk runs the scribble LLM-cleanup/vault-write
  pipeline instead); and the Triggers/MCP phrase-match check is inlined
  inside the same handler click-to-talk uses, a real (if small) coupling
  distinct from the trivially-removable Triggers settings tab.
- **Documentation (`docs/`)**:
  - Added Decision 32 to `docs/decisions.md`, recording the desktop-first
    scope reduction and deferring Web for the current phase — explicitly
    superseding Decision 3's "dual-surface" framing for this phase only,
    without altering Decision 3's historical text.
  - `docs/product.md`: moved the "Dual Surface (Native Desktop + Web
    Dashboard)" differentiator and the Next.js/Supabase MVP bullet out of
    active MVP scope into a new "Deferred for Current Phase" section.
  - `docs/requirements.md`: annotated FR-5.2 (Web Dashboard) as deferred for
    the current phase, requirement text preserved.
  - `docs/architecture.md`: annotated the Web Surface box in the
    three-surface diagram as deferred, and corrected the "Hybrid Cloud Mode"
    description — a repository audit found no Supabase code anywhere in
    `native/src-tauri` (zero matches, no dependency) and `web/`'s own
    Supabase client is a mocked stand-in returning hardcoded data, so this
    was previously describing an unbuilt, aspirational design as if it were
    implemented.
- **What was NOT changed**: No files under `web/`. No Rust or TypeScript
  source under `native/`. No settings, commands, build configuration, or
  navigation. No modification made to any existing working implementation —
  `web/` was already fully decoupled from the desktop build (no shared
  workspace config, no imports either direction) before this release.
- **Verification**: `native/` frontend — `npm install && npm run build`
  (`tsc && vite build`) completed cleanly with zero errors (one pre-existing,
  unrelated warning about a mixed static/dynamic import of
  `@tauri-apps/api/event.js`, present before this change). Rust backend —
  `cargo check` in this Linux container fails during dependency compilation
  at `gdk-sys v0.18.2`'s build script (`pkg-config` can't find `gdk-3.0` —
  the GTK/GDK system libraries Tauri's Linux windowing backend needs, e.g.
  `libgtk-3-dev`, which this container doesn't have installed), before any
  of Relay's own crates or Relay-authored Rust code are even reached. Relay
  targets Windows and this is a pre-existing container/environment gap, not
  a regression — this release touched zero Rust code, and the failure
  occurs entirely within a third-party dependency's native build step,
  upstream of anything this repository controls.

## [0.4.4] - 2026-08-20

### Real Speech Detection, Rolling Waveform & Dictation Lifecycle Hardening

A prior attempt at this fix (gating transcription on a `had_audio` flag set
by a fixed RMS threshold) landed real correctness improvements but was
independently found to be incomplete: the threshold couldn't tell a
sustained-but-silent noisy room apart from speech, the waveform still used
one scalar to scale a fixed decorative bar shape (never truly flat at
silence), and the click handler still declared "listening"/"processing"
before the native recorder confirmed either. This release addresses all
three, plus the docked-pill hotkey-visibility gap that fell out of the
same review.

- **Speech detection (`capture/mod.rs`)**:
  - Replaced the fixed `AUDIO_DETECTED_THRESHOLD` gate with a per-session
    calibration: the first 300ms of a recording measures the ambient noise
    floor (fan/room/mic-AGC noise), and only energy sustained for 200ms+
    *above that measured floor by a margin* counts towards `had_audio`.
    Fan noise, keyboard clatter, and Windows mic-enhancement processing
    sitting continuously above a static threshold no longer falsely
    triggers transcription.
  - Added unit tests (`capture::tests`) covering true silence, sustained
    ambient noise at a fixed level, sustained real speech above the
    calibrated floor, and a brief sub-threshold-duration spike — the exact
    regression scenario a static threshold couldn't distinguish.
- **Real waveform (`DictationPill.tsx`)**:
  - Replaced the fixed 15-value decorative shape scaled by one shared
    `audioLevel` scalar with a rolling per-bar history: each bar now
    renders its own actual recent audio-level sample. Silence now
    collapses every bar to its hairline minimum instead of a predetermined
    non-zero pattern.
- **Recording lifecycle (`DictationPill.tsx`)**:
  - Removed the remaining optimistic local state transitions: clicking to
    start no longer claims `listening` before `start_capture` resolves.
    The pill now exclusively reflects state the native recorder has
    confirmed via `capture-state-changed`, removing the last
    two-sources-of-truth race between the UI and the backend.
- **Docked hotkey visibility (`hotkeys/mod.rs`)**:
  - Pressing the dictation hotkey while docked (floating pill off) now
    shows the main window (without focusing it, so the actual dictation
    target keeps OS focus for text injection) and switches to the Voice
    Capture tab via the existing `navigate-tab` event, so a hotkey-triggered
    recording is actually visible instead of updating a pill that's hidden
    behind another tab or window.
  - `try_register_hotkeys` now registers the show/hide and dictation
    hotkeys independently; previously a failure on the first silently
    skipped ever attempting the second, which could leave the dictation
    hotkey completely unregistered with no visible error. A
    `hotkey-status-changed` event now carries the real per-hotkey
    registration outcome to the UI.

## [0.4.3] - 2026-08-20

### Dev Environment Setup & Workspace Install Scripts

- **Improvements**:
  - (`root`) Added `install:all` script to root `package.json` to install dependencies across both `native/` and `web/` workspaces in a single command.
  - (`native/`, `web/`) Configured and verified development dependencies and build pipelines for both native (Tauri + React + Vite) and web (Next.js + Turbopack) environments.

## [0.4.2] - 2026-08-20

### whisper-rs 0.16 Upgrade, Compilation Fixes & Open Settings Command

- **Fixes**:
  - (`native/src-tauri`) Upgraded `whisper-rs` from 0.14 to 0.16 and re-enabled the `whisper-local` feature as the default in `Cargo.toml` — local STT was silently disabled because the previous version's bindgen build failed on Windows without LLVM/libclang.
  - (`native/src-tauri`) Added `LIBCLANG_PATH` to `.cargo/config.toml` so whisper-rs-sys bindgen finds the LLVM installation on Windows.
  - (`native/src-tauri`) Imported `tauri::Manager` trait in `commands.rs` to fix `get_webview_window` not found on `AppHandle`.
  - (`native/src-tauri`) Updated `stt.rs` segment extraction to use the whisper-rs 0.16 iterator API (`state.as_iter()` + `segment.to_str_lossy()`) replacing the removed `full_n_segments`/`full_get_segment_text` methods.
- **Features**:
  - (`native/src-tauri`) Added `open_settings_window` Tauri command that surfaces the main window, focuses it, and emits a `navigate-tab` event to switch to the settings tab.
  - (`native/src-tauri`) Registered `open_settings_window` in the Tauri command handler list in `lib.rs`.
  - (`native/src`) Added `navigate-tab` event listener in `App.tsx` so the main window responds to tab-switch requests from the backend or overlay.
  - (`native/src`) Updated `DictationPill.tsx` error/warning banners to invoke `open_settings_window` instead of opening the local popover — directs users to the full settings page in the main window.
  - (`native/src`) Added an "Open All Settings in App" link at the bottom of `PillSettingsPopover.tsx` for quick access to the main settings panel.

## [0.4.1] - 2026-08-19

### Unconditional Automatic GGML Whisper Model Downloader

- **STT Model Auto-Fetch (`stt.rs`, `commands.rs`)**:
  - Removed feature gating from `ensure_default_model` in `stt.rs`.
  - Automatically fetches HuggingFace's `ggml-tiny.en.bin` model directly into `%APPDATA%\Relay\models\` on launch whenever a model path is unconfigured or missing on disk.
  - Automatically persists the downloaded model path into `settings.json`, transitioning the dictation pill seamlessly to `Click to dictate` with zero manual configuration.

## [0.4.0] - 2026-08-19

### STT Whisper Model Error Resolution & Interactive Configuration Handler

- **STT Model Validation (`commands.rs`, `stt.rs`)**:
  - Updated `ensure_stt_model_ready` to verify file existence on disk before declaring status ready.
  - Simplified STT missing model error message to a clean, actionable instruction (`Set Whisper model path in Provider Settings.`).
- **Interactive Error Action (`DictationPill.tsx`)**:
  - Formatted error banner text with truncation (`max-w-[260px]`) to prevent pill overflow.
  - Added click-to-configure interaction: clicking the error banner automatically opens the settings popover for one-click configuration.

## [0.3.9] - 2026-08-19

### Process Label Removal & Application Dark Theme Syncing

- **Process Label Removal (`DictationPill.tsx`)**:
  - Removed process indicator label (`● SNIPPING TOOL`) from the left side of the expanded dictation pill.
- **Application Dark Theme Color Synchronization (`DictationPill.tsx`, `PillSettingsPopover.tsx`)**:
  - Matched dark theme colors to Relay's exact neutral dark theme tokens (`#171717` dark card background, `#262626` dark border, `#fafafa` text), eliminating navy/slate color mismatch between the overlay pill and application dashboard.

## [0.3.8] - 2026-08-19

### Rounded-lg Component Geometry & Simultaneous Light/Dark/System Theme Syncing

- **Simultaneous Theme Synchronization (`ThemeToggle.tsx`, `DictationPill.tsx`)**:
  - Wired real-time theme syncing between the main dashboard window and the floating overlay dictation window.
  - Listens to `relay-theme-changed` events, `localStorage` theme state, and `prefers-color-scheme` media queries, toggling `.dark` class on root HTML elements simultaneously across all surfaces.
- **Rounded-lg Component Styling (`DictationPill.tsx`, `PillSettingsPopover.tsx`)**:
  - Updated pill body container, sparkle button (`✦`), settings chevron (`⌄`), audio waveform bars, hit zones, keyboard hint bar (`Hold to record`), and popover dropdown options to use `rounded-lg` / `rounded-xl` geometry.

## [0.3.7] - 2026-08-19

### Top Hint Clipping Fix & Relay Light Theme Palette Integration

- **Rust Overlay Window Expansion (`overlay.rs`)**:
  - Increased `EXPANDED_SIZE` height from `100.0` to `150.0` and `POPOVER_SIZE` height to `420.0` to eliminate horizontal top-clipping on the floating `Hold to record [Ctrl] [Space]` hint bar.
- **Relay Native Light Theme System (`DictationPill.tsx`, `PillSettingsPopover.tsx`)**:
  - Re-themed push-to-talk pill and settings popover using Relay's crisp light mode design system: pure white card background (`#ffffff`), Slate-900 typography (`#0f172a`), Relay primary blue (`#2563eb`) accents/waveforms/toggles, and subtle Slate-200 borders (`#e2e8f0`).

## [0.3.6] - 2026-08-19

### Murmur Push-to-Talk Pill Design & Edge-Flush Placement

- **Murmur Visual System Replication (`DictationPill.tsx`, `PillSettingsPopover.tsx`, `overlay.rs`)**:
  - Replicated exact Murmur paper gradients (`#faf8f3` -> `#efeae0`), box shadows, 13 idle dots, 15 terracotta waveform bars, toast notifications, and keyboard hint bar.
  - Made handle/notch 25% wider (`96px` width, `6px` height) with `border-radius: 999px 999px 0 0` (rounded-t-lg top part).
  - Fixed handle overlap bug by hiding handle (`opacity: 0`) whenever pill is expanded, recording, processing, or showing toast.
  - Updated Rust `overlay.rs` positioning so resting notch window anchors flush against the top edge of the taskbar / screen bottom without floating gap.
  - Added sub-page navigation in settings popover for Cleanup Style (`Faithful`/`Polished`/`Clean`/`Concise`) and Language (`Auto-detect`/`English (US)`/`Hinglish`/`Hindi`/`Español`).

## [0.3.5] - 2026-08-19

### Oscar-Inspired Push-to-Talk Pill Redesign & Interaction Refinement

- **Oscar Visual & Interaction Redesign (`DictationPill.tsx`, `PillSettingsPopover.tsx`, `overlay.rs`)**:
  - Removed middle-logo component overlap bug on expanded pill.
  - Replaced floating resting dot with a slim, edge-attached horizontal notch (`w-16 h-2`) when idle.
  - Added floating hotkey hint bar (`Hold to record [Ctrl] [Space]`) floating above the main pill on hover/activation.
  - Removed "RELAY" brand text in favor of minimal process indicators (`● SNIPPING TOOL`).
  - Added Oscar-style settings dropdown supporting Auto-paste, Text transform, Cleanup style (`Faithful`/`Clean`/`Professional`/`Concise`), Prompt mode (`Rewrite speech into a prompt`), and Language selection.
  - Added repository inspection docs [`docs/inspect/push-to-talk-pill.md`](file:///d:/Projects/Relay/docs/inspect/push-to-talk-pill.md) and decision log [`docs/decisions/push-to-talk-pill.md`](file:///d:/Projects/Relay/docs/decisions/push-to-talk-pill.md).

## [0.3.4] - 2026-08-19

### Native Build Fix for npm run dev:native

- **Windows Build Fix (`Cargo.toml`)**:
  - Gated optional `whisper-local` feature from `default = []` in `Cargo.toml` so `npm run dev:native` and `tauri dev` build cleanly on Windows environments without external `cmake` C++ build tools installed.

## [0.3.3] - 2026-08-19

### Multi-Monitor Active Positioning & Floating Pill Consolidation

- **Unified Floating Overlay Surface (`overlay.rs`, `hotkeys/mod.rs`)**:
  - Consolidated legacy `dictation-indicator` window into the unified `dictation-pill` overlay.
  - Implemented active-window monitor auto-detection so the floating pill appears on whichever display contains the user's active application target.
  - Hardened focus preservation and session locks across global hotkeys and overlay UI.

## [0.3.2] - 2026-08-19

### Push-to-Talk Floating Pill Upgrade

- **Push-to-Talk Overlay Redesign (`DictationPill.tsx`, `commands.rs`)**:
  - Bound overlay states directly to backend capture machine (`IDLE` → `LISTENING` → `TRANSCRIBING` → `SUCCESS` / `ERROR`).
  - Added real-time RMS microphone audio level calculations (`compute_rms_f32`) emitted at ~25Hz to drive overlay waveform animation.
  - Preserved zero-focus-theft properties (`focused(false)`) on floating overlay window for reliable OS text injection (`enigo`).
  - Recorded architectural decisions **PTT-001** through **PTT-012** in [`docs/decisions.md`](file:///d:/Projects/Relay/docs/decisions.md).

## [0.3.1] - 2026-08-19

### Model Management, Hotkey Recorder & Floating Overlay

- **Local Ollama & Model Manager (`ollama_manager.rs`, `ProviderSettings.tsx`)**:
  - Added local Ollama daemon detection, model status checking, and one-click model pulling (`llama3.2:latest`, `qwen2.5:latest`).
  - Added local Whisper GGML model selection and status monitoring (`ggml-tiny.en.bin`, `ggml-base.en.bin`).

- **Global Hotkey Recorder (`hotkeys/mod.rs`, `HotkeyRecorder.tsx`)**:
  - Added interactive Hotkey Recorder UI component allowing users to set custom key combinations for global dictation actions.
  - Bound global overlay toggle (`Ctrl+Shift+Space`) and push-to-talk dictation (`Ctrl+Space`) to OS text focus injection (`enigo`).

- **Always-on-Top Floating Overlay Window (`overlay.rs`, `FloatingPill.tsx`)**:
  - Created non-focus-stealing transparent native desktop overlay window for instant dictation state visualization.

- **In-App Categorized Release Notes (`ChangelogModal.tsx`, `changelog-dialog.tsx`)**:
  - Added 80% width modal layout with dual category tags (`Features`, `Fixes`, `Improvements`) and domain tags (`UI`, `LLM`, `Speech`, `Dictation`, `Kanban`, `Vault`, `Settings`, `Build`).

## [0.3.0] - 2026-08-19

### Universal Dictation, Global Hotkeys & Voice Chat (minor: new modules, major features)

- **Native Backend (`native/src-tauri/`)**:
  - **Features**: Added `hotkeys/` module — registers a global show/hide hotkey (`Ctrl+Shift+Space`) and a push-to-talk universal dictation hotkey (`Ctrl+Space`, both configurable) via `tauri-plugin-global-shortcut`; dictation types the transcript into whichever field has OS focus via the new `hotkeys::injection` (`enigo`) submodule, with a small always-on-top, non-focus-stealing "listening" indicator window.
  - **Features**: Added `pipeline::chat::process_chat` — in-app voice chat grounded in vault notes, with source attribution and optional spoken answers.
  - **Features**: Added `tts/` module — optional local text-to-speech via a user-configured Piper binary + voice model.
  - **Features**: Added `settings/` module — persists provider/STT/TTS/hotkey configuration to `.relay/config/settings.json`; wired new `get_settings`/`save_settings` commands.
  - **Improvements**: `capture/` now performs real microphone recording via `cpal` on a dedicated thread (resampled to 16kHz mono) instead of writing an empty WAV placeholder.
  - **Improvements**: Added `capture::stt::SttEngine` — real local transcription via `whisper-rs` (whisper.cpp), replacing the previous hardcoded fake transcript string. Model path is configurable; a clear error surfaces if unconfigured rather than silently faking output.
  - **Improvements**: `vault::VaultManager` gained `list_notes`/`search_notes` (keyword-ranked retrieval) as real, if interim, grounding for voice chat ahead of the LanceDB embedding pipeline `docs/decisions.md` already commits to.
  - **Improvements**: `stop_capture` now loads the persisted LLM provider config instead of always using hardcoded defaults, so Provider Settings actually take effect.
  - **Fixes**: Fixed `ProviderType` JSON serialization (`CloudOpenAI` etc. now serialize as `cloud_openai` etc., matching the frontend contract instead of silently mismatching on save/load).
  - **Fixes**: Fixed app icons (`icons/*.png`) not being RGBA, which made the Tauri build fail outright (`generate_context!` panic).
- **Native Frontend (`native/src/`)**:
  - **Features**: Added a "Voice Chat" tab (`components/chat/ChatPanel.tsx`) — record a question, see the grounded answer with sources, hear it spoken back if TTS is configured.
  - **Features**: Added `components/dictation/DictationIndicator.tsx`, rendered via a `#/dictation-indicator` hash route in the same bundle for the new indicator window.
  - **Improvements**: `ProviderSettings.tsx` now actually loads/saves settings via `get_settings`/`save_settings` (previously local-only UI state that did nothing), and gained STT model path, TTS binary/voice path, and hotkey configuration sections.
  - **Improvements**: Extended `ProcessedPipelineResult`/`AppSettings` types to match the backend.
- **Docs (`docs/`)**: Recorded Decisions 13–16 (universal dictation & hotkeys, real local STT, in-app voice chat/RAG-lite, optional local TTS); updated product/requirements/data-model/api/architecture/user-flows docs to match; added `docs/roadmap.md` tracking remaining competitive-research gaps (LanceDB vector RAG, real MCP client wiring, speaker diarization, multi-user).
- **Repo hygiene**: Fixed an unresolved git merge-conflict left in `README.md` from the initial boilerplate merge.
- **Note**: Merged on top of the `0.2.0` visual identity pass below; `ProviderSettings.tsx` reconciles both — the new sub-navigated Settings shell now carries this round's STT/TTS/hotkey sections instead of the flatter layout originally shipped with those fields, and `providers::LLMClient` keeps this round's `ProviderType` serde fix alongside `0.2.0`'s offline heuristic fallback.

## [0.2.0] - 2026-08-19

### Relay Visual Identity Pass ("Monochrome & Electric Blue")

- **Brand Tokens (`design-system.md`)**:
  - Repointed CSS variables across `:root` and `.dark` in [`native/src/index.css`](file:///d:/Projects/Relay/native/src/index.css), [`native/tailwind.config.cjs`](file:///d:/Projects/Relay/native/tailwind.config.cjs), and [`web/src/app/globals.css`](file:///d:/Projects/Relay/web/src/app/globals.css) to the Monochrome & Electric Blue palette (`#2563EB` light / `#60A5FA` dark).
  - Introduced 3-way semantic colors (`--success`, `--warning`, `--destructive`) and `--border-strong` tokens across light and dark modes.

- **Relay Logo (`RelayLogo`)**:
  - Built reusable SVG logo mark with asymmetric two-tone "E" in [`RelayLogo.tsx`](file:///d:/Projects/Relay/native/src/components/common/RelayLogo.tsx) (native) and [`relay-logo.tsx`](file:///d:/Projects/Relay/web/src/components/relay-logo.tsx) (web).
  - Integrated mark into native sidebar header, web sidebar ([`app-sidebar.tsx`](file:///d:/Projects/Relay/web/src/components/app-sidebar.tsx)), web login card ([`login-form.tsx`](file:///d:/Projects/Relay/web/src/components/login-form.tsx)), and web favicon ([`icon.tsx`](file:///d:/Projects/Relay/web/src/app/icon.tsx)).

- **Floating Dictation Pill (`DictationPill.tsx`)**:
  - Rebuilt push-to-talk experience as a floating dictation pill overlay following the Murmur interaction model with ~180ms hover-hold handle, state machine (`rest → ready → expanded → recording → processing → inserted/error → rest`), audio waveform keyframes, rotating mono processing captions, mode switch button (**Meeting → Kanban** vs **Voice Scribble**), and engine settings popover.
  - Added local heuristic fallback to Rust LLMClient ([`providers/mod.rs`](file:///d:/Projects/Relay/native/src-tauri/src/providers/mod.rs)) so dictation works reliably even when local Ollama is offline.

- **App Shell, Settings & Content Screens**:
  - Standardized top-level Hero header pattern across native and web views (`Today, Nitin captured.`, `How Relay behaves.`, etc.).
  - Restructured native [`ProviderSettings.tsx`](file:///d:/Projects/Relay/native/src/components/settings/ProviderSettings.tsx) and created web [`settings/page.tsx`](file:///d:/Projects/Relay/web/src/app/(dashboard)/settings/page.tsx) with domain sub-navs and Data & Privacy controls.
  - Restructured native [`ScribbleViewer.tsx`](file:///d:/Projects/Relay/native/src/components/scribble/ScribbleViewer.tsx) and created web [`notes/page.tsx`](file:///d:/Projects/Relay/web/src/app/(dashboard)/notes/page.tsx) with master list + detail pane, action toolbar pill buttons, and local-vault reassurance line.
  - Unified native and web Kanban boards with 3-way semantic priority badges.

## [0.1.2] - 2026-08-19

### Complete UI Design Pass & Theme System Refactoring

- **Design System (`design-system.md`)**:
  - Established Relay's signature **Calm Emerald-Teal & Slate Focus Palette** (`hsl(173, 70%, 38%)` primary teal, obsidian slate dark mode, clean soft slate light mode).
  - Defined CSS custom properties (`--primary`, `--background`, `--card`, `--border`, `--muted`, `--accent`, `--ring`) in both [`native/src/index.css`](file:///d:/Projects/Relay/native/src/index.css) and [`web/src/app/globals.css`](file:///d:/Projects/Relay/web/src/app/globals.css).
  - Replaced all hardcoded hex and ad-hoc Tailwind colors with theme token classes (`bg-primary`, `bg-card`, `bg-muted`, `border-border`, `text-foreground`, `text-muted-foreground`).

- **Native Capture Widget (`PTTWidget.tsx`)**:
  - Rebuilt mic push-to-talk button, mode switcher (Meeting vs Scribble), and recording state.
  - Implemented dynamic live-audio level meter visualizer (`animate-audio-bar-*`) during speech recording.
  - Added error fallback banner with retry affordance and WCAG AA contrast compliance.

- **Unified Kanban Board (`KanbanBoard.tsx` & `(dashboard)/page.tsx`)**:
  - Unified desktop native and web dashboard Kanban boards into one cohesive visual design language.
  - Added loading skeletons ([`native/src/components/ui/skeleton.tsx`](file:///d:/Projects/Relay/native/src/components/ui/skeleton.tsx)), responsive column layout grids, priority badges, and empty-state placeholders.

- **Vault Notes, Settings & Auth**:
  - Refactored [`ScribbleViewer`](file:///d:/Projects/Relay/native/src/components/scribble/ScribbleViewer.tsx), [`ProviderSettings`](file:///d:/Projects/Relay/native/src/components/settings/ProviderSettings.tsx), and [`TriggerSettings`](file:///d:/Projects/Relay/native/src/components/settings/TriggerSettings.tsx) with design tokens and accessibility attributes (`aria-label`, `aria-live`, explicit `htmlFor` bindings).
  - Refactored Web Dashboard [`LoginPage`](file:///d:/Projects/Relay/web/src/app/login/page.tsx), [`LoginForm`](file:///d:/Projects/Relay/web/src/components/login-form.tsx), and [`AppSidebar`](file:///d:/Projects/Relay/web/src/components/app-sidebar.tsx) with Relay branding.

## [0.1.1] - 2026-08-19

### Improvements & UI Refactoring

- **Native Frontend (`native/`)**:
  - Installed Radix UI primitives, `clsx`, `tailwind-merge`, and `class-variance-authority`.
  - Created `native/src/lib/utils.ts` `cn` helper function and shadcn primitives (`Button`, `Card`, `Badge`, `Input`).

## [0.1.0] - 2026-08-19

### Initial Release — Multi-Surface Architecture & Core Pipeline

- Initial scaffold of living specifications (`docs/`), native desktop app (`native/`), and hybrid Next.js web dashboard (`web/`).
