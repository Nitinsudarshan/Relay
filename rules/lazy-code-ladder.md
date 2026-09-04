---
trigger: always_on
description: YAGNI ladder — stop at the first rung that works before writing new code. Applies to every surface.
---

# Lazy Code Ladder

Adapted from [ponytail](https://github.com/DietrichGebert/ponytail) v4.9.0 (MIT),
scoped to Relay's three surfaces.

Lazy means efficient, not careless. The best code is the code never written.
Relay is a pre-alpha app with three surfaces and 1,500+ tests already carrying
the weight — every unnecessary abstraction is a thing that has to keep passing
CI forever.

## The ladder

Before writing code, stop at the first rung that holds:

1. **Does this need to exist at all?** Speculative need → skip it and say so in
   one line. If it's a real future want, it goes in `maybe_later.md`
   (see `maybe-later.md`), not into the code as a stub or ghost affordance.
2. **Already in this repo?** Check before writing:
   - Rust: the domain modules under `native/src-tauri/src/`
     (`capture/`, `pipeline/`, `triggers/`, `providers/`, `vault/`, `mcp/`) —
     a feature's parsing/validation/persistence already lives together there.
   - React: `native/src/lib/` and `native/src/components/`, `web/src/hooks/`
     and `web/src/lib/`, before adding a hook or util.
   - UI: the already-generated shadcn primitives in `<surface>/components/ui`.
   - If a knowledge graph exists at `graphify-out/`, query it
     (`graphify query "<question>"`) instead of grepping blind — see
     `.agents/rules/graphify.md`.
3. **Does the stdlib do it?** Rust `std` / TS built-ins (`Intl`, `URL`,
   `structuredClone`, `Array.prototype.*`) before anything else.
4. **Does a platform feature cover it?** `<input type="date">` over a picker
   lib, CSS over JS, a Tauri/WebView2 API over a shim, a LanceDB or SQL
   constraint over app-level validation.
5. **Does an already-installed dependency solve it?** Check `Cargo.toml` and
   both `package.json` files. Never add a new dependency for what a few lines
   cover — a new crate is also a new CMake/toolchain risk for the whisper.cpp
   build (see the README's build notes).
6. **Can it be one line?** One line.
7. **Only then:** the minimum code that works.

The ladder runs **after** you understand the problem, not instead of it. Read
the task and the code it touches, trace the real flow end to end, then climb.
Two rungs both work → take the higher one and move on.

## Rules

- No abstraction that wasn't asked for: no trait with one implementor, no
  factory for one product, no config key for a value that never changes.
- No boilerplate or scaffolding "for later". Later can scaffold for itself.
- Deletion over addition. Boring over clever — clever is what someone decodes
  at 3am. Fewest files possible.
- Shortest working diff wins, **but only once you understand the problem**.
  The smallest change in the wrong place isn't lazy, it's a second bug.
- Two options the same size → take the one that's correct on edge cases. Lazy
  means less code, not the flimsier algorithm.
- Complex request → ship the lazy version and question it in the same
  response: "Did X; Y covers it. Need full X? Say so." Don't stall on an
  answer you can default.
- **Bug fix = root cause, not symptom.** A report names a symptom. Grep every
  caller of the function you're about to touch and fix the shared function
  once — one guard there is a smaller diff than one per caller, and patching
  only the path the ticket names leaves every sibling caller broken. See
  `debugging.md`.

## Marking deliberate shortcuts

A simplification that cuts a real corner with a known ceiling (a global lock,
an O(n²) scan over what will grow, a naive heuristic, a single-speaker
assumption) gets a greppable comment naming the ceiling **and** the upgrade
trigger:

```rust
// ponytail: whole-vault rescan per save, index it if the vault passes ~5k notes
```

```ts
// ponytail: single retry, back off properly once cloud sync is on by default
```

Marker is `ponytail:` so ponytail's own `/ponytail-debt` harvester works
unchanged if the plugin is installed. Harvest the ledger with:

```bash
grep -rnE '(#|//) ?ponytail:' --exclude-dir={.git,node_modules,target,graphify-out} .
```

A shortcut comment with **no** named upgrade trigger is the one that silently
rots — always name the trigger.

## Never lazy about

Input validation at trust boundaries, error handling that prevents data loss
(the vault is the user's only copy — see `data-access.md`), security
(`security.md`), untrusted captured content (`untrusted-input.md`),
accessibility (`accessibility.md`), and anything explicitly requested. If the
user insists on the full version, build it — no re-arguing.

Never lazy about **understanding**. The ladder shortens the solution, never the
reading.

Hardware and models are never the spec ideal: a real mic clips, a real clock
drifts, Whisper invents words over room tone. Leave the calibration knob, not
just less code.

**Lazy code without its check is unfinished.** Non-trivial logic (a branch, a
parser, a chunk-gating decision, a money/security path) leaves one runnable
check behind — the smallest thing that fails if the logic breaks. Follow
`testing.md` for framework and placement. Trivial one-liners need no test;
YAGNI applies to tests too.
