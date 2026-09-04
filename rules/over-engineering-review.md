---
trigger: model_decision
description: Review a diff or the repo for over-engineering. One line per finding, tagged, with the replacement named.
---

# Over-Engineering Review

Adapted from [ponytail](https://github.com/DietrichGebert/ponytail) v4.9.0 (MIT)
(`ponytail-review` / `ponytail-audit` / `ponytail-debt`).

Use this when reviewing a diff, auditing a surface, or asked what to cut. It is
the review-time counterpart to `lazy-code-ladder.md`. Findings are listed, not
applied, unless the fix was asked for.

## Format

One line per finding — no paragraphs:

`<file>:L<line>: <tag> <what to cut>. <what replaces it>.`

Tags:

| Tag | Means | Replacement |
|---|---|---|
| `delete:` | Dead code, unused flexibility, speculative feature | Nothing |
| `stdlib:` | Hand-rolled thing `std`/TS built-ins ship | Name the function |
| `native:` | Code or dep doing what the platform already does | Name the feature |
| `yagni:` | Trait/abstraction with one implementor, config nobody sets, layer with one caller | Inline it |
| `shrink:` | Same logic, fewer lines | Show the shorter form |

Examples:

```
native/src/lib/format.ts:L12-38: stdlib: 27-line duration formatter. Intl.DurationFormat, 1 line.
native/src-tauri/src/vault/store.rs:L88: yagni: VaultStore trait, one impl. Inline until a second backend exists.
web/src/lib/date.ts:L4: native: moment.js for one format call. Intl.DateTimeFormat, 0 deps.
native/src/hooks/useChunk.ts:L52-71: delete: retry wrapper around a local synchronous call. Nothing replaces it.
```

End with the only metric that matters: `net: -<N> lines, -<M> deps possible.`
Nothing to cut → `Lean already. Ship.` and stop.

## Where to hunt in Relay

- Traits in `native/src-tauri/src/` with a single implementor. `providers/`
  legitimately has the Ollama/cloud swap (`testing.md` depends on a mock) —
  most others do not.
- A second formatter/parser for transcripts, durations, or chunk headers when
  one already exists in the other frontend. Shared shapes belong in
  `packages/shared` once needed twice, not duplicated (`project-structure.md`).
- Business logic that leaked into `commands.rs` — it should be thin
  (`rust-backend.md`), so logic there is a finding.
- Hand-written UI primitives next to the generated shadcn ones.
- Wrappers that only delegate, files exporting one thing, dead feature flags,
  config keys nothing reads.
- New dependencies where `Cargo.toml` or either `package.json` already carries
  something that covers it.

## Repo-wide audit

Same tags, whole tree instead of a diff, ranked biggest cut first. Exclude
`.git`, `node_modules`, `target`, `graphify-out`. State the net figure at the
end.

## Debt ledger

Harvest the deliberate shortcuts marked per `lazy-code-ladder.md`:

```bash
grep -rnE '(#|//) ?ponytail:' --exclude-dir={.git,node_modules,target,graphify-out} .
```

One row per marker, grouped by file:

`<file>:<line> — <what was simplified>. ceiling: <the limit>. upgrade: <the trigger>.`

Tag any marker with no named upgrade trigger `no-trigger` — those are the ones
that rot. End with `<N> markers, <M> with no trigger.`

## Boundaries

- **Scope is complexity only.** Correctness bugs, security holes, and
  performance regressions are out of scope here — route them to a normal
  review against `security.md`, `performance.md`, and `debugging.md`. Saying
  "also this is a security hole" in an over-engineering review is fine;
  *replacing* the review with one is not.
- **Never flag the one runnable check** that `lazy-code-ladder.md` requires,
  or a test required by `testing.md`, as bloat.
- Never report a per-repo savings number ("this saved X lines"). The unbuilt
  version was never written, so there is no baseline to subtract from. The
  debt ledger is the only real counted per-repo figure.
- Lists findings; applies nothing unless asked.
