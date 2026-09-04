---
trigger: always_on
description: Existence is not implementation. How to prove a change works, and how to report what you could not verify.
---

# Verification Honesty

Distilled from [gsd-core](https://github.com/open-gsd/gsd-core) v1.12.0 (MIT)
(`verification-patterns.md`, `honest-verifier.md`, `failing-direction.md`,
`planner-antipatterns.md`).

`testing.md` says what to test. This says how to prove a change is real, and
what to say when you can't.

## Existence ≠ implementation

Four levels. Report which one you actually reached:

1. **Exists** — the file/function/command is present.
2. **Substantive** — the body is real logic, not a placeholder.
3. **Wired** — it is reachable from the rest of the system.
4. **Functional** — it does the thing when invoked.

Levels 1–3 are checkable mechanically. Level 4 usually needs a run or a human.
"The file exists and exports the command" is level 1 — never report it as done.

## Stub patterns in Relay's stack

**Rust (`native/src-tauri/`):** `todo!()`, `unimplemented!()`, `Ok(())` from a
function that should persist something, a `#[tauri::command]` that returns a
hardcoded shape, `let _ = result;` swallowing an error, `.unwrap()` standing in
for the error path `rust-backend.md` requires.

**React (`native/src/`, `web/src/`):** `return null` / `<></>` from a component
that should render, `<div>Placeholder</div>`, `onClick={() => {}}`,
`onSubmit={(e) => e.preventDefault()}` and nothing else, hardcoded counts or
sample transcript text where live data belongs.

**Pipeline (`pipeline/`, `capture/`):** a chunk-gating or speaker-separation
path that always returns the same verdict, a summary derived from a fixed
template rather than the transcript, a rejected-chunk reason string that is
never actually set.

Grep before claiming done:

```bash
grep -rnE 'todo!|unimplemented!|TODO|FIXME|PLACEHOLDER|coming soon' \
  --exclude-dir={.git,node_modules,target,graphify-out} native/src native/src-tauri/src web/src
```

A `ponytail:` shortcut marker (`lazy-code-ladder.md`) is a *declared* ceiling,
not a stub — don't report it as one.

## Every check names its failing direction

A verification command with no expressible failure mode is not a check. It
reads as rigour and delivers none — a command that exits 0 on a no-op passes
green and silently.

State what failure looks like, alongside the command:

```
cd native/src-tauri && cargo test pipeline::gate   # fails on: non-zero exit, or "0 passed"
cd native && npm test -- useTriggerMatch           # fails on: any failed assertion, or 0 tests matched
cd web && npx tsc --noEmit                         # fails on: any TS error line
```

"non-zero exit" is a complete failing direction — there is no minimum length.
What isn't acceptable: "it fails if it doesn't work", "an error occurs". Those
restate the word failure without naming an observable signal.

A test filter that matches zero tests and exits 0 is a **vacuous pass**. Check
the count, not just the exit code.

## Report what you could not verify

You cannot self-detect a gap you don't perceive, so a confident "passed" on
something you never observed is the worst failure mode — nobody knows to look.

- Verified by an observed run or a passing test → **verified**, name the
  evidence.
- Not observable from here (needs Windows, a real mic, a multi-speaker call,
  a live Google Calendar, a real Supabase project) → **unverified**, and say
  what would verify it. Never "passed".
- Symbol present and wired, behavior unobserved → **wired, behavior
  unverified**. That is a distinct state from verified.

Don't over-correct: something the tests genuinely cover is verified. Abstaining
on everything is as useless as passing everything.

## No silent scope reduction

These phrases in a plan, a commit message, or a code comment mean scope was cut
without saying so:

> "v1", "simplified version", "static for now", "hardcoded for now",
> "basic version", "minimal implementation", "will be wired later",
> "dynamic in a future phase", "skip for now", "placeholder"

If the work is too large, say so and propose a split. If it's deliberately
deferred, it goes in `maybe_later.md` with the blueprint (`maybe-later.md`) and
comes *out* of the UI — Relay's no-ghost-UI rule exists precisely because a
"static for now" affordance reads to the user as a working feature.

A declared ceiling with a named upgrade trigger (`lazy-code-ladder.md`) is the
honest form of the same instinct. Use that instead.

## Before reporting done

- The repo's own gates, actually run:
  `cd native/src-tauri && cargo clippy --all-targets -- -D warnings && cargo test`,
  `cd native && npm test && npm run typecheck`,
  `cd web && npx tsc --noEmit && npm run build`.
- For a bug fix: the original failure reproduced first, then the same check
  passing.
- `VERSION` and `CHANGELOG.md` updated, with every changelog line checked
  against the real diff (`version-and-changelog.md`).
- If a gate was not run, say which and why. A skipped gate reported as green
  is the failure this whole file is about.
