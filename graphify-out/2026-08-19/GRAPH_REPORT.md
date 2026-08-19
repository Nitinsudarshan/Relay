# Graph Report - Relay  (2026-08-19)

## Corpus Check
- 159 files · ~100,552 words
- Verdict: corpus is large enough that graph structure adds value.

## Summary
- 1380 nodes · 2268 edges · 128 communities (96 shown, 32 thin omitted)
- Extraction: 100% EXTRACTED · 0% INFERRED · 0% AMBIGUOUS · INFERRED: 2 edges (avg confidence: 0.8)
- Token cost: 0 input · 0 output

## Graph Freshness
- Built from commit: `14824f38`
- Run `git rev-parse HEAD` and compare to check if the graph is stale.
- Run `graphify update .` after code changes (no API cost).

## Community Hubs (Navigation)
- settings/mod.rs
- devDependencies
- dropdown-menu.tsx
- VaultManager
- compilerOptions
- commands.rs
- app/layout.tsx
- cn
- compilerOptions
- tauri.conf.json
- AudioRecorder
- App.tsx
- components.json
- LLMClient
- dependencies
- triggers/mod.rs
- dependencies
- devDependencies
- definitions
- definitions
- components/page.tsx
- properties
- site-header.tsx
- Relay — Architectural & Product Decision Log
- properties
- Command Signatures
- permissions
- .dispatch_action
- roles.ts
- Relay — Push-to-Talk Pill Redesign & Interaction Overhaul
- AGENTS.md
- webviews
- webviews
- Charts, Graphs, and Data Visualizations
- Functional Requirements
- CapabilityRemote
- CapabilityRemote
- scripts
- web/package.json
- loading-view.tsx
- Relay
- Target
- login-form.tsx
- Relay — Data Model Specification
- Relay — Product Specification
- permissions
- Relay AI Agent Guidelines
- Relay — User Flows
- Capability
- Capability
- icon.tsx
- reposition
- Relay — System Architecture
- Relay — Testing Strategy
- desktop-schema.json
- windows-schema.json
- Code Standards (Frontend)
- Global Rules
- Relay — Changelog
- local
- local
- API Conventions
- Data Access Rules
- Code Documentation Rules
- Performance Rules
- RBAC — Not Built, and Why
- Testing Rules
- component-architecture.md
- server-client-boundary.md
- Target
- Input.tsx
- forms-and-validation.md
- responsive-design.md
- sidebar.tsx
- security.md
- sheet.tsx
- ui-components.md
- version-and-changelog.md
- ShellScopeEntryAllowedArgs
- rules/graphify.md
- workflows/graphify.md
- next
- code-standards.md
- SttEngine
- ts-node
- next.config.ts
- @types/react
- next-env.d.ts
- postcss.config.mjs
- globals.d.ts
- radix-ui
- definitions
- properties
- hotkeys/mod.rs
- native/package.json
- webviews
- CapabilityRemote
- .synthesize
- permissions
- Capability
- Relay — Roadmap & Competitive Gap Backlog
- linux-schema.json
- inject_text
- local
- rust-backend.md
- shadcn
- 3. First task — repository reconnaissance
- HotkeyRecorder.tsx
- @radix-ui/react-select
- @radix-ui/react-slot
- @radix-ui/react-tooltip
- react
- @tauri-apps/api
- @radix-ui/react-label
- Identifier
- (dashboard)/page.tsx
- 6. RESTING STATE
- Decisions
- Push-to-Talk Pill Update Report (v0.4.1)
- @radix-ui/react-checkbox
- lucide-react
- Push-to-Talk Pill Inspection

## God Nodes (most connected - your core abstractions)
1. `cn()` - 137 edges
2. `Relay — Push-to-Talk Pill Redesign & Interaction Overhaul` - 42 edges
3. `Relay — Architectural & Product Decision Log` - 31 edges
4. `AppState` - 24 edges
5. `CommandError` - 19 edges
6. `compilerOptions` - 18 edges
7. `cn()` - 17 edges
8. `Relay — Changelog` - 17 edges
9. `VaultManager` - 16 edges
10. `compilerOptions` - 16 edges

## Surprising Connections (you probably didn't know these)
- `on_dictation_pressed()` --calls--> `emit_capture_state()`  [INFERRED]
  native/src-tauri/src/hotkeys/mod.rs → native/src-tauri/src/commands.rs
- `on_dictation_released()` --calls--> `emit_capture_status_event()`  [INFERRED]
  native/src-tauri/src/hotkeys/mod.rs → native/src-tauri/src/commands.rs
- `AvatarBadge()` --calls--> `cn()`  [EXTRACTED]
  web/src/components/ui/avatar.tsx → web/src/lib/utils.ts
- `AvatarGroup()` --calls--> `cn()`  [EXTRACTED]
  web/src/components/ui/avatar.tsx → web/src/lib/utils.ts
- `AvatarGroupCount()` --calls--> `cn()`  [EXTRACTED]
  web/src/components/ui/avatar.tsx → web/src/lib/utils.ts

## Import Cycles
- None detected.

## Communities (128 total, 32 thin omitted)

### Community 0 - "settings/mod.rs"
Cohesion: 0.22
Nodes (14): AppSettings, HotkeySettings, PillPosition, Default, Error, Option, Path, Result (+6 more)

### Community 1 - "devDependencies"
Cohesion: 0.11
Nodes (19): autoprefixer, devDependencies, autoprefixer, postcss, tailwindcss, @tauri-apps/cli, @types/react, @types/react-dom (+11 more)

### Community 2 - "dropdown-menu.tsx"
Cohesion: 0.13
Nodes (21): HeaderUserMenu(), toTitleCase(), Avatar(), AvatarBadge(), AvatarFallback(), AvatarGroup(), AvatarGroupCount(), AvatarImage() (+13 more)

### Community 3 - "VaultManager"
Cohesion: 0.13
Nodes (28): process_chat(), ProcessedPipelineResult, Result, TtsSettings, ExtractedActionItem, PipelineEngine, PipelineError, ProcessedPipelineResult (+20 more)

### Community 4 - "compilerOptions"
Cohesion: 0.07
Nodes (28): esnext, **/*.mts, .next/dev/types/**/*.ts, next-env.d.ts, .next/types/**/*.ts, node_modules, **/*.ts, **/*.tsx (+20 more)

### Community 5 - "commands.rs"
Cohesion: 0.17
Nodes (37): AppSettings, HotkeySettings, KanbanCard, AppState, CaptureStatus, CommandError, emit_capture_state(), emit_capture_status_event() (+29 more)

### Community 6 - "app/layout.tsx"
Cohesion: 0.21
Nodes (6): geistMono, geistSans, metadata, LoginForm(), ThemeProvider(), Toaster()

### Community 7 - "cn"
Cohesion: 0.09
Nodes (32): Field(), FieldContent(), FieldDescription(), FieldError(), FieldGroup(), FieldLabel(), FieldLegend(), FieldSeparator() (+24 more)

### Community 8 - "compilerOptions"
Cohesion: 0.08
Nodes (23): compilerOptions, allowImportingTsExtensions, baseUrl, isolatedModules, jsx, lib, module, moduleResolution (+15 more)

### Community 9 - "tauri.conf.json"
Cohesion: 0.09
Nodes (22): app, security, windows, build, beforeBuildCommand, beforeDevCommand, devUrl, frontendDist (+14 more)

### Community 10 - "AudioRecorder"
Cohesion: 0.14
Nodes (26): Fn, Instant, ActiveSession, AudioRecorder, CapturedAudio, CaptureError, compute_rms_f32(), push_mono_with_level() (+18 more)

### Community 11 - "App.tsx"
Cohesion: 0.05
Nodes (69): App(), CaptureStatePayload, DictationPill(), DictationPillProps, PROCESSING_CAPTIONS, FloatingPill(), LANG_LABELS, PillSettingsPopover() (+61 more)

### Community 12 - "components.json"
Cohesion: 0.10
Nodes (19): aliases, components, hooks, lib, ui, utils, iconLibrary, registries (+11 more)

### Community 13 - "LLMClient"
Cohesion: 0.16
Nodes (19): Client, LLMClient, LLMResponse, ProviderConfig, ProviderError, ProviderType, Default, Error (+11 more)

### Community 14 - "dependencies"
Cohesion: 0.12
Nodes (17): dependencies, class-variance-authority, clsx, @radix-ui/react-dialog, @radix-ui/react-dropdown-menu, @radix-ui/react-tabs, react-dom, tailwind-merge (+9 more)

### Community 15 - "triggers/mod.rs"
Cohesion: 0.24
Nodes (13): Error, Option, Path, Result, String, Vec, test_default_triggers(), test_match_transcript() (+5 more)

### Community 16 - "dependencies"
Cohesion: 0.12
Nodes (17): next-themes, sonner, dependencies, class-variance-authority, clsx, lucide-react, next-themes, react (+9 more)

### Community 17 - "devDependencies"
Cohesion: 0.12
Nodes (17): eslint, eslint-config-next, @tailwindcss/postcss, tw-animate-css, @types/node, devDependencies, eslint, eslint-config-next (+9 more)

### Community 18 - "definitions"
Cohesion: 0.12
Nodes (16): definitions, Number, PermissionEntry, ShellScopeEntryAllowedArg, Target, Value, anyOf, description (+8 more)

### Community 19 - "definitions"
Cohesion: 0.12
Nodes (16): definitions, Number, PermissionEntry, ShellScopeEntryAllowedArg, ShellScopeEntryAllowedArgs, Value, anyOf, description (+8 more)

### Community 20 - "components/page.tsx"
Cohesion: 0.08
Nodes (29): Accordion(), AccordionContent(), AccordionItem(), AccordionTrigger(), Checkbox(), Dialog(), DialogContent(), DialogDescription() (+21 more)

### Community 21 - "properties"
Cohesion: 0.13
Nodes (15): properties, default, description, type, type, array, null, description (+7 more)

### Community 22 - "site-header.tsx"
Cohesion: 0.15
Nodes (16): DashboardLayout(), AppSidebar(), HeaderUserMenu, SiteHeader(), Breadcrumb(), BreadcrumbEllipsis(), BreadcrumbItem(), BreadcrumbLink() (+8 more)

### Community 23 - "Relay — Architectural & Product Decision Log"
Cohesion: 0.06
Nodes (31): Decision 10: Trigger Phrases are User-Customizable, Decision 11: Target Build Environment, Decision 13: Universal Dictation & Global Hotkeys, Decision 14: Real Local Speech-to-Text (whisper-rs), Decision 15: In-App Voice Chat, Grounded in Vault Notes (RAG-lite for now), Decision 16: Optional Local Text-to-Speech (Piper), Decision 17: Hybrid-Mode Architecture, Decision 18 (PTT-001): Preserve Backend Ownership of Capture State (+23 more)

### Community 24 - "properties"
Cohesion: 0.15
Nodes (13): properties, Identifier, default, description, type, description, oneOf, type (+5 more)

### Community 25 - "Command Signatures"
Cohesion: 0.17
Nodes (11): 1. Tauri Commands API (`native/src-tauri/src/commands.rs`), 2. Web Route Handlers (`web/src/app/api/...`), Capture Commands, Command Signatures, Common Response Shape, Kanban Commands, Pipeline Commands, Relay — API Conventions & Specifications (+3 more)

### Community 26 - "permissions"
Cohesion: 0.17
Nodes (12): $ref, array, null, description, items, type, uniqueItems, description (+4 more)

### Community 27 - ".dispatch_action"
Cohesion: 0.53
Nodes (5): McpError, McpRouter, McpToolCallResult, Result, String

### Community 28 - "roles.ts"
Cohesion: 0.33
Nodes (7): auth(), checkRole(), getUserRole(), isTrueAdmin(), UserAppMetadata, UserRole, VolunteerType

### Community 29 - "Relay — Push-to-Talk Pill Redesign & Interaction Overhaul"
Cohesion: 0.05
Nodes (40): 0. Role / execution mode, 10. Hover-out delay, 11. Settings popover, 12. Settings rule — NO FAKE CONTROLS, 13. Dummy-data / placeholder detection, 14. Ollama status MUST be truthful, 15. Whisper status MUST also be truthful, 16. Prompt Mode (+32 more)

### Community 30 - "AGENTS.md"
Cohesion: 0.20
Nodes (6): Accessibility Rules, Rules, Design System Rules, Rules, Project Folder Structure, Rules

### Community 31 - "webviews"
Cohesion: 0.20
Nodes (10): type, webviews, windows, items, description, items, type, description (+2 more)

### Community 32 - "webviews"
Cohesion: 0.20
Nodes (10): type, webviews, windows, items, description, items, type, description (+2 more)

### Community 33 - "Charts, Graphs, and Data Visualizations"
Cohesion: 0.20
Nodes (9): 1 Color (`--color-chart-primary` only), 2 Colors (`--color-chart-primary` + `--color-chart-accent-1`), 3 Colors (`--color-chart-primary` + `--color-chart-accent-1` + `--color-chart-accent-2`), 4+ Colors, Charts, Graphs, and Data Visualizations, Color Tokens, Hard Rules, How Many Colors to Use (+1 more)

### Community 34 - "Functional Requirements"
Cohesion: 0.22
Nodes (8): 1. Audio Capture & Speech-to-Text (STT), 2. Processing Pipeline, 3. Storage & Vault, 4. Provider Abstraction, 5. Dual Surfaces, Functional Requirements, Non-Functional Requirements, Relay — Functional & Non-Functional Requirements

### Community 35 - "CapabilityRemote"
Cohesion: 0.22
Nodes (9): description, properties, required, type, CapabilityRemote, urls, urls, description (+1 more)

### Community 36 - "CapabilityRemote"
Cohesion: 0.22
Nodes (9): description, properties, required, type, CapabilityRemote, urls, urls, description (+1 more)

### Community 37 - "scripts"
Cohesion: 0.18
Nodes (10): name, private, scripts, build, build:native, build:web, dev, dev:native (+2 more)

### Community 38 - "web/package.json"
Cohesion: 0.22
Nodes (8): name, private, scripts, build, dev, lint, start, version

### Community 40 - "Relay"
Cohesion: 0.33
Nodes (5): Getting Started (native desktop app), Getting Started (web dashboard), License, Relay, Structure

### Community 41 - "Target"
Cohesion: 0.67
Nodes (3): Target, description, oneOf

### Community 42 - "login-form.tsx"
Cohesion: 0.11
Nodes (21): DEMO_NOTES, ScribbleNote, SettingsSection, CHANGELOG_DATA, ChangelogDialog(), ChangelogDialogProps, AuthMode, ModeToggle() (+13 more)

### Community 43 - "Relay — Data Model Specification"
Cohesion: 0.29
Nodes (6): 1. Vault Markdown Note Schema, 2. Kanban Card Schema, 3. Trigger Phrase Configuration Schema (`triggers.json`), 4. App Settings Configuration Schema (`settings.json`), 5. LanceDB Vector Record Schema, Relay — Data Model Specification

### Community 44 - "Relay — Product Specification"
Cohesion: 0.29
Nodes (6): Core Value Proposition & Competitive Differentiators, In Scope for MVP, Out of Scope for MVP, Overview, Relay — Product Specification, Target User

### Community 45 - "permissions"
Cohesion: 0.29
Nodes (7): $ref, description, items, type, uniqueItems, items, permissions

### Community 46 - "Relay AI Agent Guidelines"
Cohesion: 0.33
Nodes (6): Knowledge Graph, Precedence, Relay AI Agent Guidelines, Rule Index, Safety Requirements, Starting State

### Community 47 - "Relay — User Flows"
Cohesion: 0.22
Nodes (8): Flow 1: Push-to-Talk Capture to Kanban Card, Flow 2: Audio Scribble to Structured Markdown Note, Flow 3: Configurable Trigger Phrase Execution, Flow 4: Hybrid Mode Web Dashboard Access, Flow 5: Global Show/Hide Hotkey, Flow 6: Universal Dictation (Type Anywhere), Flow 7: Voice Chat Over Vault Notes, Relay — User Flows

### Community 48 - "Capability"
Cohesion: 0.33
Nodes (6): description, required, type, Capability, identifier, permissions

### Community 49 - "Capability"
Cohesion: 0.33
Nodes (6): description, required, type, Capability, identifier, permissions

### Community 50 - "icon.tsx"
Cohesion: 0.33
Nodes (4): alt, contentType, runtime, size

### Community 51 - "reposition"
Cohesion: 0.41
Nodes (12): Monitor, active_monitor(), compute_anchor(), ensure_pill_window(), reposition(), reposition_pill(), AppHandle, Option (+4 more)

### Community 52 - "Relay — System Architecture"
Cohesion: 0.40
Nodes (4): Data Access & Security Model, Relay — System Architecture, Rust Backend Module Design (`native/src-tauri/src/`), Three-Surface Overview

### Community 53 - "Relay — Testing Strategy"
Cohesion: 0.40
Nodes (4): 1. Rust Backend Unit & Integration Tests (`native/src-tauri/`), 2. React Native Desktop UI Tests (`native/src/`), 3. Web Dashboard Tests (`web/`), Relay — Testing Strategy

### Community 54 - "desktop-schema.json"
Cohesion: 0.40
Nodes (4): anyOf, description, $schema, title

### Community 55 - "windows-schema.json"
Cohesion: 0.40
Nodes (4): anyOf, description, $schema, title

### Community 56 - "Code Standards (Frontend)"
Cohesion: 0.40
Nodes (4): Code Standards (Frontend), Imports, Naming conventions, Rules

### Community 57 - "Global Rules"
Cohesion: 0.40
Nodes (4): Global Rules, Precedence, Rule files, Scope

### Community 58 - "Relay — Changelog"
Cohesion: 0.06
Nodes (33): [0.1.0] - 2026-08-19, [0.1.1] - 2026-08-19, [0.1.2] - 2026-08-19, [0.2.0] - 2026-08-19, [0.3.0] - 2026-08-19, [0.3.1] - 2026-08-19, [0.3.2] - 2026-08-19, [0.3.3] - 2026-08-19 (+25 more)

### Community 59 - "local"
Cohesion: 0.50
Nodes (4): default, description, type, local

### Community 60 - "local"
Cohesion: 0.50
Nodes (4): default, description, type, local

### Community 61 - "API Conventions"
Cohesion: 0.50
Nodes (3): API Conventions, Tauri commands (native/src-tauri/src/commands.rs), web/ route handlers (if any are needed beyond Server Actions)

### Community 62 - "Data Access Rules"
Cohesion: 0.50
Nodes (3): Cloud storage (Supabase) — hybrid mode only, Data Access Rules, Local storage (markdown vault + LanceDB) — local-only mode, always available

### Community 63 - "Code Documentation Rules"
Cohesion: 0.50
Nodes (3): Code Documentation Rules, Rust format, TypeScript/React format

### Community 64 - "Performance Rules"
Cohesion: 0.50
Nodes (3): Frontend (native/src/ and web/src/), Performance Rules, Rust backend (native/src-tauri/)

### Community 65 - "RBAC — Not Built, and Why"
Cohesion: 0.50
Nodes (3): If the team/sharing direction is ever picked up, RBAC — Not Built, and Why, Why this is deliberately absent

### Community 66 - "Testing Rules"
Cohesion: 0.50
Nodes (3): Frontend (native/src/ and web/src/), Rust backend (native/src-tauri/), Testing Rules

### Community 69 - "Target"
Cohesion: 0.67
Nodes (3): Target, description, oneOf

### Community 73 - "sidebar.tsx"
Cohesion: 0.10
Nodes (34): data, NavItem, NavMain(), NavSecondary(), NavUser(), Collapsible(), CollapsibleContent(), CollapsibleTrigger() (+26 more)

### Community 75 - "sheet.tsx"
Cohesion: 0.18
Nodes (8): Sheet(), SheetContent(), SheetDescription(), SheetFooter(), SheetHeader(), SheetOverlay(), SheetTitle(), SheetTrigger()

### Community 78 - "ShellScopeEntryAllowedArgs"
Cohesion: 0.67
Nodes (3): ShellScopeEntryAllowedArgs, anyOf, description

### Community 85 - "SttEngine"
Cohesion: 0.18
Nodes (15): c_int, ensure_default_model(), num_cpus(), Arc, Default, Mutex, Option, Path (+7 more)

### Community 97 - "definitions"
Cohesion: 0.12
Nodes (16): definitions, Number, PermissionEntry, ShellScopeEntryAllowedArg, ShellScopeEntryAllowedArgs, Value, anyOf, description (+8 more)

### Community 98 - "properties"
Cohesion: 0.15
Nodes (13): properties, Identifier, default, description, type, description, oneOf, type (+5 more)

### Community 99 - "hotkeys/mod.rs"
Cohesion: 0.33
Nodes (13): apply_hotkeys(), DictationState, on_dictation_pressed(), on_dictation_released(), register_hotkeys(), AppHandle, Option, Result (+5 more)

### Community 100 - "native/package.json"
Cohesion: 0.20
Nodes (9): name, private, scripts, build, dev, preview, tauri, type (+1 more)

### Community 101 - "webviews"
Cohesion: 0.20
Nodes (10): type, webviews, windows, items, description, items, type, description (+2 more)

### Community 102 - "CapabilityRemote"
Cohesion: 0.22
Nodes (9): description, properties, required, type, CapabilityRemote, urls, urls, description (+1 more)

### Community 103 - ".synthesize"
Cohesion: 0.33
Nodes (7): Error, Option, Result, String, TtsSettings, TtsEngine, TtsError

### Community 104 - "permissions"
Cohesion: 0.17
Nodes (12): $ref, array, null, description, items, type, uniqueItems, description (+4 more)

### Community 105 - "Capability"
Cohesion: 0.33
Nodes (6): description, required, type, Capability, identifier, permissions

### Community 106 - "Relay — Roadmap & Competitive Gap Backlog"
Cohesion: 0.40
Nodes (4): Explicitly not gaps (already real), Relay — Roadmap & Competitive Gap Backlog, Shipped this round (real, not mocked), Still stubbed / not real yet — prioritized backlog

### Community 107 - "linux-schema.json"
Cohesion: 0.40
Nodes (4): anyOf, description, $schema, title

### Community 108 - "inject_text"
Cohesion: 0.50
Nodes (4): inject_text(), InjectionError, Result, String

### Community 109 - "local"
Cohesion: 0.50
Nodes (4): default, description, type, local

### Community 113 - "3. First task — repository reconnaissance"
Cohesion: 0.25
Nodes (8): 3. First task — repository reconnaissance, AI, Audio, Desktop positioning, Hotkey, Packaging, Pill, Settings

### Community 115 - "HotkeyRecorder.tsx"
Cohesion: 0.43
Nodes (6): eventToAccelerator(), HotkeyRecorder(), HotkeyRecorderProps, MODIFIER_KEYS, NAMED_KEYS, normalizeKey()

### Community 123 - "Identifier"
Cohesion: 0.67
Nodes (3): Identifier, description, oneOf

### Community 124 - "(dashboard)/page.tsx"
Cohesion: 0.36
Nodes (4): DashboardPage(), getSupabaseClient(), MockSupabaseClient, SupabaseKanbanCard

### Community 127 - "Decisions"
Cohesion: 0.20
Nodes (9): Context, Decision 1: Oscar-Style Edge-Attached Horizontal Notch, Decision 2: Removal of Heavy "RELAY" Branding Text, Decision 3: Floating Hotkey Hint Bar (`Hold to record [Ctrl] [Space]`), Decision 4: Smooth Hover State Transitions with Intent & Grace Delays, Decision 5: Real Settings in Dropdown Surface, Decision Log — Push-to-Talk Pill Redesign & Interaction Refinement, Decisions (+1 more)

### Community 130 - "Push-to-Talk Pill Update Report (v0.4.1)"
Cohesion: 0.40
Nodes (4): Files Modified, Push-to-Talk Pill Update Report (v0.4.1), Verification Results, What Changed

## Knowledge Gaps
- **532 isolated node(s):** `name`, `private`, `version`, `type`, `dev` (+527 more)
  These have ≤1 connection - possible missing edges or undocumented components.
- **32 thin communities (<3 nodes) omitted from report** — run `graphify query` to explore isolated nodes.

## Suggested Questions
_Questions this graph is uniquely positioned to answer:_

- **Why does `VaultManager` connect `VaultManager` to `commands.rs`?**
  _High betweenness centrality (0.015) - this node is a cross-community bridge._
- **Why does `AppState` connect `commands.rs` to `AudioRecorder`, `VaultManager`, `SttEngine`?**
  _High betweenness centrality (0.015) - this node is a cross-community bridge._
- **Why does `cn()` connect `cn` to `dropdown-menu.tsx`, `sidebar.tsx`, `login-form.tsx`, `sheet.tsx`, `components/page.tsx`, `site-header.tsx`?**
  _High betweenness centrality (0.010) - this node is a cross-community bridge._
- **What connects `name`, `private`, `version` to the rest of the system?**
  _532 weakly-connected nodes found - possible documentation gaps or missing edges._
- **Should `devDependencies` be split into smaller, more focused modules?**
  _Cohesion score 0.10526315789473684 - nodes in this community are weakly interconnected._
- **Should `dropdown-menu.tsx` be split into smaller, more focused modules?**
  _Cohesion score 0.13227513227513227 - nodes in this community are weakly interconnected._
- **Should `VaultManager` be split into smaller, more focused modules?**
  _Cohesion score 0.12775842044134728 - nodes in this community are weakly interconnected._