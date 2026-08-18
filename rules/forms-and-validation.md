---
trigger: always_on
description: Standard pattern for forms and input validation
globs: "native/src/**, web/src/**"
---

# Forms & Validation

Applies to both `native/src/` and `web/src/` — the most important form in
Relay is the trigger-phrase config form (decision 10), which is what makes
"user-customizable trigger phrases" a real feature rather than a hardcoded
list.

## Rules

- Build forms with shadcn's `Form` component (`npx shadcn add form`), which
  wraps `react-hook-form`.
- Define the schema with `zod` and connect it via `@hookform/resolvers/zod`.
  Add these dependencies (`react-hook-form`, `zod`, `@hookform/resolvers`)
  in whichever surface first builds a form, rather than hand-rolling
  `useState`-per-field forms.
- Validation rules live in the `zod` schema, not scattered across
  `onChange` handlers or submit-time `if` checks.
- Re-validate with the same (or a shared) `zod` schema before persisting:
  in `native/`, that means the Rust backend re-validates a trigger-phrase
  mapping (via its own schema/struct, not literally re-running the
  frontend's zod) before writing it to the vault, since client-side
  validation alone is never sufficient. In `web/`, re-validate in the route
  handler / Server Action before writing to Supabase.
- Show field-level errors using shadcn's `FormMessage` (wires up
  `aria-describedby` automatically — keeps this consistent with
  `accessibility.md`), not ad hoc red text under inputs.
- The trigger-phrase config form specifically must validate: no duplicate
  phrases, no empty phrase or empty action mapping, and a sane action type
  (only mapping to actions the trigger system actually supports) — get this
  wrong and decision 10's whole "customizable, not hardcoded" premise breaks
  silently at runtime instead of at input time.
