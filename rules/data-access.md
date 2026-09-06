---
trigger: always_on
description: Local vault/LanceDB access vs. Supabase cloud access, and where each is allowed
globs: "native/src-tauri/**, native/src/**"
---

# Data Access Rules

Relay has two storage layers (local vault/LanceDB and native cloud sync/diagnostics).
Keep them clearly separated rather than mixing patterns.

## Local storage (markdown vault + LanceDB) — local-only mode, always available

- All local vault/LanceDB access goes through `native/src-tauri/src/vault/`
  — never scatter file I/O or LanceDB queries through `pipeline/`,
  `triggers/`, or command handlers directly. Those modules call a named
  `vault::` function.
- No authentication is required for local-only mode — single machine,
  single user. Don't add an auth check here; it has nothing to protect against
  in this mode.
- Handle vault/LanceDB errors explicitly with the `thiserror` types from
  `rust-backend.md` — surface a user-facing message in the native UI rather
  than letting a failed local read/write silently produce an empty state.

## Cloud storage (Supabase) — native desktop backend only

- Supabase is only reachable from the **Rust backend** (`native/src-tauri`)
  (for telemetry, heartbeat, identity, and updates) — never from `native/src/`
  directly.
- Never use the Supabase service-role key in any code that ships to the Tauri
  frontend bundle.
- Assume Row Level Security (RLS) is the primary authorization layer for cloud
  data — a query without a matching policy should fail closed, not silently return
  everything.
- Keep cloud-query logic in the Rust backend's designated service modules
  (`identity/supabase.rs`, `updates/`) — never inline queries across the codebase.
- Handle Supabase errors explicitly — log errors and surface clean diagnostics
  rather than letting a failed network call cause desktop hangs or crashes.

