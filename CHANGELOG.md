# Relay — Changelog

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
