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

## Flow 8: Native OS Meeting Notification Alerts
1. Meeting engine detects an upcoming, unrecorded, or active meeting.
2. Rust reminder engine transitions reminder state to `Fired`.
3. System emits a native Windows OS Toast Notification (`app.notification()`).
4. Windows displays the native toast notification without opening any Tauri WebView window.
5. User clicks notification or action -> Relay main window opens and focuses relevant meeting context.

## Flow 9: Capturing a Web Page or AI Conversation
1. One-time setup: the user builds the extension (`npm --prefix native run build:extension`), loads `native/browser-extension` unpacked, turns **Browser capture** on in Relay's Capture settings, and pastes the port and pairing token into the extension's Options.
2. On any page, the user presses the extension's shortcut (`Ctrl+Shift+Y`) or clicks the Relay toolbar button. That gesture is what grants the extension access to this one tab — nothing before it did.
3. The extension injects its extractor into the tab, which reads the rendered document: a site-specific extractor for ChatGPT, Claude or GitHub; otherwise the generic article extractor; otherwise the page's visible text. The toolbar badge shows `…`.
4. The extractor returns a structured, text-only payload — blocks or conversation turns, plus `<head>` metadata and a coverage verdict saying how much of the page it could honestly claim. Nothing is sent if there was nothing readable; the badge shows `✕` with the reason.
5. The extension posts the payload to `http://127.0.0.1:<port>/v1/capture` with the pairing token in a header. Relay checks the origin, the token and the size before reading the body.
6. Relay derives provenance from the URL itself — application, domain, capture type — never from what the payload claimed. It then sanitizes every string, normalizes into markdown, and writes the artifact **and the raw payload** into `.relay/vault/captures/<id>/`. The badge shows `✓`, and Relay's Captures surface shows *Saved*.
7. Only now does analysis run, in the background: a summary, topics and entities, using the configured provider. If it fails, the surface says the capture is intact and offers *Analyse* again — the captured content is never at risk.
8. The user can open the capture to read it, see exactly where it came from and what was left out, read the raw stored payload, or add it to Scribbles — which is what carries it into search and the knowledge graph. It is searchable by Talkback either way.
