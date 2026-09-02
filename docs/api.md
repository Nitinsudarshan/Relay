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
- `stop_capture() -> Result<Option<ProcessedPipelineResult>, CommandError>`
  Stops audio capture. `AudioRecorder::stop` reports `had_audio: bool` — whether any captured chunk actually crossed the mic input threshold while the session was recording — and this command gates on it: if `had_audio` is `false` (silence/no input the whole time), it emits a `capture-state-changed` event with `status: "NO_SPEECH"` and returns `Ok(None)` **without ever invoking the STT engine**. Only when `had_audio` is `true` does it emit `status: "TRANSCRIBING"` and proceed to transcribe via the configured local Whisper model, then:
  - for `"meeting"`/`"scribble"`, first checks trigger phrases (returning mode `"trigger"` on a match), then runs the meeting->Kanban or scribble->structured-note pipeline;
  - for `"chat"`, skips trigger matching and runs `pipeline::process_chat` (vault-grounded Q&A with optional spoken answer) instead.

  `ProcessedPipelineResult` additionally carries `sources: string[]` (vault note titles used as grounding, populated for chat) and `spoken_audio_base64?: string | null` (base64 WAV of the answer, if local TTS is configured).

  Universal dictation (the global push-to-talk hotkey) does **not** go through these commands — it calls `AudioRecorder`/`SttEngine` directly from the Rust hotkey handler and injects the transcript via OS keystroke simulation, bypassing Tauri IPC entirely (see `hotkeys::on_dictation_released`, which applies the same `had_audio` gate before transcribing).

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

#### Web Capture Commands
Web capture (`docs/capture.md`) — distinct from the audio capture commands
above. Acquisition is separate from interpretation:
every command below either stores or reads a capture; none of them can lose one.

- `get_capture_bridge_status() -> Result<CaptureBridgeStatus, CommandError>`
  Whether browser capture is enabled, whether a loopback listener is actually
  bound, the port in use (which differs from the configured one when that port
  was taken), the pairing token, and the desktop capture hotkey.
- `set_capture_bridge_enabled(enabled: bool) -> Result<CaptureBridgeStatus, CommandError>`
  Starts or stops the listener and persists the choice, generating a pairing
  token the first time it is switched on. Errors with
  `CAPTURE_BRIDGE_START_FAILED` if no loopback port could be bound.
- `set_capture_bridge_port(port: u16) -> Result<CaptureBridgeStatus, CommandError>`
  Rebinds on a new port. Rejects ports below 1024 with `INVALID_PORT`.
- `regenerate_capture_pairing_token() -> Result<CaptureBridgeStatus, CommandError>`
  Issues a new token, immediately unpairing every browser.
- `set_capture_analyze_on_capture(enabled: bool) -> Result<CaptureBridgeStatus, CommandError>`
  Whether Relay analyses a capture as soon as it lands. Storage never depends
  on this.
- `get_captures() -> Result<Vec<VaultFile>, CommandError>`
  Captures only, newest first. Imported documents stay on `get_vault_files`.
- `get_capture(id: String) -> Result<VaultFile, CommandError>`
- `get_capture_payload(id: String) -> Result<WebCapturePayload, CommandError>`
  The raw structured payload as captured — written once, never rewritten.
- `renormalize_capture(id: String) -> Result<VaultFile, CommandError>`
  Rebuilds the markdown from that payload, preserving id, capture time and
  version history.
- `delete_capture(id: String) -> Result<(), CommandError>` Moves it to Trash.
- `import_web_capture(payload_json: String) -> Result<VaultFile, CommandError>`
  Ingests a payload handed over inside the app rather than over the bridge.

`analyze_vault_file`, `summarize_vault_file` and `create_scribble_from_vault_file`
accept a capture id as well as a file id — a capture is a Vault artifact, and
is analysed and promoted by the same code paths. `reprocess_vault_file` is a
no-op for a capture: its text was normalized from a payload, not extracted
from bytes, so re-running document extraction on it could only destroy it.

Captures also arrive over a loopback HTTP bridge rather than through Tauri
IPC (`POST http://127.0.0.1:<port>/v1/capture`, `X-Relay-Token` required).
That surface's contract, limits and threat model are in `docs/capture.md` §5.

Progress is broadcast on the `capture-progress` event with a `stage` of
`SAVING | SAVED | ANALYSING | ANALYSED | FAILED`.

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
