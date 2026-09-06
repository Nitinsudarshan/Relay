---
trigger: always_on
description: Folder structure — where new files should be created
---

# Project Folder Structure

Relay is a native-first Rust + Tauri desktop application for Windows with a
companion browser extension for web capture. The layout below is what exists
today; put new files where it says rather than starting a parallel tree.

```
relay/
  native/                    Tauri desktop app (Windows, local mode)
    browser-extension/        Companion browser extension for web capture
    src-tauri/                Rust backend
      src/
        capture/               Push-to-talk, local Whisper/Parakeet STT, web capture wiring
        meetings_v2/           Meeting detection, diarization, summarization pipelines
        pipeline/              Kanban parser, scribble->structured-output
        triggers/              Configurable trigger-phrase system
        providers/             Local / cloud LLM providers
        vault/                 Local markdown vault + LanceDB access
        identity/              Native telemetry and anonymous identity
        updates/               Native update checks
        commands.rs            #[tauri::command] entry points (thin, see rust-backend.md)
    src/                       React frontend rendered inside the Tauri window
      components/
        ui/                    shadcn/ui primitives (generated, low-touch)
        shared/                Reusable components
      hooks/
      lib/
      types/
  docs/                      Living spec — see docs/README.md for the index
```

## Rules

- Reusable UI primitives (buttons, inputs, dialogs) go in `native/src/components/ui`
  — prefer generating via the shadcn CLI over hand-writing them.
- Shared, non-primitive components go in `native/src/components/shared` or
  feature-specific component directories (e.g. `components/meetings_v2/`).
- Rust modules under `src-tauri/src/` are organized by domain
  (`capture/`, `pipeline/`, `triggers/`, `providers/`, `vault/`, `mcp/`), not
  by technical layer — a feature's parsing, validation, and persistence logic
  live together in its own module, not scattered across generic `services/`
  or `utils/` folders.
- `commands.rs` (or equivalent domain command modules registered via Tauri) is
  the **only** place `#[tauri::command]` functions live — they call into
  domain modules, they don't contain business logic themselves (see
  `rust-backend.md`).
- `native/src/` never talks to Supabase directly — only the Rust backend does
  (see `security.md`).
- TypeScript types go in `native/src/types`.
- `docs/` is the living spec — keep it current, it's not optional
  documentation. `docs/README.md` says what each file is for and what does
  not warrant a new one.

