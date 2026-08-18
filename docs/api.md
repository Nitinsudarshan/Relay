# Relay — API Conventions & Specifications

Per `rules/api-conventions.md`, all IPC commands and web API route handlers return consistent, typed error responses.

## 1. Tauri Commands API (`native/src-tauri/src/commands.rs`)

### Common Response Shape
All Rust Tauri commands return `Result<T, CommandError>` where `CommandError` serializes to:
```typescript
interface CommandError {
  code: string;     // e.g. "CAPTURE_FAILED", "STT_ERROR", "PIPELINE_ERROR"
  message: string;  // User-facing descriptive message
}
```

### Command Signatures

#### Capture Commands
- `start_capture(mode: String) -> Result<String, CommandError>`
  Starts audio capture for mode `"meeting" | "scribble" | "trigger"`. Returns capture session ID.
- `stop_capture() -> Result<ProcessResult, CommandError>`
  Stops audio capture, transcribes, runs pipeline, and returns processing result.

#### Pipeline Commands
- `process_transcript(transcript: String, mode: String) -> Result<ProcessResult, CommandError>`
  Manually triggers pipeline processing on raw transcript text.

#### Kanban Commands
- `get_kanban_cards() -> Result<Vec<KanbanCard>, CommandError>`
  Reads all Kanban markdown card files from vault.
- `update_card_status(card_id: String, status: String) -> Result<KanbanCard, CommandError>`
  Updates frontmatter status in card file.

#### Trigger Settings Commands
- `get_triggers() -> Result<Vec<TriggerConfig>, CommandError>`
- `save_trigger(trigger: TriggerConfig) -> Result<Vec<TriggerConfig>, CommandError>`
- `delete_trigger(trigger_id: String) -> Result<Vec<TriggerConfig>, CommandError>`

#### Provider Settings Commands
- `get_settings() -> Result<AppSettings, CommandError>`
- `save_settings(settings: AppSettings) -> Result<AppSettings, CommandError>`

---

## 2. Web Route Handlers (`web/src/app/api/...`)

Standard JSON format:
```typescript
interface ApiResponse<T> {
  success: boolean;
  data?: T;
  error?: string;
}
```

### Routes
- `GET /api/kanban`: Returns synced Kanban tasks for authenticated user.
- `PATCH /api/kanban/[id]`: Updates task status or attributes.
- `GET /api/notes`: Returns user markdown note metadata.
