---
trigger: always_on
description: Local vault/LanceDB access vs. Supabase cloud access, and where each is allowed
globs: "native/src-tauri/**, web/src/**"
---

# Data Access Rules

Relay has two genuinely different storage layers (decisions 1, 3, 12 in
`Relay - Decision Log.md`), unlike NGConnect's single Supabase-backed
`data-access.md` — keep them clearly separated rather than reusing one
pattern for both.

## Local storage (markdown vault + LanceDB) — local-only mode, always available

- All local vault/LanceDB access goes through `native/src-tauri/src/vault/`
  — never scatter file I/O or LanceDB queries through `pipeline/`,
  `triggers/`, or command handlers directly. Those modules call a named
  `vault::` function, the same principle as NGConnect's
  "components call a named function, not the client directly."
- No authentication is required for local-only mode — single machine,
  single user (decision 1's primary-users framing). Don't add an auth check
  here; it has nothing to protect against in this mode.
- Handle vault/LanceDB errors explicitly with the `thiserror` types from
  `rust-backend.md` — surface a user-facing message in the native UI rather
  than letting a failed local read/write silently produce an empty state.

## Cloud storage (Supabase) — hybrid mode only

- Supabase is only reachable from **server-side code**: the Rust backend
  (for the native app's hybrid sync) and `web/`'s server-side code (Server
  Components, Server Actions, route handlers) — never from `native/src/`
  directly and never from a `web/` Client Component. This mirrors
  NGConnect's rule but the boundary is per-surface, not per-file-type.
- Never call `createClient` (browser or server, in `web/`) directly inside a
  component or route handler — always import the pre-configured client from
  `web/src/lib/supabase`.
- Never use the Supabase service-role key in any code that ships to a
  browser or to the Tauri frontend bundle. Service-role access is
  Rust-backend-only or `web/` server-only (see `security.md`).
- Assume Row Level Security (RLS) is the primary authorization layer once
  hybrid mode's real login (decision 12) is in place — a query without a
  matching policy should fail closed, not silently return everything.
- Keep cloud-query logic in `web/src/lib/<domain>/` (or the Rust backend's
  `providers/`/`vault/` layer for the native app's hybrid sync) — don't
  inline `.from('table').select(...)` chains deep inside component JSX.
- Handle Supabase errors explicitly — check `error` on every call and
  surface a user-facing message rather than letting a failed query silently
  render an empty state. Design specifically for the free-tier
  auto-pause-after-idle-week behavior flagged in decision 12 — a slow first
  request after inactivity is expected, not a bug to hide from the user.
