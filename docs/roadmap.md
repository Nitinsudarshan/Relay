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

3. **Speaker identity across meetings** — diarization itself shipped in
   0.31.0: `meetings_v2::diarize` clusters the recorded audio into distinct
   voices, so a call with several remote participants reads as several
   speakers. It uses MFCC statistics and a pitch estimate rather than a neural
   embedding, which is a deliberate trade — no ONNX runtime, no model download,
   and no biometric data, against not being able to tell two similar voices
   apart on one channel or recognise a voice in a later meeting.
   `DiarizationReport::well_separated` reports when a roster should not be
   trusted. The remaining gaps are the voiceprint library (rung 2 of
   `Meeting-rules/meeting_speaker_identification.md`, and the feature that
   would create biometric data — `maybe_later.md` item 18) and calendar
   attendees (rung 3 — `maybe_later.md` item 19).

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
- Multi-speaker attribution in meetings — real since 0.31.0, with its own
  confidence reporting. See item 3 for what remains.
- Hallucination screening on transcripts — real since 0.31.0. Every chunk is
  gated on measured voiced time before decoding and screened for decoder loops
  and subtitle filler after; a rejected chunk records what was discarded and
  why. `docs/meetings/TRANSCRIPT_AND_SPEAKER_REBUILD.md` has the trace.
- Configurable trigger-phrase matching (`TriggerEngine`) — real, just its
  downstream MCP dispatch (item 2 above) is the stub.
