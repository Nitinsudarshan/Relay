# Graph Report - Relay  (2026-08-20)

## Corpus Check
- 156 files · ~73,235 words
- Verdict: corpus is large enough that graph structure adds value.

## Summary
- 1240 nodes · 2258 edges · 129 communities (73 shown, 56 thin omitted)
- Extraction: 100% EXTRACTED · 0% INFERRED · 0% AMBIGUOUS · INFERRED: 2 edges (avg confidence: 0.8)
- Token cost: 0 input · 0 output

## Graph Freshness
- Built from commit: `e60c4837`
- Run `git rev-parse HEAD` and compare to check if the graph is stale.
- Run `graphify update .` after code changes (no API cost).

## Community Hubs (Navigation)
- components/page.tsx
- commands.rs
- capture/mod.rs
- VaultManager
- Relay — Push-to-Talk Pill Redesign & Interaction Overhaul
- cn
- sidebar.tsx
- LLMClient
- Relay — Architectural & Product Decision Log
- compilerOptions
- web/src/lib/utils.ts
- dropdown-menu.tsx
- hotkeys/mod.rs
- compilerOptions
- tauri.conf.json
- ProviderSettings.tsx
- (dashboard)/layout.tsx
- DictationPill.tsx
- components.json
- devDependencies
- SttEngine
- site-header.tsx
- triggers/mod.rs
- devDependencies
- dependencies
- dependencies
- TriggerSettings.tsx
- index.ts
- settings/mod.rs
- reposition
- Command Signatures
- scripts
- app/layout.tsx
- .dispatch_action
- ScribbleViewer.tsx
- AGENTS.md
- Decisions
- native/package.json
- default.json
- Charts, Graphs, and Data Visualizations
- Functional Requirements
- Relay — User Flows
- .synthesize
- web/package.json
- Relay — Changelog
- Relay — Product Specification
- 3. First task — repository reconnaissance
- popover.tsx
- component-architecture.md
- Relay — Data Model Specification
- loading-view.tsx
- Relay AI Agent Guidelines
- App.tsx
- Relay
- icon.tsx
- Relay — System Architecture
- Relay — Roadmap & Competitive Gap Backlog
- Relay — Testing Strategy
- inject_text
- Code Standards (Frontend)
- Global Rules
- Push-to-Talk Pill Update Report (v0.4.1)
- shadcn
- API Conventions
- Data Access Rules
- Code Documentation Rules
- Performance Rules
- RBAC — Not Built, and Why
- Testing Rules
- Push-to-Talk Pill Inspection
- forms-and-validation.md
- responsive-design.md
- cn
- security.md
- server-client-boundary.md
- ui-components.md
- version-and-changelog.md
- Input.tsx
- rules/graphify.md
- workflows/graphify.md
- [0.1.1] - 2026-08-19
- [0.1.2] - 2026-08-19
- [0.2.0] - 2026-08-19
- [0.3.0] - 2026-08-19
- [0.3.2] - 2026-08-19
- [0.3.3] - 2026-08-19
- [0.3.4] - 2026-08-19
- [0.3.5] - 2026-08-19
- [0.3.6] - 2026-08-19
- [0.3.7] - 2026-08-19
- rust-backend.md
- [0.3.9] - 2026-08-19
- [0.4.0] - 2026-08-19
- [0.4.1] - 2026-08-19
- [0.1.0] - 2026-08-19
- [0.4.3] - 2026-08-20
- [0.4.4] - 2026-08-20
- [0.4.5] - 2026-08-20
- [0.4.7] - 2026-08-20
- [0.4.8] - 2026-08-20
- [0.5.0] - 2026-08-20
- [0.3.8] - 2026-08-19
- 6. RESTING STATE
- [0.3.1] - 2026-08-19
- [0.4.6] - 2026-08-20
- @radix-ui/react-checkbox
- @radix-ui/react-label
- @radix-ui/react-select
- lucide-react
- @radix-ui/react-tooltip
- react
- @tauri-apps/api
- next
- radix-ui
- code-standards.md
- ts-node
- next.config.ts
- next-env.d.ts
- @types/react
- postcss.config.mjs
- globals.d.ts
- [0.4.2] - 2026-08-20
- @radix-ui/react-slot

## God Nodes (most connected - your core abstractions)
1. `cn()` - 137 edges
2. `Relay — Push-to-Talk Pill Redesign & Interaction Overhaul` - 42 edges
3. `Relay — Architectural & Product Decision Log` - 38 edges
4. `AppState` - 29 edges
5. `CommandError` - 28 edges
6. `Relay — Changelog` - 28 edges
7. `VaultManager` - 25 edges
8. `compilerOptions` - 18 edges
9. `cn()` - 17 edges
10. `compilerOptions` - 16 edges

## Surprising Connections (you probably didn't know these)
- `on_dictation_pressed()` --calls--> `emit_capture_state()`  [INFERRED]
  native/src-tauri/src/hotkeys/mod.rs → native/src-tauri/src/commands.rs
- `stop_dictation_session()` --calls--> `emit_capture_status_event()`  [INFERRED]
  native/src-tauri/src/hotkeys/mod.rs → native/src-tauri/src/commands.rs
- `PopoverContent()` --calls--> `cn()`  [EXTRACTED]
  native/src/components/ui/popover.tsx → native/src/lib/utils.ts
- `AvatarBadge()` --calls--> `cn()`  [EXTRACTED]
  web/src/components/ui/avatar.tsx → web/src/lib/utils.ts
- `AvatarGroup()` --calls--> `cn()`  [EXTRACTED]
  web/src/components/ui/avatar.tsx → web/src/lib/utils.ts

## Import Cycles
- None detected.

## Communities (129 total, 56 thin omitted)

### Community 0 - "components/page.tsx"
Cohesion: 0.07
Nodes (32): Accordion(), AccordionContent(), AccordionItem(), AccordionTrigger(), Checkbox(), Dialog(), DialogContent(), DialogDescription() (+24 more)

### Community 1 - "commands.rs"
Cohesion: 0.14
Nodes (53): AppSettings, HotkeySettings, KanbanCard, AppState, CaptureStatus, ChangelogEntry, ChangelogItem, choose_vault_folder() (+45 more)

### Community 2 - "capture/mod.rs"
Cohesion: 0.11
Nodes (35): Fn, Instant, ActiveSession, AudioDetectionState, AudioRecorder, brief_spike_shorter_than_min_duration_does_not_trigger_had_audio(), CapturedAudio, CaptureError (+27 more)

### Community 3 - "VaultManager"
Cohesion: 0.17
Nodes (23): process_chat(), ProcessedPipelineResult, Result, TtsSettings, KanbanCard, parse_debug_string_list(), Error, Mutex (+15 more)

### Community 4 - "Relay — Push-to-Talk Pill Redesign & Interaction Overhaul"
Cohesion: 0.05
Nodes (40): 0. Role / execution mode, 10. Hover-out delay, 11. Settings popover, 12. Settings rule — NO FAKE CONTROLS, 13. Dummy-data / placeholder detection, 14. Ollama status MUST be truthful, 15. Whisper status MUST also be truthful, 16. Prompt Mode (+32 more)

### Community 5 - "cn"
Cohesion: 0.09
Nodes (32): Field(), FieldContent(), FieldDescription(), FieldError(), FieldGroup(), FieldLabel(), FieldLegend(), FieldSeparator() (+24 more)

### Community 6 - "sidebar.tsx"
Cohesion: 0.09
Nodes (39): data, ChangelogDialog(), NavItem, NavMain(), NavSecondary(), NavUser(), Collapsible(), CollapsibleContent() (+31 more)

### Community 7 - "LLMClient"
Cohesion: 0.14
Nodes (21): Client, ExtractedActionItem, PipelineEngine, PipelineError, ProcessedPipelineResult, Error, Option, Result (+13 more)

### Community 8 - "Relay — Architectural & Product Decision Log"
Cohesion: 0.05
Nodes (38): Decision 10: Trigger Phrases are User-Customizable, Decision 11: Target Build Environment, Decision 13: Universal Dictation & Global Hotkeys, Decision 14: Real Local Speech-to-Text (whisper-rs), Decision 15: In-App Voice Chat, Grounded in Vault Notes (RAG-lite for now), Decision 16: Optional Local Text-to-Speech (Piper), Decision 17: Hybrid-Mode Architecture, Decision 18 (PTT-001): Preserve Backend Ownership of Capture State (+30 more)

### Community 9 - "compilerOptions"
Cohesion: 0.07
Nodes (28): esnext, **/*.mts, .next/dev/types/**/*.ts, next-env.d.ts, .next/types/**/*.ts, node_modules, **/*.ts, **/*.tsx (+20 more)

### Community 10 - "web/src/lib/utils.ts"
Cohesion: 0.09
Nodes (24): DEMO_NOTES, ScribbleNote, DashboardPage(), SettingsSection, CHANGELOG_DATA, ChangelogDialogProps, AuthMode, RelayLogo() (+16 more)

### Community 11 - "dropdown-menu.tsx"
Cohesion: 0.13
Nodes (21): HeaderUserMenu(), toTitleCase(), Avatar(), AvatarBadge(), AvatarFallback(), AvatarGroup(), AvatarGroupCount(), AvatarImage() (+13 more)

### Community 12 - "hotkeys/mod.rs"
Cohesion: 0.17
Nodes (23): Duration, apply_hotkeys(), DictationState, HotkeyRegistrationStatus, on_dictation_pressed(), on_dictation_released(), register_hotkeys(), AppHandle (+15 more)

### Community 13 - "compilerOptions"
Cohesion: 0.08
Nodes (23): compilerOptions, allowImportingTsExtensions, baseUrl, isolatedModules, jsx, lib, module, moduleResolution (+15 more)

### Community 14 - "tauri.conf.json"
Cohesion: 0.09
Nodes (22): app, security, windows, build, beforeBuildCommand, beforeDevCommand, devUrl, frontendDist (+14 more)

### Community 15 - "ProviderSettings.tsx"
Cohesion: 0.20
Nodes (11): eventToAccelerator(), HotkeyRecorder(), HotkeyRecorderProps, MODIFIER_KEYS, NAMED_KEYS, normalizeKey(), DEFAULT_SETTINGS, ProviderSettings() (+3 more)

### Community 16 - "(dashboard)/layout.tsx"
Cohesion: 0.15
Nodes (15): DashboardLayout(), AppSidebar(), SiteHeader(), SidebarInset(), User, UserContext, UserProvider(), auth() (+7 more)

### Community 17 - "DictationPill.tsx"
Cohesion: 0.22
Nodes (17): CaptureStatePayload, DictationPillProps, PROCESSING_CAPTIONS, SILENT_LEVEL_HISTORY, LANG_LABELS, PillSettingsPopover(), PillSettingsPopoverProps, STYLE_LABELS (+9 more)

### Community 18 - "components.json"
Cohesion: 0.10
Nodes (19): aliases, components, hooks, lib, ui, utils, iconLibrary, registries (+11 more)

### Community 19 - "devDependencies"
Cohesion: 0.11
Nodes (19): autoprefixer, devDependencies, autoprefixer, postcss, tailwindcss, @tauri-apps/cli, @types/react, @types/react-dom (+11 more)

### Community 20 - "SttEngine"
Cohesion: 0.18
Nodes (15): c_int, ensure_default_model(), num_cpus(), Arc, Default, Mutex, Option, Path (+7 more)

### Community 21 - "site-header.tsx"
Cohesion: 0.27
Nodes (9): ModeToggle(), HeaderUserMenu, Breadcrumb(), BreadcrumbEllipsis(), BreadcrumbItem(), BreadcrumbLink(), BreadcrumbList(), BreadcrumbPage() (+1 more)

### Community 22 - "triggers/mod.rs"
Cohesion: 0.24
Nodes (13): Error, Option, Path, Result, String, Vec, test_default_triggers(), test_match_transcript() (+5 more)

### Community 23 - "devDependencies"
Cohesion: 0.12
Nodes (17): eslint, eslint-config-next, @tailwindcss/postcss, tw-animate-css, @types/node, devDependencies, eslint, eslint-config-next (+9 more)

### Community 24 - "dependencies"
Cohesion: 0.12
Nodes (17): dependencies, class-variance-authority, clsx, @radix-ui/react-dialog, @radix-ui/react-dropdown-menu, @radix-ui/react-tabs, react-dom, tailwind-merge (+9 more)

### Community 25 - "dependencies"
Cohesion: 0.12
Nodes (17): next-themes, sonner, dependencies, class-variance-authority, clsx, lucide-react, next-themes, react (+9 more)

### Community 26 - "TriggerSettings.tsx"
Cohesion: 0.27
Nodes (7): ChatTurn, Card, CardContent, CardDescription, CardFooter, CardHeader, CardTitle

### Community 27 - "index.ts"
Cohesion: 0.13
Nodes (16): countWords(), formatNoteTimestamp(), VaultViewState, VoiceNotePage(), ChangelogEntry, ChangelogItem, HotkeySettings, PillPosition (+8 more)

### Community 28 - "settings/mod.rs"
Cohesion: 0.22
Nodes (15): AppSettings, HotkeySettings, PillPosition, Default, Error, Option, Path, Result (+7 more)

### Community 29 - "reposition"
Cohesion: 0.41
Nodes (12): Monitor, active_monitor(), compute_anchor(), ensure_pill_window(), reposition(), reposition_pill(), AppHandle, Option (+4 more)

### Community 30 - "Command Signatures"
Cohesion: 0.17
Nodes (11): 1. Tauri Commands API (`native/src-tauri/src/commands.rs`), 2. Web Route Handlers (`web/src/app/api/...`), Capture Commands, Command Signatures, Common Response Shape, Kanban Commands, Pipeline Commands, Relay — API Conventions & Specifications (+3 more)

### Community 31 - "scripts"
Cohesion: 0.17
Nodes (11): name, private, scripts, build, build:native, build:web, dev, dev:native (+3 more)

### Community 32 - "app/layout.tsx"
Cohesion: 0.21
Nodes (6): geistMono, geistSans, metadata, LoginForm(), ThemeProvider(), Toaster()

### Community 33 - ".dispatch_action"
Cohesion: 0.53
Nodes (5): McpError, McpRouter, McpToolCallResult, Result, String

### Community 34 - "ScribbleViewer.tsx"
Cohesion: 0.29
Nodes (6): DEMO_NOTES, ScribbleNote, ScribbleViewer(), ScribbleViewerProps, Input, InputProps

### Community 35 - "AGENTS.md"
Cohesion: 0.20
Nodes (6): Accessibility Rules, Rules, Design System Rules, Rules, Project Folder Structure, Rules

### Community 36 - "Decisions"
Cohesion: 0.20
Nodes (9): Context, Decision 1: Oscar-Style Edge-Attached Horizontal Notch, Decision 2: Removal of Heavy "RELAY" Branding Text, Decision 3: Floating Hotkey Hint Bar (`Hold to record [Ctrl] [Space]`), Decision 4: Smooth Hover State Transitions with Intent & Grace Delays, Decision 5: Real Settings in Dropdown Surface, Decision Log — Push-to-Talk Pill Redesign & Interaction Refinement, Decisions (+1 more)

### Community 37 - "native/package.json"
Cohesion: 0.20
Nodes (9): name, private, scripts, build, dev, preview, tauri, type (+1 more)

### Community 38 - "default.json"
Cohesion: 0.20
Nodes (9): description, identifier, permissions, $schema, windows, core:default, core:event:default, dictation-pill (+1 more)

### Community 39 - "Charts, Graphs, and Data Visualizations"
Cohesion: 0.20
Nodes (9): 1 Color (`--color-chart-primary` only), 2 Colors (`--color-chart-primary` + `--color-chart-accent-1`), 3 Colors (`--color-chart-primary` + `--color-chart-accent-1` + `--color-chart-accent-2`), 4+ Colors, Charts, Graphs, and Data Visualizations, Color Tokens, Hard Rules, How Many Colors to Use (+1 more)

### Community 40 - "Functional Requirements"
Cohesion: 0.22
Nodes (8): 1. Audio Capture & Speech-to-Text (STT), 2. Processing Pipeline, 3. Storage & Vault, 4. Provider Abstraction, 5. Dual Surfaces, Functional Requirements, Non-Functional Requirements, Relay — Functional & Non-Functional Requirements

### Community 41 - "Relay — User Flows"
Cohesion: 0.22
Nodes (8): Flow 1: Push-to-Talk Capture to Kanban Card, Flow 2: Audio Scribble to Structured Markdown Note, Flow 3: Configurable Trigger Phrase Execution, Flow 4: Hybrid Mode Web Dashboard Access, Flow 5: Global Show/Hide Hotkey, Flow 6: Universal Dictation (Type Anywhere), Flow 7: Voice Chat Over Vault Notes, Relay — User Flows

### Community 42 - ".synthesize"
Cohesion: 0.33
Nodes (7): Error, Option, Result, String, TtsSettings, TtsEngine, TtsError

### Community 43 - "web/package.json"
Cohesion: 0.22
Nodes (8): name, private, scripts, build, dev, lint, start, version

### Community 44 - "Relay — Changelog"
Cohesion: 0.25
Nodes (7): [0.6.0] - 2026-08-20, [0.7.0] - 2026-08-20, [0.7.1] - 2026-08-20, Dynamic Changelog, 1-Click Theme Toggle & UI Streamlining, Relay — Changelog, Toggle-to-Talk — Optional Press-Once Dictation Mode, Voice Note — Universal Dictation History & Configurable Vault Directory Location

### Community 45 - "Relay — Product Specification"
Cohesion: 0.25
Nodes (7): Core Value Proposition & Competitive Differentiators, Deferred for Current Phase, In Scope for MVP, Out of Scope for MVP, Overview, Relay — Product Specification, Target User

### Community 46 - "3. First task — repository reconnaissance"
Cohesion: 0.25
Nodes (8): 3. First task — repository reconnaissance, AI, Audio, Desktop positioning, Hotkey, Packaging, Pill, Settings

### Community 47 - "popover.tsx"
Cohesion: 0.33
Nodes (3): PopoverContent(), PopoverContext, PopoverProps

### Community 49 - "Relay — Data Model Specification"
Cohesion: 0.29
Nodes (6): 1. Vault Markdown Note Schema, 2. Kanban Card Schema, 3. Trigger Phrase Configuration Schema (`triggers.json`), 4. App Settings Configuration Schema (`settings.json`), 5. LanceDB Vector Record Schema, Relay — Data Model Specification

### Community 51 - "Relay AI Agent Guidelines"
Cohesion: 0.33
Nodes (6): Knowledge Graph, Precedence, Relay AI Agent Guidelines, Rule Index, Safety Requirements, Starting State

### Community 52 - "App.tsx"
Cohesion: 0.20
Nodes (9): App(), TAB_LABELS, DictationPill(), FloatingPill(), RelayLogo(), RelayLogoProps, ThemeMode, ThemeToggle() (+1 more)

### Community 53 - "Relay"
Cohesion: 0.33
Nodes (5): Getting Started (native desktop app), Getting Started (web dashboard), License, Relay, Structure

### Community 54 - "icon.tsx"
Cohesion: 0.33
Nodes (4): alt, contentType, runtime, size

### Community 55 - "Relay — System Architecture"
Cohesion: 0.40
Nodes (4): Data Access & Security Model, Relay — System Architecture, Rust Backend Module Design (`native/src-tauri/src/`), Three-Surface Overview

### Community 56 - "Relay — Roadmap & Competitive Gap Backlog"
Cohesion: 0.40
Nodes (4): Explicitly not gaps (already real), Relay — Roadmap & Competitive Gap Backlog, Shipped this round (real, not mocked), Still stubbed / not real yet — prioritized backlog

### Community 57 - "Relay — Testing Strategy"
Cohesion: 0.40
Nodes (4): 1. Rust Backend Unit & Integration Tests (`native/src-tauri/`), 2. React Native Desktop UI Tests (`native/src/`), 3. Web Dashboard Tests (`web/`), Relay — Testing Strategy

### Community 58 - "inject_text"
Cohesion: 0.50
Nodes (4): inject_text(), InjectionError, Result, String

### Community 59 - "Code Standards (Frontend)"
Cohesion: 0.40
Nodes (4): Code Standards (Frontend), Imports, Naming conventions, Rules

### Community 60 - "Global Rules"
Cohesion: 0.40
Nodes (4): Global Rules, Precedence, Rule files, Scope

### Community 61 - "Push-to-Talk Pill Update Report (v0.4.1)"
Cohesion: 0.40
Nodes (4): Files Modified, Push-to-Talk Pill Update Report (v0.4.1), Verification Results, What Changed

### Community 63 - "API Conventions"
Cohesion: 0.50
Nodes (3): API Conventions, Tauri commands (native/src-tauri/src/commands.rs), web/ route handlers (if any are needed beyond Server Actions)

### Community 64 - "Data Access Rules"
Cohesion: 0.50
Nodes (3): Cloud storage (Supabase) — hybrid mode only, Data Access Rules, Local storage (markdown vault + LanceDB) — local-only mode, always available

### Community 65 - "Code Documentation Rules"
Cohesion: 0.50
Nodes (3): Code Documentation Rules, Rust format, TypeScript/React format

### Community 66 - "Performance Rules"
Cohesion: 0.50
Nodes (3): Frontend (native/src/ and web/src/), Performance Rules, Rust backend (native/src-tauri/)

### Community 67 - "RBAC — Not Built, and Why"
Cohesion: 0.50
Nodes (3): If the team/sharing direction is ever picked up, RBAC — Not Built, and Why, Why this is deliberately absent

### Community 68 - "Testing Rules"
Cohesion: 0.50
Nodes (3): Frontend (native/src/ and web/src/), Rust backend (native/src-tauri/), Testing Rules

### Community 73 - "cn"
Cohesion: 0.20
Nodes (12): ChangelogModal(), ChangelogModalProps, KanbanBoardProps, Badge(), BadgeProps, badgeVariants, Button, ButtonProps (+4 more)

## Knowledge Gaps
- **428 isolated node(s):** `name`, `private`, `version`, `type`, `dev` (+423 more)
  These have ≤1 connection - possible missing edges or undocumented components.
- **56 thin communities (<3 nodes) omitted from report** — run `graphify query` to explore isolated nodes.

## Suggested Questions
_Questions this graph is uniquely positioned to answer:_

- **Why does `cn()` connect `cn` to `components/page.tsx`, `sidebar.tsx`, `web/src/lib/utils.ts`, `dropdown-menu.tsx`, `(dashboard)/layout.tsx`, `site-header.tsx`?**
  _High betweenness centrality (0.020) - this node is a cross-community bridge._
- **Why does `VaultManager` connect `VaultManager` to `commands.rs`, `LLMClient`?**
  _High betweenness centrality (0.016) - this node is a cross-community bridge._
- **Why does `AppState` connect `commands.rs` to `capture/mod.rs`, `VaultManager`, `SttEngine`?**
  _High betweenness centrality (0.014) - this node is a cross-community bridge._
- **What connects `name`, `private`, `version` to the rest of the system?**
  _428 weakly-connected nodes found - possible documentation gaps or missing edges._
- **Should `components/page.tsx` be split into smaller, more focused modules?**
  _Cohesion score 0.0673758865248227 - nodes in this community are weakly interconnected._
- **Should `commands.rs` be split into smaller, more focused modules?**
  _Cohesion score 0.1385359951603146 - nodes in this community are weakly interconnected._
- **Should `capture/mod.rs` be split into smaller, more focused modules?**
  _Cohesion score 0.11265969802555169 - nodes in this community are weakly interconnected._