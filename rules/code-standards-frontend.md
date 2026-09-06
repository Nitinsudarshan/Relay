---
trigger: always_on
description: TypeScript and React coding conventions for native frontend
globs: "native/src/**/*.ts, native/src/**/*.tsx"
---

# Code Standards (Frontend)

Applies to `native/src/` (the Tauri-embedded frontend).

## Rules

- Use TypeScript strictly. No implicit `any`. If a type genuinely can't be
  known, use `unknown` and narrow it — never silently widen to `any`.
- Use functional React components only. No class components.
- Use arrow functions for all components: `const KanbanBoard = () => { ... }`,
  not `function KanbanBoard() { ... }`.
- Follow formatting standards (run `npm run lint` / `npm test` in `native/`
  before considering a change done).
- Use named exports for components, hooks, and utilities. Use a default
  export only where the framework requires it (`native/src/App.tsx`).

## Naming conventions

- Components: `PascalCase` (`KanbanCard.tsx`, `TriggerPhraseForm.tsx`), one
  component per file, filename matches the component name.
- Hooks: `camelCase` prefixed with `use` (`useTriggerPhrases.ts`,
  `useCaptureStatus.ts`), live in `native/src/hooks`.
- Utilities/helpers: `camelCase` (`formatTranscript.ts`), live in
  `native/src/lib`.
- Types/interfaces: `PascalCase`, no `I` prefix (`KanbanItem`, not
  `IKanbanItem`).
- Booleans: prefix with `is`/`has`/`should` (`isRecording`, `hasCloudSync`).

## Imports

- Use the `@/` path alias (as configured in `native/tsconfig.json`)
  for all cross-folder imports within `native/src/` — never deep relative
  paths like `../../../lib/vault/client`.
- Group imports in this order, separated by a blank line: external packages
  → `@/` internal imports → relative imports → types.

