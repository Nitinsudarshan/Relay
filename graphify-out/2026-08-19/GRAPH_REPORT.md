# Graph Report - Relay  (2026-08-19)

## Corpus Check
- 131 files · ~61,115 words
- Verdict: corpus is large enough that graph structure adds value.

## Summary
- 978 nodes · 1565 edges · 96 communities (74 shown, 22 thin omitted)
- Extraction: 100% EXTRACTED · 0% INFERRED · 0% AMBIGUOUS
- Token cost: 0 input · 0 output

## Graph Freshness
- Built from commit: `86e574ff`
- Run `git rev-parse HEAD` and compare to check if the graph is stale.
- Run `graphify update .` after code changes (no API cost).

## Community Hubs (Navigation)
- sidebar.tsx
- devDependencies
- dropdown-menu.tsx
- VaultManager
- compilerOptions
- web/src/lib/utils.ts
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
- select.tsx
- rust-backend.md
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
- Next.js Shadcn Boilerplate
- client.ts
- login-form.tsx
- Relay — Data Model Specification
- Relay — Product Specification
- permissions
- Relay AI Agent Guidelines
- Relay — User Flows
- Capability
- Capability
- icon.tsx
- accessibility.md
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
- Identifier
- Target
- Target
- Input.tsx
- forms-and-validation.md
- responsive-design.md
- roles.ts
- security.md
- server-client-boundary.md
- ui-components.md
- version-and-changelog.md
- rules/graphify.md
- workflows/graphify.md
- next
- code-standards.md
- ts-node
- next.config.ts
- @types/react
- next-env.d.ts
- postcss.config.mjs
- globals.d.ts
- radix-ui
- eslint-config-next

## God Nodes (most connected - your core abstractions)
1. `cn()` - 135 edges
2. `compilerOptions` - 18 edges
3. `compilerOptions` - 16 edges
4. `useSidebar()` - 15 edges
5. `Relay — Architectural & Product Decision Log` - 13 edges
6. `AudioRecorder` - 12 edges
7. `VaultManager` - 11 edges
8. `definitions` - 10 edges
9. `definitions` - 10 edges
10. `Button()` - 10 edges

## Surprising Connections (you probably didn't know these)
- `AvatarBadge()` --calls--> `cn()`  [EXTRACTED]
  web/src/components/ui/avatar.tsx → web/src/lib/utils.ts
- `AvatarGroup()` --calls--> `cn()`  [EXTRACTED]
  web/src/components/ui/avatar.tsx → web/src/lib/utils.ts
- `AvatarGroupCount()` --calls--> `cn()`  [EXTRACTED]
  web/src/components/ui/avatar.tsx → web/src/lib/utils.ts
- `BreadcrumbEllipsis()` --calls--> `cn()`  [EXTRACTED]
  web/src/components/ui/breadcrumb.tsx → web/src/lib/utils.ts
- `CardAction()` --calls--> `cn()`  [EXTRACTED]
  web/src/components/ui/card.tsx → web/src/lib/utils.ts

## Import Cycles
- None detected.

## Communities (96 total, 22 thin omitted)

### Community 0 - "sidebar.tsx"
Cohesion: 0.07
Nodes (46): DashboardLayout(), AppSidebar(), data, NavItem, NavMain(), NavSecondary(), NavUser(), SiteHeader() (+38 more)

### Community 1 - "devDependencies"
Cohesion: 0.07
Nodes (28): autoprefixer, devDependencies, autoprefixer, postcss, tailwindcss, @tauri-apps/cli, @types/react, @types/react-dom (+20 more)

### Community 2 - "dropdown-menu.tsx"
Cohesion: 0.13
Nodes (21): HeaderUserMenu(), toTitleCase(), Avatar(), AvatarBadge(), AvatarFallback(), AvatarGroup(), AvatarGroupCount(), AvatarImage() (+13 more)

### Community 3 - "VaultManager"
Cohesion: 0.16
Nodes (20): ExtractedActionItem, PipelineEngine, PipelineError, ProcessedPipelineResult, Error, Option, Result, String (+12 more)

### Community 4 - "compilerOptions"
Cohesion: 0.07
Nodes (28): esnext, **/*.mts, .next/dev/types/**/*.ts, next-env.d.ts, .next/types/**/*.ts, node_modules, **/*.ts, **/*.tsx (+20 more)

### Community 5 - "web/src/lib/utils.ts"
Cohesion: 0.14
Nodes (6): Checkbox(), Progress(), Skeleton(), Slider(), Switch(), Textarea()

### Community 6 - "app/layout.tsx"
Cohesion: 0.21
Nodes (6): geistMono, geistSans, metadata, LoginForm(), ThemeProvider(), Toaster()

### Community 7 - "cn"
Cohesion: 0.14
Nodes (24): Field(), FieldContent(), FieldDescription(), FieldError(), FieldGroup(), FieldLabel(), FieldLegend(), FieldSeparator() (+16 more)

### Community 8 - "compilerOptions"
Cohesion: 0.08
Nodes (23): compilerOptions, allowImportingTsExtensions, baseUrl, isolatedModules, jsx, lib, module, moduleResolution (+15 more)

### Community 9 - "tauri.conf.json"
Cohesion: 0.09
Nodes (22): app, security, windows, build, beforeBuildCommand, beforeDevCommand, devUrl, frontendDist (+14 more)

### Community 10 - "AudioRecorder"
Cohesion: 0.11
Nodes (30): Arc, Instant, KanbanCard, Mutex, ActiveSession, AudioRecorder, CaptureError, CaptureResult (+22 more)

### Community 11 - "App.tsx"
Cohesion: 0.12
Nodes (31): App(), PTTWidget(), PTTWidgetProps, KanbanBoard(), KanbanBoardProps, ScribbleViewer(), ScribbleViewerProps, ProviderSettings() (+23 more)

### Community 12 - "components.json"
Cohesion: 0.10
Nodes (19): aliases, components, hooks, lib, ui, utils, iconLibrary, registries (+11 more)

### Community 13 - "LLMClient"
Cohesion: 0.25
Nodes (12): Client, Default, LLMClient, LLMResponse, ProviderConfig, ProviderError, ProviderType, Error (+4 more)

### Community 14 - "dependencies"
Cohesion: 0.06
Nodes (33): dependencies, class-variance-authority, clsx, lucide-react, @radix-ui/react-checkbox, @radix-ui/react-dialog, @radix-ui/react-dropdown-menu, @radix-ui/react-label (+25 more)

### Community 15 - "triggers/mod.rs"
Cohesion: 0.24
Nodes (13): Error, Option, Path, Result, String, Vec, test_default_triggers(), test_match_transcript() (+5 more)

### Community 16 - "dependencies"
Cohesion: 0.12
Nodes (17): next-themes, sonner, dependencies, class-variance-authority, clsx, lucide-react, next-themes, react (+9 more)

### Community 17 - "devDependencies"
Cohesion: 0.12
Nodes (17): eslint, shadcn, @tailwindcss/postcss, tw-animate-css, @types/node, devDependencies, eslint, shadcn (+9 more)

### Community 18 - "definitions"
Cohesion: 0.12
Nodes (16): definitions, Number, PermissionEntry, ShellScopeEntryAllowedArg, ShellScopeEntryAllowedArgs, Value, anyOf, description (+8 more)

### Community 19 - "definitions"
Cohesion: 0.12
Nodes (16): definitions, Number, PermissionEntry, ShellScopeEntryAllowedArg, ShellScopeEntryAllowedArgs, Value, anyOf, description (+8 more)

### Community 20 - "components/page.tsx"
Cohesion: 0.09
Nodes (26): Accordion(), AccordionContent(), AccordionItem(), AccordionTrigger(), Dialog(), DialogContent(), DialogDescription(), DialogFooter() (+18 more)

### Community 21 - "properties"
Cohesion: 0.13
Nodes (15): properties, default, description, type, type, array, null, description (+7 more)

### Community 22 - "site-header.tsx"
Cohesion: 0.24
Nodes (11): ModeToggle(), HeaderUserMenu, Breadcrumb(), BreadcrumbEllipsis(), BreadcrumbItem(), BreadcrumbLink(), BreadcrumbList(), BreadcrumbPage() (+3 more)

### Community 23 - "Relay — Architectural & Product Decision Log"
Cohesion: 0.14
Nodes (13): Decision 10: Trigger Phrases are User-Customizable, Decision 11: Target Build Environment, Decision 12: Hybrid-Mode Architecture, Decision 1: Build Path & Relationship to Mnemos, Decision 2: Technology Stack, Decision 3: Hybrid Deployment Includes Web Surface, Decision 4: Cost Ceiling is a Hard Constraint, Decision 5: No Meeting-Bot Architecture (+5 more)

### Community 24 - "properties"
Cohesion: 0.15
Nodes (13): properties, Identifier, default, description, type, description, oneOf, type (+5 more)

### Community 25 - "Command Signatures"
Cohesion: 0.17
Nodes (11): 1. Tauri Commands API (`native/src-tauri/src/commands.rs`), 2. Web Route Handlers (`web/src/app/api/...`), Capture Commands, Command Signatures, Common Response Shape, Kanban Commands, Pipeline Commands, Provider Settings Commands (+3 more)

### Community 26 - "permissions"
Cohesion: 0.17
Nodes (12): $ref, array, null, description, items, type, uniqueItems, description (+4 more)

### Community 27 - ".dispatch_action"
Cohesion: 0.53
Nodes (5): McpError, McpRouter, McpToolCallResult, Result, String

### Community 28 - "select.tsx"
Cohesion: 0.18
Nodes (9): Select(), SelectContent(), SelectItem(), SelectLabel(), SelectScrollDownButton(), SelectScrollUpButton(), SelectSeparator(), SelectTrigger() (+1 more)

### Community 30 - "AGENTS.md"
Cohesion: 0.20
Nodes (6): Component Architecture, Rules, Design System Rules, Rules, Project Folder Structure, Rules

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

### Community 40 - "Next.js Shadcn Boilerplate"
Cohesion: 0.25
Nodes (7): Components Included, Features, Getting Started, License, Next.js Shadcn Boilerplate, Relay, Structure

### Community 41 - "client.ts"
Cohesion: 0.36
Nodes (4): DashboardPage(), getSupabaseClient(), MockSupabaseClient, SupabaseKanbanCard

### Community 42 - "login-form.tsx"
Cohesion: 0.21
Nodes (11): AuthMode, Badge(), badgeVariants, Card(), CardAction(), CardContent(), CardDescription(), CardFooter() (+3 more)

### Community 43 - "Relay — Data Model Specification"
Cohesion: 0.29
Nodes (6): 1. Vault Markdown Note Schema, 2. Kanban Card Schema, 3. Trigger Phrase Configuration Schema (`triggers.json`), 4. Provider Settings Configuration Schema (`settings.json`), 5. LanceDB Vector Record Schema, Relay — Data Model Specification

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
Cohesion: 0.33
Nodes (5): Flow 1: Push-to-Talk Capture to Kanban Card, Flow 2: Audio Scribble to Structured Markdown Note, Flow 3: Configurable Trigger Phrase Execution, Flow 4: Hybrid Mode Web Dashboard Access, Relay — User Flows

### Community 48 - "Capability"
Cohesion: 0.33
Nodes (6): description, required, type, Capability, identifier, permissions

### Community 49 - "Capability"
Cohesion: 0.33
Nodes (6): description, required, type, Capability, identifier, permissions

### Community 50 - "icon.tsx"
Cohesion: 0.33
Nodes (4): alt, contentType, runtime, size

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
Cohesion: 0.25
Nodes (7): [0.1.0] - 2026-08-19, [0.1.1] - 2026-08-19, [0.1.2] - 2026-08-19, Complete UI Design Pass & Theme System Refactoring, Improvements & UI Refactoring, Initial Release — Multi-Surface Architecture & Core Pipeline, Relay — Changelog

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

### Community 67 - "Identifier"
Cohesion: 0.67
Nodes (3): Identifier, description, oneOf

### Community 68 - "Target"
Cohesion: 0.67
Nodes (3): Target, description, oneOf

### Community 69 - "Target"
Cohesion: 0.67
Nodes (3): Target, description, oneOf

### Community 73 - "roles.ts"
Cohesion: 0.33
Nodes (7): auth(), checkRole(), getUserRole(), isTrueAdmin(), UserAppMetadata, UserRole, VolunteerType

## Knowledge Gaps
- **365 isolated node(s):** `name`, `private`, `version`, `type`, `dev` (+360 more)
  These have ≤1 connection - possible missing edges or undocumented components.
- **22 thin communities (<3 nodes) omitted from report** — run `graphify query` to explore isolated nodes.

## Suggested Questions
_Questions this graph is uniquely positioned to answer:_

- **Why does `cn()` connect `cn` to `sidebar.tsx`, `dropdown-menu.tsx`, `web/src/lib/utils.ts`, `login-form.tsx`, `components/page.tsx`, `site-header.tsx`, `select.tsx`?**
  _High betweenness centrality (0.029) - this node is a cross-community bridge._
- **Why does `VaultManager` connect `VaultManager` to `AudioRecorder`?**
  _High betweenness centrality (0.005) - this node is a cross-community bridge._
- **Why does `AppState` connect `AudioRecorder` to `VaultManager`?**
  _High betweenness centrality (0.004) - this node is a cross-community bridge._
- **What connects `name`, `private`, `version` to the rest of the system?**
  _365 weakly-connected nodes found - possible documentation gaps or missing edges._
- **Should `sidebar.tsx` be split into smaller, more focused modules?**
  _Cohesion score 0.06766917293233082 - nodes in this community are weakly interconnected._
- **Should `devDependencies` be split into smaller, more focused modules?**
  _Cohesion score 0.06896551724137931 - nodes in this community are weakly interconnected._
- **Should `dropdown-menu.tsx` be split into smaller, more focused modules?**
  _Cohesion score 0.13227513227513227 - nodes in this community are weakly interconnected._