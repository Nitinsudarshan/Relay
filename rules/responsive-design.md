---
trigger: always_on
description: Mobile-first responsive layout rules — web dashboard only
globs: "web/src/**/*.tsx"
---

# Responsive Design Rules

This applies to **`web/` only**. `native/`'s Tauri window is a fixed desktop
surface — there's no mobile breakpoint concern there; instead, make sure
`native/src/` layouts handle window resize reasonably (min-width constraints,
no fixed-pixel layouts that break if the user resizes the app window), which
`design-system.md` and `component-architecture.md` already cover via
relative-unit guidance.

## Rules (web/)

- Write base (unprefixed) Tailwind classes for the mobile layout, then add
  `sm:`, `md:`, `lg:`, `xl:` modifiers to adapt for larger screens — never
  the reverse.
- Avoid fixed pixel widths (`w-[400px]`) on layout containers; use relative
  units (`w-full`, `max-w-*`, `flex-1`).
- Use container padding (`px-4 md:px-6 lg:px-8`) for page-level layout
  instead of margins on individual children.
- The Kanban view and any dense data view (dogfooding metrics, vault
  browser) must have a usable mobile fallback — either a card-based layout
  below `md:`, or horizontal scroll with a visible scroll affordance. Don't
  ship a table/board that's simply unusable on a phone, since the web
  dashboard's whole point is being reachable outside the desktop app.
- Test interactive components (dialogs, dropdowns, sheets — e.g. the
  trigger-phrase config form) at mobile width — prefer shadcn's `Sheet` over
  `Dialog` for mobile-triggered panels where appropriate.
