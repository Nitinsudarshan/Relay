# Relay — User Flows

## Flow 1: Push-to-Talk Capture to Kanban Card
1. User presses and holds global hotkey (or clicks and holds floating PTT widget).
2. PTT widget changes visual state to "Recording..." with audio level animation.
3. User speaks meeting notes or task items and releases hotkey.
4. Audio recording stops; WAV file is sent to local STT engine (Whisper/Parakeet).
5. STT returns raw transcript.
6. Pipeline module routes transcript to `Meeting -> Kanban` LLM prompt.
7. LLM extracts array of structured action items.
8. Vault module writes each item as a markdown card in `.relay/vault/kanban/` (real-time indexing into an embedded vector store is decided but not yet built — see `docs/roadmap.md`; notes are searchable today via keyword-ranked retrieval).
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

## Flow 5: Global Show/Hide Hotkey
1. User presses `Ctrl+Shift+Space` (configurable) while any other application is focused.
2. The OS-level `tauri-plugin-global-shortcut` hotkey fires regardless of which window has focus.
3. Relay's main window is shown and given focus if it was hidden, or hidden if it was already visible.

## Flow 6: Universal Dictation (Type Anywhere)
1. User places their text cursor in any other application (email, Slack, an IDE, a browser form field).
2. User presses and holds `Ctrl+Space` (configurable).
3. A small always-on-top, non-focus-stealing "🎙 Listening…" indicator appears; Relay's own window never needs to be visible. Audio capture starts immediately.
4. User speaks and releases the hotkey; the indicator disappears immediately.
5. Captured audio is transcribed locally (Whisper only — no LLM pipeline, no vault write).
6. The transcript is typed into whatever field currently has OS focus via simulated keystrokes (`enigo`), exactly as if the user had typed it themselves.

## Flow 7: Voice Chat Over Vault Notes
1. User opens the "Voice Chat" tab and clicks the mic button.
2. User asks a question out loud (e.g. "What did we decide about the Kanban schema?") and clicks stop.
3. Audio is transcribed locally.
4. Vault module ranks notes by keyword overlap with the question and returns the top matches as grounding context.
5. The configured LLM provider answers the question using only that grounding context, instructed to say so honestly if the notes don't contain the answer.
6. The answer is shown in the chat thread along with the source note titles used.
7. If a local Piper TTS binary and voice model are configured, the answer is also synthesized as audio and played back automatically; otherwise the response is text-only.
