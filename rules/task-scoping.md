---
trigger: always_on
description: How to size and specify a unit of work, when to split it, and which gate to use at each checkpoint.
---

# Task Scoping

Distilled from [gsd-core](https://github.com/open-gsd/gsd-core) v1.12.0 (MIT)
(`the-phase-loop.md`, `planner-antipatterns.md`, `gates.md`,
`context-budget.md`).

Relay does not run GSD's `.planning/` machinery, and adopting it isn't the
point of this file. What ports is the sizing and specificity discipline: the
failure modes it guards against are the ones a three-surface repo hits anyway.

## Good scope

A well-sized unit of work:

- States its goal in one sentence that is neither trivial nor suspiciously
  broad.
- Has bounded unknowns — the questions have answers that don't depend on other
  work finishing first.
- Splits into a handful of non-overlapping changes, not dozens.
- Has a testable definition of done that can be checked without reading the
  whole repo.

For Relay: *"Reject a chunk whose RMS never clears the speech gate, recording
the reason"* is right-sized. *"Improve meeting intelligence"* is not — it
contains several independent concerns. *"Fix the typo in the README"* is below
the threshold where any of this adds value; just do it.

When in doubt, split. A smaller unit finishes faster, verifies more
confidently, and is cheaper to abandon when a decision turns out wrong.

## Split signals

**Always split when:**

- More than ~3 tasks in one unit.
- It crosses subsystems. On Relay that means **Rust backend + native frontend
  + web dashboard are separate units** — the surfaces have different test
  gates, different conventions (`rust-backend.md` vs
  `code-standards-frontend.md`), and different reviewers' attention.
- Any single task touches more than ~5 files.
- Discovery and implementation are in the same unit — find out first, then
  decide.
- A human checkpoint sits in the middle of implementation.

**Consider splitting** at natural seams: the Tauri IPC boundary, the
vault-write boundary, the pipeline stage boundary.

Aim to finish a unit inside roughly half the context window, not 80% — leaving
room for the unexpected is what keeps quality even from start to finish
(`context-engineering.md`).

## Specificity

**The test: could a different agent, with no memory of this conversation,
execute the task without asking a clarifying question?** If not, add detail.

| Too vague | Just right |
|---|---|
| "Handle errors" | "Return `Err(PipelineError::GateRejected { reason })` from `gate()`, surface it as the chunk's rejection reason in the meeting header, toast on the frontend" |
| "Add speaker names" | "Accept a typed name correction `{chunk_id, speaker_id, name}` from the meeting view, apply it to subsequent chunks in the same recording, leave the immutable STT output untouched" |
| "Style the diagnostics panel" | "Diagnostics telemetry cards: 3-col grid at `lg`, 1-col below, tokens from `design-system.md`, RMS/peak values right-aligned monospace" |
| "Set up the sync" | "Add a `meetings` table to Supabase with UUID id, `user_id` FK, `recorded_at` timestamptz; push on meeting finalize only in hybrid mode" |

## Anti-patterns

- **Asking the user to do what you can automate.** If there's a CLI or an API
  for it, run it. Reserve checkpoints for things only a human can judge — how
  a recording actually sounded, whether a summary reads right.
- **Verification fatigue.** One checkpoint after a meaningful whole, not one
  after every step.
- **Reflexive context chaining.** Don't carry along every prior file's output
  "for context". Reference the earlier work only where the current task
  genuinely uses a type, an export, or a decision from it.
- **Silent scope reduction.** Never ship less and describe it as more — see
  the prohibited-phrase list in `verification-honesty.md`. Propose a split
  instead.
- **Re-litigating locked decisions.** `docs/decisions.md` decisions are
  settled. Raise a genuine conflict; don't quietly reopen one
  (`global.md` precedence).

## Gates

Every checkpoint is one of four kinds. Naming the kind makes the failure
behavior obvious:

| Gate | Purpose | On failure | Relay example |
|---|---|---|---|
| **Pre-flight** | Validate preconditions before starting | Block entry, no partial work | Cargo/npm deps and CMake toolchain present before a build task |
| **Revision** | Judge output quality, loop back with specific feedback | Loop, with an iteration cap | `cargo clippy -D warnings` → fix → re-run (cap the loop; escalate if the count stops dropping) |
| **Escalation** | Surface an unresolvable decision to the user | Pause, present options, wait | Two calendar events fit a recording equally well — Relay shows both rather than guessing |
| **Abort** | Stop to prevent damage or waste | Stop, preserve state, report why | Context in the POOR tier mid-task; a vault write that would overwrite the user's only copy |

Selection heuristic: start with pre-flight. If the check happens *after* work
is produced, it's a revision gate. If the revision loop can't resolve it,
escalate. If continuing is dangerous, abort. Always cap a revision loop — an
uncapped one is an infinite loop with extra steps.
