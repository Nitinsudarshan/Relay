---
trigger: always_on
description: Design tokens — color, spacing, radius, typography
globs: "native/src/**/*.tsx, native/src/**/*.css"
---

# Design System Rules

This file defines the design token system for Relay. Follow these tokens
consistently across `native/src/`.

## Rules

- Use design tokens instead of hardcoded colors. Never write a raw hex value
  in a component — use the corresponding Tailwind/theme class (`bg-primary`,
  `text-muted-foreground`, etc.).

- Use theme variables for anything that has one: color, spacing, radius,
  shadow.
- Use Tailwind's spacing scale (`p-4`, `gap-2`, ...) — no arbitrary pixel
  values like `p-[13px]` unless matching a fixed external constraint.
- Radius: use CSS radius variables (`--radius`, `rounded-sm/md/lg/xl` mapped
  from them). Default to `rounded-lg` for cards/panels/modals (Kanban
  columns, the capture widget), `rounded-md` for buttons, inputs, and
  badges — match shadcn's own defaults for a given primitive unless a real
  design reason calls for an override.
- Follow the typography scale defined in the theme (`text-sm`, `text-base`,
  `text-lg`, etc.) — no arbitrary `text-[15px]` sizing.
- Support both light and dark mode for every new token usage (see
  `ui-components.md`) — this matters especially for `native/`'s always-on capture
  widget, since it may sit visible over other apps for long stretches.

