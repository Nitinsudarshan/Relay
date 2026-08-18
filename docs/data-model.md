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

## 4. Provider Settings Configuration Schema (`settings.json`)
Stored at `.relay/config/settings.json`:

```json
{
  "active_provider": "ollama",
  "ollama": {
    "host": "http://localhost:11434",
    "model": "llama3.2:latest"
  },
  "cloud": {
    "provider_type": "openai",
    "api_key": "sk-...",
    "model": "gpt-4o-mini"
  },
  "stt_provider": "whisper_local",
  "vault_path": "./.relay/vault"
}
```

## 5. LanceDB Vector Record Schema
Table `note_embeddings` inside LanceDB database `.relay/lancedb`:

| Field | Type | Description |
|---|---|---|
| `id` | String (PK) | Note or card ID |
| `vector` | FixedSizeList<Float32, 384> | Dense embedding vector |
| `path` | String | File path in markdown vault |
| `content_snippet` | String | First 500 characters of content |
| `created_at` | String | ISO timestamp |
