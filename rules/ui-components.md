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

This is a from-scratch repo, so there's no NGConnect-style "known drift to
clean up" section here yet — if a future change introduces a raw `recharts`
import bypassing the shadcn wrapper, add that drift note here rather than
letting it go undocumented.
