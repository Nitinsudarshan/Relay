---
trigger: always_on
description: Async/non-blocking rules for the Rust backend, plus bundle-size and rendering basics for both frontends
globs: "native/**, web/src/**/*.tsx"
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

## Frontend (native/src/ and web/src/)

- Always use `next/image` for images in `web/` (not a raw `<img>` tag) —
  `native/`'s Vite-based frontend doesn't have this specific optimization,
  but should still avoid shipping unoptimized large images.
- Dynamically import heavy client-only libraries (`next/dynamic` in `web/`,
  lazy `import()` in `native/`) where they're not needed on first paint —
  especially chart components (see `charts.md`).
- Don't put chart-heavy views (dogfooding metrics) on a page that doesn't
  need them on initial load — lazy-load them behind a tab or route split.
- Memoize expensive derived data (`useMemo`) when transforming larger
  datasets (a long meeting transcript, a large Kanban board) in a Client
  Component — don't recompute on every render.
