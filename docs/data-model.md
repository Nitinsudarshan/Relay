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

## 6. Web Capture Schema

A capture is a `VaultFile` (`native/src-tauri/src/vault/file.rs`) with the
optional `capture` field populated. Captures live in their own directory so
the Files surface — which lists `vault/files/` only — is unaffected:

```text
.relay/vault/captures/<capture_id>/
  metadata.json                     # the VaultFile record
  original/<Sanitized-Title>.json   # the raw structured payload, written once
```

`file_type` is `"webcapture"`, `mime_type` is `"application/json"`,
`content` is the normalized markdown, `vault_path` points at the raw payload,
and `last_known_source_path` is the captured URL. `content_hash` is a SHA-256
over the *captured content* (title, url, structured content) rather than the
rendered markdown, which carries the capture timestamp and would otherwise
make every re-capture look like a change.

`capture` carries provenance only. Semantic fields (`summary`, `tags`,
`topics`, `entities`, `ai_metadata`) are produced later by analysis and are
deliberately kept out of it:

```json
{
  "source_type": "web",
  "capture_type": "conversation",
  "application": "ChatGPT",
  "domain": "chatgpt.com",
  "url": "https://chatgpt.com/c/2b1f0e3a",
  "page_title": "Designing Relay Capture",
  "captured_at": "2026-02-14T09:30:12.884Z",
  "browser_captured_at": "2026-02-14T09:30:12.104Z",
  "browser": "Mozilla/5.0 ...",
  "extractor_id": "chatgpt",
  "extractor_version": 1,
  "fidelity": "structured",
  "coverage": "rendered_dom",
  "notes": ["Only the turns the page had rendered were captured. ..."],
  "message_count": 24,
  "block_count": 58,
  "skipped_block_count": 0,
  "truncated": false,
  "canonical_url": null,
  "author": null,
  "published_at": null,
  "language": "en",
  "version": 1,
  "previous_capture_id": null,
  "recapture_count": 0
}
```

`capture_type` is one of `conversation | article | repository | issue |
pull_request | discussion | code | page`; `fidelity` is `structured |
generic | text_only`; `coverage` is `full_document | rendered_dom | partial |
unknown`. See `docs/capture.md` §6 for what each coverage value is allowed to
claim.

The raw payload under `original/` is the wire format shared with the browser
extension (`native/src/webcapture/types.ts` ↔
`native/src-tauri/src/capture/web/mod.rs`). It is text-only by construction —
no field in it can carry HTML — and is never rewritten, which is what lets
`renormalize_capture` rebuild the markdown from it after normalization
improves.

## 7. Derived Data Schema

Analysis output is stored **beside** a source artifact, never inside it. The
source record (`metadata.json`) is what Relay captured; derived data is what
Relay concluded, and re-analysing must never be able to rewrite the former.

```text
.relay/vault/captures/<source_id>/     (or vault/files/<source_id>/)
  metadata.json                        # the VaultFile record — source truth
  original/<Sanitized-Title>.json      # raw payload, written once
  context.json                         # SourceContext, the read path for the UI
  derived/
    summary.json                       # DerivedData { derived_type: "summary" }
    context.json                        # DerivedData { derived_type: "context" }
    enrichment.json                    # DerivedData { derived_type: "enrichment" }
```

Every derived record names the source it came from and how it was produced
(`native/src-tauri/src/pipeline/analysis/derived.rs`):

```json
{
  "id": "cap_2b1f::context",
  "source_id": "cap_2b1f",
  "derived_type": "context",
  "version": 2,
  "created_at": "2026-09-03T10:04:11Z",
  "updated_at": "2026-09-04T08:15:00Z",
  "analysis": {
    "analysis_type": "context",
    "status": "succeeded",
    "prompt_id": "repository.context",
    "prompt_version": 1,
    "provider": "ollama",
    "model": "llama3.2:latest",
    "deterministic": false,
    "source_coverage": "partial",
    "generated_at": "2026-09-04T08:15:00Z",
    "prompt_tokens": 4210,
    "completion_tokens": 812
  },
  "payload_kind": "structured",
  "payload": { "...": "the SourceContext, or summary text for a summary" }
}
```

`status` is the field that carries Relay's trust model, and its three
meaningful values are not interchangeable:

| Status | Meaning |
|---|---|
| `succeeded` | A model answered and the answer validated against the contract. |
| `insufficient_evidence` | The analysis ran correctly and the source did not carry what was asked for. A successful, honest outcome — including when a deterministic fallback produced the payload. |
| `failed` | No usable answer: no model responded, or the response could not be validated. |

`deterministic: true` means no model read the source and a pattern-matching
fallback produced the payload. Such a record never names a `model`, because
none answered.

`prompt_id` and `prompt_version` come from the registry in
`pipeline/analysis/prompts.rs`. They are recorded so that derived data
produced under an older prompt cannot silently claim to be the output of a
newer one.

### Regeneration policy

Re-analysis **replaces** the record for a given `(source_id, derived_type)`
pair and increments its `version`. Relay keeps the latest derived
representation, not a history — the same behaviour `context.json` always had,
now stated and applied consistently. The source is never modified, so any
analysis can be run again.

### Backward compatibility

`context.json` remains the read path for the Captures UI and is written for
every capture, including vaults that predate this layout. A `context.json` in
the pre-v0.28.5 shape (a bare `ConversationContext` with no `kind` envelope)
still deserializes, and gets `analysis: null` — no metadata is invented for a
context produced before the contract existed. The semantic fields on
`VaultFile` (`summary`, `tags`, `topics`, `entities`, `ai_metadata`) are still
written and read; `derived/` is populated alongside them rather than instead
of them.

## 8. Capture Settings

Part of `settings.json` (§4), under `capture`:

```json
{
  "capture": {
    "bridge_enabled": false,
    "bridge_port": 8765,
    "pairing_token": null,
    "analyze_on_capture": true
  }
}
```

Off by default: capture needs a paired browser extension before it can do
anything, so a fresh install opens no listening socket. `pairing_token` is a
256-bit hex secret generated when capture is first enabled. `hotkeys` also
gains `capture_hotkey` (default `Ctrl+Shift+C`), which opens the Captures
surface — reading a page is triggered from inside the browser, for the reason
given in `docs/capture.md` §1.
