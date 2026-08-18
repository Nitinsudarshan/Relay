---
trigger: always_on
description: TypeScript and React coding conventions, shared by both React surfaces
globs: "native/src/**/*.ts, native/src/**/*.tsx, web/src/**/*.ts, web/src/**/*.tsx"
---

# Code Standards (Frontend)

Applies equally to `native/src/` (the Tauri-embedded frontend) and `web/src/`
(the Next.js hybrid dashboard) — adapted from NGConnect's `code-standards.md`,
generalized since Relay has two React surfaces instead of one.

## Rules

- Use TypeScript strictly. No implicit `any`. If a type genuinely can't be
  known, use `unknown` and narrow it — never silently widen to `any`.
- Use functional React components only. No class components.
- Use arrow functions for all components: `const KanbanBoard = () => { ... }`,
  not `function KanbanBoard() { ... }`.
- Follow Prettier formatting (run `npm run lint` in the relevant surface
  before considering a change done).
- Use named exports for components, hooks, and utilities. Use a default
  export only where the framework requires it (`web/`'s `page.tsx`/
  `layout.tsx` in the App Router; `native/`'s Vite entry point).

## Naming conventions

- Components: `PascalCase` (`KanbanCard.tsx`, `TriggerPhraseForm.tsx`), one
  component per file, filename matches the component name.
- Hooks: `camelCase` prefixed with `use` (`useTriggerPhrases.ts`,
  `useCaptureStatus.ts`), live in `<surface>/hooks`.
- Utilities/helpers: `camelCase` (`formatTranscript.ts`), live in
  `<surface>/lib`.
- Types/interfaces: `PascalCase`, no `I` prefix (`KanbanItem`, not
  `IKanbanItem`).
- Booleans: prefix with `is`/`has`/`should` (`isRecording`, `hasCloudSync`).

## Imports

- Use the `@/` path alias (as configured in each surface's `tsconfig.json`)
  for all cross-folder imports within that surface — never deep relative
  paths like `../../../lib/vault/client`.
- Group imports in this order, separated by a blank line: external packages
  → `@/` internal imports → relative imports → types.
- `native/src/` and `web/src/` are separate TypeScript projects — don't
  import across them directly. Shared code goes in `packages/shared/` once
  it's genuinely needed twice (see `project-structure.md`).
