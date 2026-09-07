# Brief — Voice Pipeline Unification

**Branch** `claude/relay-meetings-review-y3dysa` · **Base** `ff03e72` · **Head** `ca231af`
**Date** 7 Sept 2026 · 7 commits · 31 files · +3,694 / −695

---

## Why this work happened

A five-minute Hindi/Hinglish standup produced a two-sentence summary
("The discussion focused on the Volunteer Project"), while Granola produced a
full structured record of the same meeting with names, numbers, decisions and
owners. The summary looked like the bug. It was not.

The transcript header said **56 words for 4m55s** — about 0.2 words/second,
where conversation runs 2–4. Everything downstream then behaved *correctly* on
a broken input: the topic band for 56 words is 1–2 topics, so one topic passed
validation; the thinness floor is 14 words, so a 20-word summary passed. The
prompts were never the problem.

## The core finding

Relay already contained the hallucination screen, glossary priming and
transcript normalization that good speech capture needs. **All of it sat behind
one of five paths that decode audio.**

Two transcriber implementations existed. `SttEngine` took a
`WhisperDecodingConfig` and honoured it. `StreamingTranscriber` accepted no
config at all — it hardcoded its parameters inside `transcribe()` and clamped
the encoder context — and Talkback and the live meeting clock both used it.

---

## What shipped

| Commit | What |
|---|---|
| `355c18b` | STT language resolved per window class; Talkback turn context capped; first Devanagari fixtures |
| `061d35c` | One configured decode path for every surface; speech screen moved below meetings; per-decode observability |
| `7866793` | Per-surface decode policy; deterministic text cleanup reaches dictation |
| `7a11966` | Devanagari→Latin romanization at the projection layer; language threaded into prompts; `large-v3-turbo` tier |
| `69f3917` | Staged, computed-prompt analyses modelled on the shared spine; §4.3 contradiction settled |
| `af91877` | `providers::Completer` — the completion seam that unblocks the meetings migration |
| `ca231af` | Hindi/Hinglish action-item gates; the virama tokenizer bug; injection recovery guarantee |

### New modules

- **`capture::romanize`** — Devanagari→Latin, deterministic, no dependency.
  Written rather than taken from a crate: `any_ascii` and `deunicode` are
  per-codepoint tables, and Devanagari needs state (a consonant's inherent *a*
  is replaced by a following vowel sign or suppressed by a virama). A stateless
  table renders क्या as "kaya"; the word is "kya".
- **`capture::text_normalize`** — the deterministic cleanup rules, promoted out
  of `meetings_v2` so dictation can use them.
- **`capture::speech_health`** — the speech gate and hallucination screen, moved
  down from `meetings_v2` (which depends on `capture`, so the dependency ran
  backwards). Re-exported under the old name; all 40 call sites unchanged.

### Key design decisions

- **Mechanism in one wave, policy in the next.** Presets, `audio_ctx` and
  `single_segment` became configurable before any value changed, so the
  refactor was reviewable separately from the behaviour change.
- **One preset setting, per-surface defaults.** `SttSettings::preset` empty
  means "let each surface choose" — dictation defaults Fast (latency-bound),
  meetings Quality (recall-bound). A stated preference wins everywhere.
- **Romanization is a projection, never a write.** Applied at the command
  boundary by walking the serialized payload, so `transcript.jsonl` stays
  byte-identical and a field added later is projected too. Safe because the
  transform only touches Devanagari — ids and paths are ASCII by construction.
- **`TextProfile::Dictated` vs `::Transcript`.** The one rule that genuinely
  differs by surface is the one that *adds*: sentence repair appends a period,
  right for a stored segment and wrong for text entering a focused field.

---

## Corrections to earlier claims in this work

Recorded because the artifacts and early commits carried them.

1. **The live meeting clock already screened.** `live_stt.rs` profiles the audio
   and calls `assess`, using the same `mean_no_speech_prob: 0.0` approach later
   written for Talkback. What it lacked was the vocabulary, the three
   thresholds and a visible configuration. **Talkback was the only genuinely
   unscreened path.**
2. **Talkback's latency was never the timeout.** The answer path already had
   `timeout_secs: 90`, `max_output_tokens: 400` and streaming, and both the
   router and retrieval are deterministic. The cost was the *context budget*:
   ~10,300 characters of retrieval at the default 8192-token window, and on a
   local model prompt ingestion dominates time-to-first-token. Capped at 3,600.
3. **M4 was not a bug.** `LIVE_AUDIO_CTX` (768 frames ≈ 15.4s) covers the
   12-second live utterance cap, so it never truncates, and `single_segment`
   there prevents whisper discarding a sub-2-second window. Both correct. What
   was missing was any statement of the clamp/cap relationship — now a test.
4. **"No Devanagari anywhere" needed a qualifier.** Romanized Hinglish coverage
   existed; Devanagari coverage did not. The blind spot was script-shaped.
5. **Injection was not silently aborting.** Every non-Success outcome already
   sent a toast *and* a `FOCUS_CHANGED` status. A real bug was found underneath:
   those branches promised the text was on the clipboard, and it was not when
   copying was off *and* the method was Keystrokes.

---

## Bugs found that were not on the plan

- **The virama tokenizer bug.** `qualify::words` split on anything
  `char::is_alphanumeric` rejects. Unicode's `Alphabetic` covers Devanagari
  vowel signs but *not* the virama, which is word-internal — so `फॉर्म` became
  `फॉर` + `म`, and consonant clusters are most Hindi words. No Hindi vocabulary
  could ever have matched. Fixed via
  `capture::text_normalize::is_word_internal`.
- **`output_script` was a dead setting.** Parsed, persisted, round-trip tested,
  shown in the UI with a description of behaviour that did not exist, and read
  by nothing.
- **`notes_language` likewise** — never threaded into any prompt.

---

## Verification

```
cargo test                                  1337 passed, 4 ignored
cargo clippy --all-targets -- -D warnings   clean
npx tsc --noEmit                            clean (pinned TS 5.6 — run npm ci first)
npm test                                    502 passed, 41 files
npm run verify:rules                        clean
```

**Two pre-existing failures, neither from this work:**

- `actions::dispatcher::test_read_only_action_executes_without_confirmation`
  shells out to open a URL; fails on any headless host. Verified by stashing.
- `tts::installer::the_pinned_engine_installs_and_speaks` (`#[ignore]`d)
  downloads and runs a TTS engine; needs network.

---

## What is left

### Ready to build

**Finish M1 — migrate meetings onto the shared spine.** Nothing structural is
in the way now. The one remaining piece is named in both TODOs: `MeetingLlm`
carries `prompt_budget_chars`, which extraction uses to decide how many passes
a long transcript needs, and the shared service has no equivalent. Then swap
stage by stage, keeping the repair loop and deterministic floor in
`meetings_v2::processing`, and point `ScriptedLlm` at the new seam rather than
deleting its coverage.

**T5 — Talkback's `assemble.rs` onto the same spine.** Smaller version of M1,
with its own budget and prompt kind.

**T2 — hedged requests.** Fire a duplicate when the first is slow (~1.8s, above
p50 so the common turn never pays twice). From `bolna`. Independent of
everything else.

**D1 — the rewrite layer as a reviewable diff.** From `prismical`. The real
objection to an LLM rewrite is not that it might be wrong but that a silent
rewrite gives no way to notice; a diff removes the objection rather than
hedging against it. Substantial frontend work, deserves its own change.

### Needs a decision, not more code

**G6 denoise and T4 smart turn detection** both require an ONNX model plus a
runtime in the Tauri binary. That is a heavyweight new dependency and a bundled
model in an app whose README states it does not bundle models. Not a call to
make unilaterally.

**G6's AGC half** needs no dependency but changes recorded audio, which is the
source of truth. Measure first: Diagnostics already reports RMS and peak. If
input levels are fine, a gain stage is risk without benefit.

**The parallel LID detector** (from `bolna`) remains the right long-term answer
to the language problem — a language-locked recognizer plus a separate detector
on the same audio, at zero added ASR latency. It belongs after the
observability from `061d35c` can show whether it beats window-class pinning.

### Unserved by the field

No repository in the 15-repo reference scan romanizes anything. `capture::romanize`
has no upstream to track and two recorded limitations:

- An unmarked loanword reads as the letter it was spelled with (`फॉर्म` →
  "phorm", because फ genuinely is *ph*). Guessing that a candra-o marks a
  loanword would mis-romanize every native word using one.
- Medial schwas are kept (`इंटरव्यू` → "intaravyu"). The obvious deletion rule
  turns `सप्ताह` into "sptaah".

---

## Environment notes for a fresh session

This container needs setup before Rust will build:

```bash
apt-get update -qq && apt-get install --no-install-recommends -y \
  libwebkit2gtk-4.1-dev libjavascriptcoregtk-4.1-dev libsoup-3.0-dev \
  libgtk-3-dev libayatana-appindicator3-dev librsvg2-dev \
  libasound2-dev libxdo-dev libclang-dev build-essential cmake

cd native && npm ci     # else npx fetches a floating TypeScript and tsconfig warns
```

Cold `cargo test --no-run` dominates the first run (whisper.cpp).
`git log --oneline ff03e72..HEAD` is the work in this brief.

## Reference

Three artifacts, kept current with the corrections above:

- Voice Pipeline Audit — the five-path matrix and the original diagnosis
- What Relay Should Steal — 15 repos read against Relay's defects
- Relay Build Order — 29 items by surface, with shipped state
