# Relay — Repository Cleanup, Dead Code Audit & Web Removal Report

> Date: 2026-09-07 (updated 2026-09-10)  
> Repository Version: v0.41.0  
> Scope: Full repository audit for dead code, obsolete architecture, Next.js web application removal, and backlog cleanup.
>
> **Update (2026-09-10):** `meetings_v2`, `talkback`, `tts`, and `calendar` modules have been fully removed from the Rust backend and frontend. All architectural and functional knowledge is preserved in [`removed.md`](../removed.md). Section H notes are updated accordingly.

---

## A. Confirmed Deletions

The following files and directories are confirmed dead, obsolete, or explicitly targeted for removal with zero runtime, build, or test dependencies:

1. **`web/` Directory (Entirety)**
   - All 4 subdirectories and 8 root files in `web/` (including `web/src/`, `web/node_modules/`, `web/package.json`, `web/package-lock.json`, `web/next.config.ts`, `web/tsconfig.json`, `web/components.json`, `web/postcss.config.mjs`, `web/scripts/`).
2. **Web-Only Rule Files in `rules/`**
   - `rules/server-client-boundary.md`: Exclusively applied to App Router Server vs Client components in `web/` (globs: `web/src/app/**, web/src/components/**`). Has no applicability to Tauri desktop SPA.
   - `rules/responsive-design.md`: Exclusively applied to mobile-first responsive layout rules in `web/` (globs: `web/src/**/*.tsx`). Tauri window is a desktop surface with fixed/resizable bounds governed by `design-system.md`.
3. **Web-Only CI Workflow Steps in `.github/workflows/ci.yml`**
   - `web-dashboard` job (lines 117–143) executing `npm ci`, `tsc --noEmit`, and `npm run build` in `web/`.
4. **Obsolete Root `package.json` Scripts**
   - `"dev:web": "npm --prefix web run dev"`
   - `"build:web": "npm --prefix web run build"`
   - `"dev": "npm --prefix web run dev"` -> Updated to `"npm --prefix native run dev"` (or native tauri dev)
   - `"build": "npm --prefix web run build"` -> Updated to `"npm --prefix native run build"`
   - `"install:all": "npm --prefix native install && npm --prefix web install"` -> Updated to `"npm --prefix native install"`
   - `"typecheck": "npm --prefix native run typecheck && npm --prefix web run typecheck"` -> Updated to `"npm --prefix native run typecheck"`
5. **`maybe_later.md` Items 1–7**
   - Item 1: In-App Navigation Keyboard Shortcuts (`Alt + 1..4` / `Ctrl + 1..4`)
   - Item 2: Post-Migration Legal & Governance Hardening
   - Item 3: Meeting Detection, Diarization & Vault Integration
   - Item 4: Per-Second Channel Provenance for Turn-Level Speaker Attribution
   - Item 5: Honest Failure Reporting from `LLMClient::complete`
   - Item 6: Frontend Test Runner (Vitest + React Testing Library)
   - Item 7: Semantic Retrieval for Talkback (Embeddings + Hybrid Scoring)
   - Note: Underlying features (meeting intelligence, diarization, frontend vitest test suite, retrieval, LLM fallback handling) remain active and intact in the codebase.
6. **Obsolete Web User Flow in `docs/user-flows.md`**
   - Flow 4: Hybrid Mode Web Dashboard Access (describes logging into `https://relay.app` and viewing synced cards via Next.js dashboard). Remaining flows renumbered 1–8.

---

## B. Web Application Deletion

The retired Next.js web application lived in `web/`.
- **Relationship to Native**: Confirmed 0 import, build, or runtime coupling to `native/` or `native/src-tauri/`.
- **Supabase Mock in Web**: The web app used a stubbed `MockSupabaseClient` with hardcoded dummy data; it did not share or manage live native databases.
- **Root Manifest Cleanup**: Cleaned all `npm --prefix web` commands from root `package.json`.
- **CI/CD Cleanup**: Removed `web-dashboard` job from `.github/workflows/ci.yml`.

---

## C. Rust Dead-Code Audit (`native/src-tauri/`)

- **Clippy & Check Status**: `cargo check` and `cargo clippy --all-targets -- -D warnings` passed with 0 warnings and 0 errors.
- **Command Registration Audit**: Inspected all ~180 `#[tauri::command]` functions in `native/src-tauri/src/commands.rs`. Every single command is registered in the Tauri invoke handler in `native/src-tauri/src/lib.rs`.
- **Domain Modules**:
  - `capture/`: Active audio recording & STT (`cpal`, `whisper-rs`).
  - `capture/web/`: Active web capture loopback listener (`bridge.rs`), source classification, sanitization, and normalization (`normalize.rs`).
  - `vault/`: Active local markdown vault, Scribbles, trash lifecycle, notes, and file operations.
  - `identity/`: Active native Supabase client for anonymous installation tracking, heartbeat, and diagnostics.
  - `updates/`: Active app release checks via Supabase `app_releases` and GitHub Releases.
  - `memory/`, `entities/`, `relationships/`, `retrieval/`, `context/`: Active knowledge architecture.
  - ~~`meetings_v2/`~~, ~~`talkback/`~~, ~~`tts/`~~, ~~`calendar/`~~: **Removed** (2026-09-10). Full specification preserved in [`removed.md`](../removed.md).
- **Result**: Zero dead Rust code. All Rust modules and commands are actively registered and operational.

---

## D. Frontend Dead-Code Audit (`native/src/`)

- **TypeScript Compilation**: `tsc --noEmit` passed with 0 errors.
- **Vitest Suite**: 25 test files passed (348 tests). *(Updated 2026-09-10 after meetings/talkback removal reduced the test count.)*
- **Component Audit**:
  - `components/capture/`: `DictationPill`, `FloatingPill` (rendered via `main.tsx` for overlay window), `PillSettingsPopover`.
  - `components/knowledge/`: First-class Knowledge Graph canvas and force physics.
  - `components/captures/`: Web capture viewer, payload inspector, and context tab.
  - `components/files/`: Document import and viewer.
  - `components/home/`: Home metrics dashboard.
  - `components/voicenotes/`: Voice note recorder and manager.
  - `components/scribble/`: Scribble editor, graph viewer, and merge modal.
  - `components/settings/`: Provider settings, STT diagnostics, trash settings.
  - `components/common/`: Confirmation modal, empty state, sidebar, page header, markdown view, modals.
  - `components/ui/`: Standard shadcn primitives (`button`, `badge`, `card`, `dropdown-menu`, `input`, `skeleton`, `switch`, `tooltip`).
  - ~~`components/meetings_v2/`~~, ~~`components/talkback/`~~: **Removed** (2026-09-10). See [`removed.md`](../removed.md).
- **Result**: Zero dead frontend components. All views and components have live consumers in `App.tsx`, `main.tsx`, or test suites.

---

## E. Scripts Audit (`scripts/`)

Inspected every script in `scripts/`:
1. `scripts/verify-commit-rules.js`: Enforces manifest version sync across `VERSION`, `package.json`, `native/package.json`, `native/src-tauri/tauri.conf.json`, `native/src-tauri/Cargo.toml`, and README rules. Tested by `verify-commit-rules.test.mjs`. **KEEP.**
2. `scripts/prepare-release.mjs`: Conventional commits parser, changelog generator, and release manifest bumper. Tested by `prepare-release.test.mjs`. **KEEP.**
3. `scripts/build-voice-manifest.mjs`: Piper TTS engine & voice asset manifest builder. *(Orphaned — TTS removed 2026-09-10, but the archive-reading utilities remain tested and usable for future download tooling.)*
4. `scripts/lib/archive-index.mjs` & `scripts/lib/fake-archives.mjs`: Zero-dependency ZIP / TAR reader and test fixture generator. **KEEP** — archive reading utilities with their own passing tests, independent of the removed TTS installer.
5. `scripts/capture-validation/`: Headless Chromium DevTools Protocol runner for web capture validation across virtualized ChatGPT, Claude, and GitHub pages. **KEEP.**
- **Result**: All 40 unit tests in `node --test "scripts/**/*.test.mjs"` pass. Zero dead scripts.

---

## F. Documentation Audit (`docs/` & `Meeting-rules/`)

Inspected all documentation files:
- `docs/README.md`: Living documentation index. (UPDATE: remove `web/` references).
- `docs/architecture.md`: System architecture. (UPDATE: remove deferred Web Surface box and Next.js descriptions; describe desktop + browser extension architecture).
- `docs/user-flows.md`: User flows. (UPDATE: delete Flow 4 "Hybrid Mode Web Dashboard Access" and renumber remaining flows).
- `docs/product.md`: Product spec. (UPDATE: update dual-surface reference).
- `docs/requirements.md`: Functional requirements. (UPDATE: update FR-5.2 and `maybe_later.md` references).
- `docs/data-model.md`: Data model spec. Already describes vault, captures, and derived data. **KEEP.**
- `docs/decisions.md`: Append-only decision records. **KEEP.** Append Decision 66 documenting the web surface removal.
- `docs/capture.md`, `docs/capture/RESEARCH.md`, `docs/capture/BENCHMARKS.md`: Web capture specifications and browser benchmarks. **KEEP.** (UPDATE: update `maybe_later.md` section numbers).
- `docs/meetings/*`: V2 meeting intelligence specs, legacy removals, and audits. **KEEP.** (UPDATE: update `maybe_later.md` item references in `TRANSCRIPT_AND_SPEAKER_REBUILD.md`).
- `docs/talkback/*`: Architecture, benchmarks, and research for Talkback agent. **KEEP.**
- `Meeting-rules/*`: Load-bearing behavioral specifications cited directly by Rust backend doc comments. **KEEP.**
- `docs/archive/prompt-mode.md`: Historical archive of Prompt Mode removed in v0.15.0. **KEEP.**

---

## G. Stale References to Update

1. **`README.md`**:
   - Remove Next.js web dashboard bullet in Features.
   - Remove `npm run dev:web` and `cd web && npm install` from Quick Start / Install.
   - Remove `cd web && npx tsc --noEmit && npm run build` from Tests.
   - Ensure strict compliance with `rules/readme.md` (single H1, blockquote tagline, valid badge URLs, language-tagged code blocks).
2. **`AGENTS.md`**:
   - Remove `web/` row from Surfaces table.
   - Remove `web/` from verification commands (`cd web && npm ci && npx tsc --noEmit`).
   - Remove rule index entries for `responsive-design.md` and `server-client-boundary.md`.
3. **`rules/` Directory**:
   - `rules/global.md`: Update description of surfaces and remove `server-client-boundary.md` and `responsive-design.md` from table and precedence lists.
   - `rules/project-structure.md`: Remove `web/` directory tree and web-specific rules.
   - `rules/api-conventions.md`: Remove Next.js route handler section.
   - `rules/data-access.md`: Remove `web/` references; clarify cloud storage is native Rust backend to Supabase.
   - `rules/forms-and-validation.md`: Remove `web/` route handler references.
   - `rules/component-architecture.md`: Remove `web/` co-location rules.
   - `rules/security.md`: Remove `NEXT_PUBLIC_*` and `web/` references.
   - `rules/code-standards-frontend.md`: Update to focus on `native/src/`.
   - `rules/testing.md`: Remove `web/` references.
   - `rules/performance.md`: Remove `next/image` and `next/dynamic` references.
   - `rules/ui-components.md`, `rules/documentation.md`, `rules/design-system.md`, `rules/verification-honesty.md`, `rules/lazy-code-ladder.md`, `rules/over-engineering-review.md`, `rules/version-and-changelog.md`: Remove `web/` globs and references.
4. **`maybe_later.md` Cross-References**:
   - Renumber remaining items 8–19 to 1–12.
   - Update cross-references in `docs/capture.md` (§13->§6, §14->§7, §16->§9), `docs/capture/RESEARCH.md`, `docs/requirements.md`, `docs/roadmap.md`, and `docs/meetings/TRANSCRIPT_AND_SPEAKER_REBUILD.md`.

---

## H. Keep / Verify

1. **`native/browser-extension/` & `native/src/webcapture/`**:
   - **KEEP.** This is Relay's active browser web capture system, completely distinct from the removed `web/` application. Builds into Chrome/Edge extension.
2. **`supabase/` Directory**:
   - **KEEP.** `supabase/migrations/20260822_relay_account_schema.sql` establishes tables (`relay_accounts`, `installations`, `diagnostics_events`, `app_releases`) and RPC functions (`register_installation_heartbeat`, `ingest_diagnostic_event`) actively consumed by the native Rust backend (`native/src-tauri/src/identity/supabase.rs`, `native/src-tauri/src/updates/mod.rs`).
3. ~~**`Meeting-rules/`**~~:
   - **Removed** (2026-09-10) along with `meetings_v2`. Content preserved in [`removed.md`](../removed.md).
4. **`scripts/capture-validation/`**:
   - **KEEP.** Live real-browser CDP test suite for web capture verification.

---

## I. Dependency Cleanup

1. **JavaScript**:
   - `web/package.json` (and `web/node_modules/`, `web/package-lock.json`) is deleted entirely, removing `next`, `react@19`, `eslint-config-next`, `@tailwindcss/postcss`, etc.
   - `package.json` (root) has zero dependencies; scripts updated to native-only.
   - `native/package.json` retains only active native dependencies.
2. **Rust**:
   - All crates in `native/src-tauri/Cargo.toml` are confirmed in active use.

---

## J. Verification Plan

1. **Repository Rules & Manifest Validation**:
   ```bash
   npm run verify:rules
   ```
2. **Scripts Test Suite**:
   ```bash
   npm run test:scripts
   ```
3. **Rust Backend Validation**:
   ```bash
   cd native/src-tauri && cargo clippy --all-targets -- -D warnings && cargo test
   ```
4. **Native Frontend Validation**:
   ```bash
   cd native && npm run typecheck && npm test && npm run build && npm run build:extension
   ```
5. **Git Status & Diff Review**:
   - Ensure clean tree with no orphaned or untracked debris.
