# Capture v2 — progressive traversal: research and design

Written against v0.26.0, before implementation. This is the research pass the
work in v0.27.0 was built from: what a browser actually makes available when
a page is progressively traversed, what it does not, and which of the two the
code is allowed to claim.

Read it as history plus rationale. `docs/capture.md` is the living
specification; where the two disagree, that one is right.

## 0. What prompted it

v0.26.0 captures the DOM as it stands at the moment of invocation. Real browser
testing showed that this is a floor, not a ceiling:

- a long ChatGPT thread scrolled to the bottom yields the last handful of turns
  and nothing before them;
- lazily-loaded sections and images are not in the DOM until you approach them;
- generated images do not appear where the extractor looks for them at all.

The v0.26.0 answer was to report the gap (`rendered_dom` plus a note). That is
honest, and insufficient: the browser *would* hand over most of the missing
content if asked. So the first question is **how much more, at what cost, and
with what risk**.

### The observation that reframed it

A long prompt was pasted into Claude. Claude shortened the message and showed
"Show more". Nobody clicked it. Relay captured, and the artifact reported:

> ✓ The whole page was captured

Two things were tangled together in that, and separating them is what the rest
of this document is built on.

**Was the content missing?** No. Claude's shortening is CSS, and Relay's block
walk reads `textContent`. Verified in Chromium (§2.5): a
`-webkit-line-clamp: 3` box holding 2,904 characters returned all 2,904 from
`textContent` *and* from `innerText`. The text was never missing. So the
tempting fix — find "Show more", click it — would have been an interaction that
bought nothing, on someone else's page, for content Relay already had.

**Was the claim defensible?** No, and this was the real defect. It was not
caused by the truncation at all (§6). A successful extraction is not a complete
capture, and v0.26.0 had no way to tell the difference.

That reframes the work into two goals that pull in opposite directions and both
have to hold:

1. acquire much more of the source than a single DOM read can reach;
2. be much more conservative about what the result is allowed to claim — and
   about what it is allowed to *do* downstream (§9).

Hence the ordering principle everything below follows: **inspect first,
interact only when necessary.**

## 1. What could and could not be verified here

This matters more than any conclusion below, so it comes first.

| Claim | How it was established |
|---|---|
| How browsers expose clipped, collapsed, hidden and virtualized content (§2.5) | **Measured** in real Chromium — these are the load-bearing facts, and every number quoted below came out of a run |
| Traversal, settle detection, expansion, dedup and ordering behave as designed | **Verified** in real Chromium (`scripts/capture-validation/`) against fixture pages that reproduce virtualization, network-paged history, lazy loading, CSS truncation, disclosure widgets and hostile controls: 35 assertions |
| Timing and payload numbers | **Measured** in that runner — see `capture/BENCHMARKS.md` |
| Extraction, merging, availability classification and the safety classifier | **Verified** by unit tests in jsdom |
| The trust boundary downstream of capture | **Verified** by Rust tests that keep adversarial content and assert where it can and cannot reach |
| ChatGPT's and Claude's DOM markers, virtualization behaviour, attachment and image shapes | **Not verified here.** Taken from published sources, cited inline. The container this work was done in has no egress to `chatgpt.com` or `claude.ai`, and both require an authenticated session. Every site-specific selector is therefore *evidence-backed but unvalidated*, and is written as a layered strategy that degrades rather than a fact |
| One capture of a real Claude thread | **Reported by the user**, and reproduced as a fixture rather than taken on faith — §0, §6 |

The manual browser procedure in `docs/capture.md` §12 is what closes that
last row, and it has been extended to cover the cases this document raises.
Nothing in the shipped code claims a source-specific behaviour was observed.

## 2. Browser behaviour, source by source

### 2.1 The general mechanisms

Four distinct mechanisms hide content from a one-shot DOM read, and they need
different answers:

| Mechanism | What the DOM holds | What reveals it | Is it recoverable? |
|---|---|---|---|
| **Virtualization** | Only the mounted window; off-screen items are *removed* | Scrolling to them | Yes, but only *while they are mounted* — a second read after scrolling past loses them again |
| **Lazy loading of media** | The element, with no `src` (or a placeholder `data-src`) | Approaching the viewport | Yes, and it stays revealed |
| **Network-paged history** | The most recent page of items | Reaching the scroll boundary, then waiting on a fetch | Yes, and it stays in the DOM (until virtualization unmounts it) |
| **Truncation / collapse** | The *truncated* text, plus a control | Activating the control | Yes, and it stays revealed |

The first is the one that dictates the architecture. Because virtualized items
are unmounted as you pass them, **content must be harvested during traversal,
not after it**. A "scroll to the bottom, then extract" design captures the end
of a long thread and nothing else — which is the bug v2 exists to fix, in a
new costume.

`content-visibility: auto` deserves a note: it skips rendering, but the nodes
and their text stay in the DOM, so `textContent`-based extraction already sees
them. It affects `innerText` (which is layout-dependent) but not the block
walk. It is not a mechanism Relay has to defeat.

### 2.2 ChatGPT

ChatGPT is a **virtualizing** conversation surface.

- Turns are wrapped in `article`/`section` elements carrying
  `data-testid="conversation-turn-N"`; the role is stated on a descendant
  `[data-message-author-role]`.[^turnsel][^scrapfly]
- Short conversations mount every turn. Long ones virtualize: the scroll
  container reports a large `scrollHeight` while only a small number of turns
  are mounted, and turns scrolled out of view are removed from the DOM.[^virt][^exporter]
- Pinning to the top of a long thread triggers a **network fetch of older
  history** — reaching the boundary once is not enough, the boundary
  moves.[^exporter]
- Code blocks are CodeMirror-based on current `chatgpt.com`, and also appear
  as custom `code-block` elements.[^exporter]
- **Generated images are rendered outside `[data-message-author-role]`.** They
  live inside the `section[data-testid^="conversation-turn-"]` wrapper, as
  `img[src*="/backend-api/estuary/content"]` or `img[alt^="Generated image"]`,
  and an extractor that walks only role-bearing elements never sees
  them.[^images]
- Image and file references are authenticated same-origin URLs
  (`chatgpt.com/backend-api/…`, redirecting to `files.oaiusercontent.com/…`);
  they resolve only with the user's session.[^images][^assets]
- Files the model produced are exposed as `sandbox:/mnt/data/<name>` links —
  an opaque reference, not a fetchable URL.[^sandbox]

**Direct consequence for Relay**: v0.26.0's ChatGPT extractor uses
`[data-message-author-role]` as its primary selector, so on the published
evidence it misses generated images entirely. The v2 extractor keys on the
turn wrapper and reads the role from the descendant, which is both more
complete and a better ordinal source.

### 2.3 Claude

Claude is **not** a virtualizing conversation surface, and assuming symmetry
with ChatGPT would have produced the wrong engine.

- The web UI loads every message into the DOM at once. This is documented in
  the open — a feature request asking for virtual scrolling describes the
  current behaviour as "loads all messages into the DOM at once", with RAM
  growth and tab crashes on 1,000+ message conversations as the
  consequence.[^claudevirt] It was closed as not planned.
- So for Claude the expensive part of traversal is unnecessary: one settle at
  the boundary usually mounts the whole thread. The engine measures this rather
  than being configured with it (§4.3), so if Claude starts virtualizing later
  the behaviour follows.
- Long user messages *are* shortened behind a "Show more"
  control[^claudeshowmore] — but the shortening is presentation, not omission.
  §2.5 measured a clamped box returning its full text from both accessors, so
  the right response is to read rather than click, and
  `expansions_unnecessary` counts how often that happened. The earlier
  sentence in this document's first draft — "the collapsed form is what the DOM
  holds" — was an assumption, and it was wrong.
- Human turns are marked `[data-testid="user-message"]`; assistant turns are a
  response container (`.font-claude-response`).[^contextsync]
- **Artifacts are not in the message DOM.** An artifact opens in a dedicated
  side panel with Preview and Code tabs; the message itself carries only a
  card.[^artifacts] Capturing artifact *source* therefore requires opening a
  panel — one at a time, changing the app's view state.

**Direct consequence for Relay**: the traversal plan for Claude is
expansion-heavy and scroll-light, and artifacts are recorded as
discovered-but-not-captured rather than clicked open (§5).

### 2.4 GitHub and generic pages

- GitHub issue and PR timelines hide the middle of long threads behind
  "Load more…" controls, and hide outdated review comments behind
  disclosure widgets. These are content-disclosure controls in exactly the
  sense §4 defines, and they are the reason the GitHub plan enables expansion
  at all.
- Generic long pages are dominated by lazy media and `IntersectionObserver`
  reveal animations. Scrolling once, top to bottom, is close to sufficient;
  the payoff is image `src` attributes and any `<details>` content.
- Feeds (a generic page that virtualizes) exist, so the generic plan uses the
  same sample-and-merge path rather than a single read — the degenerate case
  of one sample is the same code.

### 2.5 What Chromium actually does — measured

Four measurements decided the design, and two of them refuted the hypothesis
they were made to test.

**CSS truncation does not hide text from either accessor.** A box with
`-webkit-line-clamp: 3` and one with `max-height` + `overflow: hidden`, each
holding 2,904 characters:

| | `textContent` | `innerText` | `scrollHeight` | `clientHeight` |
|---|---|---|---|---|
| `-webkit-line-clamp: 3` | 2,904 | 2,904 | 777 | 63 |
| `max-height: 63px` | 2,904 | 2,904 | 777 | 63 |

So the text is fully available, and `scrollHeight > clientHeight` is the
reliable signal that a box is showing less than it holds. The initial theory —
that clipping inflated the coverage ratio because `innerText` omits clipped
text — is **wrong**, and would have produced a fix for a mechanism that does
not exist.

**A closed `<details>` is where the ratio actually breaks.** Its body is in
`textContent` and *not* in `innerText`, while its children report a normal
computed `display` and a non-null `offsetParent` — so Relay's `isHidden` does
not skip it and the block walk captures it. That is the right outcome for the
content and the wrong one for the arithmetic: on a Claude-shaped fixture,
`body.innerText` measured 5,230 characters against 5,297 of extractable text.
A numerator from `textContent` over a denominator from `innerText` can exceed
1, which sails past a 0.9 threshold. Screen-reader-only text positioned
off-screen does the same thing.

**`content-visibility: auto` is not a hazard.** Its text appears in both
`textContent` and `innerText`. Nothing needs to defeat it.

**A programmatic click drives real handlers.** `element.click()` fired a
listener added with `addEventListener`, flipped `aria-expanded`, and the
handler's DOM mutation landed — with `event.isTrusted === false`. So expansion
works, and a site that gates on `isTrusted` will simply not expand, which is
counted as `expansions_failed` rather than silently ignored.

**And the walk works.** A 500-item virtualized list, stepped by its mounted
extent with one item of overlap: 83 steps, 2.77 s, all 500 items seen, none
missed. That is the floor the settle logic is added on top of.

## 3. Content availability, and the least-invasive rule

The mechanisms in §2.1 are not one problem. They are six, they call for
differently invasive answers, and conflating them produces either a thin
capture or a browser agent. So they are named in the contract
(`ContentAvailability`) and counted separately on every artifact:

| State | What the DOM holds | The answer |
|---|---|---|
| `outside_viewport` | The content, off screen | Extract it. No interaction. |
| `visually_truncated` | The content **in full**, shortened by CSS | Extract it. **Do not click.** |
| `collapsed` | Only the short form | Expand — the one state that earns a click. |
| `not_loaded` | Nothing yet | Traverse to it. |
| `virtualized` | A moving window | Traverse, harvesting while mounted. |
| `inaccessible` | Nothing reachable | Report it. Do not bypass it. |

Acquisition tries them in that order of invasiveness:

1. extract what is in the DOM;
2. extract what is in the DOM but not rendered (already the case — the block
   walk reads `textContent`, so a closed `<details>` and an off-screen label
   are captured without touching anything);
3. expand a disclosure control, but only after proving its content is genuinely
   absent;
4. traverse when content is not loaded;
5. use the source's own strategy where a generic one will not do;
6. report what remains out of reach.

The distinction between states 2 and 3 is not cosmetic. It is measurable: the
validated Claude fixture reports `expansions_unnecessary: 2` and
`expansions_opened: 0`, and the clipped container is still clipped when the
capture finishes. Interaction is not added because it is technically possible.

## 4. The traversal algorithm

### 4.1 Where to start — the four options

The evidence answers this directly:

- **A. Start at the current position.** Rejected. On a virtualizing surface
  everything above the viewport is unmounted, so this cannot see the start of
  a conversation — the exact failure being fixed.
- **B. Move to the top, then traverse down.** Correct for virtualization, but
  incomplete on its own: for ChatGPT the top *moves* as older history is
  fetched, so a single seek to the boundary lands in the middle of the thread.
- **C. Traverse up, then down.** The right instinct, wrong shape. What is
  actually needed is not a slow upward crawl but a **bounded rewind**: seek to
  the boundary, settle, and if the boundary moved, seek again — until it stops
  moving or the budget runs out. Crawling upward step by step costs the same
  content in twice the time.
- **D. Source-specific.** Necessary regardless: Claude needs almost no
  scrolling and a lot of expansion; ChatGPT needs the opposite; a static
  article needs one pass.

**What was built: B with a bounded rewind phase, parameterised per source
(D).** One engine, a plan per source.

### 4.2 The loop

```text
record the user's scroll position
  ↓
resolve the scroll surface (plan selectors → the window)
  ↓
rewind: seek to the boundary, settle, repeat while the boundary moves   ← bounded
  ↓
┌─ per step ─────────────────────────────────────────────────┐
│ settle (mutations quiet + signature stable, or timeout)     │
│ discover expandable elements in the current window          │
│ classify each one; activate only what passes                │
│ settle again if anything was activated                      │
│ sample: harvest what is mounted right now                   │
│ compute the next step from the mounted extent               │
└─────────────────────────────────────────────────────────────┘
  ↓  until: reached the end · no progress · budget spent · interrupted
merge samples: deduplicate, order, keep the richest version
  ↓
restore the user's scroll position
```

### 4.3 The step size is adaptive, and this is the main performance decision

Stepping by a fixed fraction of the viewport is the obvious choice and the
wrong one. A 1,000-turn thread at ~400px per turn is ~400,000px; at 600px per
step that is ~660 steps, and at even 150ms per step that is a 100-second
capture. Unacceptable per the brief, and unnecessary.

Instead each step measures **the mounted extent** — the span from the first to
the last mounted item — and steps to just short of its bottom edge, keeping
one item of overlap so nothing can be unmounted before it was harvested. The
consequences:

- Claude, which mounts everything, finishes in two or three steps.
- ChatGPT steps by its whole mounted window rather than by one viewport, which
  is several turns at a time.
- A page with no identifiable items falls back to viewport × overlap.

Overlap is not optional: without it a step can unmount the very items it
scrolls past, and the capture silently loses a band of content per step.

### 4.4 Termination, and why each reason is recorded

Traversal stops for exactly one of: `reached_end`, `no_progress`,
`step_budget`, `time_budget`, `expansion_budget`, `user_interrupted`,
`navigation_detected`, `error`, `not_needed`. The reason is stored on the
artifact, because it is the difference between "this is the whole thread" and
"this is the first ten seconds of a very long thread". `reached_end` is the
only one that supports a `full` claim, and only alongside the other evidence
in §6.

Infinite loops are prevented by three independent bounds — a step count, a
wall-clock budget, and a no-progress counter — plus a monotonic scroll
position, so a page that resets `scrollTop` on every read cannot trap the
engine.

### 4.5 The user's scroll position

Recorded before the rewind and restored in a `finally`, for both the window
and the resolved surface. If the surface disappeared during traversal, that is
recorded as `scroll_restored: false` rather than silently ignored.

Traversal also aborts on user interaction — a wheel, touch, or key event
during the run sets an abort flag and the engine restores and returns what it
has, with `user_interrupted` as the reason. A capture is a background
courtesy; it does not get to fight the user for their own page.

## 5. Expansion: an explicit classifier, not a click-everything pass

The brief's constraint is the right one: no generic "click anything containing
*more*" mechanism. What was built is a classifier with a deny-list that wins,
an allow-list that is not sufficient on its own, and a structural requirement
that no label can override.

**A candidate must be inside the content container.** Page chrome — nav,
header, footer, toolbars, menus, dialogs, anything inside a `form`, anything
inside a `contenteditable` composer — is out of scope before any label is read.
This single rule removes most of the risk, because destructive controls live in
chrome and in per-message action rows, not in the prose.

**Then, in order:**

1. **Structural deny.** `disabled`, hidden, zero-sized, `type=submit`,
   `type=reset`, `aria-haspopup` (a menu or dialog opener — "More actions" is
   exactly this), an `<a>` whose `href` would navigate, or membership of a
   plan-declared forbidden region. Also anything already activated once
   (tracked in a `WeakSet`, so a toggle is never flipped back).
2. **Label deny.** The accessible name (text, `aria-label`, `title`) matched
   against delete · remove · discard · regenerate · retry · submit · send ·
   share · publish · download · export · copy · save · settings · preferences ·
   sign out · subscribe · upgrade · buy · purchase · new chat · edit · rename ·
   report · flag · vote · like · follow · install · run · execute · deploy ·
   merge · approve · request changes · close · reopen · archive · more actions.
   A deny match ends it — a control labelled "Show more actions" is not a
   disclosure control.
3. **Positive evidence, any one of:**
   - `aria-expanded="false"` — by definition a disclosure widget, and the
     strongest signal available;
   - a `<summary>` inside a closed `<details>` — disclosure with no script at
     all;
   - an accessible name on the allow-list: show/read/see/view more · show all ·
     see all · expand · expand all · show full … · continue reading · show
     original · show details · load more · show earlier · show previous · show
     thinking · show reasoning · show N more …;
   - a plan-declared, source-verified selector.
4. **Verification after the fact.** `aria-expanded` flipping to `true`, or the
   content signature changing, counts as a success; neither changing counts as
   `expansions_failed`. Both numbers are reported, because an expansion that
   silently stopped working is how a site redesign shows up first.

**Two runtime guards** back the classifier up, on the principle that a
classifier over untrusted input will eventually be wrong:

- a capture-phase `submit` listener on the document, installed for the
  duration of the expansion phase, that cancels any form submission the page
  attempts;
- a `location.href` snapshot compared after every activation; a change stops
  all further expansion and is recorded as `navigation_detected`.

Budgets: a per-capture cap on activations and a per-step cap, so a page with
ten thousand `<details>` cannot turn a capture into a clicking marathon.

## 6. Attachments, images and generated files

### 6.1 The decision on bytes

**v2 captures metadata and references, never file bytes.** This is a
deliberate refusal, and it is worth stating why, because it is technically
possible: a content script's `fetch` runs with the page's cookies, so a
ChatGPT `estuary` image URL or a GitHub attachment *would* resolve, and at
least one existing exporter does exactly that.[^exporter]

Against it, all four of:

1. **It is not what the user asked for.** The gesture was "capture this page".
   Turning that into "download every file referenced by this page" is a
   different act with different consequences, and the brief says as much.
2. **The payload contract forbids it.** The extension↔Relay contract is
   text-only by design — it is what makes normalization a total function over
   untrusted input. Base64 bytes would be the first field in it that is not
   text-shaped, and the 8 MiB body limit would be spent by two screenshots.
3. **It changes the privacy story.** Today a capture reads the tab the user
   invoked it on. Fetching authenticated asset URLs makes Relay a client of
   the site's API, which is a materially larger claim to be making in a
   feature whose selling point is least privilege.
4. **It is separable.** Metadata now, bytes later behind an explicit setting,
   loses nothing: the reference is preserved, so a later version can resolve
   it. The reverse — shipping downloads and retracting them — is not
   available.

So every attachment and image record carries `content_captured: false` and a
`content_note` saying, in plain language, that the file itself was not
retrieved and why. Deferred properly in `maybe_later.md` §16.

### 6.2 How they are represented

Not as a parallel array — as **two new block types in the existing ordered
vocabulary**:

```text
attachment  ├── name          image  ├── alt
            ├── mime                 ├── caption
            ├── size_bytes           ├── src         (http(s) only)
            ├── href     (http(s))   ├── reference   (blob:/data:/sandbox:)
            ├── reference (opaque)   ├── width/height
            ├── kind                 ├── origin
            ├── preview              ├── content_captured
            ├── content_captured     └── content_note
            └── content_note
```

`kind` is `user_upload | assistant_generated | linked | unknown`; `origin` is
`user_upload | assistant_generated | page | unknown`.

Blocks rather than side-tables because blocks are already the single ordered
content vocabulary, already carry message association (they sit inside the
message's own block list), and already degrade correctly on an older Relay —
unknown block types are skipped and counted. A parallel `attachments[]` array
would have needed its own association field, its own ordering rule, and its
own version-skew story, and would have duplicated the inline images the
`image` block already carries.

Counts for the completeness model are derived by walking blocks, so they
cannot drift from the content.

A Claude artifact is an `attachment` block with `kind:
assistant_generated`, whatever the card exposed, and a note that Relay does
not open side panels. `sandbox:/mnt/data/…` becomes `reference`, never `href`,
because it is not a URL and must never reach a link renderer.

### 6.3 Images

Provenance, content and interpretation are kept apart.
Relay preserves the reference, the alt text, the caption, the dimensions and
the association with a message. It does not describe images, and it does not
replace one with a description — visual analysis is an Analyse-stage concern
and would be a source-faithfulness violation here.

`blob:` and `data:` sources are recorded as `reference` rather than `src`, so
they are preserved as evidence without being emitted as a link target that
would never resolve outside the page that made it.

## 7. The completeness model

The four states map onto the four values already persisted on artifacts, plus
one new one; the vocabulary on disk does not churn.

| Model | Stored `coverage` | Means |
|---|---|---|
| FULL | `full_document` | Positive evidence the whole thing was seen |
| PARTIAL | `partial` | Content demonstrably missing — a budget ran out, a cap was hit, ordinals have gaps |
| LOADED_ONLY | `rendered_dom` | Only what was mounted; traversal did not run, or could not prove it finished |
| FAILED | `failed` *(new)* | Traversal errored or was cut short; what is here is a fragment of unknown size |
| — | `unknown` | The page reported nothing measurable |

`full_document` for a conversation now has a route to being true, which it did
not have in v0.26.0. It requires **all** of: a reveal pass that ran and
terminated `reached_end`; no gaps in the page's own turn numbering; nothing left
collapsed or inaccessible; and no expansion attempted and failed. Anything less
is `rendered_dom` or `partial`.

### 7.1 What actually went wrong in v0.26.0

Worth stating precisely, because the diagnosis is the reason the rules above
look like this — and because the first hypothesis was wrong.

`assessCoverage` compared a numerator derived from `textContent` against a
denominator derived from `innerText`. `innerText` is layout-dependent and omits
text that is present in the DOM but not rendered; the extractor's numerator
includes exactly that text. Measured (§2.5): 5,230 against 5,297 on a
Claude-shaped fixture with one collapsed thinking block and one screen-reader
label. The ratio can exceed 1, which clears a 0.9 threshold, and the capture
claims a whole document.

Reproduced end to end in Chromium against the shipped v0.26.0 bundle: a
Claude-shaped page with a clipped message and a collapsed block produced
`extractor: generic`, `coverage: full_document`, and — for what it is worth —
**the marker at the end of the clipped message present in the payload**. Both
halves of the original report, in one run: the content was there, and the claim
was not earned.

The truncation was a red herring. Three things fix the actual fault:

1. the denominator is computed with the same visibility rules the extractor
   uses (`extractableTextLength`), so it is like-for-like;
2. the ratio is clamped at 1, so an inverted comparison cannot pass a
   threshold;
3. a page Relay has a site extractor for can never reach `full_document`
   through the generic path. A known site that Relay failed to recognise is
   evidence *against* completeness — and this is the rule that would have caught
   the reported case on its own.

And one more, on the other side of the boundary: the verdict is **derived in
Rust**, not accepted from the browser. `resolve_coverage` takes the claim and
the reveal pass's numbers, and the numbers may contradict the claim but never
strengthen it.

Alongside it, measurable diagnostics — every one either counted or absent,
never estimated:

```text
steps · scroll span · duration
expansions:  found · opened · refused · failed
messages:    discovered · captured · missing (from ordinal gaps)
attachments: discovered · captured        images: discovered · captured
duplicates dropped · settle timeouts · virtualized · scroll restored
termination reason · inaccessible content, named
```

"Discovered vs captured" is the pair that carries the weight: a conversation
where 340 turns were discovered and 340 captured is a different artifact from
one where 340 were discovered and 90 captured, and v0.26.0 could not tell them
apart. Where the browser cannot supply a number — a total turn count for a
thread whose history was never fetched, for instance — the field is absent
rather than guessed.

## 8. Security model

The premise is unchanged: **a webpage is untrusted input**. Traversal adds
*writes* to the page for the first time, so the boundary is drawn explicitly.

The engine may: read the DOM, set `scrollTop`/`scrollTo` on the surface it
resolved, and activate elements its classifier passed. That is all.

It must not, and structurally cannot: submit a form (chrome is out of scope,
`type=submit` is denied, and a capture-phase listener cancels submissions
anyway); send a message (composers are `contenteditable`, hence forbidden
regions, and send/submit are on the deny-list); delete or modify content;
navigate (navigating anchors are denied pre-flight, and a URL change halts
expansion); purchase (buy/purchase/upgrade/subscribe denied); change settings
(denied by label, and settings live in chrome); execute captured script
(nothing captured is ever evaluated, and the content script runs in the
isolated world so it cannot reach page JavaScript); or cross a security
boundary (no shadow-DOM piercing, no cross-origin iframe access, no new
permissions — `activeTab` and `scripting` already cover reading and clicking
in the tab the user invoked capture on).

Everything Relay already does on the way in — control-character and bidi
stripping, non-`http(s)` target dropping, fence escaping, the mermaid
downgrade, the filename allowlist — applies unchanged to the new block types,
and `reference` is deliberately a plain string that no renderer treats as a
link.

## 9. The downstream boundary: capture is not trust

Acquiring more of the web puts more web text in front of a model, and every
provider's chat format delivers that text in a *role* — `user` — that a model is
trained to obey. `LLMClient::complete` puts its argument there. So the same
change that makes capture more complete makes this boundary load-bearing rather
than theoretical.

```text
CAPTURE      != TRUST
PROVENANCE   != AUTHORITY
COMPLETENESS != PERMISSION TO EXECUTE
```

Three concepts, kept apart, and the separation *is* the design:

- **Provenance** — where it came from. On `CaptureProvenance`.
- **Content** — what the source said, preserved verbatim, including text that
  reads like an instruction.
- **Trust** — what downstream systems may do with it. Always
  `external_untrusted`, and **not a function of the domain**. ChatGPT, Claude,
  GitHub, a documentation site and an anonymous blog all produce external,
  untrusted data. There is no allowlist of trusted sources, and a test asserts
  there never is.

### Why an envelope and not a filter

Filtering suspicious text is the tempting answer and the wrong one. It destroys
source fidelity — a vault that edits its sources is not a record — it is
trivially evaded, and it cannot be evaluated: there is no way to distinguish a
legitimate quotation of a prompt injection from an attempt at one, and a
knowledge vault has to hold both. Every regression test here therefore works by
*keeping* the sentence and asserting where it can and cannot reach.

So the defence is structural. Captured content is framed as data inside a
delimiter carrying a per-call nonce, with a standing rule in the system prompt
saying what the frame means: instructions inside it are content, requests
inside it are not the user's, claims inside it are the source's. The nonce is
what lets the content through unmodified — a page cannot have written the
closing marker into its own text in advance, so the frame holds without editing
a byte.

This is a boundary, not a guarantee. A model can still be talked past a frame.
What it provides is the thing that was missing: captured text is never
delivered as an unmarked instruction, and every system downstream can tell
which is which.

### What the audit of the path found

Tracing capture → normalize → Analyse → promotion → Talkback → LLM context
turned up two places where captured content was indistinguishable from the
user's own:

1. **Analysis.** `enrich_vault_file` and `summarize_vault_file` passed
   `file.content` straight to `complete()`, which delivers it as the user
   message. A captured page's instructions therefore arrived in the one role a
   model is trained to follow. Now: a capture is framed, and the analysis
   system prompt carries the boundary rule. Non-captures are unchanged — the
   user's own notes are the user's own words.
2. **Talkback.** `render_context` heads its block *"CONTEXT — from the user's
   own Relay data"*, and the grounded prompt says to answer only from the
   context. A retrieved web capture landed inside that framing with nothing
   marking it as external — the exact shape that turns a page's instructions
   into the user's. Now: `SourceType::Capture.is_external()` is true, external
   items are labelled `EXTERNAL`, and both prompts say what that means.

Promotion to a Scribble already carried provenance; it now carries `trust` too,
so the knowledge graph can distinguish a page's claim from a fact the user
asserted.

Deliberately **not** built: a prompt-injection classifier. It would be a
dependency, a false-positive surface, and a worse defence than the structural
one — and making capture depend on a model would invert the acquisition-first
principle the whole feature rests on.

## 10. Performance expectations

Set before measuring, so the numbers in `docs/capture/BENCHMARKS.md` can
disagree with them:

| Shape | Expected traversal | Why |
|---|---|---|
| Static article | under 1s | one or two samples, no expansion |
| Non-virtualized conversation, 100 turns | ~1s | everything mounted; a rewind, a settle, a handful of expansions |
| Virtualized conversation, 500 turns | 3–8s | mounted-extent stepping, ~30–60 steps |
| Virtualized conversation, 1,000+ turns | budget-bound | reports `time_budget` and `partial` rather than running for two minutes |

Two mechanisms keep the cost down: sampling is skipped when the content
signature has not changed since the last sample (so a static page pays for one
extraction, not forty), and settle windows are short — a typical settle is one
or two 50ms quiet ticks, with a 1.2s ceiling, not a fixed sleep.

The 8 MiB payload ceiling and the existing caps are unchanged, and now bite
sooner because more content is found. That is the correct behaviour: hitting a
cap is reported as `partial` with a count.

## 11. Architecture

The shape that was built:

```text
source detector  →  traversal plan  →  reveal engine  →  extractor  →  merge
                                            ↑ sample ↓
                                       generic fallback
                                            ↓
                                      normalization (Rust)
                                            ↓
                                       Relay ingestion
```

The separation that matters: **the engine knows how to expose content; the
extractor knows how to interpret it.** The engine's only contact with
extraction is a sampler callback it invokes after each settle. It has no
concept of a message, a role, or an attachment. The extractor has no concept
of scrolling.

They are coupled at exactly one point, and deliberately: an extractor may
expose `harvest(doc)` — "what is mounted right now" — which the engine's
sampler calls, and `extract` becomes `assemble(harvest(...))`. That keeps one
set of selectors per site rather than two, which is the coupling that would
actually have hurt.

Site knowledge stays in one file per site: `chatgpt.ts` exports both its
extractor and its traversal plan. Adding a source is still a module and a
registry line.

## 12. Implementation plan

1. Contract: two block types, richer image fields, a traversal diagnostics
   record, a fifth coverage value — in `types.ts` and `mod.rs` together.
   `PROTOCOL_VERSION` stays at **1**: every change is additive, unknown fields
   deserialize to defaults and unknown blocks are skipped and counted, so a new
   extension against an old Relay and an old extension against a new Relay both
   degrade instead of failing. Bumping it would break both directions to
   express nothing.
2. Engine: `traversal/{surface,settle,expand,engine,plans,types}.ts`.
3. Merge: identity, ordinal, offset, richness, gap detection.
4. Extractors: `harvest` on the conversation spec; the ChatGPT turn-wrapper
   fix; attachment and image discovery per source.
5. Async all the way out: `capture.ts`, `content.ts`, `background.ts`.
6. Rust: new blocks rendered and sanitized, diagnostics persisted, coverage
   recomputed, notes written.
7. UI: the new coverage value, the diagnostics panel.
8. The trust boundary: `pipeline/source_boundary.rs`, wired into analysis,
   summarisation, Talkback context and promotion metadata.
9. Tests: unit in jsdom, contract fixtures regenerated, Rust coverage of the
   derived verdict and the boundary, and a real-Chromium validation runner over
   fixture pages that reproduce the behaviours in §2.

## 13. What was rejected

- **Screenshots and OCR.** Still not a rung. The goal is structured
  acquisition; a picture of text is a downgrade from `text_only`, not a
  fallback below it. (`maybe_later.md` §14.)
- **Fetching file bytes.** §6.1. (`maybe_later.md` §16.)
- **Opening Claude artifact panels.** One at a time, changes the app's view
  state, and on some routes changes the URL. Recorded as inaccessible instead.
- **Reading the sites' internal APIs** (`/backend-api/conversation`). It would
  return the whole thread as JSON and be far more complete than any DOM read.
  It is also a different product: an authenticated API client, using an
  undocumented endpoint, breaking on any change, and reading conversations the
  user did not invoke capture on. Out of scope by design, not by difficulty.
- **A generic click-everything expansion pass.** §5.
- **Clicking "Show more" because it is there.** §2.5, §3 — the content is
  usually already in the DOM, and the click is a side effect on someone's page
  for nothing.
- **Filtering adversarial text out of captured content.** §9.
- **A prompt-injection classifier as a prerequisite.** §9.
- **Crawling upward step by step.** §4.1, option C — a bounded rewind reaches
  the same boundary far faster.
- **Adding Playwright to the repo.** The real-browser runner drives Chromium
  over the DevTools Protocol using only Node built-ins, so validation gained a
  script rather than a toolchain, and CI is unchanged.

[^turnsel]: `section[data-testid^="conversation-turn-"]` as the turn wrapper —
  [chatgpt-chat-exporter](https://github.com/rashidazarang/chatgpt-chat-exporter).
[^scrapfly]: `data-message-author-role` as the primary role marker, with
  `conversation-turn-N` as a fallback —
  [How to Scrape ChatGPT Responses in 2026](https://scrapfly.io/blog/posts/how-to-scrape-chatgpt).
[^virt]: Long threads virtualize: a large `scrollHeight` with few turns
  mounted — [ChatGPT-Virtual-Scroll](https://github.com/9ghtX/ChatGPT-Virtual-Scroll)
  and [chatgpt-lag-fixer](https://github.com/bramvdg/chatgpt-lag-fixer).
[^exporter]: "ChatGPT virtualizes long conversations — messages scrolled out
  of view are removed from the DOM, so a single DOM pass could only ever
  export a fragment"; auto-scroll top to bottom serializing each message while
  it exists, then restore scroll position; a wall-clock budget; waiting on a
  network fetch of older history at the top; keying messages by a hash of
  their *whole* text after prefix-keying collapsed distinct messages; embedding
  image bytes from authenticated endpoints —
  [chatgpt-chat-exporter](https://github.com/rashidazarang/chatgpt-chat-exporter).
[^images]: Generated images render inside the turn wrapper but outside
  `[data-message-author-role]`, as
  `img[src*="/backend-api/estuary/content"]` / `img[alt^="Generated image"]` —
  [ai-browser-bridge#2](https://github.com/YosefHayim/ai-browser-bridge/issues/2).
[^assets]: `chatgpt.com/backend-api/…` asset URLs redirect to
  `files.oaiusercontent.com/…` and require the session —
  [ChatGPT gen_id & Asset Pointers](https://www.hiapi.ai/en/blog/chatgpt-gen-id-asset-pointer-images).
[^sandbox]: Model-produced files appear as `sandbox:/mnt/data/…` references —
  [community.make.com](https://community.make.com/t/how-can-i-download-a-file-from-a-sandbox-mnt-data-url-generated-by-chatgpt/77162).
[^claudevirt]: The claude.ai web UI "loads all messages into the DOM at once";
  virtual scrolling was requested and closed as not planned —
  [claude-code#24146](https://github.com/anthropics/claude-code/issues/24146).
[^claudeshowmore]: Long user messages are collapsed to a summary box behind a
  "Show more" control —
  [claude-code#46046](https://github.com/anthropics/claude-code/issues/46046),
  [claude-code#68487](https://github.com/anthropics/claude-code/issues/68487).
[^contextsync]: `[data-testid="user-message"]` and `.font-claude-response` —
  [context-sync](https://github.com/Vineetpandey0/context-sync).
[^artifacts]: Artifacts open in a dedicated side panel with Preview and Code
  tabs; the message carries a card —
  [Claude Artifacts guide](https://www.ai-toolbox.co/claude-management-and-productivity/how-to-use-claude-artifacts-guide-2026).
