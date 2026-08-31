# Relay — Go-to-Market Plan

Traced from **v0.20.0**, written 2026-08-31. This states what has to be true
before Relay can reach a user who is not willing to compile it, and in what
order. Like `roadmap.md`, it exists so that "we haven't done this yet" is
written down rather than silently implied.

Where this file and the code disagree, the code is right and this document is
a bug. Where it names a competitor or a channel, that is a judgement call from
a specific date, not a fact — re-check before acting on it.

---

## The finding

Relay cannot currently be obtained by anyone unwilling to install Rust 1.75,
CMake, a C++ toolchain and build from source on Windows.

- There are **zero published GitHub releases**.
- **CI never builds a Windows binary** — all four jobs in `ci.yml` are
  `runs-on: ubuntu-latest`, and there is no release workflow.
- The repository is public with no description, no topics, Discussions off,
  and **no screenshot, GIF or video anywhere in the tree** — for a product
  whose flagship surface is an animated, voice-reactive agent.

So the question is not which channels to use. It is what has to exist before
spending a first impression, which on Hacker News and r/LocalLLaMA you get
roughly once.

---

## Gate 0 — three blockers, before anything outward-facing

### 1. There is no release pipeline

`.github/workflows/` contains only `ci.yml`. `tauri.conf.json` already sets
`bundle.active: true` with `targets: "all"`, so the bundler is configured — it
has simply never been run on Windows.

**Fix**: a `release.yml` on `windows-latest`, triggered on `v*` tags, running
`npm run build:native` and attaching the NSIS/MSI bundle to a GitHub Release.
Smallest change, largest effect.

### 2. A packaged build would expose a data-loss bug

`base_dir` is `std::env::current_dir()/.relay`, and the vault, `settings.json`
and the Whisper models all hang off it. Launched from a Start Menu shortcut
`current_dir()` is typically `C:\Windows\System32`; launched from its install
folder, `Program Files`. Neither is writable by a standard user, and the
location changes with how Relay was started.

This is `maybe_later.md` item 11, deferred correctly *while there are no
installs*. It becomes the highest-priority engineering item the moment one
ships.

**Fix**: copy the shape v0.19.1 already used for TTS (Decision 52) —
`tts::discovery::default_tts_root` resolves from OS app-data. Detect a legacy
process-relative `.relay` and offer to migrate, never move silently
(Decision 38).

### 3. Talkback ships without a voice

`resources/voice-manifest.json` ships with empty checksums by design;
`validate()` rejects an unverifiable catalogue and Relay reports "automatic
voice setup isn't available in this build" (Decision 54). That is the correct
refusal — but the shipped state is mute, and Talkback is the most demo-able
feature Relay has.

`scripts/build-voice-manifest.mjs` must run once on a networked machine. The
engine half was verified against the real GitHub artifacts; the voice half was
not, because `huggingface.co` is blocked from the development environment.

**Fix**: roughly thirty minutes on an unblocked machine. It gates the demo
recording, which gates every channel below.

### Also: decide code signing before tag time

An unsigned Tauri installer triggers the full-screen "Windows protected your
PC" SmartScreen warning. For a tool whose pitch is *your audio never leaves
your machine*, that warning is uniquely corrosive.

Azure Trusted Signing (~$10/mo, needs a verifiable organisation) is the
cheapest credible route; an OV certificate is $200–400/yr. Shipping unsigned
is survivable for an alpha **if the README says so above the fold**. It is not
survivable if users discover it themselves.

---

## Gate 1 — make it legible

All cheap, all currently undone.

- **A 30–60 second screen recording**: push-to-talk dictation into a real app,
  then asking Talkback a question out loud and getting a cited, spoken answer.
  Relay is a voice product with an animated interface; text cannot sell it.
- **Repository metadata**: description, topics (`whisper`, `tauri`,
  `local-first`, `rust`, `windows`, `ollama`, `privacy`), Discussions on,
  screenshots in the README.
- **Fix the licence detection**: GitHub reports `NOASSERTION`, so the sidebar
  never says AGPL-3.0. The AGPL text was edited — the FSF copyright line was
  replaced with "Copyright (C) 2026 Relay Maintainers", and the file is 643
  lines against upstream's ~661 — so GitHub's fingerprint match fails. Restore
  the verbatim text; put the copyright in a `NOTICE` and in source headers.
- **Reconcile the docs**: `product.md` still lists Voice Chat & TTS as
  "Deferred (Decision 34)" while the changelog shows Talkback shipping across
  v0.18.0–v0.20.0. A reader who notices concludes the documentation cannot be
  trusted.

---

## Positioning

**Lead with Windows.** Superwhisper, MacWhisper, Wispr Flow and Granola are
Mac-first or Mac-only. The README currently buries Windows under
"Requirements", as though it were a limitation. It is the wedge.

> Relay — the local-first voice assistant for Windows. Your speech becomes
> notes, tasks and meeting decisions. Your audio never leaves your machine.

Four claims, each backed by shipped code:

1. **Windows-native** — where the good tools aren't.
2. **Actually local** — Whisper + Ollama + Piper; no account needed to use it.
3. **No meeting bot** — nothing joins your call, so no consent, platform or
   legal problem.
4. **Structure, not transcripts** — decisions with their reasoning, owned
   action items, Kanban cards.

**The sleeper: Hindi–English code-switching.** Parakeet was rejected
specifically because it lacks Hindi (`talkback/RESEARCH.md` §B), and v0.19.3
shipped a Hindi voice. Almost no competitor in this category cares. Combined
with zero subscription cost and a Windows-dominant desktop base, India is a
concrete, uncontested market.

**Three claims to avoid:**

- *"AI meeting notetaker"* — crowded; Granola and Otter own the phrase.
- *"Rewind for Windows"* — always-on capture is structurally excluded on
  purpose (Decision 5). Do not invite the comparison you rejected.
- *Semantic or vector search* — retrieval is lexical. The README already had
  to be corrected once for claiming LanceDB (Decision 48); marketing copy must
  not reintroduce it.

---

## The licensing question, and its deadline

AGPL-3.0, plus "zero recurring cost", plus no commercial entity, equals no
revenue path. `maybe_later.md` item 2 flags dual-licensing as undecided. That
is fine to defer, with one hard deadline:

**If Relay might ever be dual-licensed, a CLA or DCO must be in place before
the first external contributor.** Once outside contributions land under AGPL
without one, relicensing means tracking down every contributor. Launch is
precisely the event that produces external contributors, so `CONTRIBUTING.md`
with a DCO sign-off line is a launch blocker — and a cheap one.

Three coherent postures, which change what gets built:

- **Portfolio / proof of work** — optimise for artifact quality and the
  write-up. Add a DCO anyway; no further urgency.
- **Community project** — optimise for contributors: good-first-issues,
  Discussions, fast triage.
- **Open core** — AGPL desktop core, commercial team sync. The `web/` +
  Supabase surface is already the natural commercial edge. Needs the CLA from
  day one.

---

## Channels, in priority order

Ranked by fit with what Relay is, not by reach.

| # | Channel | Why it fits | Needs first |
|---|---|---|---|
| 1 | **r/LocalLLaMA** | Runs Whisper, Ollama and Piper on their own hardware; values Windows support and AGPL. Will read the architecture docs — and will find the `heuristic_fallback` caveat (`maybe_later.md` item 5) if oversold. | Installer + GIF, honest framing |
| 2 | **Show HN** | Lead with the engineering story. The v0.19.1 changelog entry — an installer pointed at a Python wheel for weeks, caught before shipping — is a strong post in its own right. | Landing page, installer, video, 6h availability |
| 3 | **GitHub itself** | Description, topics, Discussions, screenshots, a pinned roadmap issue. Cheapest work here and entirely undone. | Nothing |
| 4 | **Awesome lists** | `awesome-tauri`, `awesome-whisper`, `awesome-selfhosted`, `awesome-local-ai`. Durable, high-intent, compounding. | A published release |
| 5 | **Tauri / r/rust** | Relay is a legitimate Tauri 2 showcase: real audio capture, 714 tests, a hard native packaging problem solved in public. | The engineering write-up |
| 6 | **Short-form video** | The orb is watchable; 30 seconds travels further than any paragraph. | Gate 0.3 — it needs a voice |
| 7 | **India / NavGurukul** | Hindi support, no subscription, Windows-dominant base. Uncontested and personally reachable. | A Hindi demo clip |
| 8 | **Product Hunt** | Sends non-technical users who churn on a pre-alpha and leave the impression Relay is flaky. | A stable product — not now |

Deliberately excluded for now: paid ads, press outreach, regulated-industry
sales. All three need a support story that does not exist.

---

## Sequence

Each phase gates the next.

| Phase | Duration | Work | Exit condition |
|---|---|---|---|
| **0 — Unblock distribution** | 2–3 weeks | `base_dir` migration, release workflow, provision the voice manifest, signing decision, DCO, LICENCE fix | A stranger on Windows downloads one file, runs it, and dictates within five minutes |
| **1 — Make it legible** | 1 week, parallel | Demo GIF and video, README rewrite, repo metadata, reconcile `product.md`, a landing page | Someone who has never heard of Relay understands it in thirty seconds |
| **2 — Quiet alpha** | 3–4 weeks | 20–50 hand-recruited Windows users; find the crash that only happens on someone else's machine, the audio device that won't enumerate, the antivirus that quarantines `piper.exe` | Ten people use it in week two without being asked |
| **3 — Public launch** | 1 week | Show HN, then r/LocalLLaMA, r/Windows, Tauri/Rust, awesome-list PRs — staggered over ~3 days, maintainer present in every thread | Honest feedback at volume |
| **4 — Compound** | ongoing | Engineering posts from the changelog; ship visibly; turn issues into a public roadmap | None — this is the durable part |

Note on the landing page: `web/` is a login-gated Supabase dashboard, not a
marketing site, and is deferred under Decision 32. GitHub Pages off `docs/` is
the faster route.

---

## What to measure

Stars are a vanity metric for a desktop app and will mislead — they measure
interest in the idea, not use of the product.

| Metric | Target | Why |
|---|---|---|
| Install → first successful dictation | > 70% | The only conversion that matters |
| Day-7 return rate | > 30% | Among alpha testers, unprompted |
| Crash-free session rate | > 95% | Windows, real hardware, real audio devices |
| Time to first word | < 5 min | Download to dictating; longer means the funnel leaks here |
| Unprompted descriptions | qualitative | How many testers describe Relay in words worth putting on the landing page |

**Explicitly not tracked**: stars, HN points, impressions, follower counts.
They move independently of whether anyone uses Relay on a Tuesday.

---

## Summary

Relay's problem is not awareness. It is that there is nothing to hand an
interested person. Build the installer, fix the path bug it would expose, give
Talkback its voice, record sixty seconds of it working — then the channels are
the easy part, because the product is genuinely differentiated and the
engineering is unusually honest.
