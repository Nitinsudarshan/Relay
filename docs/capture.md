# Relay Capture — web capture architecture

Traced from v0.26.0. Capture turns the page or conversation a user is looking
at into a durable Vault artifact, with its source, its structure, and an
honest record of how much of it could actually be read.

The design principle everything below follows: **acquisition first,
interpretation second**. A capture is complete and on disk before any model
sees it. Analysis is a separate step that is allowed to fail.

## 1. Where the work happens, and why it has to

Extraction happens **in the browser**. That is not a preference — it is the
only place the rendered DOM exists, and it is the only place a browser will
grant access to it.

Chrome's `activeTab` permission is granted only in response to one of four
gestures, all made inside the browser: executing the extension's action
(toolbar button), executing a context-menu item, executing a keyboard
shortcut from the `commands` API, or accepting an omnibox suggestion. The
grant covers that one tab, and is revoked on navigation to a different
origin.[^activetab]

This has a direct product consequence, and it changed the original design:

> **An OS-level hotkey in Relay cannot read the page you are looking at.**

A global hotkey pressed while the browser is focused is not one of the four
gestures. For Relay to read the tab from outside the browser, the extension
would need standing host permission for every site the user visits
(`<all_urls>`), which is exactly the access this feature is built to avoid.
So the capture *trigger* lives in the browser (`Ctrl+Shift+Y` by default,
editable at `chrome://extensions/shortcuts`), and Relay's own
`capture_hotkey` (default `Ctrl+Shift+C`) opens the Captures surface rather
than pretending to capture. `Ctrl+Space+C` — the shape originally sketched —
is additionally not a registrable accelerator: an OS shortcut is modifiers
plus one key, and `Space` is not a modifier. `Ctrl+Space` is already
push-to-talk dictation.

Desktop-initiated browser capture is not abandoned, it is deferred with its
constraints written down — see `maybe_later.md` §13.

## 2. Architecture

```text
   ┌── browser ──────────────────────────────┐   ┌── Relay (Rust) ─────────────┐
   │ Ctrl+Shift+Y / toolbar button           │   │                             │
   │        ↓ (grants activeTab)             │   │                             │
   │ service worker                          │   │                             │
   │        ↓ scripting.executeScript        │   │                             │
   │ extractor registry (isolated world)     │   │                             │
   │   chatgpt · claude · github · generic   │   │                             │
   │        ↓ structured, text-only payload  │   │                             │
   │ POST http://127.0.0.1:<port>/v1/capture │──▶│ bridge: token + origin +    │
   │        X-Relay-Token                    │   │ size checks                 │
   └─────────────────────────────────────────┘   │        ↓                    │
                                                 │ parse + validate            │
                                                 │        ↓                    │
                                                 │ source detection (from URL) │
                                                 │        ↓                    │
                                                 │ sanitize + normalize        │
                                                 │        ↓                    │
                                                 │ VaultFile + raw payload     │
                                                 │        ↓ (separate, failable)│
                                                 │ analysis → summary, topics  │
                                                 └─────────────────────────────┘
```

The split of responsibility is deliberate: the browser is trusted to *read* a
page, never to *label* one. Source detection, capture-type classification and
sanitization all run in Rust, from the URL and the content — never from what
the payload claimed to be.

### Files

| Path | Role |
|---|---|
| `native/src/webcapture/types.ts` | The payload contract, shared with Rust. |
| `native/src/webcapture/dom.ts` | DOM → blocks, hidden-content rules, coverage assessment. |
| `native/src/webcapture/extractors/` | `generic`, `conversation` (shared), `chatgpt`, `claude`, `github`, and the registry. |
| `native/src/webcapture/capture.ts` | The fallback ladder and payload assembly. |
| `native/src/webcapture/background.ts` | Service worker: holds the token, posts to Relay. |
| `native/src/webcapture/content.ts` | The injected entry point. |
| `native/src/webcapture/options.ts` | Pairing screen. |
| `native/browser-extension/` | `manifest.json`, `options.html`, README; build output lands here. |
| `native/src-tauri/src/capture/web/source.rs` | URL parsing, application and capture-type detection. |
| `native/src-tauri/src/capture/web/normalize.rs` | Sanitization, markdown rendering, fidelity and coverage. |
| `native/src-tauri/src/capture/web/bridge.rs` | The loopback listener, its auth and its limits. |
| `native/src-tauri/src/capture/web/mod.rs` | Wire types, validation, `ingest`. |
| `native/src/components/captures/` | The Captures surface. |
| `native/src/components/settings/CaptureSettingsView.tsx` | Enable, pair, and explain. |

## 3. Transport: why loopback and not native messaging

Both are supported browser↔desktop channels. Native messaging avoids sockets
entirely, and its message ceiling in the browser→host direction is 4 GB
(1 MB the other way).[^nativemsg] It was rejected for v1 because it requires,
per browser: a registry key under
`HKCU\SOFTWARE\Google\Chrome\NativeMessagingHosts\<name>`, a host manifest
file, an `allowed_origins` list pinned to a specific extension id (no
wildcards), and a **second executable** that the browser — not Relay —
launches, which then has to reach the already-running Relay process anyway.
That is three moving parts and an installer step to replace one in-process
listener, in an app that is by definition already running when a capture
happens.

The loopback bridge's costs are named rather than glossed: any *local*
process can reach the port (mitigated by the pairing token, §5), and Chrome's
Local Network Access permission — shipping from Chrome 142 — gates requests
from a public origin to loopback.[^lna] The specification defines address
spaces and the crossings that prompt, and says nothing about
`chrome-extension://` origins; whether extension service workers are treated
as public is **not settled by the documentation and needs verifying against a
released Chrome**. If extensions turn out to be in scope, native messaging is
the migration path, and it is confined to `bridge.rs` plus
`background.ts` — the payload contract, the extractors, normalization and
storage are unaffected.

## 4. The fallback ladder

Every capture reports which rung it reached; none silently substitutes for
the one above it.

| Rung | Strategy | Fidelity | When |
|---|---|---|---|
| 1 | A site extractor recognised the page | `structured` | ChatGPT, Claude, GitHub |
| 2 | Generic main-content extraction | `generic` | Anything with an article region |
| 3 | The page's visible text | `text_only` | No recognisable structure |
| 4 | Refuse | — | Nothing readable: no artifact is created |

A site extractor that throws does not cost the capture — the page falls to
rung 2 and the artifact carries a note naming the extractor that failed,
which is also the early warning that it needs updating.

Screenshot and OCR are deliberately **not** rungs. They are logged as
deferred (`maybe_later.md` §14): the whole point of the feature is structured
acquisition, and a screenshot below `text_only` would add a large dependency
to serve a case that is already honestly labelled.

## 5. Security and privacy model

**Permissions.** The extension declares `activeTab`, `scripting`, `storage`,
and one host permission: `http://127.0.0.1/*`. There is no `<all_urls>`, no
declared content script, and no permission to any website. A page the user
has not invoked capture on is never read.

**Data flow.** Browser → `127.0.0.1` → Relay's vault. There is no server, no
third party, and no telemetry in the capture path. Analysis uses whichever
LLM provider the user has already configured; with the default local Ollama
provider, captured content never leaves the machine at all.

**The listener.** Bound to `127.0.0.1` only, never `0.0.0.0`. Off by default:
a fresh install opens no socket, because capture cannot work before an
extension is installed and paired anyway.

**Authentication.** Every route — including `/v1/health` — requires a 256-bit
pairing token in an `X-Relay-Token` header, compared in constant time. The
token is generated when capture is first enabled, is displayed in Settings
for copy-paste pairing, and is never logged. Regenerating it unpairs every
browser immediately.

**Origin checks.** Only `chrome-extension://`, `moz-extension://`,
`extension://` and `safari-web-extension://` origins are accepted, and the
check runs *before* the token comparison. The browser sets `Origin` itself, so
a page on the open web cannot forge one. Responses name exactly one allowed
origin — never `*`.

**Limits.** 16 KiB of request line and headers, 8 MiB of body, enforced while
reading rather than after; 10-second read and write timeouts; one thread per
connection so a stalled client cannot block the next capture.

**A webpage is untrusted input.** Nothing captured is ever executed, and
nothing is stored as HTML — the payload has no field that can carry markup.
On the way in, `normalize.rs`:

- strips C0/C1 control characters, the BOM, and the Unicode
  bidirectional overrides (a forgery primitive in an archive, not formatting);
- drops any link or image target that is not an ordinary `http(s)` URL, and
  records how many it dropped;
- fences code with a backtick run longer than any inside it, so captured code
  cannot escape its block;
- **downgrades `mermaid` code fences to `text`** — Relay's markdown view
  renders mermaid to SVG and injects the result with `dangerouslySetInnerHTML`,
  so a captured page must never reach that renderer. The diagram source is
  still preserved, as text;
- escapes `|` in table cells, and caps every string, list, table and block
  count;
- builds the stored payload's filename from a strict allowlist, so a page
  title of `../../../etc/passwd` cannot produce a path that leaves its
  directory.

Unknown block types from a newer extension deserialize to `Unknown`, are
skipped, and are counted in the artifact's provenance — a version skew
degrades a capture instead of failing it.

## 6. Completeness — the honest part

A DOM is not a document. Chat interfaces render only the turns currently on
screen; feeds load as you scroll; content can exist only in application
state. So a capture never claims completeness it cannot evidence:

- `full_document` — ~90%+ of the page's visible text was recognised as
  content **and** nothing on the page looks virtualized. This is the only
  value that means "the whole page".
- `rendered_dom` — the default for anything DOM-derived, and always the
  answer for a conversation. Carries a note saying so in plain language.
- `partial` — content was demonstrably dropped: a cap was hit, or the
  extractor knows it missed part of the page.
- `unknown` — the page reported no visible text.

Conversation extractors add what the page's own scroll state implies: if the
scroller is not at its beginning, the artifact says the conversation was not
scrolled to the start. Relay does **not** scroll the user's page to read more
of it — that is a side effect a capture should not have, and it still would
not prove the result was complete.

Every one of these statements lands in `capture.notes` and is shown verbatim
in the capture's *Where it came from* tab, and as a badge on the list when a
capture is not complete.

## 7. Storage

A capture is a `VaultFile` with a `capture: Option<CaptureProvenance>` field —
the same model imported documents use, so summarisation, analysis, promotion
to a Scribble, Trash and restore all work with no second code path. Captures
live in their own directory (`vault/captures/`) rather than mixed into
`vault/files/`, which is what keeps the Files surface, its dedupe rules and
its text-extraction path exactly as they were.

```text
.relay/vault/captures/<capture_id>/
  metadata.json                     # VaultFile: normalized markdown + provenance
  original/<Sanitized-Title>.json   # the raw structured payload, written once
```

Layout and schema: `docs/data-model.md` §6. Commands: `docs/api.md`.

**Provenance is not semantics.** `capture` holds only where the content came
from and how completely it was acquired. `summary`, `tags`, `topics` and
`entities` are produced later by analysis. Re-analysing a capture can never
rewrite the record of its source, and `renormalize_capture` can rebuild the
markdown from the preserved payload without touching identity or history.

**Re-capture** follows the convention file import already set. Identical
content from the same URL bumps `recapture_count` on the existing artifact.
*Changed* content from the same URL becomes a new artifact with
`version: n+1` and `previous_capture_id` pointing at the one it supersedes —
a page that changed is new information, not a duplicate.

## 8. How a capture joins the rest of Relay

| System | How captures participate |
|---|---|
| Analysis | `enrich_vault_file` / `summarize_vault_file`, unchanged — a capture is analysed by the same canonical contract as a document. |
| Talkback | A new `SourceType::Capture`, gathered from `list_captures()`, weighted like an imported file. |
| Search & knowledge graph | Via promotion to a Scribble (`create_scribble_from_vault_file`), exactly as imported files do. The promoted Scribble's `source_type` is `browser_conversation` or `browser_page` — constants that already existed on the model and that capture is what finally populates. |
| Trash | Trashed as `item_type: "capture"` and restored into `vault/captures/`. |

## 9. Supported sources

| Source | Extractor | Produces |
|---|---|---|
| ChatGPT (`chatgpt.com`, `chat.openai.com`) | `chatgpt` | Conversation, roles from `data-message-author-role`, falling back to the turn's own accessibility label |
| Claude (`claude.ai`) | `claude` | Conversation, roles from which selector matched |
| GitHub (`github.com`) | `github` | Repository (description + README), issue/PR/discussion as a conversation, a single file as one code block |
| Everything else | `generic` | Article: headings, paragraphs, lists, code, quotes, tables, images, links, plus `<head>` metadata |

Site extractors are independent modules with an ordered list of selector
strategies; the strategy that recognises the most turns wins, and a fallback
winning is itself recorded as a note. Adding a site is a new file and one line
in `extractors/index.ts`.

## 10. Known limitations

- **Selectors drift.** ChatGPT's and Claude's DOM markers are implementation
  details of sites that redesign often. Each extractor has layered fallbacks
  and hands over to the generic extractor rather than failing, but structured
  conversation extraction *will* need maintenance.
- **Only rendered content.** Virtualized conversations, infinite scroll,
  collapsed sections and content that exists solely in application state are
  not reachable from the DOM. Relay reports this; it does not solve it.
- **No screenshot or OCR fallback.** Deferred, see §4.
- **Shadow DOM and cross-origin iframes** are not traversed.
- **PDF viewers, the Chrome Web Store, and other privileged pages** cannot be
  scripted by any extension; the toolbar badge says so.
- **Firefox** is structurally compatible (`scripting` + `activeTab` from
  Firefox 101) but the manifest ships Chrome/Edge shapes and Firefox is not
  validated.
- **Local Network Access** may affect the loopback transport in Chrome 142+;
  unverified, see §3.
- **No browser-level automated tests.** See §11.

## 11. Testing

| Layer | Where | What |
|---|---|---|
| DOM extraction | `native/src/webcapture/dom.test.ts` | Block types, hidden content, malformed markup, deep nesting, duplicates, unicode, links, metadata, every coverage verdict |
| Site extractors | `native/src/webcapture/extractors/*.test.ts` | Role attribution, turn ordering, code/tables/lists, fallback strategies, hidden turns, giving up rather than guessing |
| Generic extraction | `extractors/generic.test.ts` | Article, docs page, blog, nav/sidebar noise, tables, code, images, near-empty pages |
| The ladder | `capture.test.ts` | Strategy selection, a throwing extractor, refusing an empty page, caps and truncation, JSON round-trip |
| **The contract** | `contract.test.ts` + `capture::web::contract_tests` | Payload fixtures generated by the TypeScript side and consumed by Rust tests, so the two models cannot drift apart silently |
| Source detection | `capture::web::source` | URL parsing, userinfo attribution, GitHub path shapes, unknown sites, rejected schemes |
| Normalization | `capture::web::normalize` | Sanitization, bidi/control stripping, mermaid downgrade, fence escaping, tables, unicode, coverage downgrade, empty refusal |
| The bridge | `capture::web::bridge` | Token comparison, origin refusal, preflight, size limits, unknown routes, CORS headers, and a real loopback socket accepting and refusing requests |
| Persistence | `capture::web::ingest_tests` | Payload → artifact, raw payload preserved, re-capture and versioning, Files isolation, promotion provenance, Trash round-trip, Talkback candidacy, path-traversal refusal, a 1200-turn conversation |
| UI | `components/captures/*.test.tsx` | Completeness wording, filtering, the bridge-off warning, confirmed deletion |

**Browser-level validation has not been automated**, and this document does
not claim it has. Adding a real browser driver to Relay would mean a new
toolchain, and the fixtures above cover the extraction logic. Fixtures cannot
tell you a site changed its markup last week. The manual procedure is:

1. `cd native && npm run build:extension`, then load
   `native/browser-extension` unpacked (`chrome://extensions` → Developer
   mode → Load unpacked).
2. Relay → Settings → Capture → enable, then paste the port and token into the
   extension's Options and choose **Save and test**.
3. Capture, and check the artifact's *Where it came from* tab each time:
   - a short ChatGPT conversation, then a long one (100+ turns) — confirm turn
     order, roles, and that coverage reads `rendered_dom`;
   - the same for a Claude conversation;
   - a GitHub repository, an issue with several comments, and a source file;
   - a long article, a documentation page with tables and code, and a page
     that is mostly navigation;
   - a page with nothing readable — confirm the badge reports failure and that
     **no artifact appears** in Captures;
   - the same page twice unchanged (expect one artifact, re-capture counted),
     then after it changes (expect a second artifact at version 2).
4. Regenerate the contract fixtures if the payload shape changed:
   `RELAY_UPDATE_CAPTURE_FIXTURES=1 npm test`, then run `cargo test` and
   review the diff.

## 12. Extension points

- **A new site**: add a module under `extractors/`, register it in
  `extractors/index.ts`. Nothing in the capture path knows which sites exist.
- **A new capture type**: add the constant in `source.rs` and the label in
  `captureFormatting.ts`.
- **A new block type**: add a variant to `ContentBlock` on both sides and a
  case in `render_block`. Older Relay builds skip it and say so.
- **A non-web source** (a desktop window, a PDF viewer): `source_type` on
  `CaptureProvenance` exists for exactly this. Everything downstream of
  `ingest` — storage, analysis, Talkback, promotion — is source-agnostic
  already.
- **A different transport**: confined to `bridge.rs` and `background.ts`.

[^activetab]: Chrome Extensions documentation, *The `activeTab` permission* —
  [source](https://github.com/GoogleChrome/developer.chrome.com/blob/main/site/en/docs/extensions/mv3/manifest/activeTab/index.md).
[^nativemsg]: Chrome documentation, *Native messaging* — host manifest fields,
  Windows registry location, framing, and the 1 MB / 4 GB size limits.
[^lna]: `WICG/local-network-access` explainer, and Chrome's Local Network
  Access announcement (Chrome 142).
