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
