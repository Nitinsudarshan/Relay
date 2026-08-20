# Relay — Architectural & Product Decision Log

This log records material architectural, technical, and product decisions for Relay in the standard format specified by the project rules.

---

### Decision 1: Build Path & Relationship to Mnemos
- **Context**: Need to determine codebase origin and relationship to prior prototypes or references.
- **Decision made**: Build Relay completely from scratch in this brand-new repository. No code copied or forked from Mnemos or Meetily.
- **Reason**: Relay's three-surface architecture (Rust backend + Tauri desktop + Next.js web client) differs fundamental from prior prototypes. Starting clean avoids carrying over technical debt or mismatched paradigms.
- **Alternatives considered**: Forking Mnemos or adapting Meetily.
- **Impact**: Full control over component architecture, type definitions, and backend runtime.

---

### Decision 2: Technology Stack
- **Context**: Choosing backend runtime and native shell for Windows desktop capture and local execution.
- **Decision made**: Rust backend (`native/src-tauri/`) + Tauri 2.0 / React frontend (`native/src/`) for desktop, plus Next.js + Shadcn + Supabase for the web client (`web/`).
- **Reason**: Rust delivers high efficiency, native Windows audio/WASAPI capture capabilities, fast local inference orchestration, low footprint, and security.
- **Alternatives considered**: Python backend (PyInstaller / FastAPI), n8n workflow engine, Electron.
- **Impact**: Accepted learning-curve risk for Rust; zero Electron bloat on Windows.

---

### Decision 3: Hybrid Deployment Includes Web Surface
- **Context**: Primary desktop app vs remote/cloud access.
- **Decision made**: Dual-surface model: Windows native app for primary local capture & processing; web client for hybrid/cloud mode.
- **Reason**: Allows access to notes, Kanban board, and structured outputs from any browser when away from the desktop machine.
- **Alternatives considered**: Desktop-only application; tunnel access directly into Windows desktop.
- **Impact**: Requires shared data representations and synchronization via Supabase in hybrid mode.

---

### Decision 4: Cost Ceiling is a Hard Constraint
- **Context**: Monetization vs cost-to-run for personal/builder usage.
- **Decision made**: Every cloud-optional feature must function fully at $0 recurring cost using local STT (Whisper/Parakeet), Ollama, and local files.
- **Reason**: The user requires a zero-cost local baseline. Paid cloud APIs (OpenAI, Gemini, Claude) are optional toggle overlays.
- **Alternatives considered**: Cloud-first or cloud-only API design.
- **Impact**: Graceful degradation to local-only mode when offline or without API keys.

---

### Decision 5: No Meeting-Bot Architecture
- **Context**: Audio capture methodology for meetings.
- **Decision made**: Use push-to-talk / on-screen capture affordances only; do NOT build meeting bots (Zoom/Teams/Meet bot joiners).
- **Reason**: Structural legal/platform risk (e.g. BIPA lawsuits against bot services, platform restrictions on third-party bots). Local PTT audio capture is legally safe and platform-agnostic.
- **Alternatives considered**: Virtual audio cable recording or headless bot joiners.
- **Impact**: Zero meeting-bot infrastructure required; user triggers capture via hotkey or floating widget.

---

### Decision 6: Retrieval Architecture
- **Context**: Context retrieval over stored vault notes and historical transcripts.
- **Decision made**: Use embedded vector search (LanceDB) over markdown notes for MVP. Graph-based retrieval (GraphRAG) is explicitly deferred post-MVP.
- **Reason**: Vector RAG is fast, lightweight, runs embedded in Rust without separate server processes, and provides excellent results for <10,000 documents.
- **Alternatives considered**: Full Knowledge Graph / GraphRAG, plain keyword search.
- **Impact**: Simple schema with LanceDB tables storing embeddings alongside markdown note file paths.

---

### Decision 7: Kanban Delivery Scope for MVP
- **Context**: Presenting actionable items extracted from meetings.
- **Decision made**: List-to-board rendering for MVP (parsing structured lists of tasks into Kanban columns: To Do, In Progress, Done). Drag-and-drop persistence deferred post-MVP.
- **Reason**: Validates the meeting-to-task parsing pipeline first without getting bogged down in complex drag-and-drop state synchronization across files.
- **Alternatives considered**: Custom drag-and-drop board builder.
- **Impact**: Focuses engineering effort on LLM extraction reliability.

---

### Decision 8: MCP Integrations
- **Context**: External system integrations for Calendar, Notion, and Google Drive.
- **Decision made**: Reuse official/community MCP servers as-is (`nspady/google-calendar-mcp`, `makenotion/notion-mcp-server`, `isaacphi/mcp-gdrive`).
- **Reason**: No custom integration code needed for these 3 services; standard MCP client wiring in Rust handles tool execution.
- **Alternatives considered**: Building direct REST client wrappers for Google / Notion APIs.
- **Impact**: Standardization on Model Context Protocol across all trigger actions.

---

### Decision 9: No Rust-vs-Python Benchmarking Spike
- **Context**: Choice of language for audio and LLM pipeline orchestration.
- **Decision made**: Proceed directly with Rust (`native/src-tauri`). Spike comparing Python vs Rust is cancelled.
- **Reason**: Decision 2 committed to Rust for native desktop integration and memory efficiency.
- **Alternatives considered**: Python backend prototype.
- **Impact**: All backend code written in idiomatic Rust.

---

### Decision 10: Trigger Phrases are User-Customizable
- **Context**: Voice commands mapping to system actions.
- **Decision made**: Build a fully configurable trigger-phrase engine where users define custom phrase -> action mappings in a settings surface.
- **Reason**: Fixed trigger lists restrict utility. Users need tailored phrases (e.g. "Schedule sync", "Remind me to submit report", "Save note to Drive").
- **Alternatives considered**: Hardcoded list of voice command keywords.
- **Impact**: Intent classifier must match against dynamic user-configured lists and parameters.

---

### Decision 11: Target Build Environment
- **Context**: Development environment.
- **Decision made**: Target build environment is Google Antigravity.
- **Reason**: System tools, terminal, and AI pair-programming agent environment.
- **Alternatives considered**: Standard manual CLI workflow.
- **Impact**: All build and test workflows validated in Antigravity.

---

### Decision 13: Universal Dictation & Global Hotkeys
- **Context**: The original scope decided a PTT global hotkey + floating widget for Relay's *own* capture modes (meeting/scribble), but left "type transcribed speech into whatever app/field currently has OS focus" (universal dictation) and a global show/hide hotkey undecided by omission.
- **Decision made**: Add two OS-wide global hotkeys via `tauri-plugin-global-shortcut`: a show/hide toggle (default `Ctrl+Shift+Space`) and a push-to-talk universal dictation hotkey (default `Ctrl+Space`, configurable, held down to record). On release, the captured audio is transcribed (STT only, no LLM pipeline) and typed into whichever field has OS focus via the `enigo` crate's keystroke simulation — not necessarily Relay's own window. A small always-on-top, non-focus-stealing indicator window shows "🎙 Listening…" while the hotkey is held.
- **Reason**: This closes a real gap identified against competitors (Handy, VoiceInk, espanso, OpenWhispr) — dictation confined to Relay's own chat/capture UI is a materially smaller feature than typing into Slack, email, or code anywhere on the machine. Relay's existing Rust/Tauri/cpal audio pipeline made this cheaper to add natively than it would be to bolt onto a Python-backed app (no JS `MediaRecorder` bridging needed — the OS-hotkey handler calls the same `AudioRecorder`/`SttEngine` used elsewhere in Rust directly).
- **Alternatives considered**: Confining dictation to Relay's own window only (rejected — doesn't solve the "type into Slack/email/code" use case that makes dictation tools useful); a JS-side `MediaRecorder`-in-hidden-window approach as used by prior Tauri prototypes (rejected — unnecessary indirection given capture already happens in Rust).
- **Impact**: New `native/src-tauri/src/hotkeys/` module (global shortcut registration, indicator window lifecycle, `injection.rs` for `enigo` text injection). `AudioRecorder::start`/`stop` are called directly from the hotkey handler, bypassing Tauri IPC entirely for this flow.

---

### Decision 14: Real Local Speech-to-Text (whisper-rs)
- **Context**: `capture::AudioRecorder` originally returned a hardcoded placeholder transcript string — real STT was never wired up, and `start()` didn't actually record audio either (it created an empty WAV file). Universal dictation and voice chat both need real transcription to be meaningful.
- **Decision made**: Implement real microphone capture via `cpal` (resampled to 16kHz mono) and real local transcription via `whisper-rs` (Rust bindings to whisper.cpp), loading a user-configured GGML model path (`stt.whisper_model_path` in settings). No model ships in the repo — the user downloads one (e.g. from `ggerganov/whisper.cpp` on Hugging Face) and points Relay at it.
- **Reason**: Upholds Decision 4 (zero recurring cost) and Decision 6/product.md's "local STT (Parakeet/Whisper)" commitment for real, not just in the docs. whisper.cpp via `whisper-rs` needs no Python runtime, matching Decision 2's from-scratch-Rust commitment.
- **Alternatives considered**: A Python/faster-whisper sidecar process (rejected — reintroduces the Python dependency Decision 1/2 deliberately avoided); a cloud STT fallback as the default (rejected — would violate the zero-cost-by-default baseline; cloud STT remains a future option, not implemented here).
- **Impact**: `capture::stt::SttEngine` lazily loads and caches the whisper.cpp context; `AudioRecorder` now performs a real dedicated-thread `cpal` capture instead of writing an empty WAV placeholder.

---

### Decision 15: In-App Voice Chat, Grounded in Vault Notes (RAG-lite for now)
- **Context**: Neither the original decisions nor the docs described a conversational "record → transcribe → answer → speak back" surface — vector search (Decision 6) was scoped as an internal retrieval-cost optimization, not a user-facing Q&A feature.
- **Decision made**: Add a "Voice Chat" tab: record a spoken question, transcribe it, retrieve the most relevant vault notes, ask the configured LLM provider for an answer grounded in those notes (with source titles shown), and optionally speak the answer back via a local TTS engine. Retrieval currently uses simple term-overlap scoring over vault note content (`VaultManager::search_notes`), explicitly as a stand-in for the embedded LanceDB vector search Decision 6 already commits to — that embedding pipeline is tracked as backlog (see `docs/roadmap.md`), not implemented in this round.
- **Reason**: This was explicitly requested as core product behavior ("voice input inside the app: record → transcribe → answer → speak back") and is a natural extension of Decision 6's vault-grounded-retrieval commitment. Shipping a real (if simple) retrieval step now, rather than mocking the whole feature, keeps the answers honestly grounded instead of faking groundedness.
- **Alternatives considered**: Waiting for the full LanceDB embedding pipeline before shipping any chat UI (rejected — would block a requested feature on unrelated, larger infrastructure work); ungrounded chat with no vault retrieval (rejected — contradicts Relay's core "grounded in your own notes" positioning).
- **Impact**: New `pipeline::chat::process_chat` function and `VaultManager::search_notes`/`list_notes`. Chat mode reuses the existing `start_capture`/`stop_capture` commands (mode `"chat"`) rather than adding a parallel command surface, and skips trigger-phrase matching (a question containing "remind me" shouldn't be hijacked into firing a reminder).

---

### Decision 16: Optional Local Text-to-Speech (Piper)
- **Context**: product.md's stack references Piper TTS, but no code path ever called it; "speak back" was undecided.
- **Decision made**: Add `tts::TtsEngine`, which shells out to a user-configured Piper binary + voice model to synthesize the voice chat answer as WAV audio, returned to the frontend as base64 and played via a standard HTML `<audio>` element. If either path is unconfigured, this silently degrades to text-only rather than failing the chat response.
- **Reason**: Keeps "speak back" zero-cost and local (Piper is free/local, per the original Technology Stacks research), while not making it a hard requirement — voice chat is fully usable without it.
- **Alternatives considered**: A cloud TTS API as the default (rejected — violates zero-cost-by-default); making Piper mandatory for voice chat to function (rejected — unnecessarily blocks a text-based answer on an optional nicety).
- **Impact**: New `native/src-tauri/src/tts/` module. `ProcessedPipelineResult` gains `sources: Vec<String>` and `spoken_audio_base64: Option<String>` fields (empty/`None` for the pre-existing meeting/scribble modes).

---

### Decision 17: Hybrid-Mode Architecture
- **Context**: Auth and data storage model when hybrid mode is active.
- **Decision made**: Use cloud storage (Supabase PostgreSQL + RLS + real password/token auth) for hybrid mode, NOT remote/tunnel access to the local desktop.
- **Reason**: Remote tunneling introduces network complexity, firewall issues, and desktop uptime dependencies. Cloud BaaS ensures reliable web access. Supabase auto-pause is mitigated with client-side status checks.
- **Alternatives considered**: Tailscale/ngrok tunnel to local machine; custom auth server.
- **Impact**: Clear separation: local mode uses Markdown vault + LanceDB; hybrid mode syncs to Supabase.

---

### Decision 18 (PTT-001): Preserve Backend Ownership of Capture State
- **Context**: UI floating dictation pill overlay redesign vs backend capture state management.
- **Decision made**: Rust backend (`AudioRecorder`, `hotkeys/mod.rs`, `commands.rs`) remains the single source of truth for capture session state.
- **Reason**: Prevents UI/backend state drift, duplicate capture triggers, or lost recording sessions.
- **Alternatives considered**: Frontend-owned `isRecording` state in React.
- **Impact**: React floating pill acts strictly as a consumer of backend events (`capture-state-changed`, `capture-level`).

---

### Decision 19 (PTT-002): Reuse Existing AudioRecorder
- **Context**: Push-to-talk pill overlay recording logic.
- **Decision made**: The floating pill overlay must consume the existing `AudioRecorder` rather than initializing a separate recording pipeline.
- **Reason**: Avoids duplicate microphone capture threads, resource contention, and session conflicts.
- **Impact**: Shared session management across global hotkeys and UI affordances.

---

### Decision 20 (PTT-003): Reuse Existing Local whisper-rs STT
- **Context**: Speech transcription for push-to-talk dictation.
- **Decision made**: PTT dictation uses Relay's existing local `whisper-rs` STT pipeline (with heuristic fallback when unconfigured).
- **Reason**: Preserves zero-cost local operation and offline privacy commitments.
- **Impact**: Fast local transcription without external API dependencies.

---

### Decision 21 (PTT-004): Floating Pill as Control/Presentation Surface Only
- **Context**: Responsibility boundary for floating dictation pill.
- **Decision made**: The pill window is purely a presentation and control surface. A UI crash, hide, or unmount must never corrupt active capture or text injection.
- **Reason**: Decouples UI overlay rendering from background audio recording and OS focus injection.
- **Impact**: Dictation completes reliably even if the overlay window is minimized or hidden.

---

### Decision 22 (PTT-005): Preserve Global Push-to-Talk Hotkey Interaction
- **Context**: Interaction model for OS-wide dictation.
- **Decision made**: Press-and-hold global hotkey (`Ctrl+Space`) with release-triggered text injection remains the primary interaction model.
- **Reason**: Delivers fast, frictionless universal dictation across all desktop applications.
- **Impact**: Native `tauri-plugin-global-shortcut` press/release handlers remain core trigger paths.

---

### Decision 23 (PTT-006): Secondary Click-to-Talk Affordance
- **Context**: Mouse-driven dictation trigger on floating pill overlay.
- **Decision made**: Support click-to-talk on the floating pill using the exact same backend state machine.
- **Reason**: Provides accessibility and mouse convenience without introducing parallel state pipelines.
- **Impact**: Prevents simultaneous mouse/keyboard capture sessions via backend session locks.

---

### Decision 24 (PTT-007): Zero OS Focus Theft
- **Context**: Window focus management during text injection.
- **Decision made**: The floating dictation pill overlay must never steal OS window focus (`focused(false)`, `always_on_top(true)`, `skip_taskbar(true)`).
- **Reason**: OS text injection via `enigo` relies on preserving the user's active application target (Chrome, VS Code, Slack, Notepad, etc.).
- **Impact**: Transcribed text reliably lands in the user's target input field.

---

### Decision 25 (PTT-008): Compact State Visualization Over Live Transcripts
- **Context**: On-screen overlay content during speech capture.
- **Decision made**: The floating pill communicates state (`IDLE` → `LISTENING` → `TRANSCRIBING` → `SUCCESS` / `ERROR`) and live audio level rather than streaming raw transcripts.
- **Reason**: Keeps overlay compact (~420x72px), non-distracting, and privacy-preserving.
- **Impact**: Minimal visual footprint on desktop.

---

### Decision 26 (PTT-009): Real Audio RMS Level Waveform
- **Context**: Visual feedback during active microphone recording.
- **Decision made**: Drive the listening waveform from real-time Root Mean Square (RMS) audio level metrics calculated in Rust and emitted at ~25Hz.
- **Reason**: Gives immediate, tactile visual confirmation of microphone pickup without IPC bloat.
- **Impact**: Emits lightweight `{ level: f32 }` payload every 40ms during recording.

---

### Decision 27 (PTT-010): Structured Capture Event Architecture
- **Context**: Event payload format for `capture-state-changed`.
- **Decision made**: Extend `capture-state-changed` events with structured state machine status (`IDLE`, `STARTING`, `LISTENING`, `STOPPING`, `TRANSCRIBING`, `PROCESSING`, `INJECTING`, `SUCCESS`, `ERROR`, `CANCELLED`, `NO_SPEECH`).
- **Reason**: Provides complete state synchronization across all application windows.
- **Impact**: Single unified state event schema for desktop overlay and main window.

---

### Decision 28 (PTT-011): Defer Dictation Indicator Consolidation
- **Context**: Coexistence of `dictation-pill` and `dictation-indicator` windows.
- **Decision made**: Preserve `dictation-indicator` until the upgraded `dictation-pill` passes full regression testing across all platforms.
- **Reason**: Avoids breaking existing shortcut flows during transition.
- **Impact**: Safe, incremental migration to unified pill overlay.

---

### Decision 29 (PTT-012): Defer Active-Monitor Utility Positioning
- **Context**: Multi-monitor overlay placement.
- **Decision made**: Use bottom-center primary monitor positioning for MVP, deferring active-window monitor auto-detection to post-MVP utility.
- **Reason**: Focuses immediate scope on core PTT reliability and focus preservation.
- **Impact**: Clean, predictable default overlay placement across single and multi-monitor setups.

---

### Decision 30 (PTT-013): Supersede Decision 28 — Remove `dictation-indicator`, Single-Window RESTING/EXPANDED Pill
- **Context**: In practice, having both `dictation-indicator` (a hardcoded solid-red "🎙 Listening…" box, shown/hidden directly by the hotkey handler) and `dictation-pill` (which independently reacts to the same `capture-state-changed` event) meant Ctrl+Space produced two separate, uncoordinated visual surfaces at once — confusing, and not what was asked for. The pill was also a single fixed-size (420×72) window at all times, so its fully-transparent, click-through-unaware bounds around a compact idle state were larger than the visible content, which is exactly what causes clicks meant for whatever's underneath to land on the invisible overlay instead, and would clip/hide any future dialog that overflowed those fixed bounds.
- **Decision made**: Remove `dictation-indicator` entirely (window creation, show/hide calls, the component, its hash route, `INDICATOR_WINDOW_LABEL`) — verified nothing else in the codebase referenced it. `dictation-pill` is now the *only* PTT visual surface, with one native window and one presentation state machine: RESTING (compact, e.g. ~56×56) or EXPANDED (~420×72, the listening/processing/success/error body). Ctrl+Space and hovering the resting pill both expand it (backend capture events and local hover state respectively); releasing/mouse-away collapses it back. The frontend reports its own RESTING/EXPANDED choice to Rust via a new `set_pill_expanded` command, which resizes *and* re-anchors the native window to tightly match — never a bigger invisible hit-region than what's actually visible.
- **Reason**: This is what was actually asked for (one PTT surface, not two), and closes the invisible-overlay class of bug (stray click-throughs, potential dialog clipping) at the architecture level rather than patching it with CSS z-index, which can't affect native window ordering or hit-testing at all.
- **Alternatives considered**: Keeping `dictation-indicator` as a fallback for the (now nonexistent) case where the pill fails to load (rejected — adds a second surface back for a case that's already handled by the pill's own error phase); resizing the pill window continuously to match arbitrary content width (rejected as unnecessary complexity — two fixed sizes cover every current phase's content).
- **Impact**: `hotkeys/mod.rs` loses `INDICATOR_WINDOW_LABEL`, `ensure_indicator_window`, `compute_bottom_right_position`, `show_indicator`, `hide_indicator`, and the `WebviewUrl`/`WebviewWindowBuilder` imports they needed. `DictationIndicator.tsx` and its `#/dictation-indicator` route are deleted.

---

### Decision 31 (PTT-014): Work-Area-Aware, Cursor-Monitor-Relative Pill Positioning
- **Context**: The original `compute_bottom_center_position`/`compute_bottom_right_position` hardcoded `screen_height - pill_height - 48` off the *primary* monitor's full size (not its usable work area), which would sit the pill on top of a taskbar rather than above it, ignore auto-hide taskbars, and always use the primary monitor regardless of which one the user was actually on.
- **Decision made**: Position the pill using Tauri's `Monitor::work_area()` (the OS-reported usable desktop region, which already excludes a fixed taskbar/dock and adapts to an auto-hidden one — no taskbar height is ever hardcoded) of whichever monitor is currently under the OS cursor (`AppHandle::cursor_position` + `monitor_from_point`), falling back to the primary monitor if that lookup fails. Add a `UiSettings.pill_position` (`BottomCenter` | `TopCenter` | `LeftCenter` | `RightCenter`) anchor choice, exposed in Settings. Position is recomputed from scratch every time the pill is shown or resized (rather than cached), which is what actually keeps it correct across monitor changes, resolution changes, DPI changes, and position-setting changes without needing a live monitor-hotplug subscription Tauri's public API doesn't expose.
- **Reason**: Matches how OS-native flyout/notification widgets behave, and closes the "sits under the taskbar" / "wrong monitor" class of bug at the source instead of adding more hardcoded offsets.
- **Alternatives considered**: Tracking the foreground application's window/monitor directly (rejected — Tauri has no cross-platform public API for "which window is focused system-wide," so the cursor's monitor, which is where the user's attention almost always already is, is the closest available proxy); a persistent monitor-configuration watcher (rejected — recomputing on every show/resize is simpler and covers the same cases in practice, since the pill isn't visible/interactable in between).
- **Impact**: `overlay.rs`'s `compute_anchor`/`active_monitor` replace the old primary-monitor-only math. `PillPosition` lives in `settings::mod` (persisted) and is threaded through `ensure_pill_window`, `set_expanded`, and the new `set_pill_position`/`reposition_pill` command/function.

---

### Decision 32: Desktop-First Scope Reduction — Web Surface Deferred (Scope-Reduction Set 1)
- **Context**: Relay's active product is being reduced to a single, stable, desktop-first surface: global PTT + click-to-talk + local STT + text injection through one Dictation Pill. A repository audit (Scope-Reduction Set 0, 2026-08-20) confirmed `web/` (the Next.js dashboard) has zero import, build, or runtime coupling to `native/` — no shared workspace config, no path references in either direction, and the "desktop app syncs vault notes to Supabase" claim in `docs/architecture.md` has no corresponding implementation anywhere in `native/src-tauri` (a case-insensitive grep for "supabase" across the entire Rust backend returns zero matches, and there is no Supabase/Postgres crate dependency). `web/`'s own Supabase client is itself a `MockSupabaseClient` returning hardcoded data, and its auth layer returns a hardcoded dummy user — the web surface's own hybrid-sync code is a stub, not just disconnected from the desktop app.
- **Decision made**: Web is deferred for the current product phase. Its implementation is preserved untouched under `web/`; it is removed from active MVP scope in `docs/product.md` and `docs/requirements.md`, and its presence in `docs/architecture.md`'s three-surface diagram is annotated as deferred rather than redrawn.
- **Reason**: The Web surface adds zero risk to remove from active scope because it was never wired into the desktop build in the first place — there is no navigation entry, startup path, build step, or IPC surface in `native/` referencing it. Preserving the implementation costs nothing; rebuilding, migrating, or dismantling it would be pure wasted effort against a surface that isn't part of the current MVP.
- **Existing Functionality Preserved**: All of `web/`'s existing code, and all of `native/`'s existing functionality (PTT, click-to-talk, capture, STT, injection, Kanban, Voice Chat, Triggers, Settings), is completely unchanged. Nothing was deleted, migrated, or refactored.
- **Explicitly Not Changed**: No files under `web/`. No Rust or TypeScript code under `native/`. No settings, commands, build configuration, or CI.
- **Deferred**: The entire `web/` surface — Next.js dashboard, Supabase-backed hybrid auth/sync (not yet implemented on either side), and any future "access notes/Kanban from a browser" functionality — remains available to resume later, unmodified.
- **Supersedes**: Decision 3 ("Hybrid Deployment Includes Web Surface") is superseded for the current desktop-first MVP phase only — not deleted or invalidated as history. Dual-surface hybrid deployment may be resumed in a future phase; this decision does not rule it out permanently.

---

### Decision 33: Desktop-First Scope Reduction — Kanban Deferred (Scope-Reduction Set 2)
- **Context**: Continuing the desktop-first scope reduction (Decision 32), a repository audit found Kanban's own feeder pipeline (`PipelineEngine::process_meeting`, matched on `mode:"meeting"`) is already unreachable in the running app — no frontend code path sets that mode since the PTT redesign collapsed capture into one mode-less Dictation Pill. `KanbanBoard.tsx` was reachable only via its own sidebar nav tab (not the default view), and its backend read path (`get_kanban_cards` → `VaultManager::list_kanban_cards`) is cleanly isolated from the vault/notes machinery Scribble Notes and Voice Chat actually use.
- **Decision made**: Remove Kanban's active navigation from `native/src/App.tsx`: the sidebar "Kanban Board" nav button, the `kanban` branch of the tab switcher and hero header, the `kanban` value from the `navigate-tab` event allowlist, and the unconditional `get_kanban_cards` fetch that previously ran on every app launch regardless of whether Kanban was ever opened. `KanbanBoard.tsx`, the `KanbanCard` type, and the backend `get_kanban_cards` command / `VaultManager` Kanban read/write methods are all left fully intact and untouched.
- **Reason**: This is the "Option 1 — remove active surface only" path the scope-reduction process prefers: no code needed to change inside the Kanban implementation itself, because its only active-surface footprint was `App.tsx`'s navigation and an unconditional startup fetch. Removing the startup fetch also resolves the one flagged "required at startup" concern from the Set 0 audit.
- **Existing Functionality Preserved**: `KanbanBoard.tsx` (unmodified), the `KanbanCard` type in `types/index.ts` (unmodified, still used by `KanbanBoard.tsx`), the `get_kanban_cards` Tauri command, and `VaultManager`'s Kanban card read/write methods and `.relay/vault/kanban/` storage — all unchanged and available to reconnect later.
- **Explicitly Not Changed**: No Rust code. No settings (Kanban had none). Scribble Notes, Voice Chat, PTT, click-to-talk, hotkeys, STT, and text injection — untouched.
- **Deferred**: The Kanban board UI and its meeting-transcript-to-task-card pipeline concept, resumable by re-adding the nav entry and (separately, if ever needed) wiring a UI path that sets `mode:"meeting"` again.
- **Supersedes**: Decision 7 ("Kanban Delivery Scope for MVP") is superseded for the current desktop-first MVP phase only — not deleted or invalidated as history.

---

### Decision 34: Desktop-First Scope Reduction — Voice Chat & TTS Deferred (Scope-Reduction Set 3)
- **Context**: Continuing the desktop-first scope reduction (Decisions 32–33), the Set 0 audit confirmed Voice Chat is fully implemented and active — `ChatPanel.tsx`, wired into `App.tsx`'s "Voice Chat" nav tab, backed by real `pipeline::chat::process_chat` (vault-grounded retrieval + LLM answer + optional TTS) — contrary to an initial assumption that it might not exist. TTS (`tts::TtsEngine`, shelling out to a user-configured Piper binary) has exactly one call site in the entire backend: `pipeline::chat::process_chat`. The audit also confirmed `providers::LLMClient` (the shared Ollama/cloud client) is used identically by Voice Chat, the Kanban/meeting pipeline, and Scribble Notes — it is genuinely shared infrastructure, not something Voice Chat owns.
- **Decision made**: Remove Voice Chat's active navigation from `native/src/App.tsx` (sidebar "Voice Chat" nav button, the `chat` tab-switch/hero-header branch, `chat` from the `navigate-tab` allowlist), the same pattern used for Kanban in Decision 33. Additionally remove the Piper TTS configuration block from `native/src/components/settings/ProviderSettings.tsx` (General section) — its only stated purpose, per its own caption text, was to "skip 'speak back' in voice chat," so it is Voice Chat's settings surface, not an independent one. `ChatPanel.tsx`, `pipeline::chat::process_chat`, `tts::TtsEngine`, and `providers::LLMClient` are left fully untouched.
- **Reason**: Per the scope-reduction rules, TTS is removed only if it has no active consumer and removal is low-risk; `process_chat` (preserved code) still calls it, so the module itself is not removed, only its UI entry points (Voice Chat's tab and its settings block) — this makes both features unreachable from the UI without touching any Rust code or the shared LLM client Kanban/meeting and Scribble Notes depend on.
- **Existing Functionality Preserved**: `ChatPanel.tsx`, `pipeline::chat::process_chat`, `tts::TtsEngine`, `providers::LLMClient` (used identically by the in-scope Kanban/meeting and Scribble pipelines), and the `TtsSettings`/`AppSettings.tts` struct fields and their load/save round-trip — all unchanged.
- **Explicitly Not Changed**: No Rust code. `DEFAULT_SETTINGS.tts` and the `AppSettings`/`ProcessedPipelineResult` TypeScript types in `types/index.ts` are unchanged (still needed for settings round-tripping and by `ChatPanel.tsx` itself). Kanban, PTT, click-to-talk, hotkeys, STT, and text injection — untouched.
- **Deferred**: The Voice Chat tab, its "speak back" TTS configuration UI, and the underlying vault-grounded Q&A + optional-speech feature — resumable by re-adding the nav entry and settings block.
- **Supersedes**: Decision 15 ("In-App Voice Chat, Grounded in Vault Notes") and Decision 16 ("Optional Local Text-to-Speech (Piper)") are superseded for the current desktop-first MVP phase only — not deleted or invalidated as history.

---

### Decision 35: Desktop-First Scope Reduction — Triggers/MCP Deferred (Scope-Reduction Set 4)
- **Context**: Continuing the desktop-first scope reduction (Decisions 32–34), the Set 0 audit found Triggers/MCP genuinely coupled to the core capture path, not just to a settings screen: `TriggerEngine::match_transcript`/`McpRouter::dispatch_action` ran inline inside `process_captured_audio` (the function backing click-to-talk's "scribble" mode) for any capture whose mode wasn't `"chat"` — meaning a spoken phrase matching one of the two *enabled-by-default* triggers ("schedule meeting", "remind me to") would silently short-circuit click-to-talk dictation into a canned MCP-stub reply instead of running the normal cleanup pipeline, on every fresh install, not only for users who had configured their own triggers. `TriggerSettings.tsx` was also reachable from two places: `App.tsx`'s own "Trigger Phrases" tab, and a redundant "Triggers & MCP" sub-section inside `ProviderSettings.tsx`'s own settings sub-nav.
- **Decision made**: Remove both active entry points to `TriggerSettings.tsx` (the `App.tsx` top-level tab and the `ProviderSettings.tsx` sub-nav section). Remove the inline trigger-match-and-MCP-dispatch block from `process_captured_audio` in `native/src-tauri/src/commands.rs`, so capture processing falls straight through to the mode-based pipeline dispatch (meeting/chat/scribble) exactly as it would for any transcript that didn't match a trigger. `TriggerEngine`, `McpRouter`, and the `get_triggers`/`save_triggers` Tauri commands are left completely intact — only their automatic invocation inside the core capture path is removed, not the modules themselves.
- **Reason**: Per the scope-reduction rules, active entry points for a feature not required by core should be removed; the settings UI was the obvious entry point, but the audit specifically flagged the inline dispatch as a second, more consequential one — it executed automatically on the core dictation path with no way for a user to see or control it once the settings UI is gone, which is precisely the "accidentally triggered by the core workflow" state the rules prohibit for deferred functionality. `TriggerEngine`/`McpRouter` are not constructed as app-wide state (no `.manage()` entry in `lib.rs`) and were used only from the two now-removed call sites, so this is a self-contained, low-risk removal.
- **Existing Functionality Preserved**: `native/src-tauri/src/triggers/mod.rs` (`TriggerEngine`, unmodified, including its unit tests), `native/src-tauri/src/mcp/mod.rs` (`McpRouter`, unmodified), the `get_triggers`/`save_triggers` Tauri commands, and `TriggerSettings.tsx` itself — all unchanged and available to reconnect later.
- **Explicitly Not Changed**: PTT, click-to-talk's capture/transcription steps, hotkeys, STT, text injection, Kanban, Voice Chat, Scribble Notes, and every setting other than the two Triggers nav entries.
- **Deferred**: Trigger-phrase configuration and MCP action dispatch — resumable by re-adding the nav entries and the `process_captured_audio` dispatch block.
- **Supersedes**: Decision 8 ("MCP Integrations") and Decision 10 ("Trigger Phrases are User-Customizable") are superseded for the current desktop-first MVP phase only — not deleted or invalidated as history.

