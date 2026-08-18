---
trigger: always_on
description: Design tokens — color, spacing, radius, typography
globs: "native/src/**/*.tsx, web/src/**/*.tsx, **/*.css"
---

# Design System Rules

Relay hasn't picked a brand palette yet — unlike NGConnect's `design-system.md`
(which named specific hex values), this file defines the **system**, not the
values. The first time this file is actually touched while building, define
the tokens below in `native/src/`'s and `web/`'s theme CSS, then follow them
consistently across both surfaces — don't let them drift apart into two
different palettes for one product.

## Rules

- Use design tokens instead of hardcoded colors. Never write a raw hex value
  in a component — use the corresponding Tailwind/theme class (`bg-primary`,
  `text-muted-foreground`, etc.), defined once and shared in spirit (not
  necessarily in a single shared CSS file, since `native/` and `web/` are
  separate builds) between both surfaces.
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
  `ui-components.md`) — this matters more for `native/`'s always-on capture
  widget than for most web dashboards, since it may sit visible over other
  apps for long stretches.
