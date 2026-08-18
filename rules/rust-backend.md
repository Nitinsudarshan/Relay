---
trigger: always_on
description: Rust conventions for the native/src-tauri backend — error handling, module layout, async, Tauri command exposure
globs: "native/src-tauri/**/*.rs"
---

# Rust Backend Rules

This file has no NGConnect equivalent — decision 2 in `Relay - Decision Log.md`
chose a from-scratch Rust backend, which the original agent/rule set never
covered. Treat this as the primary reference for `native/src-tauri/`.

## Rules

- Use `Result<T, E>` for anything fallible. No `unwrap()`/`expect()` outside
  tests, `main()` startup, or a genuinely-impossible-to-fail invariant you've
  commented as such — a panicked STT/LLM call should never take down the
  whole desktop app.
- Define domain error types with `thiserror` per module (`CaptureError`,
  `PipelineError`, `TriggerError`, ...) rather than passing raw `String`
  errors around — this is what lets `api-conventions.md`'s consistent
  command-response shape actually work.
- Use `tokio` for anything async (STT calls, LLM provider calls, MCP
  requests) — never block the async runtime with a synchronous long-running
  call; spawn it (`tokio::spawn` / `spawn_blocking` for CPU-bound work like
  local Whisper inference) instead. See `performance.md` for why this
  matters specifically for the Tauri main thread.
- `#[tauri::command]` functions in `commands.rs` are thin: they deserialize
  input, call into the relevant `pipeline/`/`triggers/`/`vault/` module
  function, and map its `Result` into the shared command-response shape (see
  `api-conventions.md`). No parsing, no business logic, no direct
  LanceDB/vault I/O inline in a command function.
- Use `tracing` for logging (with spans around capture/pipeline/trigger
  operations), not `println!`/`eprintln!` — this is what makes the two
  flagged technical risks (Kanban parsing quality, trigger-phrase false
  positives) debuggable from real usage logs during dogfooding (milestone
  10).
- Run `cargo clippy` and `cargo fmt` before considering a change done — same
  bar as `npm run lint` for the frontend surfaces.
- Module boundaries follow `project-structure.md`'s domain split
  (`capture/`, `pipeline/`, `triggers/`, `providers/`, `vault/`, `mcp/`) —
  don't introduce a generic `utils.rs` or `helpers.rs` as a catch-all; if
  something doesn't obviously belong to one domain module, that's a sign the
  module boundaries need revisiting, not a reason to add a junk-drawer file.
- The `providers/` module is where decision 2's hybrid Ollama/cloud-LLM
  toggle and decision 9 (no Rust-vs-Python spike, straight to Rust) actually
  live — keep the provider interface swappable (one trait, multiple
  implementations) rather than hardcoding Ollama calls throughout
  `pipeline/`.
