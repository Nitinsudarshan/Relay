---
trigger: always_on
description: Folder structure — where new files should be created
---

# Project Folder Structure

Relay is a from-scratch, three-surface repo (decisions 1–3 in
`Relay - Decision Log.md`) — there is no existing NGConnect-style single
`src/` tree to follow. This is the target layout; create it as you scaffold
rather than expecting it to already exist.

```
relay/
  native/                    Tauri desktop app (Windows, local mode)
    src-tauri/                Rust backend
      src/
        capture/               Push-to-talk, local Whisper/Parakeet STT wiring
        pipeline/               Meeting->Kanban parser, scribble->structured-output
        triggers/               Configurable trigger-phrase system (decision 10)
        providers/              Ollama / hybrid cloud-LLM toggle
        vault/                  Local markdown vault + LanceDB access
        mcp/                    Calendar/Notion/Drive MCP client wiring
        commands.rs             #[tauri::command] entry points (thin, see rust-backend.md)
    src/                       React frontend rendered inside the Tauri window
      components/
      hooks/
      lib/
  web/                        Next.js + Shadcn hybrid-mode dashboard
    src/
      app/                     App Router routes
      components/
        ui/                     shadcn/ui primitives (generated, low-touch)
        shared/                 Reusable cross-route components
      contexts/
      hooks/
      lib/
        supabase/               Supabase client setup (hybrid mode only)
      types/
  docs/                      Living spec — product.md, decisions.md, etc.
                               (seeded from Relay - IDE Build Prompt.md section 6)
```

## Rules

- Code that's genuinely shared between `native/src/` and `web/src/` (a type,
  a small pure utility) goes in a top-level `packages/shared/` once it's
  needed twice — don't create it speculatively before a second real user
  exists.
- Reusable UI primitives (buttons, inputs, dialogs) go in `<surface>/components/ui`
  — prefer generating via the shadcn CLI over hand-writing them, in both
  `native/src/` and `web/src/`.
- Cross-route, non-primitive components go in `<surface>/components/shared`.
- Route-specific `web/` components used by only one page can be co-located
  next to that route (`web/src/app/(dashboard)/kanban/_components/`) rather
  than forced into `shared`.
- Rust modules under `src-tauri/src/` are organized by domain
  (`capture/`, `pipeline/`, `triggers/`, `providers/`, `vault/`, `mcp/`), not
  by technical layer — a feature's parsing, validation, and persistence logic
  live together in its own module, not scattered across generic `services/`
  or `utils/` folders.
- `commands.rs` (or an equivalent thin command layer) is the **only** place
  `#[tauri::command]` functions live — they call into `pipeline/`/`triggers/`
  modules, they don't contain business logic themselves (see
  `rust-backend.md`).
- All Supabase client creation in `web/` goes through `web/src/lib/supabase`
  — never call `createClient` directly from a component or route handler
  (see `data-access.md`). `native/` never talks to Supabase directly — only
  the Rust backend does, in hybrid mode (see `security.md`).
- Shared TypeScript types go in `<surface>/types` (or `packages/shared` once
  it exists); don't redefine the same shape in both frontends.
- `docs/` is the living spec required by `Relay - IDE Build Prompt.md`
  section 6 — keep it current, it's not optional documentation.
