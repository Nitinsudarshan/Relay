# Relay — Changelog

## [0.2.0] - 2026-08-19

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

## [0.1.1] - 2026-08-19

### Improvements & UI Refactoring

- **Native Frontend (`native/`)**:
  - Installed `@radix-ui/react-slot`, `@radix-ui/react-tabs`, `@radix-ui/react-dialog`, `@radix-ui/react-badge`, `clsx`, `tailwind-merge`, and `class-variance-authority`.
  - Created [`native/src/lib/utils.ts`](file:///d:/Projects/Relay/native/src/lib/utils.ts) `cn` helper function.
  - Added shadcn UI primitives: [`Button`](file:///d:/Projects/Relay/native/src/components/ui/button.tsx), [`Card`](file:///d:/Projects/Relay/native/src/components/ui/card.tsx), [`Badge`](file:///d:/Projects/Relay/native/src/components/ui/badge.tsx), [`Input`](file:///d:/Projects/Relay/native/src/components/ui/input.tsx).
  - Refactored `PTTWidget`, `KanbanBoard`, `ScribbleViewer`, `TriggerSettings`, and `ProviderSettings` to strictly use shadcn UI components and design tokens per `rules/ui-components.md`.

## [0.1.0] - 2026-08-19

### Initial Release — Multi-Surface Architecture & Core Pipeline

- **Living Specification (`docs/`)**:
  - `product.md`: Product vision, target user, core problems, scope boundaries.
  - `decisions.md`: Seeded all 12 pre-confirmed architectural decisions.
  - `requirements.md`: Detailed functional and non-functional requirements.
  - `user-flows.md`: Defined end-to-end PTT capture, meeting parsing, scribble, and trigger flows.
  - `architecture.md`: System design, Rust domain module boundaries, and IPC bridge.
  - `data-model.md`: Schemas for Vault Notes, Kanban Cards, Triggers, Provider Settings, and LanceDB.
  - `api.md`: Tauri IPC `CommandResponse<T>` and web API route handler specifications.
  - `testing.md`: Testing strategy across Rust backend and React/Next.js frontends.

- **Native Desktop App (`native/`)**:
  - `src-tauri`: Rust backend domain modules (`providers`, `capture`, `vault`, `pipeline`, `triggers`, `mcp`, `commands.rs`).
  - `src`: React + TypeScript desktop UI (`PTTWidget`, `KanbanBoard`, `ScribbleViewer`, `TriggerSettings`, `ProviderSettings`, `App`).
  - Generated application icon assets for Windows resource compilation.

- **Web Dashboard Surface (`web/`)**:
  - Restructured Next.js boilerplate into `web/` per `project-structure.md`.
  - Configured Turbopack root & Supabase client interface for hybrid mode.
  - `(dashboard)/page.tsx`: Hybrid dashboard showing synced Kanban task board.
