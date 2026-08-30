---
trigger: always_on
description: shadcn/ui and styling conventions — mandatory for all UI, including charts
globs: "native/src/**/*.tsx, web/src/**/*.tsx"
---

# UI Component Rules

**Always use shadcn/ui for components**, in both `native/src/` and `web/src/`.
This applies to everything — buttons, inputs, forms, dialogs, sheets,
tables, cards, dropdowns, tabs, tooltips, badges, navigation, AND
charts/graphs. Do not hand-build a custom component if a shadcn/ui
equivalent exists.

## Rules

- Use shadcn primitives before creating anything custom. Check
  `<surface>/components/ui` first; if the primitive doesn't exist yet,
  generate it via the shadcn CLI (`npx shadcn add <component>`) in that
  surface — do not hand-roll a replacement.
- **Charts and graphs must use shadcn's chart component**
  (`npx shadcn add chart`, wrapping Recharts) — not raw `recharts` used
  directly, and not any other charting library. Relay's own chart use is
  smaller than NGConnect's (mainly dogfooding/parsing-accuracy views, see
  `charts.md`), but the rule is the same: reach it only through
  `ChartContainer`/`ChartTooltip`.
- Do not write raw CSS (`.module.css`, inline `style={{}}`) or Tailwind-only
  custom components for things shadcn already provides.
- Components must support both light mode and dark mode — use theme-aware
  classes (`bg-background`, `text-foreground`, `border-border`) rather than
  explicit `bg-white`/`bg-black` or ad hoc `dark:` overrides.
- Use shadcn blocks as the starting point for common layouts (cards, forms,
  tables, dashboards) rather than building the same layouts from scratch.
- When a shadcn primitive needs project-specific behavior (e.g. a
  `KanbanColumn` wrapper with drag state, once that's built post-MVP), wrap
  it in `<surface>/components/shared` rather than editing the generated
  file in `<surface>/components/ui` directly.
- The only acceptable exception is when genuinely no shadcn equivalent
  exists (e.g. the native capture widget's always-on-top overlay chrome) —
  and even then it must still use design tokens from `design-system.md`,
  not raw hardcoded styling.

## No fake controls

A control must never present functionality that isn't actually wired up.
This was originally the push-to-talk pill's rule (Decision 25/PTT-008) and
applies to every surface: a toggle that looks live but changes nothing is
worse than no toggle, because the user cannot tell the difference between
"off" and "broken".

When the backing capability isn't available, pick one and make it visible:

- **Warn** — `⚠ Ollama not configured` alongside the control.
- **Disable** — render it disabled with the reason (`Unavailable — configure an LLM`).
- **Prompt setup** — replace it with a `[Configure]` call to action.

The check applies at review time too: if a setting exists in the settings
schema and in the UI, something in the code must read it. A persisted
setting no code path consumes is a fake control that happens to survive a
restart.

## Known drift

- `PillSettingsPopover.tsx` and `MarkdownView.tsx` still carry most of the
  repo's remaining hardcoded hex colors and ad hoc `dark:` overrides. New
  work in those files should use design tokens (`design-system.md`) rather
  than extending the existing exceptions.
