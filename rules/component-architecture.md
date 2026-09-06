---
trigger: always_on
description: How components and pages should be split and organized
globs: "native/src/**"
---

# Component Architecture

Applies to `native/src/`. Page/screen components must be modular.

## Rules

- Each major section of a screen must be its own component (e.g. the Kanban
  view should compose `<KanbanColumn />`, `<KanbanCard />`,
  `<CaptureStatusBar />` — not one large JSX tree).
- Avoid large monolithic page/screen files. If a page/view component
  exceeds ~150 lines of JSX (excluding imports/types), extract sections into
  components.
- Break complex UI into reusable components before duplicating markup a
  second time (rule of two: if you're about to copy-paste a block, extract
  it instead).
- Anything used across multiple views belongs in `native/src/components/shared`
  or `native/src/components/ui`.
- Keep data-fetching and presentation separate where practical: a component
  that fetches from the Rust backend (via Tauri's `invoke`) should hand plain
  props down to a "dumb" presentational component, rather than mixing invoke
  logic through deeply nested JSX.

