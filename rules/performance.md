---
trigger: always_on
description: Async/non-blocking rules for the Rust backend, plus bundle-size and rendering basics for native frontend
globs: "native/**"
---

# Performance Rules

## Rust backend (native/src-tauri/)

- STT and LLM calls must never block the Tauri main thread — run them via
  `tokio::spawn`/`spawn_blocking` (see `rust-backend.md`) so the capture UI
  stays responsive during transcription/parsing, which can take real time
  even for local Whisper inference.
- Don't hold a LanceDB or vault-file lock longer than the actual read/write
  needs — a slow meeting→Kanban parse shouldn't block an unrelated vault
  read.

## Frontend (native/src/)

- Avoid shipping unoptimized large images.
- Dynamically import heavy client-only libraries (lazy `import()` in `native/`)
  where they're not needed on first paint — especially chart components (see `charts.md`).
- Don't put chart-heavy views (dogfooding metrics) on a page that doesn't
  need them on initial load — lazy-load them behind a tab or modal.
- Memoize expensive derived data (`useMemo`) when transforming larger
  datasets (a long meeting transcript, a large Kanban board) — don't
  recompute on every render.

