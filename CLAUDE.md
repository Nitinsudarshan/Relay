# CLAUDE.md — working agreement for the meeting-notes pipeline

`AGENTS.md` and `rules/` still govern everything (module layout, error handling,
design tokens, versioning). This file adds the constraints specific to the
meeting transcription and note-generation pipeline, which has invariants those
rules do not cover.

## The specification lives in `Meeting-rules/`

| File | Governs |
| --- | --- |
| `meeting_transcript_summary.md` | How a transcript becomes a summary |
| `meeting_action_items_tasks.md` | How a transcript becomes to-dos |
| `meeting_title_headings.md` | How a recording gets named |
| `meeting_speaker_identification.md` | Attribution, settings, inference rules |
| `meeting_notes_competitive_teardown.md` | Why the above look the way they do |

These are the specification, not documentation of current behaviour. When code
and a rules file disagree, the rules file is right and the code is a bug.

**Prompt text is loaded from these files at runtime. It is never copied into
Rust or TypeScript source.** A prompt duplicated into source drifts from the
file silently, and a drift nobody can see is the worst kind. If a prompt needs
assembly (budgeting, injected metadata), assemble it *around* the file's text —
do not paraphrase the file.

## Pipeline invariants

1. **Deterministic work belongs in Rust, not in a prompt.** Stripping tags,
   collapsing decoder loops, removing filler, resolving dates, checking output
   shape: all of it is code. Every token spent asking an 8B model to do a
   regex's job is a token not spent on comprehension, done less reliably.

2. **ASR output is immutable.** `text_raw` is written once, by the transcriber,
   and never edited. Normalization and attribution are *separate layers keyed
   to segment ranges*, so either can be re-run, corrected, or thrown away
   without touching the source. Anything that rewrites transcript text in place
   is a bug, however convenient.

3. **Attribution is a layer, not a column on the segment.** This is what makes
   re-running diarization safe and what lets an `Unassigned` to-do re-own
   itself when the user later names a cluster.

4. **Every generated claim carries an evidence span** (`start_ms`, `end_ms`).
   It powers click-to-seek, and it is the strongest hallucination guard
   available: a model that must cite a timestamp cannot invent a task.

5. **Diarization and identification are different features.** Clustering
   voices is automatic and produces `Speaker 2`. Attaching a name is a human
   act, once, that then persists. The UI must be able to show "we know there
   were four people and we do not know who they are" honestly.

6. **Comprehension precedes structure.** Summarization is two calls: Stage A
   builds a topic inventory, Stage B writes from the inventory with the
   transcript out of context. One call lets a small model collapse the two and
   revert to copying, which is the failure the whole pipeline exists to fix.

7. **Never guess an owner, a name, or a number.** Abstaining is a correct
   answer everywhere in this pipeline. `Unassigned` beats a wrong owner;
   omitting a figure beats a rounded one.

## Local-first constraints

- **No network calls in the default path.** Cloud models are opt-in and must
  never be required for a core feature.
- **No Python runtime**, which rules out pyannote. Diarization uses
  `sherpa-onnx` (Rust bindings, ships Windows binaries).
- **Windows is the primary target.** Feature-detect anything platform-specific
  and degrade gracefully; never assume macOS.
- **Biometric data is gated.** A persistent voiceprint is regulated data
  (Illinois BIPA; GDPR Art. 9). Speaker enrollment does not ship before the
  consent and deletion UI exists — see `meeting_speaker_identification.md` §6.

## Model output

- Models emit JSON validated against a schema. Rendering is the frontend's job.
- Validation is not advisory: the self-check lists in the rules files are
  implemented as code that runs before anything reaches the UI. On failure,
  retry once naming the specific violation; on a second failure, degrade
  visibly rather than shipping invalid output.
- A fallback that copies transcript sentences is not a fallback. It is the
  failure being shipped under another name. If generation fails, say so.

## Commands

```bash
# Rust backend (from native/src-tauri)
cargo test --lib                 # unit tests
cargo clippy --lib -- -D warnings

# Frontend (from native)
npx tsc --noEmit -p tsconfig.json
npm run build
```

On a Linux dev box, Tauri needs `libgtk-3-dev libwebkit2gtk-4.1-dev
libsoup-3.0-dev librsvg2-dev libayatana-appindicator3-dev libasound2-dev
libxdo-dev libclang-dev`, and `CMAKE`/`LIBCLANG_PATH` must be overridden in the
environment because `.cargo/config.toml` pins Windows paths.

## Working style for this pipeline

- One phase at a time, tests and a commit per phase.
- Do not tune prompts before the normalizer, decode config, and channel split
  have landed — that is tuning against noise which is about to disappear.
- Fixtures live in `native/src-tauri/tests/fixtures/meetings/`. A quality claim
  without a fixture behind it is an opinion.
