---
trigger: always_on
description: Master index and precedence rules for the /Rules directory. Always load this file first.
---

# Global Rules

All development on Relay must follow the rules defined in this `Rules/` directory.
This file is the entry point — read it first, then apply the specific files
relevant to the surface you're editing.

Relay is **three surfaces sharing one repo**, per `Relay - Decision Log.md`
(decisions 1, 2, 3, 12):
- **`native/src-tauri/`** — the Rust (Axum-style) backend: capture, STT,
  the meeting→Kanban and scribble→structured-output pipelines, the
  configurable trigger-phrase system, the local vault/LanceDB, MCP wiring.
- **`native/src/`** — the React frontend rendered inside the Tauri window
  (Windows desktop, local mode).
- **`web/`** — the Next.js + Shadcn hybrid-mode dashboard (cloned from
  Active Projects/Starter Template), talking to Supabase for cloud storage
  + auth (hybrid mode only).

## Rule files

| File | Covers | Applies to |
|---|---|---|
| `project-structure.md` | Repo layout — where new files go | All three surfaces |
| `rust-backend.md` | Error handling, module layout, async, Tauri command exposure | `native/src-tauri/` only |
| `code-standards-frontend.md` | TypeScript/React conventions, naming, imports | `native/src/` + `web/` |
| `component-architecture.md` | How to split/organize components | `native/src/` + `web/` |
| `design-system.md` | Color, spacing, radius, typography tokens | `native/src/` + `web/` |
| `ui-components.md` | shadcn/ui usage, styling approach | `native/src/` + `web/` |
| `charts.md` | Charts, graphs, data visualizations (dogfooding/accuracy views) | `native/src/` + `web/` |
| `responsive-design.md` | Mobile-first / breakpoint rules | `web/` only — `native/` is a fixed desktop window |
| `accessibility.md` | a11y requirements | `native/src/` + `web/` |
| `documentation.md` | Comment/docstring format (TS and Rust) | All three surfaces |
| `data-access.md` | Local vault/LanceDB access vs. Supabase cloud access | All three surfaces |
| `security.md` | Secrets, env vars, hybrid-mode auth | All three surfaces |
| `server-client-boundary.md` | Server vs. Client Components in the App Router | `web/` only |
| `forms-and-validation.md` | Standard form/validation pattern (react-hook-form + zod) | `native/src/` + `web/` |
| `testing.md` | What to test, frameworks, file placement | All three surfaces |
| `api-conventions.md` | Tauri command response shape; web route handler shape | `native/src-tauri/` + `web/` |
| `performance.md` | Bundle size, async/non-blocking rules, memoization | All three surfaces |
| `rbac-settings.md` | Why this is intentionally not built yet | Reference only — not active |
| `version-and-changelog.md` | Versioning and changelog maintenance requirement | Whole repo |

Two NGConnect rule files were **not** carried over — `data-import.md`
(Excel/CSV import safety) has no Relay feature to attach to, and
`greetings.md` documented a specific NGConnect dashboard component with no
Relay analog. Don't recreate either speculatively; add a rule file only once
a real feature needs it.

## Precedence

If two rules conflict, resolve in this order (most specific wins):

1. A rule scoped to the exact file/folder/surface being edited
2. `security.md`, `data-access.md` (safety/correctness > style)
3. `server-client-boundary.md`, `api-conventions.md`, `rust-backend.md` (architecture correctness)
4. `code-standards-frontend.md` / `component-architecture.md` / `project-structure.md`
5. `forms-and-validation.md` / `testing.md` / `performance.md`
6. `design-system.md` / `ui-components.md` / `charts.md` / `responsive-design.md` / `accessibility.md`
7. `documentation.md`

If a conflict can't be resolved this way, stop and ask rather than guessing —
see `Relay - IDE Build Prompt.md` section 14 for what actually warrants
stopping.

## Scope

These rules apply to all code generated or edited under `native/` and `web/`.
They don't apply to one-off root-level maintenance scripts unless explicitly
asked. Every decision referenced by number here (e.g. "decision 2") refers to
`App Ideas/Relay/Relay - Decision Log.md` — read it before starting if you
haven't already; it resolves several things these rule files assume as given.
