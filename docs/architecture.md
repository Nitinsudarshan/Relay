# Relay — System Architecture

## Three-Surface Overview

Relay is architected into three distinct surfaces in a single repository:

```
                          ┌─────────────────────────────────────┐
                          │         Web Surface (web/)          │
                          │   Next.js 15 + Shadcn + Supabase    │
                          └──────────────────┬──────────────────┘
                                             │ Hybrid Sync (Supabase Auth/DB)
                                             ▼
┌────────────────────────────────────────────────────────────────────────┐
│                        Native Desktop (native/)                        │
│                                                                        │
│  ┌──────────────────────────────────────────────────────────────────┐  │
│  │                     React UI (native/src/)                       │  │
│  │   PTT Widget | Kanban Board | Voice Chat | Trigger/Provider      │  │
│  │   Settings | Dictation Indicator (separate always-on-top window) │  │
│  └─────────────────────────────────┬────────────────────────────────┘  │
│                                    │ Tauri IPC (invoke commands)       │
│  ┌─────────────────────────────────▼────────────────────────────────┐  │
│  │                 Rust Backend (native/src-tauri/)                 │  │
│  │                                                                  │  │
│  │  ┌──────────────┐  ┌──────────────┐  ┌────────────────────────┐  │  │
│  │  │   capture    │  │   pipeline   │  │        triggers        │  │  │
│  │  │(cpal+whisper)│  │(Kanban/Scrib/│  │ (Intent/MCP Router)    │  │  │
│  │  │              │  │ Chat/RAG)    │  │                        │  │  │
│  │  └──────────────┘  └──────────────┘  └────────────────────────┘  │  │
│  │  ┌──────────────┐  ┌──────────────┐  ┌────────────────────────┐  │  │
│  │  │  providers   │  │    vault     │  │          mcp           │  │  │
│  │  │(Ollama/Cloud)│  │(Vault/search)│  │(Calendar/Notion/Drive) │  │  │
│  │  └──────────────┘  └──────────────┘  └────────────────────────┘  │  │
│  │  ┌──────────────┐  ┌──────────────┐  ┌────────────────────────┐  │  │
│  │  │   hotkeys    │  │     tts      │  │       settings         │  │  │
│  │  │(global+inject│  │   (Piper)    │  │  (settings.json I/O)   │  │  │
│  │  └──────────────┘  └──────────────┘  └────────────────────────┘  │  │
│  └──────────────────────────────────────────────────────────────────┘  │
└────────────────────────────────────────────────────────────────────────┘
```

## Rust Backend Module Design (`native/src-tauri/src/`)

- `capture`: Manages audio device input via `cpal` on a dedicated thread (resampled to 16kHz mono), writes WAV files, and (`capture::stt`) transcribes via a local Whisper model (`whisper-rs`).
- `pipeline`: Core prompt engines (all in `mod.rs` except `chat.rs`).
  - `process_meeting`: Extracts structured JSON tasks from raw meeting transcripts.
  - `process_scribble`: Generates structured Markdown documents from unstructured voice scribbles.
  - `chat.rs` / `process_chat`: Voice chat — retrieves grounding notes from the vault, asks the LLM provider, optionally synthesizes speech via `tts`.
- `triggers`: Dynamic phrase matching and classification against `triggers.json`. Extracts parameters and dispatches to tool handlers. Skipped for chat mode.
- `providers`: Unified `LLMClient`.
  - Ollama: Connects to `http://localhost:11434`.
  - Cloud: Connects to OpenAI / Gemini / Anthropic OpenAI-compatible endpoints.
- `vault`: Reads and writes markdown note files with frontmatter headers. `search_notes`/`list_notes` provide keyword-ranked retrieval over vault notes — a placeholder for the embedded LanceDB vector search Decision 6 commits to (see `docs/roadmap.md`).
- `mcp`: Interface for dispatching trigger actions (`google-calendar`, `notion`, `gdrive`, OS notifications) — currently returns stubbed success results; real MCP client wiring is tracked as backlog (`docs/roadmap.md`).
- `hotkeys`: Registers the show/hide and universal-dictation global OS hotkeys (`tauri-plugin-global-shortcut`), manages the always-on-top listening indicator window, and (`hotkeys::injection`) types transcribed text into whatever field has OS focus via `enigo`.
- `tts`: Optional local text-to-speech via a user-configured Piper binary + voice model, used by voice chat's "speak back."
- `settings`: Loads/saves `AppSettings` (provider, STT, TTS, hotkey config) at `.relay/config/settings.json`.
- `commands.rs`: Exposes thin `#[tauri::command]` functions returning `Result<T, CommandError>`.

## Data Access & Security Model
- **Local-Only Mode**: No authentication required. Notes saved in `.relay/vault`. Vector indices stored in `.relay/lancedb`. Zero network activity required.
- **Hybrid Cloud Mode**: Supabase client in `web/src/lib/supabase` for web dashboard authentication. Desktop app syncs vault notes to Supabase PostgreSQL database using RLS policies.
