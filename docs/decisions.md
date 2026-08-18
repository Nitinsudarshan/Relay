# Relay — Architectural & Product Decision Log

This log records material architectural, technical, and product decisions for Relay in the standard format specified by the project rules.

---

### Decision 1: Build Path & Relationship to Mnemos
- **Context**: Need to determine codebase origin and relationship to prior prototypes or references.
- **Decision made**: Build Relay completely from scratch in this brand-new repository. No code copied or forked from Mnemos or Meetily.
- **Reason**: Relay's three-surface architecture (Rust backend + Tauri desktop + Next.js web client) differs fundamental from prior prototypes. Starting clean avoids carrying over technical debt or mismatched paradigms.
- **Alternatives considered**: Forking Mnemos or adapting Meetily.
- **Impact**: Full control over component architecture, type definitions, and backend runtime.

---

### Decision 2: Technology Stack
- **Context**: Choosing backend runtime and native shell for Windows desktop capture and local execution.
- **Decision made**: Rust backend (`native/src-tauri/`) + Tauri 2.0 / React frontend (`native/src/`) for desktop, plus Next.js + Shadcn + Supabase for the web client (`web/`).
- **Reason**: Rust delivers high efficiency, native Windows audio/WASAPI capture capabilities, fast local inference orchestration, low footprint, and security.
- **Alternatives considered**: Python backend (PyInstaller / FastAPI), n8n workflow engine, Electron.
- **Impact**: Accepted learning-curve risk for Rust; zero Electron bloat on Windows.

---

### Decision 3: Hybrid Deployment Includes Web Surface
- **Context**: Primary desktop app vs remote/cloud access.
- **Decision made**: Dual-surface model: Windows native app for primary local capture & processing; web client for hybrid/cloud mode.
- **Reason**: Allows access to notes, Kanban board, and structured outputs from any browser when away from the desktop machine.
- **Alternatives considered**: Desktop-only application; tunnel access directly into Windows desktop.
- **Impact**: Requires shared data representations and synchronization via Supabase in hybrid mode.

---

### Decision 4: Cost Ceiling is a Hard Constraint
- **Context**: Monetization vs cost-to-run for personal/builder usage.
- **Decision made**: Every cloud-optional feature must function fully at $0 recurring cost using local STT (Whisper/Parakeet), Ollama, and local files.
- **Reason**: The user requires a zero-cost local baseline. Paid cloud APIs (OpenAI, Gemini, Claude) are optional toggle overlays.
- **Alternatives considered**: Cloud-first or cloud-only API design.
- **Impact**: Graceful degradation to local-only mode when offline or without API keys.

---

### Decision 5: No Meeting-Bot Architecture
- **Context**: Audio capture methodology for meetings.
- **Decision made**: Use push-to-talk / on-screen capture affordances only; do NOT build meeting bots (Zoom/Teams/Meet bot joiners).
- **Reason**: Structural legal/platform risk (e.g. BIPA lawsuits against bot services, platform restrictions on third-party bots). Local PTT audio capture is legally safe and platform-agnostic.
- **Alternatives considered**: Virtual audio cable recording or headless bot joiners.
- **Impact**: Zero meeting-bot infrastructure required; user triggers capture via hotkey or floating widget.

---

### Decision 6: Retrieval Architecture
- **Context**: Context retrieval over stored vault notes and historical transcripts.
- **Decision made**: Use embedded vector search (LanceDB) over markdown notes for MVP. Graph-based retrieval (GraphRAG) is explicitly deferred post-MVP.
- **Reason**: Vector RAG is fast, lightweight, runs embedded in Rust without separate server processes, and provides excellent results for <10,000 documents.
- **Alternatives considered**: Full Knowledge Graph / GraphRAG, plain keyword search.
- **Impact**: Simple schema with LanceDB tables storing embeddings alongside markdown note file paths.

---

### Decision 7: Kanban Delivery Scope for MVP
- **Context**: Presenting actionable items extracted from meetings.
- **Decision made**: List-to-board rendering for MVP (parsing structured lists of tasks into Kanban columns: To Do, In Progress, Done). Drag-and-drop persistence deferred post-MVP.
- **Reason**: Validates the meeting-to-task parsing pipeline first without getting bogged down in complex drag-and-drop state synchronization across files.
- **Alternatives considered**: Custom drag-and-drop board builder.
- **Impact**: Focuses engineering effort on LLM extraction reliability.

---

### Decision 8: MCP Integrations
- **Context**: External system integrations for Calendar, Notion, and Google Drive.
- **Decision made**: Reuse official/community MCP servers as-is (`nspady/google-calendar-mcp`, `makenotion/notion-mcp-server`, `isaacphi/mcp-gdrive`).
- **Reason**: No custom integration code needed for these 3 services; standard MCP client wiring in Rust handles tool execution.
- **Alternatives considered**: Building direct REST client wrappers for Google / Notion APIs.
- **Impact**: Standardization on Model Context Protocol across all trigger actions.

---

### Decision 9: No Rust-vs-Python Benchmarking Spike
- **Context**: Choice of language for audio and LLM pipeline orchestration.
- **Decision made**: Proceed directly with Rust (`native/src-tauri`). Spike comparing Python vs Rust is cancelled.
- **Reason**: Decision 2 committed to Rust for native desktop integration and memory efficiency.
- **Alternatives considered**: Python backend prototype.
- **Impact**: All backend code written in idiomatic Rust.

---

### Decision 10: Trigger Phrases are User-Customizable
- **Context**: Voice commands mapping to system actions.
- **Decision made**: Build a fully configurable trigger-phrase engine where users define custom phrase -> action mappings in a settings surface.
- **Reason**: Fixed trigger lists restrict utility. Users need tailored phrases (e.g. "Schedule sync", "Remind me to submit report", "Save note to Drive").
- **Alternatives considered**: Hardcoded list of voice command keywords.
- **Impact**: Intent classifier must match against dynamic user-configured lists and parameters.

---

### Decision 11: Target Build Environment
- **Context**: Development environment.
- **Decision made**: Target build environment is Google Antigravity.
- **Reason**: System tools, terminal, and AI pair-programming agent environment.
- **Alternatives considered**: Standard manual CLI workflow.
- **Impact**: All build and test workflows validated in Antigravity.

---

### Decision 12: Hybrid-Mode Architecture
- **Context**: Auth and data storage model when hybrid mode is active.
- **Decision made**: Use cloud storage (Supabase PostgreSQL + RLS + real password/token auth) for hybrid mode, NOT remote/tunnel access to the local desktop.
- **Reason**: Remote tunneling introduces network complexity, firewall issues, and desktop uptime dependencies. Cloud BaaS ensures reliable web access. Supabase auto-pause is mitigated with client-side status checks.
- **Alternatives considered**: Tailscale/ngrok tunnel to local machine; custom auth server.
- **Impact**: Clear separation: local mode uses Markdown vault + LanceDB; hybrid mode syncs to Supabase.
