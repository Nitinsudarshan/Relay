# Relay — Functional & Non-Functional Requirements

## Functional Requirements

### 1. Audio Capture & Speech-to-Text (STT)
- **FR-1.1**: The system MUST support Push-to-Talk (PTT) capture via a configurable global hotkey and on-screen floating widget button.
- **FR-1.2**: The system MUST record audio from the default system microphone using WAV format (16kHz mono).
- **FR-1.3**: The system MUST transcribe recorded audio using a local Whisper/Parakeet model or a cloud STT API fallback when configured.

### 2. Processing Pipeline
- **FR-2.1 (Meeting -> Kanban)**: The system MUST process meeting transcripts using an LLM prompt to extract structured action items containing `title`, `assignee`, `status` (`todo`, `in_progress`, `done`), `due_date`, and `description`.
- **FR-2.2 (Scribble -> Structured Output)**: The system MUST process rambling audio scribbles using prompt templates to produce structured markdown notes containing summary, key decisions, and follow-ups.
- **FR-2.3 (Trigger Phrase Engine)**: The system MUST classify transcribed text against user-defined trigger phrase mappings and route matching intents to specified actions (e.g. MCP Calendar entry, Local OS notification).

### 3. Storage & Vault
- **FR-3.1 (Markdown Vault)**: The system MUST save all processed outputs as Obsidian-compatible Markdown files with YAML frontmatter headers in a local directory (`.relay/vault`).
- **FR-3.2 (Vector Store)**: The system MUST automatically generate text embeddings and store them in an embedded LanceDB vector database for semantic search over historical notes.

### 4. Provider Abstraction
- **FR-4.1**: The backend MUST support swapping between local Ollama LLM models and cloud APIs (OpenAI, Gemini, Anthropic) via a unified settings configuration.

### 5. Dual Surfaces
- **FR-5.1 (Native Desktop)**: The Windows app MUST render a responsive floating PTT widget, Kanban board viewer, note browser, trigger phrase config UI, and provider settings.
- **FR-5.2 (Web Dashboard)**: The Next.js web application MUST allow authentication and viewing/managing synced vault notes and Kanban cards in hybrid mode.

## Non-Functional Requirements

- **NFR-1 (Cost)**: Baseline operation MUST require $0 recurring cloud cost (local-first).
- **NFR-2 (Performance)**: Audio capture start latency MUST be <100ms. Local transcription and processing MUST execute asynchronously without blocking UI threads.
- **NFR-3 (Privacy & Security)**: Audio data and markdown vaults MUST remain stored locally by default. No cloud transmission occurs unless hybrid mode is enabled by the user.
