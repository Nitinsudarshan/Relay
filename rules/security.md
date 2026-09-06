---
trigger: always_on
description: Secrets, environment variables, and hybrid-mode auth handling
---

# Security Rules

## Rules

- Never hardcode API keys, Supabase URLs/keys, Ollama/cloud-LLM API keys, or
  any credential in source code — read from environment variables or secure storage.
  In `native/src-tauri/`, load secrets via `dotenvy` or user settings at runtime,
  never compiled into the binary as a literal.
- Never commit `.env`/`.env.local` — confirm they stay covered by
  `.gitignore` before adding any new env-dependent feature.
- **Local-only mode needs no auth at all** — single machine, single user
  (decision 1). Don't build a login screen or session logic that only ever
  runs in this mode.
- Any future cloud auth checks belong strictly in the Rust backend —
  a client-side redirect or conditional render in React is a UX nicety, not
  a security boundary.
- Never expose the Supabase service-role key to `native/src/` or any
  browser-reachable bundle.
- When writing a script or command that touches cloud-stored user data,
  require an explicit confirmation step before it mutates data.
- Log errors without leaking sensitive payloads (tokens, transcript
  content that could be sensitive) into client-visible console output in
  production code paths.

