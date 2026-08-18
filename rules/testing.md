---
trigger: always_on
description: What to test, what not to, and where tests live — across all three surfaces
---

# Testing Rules

## Rust backend (native/src-tauri/)

- Use `cargo test` with standard `#[test]`/`#[tokio::test]` — no separate
  framework decision needed, unlike the frontend's Vitest choice below.
- Prioritize testing the two flagged technical risks first (see
  `Relay - IDE Build Prompt.md`): `pipeline/` (meeting→Kanban extraction
  quality) and `triggers/` (trigger-phrase matching, including
  false-positive/false-negative behavior across a range of user-defined
  phrases, not just the two examples from research). These carry the actual
  product risk.
- Test `providers/`'s trait-based Ollama/cloud-LLM swap with a fake/mock
  provider — don't hit a real LLM API or a real Ollama instance in CI.
- Don't chase coverage on `commands.rs` itself — it should be thin enough
  (per `rust-backend.md`) that the real logic being tested lives in the
  modules it calls, not in the command wrapper.

## Frontend (native/src/ and web/src/)

- Use Vitest + React Testing Library for unit/component tests in both
  surfaces — don't introduce a second framework later without discussion.
- Prioritize testing over presentation: the trigger-phrase config form's
  validation logic and any client-side transcript/Kanban formatting logic
  get unit tests first.
- Don't write tests for shadcn/ui primitives themselves — test your own
  logic that wraps/composes them.
- Don't chase 100% coverage on simple presentational components with no
  branching logic.
- Co-locate test files next to the code they test:
  `useTriggerMatch.ts` → `useTriggerMatch.test.ts` in the same folder.
- For anything touching Supabase in a `web/` test, use a mocked client —
  never point tests at a real project database.
