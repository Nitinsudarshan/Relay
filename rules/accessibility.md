---
trigger: always_on
description: Accessibility requirements for all UI
globs: "native/src/**/*.tsx, web/src/**/*.tsx"
---

# Accessibility Rules

Applies to both `native/src/` and `web/src/`.

## Rules

- All interactive elements must have accessible labels — a visible label, or
  `aria-label`/`aria-labelledby` when there's no visible text (e.g. the
  push-to-talk icon button, capture-widget controls).
- Use semantic HTML wherever possible (`<button>` not `<div onClick>`,
  `<nav>`, `<main>`, `<table>` for tabular data) rather than reaching for
  ARIA roles first.
- Forms (notably the trigger-phrase config form) must include validation
  messages that are programmatically associated with their field
  (`aria-describedby`), not just visually placed nearby.
- Ensure keyboard navigation works correctly: all interactive elements
  reachable via Tab, visible focus states preserved, and modals/sheets trap
  focus while open. This matters especially for the native capture widget,
  which may be operated hands-mostly-busy (mid-meeting).
- Maintain sufficient color contrast for text and icons against their
  background (aim for WCAG AA — 4.5:1 for normal text, 3:1 for large
  text/icons) — check this especially for muted/secondary text tokens.
- Images must have meaningful `alt` text, or `alt=""` if purely decorative.
