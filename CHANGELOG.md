# Relay — Changelog

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
