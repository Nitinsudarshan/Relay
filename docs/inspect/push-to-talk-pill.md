# Push-to-Talk Pill Inspection

## Repository & Component Mapping

| Item | File | Component / Function | Responsibility | Dependencies | Planned Action |
| --- | --- | --- | --- | --- | --- |
| **Entry Point** | `native/src/main.tsx` | `isOverlayWindow` routing | Detects `#/dictation-pill` hash route and mounts `<FloatingPill />` | Vite, React DOM | Preserve |
| **Window Host** | `native/src/components/capture/FloatingPill.tsx` | `FloatingPill` | Hosts dictation surface inside overlay window | `DictationPill` | Preserve |
| **Core Surface** | `native/src/components/capture/DictationPill.tsx` | `DictationPill` | Unified notch, pill, listening, processing, and popup state machine | React, Lucide Icons, `@tauri-apps/api` | **Redesign**: Fix overlap bug, add horizontal notch, Oscar styling, hotkey hint |
| **Settings Popover** | `native/src/components/capture/PillSettingsPopover.tsx` | `PillSettingsPopover` | Dropdown panel with auto-paste, transform, cleanup style, prompt mode, language | Lucide Icons, Tailwind CSS | **Update**: Match Oscar dropdown structure with real settings |
| **Types & Statuses** | `native/src/components/capture/PillTypes.ts` | Types | Types for state machine, dependency checks, and diagnostics | TypeScript | **Update**: Add cleanup style & language types |
| **Window Positioning** | `native/src-tauri/src/overlay.rs` | `ensure_pill_window`, `reposition`, `compute_anchor` | Monitor work-area detection, screen edge anchoring, taskbar inset handling | Tauri `WebviewWindowBuilder`, `Monitor` | **Update**: Adjust resting size to notch dimensions (`64x12`), re-anchor to edge |
| **IPC Commands** | `native/src-tauri/src/commands.rs` | `set_pill_window_mode`, `start_capture`, `stop_capture` | Communication between React UI and Rust capture engine | Tokio, Tauri IPC | Preserve IPC layer |
| **Global Hotkey** | `native/src-tauri/src/hotkeys/mod.rs` | `register_hotkeys` | Registers OS global shortcut (`Ctrl+Space`) and triggers capture | `tauri-plugin-global-shortcut` | Preserve |
| **Active App Detection**| `native/src-tauri/src/capture/mod.rs` / `overlay.rs` | App detection | Identifies current foreground process name for pill label | Windows Win32 API | Integrate Process Name detection |
| **STT Engine** | `native/src-tauri/src/capture/stt.rs` | `SttEngine` | Local Whisper transcription | `whisper-rs` | Preserve |
| **LLM / Transformation**| `native/src-tauri/src/providers/mod.rs` | `LLMClient` | Prompt mode & cleanup transformations | Ollama / Cloud API | Preserve |
