---
trigger: always_on
description: Consistent response shape for Tauri commands and web route handlers
globs: "native/src-tauri/src/commands.rs, web/src/app/api/**"
---

# API Conventions

Relay has two different "API" surfaces — Tauri commands (native ↔ Rust
backend) and, if `web/` needs any of its own route handlers against
Supabase, standard Next.js route handlers. Both follow the same underlying
principle NGConnect used for its route handlers: one consistent response
shape per surface, validate before touching storage, never leak raw internal
errors to the caller.

## Tauri commands (native/src-tauri/src/commands.rs)

- Every `#[tauri::command]` returns a consistent shape — e.g.
  `Result<T, CommandError>` where `CommandError` serializes to
  `{ message: String, code: String }` — don't mix ad hoc `String` errors and
  typed errors across different commands.
- Validate input at the top of the command before calling into
  `pipeline/`/`triggers/`/`vault/` — reject invalid input with a clear
  `CommandError` rather than letting a malformed call reach business logic.
- Map internal `thiserror` error types (see `rust-backend.md`) to
  `CommandError` at the command boundary — never forward a raw internal
  error type or a LanceDB/filesystem error message verbatim to the frontend.
- Auth/authorization has nothing to check in local-only mode (decision 1) —
  don't add a no-op check for its own sake. Hybrid-mode commands that touch
  Supabase-backed data check auth first, before any query, same principle
  as NGConnect's "fail fast with 401/403."

## web/ route handlers (if any are needed beyond Server Actions)

- Use route handlers for anything called from a Client Component via
  `fetch`, or integrations with external systems. Use Server Actions for
  simple form submissions that don't need a stable external endpoint.
- Every route handler returns a consistent JSON shape:
  `{ success: boolean, data?: T, error?: string }`.
- Validate input at the top of the handler before touching Supabase.
- Wrap Supabase calls in try/catch; return a generic error to the client and
  log the real error server-side — never forward a raw Supabase/Postgres
  error message to the client (see `security.md`).
- Auth/authorization check happens first, before any query.
