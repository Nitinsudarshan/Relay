---
trigger: model_decision
description: Debugging discipline and the bug-pattern checklist to scan before forming a hypothesis.
---

# Debugging

Distilled from [gsd-core](https://github.com/open-gsd/gsd-core) v1.12.0 (MIT)
(`debugger-philosophy.md`, `common-bug-patterns.md`).

## Roles

The user knows what they expected, what happened, the error text, and when it
started. They do **not** know the cause, the file, or the fix — don't ask.
Ask about the experience; investigate the cause yourself.

## Debugging code you wrote

Harder, not easier: you remember the intent, not what you actually
implemented, and your design decisions feel obviously correct.

1. Read it as if someone else wrote it.
2. Treat your implementation decisions as hypotheses, not facts.
3. The code's behavior is truth; your mental model is a guess.
4. Prioritize what you touched most recently — those are prime suspects.

The hardest admission is "I implemented this wrong", not "the requirements
were unclear".

## Scan the checklist before hypothesizing

These cover most bugs. Match the symptom to a category first.

| Symptom | Check first |
|---|---|
| `Cannot read property of undefined` | Null/undefined access |
| `X is not a function` | Import/module shape, type coercion |
| Works sometimes, fails sometimes | Async/timing, state |
| Works locally, fails in CI | Environment/config, path case |
| Wrong data on screen | Data shape at a boundary, stale state |
| Off by one item / last item missing | Boundary condition |
| Memory or handle growth | Missing cleanup, closure capture |

Relay-specific instances of each:

- **Tauri IPC boundary** — the most common data-shape mismatch in this repo.
  A Rust struct's serde shape vs the TS type the frontend expects; `snake_case`
  vs `camelCase`; `Option<T>` arriving as `null` vs absent; an enum serialized
  as a tag object rather than a string. Both sides must change together
  (`api-conventions.md`).
- **Async/timing** — a missing `.await`, a 30-second chunk landing after the
  recording stopped, a Tokio task outliving the window, two writers to the same
  vault file, a `tokio::spawn` whose `JoinHandle` is dropped so its panic is
  never seen.
- **React state** — a stale closure in a chunk-arrival handler capturing the
  transcript at bind time, state updated with a mutated array so no re-render
  fires, two sources of truth for the same meeting (server state and local
  state drifting).
- **Environment/config** — path **case sensitivity**: this repo builds on
  Windows and CI runs Linux, so `Components/` vs `components/` passes locally
  and fails CI. Also missing env vars per `security.md`, and the
  CMake/GTK/ALSA toolchain deps the whisper.cpp build needs.
- **Model/audio reality** — Whisper transcribing room tone into plausible
  subtitle filler is not a code bug and not fixed by retrying. Check the
  gate/screen path before hunting downstream.
- **Type/coercion** — `"5" > "10"` is `true`; `0` and `""` are valid values
  that are falsy; f32/f64 precision on audio timestamps and talk-share
  percentages.
- **Error handling** — an empty `catch {}`, a `let _ =` on a `Result`, an error
  logged but not surfaced. In Rust, per `rust-backend.md`, propagate.

Each matched pattern is a hypothesis candidate — verify or eliminate it with
evidence. If nothing matches, go open-ended.

## Discipline

- **Change one variable.** One change, test, observe, write it down, repeat.
  Multiple changes at once means you learn nothing from the result.
- **Read completely.** Whole functions, plus imports, config, and the tests.
  Cross-surface bugs need both sides read (`context-engineering.md` says
  narrow the file set, not your reading of the file).
- **Root cause, not symptom.** Grep every caller of the function you're about
  to touch and fix it once where they all route through
  (`lazy-code-ladder.md`).
- **"I don't know why this fails" is a good state.** "It must be X" is
  dangerous — you've stopped thinking.

## Cognitive biases

| Bias | Trap | Antidote |
|---|---|---|
| Confirmation | Only seeking evidence that supports your hypothesis | Ask "what would prove me wrong?" and go look for that |
| Anchoring | The first explanation becomes the frame | Generate 3+ independent hypotheses before investigating any |
| Availability | Recent bug → assume the same cause | Treat each bug as novel until evidence says otherwise |
| Sunk cost | Two hours down one path, keep going | Every 30 min: "starting fresh, is this still the path?" |
| Single-cause | A linear why-chain stops at one cause; the unaddressed second cause brings the bug back | Branch across at least two categories before committing to a root cause |

## When to restart

Restart when: 2+ hours with no progress; 3+ "fixes" that didn't work; you
can't explain the current behavior; the fix works and you don't know why (that
isn't fixed, that's luck).

Restart protocol: write down what you know for certain, write down what you've
ruled out, list hypotheses that are *different* from the previous ones, begin
from evidence again.

## Closing a bug

Reproduce the original failure, fix the root cause, show the same check
passing, and add the one runnable check that fails if it regresses
(`testing.md`). Report it per `verification-honesty.md` — if the real fix can
only be confirmed on Windows hardware, say that rather than calling it verified.
