# Relay — Data Model Specification

## 1. Vault Markdown Note Schema
Markdown files are stored in `.relay/vault/notes/<id>.md` with YAML frontmatter headers:

```markdown
---
id: "note_123456789"
title: "Product Architecture Sync"
type: "meeting" | "scribble" | "trigger"
created_at: "2026-08-19T01:50:00Z"
updated_at: "2026-08-19T01:50:00Z"
tags: ["meeting", "architecture"]
source_audio: ".relay/audio/20260819_015000.wav"
---

# Executive Summary
...

## Key Takeaways
- ...
```

## 2. Kanban Card Schema
Kanban tasks are represented as structured Markdown files in `.relay/vault/kanban/<id>.md`:

```markdown
---
id: "card_987654321"
title: "Scaffold Rust Tauri backend"
assignee: "Nitin"
status: "todo" | "in_progress" | "done"
priority: "high" | "medium" | "low"
due_date: "2026-08-25"
created_at: "2026-08-19T01:50:00Z"
source_meeting_id: "note_123456789"
---

### Description
Implement Rust backend domain modules per `project-structure.md`.
```

## 3. Trigger Phrase Configuration Schema (`triggers.json`)
Stored at `.relay/config/triggers.json`:

```json
{
  "triggers": [
    {
      "id": "trig_001",
      "phrase": "schedule meeting",
      "action_type": "mcp_calendar",
      "target_tool": "google_calendar_create_event",
      "parameters": {
        "calendar_id": "primary"
      },
      "enabled": true
    },
    {
      "id": "trig_002",
      "phrase": "remind me to",
      "action_type": "local_reminder",
      "target_tool": "os_notification",
      "parameters": {},
      "enabled": true
    }
  ]
}
```

## 4. App Settings Configuration Schema (`settings.json`)
Stored at `.relay/config/settings.json`. Mirrors the Rust `AppSettings` struct
(`native/src-tauri/src/settings/mod.rs`) exactly — this is the real
implemented shape, not an aspirational one:

```json
{
  "provider": {
    "active_provider": "ollama",
    "ollama_host": "http://localhost:11434",
    "ollama_model": "llama3.2:latest",
    "cloud_api_key": null,
    "cloud_model": "gpt-4o-mini"
  },
  "stt": {
    "whisper_model_path": null
  },
  "tts": {
    "piper_binary_path": null,
    "piper_voice_path": null
  },
  "hotkeys": {
    "show_hide_hotkey": "Ctrl+Shift+Space",
    "dictation_hotkey": "Ctrl+Space"
  }
}
```

`active_provider` is one of `"ollama" | "cloud_openai" | "cloud_gemini" | "cloud_anthropic"`.
`stt.whisper_model_path` must point at a local GGML Whisper model file (not
bundled with Relay) for any transcription — meeting/scribble capture, voice
chat, and universal dictation — to work. `tts.*` are both optional; when
either is unset, voice chat answers are text-only.

## 5. LanceDB Vector Record Schema
Table `note_embeddings` inside LanceDB database `.relay/lancedb`:

| Field | Type | Description |
|---|---|---|
| `id` | String (PK) | Note or card ID |
| `vector` | FixedSizeList<Float32, 384> | Dense embedding vector |
| `path` | String | File path in markdown vault |
| `content_snippet` | String | First 500 characters of content |
| `created_at` | String | ISO timestamp |
