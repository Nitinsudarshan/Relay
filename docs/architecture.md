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
│  │        PTT Floating Widget | Kanban Board | Trigger Settings     │  │
│  └─────────────────────────────────┬────────────────────────────────┘  │
│                                    │ Tauri IPC (invoke commands)       │
│  ┌─────────────────────────────────▼────────────────────────────────┐  │
│  │                 Rust Backend (native/src-tauri/)                 │  │
│  │                                                                  │  │
│  │  ┌──────────────┐  ┌──────────────┐  ┌────────────────────────┐  │  │
│  │  │   capture    │  │   pipeline   │  │        triggers        │  │  │
│  │  │ (WASAPI/STT) │  │(Kanban/Scrib)│  │ (Intent/MCP Router)    │  │  │
│  │  └──────────────┘  └──────────────┘  └────────────────────────┘  │  │
│  │  ┌──────────────┐  ┌──────────────┐  ┌────────────────────────┐  │  │
│  │  │  providers   │  │    vault     │  │          mcp           │  │  │
│  │  │(Ollama/Cloud)│  │(Vault/Lance) │  │(Calendar/Notion/Drive) │  │  │
│  │  └──────────────┘  └──────────────┘  └────────────────────────┘  │  │
│  └──────────────────────────────────────────────────────────────────┘  │
└────────────────────────────────────────────────────────────────────────┘
```

## Rust Backend Module Design (`native/src-tauri/src/`)

- `capture`: Manages audio device input via WASAPI (`cpal`), writes audio buffer to WAV files, calls local Whisper / Parakeet model or cloud STT.
- `pipeline`: Core prompt engines.
  - `meeting.rs`: Extracts structured JSON tasks from raw meeting transcripts.
  - `scribble.rs`: Generates structured Markdown documents from unstructured voice scribbles.
- `triggers`: Dynamic phrase matching and classification against `triggers.json`. Extracts parameters and dispatches to tool handlers.
- `providers`: Unified `LLMProvider` trait.
  - `OllamaProvider`: Connects to `http://localhost:11434`.
  - `CloudProvider`: Connects to OpenAI / Gemini / Anthropic OpenAI-compatible endpoints.
- `vault`: Reads and writes markdown note files with frontmatter headers. Interfaces with embedded `LanceDB` vector table for embedding generation and similarity search.
- `mcp`: JSON-RPC client interface for connecting to MCP servers (`google-calendar`, `notion`, `gdrive`) and OS desktop notifications.
- `commands.rs`: Exposes thin `#[tauri::command]` functions returning standardized `CommandResponse<T>`.

## Data Access & Security Model
- **Local-Only Mode**: No authentication required. Notes saved in `.relay/vault`. Vector indices stored in `.relay/lancedb`. Zero network activity required.
- **Hybrid Cloud Mode**: Supabase client in `web/src/lib/supabase` for web dashboard authentication. Desktop app syncs vault notes to Supabase PostgreSQL database using RLS policies.
