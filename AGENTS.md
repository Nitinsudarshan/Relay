# Relay AI Agent Guidelines

Relay is a **Rust + Tauri/React desktop app for Windows** with a browser
extension capture system (`native/browser-extension`). You **must follow** all
rules in the `rules/` directory for any code change here.

Start with `docs/README.md` for the map of what documentation exists and what
each file is for. `rules/` is the coding-convention layer underneath it.

## Surfaces

| Path | What it is |
|---|---|
| `native/src-tauri/` | Rust backend: capture, STT, vault, triggers, MCP wiring. |
| `native/src/` | React frontend rendered inside the Tauri window. |
| `native/browser-extension/` | Browser extension for structured web and AI conversation capture into the vault. |

## Rule Index

All paths are relative to `rules/`.

- [global.md](rules/global.md): Master index, per-surface applicability, and precedence rules.
- [project-structure.md](rules/project-structure.md): Repo layout — where new files go across all surfaces.
- [rust-backend.md](rules/rust-backend.md): Error handling, module layout, async, Tauri command exposure (`native/src-tauri` only).
- [code-standards-frontend.md](rules/code-standards-frontend.md): TypeScript/React conventions.
- [component-architecture.md](rules/component-architecture.md): Component organization and splitting.
- [design-system.md](rules/design-system.md): Design tokens (colors, spacing, radius, typography).
- [ui-components.md](rules/ui-components.md): shadcn/ui usage, styling conventions, and the "no fake controls" rule.
- [charts.md](rules/charts.md): Charts, graphs, and data visualizations.
- [accessibility.md](rules/accessibility.md): Accessibility requirements for UI.
- [documentation.md](rules/documentation.md): Code commenting format (TypeScript and Rust).
- [data-access.md](rules/data-access.md): Local vault access vs. Supabase cloud access.
- [security.md](rules/security.md): Secrets, environment variables, hybrid-mode auth.
- [forms-and-validation.md](rules/forms-and-validation.md): Standard form pattern (shadcn Form + react-hook-form + zod).
- [testing.md](rules/testing.md): What to test, frameworks, and file placement across all surfaces.
- [api-conventions.md](rules/api-conventions.md): Tauri command response shape.
- [performance.md](rules/performance.md): Async/non-blocking rules, bundle size, memoization.
- [rbac-settings.md](rules/rbac-settings.md): Why RBAC is intentionally not built yet.
- [version-and-changelog.md](rules/version-and-changelog.md): Release versioning, changelog ownership, and conventional change metadata.
- [readme.md](rules/readme.md): Machine-readable rules for generating, auditing, or rewriting `README.md`.
- [maybe-later.md](rules/maybe-later.md): Policy and format for logging deferred features to `maybe_later.md`.

## Verifying a change

CI runs these on every push and pull request (`.github/workflows/ci.yml`);
run them locally before pushing:

```bash
# Rust backend
cd native/src-tauri && cargo clippy --all-targets -- -D warnings && cargo test

# Native frontend
cd native && npm ci && npx tsc --noEmit && npm test
```

Building the Rust crate needs a C/C++ toolchain and CMake for whisper.cpp. On
Linux it additionally needs the GTK/WebKit and ALSA development headers — see
the `system dependencies` step in the CI workflow for the exact package list.

The Rust toolchain is pinned in `native/src-tauri/rust-toolchain.toml`, and
rustup installs it automatically on your first `cargo` call. Do not work around
it: CI runs `clippy -D warnings`, so a different compiler means a different
lint set, and "passes locally, fails CI" follows immediately. Bump the pin
deliberately, in its own commit, fixing whatever the newer clippy finds.

`cargo fmt --check` is deliberately **not** in CI yet: the crate predates any
formatting pass and currently differs from rustfmt in 45 files. Running
`cargo fmt` once, as its own commit, is what unblocks adding that gate.

## Knowledge Graph

This project can generate a graphify knowledge graph into `graphify-out/` (see
`.agents/rules/graphify.md`). The directory is generated output and is **not**
checked in — run `graphify update .` to create or refresh it.

- `graphify query "<question>"` for codebase/architecture questions.
- `graphify path "<A>" "<B>"` to trace relationships between files/symbols.
- `graphify explain "<concept>"` for focused concept lookups.
- `graphify-out/<date>/GRAPH_REPORT.md` for a broad architecture overview, once generated.

## Precedence

If two rules conflict, resolve in this order (most specific wins):
1. A rule scoped to the exact surface/file/folder being edited.
2. `security.md` / `data-access.md` (safety/correctness > style).
3. `api-conventions.md` / `rust-backend.md` (architecture correctness).
4. `code-standards-frontend.md` / `component-architecture.md` / `project-structure.md`.
5. `forms-and-validation.md` / `testing.md` / `performance.md`.
6. `design-system.md` / `ui-components.md` / `charts.md` / `accessibility.md`.
7. `documentation.md` / `readme.md` / `version-and-changelog.md`.

## Working in an established codebase

This repo is well past its from-scratch phase — it is at v0.41.0 with a
shipped capture pipeline, scribbles, and a vault. Two habits
matter more here than they did at the start:

- **Read the code before the prose.** Where a document and the source
  disagree, the source is right and the document is a bug. Fix it or delete
  it in the same change rather than working around it.
- **Leave the marker where the gap is.** If you knowingly leave something
  incomplete, put a `TODO(context):` comment next to the code it affects.
  Do not open a new root-level markdown file to hold it — that is how the
  documentation pile grew in the first place. Deferred *features* go in
  `maybe_later.md` per `rules/maybe-later.md`; deferred *code* gets a
  comment.

## Safety & Automated Maintenance Requirements

- **Never** hardcode secrets or credentials in source code.
- **Never** hardcode machine-specific absolute paths in committed build
  configuration (`.cargo/config.toml`, scripts, CI). They break every other
  machine, including CI.
- **Never** bypass RLS assumptions once hybrid mode's cloud storage is in
  place; queries missing matching policies should fail closed.
- **Never** expose the Supabase service-role key to `native/src/` or any
  browser-reachable bundle.
- **Always** confirm with the user before running any command that mutates
  cloud-stored user data.
- **Local-only mode needs no auth** — don't build login/session logic that
  only ever runs there. **Hybrid mode requires real login** against the cloud
  backend — not LAN-only or tunnel-based access to the Windows machine, a
  framing already considered and rejected (`docs/decisions.md`, Decision 12).
- **No Agent-Owned Version Bumps**: Development agents MUST NOT independently
  increment the release version in `VERSION` or manifests, nor modify the
  canonical release changelog (`CHANGELOG.md`) during ordinary development work.
  Release versions and changelog entries are owned exclusively by the release
  pipeline (`.github/workflows/release.yml`).
- **Mandatory Commit & Push Execution**: After every task, before committing:
  1. Inspect changes and run verification gates:
     - `npm run verify:rules` (manifest consistency, repository & README rules)
     - `cd native/src-tauri && cargo clippy --all-targets -- -D warnings && cargo test`
     - `cd native && npx tsc --noEmit && npm test`
  2. Write conventional commit messages (`feat:`, `fix:`, `refactor:`, `BREAKING CHANGE:`, etc.)
     communicating change impact per `rules/version-and-changelog.md`.
  3. Audit `README.md` per `rules/readme.md` whenever scripts, commands,
     dependencies, or features are modified.
  4. Push feature branch and create/update the PR with an accurate summary of changes.
