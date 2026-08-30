# Relay — Roadmap & Competitive Gap Backlog

This tracks what's real vs. stubbed today, and what competitive research
suggests should come next — the meeting-notetaker teardown that informs it is
`Meeting-rules/meeting_notes_competitive_teardown.md`. It exists so "not implemented yet" is written down explicitly
instead of silently implied by absence — see `rules/version-and-changelog.md`'s
spirit of keeping the living spec honest.

## Shipped this round (real, not mocked)
- Real microphone capture (`cpal`, resampled to 16kHz mono) — previously wrote an empty WAV placeholder.
- Real local transcription (`whisper-rs` / whisper.cpp) — previously returned a hardcoded fake transcript string.
- Global show/hide hotkey and push-to-talk universal dictation with OS-wide text injection (`enigo`) and a listening indicator window.
- In-app voice chat: record → transcribe → vault-grounded answer with sources → optional local TTS speak-back (Piper).
- Persisted settings (provider/STT/TTS/hotkeys) — previously the settings UI didn't call the backend at all.
- Keyword-ranked note retrieval (`VaultManager::search_notes`) as real (if simple) grounding for voice chat.

## Still stubbed / not real yet — prioritized backlog

1. **Embedded vector RAG (LanceDB)** — `docs/decisions.md` Decision 6 already
   commits to this; today's `search_notes` is plain keyword/term-overlap
   scoring, not embeddings. This is the highest-priority follow-up since
   voice chat and future semantic search depend on retrieval quality.
   Competitive research flags LightRAG as the cheaper alternative to full
   GraphRAG if/when note volume grows past ~1K documents.

2. **Real MCP client wiring** — `McpRouter::dispatch_action` (`native/src-tauri/src/mcp/mod.rs`)
   returns hardcoded success strings for every action type; no MCP server is
   actually called. Decision 8 names the three servers to reuse as-is
   (`nspady/google-calendar-mcp`, `makenotion/notion-mcp-server`,
   `isaacphi/mcp-gdrive`) — none are wired up yet. This is the "live external
   connectors" gap relative to Onyx-style tools; Calendar is the MVP target,
   Notion/Drive are explicitly Post-MVP per `docs/product.md`.

3. **Meeting speaker diarization** — Relay transcribes meetings as a single
   stream with no per-speaker attribution (OpenWhispr and Screenpipe both do
   this; Relay's data model has no `speaker` field anywhere). Whisper-only
   diarization requires a separate model/pass (e.g. pyannote or whisper.cpp's
   experimental tinydiarize) — not started.

4. **Multi-user / team features** — explicitly flagged in `docs/decisions.md`
   as "noted for later, not decided," and the IDE Build Prompt calls scope
   creep toward this a named product risk. Decision 12's Supabase-based real
   auth is a reasonable foundation to extend into this later, but no sharing
   model, permissions, or shared-vault schema exists.

5. **Continuous background capture** — structurally excluded on purpose
   (Decision 5 — PTT/on-screen-triggered capture only, no meeting-bot or
   always-on recording). Not a gap to close; listed here only so it isn't
   mistaken for an oversight.

## Explicitly not gaps (already real)
- Global hotkey, universal dictation, in-app voice chat, and local TTS —
  all shipped this round (see above), not stubs.
- Configurable trigger-phrase matching (`TriggerEngine`) — real, just its
  downstream MCP dispatch (item 2 above) is the stub.
