---
trigger: always_on
description: Secrets, environment variables, and hybrid-mode auth handling
---

# Security Rules

## Rules

- Never hardcode API keys, Supabase URLs/keys, Ollama/cloud-LLM API keys, or
  any credential in source code — read from environment variables. In
  `web/`, use `process.env.NEXT_PUBLIC_*` only for values genuinely needed
  client-side, unprefixed `process.env.*` for server-only values. In
  `native/src-tauri/`, load secrets via a crate like `dotenvy` at startup,
  never compiled into the binary as a literal.
- Never commit `.env`/`.env.local` — confirm they stay covered by
  `.gitignore` before adding any new env-dependent feature.
- Anything prefixed `NEXT_PUBLIC_` is exposed to the browser — never put a
  secret (service-role key, private LLM API token) behind that prefix.
- **Local-only mode needs no auth at all** — single machine, single user
  (decision 1). Don't build a login screen or session logic that only ever
  runs in this mode.
- **Hybrid mode requires real login** — password/token-based auth against
  the cloud backend (decision 12), not a LAN-only or tunnel-based access
  model; that framing was explicitly considered and rejected during
  planning. Auth checks for hybrid-mode data belong server-side (the Rust
  backend, or `web/`'s server-side code) — a client-side redirect or
  conditional render is a UX nicety, not a security boundary.
- Never expose the Supabase service-role key to `native/src/`, `web/`
  Client Components, or any browser-reachable bundle.
- When writing a script or command that touches cloud-stored user data,
  require an explicit confirmation step before it mutates data.
- Log errors without leaking sensitive payloads (tokens, transcript
  content that could be sensitive) into client-visible console output in
  production code paths.
