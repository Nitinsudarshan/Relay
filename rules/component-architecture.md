---
trigger: always_on
description: How components and pages should be split and organized
globs: "native/src/**, web/src/**"
---

# Component Architecture

Applies to both `native/src/` and `web/src/`. Page/screen components must be
modular.

## Rules

- Each major section of a screen must be its own component (e.g. the Kanban
  view should compose `<KanbanColumn />`, `<KanbanCard />`,
  `<CaptureStatusBar />` — not one large JSX tree).
- Avoid large monolithic page/screen files. If a page/route component
  exceeds ~150 lines of JSX (excluding imports/types), extract sections into
  components.
- Break complex UI into reusable components before duplicating markup a
  second time (rule of two: if you're about to copy-paste a block, extract
  it instead).
- Co-locate a route-specific `web/` component with its route
  (`web/src/app/(dashboard)/kanban/_components/`) only if it's not reused
  elsewhere. Anything used by 2+ routes belongs in `<surface>/components/shared`
  or `<surface>/components/ui`.
- Keep data-fetching and presentation separate where practical: a component
  that fetches from the Rust backend (via Tauri's `invoke`) or from Supabase
  (`web/`, hybrid mode) should hand plain props down to a "dumb"
  presentational component, rather than mixing fetch logic through deeply
  nested JSX.
