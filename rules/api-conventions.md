---
trigger: always_on
description: Consistent response shape for Tauri commands
globs: "native/src-tauri/src/commands.rs, native/src-tauri/src/**/commands.rs"
---

# API Conventions

Relay's internal API surface consists of Tauri commands (native React frontend ↔
Rust backend). Commands follow a consistent response shape: validate before
touching storage, and never leak raw internal errors to the caller.

## Tauri commands (native/src-tauri/src/commands.rs)

- Every `#[tauri::command]` returns a consistent shape — e.g.
  `Result<T, CommandError>` or `Result<T, String>` — don't mix unexpected
  shapes across related commands.
- Validate input at the top of the command before calling into
  domain modules — reject invalid input with a clear error rather than
  letting a malformed call reach business logic.
- Map internal `thiserror` error types (see `rust-backend.md`) to
  user-facing errors at the command boundary — never forward raw internal
  error details or a LanceDB/filesystem error message verbatim to the frontend.
- Auth/authorization has nothing to check in local-only mode (decision 1) —
  don't add a no-op check for its own sake. Native commands that touch
  cloud-backed telemetry or updates check configuration first.

