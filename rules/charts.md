---
globs: native/src/**/*.tsx, web/src/**/*.tsx
---

# Charts, Graphs, and Data Visualizations

CONTEXT: This applies specifically to CHARTS, GRAPHS, and DATA
VISUALIZATIONS in Relay (Recharts components via shadcn, see
`ui-components.md`) — not general UI colors, buttons, or badges. Relay's own
chart surface is smaller than a typical dashboard app: mainly milestone 10's
dogfooding metrics (parsing accuracy over time, trigger-phrase hit/miss
rate, meeting vs. scribble capture counts). The color **system** below is
carried over from the same framework used elsewhere; the specific hex values
are placeholders — define Relay's real tokens in `design-system.md`'s theme
CSS, then this file's decision rules apply on top of them.

## Color Tokens

Define these once (naming convention only, pick your own values):
`--color-chart-primary`, `--color-chart-accent-1`, `--color-chart-accent-2`,
`--color-chart-success`, `--color-chart-danger`, `--color-chart-warning`,
`--color-chart-neutral`, `--color-chart-grid`, `--color-chart-text`,
`--color-chart-surface`.

`accent-1` and `accent-2` should be a split-complementary pair built off the
primary — pre-harmonized to work together, so use them as a set rather than
picking a third color ad hoc.

## How Many Colors to Use

Decide by what the chart is actually showing:

### 1 Color (`--color-chart-primary` only)
Use when: a single metric, single series over time, or one hero number.
Examples: "Kanban items extracted per meeting" over time, "Captures this
week" bar chart. Everything else (axis, grid, labels) uses neutral/gray
tokens — never invent a second data color just to fill space.

### 2 Colors (`--color-chart-primary` + `--color-chart-accent-1`)
Use when: comparing exactly two series or two time periods.
Examples: "Meeting captures vs. scribble captures," "Trigger matches vs.
misses this week." Primary = the subject being emphasized; accent-1 = the
comparison point.

### 3 Colors (`--color-chart-primary` + `--color-chart-accent-1` + `--color-chart-accent-2`)
Use when: comparing three genuinely distinct, equally-weighted series.
Example: "Meeting / Scribble / Trigger-phrase" capture-type breakdown.

**Separate case — 2 meaningful series + a residual:** use primary +
accent-1 + neutral for the "long tail." Example: "Calendar trigger / Notion
push / Other" — the first two get primary/accent-1, everything long-tail
gets neutral.

Also use primary + success + danger for semantic delta charts: primary
(current value) + success (increase) + danger (decrease) — e.g. week-over-
week parsing-accuracy change.

### 4+ Colors
AVOID for standard charts. If a chart seems to need 4+ distinct data colors,
change the CHART TYPE, not add more hues — a stacked bar with one hue at
varying opacity, small multiples, or fold minor categories into neutral
"Other."

## Semantic Colors

Success/danger/warning are reserved ONLY for trend indicators and status/
alert states (e.g. "trigger-phrase false-positive rate rising"). Never use
them as general categorical colors for unrelated data series.

## Hard Rules

- **Never** cycle through all tokens like a rainbow palette.
- **Every chart** must work in both light and dark mode via the CSS
  variables — never hardcode hex values directly in a Recharts component.
- **Gridlines, axis text, and chart background** always use
  `--color-chart-grid` / `--color-chart-text` / `--color-chart-surface`,
  never primary/accent/success/danger/warning.
- **When in doubt, use fewer colors.** Given how few charts Relay's MVP
  actually needs (dogfooding metrics, mostly), reaching for a complex
  multi-series chart at all is worth a second look before building it.
