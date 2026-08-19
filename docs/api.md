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
  Starts audio capture for mode `"meeting" | "scribble" | "chat"`. Returns capture session ID.
- `stop_capture() -> Result<ProcessedPipelineResult, CommandError>`
  Stops audio capture, transcribes via the configured local Whisper model, and:
  - for `"meeting"`/`"scribble"`, first checks trigger phrases (returning mode `"trigger"` on a match), then runs the meeting->Kanban or scribble->structured-note pipeline;
  - for `"chat"`, skips trigger matching and runs `pipeline::process_chat` (vault-grounded Q&A with optional spoken answer) instead.

  `ProcessedPipelineResult` additionally carries `sources: string[]` (vault note titles used as grounding, populated for chat) and `spoken_audio_base64?: string | null` (base64 WAV of the answer, if local TTS is configured).

  Universal dictation (the global push-to-talk hotkey) does **not** go through these commands — it calls `AudioRecorder`/`SttEngine` directly from the Rust hotkey handler and injects the transcript via OS keystroke simulation, bypassing Tauri IPC entirely (see `hotkeys::on_dictation_released`).

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

#### Settings Commands
- `get_settings() -> Result<AppSettings, CommandError>`
  Returns the current provider/STT/TTS/hotkey configuration (see `docs/data-model.md` §4).
- `save_settings(settings: AppSettings) -> Result<(), CommandError>`
  Persists settings to `.relay/config/settings.json` and updates the running app's in-memory config immediately (LLM provider, STT model path, TTS paths). Hotkey changes take effect on next launch — they're only read once at startup.

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
