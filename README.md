# Relay

Relay is a hybrid (local + cloud) AI voice and memory assistant. It turns
push-to-talk speech into structured, actionable system state — Kanban task
cards, calendar events, reminders, and polished Markdown notes — instead of
a wall of transcript text you have to re-read and re-type.

It also works as a universal dictation and voice-chat tool: a global hotkey
transcribes speech and types it directly into whatever app or field
currently has focus, and an in-app voice chat answers questions grounded in
your own vault notes.

See `docs/product.md` for the full product spec, `docs/decisions.md` for the
architectural decision log, and `AGENTS.md` for repository conventions.

## Structure

- `native/` — Windows desktop app: Rust backend (`native/src-tauri/`) +
  Tauri/React frontend (`native/src/`). This is the primary surface.
- `web/` — Next.js + Shadcn + Supabase dashboard for hybrid (cloud-synced)
  mode.
- `docs/` — Living product/architecture specification.

## Getting Started (native desktop app)

```bash
cd native
npm install
npm run tauri dev
```

## Getting Started (web dashboard)

```bash
cd web
npm install
npm run dev
```

Open [http://localhost:3000](http://localhost:3000) with your browser to see
the result.

## License

MIT
