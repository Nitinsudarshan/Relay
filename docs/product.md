# Relay — Product Specification

## Overview
Relay is a hybrid (local + cloud) AI voice and memory assistant that converts captured speech into structured, actionable system state — Kanban task cards, calendar events, reminders, and polished document markdown notes — eliminating manual data re-entry.

## Target User
The primary user is a builder or power user with a meeting-heavy and task-heavy workflow who needs instant voice capture, automated task extraction, audio scribble structuring, and configurable voice shortcuts without relying on cloud subscriptions or intrusive meeting bots.

## Core Value Proposition & Competitive Differentiators
1. **Bot-Free Voice Capture**: Operates strictly via push-to-talk (PTT) and local system audio, avoiding the legal, platform, and privacy issues of third-party meeting bots.
2. **Transcript-to-Kanban**: Automatically parses meeting transcripts into actionable, structured task cards formatted for a Kanban workflow, rather than generating wall-of-text summaries.
3. **Voice Scribble to Structured Output**: Transforms rambling audio notes into polished markdown templates (executive summaries, key decisions, action points).
4. **User-Customizable Trigger Phrases**: Allows users to configure arbitrary phrase-to-action mappings (e.g., "Schedule quick sync" -> Calendar MCP call; "Remind me in 2 hours" -> Local OS notification).
5. **Local-First with Zero Recurring Cost**: Runs 100% locally by default using local STT (Parakeet/Whisper), Ollama, and embedded LanceDB vector RAG over an Obsidian-style markdown vault.
6. **Dual Surface (Native Desktop + Web Dashboard)**: Windows native app for local capture and processing; Next.js web client for remote cloud access in hybrid mode.

## In Scope for MVP
- Push-to-talk capture with floating overlay widget & global hotkey.
- Local Speech-to-Text (Parakeet / Whisper) & local Ollama LLM provider toggle (with cloud LLM option).
- Meeting transcript -> Kanban list parser (list-to-board rendering).
- Audio scribble -> structured prompt engine (templates).
- Obsidian-style Markdown Vault storage + embedded LanceDB vector search.
- Configurable Trigger Phrase Engine mapped to MCP actions (Google Calendar, reminders).
- Next.js Web Dashboard + Supabase cloud auth/storage for hybrid mode.

## Out of Scope for MVP
- Drag-and-drop Kanban card persistence (list-to-board only).
- GraphRAG / Knowledge graph retrieval.
- Third-party meeting-bot joiners.
- Mobile native application.
- Multi-user / team shared vaults.
