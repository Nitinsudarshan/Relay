# Prompt — Relay, next session

Continue work on `claude/relay-meeting-transcript-summary-kgypzj` (base:
`claude/relay-meetings-review-y3dysa`, head `22822b8`).

**Read `.agents/briefs/m1-and-reachability.md` first.** It records what the last
session shipped, what it got wrong, and — importantly — what has *not* been
verified. Do not re-derive it.

---

## Context you need up front

Thirteen commits are stacked here and nothing is merged. The last session
finished M1 (meetings on `pipeline::analysis`), then spent most of its time on a
pattern it found while doing so: **Relay repeatedly ships finished mechanisms
that nothing can reach** — a setting parsed and persisted and read by nobody, an
engine with no caller, a UI control wired to local state. Eight instances were
found and fixed. Assume there are more, and treat "is anything actually reading
this?" as a standing question rather than a one-off audit.

The single most important caveat: **none of the audio, injection or device code
has ever run on real hardware.** It is all tested as logic.

---

## Task 1 — chars-per-token (do this first; it is small and safe)

Four constants, two values: `3` in `pipeline/analysis/service.rs`, `3.6` in
`talkback/assemble.rs` and `context/pack.rs` (plus a test-only copy in
`meetings_v2/processing/llm.rs`).

Unify on **3**. The failure modes are asymmetric — too optimistic overflows the
window and Ollama truncates *from the front*, silently; too conservative just
wastes a little window. Devanagari also tokenizes far denser than English, so
3.6 is especially wrong for the content Relay handles.

Put the constant in one place, have the others use it, and leave a comment
saying the decode history can now measure the real ratio if anyone wants to tune
it. Expect `context/pack.rs` behaviour to change slightly — that is the point.

## Task 2 — audit the last four unread settings

`launch_at_login`, `start_minimized`, `dictation_sounds`, `show_raw_transcript`
have no Rust reader.

**Check before assuming.** The last session's first scan was wrong: it looked
for `.field` outside `settings/mod.rs` and so missed `keep_microphone_warm`,
which is read by an accessor *inside* that file and was wired all along. Search
for the field name everywhere, then check whether any accessor that reads it has
external callers.

Then:
- `dictation_sounds` and `show_raw_transcript` may be legitimately frontend-only.
  If the frontend genuinely acts on them, leave them and say so.
- `launch_at_login` and `start_minimized` cannot work from the frontend. Wire
  them (autostart plugin for the first, window state for the second) or remove
  them and record why in `maybe_later.md`, following the `auto_learn_words`
  precedent in item 13.

## Task 3 — ONNX packaging spike (blocked on Windows; write it, do not run it)

G6 denoise and T4 smart turn detection are approved to proceed, but the first
move is proving `ort` builds, bundles and runs in a real Windows Tauri binary.
That cannot be done in this container.

Write the spike so the user can run it, and stop there. Do **not** start either
feature until it passes. When it does, settle whether models download on demand
(the way `large-v3-turbo` now does) or the README's no-bundled-models claim gets
amended — that is a product decision, not yours.

---

## Not your tasks — these need the user's Windows machine

Do not attempt these, and do not claim them as done. Report them as outstanding
if asked.

1. **Verify the dictation cleanup replacement.** Highest risk in the branch. If
   `capture::text_normalize::cursor_steps` is wrong somewhere unanticipated, the
   selection over-runs and deletes text the user wrote before dictating. Needs
   hand-testing in a browser field, a native app, and one Devanagari case.
2. **Verify microphone selection** — that cpal's enumerated names match the
   picker and that choosing one changes what records.
3. **Re-test the original bug.** A Hinglish standup, with Hindi added to
   *Languages I Speak* and `large-v3-turbo` set as the meetings model, compared
   against the Granola output that started this work. This is the acceptance
   test for the whole branch.
4. **Read the decode history after ~50 real decodes** (Diagnostics → Decode
   History). Two numbers close two open decisions: *quiet runs* near zero means
   G6's AGC is not worth building; a *pinned* words/sec median well below the
   *auto-detected* one is what would justify the LID detector.

---

## Invariants — do not break these

- **No audio is ever persisted.** No `source_audio` on new notes, no
  `.relay/audio/`, no WAV/MP3/OGG, no audio ids on any record. Existing TTS
  fields are unrelated and must not be repurposed.
- **`transcript.jsonl` is immutable.** Normalization, summarization, speaker
  renames and regeneration all leave it byte-identical. Romanization is a
  projection applied at the command boundary, never a write.
- **A Tier 2 rewrite may never touch a meeting transcript** — `docs/decisions.md`
  Decision 65.
- **The summarizer stays two-stage.** No map-reduce over transcript chunks —
  Decision 66.
- **No external-app learning.** No accessibility monitoring, clipboard watching
  or keystroke interception. Relay learns only from explicit corrections made
  inside it.
- **Local-first.** No cloud correction services, telemetry or remote databases.
- **Do not add a second persistence layer.** Notes go through
  `vault::update_note_content`; settings through `save_settings`.

---

## Verification — all five must pass before you commit

```bash
cd native/src-tauri && cargo test                 # 1415 passed, 4 ignored
cd native/src-tauri && cargo clippy --all-targets -- -D warnings
cd native && npx tsc --noEmit
cd native && npm test                             # 521 passed, 42 files
npm run verify:rules                              # from the repo root
```

**One test fails and always has**:
`actions::dispatcher::test_read_only_action_executes_without_confirmation`
shells out to open a URL and cannot on a headless host. Verified pre-existing by
stashing. Do not try to fix it; do not count it as a regression.

If a test you wrote fails, read it before changing it — three tests in the last
session caught genuine bugs on their first run, and in two more cases the
*assertion* was wrong while the code was right. Work out which before editing
either.

## Environment

```bash
apt-get update -qq && apt-get install --no-install-recommends -y \
  libwebkit2gtk-4.1-dev libjavascriptcoregtk-4.1-dev libsoup-3.0-dev \
  libgtk-3-dev libayatana-appindicator3-dev librsvg2-dev \
  libasound2-dev libxdo-dev libclang-dev build-essential cmake

cd native && npm ci
```

Cold `cargo test --no-run` dominates the first run (whisper.cpp). Start it in the
background before reading code.

## Working notes

- Commit each task separately with a message that says *why*, not just what.
- Push to `claude/relay-meeting-transcript-summary-kgypzj`. Do not open a PR
  unless asked.
- If you find another unreachable mechanism, fix it and say so — that is the
  through-line of this branch, not a distraction from it.
- Say plainly what you did not verify. The last brief's most useful section is
  the one listing what has never run.
