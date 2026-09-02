# Relay — Testing Strategy

What is actually tested today, and how to run it. `rules/testing.md` holds the
conventions (frameworks, placement, what not to test); this file holds the
state of play. CI runs all of it on every push and pull request —
`.github/workflows/ci.yml`.

## 1. Rust backend (`native/src-tauri/`)

825 tests (+4 ignored benchmarks), `cargo test`.

```bash
cd native/src-tauri
cargo test
cargo clippy --all-targets -- -D warnings
```

Building needs a C/C++ toolchain and CMake for whisper.cpp. On Linux it also
needs the GTK/WebKit and ALSA development headers — the CI workflow's `system
dependencies` step is the authoritative package list.

The compiler version is pinned in `native/src-tauri/rust-toolchain.toml` and
installed by rustup on the first `cargo` call. That pin is what makes a local
`clippy -D warnings` mean the same thing as CI's: lints are added between Rust
releases, so a floating `stable` turns "passes locally" into a coin flip.

Where the coverage sits:

- `meetings_v2/processing/` — the largest concentration by far: normalization,
  speaker attribution, extraction, qualification, validation, summarization,
  and the context windower.
- `meetings_v2/processing/eval.rs` — a deterministic, model-free scorer over a
  fixture set, with hallucination as a hard fail. This is the quality gate for
  summary output; it does not call a model and does not need one.
- `settings/` — schema defaults, serde aliases, and backward compatibility with
  settings files written by earlier versions.
- `vault/` — frontmatter parsing, note and scribble CRUD, merge behaviour.
- `capture/evaluation.rs` — the STT benchmark matrix and VAD behaviour.
- `capture/web/` — the web capture path, and the second-largest concentration
  after meetings: URL-based source detection, sanitization (control and
  bidirectional characters, non-`http(s)` link targets, `mermaid` fence
  downgrade, fence escaping), coverage downgrade when caps are hit, refusal of
  an empty capture, the loopback bridge's token comparison / origin refusal /
  size limits over a real socket, and an end-to-end set that drives a payload
  into the vault and checks re-capture versioning, Files isolation, promotion
  provenance, a Trash round-trip and path-traversal refusal.
- `sync.rs` — mutex poison recovery, including a test that pins the failure
  mode it exists to prevent.

Providers are exercised through a fake LLM (`processing/llm.rs`'s test module),
never a live Ollama instance or a cloud API.

## 2. Native frontend (`native/src/`)

221 tests, Vitest + React Testing Library, jsdom.

```bash
cd native
npm ci
npm test              # vitest run
npm run test:watch
npm run test:coverage
npm run typecheck     # tsc --noEmit
```

Tauri modules (`@tauri-apps/api/core`, `/event`, `/window`) are stubbed
globally in `src/test/setup.ts`, because they only resolve inside a Tauri
webview. A test overrides one with
`vi.mocked(invoke).mockImplementation(...)`. `src/test/factories.ts` builds
complete, typed domain objects so a test states only what it is about.

Where the coverage sits:

- `meetings_v2/meetingProcessing.ts` — speaker and owner label resolution,
  title preference, timestamp formatting, processing status.
- `meetings_v2/meetingsViewState.ts` — active-state classification, word
  counting across the durable and live streams, selected-session resolution.
- `meetings_v2/MeetingsV2View.tsx` — behaviour tests driven entirely through
  `invoke` responses: what the list shows, adopting a recording already in
  progress, and surviving a failed load.
- `scribble/graph/graphPhysics.ts` — layout invariants: pinned nodes never
  drift, alpha always decays to zero, coincident nodes separate rather than
  producing `NaN`.
- `lib/soundEffects.ts` — the only code path behind the `dictation_sounds`
  setting, over a stubbed Web Audio API.
- `webcapture/` — the browser extension's extraction layer, tested against
  fixture documents in jsdom: block extraction (headings, lists, code, tables,
  quotes, images), hidden content, malformed markup, deep nesting, duplicate
  text, unicode, link resolution, `<head>` metadata, every coverage verdict,
  and each site extractor's role attribution, turn ordering, fallback
  strategies and refusal to guess.
- `components/captures/` — completeness wording (a capture may only claim the
  whole page when coverage says so), filtering, the bridge-off warning, and
  deletion behind a confirmation.

### The cross-language contract

`webcapture/contract.test.ts` generates the payload fixtures under
`native/src-tauri/src/capture/web/fixtures/` from committed HTML, and
`capture::web::contract_tests` consumes the same bytes. A field renamed on
either side of the extension↔backend boundary fails one of the two suites
immediately rather than failing quietly on a user's next capture. Regenerate
with:

```bash
cd native && RELAY_UPDATE_CAPTURE_FIXTURES=1 npm test
```

## 3. Web dashboard (`web/`)

No test suite yet. The dashboard is deferred (`docs/decisions.md`, Decision 32)
and has no logic of its own to test — it renders a placeholder identity and
static routes. CI typechecks and builds it:

```bash
cd web
npm ci
npx tsc --noEmit
npm run build
```

When hybrid mode starts, `rules/testing.md` already sets the rules: Vitest +
React Testing Library, a mocked Supabase client, never a real project database.

## Known gaps

- `cargo fmt --check` is not a CI gate. The crate predates any formatting pass
  and currently differs from rustfmt in 45 files; running `cargo fmt` once, as
  its own commit, is what unblocks adding it.
- Several large native components have no tests: `ProviderSettings.tsx` (1,874
  lines), `ScribbleDetailEditor.tsx`, `DictationPill.tsx`. The pill is the
  highest-value of these, since it owns the capture state machine.
- No end-to-end test drives a real recording through capture, STT, and
  processing. The eval fixtures cover the processing half of that.
- **No browser-level test drives web capture in a real browser.** The
  extraction layer is covered by jsdom fixtures and the wire format by the
  contract tests above, but a fixture cannot tell you that ChatGPT changed its
  markup last week. `docs/capture.md` §11 holds the reproducible manual
  procedure, and it is the only thing that validates the site extractors
  against live pages.
- The extension bundle build (`npm run build:extension`) is not a CI step. Its
  sources live under `native/src/webcapture/`, so they are typechecked and
  unit-tested with the rest of the frontend; what is unverified in CI is the
  bundling itself.
