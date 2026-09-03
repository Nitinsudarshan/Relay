# Capture — what the reveal pass actually costs

Measured at v0.27.0, in headless Chromium 1194 (Playwright build), 1280×900
viewport, on the container this work was done in — a shared cloud runner, not a
quiet workstation. Reproduce with:

```bash
cd native && npm run build:extension
node ../scripts/capture-validation/run.mjs
```

Every number below comes from that run, and the runner prints them, so this
file can be checked rather than believed. What is **not** measured is stated at
the bottom; that section matters as much as the table.

## The four scenarios

| Page | Shape | Time | Steps | Samples | Payload | Result |
|---|---|---|---|---|---|---|
| `claude-truncated` | 4-turn conversation, one CSS-shortened message, a collapsed thinking block, an artifact card | **61 ms** | 0 | 1 | 3.5 KiB | 4/4 turns; nothing clicked |
| `chatgpt-virtualized` | 300-turn virtualized thread, network-paged history, opened at the bottom | **6.06 s** | 74 | 76 | 36.0 KiB | 300/300 turns, no gaps |
| `lazy-article` | Long article: 6 lazily-appended sections, a lazy image, a clipped paragraph, a `<details>` | **792 ms** | 9 | 11 | 3.8 KiB | all 6 sections; image source resolved |
| `hostile-controls` | 15 action controls wearing disclosure markup, plus 2 genuine ones | **< 100 ms** | 0 | 1 | — | 0 actions fired; 1 disclosure opened |

JS heap during the 300-turn run: **2 MiB**. The merger holds one entry per
distinct turn, so memory tracks the conversation rather than the number of
samples — 76 samples produced 300 entries and 228 recognised repeats.

## Against the expectations set before measuring

`RESEARCH.md` §10 predicted these before any of it was built:

| Shape | Predicted | Measured | |
|---|---|---|---|
| Static article | under 1s | 792 ms | ✅ |
| Non-virtualized conversation, ~100 turns | ~1s | 61 ms at 4 turns | ✅ (and see below) |
| Virtualized conversation, 500 turns | 3–8s | 6.06 s at 300 turns | ⚠️ optimistic |
| Virtualized, 1,000+ turns | budget-bound, reported | budget-bound, reported | ✅ |

The one that was optimistic is worth being precise about. Stepping by the
mounted extent means the cost per *turn* depends on how many turns the page
mounts at once: this fixture mounts 7, so 300 turns took 74 steps — roughly 4
turns per step, at ~80 ms a step. A page that mounts 20 turns at a time would
cover the same thread in a third of the steps. Extrapolating this fixture's
density, the 10-second default budget covers **around 450–500 turns**, and a
1,000-turn thread of this shape will stop at `time_budget` and report
`partial`. That is the designed outcome — not a two-minute capture, and not a
silent one — but "3–8s for 500 turns" was the top of the range, not the middle.

The Claude row is fast for a reason that is the point of the whole design
rather than a property of the fixture: the engine measured that the page mounts
everything, so it never stepped at all (`steps: 0`), and it found both
shortened sections already present in the DOM, so it clicked nothing. The
expensive path is skipped when it would buy nothing.

## Where the time goes

The dominant cost is settling, not scrolling or extracting. Two measurements
made that concrete:

- At `settlePollMs: 50` with two quiet ticks, the 300-turn walk cost **160 ms a
  step** and ran out of its 10-second budget **45 turns short** (255/300,
  `time_budget`). At 25 ms it costs ~80 ms a step and finishes in 6 s with all
  300. The settle ceiling (1.2 s) is untouched in every scenario here; it
  exists for pages where content genuinely keeps arriving.
- Raw scrolling with no settle at all, measured separately in the same browser
  over a 500-item virtualized list, took **83 steps and 2.77 s** — about 33 ms
  a step, two `requestAnimationFrame`s. That is the floor; the settle logic
  costs roughly 45 ms a step on top, and buys the guarantee that nothing is
  harvested mid-render.

Sampling is skipped when the content signature has not changed, so a static
page pays for one extraction rather than one per step: `lazy-article` did 9
steps and 11 samples, `claude-truncated` did 1 sample.

## Two bugs these measurements found

Both were performance failures that presented as completeness failures, which
is exactly why the numbers are worth printing:

1. **A 1-pixel treadmill.** Guaranteeing forward progress with
   `max(current + 1, target)` turned a target *behind* the reader into a 1 px
   step. Measured: **113 steps and a full 6-second budget to advance 113
   pixels**, on a page whose next content was 900 px further down. Stepping by
   a viewport when the mounted items are behind you fixed it — 9 steps, 792 ms.
2. **Giving up before the content.** An idle counter treated "no new content
   for 3 steps" as the end of the page, and a page with two tall empty spacers
   before its lazy sections tripped it 900 px early. Idle now only ends a
   traversal at the bottom; the step and time budgets bound the rest.

## What is not measured here

- **The real `chatgpt.com` and `claude.ai`.** These fixtures reproduce the
  *behaviours* the research pass documented, not the sites. No number here says
  anything about whether those sites still use the selectors Relay looks for.
  `docs/capture.md` §14 is the manual procedure that answers that, and it is
  the only thing that can.
- **Threads of 1,000+ turns.** Inferred from the 300-turn measurement above,
  not run. The engine's behaviour at the budget is tested
  (`traversal/engine.test.ts`), the arithmetic is not.
- **Relay's ingestion time**, and end-to-end capture-to-artifact latency. The
  Rust side has a 1,200-turn normalization test but nothing timed, and the
  loopback bridge is unbenchmarked.
- **A cold browser, a loaded machine, or a slow disk.** Single runs on one
  container. Treat these as the shape of the cost, not a budget.
- **Anything about accuracy.** These are timings. What was captured is asserted
  by the validation runner's 35 checks and by the unit suites; a fast capture
  of the wrong thing would not show up here.
