---
trigger: always_on
description: How much to say, per mode — dev, review, and research have different verbosity contracts.
---

# Response Style

Distilled from [gsd-core](https://github.com/open-gsd/gsd-core) v1.12.0 (MIT)
context profiles, plus ponytail's output discipline. `lazy-code-ladder.md`
governs what gets built; this governs what gets said.

Pick the mode from the task, not the mood. Default to **dev**.

## Dev (default)

Building, fixing, refactoring.

- Lead with the code change or command; brief rationale after.
- Skip preamble — assume full context.
- Inline references (`native/src-tauri/src/pipeline/mod.rs:142`) over prose
  descriptions of where something is.
- Flag side effects and breaking changes immediately.
- End with the next actionable step.
- **Verbosity: low.** One-liner explanations unless the change is non-obvious.
  Omit background theory, alternatives not taken, and caveats that don't
  affect this task.

Pattern: `[code] → skipped: [X], add when [Y].` If the explanation is longer
than the code, cut the explanation — a paragraph defending a simplification is
complexity smuggled back in as prose.

## Review

Reviewing a diff, auditing a surface, security-reviewing a branch.

- Organize findings by severity: **blocking**, **important**, **nit**.
- Every finding cites file and line.
- Say what is correct too — confirm the good parts, don't only list defects.
- Cover: correctness (logic, off-by-one, missed edge cases), security
  (`security.md`, `untrusted-input.md`), performance (`performance.md`),
  consistency (`code-standards-frontend.md`, `rust-backend.md`), and test
  coverage (`testing.md`).
- **Verbosity: medium.** Thorough on findings, terse in explanation — one to
  three sentences each: what's wrong, why it matters, how to fix it.

For complexity-only passes use `over-engineering-review.md`'s tag format
instead.

## Research

Choosing a library, designing a pipeline stage, writing to `docs/`.

- Enumerate options before narrowing; give pros and cons, then recommend one.
- Include links, versions, and publication dates.
- Cover risks, failure modes, dependency and compatibility implications, and
  long-term maintainability.
- **Verbosity: high.** Explain the reasoning, show the evidence, state the
  assumptions — research artifacts get read by future contributors who weren't
  in the conversation. `docs/` is the living spec (`project-structure.md`), so
  write for that reader.

## Always

- Explanation the user explicitly asked for (a report, a walkthrough,
  per-phase notes) is never debt — give it in full. The rule is only against
  *unrequested* prose.
- One primary next action, alternatives listed as secondary. Don't hand over
  three equally-weighted options with no recommendation.
- Report outcomes as they are: if tests fail, show the output; if a step was
  skipped, say so.
