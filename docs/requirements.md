# Relay — Functional & Non-Functional Requirements

## Functional Requirements

### 1. Audio Capture & Speech-to-Text (STT)
- **FR-1.1**: The system MUST support Push-to-Talk (PTT) capture via a configurable global hotkey and on-screen floating widget button.
- **FR-1.2**: The system MUST record audio from the default system microphone using WAV format (16kHz mono).
- **FR-1.3**: The system MUST transcribe recorded audio using a local Whisper model (`whisper-rs`) or a cloud STT API fallback when configured. A GGML model path is user-configured in settings; if unset, capture/dictation/chat surface a clear "no model configured" error rather than silently returning fake text.
- **FR-1.4 (Global Show/Hide Hotkey)**: The system MUST register a configurable OS-wide hotkey (default `Ctrl+Shift+Space`) that toggles the main window's visibility and focus from any other application.
- **FR-1.5 (Universal Dictation)**: The system MUST register a configurable OS-wide push-to-talk hotkey (default `Ctrl+Space`) that, while held, records audio and shows a small always-on-top, non-focus-stealing "listening" indicator; on release, the audio is transcribed (no LLM pipeline) and the resulting text is typed into whichever field currently has OS focus — not necessarily Relay's own window.

### 2. Processing Pipeline
- **FR-2.1 (Meeting -> Kanban)**: The system MUST process meeting transcripts using an LLM prompt to extract structured action items containing `title`, `assignee`, `status` (`todo`, `in_progress`, `done`), `due_date`, and `description`. *(Deferred for the current desktop-first MVP phase — see `docs/decisions.md` Decision 33. Pipeline code preserved, not reachable from the active UI.)*
- **FR-2.2 (Scribble -> Structured Output)**: The system MUST process rambling audio scribbles using prompt templates to produce structured markdown notes containing summary, key decisions, and follow-ups.
- **FR-2.3 (Trigger Phrase Engine)**: The system MUST classify transcribed text against user-defined trigger phrase mappings and route matching intents to specified actions (e.g. MCP Calendar entry, Local OS notification). *(Deferred for the current desktop-first MVP phase — see `docs/decisions.md` Decision 35. `TriggerEngine`/`McpRouter` preserved, not invoked from the active capture path.)*
- **FR-2.4 (Voice Chat / Vault Q&A)**: The system MUST support recording a spoken question, transcribing it, retrieving the most relevant vault notes as grounding context, and returning an LLM answer with the source note titles shown. Trigger-phrase matching MUST be skipped for this mode. If a local TTS engine is configured, the answer MUST additionally be synthesized as audio and returned for playback; if not configured, the feature degrades to text-only rather than failing the request. *(Deferred for the current desktop-first MVP phase — see `docs/decisions.md` Decision 34. Pipeline and TTS code preserved, not reachable from the active UI.)*

### 2.5 Web Capture
- **FR-2.5.1 (Structured Web Capture)**: The system MUST be able to save the web page or conversation a user is looking at as structured text — conversation turns with role attribution, or document blocks (headings, paragraphs, lists, code, quotes, tables, images) — rather than as a screenshot. Semantic extraction is the primary mechanism; there is no screenshot or OCR path (`maybe_later.md` §14).
- **FR-2.5.2 (Provenance)**: Every capture MUST record `source_type`, `application`, `domain`, `url`, `page_title`, `captured_at`, `capture_type`, the extractor that produced it, and how the content was obtained. Provenance MUST NOT be mixed with semantic metadata (tags, topics, entities, summary), and MUST be derived by the backend from the URL rather than taken from the payload.
- **FR-2.5.3 (Honest Completeness)**: Every capture MUST state how much of the page it contains, and MUST NOT claim a full document without positive evidence. Limitations MUST be recorded in plain language on the artifact and surfaced in the UI.
- **FR-2.5.4 (Acquisition Before Interpretation)**: A capture MUST be persisted, with its raw source payload preserved, before any AI runs on it. A failed analysis MUST NOT lose or alter the captured content.
- **FR-2.5.5 (Least Privilege)**: Browser capture MUST use `activeTab` — access granted per-tab in response to an explicit in-browser gesture — and MUST NOT request standing access to all sites. Captured content MUST travel from the browser to Relay on the same machine, never through a third-party server.
- **FR-2.5.6 (Untrusted Input)**: Captured page content MUST be treated as untrusted: never executed, never stored or rendered as HTML, with control and text-direction characters removed and non-`http(s)` link targets dropped.
- **FR-2.5.7 (Re-capture)**: Capturing the same URL with unchanged content MUST NOT create a duplicate artifact. Capturing it with changed content MUST create a new version that references the one it supersedes.

### 3. Storage & Vault
- **FR-3.1 (Markdown Vault)**: The system MUST save all processed outputs as Obsidian-compatible Markdown files with YAML frontmatter headers in a local directory (`.relay/vault`).
- **FR-3.2 (Vector Store)**: The system MUST automatically generate text embeddings and store them in an embedded LanceDB vector database for semantic search over historical notes.

### 4. Provider Abstraction
- **FR-4.1**: The backend MUST support swapping between local Ollama LLM models and cloud APIs (OpenAI, Gemini, Anthropic) via a unified settings configuration.

### 5. Dual Surfaces
- **FR-5.1 (Native Desktop)**: The Windows app MUST render a responsive floating PTT widget, Kanban board viewer, voice chat panel, trigger phrase config UI, and provider/STT/TTS/hotkey settings. *(The Kanban board viewer, voice chat panel, and trigger phrase config UI are deferred for the current phase — see Decisions 33–35; implementations preserved, nav entries removed.)*
- **FR-5.2 (Web Dashboard)**: The Next.js web application MUST allow authentication and viewing/managing synced vault notes and Kanban cards in hybrid mode. *(Deferred for the current desktop-first MVP phase — see `docs/decisions.md` Decision 32. Requirement preserved for when hybrid mode resumes; not part of the active build target.)*

### 6. Notifications & Windows Integration
- **FR-6.1 (Native OS Notifications for Meetings)**: The system MUST present transient meeting notifications (upcoming, unrecorded, detected) using native Windows OS Toast Notifications (`tauri_plugin_notification`). The system MUST NOT create custom Tauri WebView windows or floating webview containers for transient meeting reminders.

## Non-Functional Requirements

- **NFR-1 (Cost)**: Baseline operation MUST require $0 recurring cloud cost (local-first).
- **NFR-2 (Performance)**: Audio capture start latency MUST be <100ms. Local transcription and processing MUST execute asynchronously without blocking UI threads.
- **NFR-3 (Privacy & Security)**: Audio data and markdown vaults MUST remain stored locally by default. No cloud transmission occurs unless hybrid mode is enabled by the user.
