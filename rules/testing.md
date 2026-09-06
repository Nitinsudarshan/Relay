---
trigger: always_on
description: What to test, what not to, and where tests live
---

# Testing Rules

## Rust backend (native/src-tauri/)

- Use `cargo test` with standard `#[test]`/`#[tokio::test]`.
- Prioritize the areas that carry real product risk: `meetings_v2/`
  (extraction quality, diarization, alignment), `pipeline/`, and `triggers/`
  (trigger-phrase matching, false-positive and false-negative behavior).
- Test provider trait swaps with fake/mock providers — don't hit real LLM APIs
  in automated CI tests.
- Don't chase coverage on `commands.rs` itself — it should be thin enough
  (per `rust-backend.md`) that the real logic lives in the modules it calls.

## Frontend (native/src/)

- Use Vitest + React Testing Library for unit/component tests in `native/src/`.
- Prioritize testing state management, formatting, and custom hooks over
  pure presentation.
- Don't write tests for shadcn/ui primitives themselves — test your own
  logic that wraps/composes them.
- Don't chase 100% coverage on simple presentational components with no
  branching logic.
- Co-locate test files next to the code they test:
  `useTriggerMatch.ts` → `useTriggerMatch.test.ts` in the same folder.

