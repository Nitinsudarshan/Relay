---
trigger: always_on
description: Keep the working context lean — what to read, what to delegate, and the repo's known context hazards.
---

# Context Engineering

Distilled from [gsd-core](https://github.com/open-gsd/gsd-core) v1.12.0 (MIT)
(`context-budget.md`, `universal-anti-patterns.md`).

Output quality degrades as the context window fills — silently, well before
anything errors. Relay is a large repo with several very large files, so this
is a live concern on most tasks, not a theoretical one.

## Relay's context hazards

Never read these whole. Grep, `sed -n` a range, or read the newest entry:

| File | Size | Read instead |
|---|---|---|
| `CHANGELOG.md` | ~312 KB | `head -60` for the latest entries, or grep the version |
| `maybe_later.md` | ~36 KB | grep the item number or feature name |
| `rules/readme.md` | ~32 KB | the section for the task at hand |
| `docs/` | ~520 KB / 23 files | `docs/README.md` is the index — read it first |

`version-and-changelog.md` requires inspecting the *latest* changelog entry
before every push. That means the top of the file, not the file.

If `graphify-out/` exists, `graphify query "<question>"` returns a scoped
subgraph — usually far smaller than `GRAPH_REPORT.md` or raw grep across three
surfaces. See `.agents/rules/graphify.md`.

## Read depth

- Read the **narrowest** thing that answers the question: frontmatter, a
  status field, a summary section, one function.
- Don't re-read a file you just edited to confirm the edit landed.
- Read the whole function, not the "relevant" lines — skimming a function to
  save tokens is how the sibling-caller bug gets missed (`debugging.md`).
  Narrow the *file set*, not your reading of the file you chose.

## Delegating

- **Delegate heavy work to subagents.** The orchestrating session routes — it
  should not itself be the thing doing a wide search across `native/`,
  `web/`, and `native/src-tauri/`.
- **Never inline a large file into a subagent prompt.** Tell the agent the
  path and let it read from disk with its own context window.
- Give a subagent the task and the constraints, not a transcript of how you
  got here.
- The orchestrator can only verify *structural* completeness of a subagent's
  output — not semantic correctness. Spot-check the substance against the
  actual requirement (`verification-honesty.md`).

## Degradation tiers

| Tier | Usage | Behavior |
|---|---|---|
| PEAK | 0–30% | Full operations — read bodies, fan out, inline results |
| GOOD | 30–50% | Prefer narrow reads, delegate aggressively |
| DEGRADING | 50–70% | Economize. Narrow reads only, minimal inlining, tell the user the budget is getting heavy |
| POOR | 70%+ | Checkpoint now. Commit or write state down before continuing |

## Warning signs

Quality drops before any threshold fires. Treat these as context pressure,
whatever the counter says:

- **Silent partial completion** — a task reported done where the
  implementation is incomplete. Files exist; behavior doesn't.
- **Increasing vagueness** — "appropriate handling", "standard patterns",
  "the usual approach" replacing specific code or specific file paths.
- **Skipped steps** — a checklist of 8 items reported against 5. On Relay,
  the tell is skipping `version-and-changelog.md` or the `cargo clippy` gate.

When you hit one: checkpoint, then start clean rather than pushing through.

## MCP schema tax

Every enabled MCP server injects its tool schema into **every turn**, whether
or not it's called — heavyweight servers cost 20k+ tokens per turn each. This
is a harness setting, not a repo setting: `enabledMcpjsonServers` /
`disabledMcpjsonServers` in `.claude/settings.json`.

Before a long session, disable what this task can't use: browser/Playwright
tools on a Rust-only task, OS-specific helpers, servers added for another
project, and duplicate servers offering the same tools. Relay's own MCP client
wiring under `native/src-tauri/src/mcp/` is unrelated to this — that's product
code, not session tooling.

## Anti-patterns

- Reading a file to "have context" without a question it answers.
- Re-litigating decisions already locked in `docs/decisions.md` — respect them
  or raise the conflict explicitly (`global.md` precedence).
- `git add .` / `git add -A` — stage specific files (a whisper.cpp build leaves
  artifacts).
- Writing planning documents the user didn't approve.
- Modifying files outside the stated scope of the task.
