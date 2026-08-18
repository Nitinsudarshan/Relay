# Relay — Changelog

## [0.1.1] - 2026-08-19

### Improvements & UI Refactoring

- **Native Frontend (`native/`)**:
  - Installed `@radix-ui/react-slot`, `@radix-ui/react-tabs`, `@radix-ui/react-dialog`, `@radix-ui/react-badge`, `clsx`, `tailwind-merge`, and `class-variance-authority`.
  - Created [`native/src/lib/utils.ts`](file:///d:/Projects/Relay/native/src/lib/utils.ts) `cn` helper function.
  - Added shadcn UI primitives: [`Button`](file:///d:/Projects/Relay/native/src/components/ui/button.tsx), [`Card`](file:///d:/Projects/Relay/native/src/components/ui/card.tsx), [`Badge`](file:///d:/Projects/Relay/native/src/components/ui/badge.tsx), [`Input`](file:///d:/Projects/Relay/native/src/components/ui/input.tsx).
  - Refactored `PTTWidget`, `KanbanBoard`, `ScribbleViewer`, `TriggerSettings`, and `ProviderSettings` to strictly use shadcn UI components and design tokens per `rules/ui-components.md`.

## [0.1.0] - 2026-08-19

### Initial Release — Multi-Surface Architecture & Core Pipeline

- **Living Specification (`docs/`)**:
  - `product.md`: Product vision, target user, core problems, scope boundaries.
  - `decisions.md`: Seeded all 12 pre-confirmed architectural decisions.
  - `requirements.md`: Detailed functional and non-functional requirements.
  - `user-flows.md`: Defined end-to-end PTT capture, meeting parsing, scribble, and trigger flows.
  - `architecture.md`: System design, Rust domain module boundaries, and IPC bridge.
  - `data-model.md`: Schemas for Vault Notes, Kanban Cards, Triggers, Provider Settings, and LanceDB.
  - `api.md`: Tauri IPC `CommandResponse<T>` and web API route handler specifications.
  - `testing.md`: Testing strategy across Rust backend and React/Next.js frontends.

- **Native Desktop App (`native/`)**:
  - `src-tauri`: Rust backend domain modules (`providers`, `capture`, `vault`, `pipeline`, `triggers`, `mcp`, `commands.rs`).
  - `src`: React + TypeScript desktop UI (`PTTWidget`, `KanbanBoard`, `ScribbleViewer`, `TriggerSettings`, `ProviderSettings`, `App`).
  - Generated application icon assets for Windows resource compilation.

- **Web Dashboard Surface (`web/`)**:
  - Restructured Next.js boilerplate into `web/` per `project-structure.md`.
  - Configured Turbopack root & Supabase client interface for hybrid mode.
  - `(dashboard)/page.tsx`: Hybrid dashboard showing synced Kanban task board.
