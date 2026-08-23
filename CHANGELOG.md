# Relay — Changelog

## [0.9.3] - 2026-08-23

### Meeting Notification Popups & Components Sidenav Route

- **Components Sidenav Navigation (`native/src/components/common/NativeSidebar.tsx`, `App.tsx`)**:
  - Added new navigation tab `components-meeting-notifications` and breadcrumb routing (`Components > Meeting > Notifications`).
  - Added dedicated `Components` navigation section in the collapsible sidebar with direct access to meeting notification design options.
- **10 Sleek Interactive System Popup Design Options (`rounded-lg`) (`native/src/components/meetings/MeetingNotificationsDesignGallery.tsx`)**:
  - Expanded the interactive gallery to feature 10 compact, non-intrusive meeting notification popup designs, all formatted with `rounded-lg` (8px border-radius) corners and prominent CTAs:
    1. *Compact HUD Bar*: Top-anchored HUD bar (`rounded-lg`) with live mic activity pulse and "Record Now" button.
    2. *Compact Quick Dock Widget*: Streamlined dock card (`rounded-lg`) with input mode toggles and REC CTA.
    3. *Animated Gradient Border Card*: Shimmering gradient border card (`rounded-lg`) with participant avatar cluster and "Start Recording Now" CTA.
    4. *Stealth Mini Floating Bar*: Ultra-compact 34px height horizontal bar (`rounded-lg`) for zero screen clutter.
    5. *Left-Accent Banner*: Left-border accent card (`rounded-lg`) with speaker badge and glowing red "Start Recording" button.
    6. *Waveform Control Bar*: Dark tech widget (`rounded-lg`) with animated audio frequency visualizer simulation and "Initiate Capture" CTA.
    7. *AI Copilot Quick Toast*: Smart assist toast (`rounded-lg`) with output preset selector chip and "Record & Transcribe" button.
    8. *Corner Action Tray*: 2-row action tray (`rounded-lg`) with participant count badge and "Capture Audio" button.
    9. *Edge-Anchored Mini HUD*: High-contrast HUD card (`rounded-lg`) with live mic input status indicator and "Start STT" CTA.
    10. *Micro Pre-Flight Command Card*: Compact pre-meeting prep card (`rounded-lg`) with mic signal check meter and "Launch Recording" CTA.
  - **Live Global Meeting Notification Wiring (`MeetingReminderToastListener.tsx`, `App.tsx`)**:
    - Wired Variant 08 (Native Inspired) directly to live Rust backend events (`meeting-reminder` event & `get_current_meeting_reminder` IPC) via `MeetingReminderToastListener`.
    - Automatically displays a floating top-right notification alert inside Relay whenever an upcoming, unrecorded, or detected meeting reminder fires.
    - Connected real actions: **Record** triggers `start_meeting_recording`, **Snooze** triggers `snooze_meeting_reminder` (5m/10m/15m/30m options), and **Dismiss** triggers `dismiss_meeting_reminder` in the Rust queue.
    - Zero production meeting logic or Tauri window side effects (100% safe isolated preview surface).

## [0.9.2] - 2026-08-23

### Repository Licensing & Governance Migration (AGPL-3.0-only)

- **Open-Source Licensing Migration (`LICENSE`, `package.json`, `native/src-tauri/Cargo.toml`, `native/package.json`, `web/package.json`)**:
  - Migrated Relay's top-level open-source license from MIT to GNU Affero General Public License Version 3 (`AGPL-3.0-only`).
  - Added complete official AGPLv3 legal text with copyright `Copyright (C) 2026 Relay Maintainers`.
  - Updated package manifest license metadata across all project surfaces (`native/src-tauri/Cargo.toml`, root `package.json`, `native/package.json`, `web/package.json`).
- **README Authoring Rules & Pre-Commit Verification (`rules/readme.md`, `scripts/verify-commit-rules.js`, `.git/hooks/`)**:
  - Created `rules/readme.md` detailing machine-readable rules for creating, auditing, and maintaining `README.md`, including AGPLv3 strategy, pre-production status rules, open-source core vs. commercial services boundaries, and trademark guidelines.
  - Implemented `scripts/verify-commit-rules.js` and installed Git `.git/hooks/pre-commit` and `.git/hooks/pre-push` to automatically enforce versioning, changelog, and README compliance on commits/pushes.
  - Updated `README.md` to reference `AGPL-3.0-only` and `rules/global.md` & `AGENTS.md` to integrate `rules/readme.md`.

## [0.9.1] - 2026-08-23

### Meetings Directory UI Restructure & Calendar View

- **Simplified Meeting Filters (`native/src/components/meetings/MeetingPage.tsx`)**:
  - Replaced the nested Standalone/Series/Calendar filter system with a flat, unified view.
  - Implemented intuitive top-level tabs: `All`, `Scheduled`, and `Completed/Recorded`.
- **Custom Agenda Calendar View (`native/src/components/meetings/CalendarView.tsx`)**:
  - Implemented a custom `CalendarView` component that renders on the right-hand side when no specific meeting is selected.
  - Replaced the blank empty state with a lightweight, visual schedule of upcoming meetings grouped by day.
- **Streamlined Capture Actions**:
  - Simplified the calendar import button text from `+ Add & start in Relay` to a cleaner `Start Capturing`.
  - Refined the "Calendar Sync" modal trigger button to dynamically display a green `Calendar Connected` state.
### Navigation & Layout Modernization (shadcn sidebar-07)

- **shadcn `sidebar-07` Sidenav Pattern (`web/`, `native/`)**:
  - Implemented collapsible icon sidebar (`w-64` expanded, `w-12` collapsed) with custom cubic bezier smooth transitions.
  - Added workspace / team switcher header with local vault vs hybrid cloud sync options.
  - Implemented core navigation with tooltips, active tab indicators, and quick vault shortcuts.
  - Added user profile footer popover menu with avatar, plan status, changelog, and settings triggers.
  - Created Radix UI primitives (`dropdown-menu`, `tooltip`) for native frontend.

## [0.9.0] - 2026-08-22

### Phase 11D.1: Relay Security & Foundation Hardening Pass

- **Supabase RLS & Ingestion Hardening (`supabase/migrations/20260822_relay_account_schema.sql`)**:
  - Replaced open RLS write policies with secure `SECURITY DEFINER` PostgreSQL RPC functions: `register_installation_heartbeat` and `ingest_diagnostic_event` with strict input validation.
  - Locked down `SELECT` on `installations` and `diagnostics_events` tables strictly to `service_role` (or authenticated installation owner).
- **Google Calendar OAuth Tokens Migrated to OS Keyring (`meetings/calendar.rs`)**:
  - Relocated all Calendar OAuth token storage from `vault/google_calendar_token.json` to the OS Keyring / Credential Manager (`keyring` crate) with an encrypted fallback in `.relay/config/`.
  - Added automated migration that purges any legacy token files in `vault/` to ensure zero secrets ever reside in the user's markdown vault.
- **Account Deletion $\ne$ Local Vault Deletion (`identity/mod.rs`, `commands.rs`, `AccountSettings.tsx`)**:
  - Implemented `delete_relay_account` Tauri command and Settings UI modal that deletes cloud profile state while leaving local markdown notes, recordings, scribbles, and meetings 100% untouched.
- **First-Run Onboarding Vocabulary Refinement (`WelcomeModal.tsx`)**:
  - Refined first-run choices to clearly present **"Continue with Google"** (Primary CTA) vs. **"Continue Locally"** (Secondary CTA: *"No account required. Your data remains entirely on this device."*), with zero "Skip" terminology.
- **Opt-In Telemetry by Default (`settings/mod.rs`)**:
  - Set `allow_anonymous_diagnostics` to default to `false` (opt-in privacy by default).

### Phase 11D: Relay Identity, Product Account & Local→Hybrid Foundation

- **Relay Account $\neq$ Local Vault Invariant (`native/src-tauri/src/identity/`)**:
  - Implemented the `RelayAccount` domain model (`identity/models.rs`) clearly separated from the local markdown vault. Signing in identifies the user and installation but strictly never uploads or moves any local notes, recordings, audio, scribbles, or meetings to the cloud.
  - Added support for `AccountMode::Local` and `AccountMode::Hybrid` with `SubscriptionInfo` scaffolding (`SubscriptionPlan::Free`, `SubscriptionPlan::Hybrid`).
- **Desktop Google Sign-In & Secure Token Storage (`identity/oauth.rs`, `identity/tokens.rs`)**:
  - Implemented real desktop Google OAuth 2.0 PKCE loopback server on `127.0.0.1:{port}/oauth/callback` with system browser launch and responsive HTML success/failure pages.
  - Secured OAuth tokens (`access_token`, `refresh_token`) inside the OS Keyring / Credential Manager (`keyring` crate) with an encrypted local fallback file, strictly outside `localStorage` and outside the vault.
  - Sign-out cleanly purges tokens from secure storage and reverts to Local mode while keeping the user's local vault completely untouched.
- **Stable Anonymous Installation Identity (`identity/installation.rs`)**:
  - Implemented UUID v4-based anonymous installation ID generated once and persisted in `.relay/config/installation.json`.
  - Survives app restarts and updates without invasive hardware fingerprinting.
  - Added UI masking (`••••••••-••••-XXXX`) with instant click-to-copy for diagnostics and support.
- **Privacy-First Diagnostics & Telemetry Firewalled Abstraction (`diagnostics/mod.rs`)**:
  - Built `DiagnosticsService` with an absolute privacy firewall: payloads strictly contain anonymous system metadata (`installation_id`, `account_id`, `relay_version`, `platform`, `os_version`, `event_type`, `timestamp`).
  - Strict Guarantee: Zero note contents, transcripts, audio recordings, or knowledge graph data can ever be collected or transmitted.
  - Controlled by an explicit user consent toggle in Settings.
- **Update Service Abstraction (`updates/mod.rs`)**:
  - Created `UpdateService` with semver comparison and offline resilience, gracefully handling network disconnections without interrupting local app usage.
- **Supabase Cloud Backend, Auth & Database Schema (`supabase/`, `native/src-tauri/src/identity/supabase.rs`)**:
  - Implemented complete PostgreSQL migration schema ([`supabase/migrations/20260822_relay_account_schema.sql`](file:///d:/Projects/Relay/supabase/migrations/20260822_relay_account_schema.sql)) with Row Level Security (RLS) for `relay_accounts`, `installations`, `diagnostics_events`, and `app_releases`.
  - Added Rust backend `SupabaseClient` for async profile upserts, installation tracking, privacy-safe telemetry dispatch, and release update checks.
  - Added `.env` configuration support with `dotenvy` in Rust backend and template in `.env.example`.
  - Upgraded Google OAuth flow to support both Supabase Auth broker (`/auth/v1/authorize?provider=google`) and direct Google OAuth with loopback hash-fragment bridge.

### Phase 11C: First-Class Meetings Capture Surface, Recurring Series & Meeting-to-Knowledge Promotion

- **First-Class Persistent Meetings Domain Model (`native/src-tauri/src/vault/meeting.rs`, `vault/mod.rs`)**:
  - Implemented `Meeting`, `MeetingSeries`, and `MeetingActionItem` structures stored in `vault/meetings/` and `vault/meeting_series/` with YAML frontmatter + Markdown body formatting.
  - Enforced architectural separation: *Meeting $\ne$ Scribble*. Moving or extracting notes into Scribbles never deletes, mutates, or collapses the persistent Meeting source record.
  - Added discrete Standalone Meeting support alongside Recurring Meeting Series groupings (individual occurrences are independently addressable, series views display the newest occurrence first).
- **Real Google Calendar OAuth 2.0 & Event Synchronization (`meetings/calendar.rs`, `commands.rs`)**:
  - Completely purged all mock/dummy calendar events, fake attendees, and hardcoded mock data.
  - Implemented real Google OAuth 2.0 PKCE / Loopback authorization on `127.0.0.1` using the minimum read-only scope (`https://www.googleapis.com/auth/calendar.events.readonly`).
  - Added secure token persistence in `vault/google_calendar_token.json` with automated token refresh before expiry.
  - Added real Google Calendar events synchronization querying primary calendar with singleEvents expansion for recurring series.
  - Built full connection lifecycle in `CalendarSyncModal.tsx`: disconnected state with `[Connect Google Calendar]`, optional custom Client ID/Secret configuration, connected state displaying real authenticated account email, `[Sync Now]`, and `[Disconnect]`.
- **Real Browser & Native Conferencing Window Detection (`meetings/mod.rs`, `commands.rs`)**:
  - Implemented active video conferencing detection via Windows Win32 `EnumWindows` and `GetWindowTextW` for Google Meet (Chrome, Edge, Firefox, Brave, Opera), Zoom, Microsoft Teams, and Cisco Webex.
  - Added window title sanitization (`clean_meeting_window_title`) to extract clean meeting topics from browser window frames.
  - Maintained strict consent-first model: `MeetingDetectionPopup.tsx` prompts the user (`[Start Recording]`, `[Not this meeting]`) and **never** records automatically without explicit consent.
- **Visual Graph Distinction & Candidate Suggestion Isolation (`graphRenderer.ts`, `GraphSettingsPanel.tsx`, `vault/mod.rs`)**:
  - Rendered `DERIVED_FROM` provenance edges with distinct dashed strokes (`ctx.setLineDash([4, 4])`) and purple/slate tones, distinguishing them from solid semantic knowledge edges.
  - Styled Meeting source nodes with distinct purple color and concentric double-ring accents.
  - Added `Meeting Sources & Provenance` filter toggle in graph settings.
  - Enforced that candidate scribbles from AI enrichment remain strictly in `meeting.candidate_scribbles` as suggestions until the user explicitly accepts them.
- **Meeting Audio Recording Pipeline & AI Enrichment (`native/src-tauri/src/pipeline/enrichment.rs`, `commands.rs`)**:
  - Integrated meeting capture with Relay's local Whisper STT engine and local LLM pipeline.
  - Added `enrich_meeting` to asynchronously extract executive summaries ($\ge 100$-word threshold rule), explicit decisions, structured action items (with assignees & priorities), open questions, and candidate scribble suggestions.
- **Full Meeting Management Dashboard & Detail View (`native/src/components/meetings/`)**:
  - `MeetingPage.tsx`: Master-detail navigation supporting Standalone and Recurring Series groupings, search across transcripts/action items/participants, and 1-click Google Calendar sync.
  - `MeetingDetailView.tsx`: Complete control surface featuring audio recording triggers, progressive disclosure tabs (*Notes & Summary*, *Decisions & Tasks*, *Open Questions*, *Audio Transcript*, *Derived Scribbles*), editable meeting metadata, and markdown export.
  - `MeetingModal.tsx` & `CalendarSyncModal.tsx`: Creation modals for standalone meetings, recurring series cadences, and calendar event imports.
- **Knowledge Graph Provenance & 30-Day Trash (`native/src-tauri/src/vault/scribble.rs`, `vault/mod.rs`)**:
  - Scribbles created from meetings (`create_scribble_from_meeting`) automatically carry `source_type: "meeting"`, `meeting_id`, and `meeting_series_id` provenance metadata.
  - The Obsidian-inspired Knowledge Graph automatically connects derived Scribbles to a virtual `source` node via `DERIVED_FROM` edges.
  - Deleting a meeting moves it to the 30-day Trash without affecting or orphaning any derived Scribbles.

## [0.8.2] - 2026-08-22

### Scribbles AI Enrichment Polish, Summary Thresholds, Exploration Fallbacks & Collapsible Content

- **Structured AI Summarization & Threshold Enforcement (`native/src-tauri/src/pipeline/enrichment.rs`, `ScribbleDetailEditor.tsx`)**:
  - Added strict $\ge 100$-word threshold: the "Summarise" action button only appears and triggers for long thoughts ($\ge 100$ words).
  - Short notes under 100 words bypass automatic summarization during enrichment.
  - Summaries render concise takeaways with bold lead-ins, numbered badges, structured bullets, and dark-theme Mermaid flowcharts via the custom `MarkdownView` component.
- **Synchronous Full-Refresh on Re-Enrichment (`native/src-tauri/src/commands.rs`, `ScribbleDetailEditor.tsx`)**:
  - `trigger_enrich_scribble` returns the enriched `Scribble` directly, enabling immediate state synchronization for title, topics, entities, summary, and questions without waiting for async event listeners.
- **Merged Note Title Derivation & Section Header Sanitization (`native/src-tauri/src/vault/mod.rs`, `enrichment.rs`)**:
  - Sanitized internal merge section headers to prevent placeholder markers (e.g. `### [Synthesis: Generating title… + 2 more]`) from leaking into synthesized markdown content.
  - Hardened LLM title derivation to strictly reject echoed placeholder phrases and derive clean concept titles directly from content.
- **Guaranteed AI Exploration Questions (`enrichment.rs`, `ScribbleDetailEditor.tsx`)**:
  - Added flexible serde deserialization aliases (`exploration_questions`, `suggested_questions`, `open_questions`) and dynamic topic-based fallback generation so exploration questions always populate and persist.
- **Collapsible Long Thought Content (`ScribbleDetailEditor.tsx`)**:
  - Added a collapsible **"Read More" / "Show Less"** toggle with bottom gradient fade for thoughts exceeding 200 words, preventing infinite scroll while preserving full detail on demand.
- **Dangling Knowledge Connections Cleanup (`vault/mod.rs`, `ScribbleDetailEditor.tsx`)**:
  - Cleaned up dangling relationship pointers when source notes are merged or moved to Trash.
  - Filtered rendered knowledge connections to strictly display active, non-trashed notes in the vault.

## [0.8.1] - 2026-08-22

### Obsidian-Fidelity Knowledge Graph Rework, Organic Physics & Stable Coordinate Persistence

- **Force-Directed Physics & Organic Equilibrium (`native/src/components/scribble/graph/graphPhysics.ts`)**:
  - Implemented Coulomb electrostatic repulsion with distance clamping, Hooke spring link forces with configurable link distance, center gravity, and velocity damping (`0.88`).
  - Simulation energy decays smoothly to a stable equilibrium without persistent jitter or continuous resource consumption.
  - Interactive force sliders dynamically reheat the live simulation with immediate physical adaptation.
- **Persistent Node Coordinates & Graph Stability (`native/src/components/scribble/graph/graphStorage.ts`)**:
  - Persists node `(x, y)` positions in `localStorage` (`relay_knowledge_graph_positions_v1`), preserving relative structure across sessions.
  - New nodes are placed organically near connected neighbors without scrambling existing graph structure.
  - Added dedicated **Reset Layout** action with confirmation modal to re-simulate positions without wiping graph settings.
- **Independent Camera Architecture (`native/src/components/scribble/KnowledgeGraphView.tsx`)**:
  - Decoupled camera transforms `{ x, y, k }` from node coordinates with zero auto-fitting after loading, dragging, or settling.
  - Added cursor-centered mouse-wheel zoom, drag panning, on-screen zoom buttons, and keyboard controls (Arrow keys, `Shift + Arrow`, `+`/`-`, `0` reset view, `Space` reheat).
- **Interactive Node Dragging & Normalized High-DPI Canvas (`graphRenderer.ts`, `KnowledgeGraphView.tsx`)**:
  - Aligned high-DPI `devicePixelRatio` scaling and CSS world-coordinate hit testing for precision cursor grabbing.
  - Node dragging pins the active node, applies live spring reactions to neighbors, and relaxes smoothly upon release.
- **Obsidian Quieting Hover & Dynamic Label Fading (`graphRenderer.ts`)**:
  - Hovering a node brightly highlights 1-hop connected neighbors/edges while quieting unrelated content to low opacity.
  - Non-linear degree-based node sizing with upper radius limits.
  - Dynamic zoom-dependent text fading with priority rendering (Hovered > Selected > High-degree > Normal) and clean ellipsis truncation.
- **Filters, Dynamic Search-Driven Groups & Local Graph (`GraphSettingsPanel.tsx`, `GraphToolbar.tsx`)**:
  - Configurable filters for Scribbles, Voice Notes, Topics, Entities, Attachments, and Orphans.
  - Search-driven custom color groups updating matching nodes in real time.
  - Scoped **Local Graph** exploration mode with configurable depth (`1`, `2`, `3`).
  - Chronological **Time-lapse** animation revealing nodes by creation timestamp.
- **Relay Context Actions & Inspector (`GraphNodeInspector.tsx`, `ScribbleViewer.tsx`)**:
  - Contextual inspector drawer displaying node metadata, summary, and clickable 1-hop connections.
  - Integrated direct actions: **Open in Editor**, **Connect Scribble**, **Merge Scribbles**, and **Move to Trash**.
  - Streamlined Scribbles navigation to dedicated **Capture**, **Workspace**, and full-screen **Knowledge Graph** tabs.

## [0.8.0] - 2026-08-21

### Phase 11B: First-Class Scribbles Knowledge Layer, Provenance Model & Obsidian-Inspired Knowledge Graph

- **First-Class Persistent Scribbles (`native/src-tauri/src/vault/scribble.rs`, `vault/mod.rs`)**:
  - Implemented the `Scribble` data model with full frontmatter Markdown serialization in `.relay/vault/scribbles/<id>.md`, compatible with Obsidian.
  - Implemented CRUD persistence (`save_scribble`, `get_scribble`, `list_scribbles`, `update_scribble`, `delete_scribble`).
  - Supports multiple capture source types (`voice`, `text`, `file`, `clipboard`, `browser_selection`, `browser_page`, `screenshot`, `meeting`) while preserving complete provenance in `source_metadata`.
- **Zero Impact on Voice Note / Capture UX (`VoiceNotePage.tsx`)**:
  - The `Ctrl + Space` hotkey and Dictation Pill flow remain 100% unchanged with zero modal prompts during recording.
  - Added dedicated **"Save as Scribble"** promotion action to Voice Note cards in `VoiceNotePage.tsx`, creating a new linked Scribble while keeping the original Voice Note and raw audio intact.
- **Asynchronous AI Enrichment (`native/src-tauri/src/pipeline/enrichment.rs`, `commands.rs`)**:
  - Non-blocking background AI enrichment extracts concise titles, summaries, topics, named entities, and suggested concept connections via `LLMClient`.
  - All AI metadata is fully editable by the user (AI suggests, user decides) and resilient to offline/error states.
- **Explicit Relationships Model (`vault/scribble.rs`, `commands.rs`)**:
  - Supported relationship types: `RELATED_TO`, `MENTIONS`, `SAME_TOPIC`, `SAME_PROJECT`, `CONTRADICTS`, `EXTENDS`, `DERIVED_FROM` with origin and confidence tracking without merging independent knowledge objects.
- **Obsidian-Inspired Knowledge Graph View (`native/src/components/scribble/KnowledgeGraphView.tsx`)**:
  - High-performance 2D Canvas graph renderer with real-time force simulation physics.
  - Circular nodes with connectivity-driven sizing (degree scaling), subtle low-weight edges, restrained color-coded node categories, and zoom-dependent intelligent labels.
  - Interactive 1-hop neighborhood highlighting, dragging, zoom/pan controls, and side inspector drawer.
  - Specialized filtering including node types and a dedicated **Orphans** filter for discovering isolated thoughts.
- **Knowledge Workspace Surface (`native/src/components/scribble/ScribbleViewer.tsx`, `ScribbleComposer.tsx`, `ScribbleDetailEditor.tsx`)**:
  - Rebuilt the Scribbles view into a complete Knowledge Workspace with 3 view modes: **Workspace** (List + Detail Editor), **Knowledge Graph** (full-screen Obsidian canvas), and **Split View** (Canvas + Editor).
  - Quick-Create bar supporting instant typed thoughts and file uploads.
  - Real-time search across thoughts, topics, and entities.

## [0.7.3] - 2026-08-21

### STT Reliability: Language Wiring Sync, Audio Telemetry & Frame-Based Voice Activity Detection (VAD)

- **Language Settings Synchronization (`commands.rs`, `stt.rs`, `DictationPill.tsx`, `ProviderSettings.tsx`)**:
  - Connected Dictation Pill language popover to Tauri IPC `save_settings` backend commands and event broadcast (`settings-changed`), ensuring settings persist to `settings.json` and sync live across all windows.
  - Hardened Whisper language configuration in `SttLanguageConfig::from_settings`: auto/empty maps to Whisper auto-detection (`None`), single language maps to locked language tag (e.g. `Some("hi")`), and multilingual pairs (e.g. Hinglish `["en", "hi"]`) safely default to multilingual auto-detection without passing invalid token tags.
- **Audio Measurement & Telemetry (`capture/mod.rs`)**:
  - Implemented `AudioStats` telemetry calculating sample count, duration, RMS, peak amplitude, near-zero percentage, and near-clipping percentage on captured and analyzed audio buffers.
  - Sanitized audio buffers against non-finite values (`NaN`/`Inf`) and clamped samples to `[-1.0, 1.0]`.
  - Analyzed 233 real microphone recordings on disk to verify dynamic range (average RMS 0.0254, max peak 0.7477, 0.00% clipping).
- **Speech Boundary Detection / VAD (`capture/mod.rs`)**:
  - Implemented deterministic frame-based energy VAD (`VadConfig`, `VadResult`) with 20ms frames, adaptive background noise floor estimation (bottom 20% frames), onset requirement (80ms), hangover duration (300ms), and safe pre/post-speech padding (250ms).
  - Validated with an offline batch experiment over 233 real user recordings, achieving an average of 24.1% dead air reduction (~1.77s saved per recording) with $<1\text{ ms}$ processing overhead and zero speech truncation.
  - Short-circuits accidental empty/click recordings (50 files identified) to prevent Whisper hallucinations on silence.

## [0.7.2] - 2026-08-20

### Voice Note Actions (Edit, Delete, Merge), High-Contrast Destructive Tokens & Geometry Polish

- **Backend (`native/src-tauri/src/vault/mod.rs`, `native/src-tauri/src/commands.rs`)**: Added
  `update_voice_note`, `delete_voice_note`, and `merge_voice_notes` Tauri commands and unit
  tests for persistent Markdown vault note editing, deletion, and chronological adjacent note merging.
- **Frontend (`native/src/components/voicenotes/VoiceNotePage.tsx`)**:
  - Removed redundant `"Voice Note"` text labels from each note card in favor of clean timestamps
    and word count chips.
  - Added full interactive action toolbar: **Edit** (inline textarea with <kbd>Ctrl+Enter</kbd>
    save / <kbd>Esc</kbd> cancel), **Delete** (safety confirmation banner), **Adjacent Merge**
    (combines split transcripts into one), and **Copy** (animated 1-click clipboard copy).
  - Enhanced delete confirmation banner with high-contrast, accessible red accents across both
    Light and Dark themes.
- **Design System (`native/src/index.css`, `web/src/app/globals.css`, `native/src/components/ui/button.tsx`)**:
  - Fixed `--destructive` and `--destructive-foreground` design tokens to vivid crimson (`#EF4444`)
    and crisp white (`#FFFFFF`) in both Light and Dark modes.
  - Standardized all button variants (`destructive`, `outline`, `secondary`, `ghost`) to consume
    theme tokens rather than hardcoded slate classes.
  - Standardized UI components across all modules and floating widgets to `rounded-lg` geometry.
  - Implemented custom minimal theme-aware scrollbars (`4px` width, transparent track, `40%` opacity `--muted-foreground` thumb) and `.no-scrollbar` utility across both native and web frontends.
- **Pill Positioning & Alignment (`ProviderSettings.tsx`, `DictationPill.tsx`, `PillSettingsPopover.tsx`, `overlay.rs`)**:
  - Replaced legacy position options with **Bottom Left**, **Bottom Center** (default), and **Bottom Right**.
  - Pushed Bottom Left and Bottom Right anchor positions flush to the monitor's work area edges.
  - Aligned internal pill components (main pill body, keyboard hint bar, notch, and settings popover) to the left on Bottom Left and to the right on Bottom Right.

## [0.7.1] - 2026-08-20

### Dynamic Changelog, 1-Click Theme Toggle & UI Streamlining

- **Backend (`native/src-tauri/src/commands.rs`, `native/src-tauri/src/lib.rs`)**: Added
  `get_app_version` and `get_changelog` Tauri commands that dynamically parse `VERSION` and
  `CHANGELOG.md` at runtime so the release notes modal and footer stay completely up to date
  without hardcoded lists.
- **Frontend (`native/src/components/ThemeToggle.tsx`)**: Streamlined the theme toggle into a
  direct 1-click toggle between light and dark mode with mode-appropriate container styling
  and icons representing the target switch mode (Moon in Light mode, Sun in Dark mode).
- **Frontend (`native/src/components/common/ChangelogModal.tsx`)**: Converted the release notes
  modal to load dynamically from the backend registry with categorized tags and domain pills.
- **Frontend (`native/src/App.tsx`, `native/src/components/capture/PTTWidget.tsx`)**: Removed
  the static `Local Mode` header badge, cleaned the `Local Vault` label, and removed obsolete
  placeholder cards.

## [0.7.0] - 2026-08-20

### Voice Note — Universal Dictation History & Configurable Vault Directory Location

Voice Note is now the persistent history of every successful Relay
transcription, stored in the existing local Vault — regardless of which
capture path produced it or whether text injection succeeded. See
`docs/decisions.md` (Decision 38) for the full design.

- **Backend**: Added a `voice_note` note type to the existing Vault
  (`native/src-tauri/src/vault/mod.rs`) — reuses the existing Markdown/
  frontmatter format and `VaultManager::save_note`, no new database. A new
  `commands::save_voice_note` funnel is called from both the global
  dictation hotkey (`hotkeys::stop_dictation_session`) and click-to-talk
  (`commands::process_captured_audio`) right after each already has a
  successful, non-empty transcript, so one recording always produces
  exactly one Voice Note, independent of injection's outcome.
- **Backend**: "Vault Directory Location" is now a real, persisted setting
  (`AppSettings.vault.directory`) instead of decorative UI text. Added
  `get_vault_location`, `choose_vault_folder` (native OS folder picker via
  `tauri-plugin-dialog`, now registered), and `set_vault_location`
  commands. `VaultManager` can repoint its root at runtime with no
  restart, so a freshly chosen folder is usable immediately.
- **Frontend**: The "Voice Capture" sidebar tab is renamed "Voice Note" and
  rebuilt as the dictation history/review surface — a banner, three stat
  cards (Total Voice Notes, Total Words, Notes Today), and a live-updating
  Transcript History list. First visit with no configured location shows
  a setup prompt (native folder picker or "Use Default Relay Vault"); an
  inaccessible location shows a recovery prompt instead of crashing.
  Settings → Vault & LanceDB now shows the real resolved path and can
  change it via the same commands. Removed the redundant "Menu" label
  above the sidebar's navigation items.
- **Preserved**: Global PTT, click-to-talk, the Dictation Pill, Scribble
  Notes, Kanban, and existing settings are all unmodified.

## [0.6.0] - 2026-08-20

### Toggle-to-Talk — Optional Press-Once Dictation Mode

Requested directly as a new capability (not part of the desktop-first
scope reduction in the entries below): holding the dictation hotkey down
for a long recording is tedious, so this adds an opt-in mode where one
press starts recording and a second press stops it.

- **Backend (`native/src-tauri/src/settings/mod.rs`)**: Added
  `HotkeySettings.toggle_to_talk: bool` (default `false` — hold-to-talk
  remains the default for everyone who doesn't opt in).
- **Backend (`native/src-tauri/src/hotkeys/mod.rs`)**: Reworked the
  dictation hotkey's press/release state machine. Added a `key_down` flag
  to `DictationState` so a genuine second press (toggle mode's "stop"
  signal) can be told apart from the OS re-firing "pressed" while a key
  stays physically held — both modes already had to filter out the
  latter; only toggle mode needed the former. Extracted the actual
  stop/transcribe/inject logic (previously inline in
  `on_dictation_released`) into a new `stop_dictation_session` function so
  both "release stops it" (hold-to-talk) and "a second press stops it"
  (toggle-to-talk) call the same path. Added a separate 10-minute
  stuck-session watchdog timeout for toggle mode
  (`MAX_PERSISTENT_RECORDING`) — the existing 60-second one
  (`MAX_DICTATION_HOLD`, sized for hold-to-talk's short-recording
  assumption) would have silently cut off the exact long recordings this
  feature exists to make easier.
- **Frontend (`native/src/components/settings/ProviderSettings.tsx`)**:
  Added a "Toggle-to-Talk" switch in Settings → General, next to the
  existing hotkey recorders. Applies immediately via `save_settings` (no
  hotkey re-registration needed, since only the interpretation of
  press/release changes, not the key combination itself). Reworded the
  "Universal Dictation" hotkey label, since "(hold to talk...)" is no
  longer always true.
- **Frontend (`native/src/components/capture/DictationPill.tsx`)**: The
  floating hint text now reads "Tap to start/stop" instead of "Hold to
  record" when toggle-to-talk is active, so the pill never states the
  wrong interaction model for how the hotkey actually behaves right now.
- **Frontend (`native/src/types/index.ts`)**: Added `toggle_to_talk` to
  the `HotkeySettings` TypeScript interface, mirroring the Rust struct.
- **What was NOT changed**: Click-to-talk (already toggle-based — each
  click flips start/stop, unaffected by this change), STT, text injection,
  the `AudioRecorder` capture primitive, `overlay.rs`, and every decision
  from the desktop-first scope reduction (Decisions 32–36).
- **Verification**: `native/` — `npm run build` (`tsc && vite build`)
  passes clean with zero errors. Rust — `cargo check` passes cleanly
  (2.4s on the warm build cache). The full press/release/repeat/watchdog
  state machine was traced by hand through every case (hold-to-talk
  unchanged; toggle-to-talk's press→press cycle; OS key-repeat in both
  modes; the watchdog firing in both modes) before considering this done
  — see Decision 37 for the reasoning and one accepted, narrow edge case.
- **Not independently tested end-to-end**: same environment limitation as
  every other change in this session — this is a headless Linux container
  with no display server or audio hardware, and Relay targets Windows.
  Compiler-level correctness is verified; live hotkey/microphone behavior
  is not.

## [0.5.0] - 2026-08-20

### Scope-Reduction Set 5 — Single Dictation Pill (Docked/Floating Mode Removed)

The one set where real architectural consolidation was expected. The
Set 0 audit found `DictationPill.tsx` was already the sole canonical pill
implementation — `FloatingPill.tsx` and `PTTWidget.tsx` were two render
*sites* for it (a separate always-on-top window vs. inline in the main
window), not competing implementations, selected by one boolean setting
(`ui.show_floating_pill`, the "Docked vs Floating" product mode this
release removes per the task's own scope table — unlike Kanban/Voice
Chat/Triggers, which are *deferred*, this is a genuine *removal*).

- **Backend (`native/src-tauri/src/settings/mod.rs`)**: Removed
  `UiSettings.show_floating_pill`. `ui.pill_position` (work-area-aware
  anchor edge) is unchanged.
- **Backend (`native/src-tauri/src/commands.rs`)**: Removed the
  `set_pill_visible` command — its only purpose was toggling the
  now-removed setting.
- **Backend (`native/src-tauri/src/lib.rs`)**: `overlay::ensure_pill_window`
  is now always called with `visible: true` at startup — the floating pill
  window is the one, permanent PTT surface. Removed `set_pill_visible` from
  the `invoke_handler!` registration.
- **Backend (`native/src-tauri/src/hotkeys/mod.rs`)**: Removed a
  docked-mode-specific compensation in `on_dictation_pressed` that showed
  the main window and switched it to the Capture tab so a docked pill's
  reaction to the hotkey would be visible. Unnecessary now — the
  always-on-top floating pill is always present and already reacts to the
  same `capture-state-changed` event regardless of the main window's state.
  Caught by a full rebuild, not by inspection alone: this was a second,
  easy-to-miss reference to the removed setting.
- **Frontend (`native/src/components/capture/PTTWidget.tsx`)**: Removed
  the `showFloatingPill` state, its settings-load effect, its
  `pill-visibility-changed` listener, and the conditional that rendered
  `DictationPill` inline as the "docked" alternative. Removed the
  `onProcessComplete` prop entirely — it was only ever needed by the
  now-removed inline render path; the floating pill (which never received
  this prop) already relies solely on the `capture-processed` backend
  event for the main window to learn about completions, so nothing was
  lost. The informational badge is now static text (no more "toggle it
  off in Settings" — there's no such toggle anymore).
- **Frontend (`native/src/App.tsx`)**: `<PTTWidget />` no longer takes a
  prop; `handleProcessComplete` is unchanged (still used directly by the
  `capture-processed` event listener).
- **Frontend (`native/src/components/settings/ProviderSettings.tsx`)**:
  Removed the "Floating Dictation Pill" toggle switch and the stale
  `show_floating_pill` field from `DEFAULT_SETTINGS`.
- **Frontend (`native/src/types/index.ts`)**: Removed `show_floating_pill`
  from the `UiSettings` TypeScript interface, mirroring the Rust struct.
- **What was NOT changed**: `DictationPill.tsx`, `FloatingPill.tsx`,
  `PillSettingsPopover.tsx`, `PillTypes.ts`, and `overlay.rs` — all
  completely untouched. The capture state machine, STT, text injection,
  and `ui.pill_position`/`set_pill_position`/`set_pill_expanded`/
  `set_pill_window_mode` (work-area/monitor/DPI-aware positioning) are
  unmodified. No settings.json migration needed — `AppSettings` has no
  `deny_unknown_fields`, so a stale `show_floating_pill` key in an existing
  user's file is silently ignored on load and simply not written back.
- **Still open, not addressed by this release**: the click-to-talk vs.
  global-hotkey text-injection divergence flagged since Set 0 — raised
  again here for visibility, deliberately left unresolved per explicit
  direction each time it came up.
- **Verification**: `native/` — `npm run build` (`tsc && vite build`)
  passes clean with zero errors (module count unchanged at 1606 — this
  set trimmed code within existing files rather than removing whole
  modules). A first build attempt caught the `ProviderSettings.tsx`
  `DEFAULT_SETTINGS` reference to the removed field as a real `tsc` type
  error, and a repo-wide grep after fixing it confirms zero remaining
  references to `show_floating_pill`, `set_pill_visible`, or
  `pill-visibility-changed` anywhere in `native/`. Rust — `cargo check`
  (with `LIBCLANG_PATH`/`CMAKE` overridden for this Linux container, same
  as Set 4) passes clean in 3.4s with the warm build cache, confirming
  `settings/mod.rs`, `commands.rs`, `lib.rs`, and `hotkeys/mod.rs` all
  compile correctly together.

## [0.4.8] - 2026-08-20

### Scope-Reduction Set 4 — Triggers/MCP Active Surface Removed

- **Frontend (`native/src/App.tsx`)**: Removed the sidebar "Trigger Phrases"
  nav button, the `triggers` branch of the tab switcher and hero header,
  and `triggers` from the `navigate-tab` allowlist. Removed the now-unused
  `TriggerSettings` and `Zap` icon imports.
- **Frontend (`native/src/components/settings/ProviderSettings.tsx`)**:
  Removed a *second*, easy-to-miss entry point to the same component — the
  "Triggers & MCP" sub-section inside the Settings screen's own sub-nav
  (`activeSection === 'triggers'`), separate from the App-level tab.
  Removed the now-unused `TriggerSettings` and `Zap` icon imports.
- **Backend (`native/src-tauri/src/commands.rs`)**: Removed the inline
  trigger-match-and-MCP-dispatch block from `process_captured_audio` — the
  function every click-to-talk ("scribble" mode) capture runs through.
  Before this change, a spoken phrase matching one of the two
  *enabled-by-default* triggers ("schedule meeting", "remind me to")
  silently short-circuited into a canned MCP-stub reply instead of the
  normal cleanup pipeline, on every fresh install — not only for users who
  had configured their own triggers. This ran automatically on the core
  capture path with no UI left to see or control it once the settings
  entries above are gone, so leaving it in place would not have actually
  deferred the feature. Removed the now-unused `use crate::mcp::McpRouter;`
  import and updated a comment that referenced "trigger-matching" as a
  downstream step it no longer is.
- **What was NOT changed**: `native/src-tauri/src/triggers/mod.rs`
  (`TriggerEngine`, including its unit tests), `native/src-tauri/src/mcp/mod.rs`
  (`McpRouter`), the `get_triggers`/`save_triggers` Tauri commands, and
  `native/src/components/settings/TriggerSettings.tsx` itself — all
  unmodified, just no longer invoked or reachable from the UI. PTT,
  click-to-talk's capture/transcription steps, hotkeys, STT, text
  injection, Kanban, Voice Chat, and Scribble Notes are unmodified.
- **Verification**: `native/` — `npm run build` (`tsc && vite build`)
  passes clean with zero errors; bundle module count dropped from 1608 to
  1606 (`TriggerSettings.tsx` and one component it exclusively imports no
  longer bundled). Rust — this container initially couldn't build past
  `gdk-sys` (Set 1) at all; installing the missing Tauri Linux
  prerequisites (`libgtk-3-dev`, `libwebkit2gtk-4.1-dev`,
  `libappindicator3-dev`, `librsvg2-dev`, `libasound2-dev`) plus overriding
  `LIBCLANG_PATH`/`CMAKE` for this invocation only (the repo's
  `.cargo/config.toml` hardcodes Windows-only paths for both, left
  untouched since it's an intentional, platform-specific config file, not
  something this change needed to touch) let `cargo check` reach and fully
  compile `relay-native-backend` (Relay's own crate, including the edited
  `commands.rs`) for the first time this session — `Finished \`dev\`
  profile [unoptimized + debuginfo] target(s)`, zero errors.

## [0.4.7] - 2026-08-20

### Scope-Reduction Set 3 — Voice Chat & TTS Active Surface Removed

- **Frontend (`native/src/App.tsx`)**:
  - Removed the sidebar "Voice Chat" nav button, the `chat` branch of the
    tab switcher and hero header, `chat` from the `navigate-tab` event's
    allowed payload list, and the `{activeTab === 'chat' && <ChatPanel />}`
    render line. `activeTab` can no longer become `'chat'`.
  - Removed the now-unused `ChatPanel` import and `Bot` icon import.
- **Frontend (`native/src/components/settings/ProviderSettings.tsx`)**:
  - Removed the "Local Text-to-Speech (Piper) — optional" settings block
    from the General section — its own caption stated its sole purpose was
    to "skip 'speak back' in voice chat," so it is Voice Chat's settings
    surface, not an independent one. Removed the now-unused `Volume2` icon
    import. `DEFAULT_SETTINGS.tts` and the `settings.tts` round-trip through
    `get_settings`/`save_settings` are unchanged — the fields simply have no
    editable UI anymore, exactly like Kanban's backend command in Set 2.
- **What was NOT changed**: `native/src/components/chat/ChatPanel.tsx` —
  untouched. `native/src-tauri/src/pipeline/chat.rs` (`process_chat`),
  `native/src-tauri/src/tts/mod.rs` (`TtsEngine`), and
  `native/src-tauri/src/providers/mod.rs` (the shared LLM client also used
  by the in-scope Kanban/meeting and Scribble pipelines) — all untouched;
  `git diff --stat native/src-tauri/` is empty for this release. The
  `AppSettings`/`ProcessedPipelineResult` TypeScript types in
  `native/src/types/index.ts` are unchanged. No settings schema changed.
  Kanban, PTT, click-to-talk, hotkeys, STT, and text injection unmodified.
- **Verification**: `native/` — `npm run build` (`tsc && vite build`)
  passes clean with zero errors; the production bundle now transforms 1608
  modules (down from 1609 after Set 2), confirming `ChatPanel.tsx` is no
  longer bundled. Repo-wide grep confirms the only remaining
  `'chat'`/`piper_*` references in `native/src` are inside `ChatPanel.tsx`
  itself (untouched) and the preserved `AppSettings`/`DEFAULT_SETTINGS`
  type shape (needed for settings round-tripping) — no orphaned
  references. `git status` confirms only the two files above changed.

## [0.4.6] - 2026-08-20

### Scope-Reduction Set 2 — Kanban Active Navigation Removed

- **Frontend (`native/src/App.tsx`)**:
  - Removed the sidebar "Kanban Board" nav button, the `kanban` branch of
    the tab switcher and hero header, and `kanban` from the `navigate-tab`
    event's allowed payload list — `activeTab` can no longer become
    `'kanban'`.
  - Removed the unconditional `get_kanban_cards` fetch that previously ran
    on every app launch (`useEffect` with an empty dependency array) and
    the `fetchKanbanCards` call inside `handleProcessComplete` — both fired
    regardless of whether the user ever opened the Kanban tab. This was the
    one "required at startup" concern flagged by the Set 0 audit.
  - Removed now-unused imports (`KanbanBoard`, `KanbanCard`, `invoke`, the
    `Kanban` icon) and the dead `cards` state. Cleaned up a stale comment
    that referenced "refreshes the board" after the fetch call it described
    was removed.
- **What was NOT changed**: `native/src/components/kanban/KanbanBoard.tsx`
  — untouched, still on disk. The `KanbanCard` type in
  `native/src/types/index.ts` — untouched (still used by `KanbanBoard.tsx`).
  The backend `get_kanban_cards` Tauri command and every `VaultManager`
  Kanban read/write method in `native/src-tauri` — untouched; `git diff
  --stat native/src-tauri/` is empty for this release. No settings changed
  (Kanban had none). Scribble Notes, Voice Chat, PTT, click-to-talk,
  hotkeys, STT, and text injection are unmodified.
- **Verification**: `native/` — `npm run build` (`tsc && vite build`)
  passes clean with zero errors; the production bundle now transforms 1609
  modules (down from 1610), confirming `KanbanBoard.tsx` is no longer
  bundled into the app. Repo-wide grep confirms zero remaining references
  to `KanbanBoard`/`get_kanban_cards` in `native/src` outside
  `KanbanBoard.tsx` and `types/index.ts` themselves. `git status` confirms
  only `native/src/App.tsx` changed under `native/`. No test suite covers
  this path (only Rust-side `#[cfg(test)]` unit tests exist, in
  `vault/mod.rs`, `capture/mod.rs`, `triggers/mod.rs` — none reference
  Kanban or App.tsx, and none were touched).

## [0.4.5] - 2026-08-20

### Scope-Reduction Set 0 (Audit) & Set 1 — Web Surface Marked Deferred

Relay is being reduced to a stable, focused desktop-first universal dictation
app (global PTT + click-to-talk + local STT + text injection through one
Dictation Pill). This is a documentation-only release: no application code
changed. Kanban, Voice Chat, TTS, Triggers/MCP, and the Dictation Pill
consolidation are explicitly **not** part of this release — each remains
gated behind its own future approval (see `docs/decisions.md` Decision 32).

- **Repository audit (Set 0, no changes)**: Read-only audit across Web,
  Kanban, Voice Chat, TTS, Triggers/MCP, the three pill components
  (`DictationPill.tsx`/`FloatingPill.tsx`/`PTTWidget.tsx`), Settings, and the
  core capture/hotkey/STT/injection path. Notable findings recorded for
  future sets rather than acted on now: Voice Chat is fully implemented and
  active (contradicting an assumption that it might not exist); the
  standalone "Dictation Indicator" window was already removed in a prior
  commit (Decision 30/PTT-013) and no longer exists in code; click-to-talk
  and the global PTT hotkey share the same backend capture session but
  diverge after transcription (only the hotkey path calls real OS text
  injection today; click-to-talk runs the scribble LLM-cleanup/vault-write
  pipeline instead); and the Triggers/MCP phrase-match check is inlined
  inside the same handler click-to-talk uses, a real (if small) coupling
  distinct from the trivially-removable Triggers settings tab.
- **Documentation (`docs/`)**:
  - Added Decision 32 to `docs/decisions.md`, recording the desktop-first
    scope reduction and deferring Web for the current phase — explicitly
    superseding Decision 3's "dual-surface" framing for this phase only,
    without altering Decision 3's historical text.
  - `docs/product.md`: moved the "Dual Surface (Native Desktop + Web
    Dashboard)" differentiator and the Next.js/Supabase MVP bullet out of
    active MVP scope into a new "Deferred for Current Phase" section.
  - `docs/requirements.md`: annotated FR-5.2 (Web Dashboard) as deferred for
    the current phase, requirement text preserved.
  - `docs/architecture.md`: annotated the Web Surface box in the
    three-surface diagram as deferred, and corrected the "Hybrid Cloud Mode"
    description — a repository audit found no Supabase code anywhere in
    `native/src-tauri` (zero matches, no dependency) and `web/`'s own
    Supabase client is a mocked stand-in returning hardcoded data, so this
    was previously describing an unbuilt, aspirational design as if it were
    implemented.
- **What was NOT changed**: No files under `web/`. No Rust or TypeScript
  source under `native/`. No settings, commands, build configuration, or
  navigation. No modification made to any existing working implementation —
  `web/` was already fully decoupled from the desktop build (no shared
  workspace config, no imports either direction) before this release.
- **Verification**: `native/` frontend — `npm install && npm run build`
  (`tsc && vite build`) completed cleanly with zero errors (one pre-existing,
  unrelated warning about a mixed static/dynamic import of
  `@tauri-apps/api/event.js`, present before this change). Rust backend —
  `cargo check` in this Linux container fails during dependency compilation
  at `gdk-sys v0.18.2`'s build script (`pkg-config` can't find `gdk-3.0` —
  the GTK/GDK system libraries Tauri's Linux windowing backend needs, e.g.
  `libgtk-3-dev`, which this container doesn't have installed), before any
  of Relay's own crates or Relay-authored Rust code are even reached. Relay
  targets Windows and this is a pre-existing container/environment gap, not
  a regression — this release touched zero Rust code, and the failure
  occurs entirely within a third-party dependency's native build step,
  upstream of anything this repository controls.

## [0.4.4] - 2026-08-20

### Real Speech Detection, Rolling Waveform & Dictation Lifecycle Hardening

A prior attempt at this fix (gating transcription on a `had_audio` flag set
by a fixed RMS threshold) landed real correctness improvements but was
independently found to be incomplete: the threshold couldn't tell a
sustained-but-silent noisy room apart from speech, the waveform still used
one scalar to scale a fixed decorative bar shape (never truly flat at
silence), and the click handler still declared "listening"/"processing"
before the native recorder confirmed either. This release addresses all
three, plus the docked-pill hotkey-visibility gap that fell out of the
same review.

- **Speech detection (`capture/mod.rs`)**:
  - Replaced the fixed `AUDIO_DETECTED_THRESHOLD` gate with a per-session
    calibration: the first 300ms of a recording measures the ambient noise
    floor (fan/room/mic-AGC noise), and only energy sustained for 200ms+
    *above that measured floor by a margin* counts towards `had_audio`.
    Fan noise, keyboard clatter, and Windows mic-enhancement processing
    sitting continuously above a static threshold no longer falsely
    triggers transcription.
  - Added unit tests (`capture::tests`) covering true silence, sustained
    ambient noise at a fixed level, sustained real speech above the
    calibrated floor, and a brief sub-threshold-duration spike — the exact
    regression scenario a static threshold couldn't distinguish.
- **Real waveform (`DictationPill.tsx`)**:
  - Replaced the fixed 15-value decorative shape scaled by one shared
    `audioLevel` scalar with a rolling per-bar history: each bar now
    renders its own actual recent audio-level sample. Silence now
    collapses every bar to its hairline minimum instead of a predetermined
    non-zero pattern.
- **Recording lifecycle (`DictationPill.tsx`)**:
  - Removed the remaining optimistic local state transitions: clicking to
    start no longer claims `listening` before `start_capture` resolves.
    The pill now exclusively reflects state the native recorder has
    confirmed via `capture-state-changed`, removing the last
    two-sources-of-truth race between the UI and the backend.
- **Docked hotkey visibility (`hotkeys/mod.rs`)**:
  - Pressing the dictation hotkey while docked (floating pill off) now
    shows the main window (without focusing it, so the actual dictation
    target keeps OS focus for text injection) and switches to the Voice
    Capture tab via the existing `navigate-tab` event, so a hotkey-triggered
    recording is actually visible instead of updating a pill that's hidden
    behind another tab or window.
  - `try_register_hotkeys` now registers the show/hide and dictation
    hotkeys independently; previously a failure on the first silently
    skipped ever attempting the second, which could leave the dictation
    hotkey completely unregistered with no visible error. A
    `hotkey-status-changed` event now carries the real per-hotkey
    registration outcome to the UI.

## [0.4.3] - 2026-08-20

### Dev Environment Setup & Workspace Install Scripts

- **Improvements**:
  - (`root`) Added `install:all` script to root `package.json` to install dependencies across both `native/` and `web/` workspaces in a single command.
  - (`native/`, `web/`) Configured and verified development dependencies and build pipelines for both native (Tauri + React + Vite) and web (Next.js + Turbopack) environments.

## [0.4.2] - 2026-08-20

### whisper-rs 0.16 Upgrade, Compilation Fixes & Open Settings Command

- **Fixes**:
  - (`native/src-tauri`) Upgraded `whisper-rs` from 0.14 to 0.16 and re-enabled the `whisper-local` feature as the default in `Cargo.toml` — local STT was silently disabled because the previous version's bindgen build failed on Windows without LLVM/libclang.
  - (`native/src-tauri`) Added `LIBCLANG_PATH` to `.cargo/config.toml` so whisper-rs-sys bindgen finds the LLVM installation on Windows.
  - (`native/src-tauri`) Imported `tauri::Manager` trait in `commands.rs` to fix `get_webview_window` not found on `AppHandle`.
  - (`native/src-tauri`) Updated `stt.rs` segment extraction to use the whisper-rs 0.16 iterator API (`state.as_iter()` + `segment.to_str_lossy()`) replacing the removed `full_n_segments`/`full_get_segment_text` methods.
- **Features**:
  - (`native/src-tauri`) Added `open_settings_window` Tauri command that surfaces the main window, focuses it, and emits a `navigate-tab` event to switch to the settings tab.
  - (`native/src-tauri`) Registered `open_settings_window` in the Tauri command handler list in `lib.rs`.
  - (`native/src`) Added `navigate-tab` event listener in `App.tsx` so the main window responds to tab-switch requests from the backend or overlay.
  - (`native/src`) Updated `DictationPill.tsx` error/warning banners to invoke `open_settings_window` instead of opening the local popover — directs users to the full settings page in the main window.
  - (`native/src`) Added an "Open All Settings in App" link at the bottom of `PillSettingsPopover.tsx` for quick access to the main settings panel.

## [0.4.1] - 2026-08-19

### Unconditional Automatic GGML Whisper Model Downloader

- **STT Model Auto-Fetch (`stt.rs`, `commands.rs`)**:
  - Removed feature gating from `ensure_default_model` in `stt.rs`.
  - Automatically fetches HuggingFace's `ggml-tiny.en.bin` model directly into `%APPDATA%\Relay\models\` on launch whenever a model path is unconfigured or missing on disk.
  - Automatically persists the downloaded model path into `settings.json`, transitioning the dictation pill seamlessly to `Click to dictate` with zero manual configuration.

## [0.4.0] - 2026-08-19

### STT Whisper Model Error Resolution & Interactive Configuration Handler

- **STT Model Validation (`commands.rs`, `stt.rs`)**:
  - Updated `ensure_stt_model_ready` to verify file existence on disk before declaring status ready.
  - Simplified STT missing model error message to a clean, actionable instruction (`Set Whisper model path in Provider Settings.`).
- **Interactive Error Action (`DictationPill.tsx`)**:
  - Formatted error banner text with truncation (`max-w-[260px]`) to prevent pill overflow.
  - Added click-to-configure interaction: clicking the error banner automatically opens the settings popover for one-click configuration.

## [0.3.9] - 2026-08-19

### Process Label Removal & Application Dark Theme Syncing

- **Process Label Removal (`DictationPill.tsx`)**:
  - Removed process indicator label (`● SNIPPING TOOL`) from the left side of the expanded dictation pill.
- **Application Dark Theme Color Synchronization (`DictationPill.tsx`, `PillSettingsPopover.tsx`)**:
  - Matched dark theme colors to Relay's exact neutral dark theme tokens (`#171717` dark card background, `#262626` dark border, `#fafafa` text), eliminating navy/slate color mismatch between the overlay pill and application dashboard.

## [0.3.8] - 2026-08-19

### Rounded-lg Component Geometry & Simultaneous Light/Dark/System Theme Syncing

- **Simultaneous Theme Synchronization (`ThemeToggle.tsx`, `DictationPill.tsx`)**:
  - Wired real-time theme syncing between the main dashboard window and the floating overlay dictation window.
  - Listens to `relay-theme-changed` events, `localStorage` theme state, and `prefers-color-scheme` media queries, toggling `.dark` class on root HTML elements simultaneously across all surfaces.
- **Rounded-lg Component Styling (`DictationPill.tsx`, `PillSettingsPopover.tsx`)**:
  - Updated pill body container, sparkle button (`✦`), settings chevron (`⌄`), audio waveform bars, hit zones, keyboard hint bar (`Hold to record`), and popover dropdown options to use `rounded-lg` / `rounded-xl` geometry.

## [0.3.7] - 2026-08-19

### Top Hint Clipping Fix & Relay Light Theme Palette Integration

- **Rust Overlay Window Expansion (`overlay.rs`)**:
  - Increased `EXPANDED_SIZE` height from `100.0` to `150.0` and `POPOVER_SIZE` height to `420.0` to eliminate horizontal top-clipping on the floating `Hold to record [Ctrl] [Space]` hint bar.
- **Relay Native Light Theme System (`DictationPill.tsx`, `PillSettingsPopover.tsx`)**:
  - Re-themed push-to-talk pill and settings popover using Relay's crisp light mode design system: pure white card background (`#ffffff`), Slate-900 typography (`#0f172a`), Relay primary blue (`#2563eb`) accents/waveforms/toggles, and subtle Slate-200 borders (`#e2e8f0`).

## [0.3.6] - 2026-08-19

### Murmur Push-to-Talk Pill Design & Edge-Flush Placement

- **Murmur Visual System Replication (`DictationPill.tsx`, `PillSettingsPopover.tsx`, `overlay.rs`)**:
  - Replicated exact Murmur paper gradients (`#faf8f3` -> `#efeae0`), box shadows, 13 idle dots, 15 terracotta waveform bars, toast notifications, and keyboard hint bar.
  - Made handle/notch 25% wider (`96px` width, `6px` height) with `border-radius: 999px 999px 0 0` (rounded-t-lg top part).
  - Fixed handle overlap bug by hiding handle (`opacity: 0`) whenever pill is expanded, recording, processing, or showing toast.
  - Updated Rust `overlay.rs` positioning so resting notch window anchors flush against the top edge of the taskbar / screen bottom without floating gap.
  - Added sub-page navigation in settings popover for Cleanup Style (`Faithful`/`Polished`/`Clean`/`Concise`) and Language (`Auto-detect`/`English (US)`/`Hinglish`/`Hindi`/`Español`).

## [0.3.5] - 2026-08-19

### Oscar-Inspired Push-to-Talk Pill Redesign & Interaction Refinement

- **Oscar Visual & Interaction Redesign (`DictationPill.tsx`, `PillSettingsPopover.tsx`, `overlay.rs`)**:
  - Removed middle-logo component overlap bug on expanded pill.
  - Replaced floating resting dot with a slim, edge-attached horizontal notch (`w-16 h-2`) when idle.
  - Added floating hotkey hint bar (`Hold to record [Ctrl] [Space]`) floating above the main pill on hover/activation.
  - Removed "RELAY" brand text in favor of minimal process indicators (`● SNIPPING TOOL`).
  - Added Oscar-style settings dropdown supporting Auto-paste, Text transform, Cleanup style (`Faithful`/`Clean`/`Professional`/`Concise`), Prompt mode (`Rewrite speech into a prompt`), and Language selection.
  - Added repository inspection docs [`docs/inspect/push-to-talk-pill.md`](file:///d:/Projects/Relay/docs/inspect/push-to-talk-pill.md) and decision log [`docs/decisions/push-to-talk-pill.md`](file:///d:/Projects/Relay/docs/decisions/push-to-talk-pill.md).

## [0.3.4] - 2026-08-19

### Native Build Fix for npm run dev:native

- **Windows Build Fix (`Cargo.toml`)**:
  - Gated optional `whisper-local` feature from `default = []` in `Cargo.toml` so `npm run dev:native` and `tauri dev` build cleanly on Windows environments without external `cmake` C++ build tools installed.

## [0.3.3] - 2026-08-19

### Multi-Monitor Active Positioning & Floating Pill Consolidation

- **Unified Floating Overlay Surface (`overlay.rs`, `hotkeys/mod.rs`)**:
  - Consolidated legacy `dictation-indicator` window into the unified `dictation-pill` overlay.
  - Implemented active-window monitor auto-detection so the floating pill appears on whichever display contains the user's active application target.
  - Hardened focus preservation and session locks across global hotkeys and overlay UI.

## [0.3.2] - 2026-08-19

### Push-to-Talk Floating Pill Upgrade

- **Push-to-Talk Overlay Redesign (`DictationPill.tsx`, `commands.rs`)**:
  - Bound overlay states directly to backend capture machine (`IDLE` → `LISTENING` → `TRANSCRIBING` → `SUCCESS` / `ERROR`).
  - Added real-time RMS microphone audio level calculations (`compute_rms_f32`) emitted at ~25Hz to drive overlay waveform animation.
  - Preserved zero-focus-theft properties (`focused(false)`) on floating overlay window for reliable OS text injection (`enigo`).
  - Recorded architectural decisions **PTT-001** through **PTT-012** in [`docs/decisions.md`](file:///d:/Projects/Relay/docs/decisions.md).

## [0.3.1] - 2026-08-19

### Model Management, Hotkey Recorder & Floating Overlay

- **Local Ollama & Model Manager (`ollama_manager.rs`, `ProviderSettings.tsx`)**:
  - Added local Ollama daemon detection, model status checking, and one-click model pulling (`llama3.2:latest`, `qwen2.5:latest`).
  - Added local Whisper GGML model selection and status monitoring (`ggml-tiny.en.bin`, `ggml-base.en.bin`).

- **Global Hotkey Recorder (`hotkeys/mod.rs`, `HotkeyRecorder.tsx`)**:
  - Added interactive Hotkey Recorder UI component allowing users to set custom key combinations for global dictation actions.
  - Bound global overlay toggle (`Ctrl+Shift+Space`) and push-to-talk dictation (`Ctrl+Space`) to OS text focus injection (`enigo`).

- **Always-on-Top Floating Overlay Window (`overlay.rs`, `FloatingPill.tsx`)**:
  - Created non-focus-stealing transparent native desktop overlay window for instant dictation state visualization.

- **In-App Categorized Release Notes (`ChangelogModal.tsx`, `changelog-dialog.tsx`)**:
  - Added 80% width modal layout with dual category tags (`Features`, `Fixes`, `Improvements`) and domain tags (`UI`, `LLM`, `Speech`, `Dictation`, `Kanban`, `Vault`, `Settings`, `Build`).

## [0.3.0] - 2026-08-19

### Universal Dictation, Global Hotkeys & Voice Chat (minor: new modules, major features)

- **Native Backend (`native/src-tauri/`)**:
  - **Features**: Added `hotkeys/` module — registers a global show/hide hotkey (`Ctrl+Shift+Space`) and a push-to-talk universal dictation hotkey (`Ctrl+Space`, both configurable) via `tauri-plugin-global-shortcut`; dictation types the transcript into whichever field has OS focus via the new `hotkeys::injection` (`enigo`) submodule, with a small always-on-top, non-focus-stealing "listening" indicator window.
  - **Features**: Added `pipeline::chat::process_chat` — in-app voice chat grounded in vault notes, with source attribution and optional spoken answers.
  - **Features**: Added `tts/` module — optional local text-to-speech via a user-configured Piper binary + voice model.
  - **Features**: Added `settings/` module — persists provider/STT/TTS/hotkey configuration to `.relay/config/settings.json`; wired new `get_settings`/`save_settings` commands.
  - **Improvements**: `capture/` now performs real microphone recording via `cpal` on a dedicated thread (resampled to 16kHz mono) instead of writing an empty WAV placeholder.
  - **Improvements**: Added `capture::stt::SttEngine` — real local transcription via `whisper-rs` (whisper.cpp), replacing the previous hardcoded fake transcript string. Model path is configurable; a clear error surfaces if unconfigured rather than silently faking output.
  - **Improvements**: `vault::VaultManager` gained `list_notes`/`search_notes` (keyword-ranked retrieval) as real, if interim, grounding for voice chat ahead of the LanceDB embedding pipeline `docs/decisions.md` already commits to.
  - **Improvements**: `stop_capture` now loads the persisted LLM provider config instead of always using hardcoded defaults, so Provider Settings actually take effect.
  - **Fixes**: Fixed `ProviderType` JSON serialization (`CloudOpenAI` etc. now serialize as `cloud_openai` etc., matching the frontend contract instead of silently mismatching on save/load).
  - **Fixes**: Fixed app icons (`icons/*.png`) not being RGBA, which made the Tauri build fail outright (`generate_context!` panic).
- **Native Frontend (`native/src/`)**:
  - **Features**: Added a "Voice Chat" tab (`components/chat/ChatPanel.tsx`) — record a question, see the grounded answer with sources, hear it spoken back if TTS is configured.
  - **Features**: Added `components/dictation/DictationIndicator.tsx`, rendered via a `#/dictation-indicator` hash route in the same bundle for the new indicator window.
  - **Improvements**: `ProviderSettings.tsx` now actually loads/saves settings via `get_settings`/`save_settings` (previously local-only UI state that did nothing), and gained STT model path, TTS binary/voice path, and hotkey configuration sections.
  - **Improvements**: Extended `ProcessedPipelineResult`/`AppSettings` types to match the backend.
- **Docs (`docs/`)**: Recorded Decisions 13–16 (universal dictation & hotkeys, real local STT, in-app voice chat/RAG-lite, optional local TTS); updated product/requirements/data-model/api/architecture/user-flows docs to match; added `docs/roadmap.md` tracking remaining competitive-research gaps (LanceDB vector RAG, real MCP client wiring, speaker diarization, multi-user).
- **Repo hygiene**: Fixed an unresolved git merge-conflict left in `README.md` from the initial boilerplate merge.
- **Note**: Merged on top of the `0.2.0` visual identity pass below; `ProviderSettings.tsx` reconciles both — the new sub-navigated Settings shell now carries this round's STT/TTS/hotkey sections instead of the flatter layout originally shipped with those fields, and `providers::LLMClient` keeps this round's `ProviderType` serde fix alongside `0.2.0`'s offline heuristic fallback.

## [0.2.0] - 2026-08-19

### Relay Visual Identity Pass ("Monochrome & Electric Blue")

- **Brand Tokens (`design-system.md`)**:
  - Repointed CSS variables across `:root` and `.dark` in [`native/src/index.css`](file:///d:/Projects/Relay/native/src/index.css), [`native/tailwind.config.cjs`](file:///d:/Projects/Relay/native/tailwind.config.cjs), and [`web/src/app/globals.css`](file:///d:/Projects/Relay/web/src/app/globals.css) to the Monochrome & Electric Blue palette (`#2563EB` light / `#60A5FA` dark).
  - Introduced 3-way semantic colors (`--success`, `--warning`, `--destructive`) and `--border-strong` tokens across light and dark modes.

- **Relay Logo (`RelayLogo`)**:
  - Built reusable SVG logo mark with asymmetric two-tone "E" in [`RelayLogo.tsx`](file:///d:/Projects/Relay/native/src/components/common/RelayLogo.tsx) (native) and [`relay-logo.tsx`](file:///d:/Projects/Relay/web/src/components/relay-logo.tsx) (web).
  - Integrated mark into native sidebar header, web sidebar ([`app-sidebar.tsx`](file:///d:/Projects/Relay/web/src/components/app-sidebar.tsx)), web login card ([`login-form.tsx`](file:///d:/Projects/Relay/web/src/components/login-form.tsx)), and web favicon ([`icon.tsx`](file:///d:/Projects/Relay/web/src/app/icon.tsx)).

- **Floating Dictation Pill (`DictationPill.tsx`)**:
  - Rebuilt push-to-talk experience as a floating dictation pill overlay following the Murmur interaction model with ~180ms hover-hold handle, state machine (`rest → ready → expanded → recording → processing → inserted/error → rest`), audio waveform keyframes, rotating mono processing captions, mode switch button (**Meeting → Kanban** vs **Voice Scribble**), and engine settings popover.
  - Added local heuristic fallback to Rust LLMClient ([`providers/mod.rs`](file:///d:/Projects/Relay/native/src-tauri/src/providers/mod.rs)) so dictation works reliably even when local Ollama is offline.

- **App Shell, Settings & Content Screens**:
  - Standardized top-level Hero header pattern across native and web views (`Today, Nitin captured.`, `How Relay behaves.`, etc.).
  - Restructured native [`ProviderSettings.tsx`](file:///d:/Projects/Relay/native/src/components/settings/ProviderSettings.tsx) and created web [`settings/page.tsx`](file:///d:/Projects/Relay/web/src/app/(dashboard)/settings/page.tsx) with domain sub-navs and Data & Privacy controls.
  - Restructured native [`ScribbleViewer.tsx`](file:///d:/Projects/Relay/native/src/components/scribble/ScribbleViewer.tsx) and created web [`notes/page.tsx`](file:///d:/Projects/Relay/web/src/app/(dashboard)/notes/page.tsx) with master list + detail pane, action toolbar pill buttons, and local-vault reassurance line.
  - Unified native and web Kanban boards with 3-way semantic priority badges.

## [0.1.2] - 2026-08-19

### Complete UI Design Pass & Theme System Refactoring

- **Design System (`design-system.md`)**:
  - Established Relay's signature **Calm Emerald-Teal & Slate Focus Palette** (`hsl(173, 70%, 38%)` primary teal, obsidian slate dark mode, clean soft slate light mode).
  - Defined CSS custom properties (`--primary`, `--background`, `--card`, `--border`, `--muted`, `--accent`, `--ring`) in both [`native/src/index.css`](file:///d:/Projects/Relay/native/src/index.css) and [`web/src/app/globals.css`](file:///d:/Projects/Relay/web/src/app/globals.css).
  - Replaced all hardcoded hex and ad-hoc Tailwind colors with theme token classes (`bg-primary`, `bg-card`, `bg-muted`, `border-border`, `text-foreground`, `text-muted-foreground`).

- **Native Capture Widget (`PTTWidget.tsx`)**:
  - Rebuilt mic push-to-talk button, mode switcher (Meeting vs Scribble), and recording state.
  - Implemented dynamic live-audio level meter visualizer (`animate-audio-bar-*`) during speech recording.
  - Added error fallback banner with retry affordance and WCAG AA contrast compliance.

- **Unified Kanban Board (`KanbanBoard.tsx` & `(dashboard)/page.tsx`)**:
  - Unified desktop native and web dashboard Kanban boards into one cohesive visual design language.
  - Added loading skeletons ([`native/src/components/ui/skeleton.tsx`](file:///d:/Projects/Relay/native/src/components/ui/skeleton.tsx)), responsive column layout grids, priority badges, and empty-state placeholders.

- **Vault Notes, Settings & Auth**:
  - Refactored [`ScribbleViewer`](file:///d:/Projects/Relay/native/src/components/scribble/ScribbleViewer.tsx), [`ProviderSettings`](file:///d:/Projects/Relay/native/src/components/settings/ProviderSettings.tsx), and [`TriggerSettings`](file:///d:/Projects/Relay/native/src/components/settings/TriggerSettings.tsx) with design tokens and accessibility attributes (`aria-label`, `aria-live`, explicit `htmlFor` bindings).
  - Refactored Web Dashboard [`LoginPage`](file:///d:/Projects/Relay/web/src/app/login/page.tsx), [`LoginForm`](file:///d:/Projects/Relay/web/src/components/login-form.tsx), and [`AppSidebar`](file:///d:/Projects/Relay/web/src/components/app-sidebar.tsx) with Relay branding.

## [0.1.1] - 2026-08-19

### Improvements & UI Refactoring

- **Native Frontend (`native/`)**:
  - Installed Radix UI primitives, `clsx`, `tailwind-merge`, and `class-variance-authority`.
  - Created `native/src/lib/utils.ts` `cn` helper function and shadcn primitives (`Button`, `Card`, `Badge`, `Input`).

## [0.1.0] - 2026-08-19

### Initial Release — Multi-Surface Architecture & Core Pipeline

- Initial scaffold of living specifications (`docs/`), native desktop app (`native/`), and hybrid Next.js web dashboard (`web/`).
