# Relay AI Agent Guidelines

This is a **Rust (Axum-style backend) + Tauri/React native app for Windows,
plus a Next.js + Shadcn + Supabase web dashboard for hybrid mode** project
(Relay). You **must follow** all rules in the `Rules/` directory for any
code change in this repo.

This file, `Rules/`, and `.agents/` were adapted from
[NGConnect's AGENTS.md](https://github.com/Nitinsudarshan/NGConnect/blob/main/AGENTS.md)
and its `rules/`/`.agents/` — a Next.js + Supabase + shadcn/ui project — for
Relay's genuinely different, three-surface architecture (see
`Relay - Decision Log.md`, decisions 1–3 and 12). Read `Relay - IDE Build Prompt.md`
first for full product/architecture context; this file and `Rules/` are the
coding-convention layer underneath it.

## Rule Index

- [global.md](Rules/global.md): Master index, per-surface applicability, and precedence rules.
- [project-structure.md](Rules/project-structure.md): Repo layout — where new files go across all three surfaces.
- [rust-backend.md](Rules/rust-backend.md): Error handling, module layout, async, Tauri command exposure (native/src-tauri only).
- [code-standards-frontend.md](Rules/code-standards-frontend.md): TypeScript/React conventions, shared by both frontends.
- [component-architecture.md](Rules/component-architecture.md): Component organization and splitting.
- [design-system.md](Rules/design-system.md): Design tokens (colors, spacing, radius, typography).
- [ui-components.md](Rules/ui-components.md): shadcn/ui usage and styling conventions, including charts.
- [charts.md](Rules/charts.md): Charts, graphs, and data visualizations.
- [responsive-design.md](Rules/responsive-design.md): Mobile-first rules — web/ dashboard only.
- [accessibility.md](Rules/accessibility.md): Accessibility requirements for UI.
- [documentation.md](Rules/documentation.md): Code commenting format (TypeScript and Rust).
- [data-access.md](Rules/data-access.md): Local vault/LanceDB access vs. Supabase cloud access.
- [security.md](Rules/security.md): Secrets, environment variables, hybrid-mode auth.
- [server-client-boundary.md](Rules/server-client-boundary.md): Server vs Client Component usage — web/ only.
- [forms-and-validation.md](Rules/forms-and-validation.md): Standard form pattern (shadcn Form + react-hook-form + zod).
- [testing.md](Rules/testing.md): What to test, frameworks, and file placement across all three surfaces.
- [api-conventions.md](Rules/api-conventions.md): Tauri command response shape; web route handler shape.
- [performance.md](Rules/performance.md): Async/non-blocking rules, bundle size, memoization.
- [rbac-settings.md](Rules/rbac-settings.md): Why RBAC is intentionally not built yet.
- [version-and-changelog.md](Rules/version-and-changelog.md): Versioning and changelog maintenance requirement.

Not carried over from NGConnect: `data-import.md` (Excel/CSV import safety —
no Relay feature to attach to) and `greetings.md` (documented a specific
NGConnect dashboard component with no Relay analog). Don't recreate either
speculatively.

## Knowledge Graph

This project has a graphify knowledge graph at `graphify-out/` (see `.agents/rules/graphify.md`).

- Use `graphify query "<question>"` (CLI) for codebase/architecture questions when `graphify-out/graph.json` exists.
- Use `graphify path "<A>" "<B>"` to trace relationships between files/symbols.
- Use `graphify explain "<concept>"` for focused concept lookups.
- Read `graphify-out/GRAPH_REPORT.md` for a broad architecture overview.
- After modifying source files (Rust or TypeScript), run `graphify update .` to keep the graph current.
- The `/graphify` slash command re-runs the full graph pipeline.

## Precedence

If two rules conflict, resolve in this order (most specific wins):
1. A rule scoped to the exact surface/file/folder being edited.
2. `security.md` / `data-access.md` (safety/correctness > style).
3. `server-client-boundary.md` / `api-conventions.md` / `rust-backend.md` (architecture correctness).
4. `code-standards-frontend.md` / `component-architecture.md` / `project-structure.md`.
5. `forms-and-validation.md` / `testing.md` / `performance.md`.
6. `design-system.md` / `ui-components.md` / `charts.md` / `responsive-design.md` / `accessibility.md`.
7. `documentation.md`.

## Starting State

Unlike NGConnect, this is a **from-scratch repo** — there is no existing
implementation and no "known drift to clean up" yet. All 12 architecture and
scope decisions referenced throughout `Rules/` are already made; see
`Relay - Decision Log.md` for the full context/reason/alternatives/impact
behind each one, and `Relay - IDE Build Prompt.md` for the build order.

## Safety Requirements

- **Never** hardcode secrets or credentials in source code.
- **Never** bypass RLS assumptions once hybrid mode's cloud storage is in
  place; queries missing matching policies should fail closed.
- **Never** expose the Supabase service-role key to `native/src/`, `web/`
  Client Components, or any browser-reachable bundle.
- **Always** confirm with the user before running any command that mutates
  cloud-stored user data.
- **Local-only mode needs no auth** — don't build login/session logic that
  only ever runs there. **Hybrid mode requires real login** (password/
  token) against the cloud backend — not LAN-only or tunnel-based access to
  the Windows machine, a framing already considered and rejected (decision
  12).
