# Relay — Product Specification

## Overview
Relay is a hybrid (local + cloud) AI voice and memory assistant that converts captured speech into structured, actionable system state — Kanban task cards, calendar events, reminders, and polished document markdown notes — eliminating manual data re-entry.

## Target User
The primary user is a builder or power user with a meeting-heavy and task-heavy workflow who needs instant voice capture, automated task extraction, audio scribble structuring, and configurable voice shortcuts without relying on cloud subscriptions or intrusive meeting bots.

## Core Value Proposition & Competitive Differentiators
1. **Bot-Free Voice Capture**: Operates strictly via push-to-talk (PTT) and local system audio, avoiding the legal, platform, and privacy issues of third-party meeting bots.
2. **Transcript-to-Kanban**: Automatically parses meeting transcripts into actionable, structured task cards formatted for a Kanban workflow, rather than generating wall-of-text summaries.
3. **Voice Scribble to Structured Output**: Transforms rambling audio notes into polished markdown templates (executive summaries, key decisions, action points).
4. **User-Customizable Trigger Phrases**: Allows users to configure arbitrary phrase-to-action mappings (e.g., "Schedule quick sync" -> Calendar MCP call; "Remind me in 2 hours" -> Local OS notification). *(Deferred for the current desktop-first MVP phase — see `docs/decisions.md` Decision 35.)*
5. **Local-First with Zero Recurring Cost**: Runs 100% locally by default using local STT (Whisper, via `whisper-rs`), Ollama, and grounded retrieval over an Obsidian-style markdown vault.
6. **Dual Surface (Native Desktop + Web Dashboard)**: Windows native app for local capture and processing; Next.js web client for remote cloud access in hybrid mode. *(Deferred for the current desktop-first MVP phase — see `docs/decisions.md` Decision 32.)*
7. **Universal Dictation, Not Just In-App Voice**: A global push-to-talk hotkey types transcribed speech directly into whatever app or field currently has OS focus (Slack, email, code editors) — not confined to Relay's own window — with a non-intrusive "listening" indicator. A separate global hotkey shows/hides Relay from anywhere in the OS.
8. **Voice Chat Grounded in Your Own Notes**: Ask a question out loud inside Relay; it retrieves relevant vault notes, answers grounded in them with sources shown, and can speak the answer back via local TTS. *(Deferred for the current desktop-first MVP phase — see `docs/decisions.md` Decision 34.)*

## In Scope for MVP
- Push-to-talk capture with floating overlay widget & global hotkey.
- Local Speech-to-Text (Whisper via `whisper-rs`) & local Ollama LLM provider toggle (with cloud LLM option).
- Meeting transcript -> Kanban list parser (list-to-board rendering).
- Audio scribble -> structured prompt engine (templates).
- Obsidian-style Markdown Vault storage + keyword-ranked note retrieval (embedded LanceDB vector search is decided but not yet built — see `docs/roadmap.md`).
- Global show/hide hotkey and push-to-talk universal dictation (types into whatever app/field has OS focus) with a listening indicator.

## Out of Scope for MVP
- Drag-and-drop Kanban card persistence (list-to-board only).
- GraphRAG / Knowledge graph retrieval.
- Third-party meeting-bot joiners.
- Mobile native application.
- Multi-user / team shared vaults.
- Meeting speaker diarization (transcription only, no speaker attribution).
- Live external connectors beyond the Calendar/local-reminder trigger actions (Notion/Drive push is decided architecturally but not wired to a real MCP client yet).
- Continuous/always-on background capture (structurally excluded — see `docs/decisions.md` Decision 5).

## Deferred for Current Phase
A desktop-first scope reduction (in progress — see `docs/decisions.md` Decision 32 onward) is narrowing the *active* MVP to: global PTT + click-to-talk + local STT + text injection through one Dictation Pill. Items below remain fully implemented in the repository but are no longer part of the active product surface for this phase. This list grows only as each further scope-reduction step is explicitly approved and completed — `docs/decisions.md` is the authoritative, dated record of what changed and what didn't.

- **Web / Hybrid Dashboard** (Decision 32): the Next.js web client (`web/`) and Supabase-backed hybrid sync. Preserved as-is; no desktop functionality depended on it.
- **Kanban Board** (Decision 33): the meeting-transcript-to-task-board UI and its `process_meeting` pipeline. `KanbanBoard.tsx`, the `KanbanCard` type, and the backend read path are preserved untouched; only the sidebar nav entry and an unconditional startup fetch were removed.
- **Voice Chat & TTS** (Decision 34): the vault-grounded voice Q&A tab and its optional local "speak back" (Piper). `ChatPanel.tsx`, `process_chat`, `TtsEngine`, and the shared LLM provider client are preserved untouched; only the nav entry and the Piper settings block were removed.
- **Triggers & MCP** (Decision 35): the configurable trigger-phrase engine and its MCP action dispatch (calendar/reminders). `TriggerEngine`, `McpRouter`, and the trigger config commands are preserved untouched; both nav entries and the inline dispatch call inside the core capture path were removed — the latter because it ran automatically on every non-chat capture, not only when explicitly configured.
