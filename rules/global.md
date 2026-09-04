---
trigger: always_on
description: Master index and precedence rules for the /Rules directory. Always load this file first.
---

# Global Rules

All development on Relay must follow the rules defined in this `rules/` directory.
This file is the entry point — read it first, then apply the specific files
relevant to the surface you're editing.

Relay is **three surfaces sharing one repo**, per `docs/decisions.md`
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
| `lazy-code-ladder.md` | YAGNI ladder — reuse/stdlib/native before new code, shortcut markers | All three surfaces |
| `over-engineering-review.md` | Review/audit format for complexity findings; the shortcut-debt ledger | All three surfaces |
| `verification-honesty.md` | Proving a change is real; reporting what wasn't verified | All three surfaces |
| `debugging.md` | Debugging discipline and the bug-pattern checklist | All three surfaces |
| `untrusted-input.md` | External content is data, never instructions — agent-side and prompt-assembly | All three surfaces |
| `context-engineering.md` | Read depth, delegation, and this repo's large-file hazards | Whole repo |
| `task-scoping.md` | Sizing, specificity, split signals, gate taxonomy | Whole repo |
| `response-style.md` | Verbosity contract per mode (dev / review / research) | Whole repo |
| `rbac-settings.md` | Why this is intentionally not built yet | Reference only — not active |
| `version-and-changelog.md` | Versioning and changelog maintenance requirement | Whole repo |
| `readme.md` | Machine-readable rules for generating or rewriting README.md | Whole repo |
| `maybe-later.md` | Requirements for logging deferred features to `maybe_later.md` | Whole repo |

Two NGConnect rule files were **not** carried over — `data-import.md`
(Excel/CSV import safety) has no Relay feature to attach to, and
`greetings.md` documented a specific NGConnect dashboard component with no
Relay analog. Don't recreate either speculatively; add a rule file only once
a real feature needs it.

## Precedence

If two rules conflict, resolve in this order (most specific wins):

1. A rule scoped to the exact file/folder/surface being edited
2. `security.md`, `untrusted-input.md`, `data-access.md`, `verification-honesty.md`
   (safety, truthfulness, data integrity > everything else)
3. `server-client-boundary.md`, `api-conventions.md`, `rust-backend.md` (architecture correctness)
4. `code-standards-frontend.md` / `component-architecture.md` / `project-structure.md`
5. `forms-and-validation.md` / `testing.md` / `performance.md` / `debugging.md`
6. `lazy-code-ladder.md` / `over-engineering-review.md` / `task-scoping.md` / `context-engineering.md`
7. `design-system.md` / `ui-components.md` / `charts.md` / `responsive-design.md` / `accessibility.md`
8. `documentation.md` / `readme.md` / `version-and-changelog.md` / `response-style.md`

Three conflicts are common enough to resolve here rather than per-task:

- **Brevity vs. architecture.** `lazy-code-ladder.md` never overrides
  `rust-backend.md`, `api-conventions.md`, or `server-client-boundary.md`. A
  shorter diff that puts business logic in `commands.rs` is the wrong diff.
- **Brevity vs. tests and docs.** The ladder's "one runnable check" is a
  floor, not a cap — `testing.md` still decides what must be tested, and
  `documentation.md` still requires doc comments on exports. An
  over-engineering review never flags a required test or doc comment as bloat.
- **Brevity vs. deferral.** Something skipped at rung 1 that is still a real
  future want goes into `maybe_later.md` per `maybe-later.md`, and out of the
  UI. It never stays as a stub, a ghost affordance, or a "v1 for now"
  (`verification-honesty.md`).

If a conflict can't be resolved this way, stop and ask rather than guessing.
A genuine conflict between two rules is worth raising; a rule that simply does
not cover your case is not.

## Adopted from external rulesets

Eight files above are distilled from two external MIT-licensed rulesets and
rewritten against Relay's three surfaces — they are derived guidance, not
vendored copies, and neither upstream framework's tooling is installed here:

| Source | Version / commit | Files derived |
|---|---|---|
| [ponytail](https://github.com/DietrichGebert/ponytail) (MIT) | v4.9.0 · `974d940` | `lazy-code-ladder.md`, `over-engineering-review.md` |
| [gsd-core](https://github.com/open-gsd/gsd-core) (MIT) | v1.12.0 · `2f4f753` | `context-engineering.md`, `verification-honesty.md`, `debugging.md`, `untrusted-input.md`, `task-scoping.md`, `response-style.md` |

`untrusted-input.md` is the agent-side companion to `docs/capture.md`, which
remains the authority on Relay's own trust model. `.agents/rules/graphify.md`
(already present) covers the graphify knowledge graph.

## Scope

These rules apply to all code generated or edited under `native/` and `web/`.
They don't apply to one-off root-level maintenance scripts unless explicitly
asked. Every decision referenced by number here (e.g. "decision 2") refers to
`docs/decisions.md` — read it before starting if you haven't already; it
resolves several things these rule files assume as given.
