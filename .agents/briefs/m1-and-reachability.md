# Brief — Finishing M1, and the Reachability Audit It Turned Into

**Branch** `claude/relay-meeting-transcript-summary-kgypzj` · **Base** `3787834` · **Head** `88e4450`
**Date** 8 Sept 2026 · 12 commits · 44 files · +4,990 / −461

---

## Why this work happened

The task was one line: finish M1, the migration of the meeting pipeline onto
`pipeline::analysis`. That took one commit.

The other eleven happened because of what finishing it revealed. M1's last
blocker was a *number* — `prompt_budget_chars`, a method on `MeetingLlm` with no
equivalent on the shared service. Chasing it meant reading the settings, the
prompt registry and the capture surfaces closely enough to notice a pattern that
turned out to be the real story of this branch.

## The core finding

**Relay's recurring defect is not missing mechanisms. It is finished mechanisms
nothing can reach.**

The previous brief found one instance and called it a bug: `output_script` was
"parsed, persisted, round-trip tested, shown in the UI with a description of
behaviour that did not exist, and read by nothing." That was not an isolated
case. This branch found seven more:

| Setting or capability | State when found |
|---|---|
| `SttSettings::preset` | Honoured on the real decode path; no UI control existed |
| `ggml-large-v3-turbo` | `ensure_accurate_model` complete, zero callers, listed as "missing" for a UI offer nobody built |
| `selected_device` | Three capture surfaces called `default_input_device()` directly |
| `prefer_builtin_mic` | Switch persisted, read by nothing |
| `auto_learn_words` | Toggle on by default, promising behaviour that cannot be built |
| `textTransform` / `cleanupStyle` | Local React state in the pill, passed to a popover, sent nowhere |
| `text_transform` | **Introduced by this branch**, unenforced for four commits |

The last row is the honest one. Knowing about the pattern did not stop me
reproducing it.

---

## What shipped

| Commit | What |
|---|---|
| `aad2848` | M1 — both meeting stages on `AnalysisService`; `MeetingLlm`/`ProviderLlm` gone |
| `dcd916c` | M6 and M7 recorded as Decisions 65 and 66 |
| `be8a36f` | Decode preset control; `large-v3-turbo` downloadable |
| `31b9cd7` | Meetings may name their own Whisper model |
| `522eba8` | T5 — Talkback's prompt registered; its context budget bounded |
| `9af1f06` | T2 — hedged generation, on providers where it can help |
| `91ee3fb` | Decode history, so the audio decisions become measurable |
| `3f1cbcc` | D1 engine — Tier 2 rewrite and the diff that makes it reviewable |
| `b56a519` | D1 surface — cleanup as an explicit action after injection |
| `d461b17` | `text_transform` enforced at the command, not only in the pill |
| `5876324` | Microphone selection honoured; `auto_learn_words` removed |
| `88e4450` | Voice Note phrase corrections and learned vocabulary |

### New modules

- **`capture::decode_history`** — the last 500 decodes, counts only. Never the
  transcript and never the initial prompt: the first is the user's speech, the
  second carries their colleagues' names. A test serializes a record and
  searches for both, which is what lets this file be attached to a bug report.
- **`capture::device`** — one resolver for which microphone opens, shared by
  dictation, meetings and Talkback. Pure over device names, so the policy is
  tested without an audio host.
- **`capture::rewrite`** — Tier 2 cleanup and a word-level diff with a
  round-trip guarantee.
- **`talkback::hedge`** — a duplicate request when the first stalls, and the
  winner-claim that keeps two streams from interleaving into one sentence.
- **`vault::correction`** — the deterministic range edit behind Voice Note
  phrase corrections.

### Key design decisions

- **The budget is computed per prompt, not per pipeline.** `MeetingLlm`'s
  constant reserved 2,400 output tokens for everything.
  `AnalysisService::prompt_budget_chars` reserves what *that prompt* may answer
  with. A test pins the meeting figure to exactly the old number, because a
  migration that silently re-chunks every long meeting is not a migration.
- **`Completer` grew a fourth method, and the doc grew a rule.** It said
  "deliberately three methods… not model names". `model_name` is the provenance
  kind rather than the deciding kind — the exact analogue of `provider_type` —
  so the doc now states *what kind* of method belongs instead of counting them.
- **Hedging is cloud-only on the merits.** Against local Ollama a duplicate
  queues behind the request it was meant to overtake, shares one CPU if it does
  not, and waits on the same cold model load either way. Not caution — no path
  to working.
- **The dictation cleanup happens after injection, not before.** Cleaning up
  first put a model call in the hot path and turned ~0.8s dictation into ~3s+,
  including for the utterances that needed nothing.
- **Learning is opt-in per correction, and the checkbox is never pre-ticked.**
  "Thursday" → "Tuesday" is an ordinary edit; one wrongly pre-ticked default
  puts junk in the dictionary of a user who did not read carefully.

---

## Corrections to claims made during this work

Recorded because they were stated confidently before being checked.

1. **"Eight settings are read by nothing."** Seven.
   `keep_microphone_warm` was wired all along, through
   `parse_keep_warm_duration` — an accessor that reads the field *inside*
   `settings/mod.rs`. The scan looked for `.field` references outside that file,
   so every accessor-based read was invisible to it. The scan was the bug, not
   the code.
2. **"Ten commits on the branch."** Nine, at the time it was said.
3. **"The Hinglish lexicons in `qualify.rs` will still drop every action
   item."** Already fixed by `ca231af` on the base branch; the file carries 90
   Devanagari runs.
4. **"Language detection is still starving the transcript."** Overstated.
   `355c18b` resolves language per window class, and a profile declaring more
   than one language auto-detects on meetings. What remains is narrower: the
   default `en`-only profile still pins.
5. **`maybe_later` item 12 said "no calendar integration exists".** It shipped —
   accounts, event matching and `ParticipantOrigin::Invited` participants. What
   is actually missing is §2.2's expected-speaker hint.

---

## Bugs found that were not on the plan

- **Talkback could lose its own grounding rules.** Its retrieval budget was
  bounded by what the window *buys* and what a spoken turn can *use* — never by
  what *fits*. At the 2,048-token floor those agreed on 2,580 characters where
  1,344 fit. Ollama truncates from the front, which is where the voice rules and
  the grounding instruction live, so the turn kept its evidence and lost the
  instruction to stay grounded in it.
- **The JSON gate, briefly.** Widening `parse_json_response` to rescue
  prose-wrapped JSON also made it dig an object out of `[{…}]` — precisely the
  filler case the gate exists to refuse. Caught by an existing test within
  minutes of writing it; the rescue now runs only on text that is not JSON at
  all.
- **The diff round-trip.** Matching words on their trimmed form emitted `Same`
  carrying only the original's whitespace, so a respaced sentence failed to
  reproduce. The property test caught it on its first run.
- **`a_small_window_keeps_its_own_figure` asserted the overrunning figure**
  while its own comment said the ceiling "is a maximum, never a floor that would
  overrun their model's context". Right about the rule, wrong about the number.
- **The vocabulary heuristic matched "Thursday" → "Tuesday"** — the exact
  counter-example the Voice Note brief names. The signal that separates it from
  "tarui" → "Tauri" is that the original is *already correctly capitalised*: a
  real word being swapped, not a misheard proper noun.
- **`auto_learn_words` could not be built as described.** "When you correct a
  transcription in the target app" means reading edits inside another
  application — accessibility APIs Relay uses nowhere, and the injection layer
  is built the other way round: it writes into a focused field and cannot read
  one back. Removed, with the reason recorded as `maybe_later` item 13.

---

## Verification

```
cargo test                                  1415 passed, 4 ignored
cargo clippy --all-targets -- -D warnings   clean
npx tsc --noEmit                            clean
npm test                                    521 passed, 42 files
npm run verify:rules                        clean
```

Baseline at the branch point was 1,337 Rust and 502 frontend.

**One pre-existing failure**, verified by stashing rather than assumed:
`actions::dispatcher::test_read_only_action_executes_without_confirmation`
shells out to open a URL and fails on any headless host. It fails identically on
the unmodified tree.

### What has not been verified

Everything below is tested as logic and has never run against real hardware or a
real application:

- **Microphone selection.** Whether cpal's enumerated names match what the
  picker shows on a real Windows machine is unknown.
- **The dictation cleanup replacement.** `cursor_steps` and `focus_unchanged`
  are tested; `enigo` driving an actual focused field is not. The failure mode
  if `cursor_steps` is wrong somewhere unanticipated is over-selection, which
  destroys text. Worth trying by hand in a browser field, a native app, and one
  Devanagari case before trusting it.
- **Hedged generation**, which needs a cloud provider and a slow one.

---

## What is left

### Decided, not yet built

- **ONNX adopted.** G6 denoise and T4 smart turn detection are cleared to
  proceed. Two substantial features plus an `ort` dependency and a
  model-distribution decision — the README's no-bundled-models claim needs
  amending, or the models download on demand the way `large-v3-turbo` now does.
  Windows packaging cannot be validated from this container.
- **Unify chars-per-token on 3.** Four constants, two values: 3 in the analysis
  spine, 3.6 in `talkback/assemble.rs` and `context/pack.rs`. The failure modes
  are asymmetric — too optimistic overflows silently, too conservative wastes a
  little window — and Devanagari tokenizes far denser than English, so 3.6 is
  especially wrong for the content Relay actually handles.
- **`launch_at_login` and `start_minimized`**, which need real backend work
  (an autostart plugin decision, and window state). Audit `dictation_sounds`
  and `show_raw_transcript` first — those may be legitimately frontend-only.

### Now measurable, previously blocked

`91ee3fb` exists because both of these were listed as blocked on "measure
first" while nothing retained the measurements.

- **G6's AGC half.** Diagnostics now reports peak and RMS at p10 across the last
  500 decodes, and counts how many never reached a quarter of full scale. If
  that count is near zero, a gain stage is risk without benefit.
- **The parallel LID detector.** The decode summary splits median words-per-
  second by whether the language was pinned or auto-detected. A pinned figure
  well below the auto-detected one is the evidence that would justify building
  it — and words-per-second is the measure that exposes it, because a decode
  pinned to the wrong language does not error, it returns less.

### Still blocked

- **The expected-speaker hint** (`maybe_later` item 12's remaining half).
- **D1's own follow-on**: an accepted rewrite is a correction Relay could learn
  from without reading anybody's window. A real feature, and a different one
  from the setting that was removed.

---

## Environment notes for a fresh session

```bash
apt-get update -qq && apt-get install --no-install-recommends -y \
  libwebkit2gtk-4.1-dev libjavascriptcoregtk-4.1-dev libsoup-3.0-dev \
  libgtk-3-dev libayatana-appindicator3-dev librsvg2-dev \
  libasound2-dev libxdo-dev libclang-dev build-essential cmake

cd native && npm ci
```

Cold `cargo test --no-run` dominates the first run (whisper.cpp).
`git log --oneline 3787834..HEAD` is the work in this brief.

## Reference

- `docs/decisions.md` — Decisions 65 (a Tier 2 rewrite may never touch a meeting
  transcript) and 66 (the summarizer stays two-stage).
- `maybe_later.md` — item 12 corrected, item 13 added.
- The previous brief, `voice-pipeline-unification.md`, whose "Ready to build"
  section this branch worked through.
