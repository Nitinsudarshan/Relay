---
trigger: always_on
description: When to use Server Components vs Client Components in the App Router — web/ only
globs: "web/src/app/**, web/src/components/**"
---

# Server / Client Component Boundary

This applies to **`web/` only**. `native/src/` is a client-rendered SPA
talking to the local Rust backend via Tauri's `invoke` — there is no
Server/Client Component distinction there at all; skip this file entirely
when working in `native/`.

## Rules (web/)

- Default to Server Components. Do not add `"use client"` unless the
  component genuinely needs one of: React state/effects, browser APIs
  (`window`, `localStorage`), event handlers, or a client-only library
  (charts, real-time subscription hooks).
- Fetch data on the server wherever possible (Server Component, Server
  Action, or route handler) and pass the result down as props — don't fetch
  in a `useEffect` inside a Client Component if the same data could be
  fetched server-side.
- Push `"use client"` as far down the tree as possible. If only a chart or a
  dropdown needs interactivity, isolate that piece as its own small Client
  Component.
- Route handlers (`web/src/app/api/**/route.ts`) and Server Actions are the
  only places allowed to use the Supabase server client or service-role
  access (see `data-access.md`, `security.md`) — never expose that path
  through a Client Component.
- When in doubt, prefer Server Component first and only convert if a
  build/runtime error or missing interactivity proves it's needed.
