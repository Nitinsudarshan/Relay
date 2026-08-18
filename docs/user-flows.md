# Relay — User Flows

## Flow 1: Push-to-Talk Capture to Kanban Card
1. User presses and holds global hotkey (or clicks and holds floating PTT widget).
2. PTT widget changes visual state to "Recording..." with audio level animation.
3. User speaks meeting notes or task items and releases hotkey.
4. Audio recording stops; WAV file is sent to local STT engine (Whisper/Parakeet).
5. STT returns raw transcript.
6. Pipeline module routes transcript to `Meeting -> Kanban` LLM prompt.
7. LLM extracts array of structured action items.
8. Vault module writes each item as a markdown card in `.relay/vault/kanban/` and updates LanceDB vector index.
9. Native UI updates Kanban Board view immediately.

## Flow 2: Audio Scribble to Structured Markdown Note
1. User selects "Scribble Mode" on floating widget and records audio thoughts.
2. STT engine generates transcript.
3. Pipeline module applies `Scribble -> Structured Output` LLM template.
4. LLM outputs formatted markdown document with title, summary bullet points, and key takeaways.
5. Note is saved to `.relay/vault/notes/` and indexed in LanceDB.
6. Note preview modal opens in native app for quick review/edit.

## Flow 3: Configurable Trigger Phrase Execution
1. User configures a custom trigger: `"Schedule meeting"` -> mapped to `Google Calendar MCP (create_event)`.
2. User captures voice: `"Schedule meeting with Design Team tomorrow at 3pm"`.
3. Intent classifier identifies trigger match `"Schedule meeting"`.
4. Trigger engine extracts arguments (`title: Design Team`, `time: tomorrow 3pm`) and executes MCP tool call.
5. OS notification displays confirmation: `"Calendar event created for Design Team tomorrow at 3pm"`.

## Flow 4: Hybrid Mode Web Dashboard Access
1. User opens Relay web dashboard (`https://relay.app` or local web server).
2. User logs in with Supabase auth credentials.
3. Dashboard fetches synced Kanban cards and vault notes from Supabase database.
4. User views and updates task card statuses on web Kanban board.
