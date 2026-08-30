# Relay meeting summarization — end-to-end quality rebuild

Audit of the pipeline as it stood at v0.15.1, the causes of poor summary quality
found in it, and the pipeline that replaces it.

Scope: `native/` only. Nothing here changes recording, chunking, the two audio
clocks, pause/resume, crash recovery, dictation, or voice notes.

---

## A. The pipeline as it was

Traced by execution path, not by filename.

```text
capture.rs          mic + system audio, 30-second durable chunks,
                    per-second channel energy track
      ↓
worker.rs           Whisper per chunk → utterance spans with channel flags
      ↓
session_store.rs    transcript.jsonl — append-only, never rewritten
      ↓
[prepare]           normalize.rs → speakers.rs → conversation.rs
                    deterministic; no model, no network
      ↓
[generate_summary]
   Stage A  extract.rs     one LLM call, whole transcript in the user message,
                           strict-JSON contract → sanitize → qualify → MeetingFacts
   Stage B  summarize.rs   one LLM call, MeetingFacts only, never the transcript
      ↓
   validate.rs             on any Error: discard the model's prose,
                           re-render deterministically from the same facts
      ↓
store.rs            processing.json + processing_log.jsonl (derived only)
      ↓
MeetingSummaryTab.tsx
```

Much of this was already right, and stayed. Two stages rather than one, so no
model has to comprehend and write at the same time. A canonical `MeetingFacts`
every view projects from, so the summary and the action-item list cannot
disagree. Provenance on every claim. A deterministic floor under the whole
feature. Raw immutability enforced by construction rather than by convention.

The quality problem was not in that shape. It was in five specific places.

---

## B. What was actually wrong

### B1. The provider silently truncated every long meeting

`providers/mod.rs::complete_ollama` posted `{model, prompt, stream: false}` with
**no `options` object**. Ollama's default context window is 4096 tokens (2048 on
older builds) and it discards what does not fit — from the front.

Stage A's user message is the entire rendered transcript. A 45-minute meeting is
roughly 7,000 words, about 10,000 tokens, plus a ~900-token system prompt. So on
any meeting past roughly a quarter of an hour, **the model never saw the
beginning** — which is where the agenda and the framing decisions are. Nothing in
Rust truncated anything, nothing logged it, and `input_chars` recorded the full
size, so the processing log positively asserted that the whole transcript had
been sent.

This was the single largest cause of "the summary missed what mattered", and no
prompt change could have touched it.

### B2. Extraction ran at a creative-writing temperature

The same call set no temperature. Ollama's default is 0.8.
`Meeting-rules/meeting_transcript_summary.md` §11 specifies 0.1 for Stage A and
0.3 for Stage B. Stage A is a strict-JSON read of a transcript whose failure mode
is confidently invented ownership, and it was running at 0.8.

`MeetingLlm::complete` had no parameter through which either stage could say what
sampling it needed, so this was not a misconfiguration — it was unexpressible.

### B3. The word cap rejected correct summaries

`SummaryMode::max_words()` was a fixed cap (220 / 550 / 1100) enforced by the
validator as an **Error**, and an Error discarded the model's prose and replaced
it with the deterministic renderer.

A ninety-minute meeting legitimately needs more than 550 words in Standard mode.
The consequence chain was: good summary → `SUMMARY_TOO_LONG` → prose discarded →
user gets a bullet dump. The same number was simultaneously far too generous for
a four-minute call, where 550 words of "summary" is padding and nothing flagged
it. Length was never adapted to the meeting, and the model was never *told* the
budget — only punished for missing it.

### B4. The facts schema had nowhere to put the most valuable information

`MeetingFacts` held title, type, key points, topics, decisions
(`statement` + `decided_by`), action items, open questions, entities.

Stage B reads the facts and nothing else — correctly, since that is what stops
the summary becoming a reshuffled transcript. But it also means **anything absent
from the schema is unreachable by the summary, whatever the prompt says**.
Missing:

- *why* a decision was made,
- risks, blockers, dependencies, constraints,
- any distinction between a proposal, a recommendation, and a decision.

"We're moving the launch to Monday because the payment integration still has
three blocking bugs" could only ever come out as "Ship on Monday." A prompt
asking for rationale was asking for something the data could not carry.

### B5. A validation failure had no repair path

`mod.rs` step 7: model prose with any Error was thrown away and replaced by
`render_markdown()`. There was no second call and no feedback. One fixable
slip — a code fence, an opening "Here is the summary", forty words over — cost
the user the entire model-written summary and downgraded them to a fact dump.
`ProviderOutputStatus::Rejected` existed precisely to record how often this
happened.

### Also found

- **No user notes anywhere.** No field on `MeetingSession`, `MeetingProcessing`,
  the overlay, or the view. The repo's own gap analysis lists this as gap 8 and
  calls it "the cheapest quality multiplier available".
- **`CloudGemini` and `CloudAnthropic` both posted to `api.openai.com`** with an
  OpenAI body and an OpenAI auth header. Selecting either sent the meeting to a
  service the user did not choose, failed authentication, and fell through to
  canned filler — which is why those providers looked like they summarized badly
  rather than not at all.
- **No request timeout.** A stalled local model left the UI on "Generating…"
  indefinitely.
- **Documented rules the code did not implement.**
  `meeting_transcript_summary.md` specifies an `## Overview` / `## Discussion` /
  `## Decisions` / `## Risks & Open Questions` / `## Next Steps` structure (§5), a
  duration-based length table (§6), chunking for long transcripts (§8), and
  per-stage temperatures (§11). None of §6, §8, or §11 was implemented, and Stage
  B emitted a different structure from §5.
- **Key points reached Stage B as a flat list**, with the topic grouping already
  discarded, so it could not organize by topic even where the mode asked it to.

### What was already good, and was kept

Two-stage separation. Segment-id provenance on every claim. The action-item
qualification gate in `qualify.rs`, which is genuinely strict and stays exactly
as it was. Owner resolution that refuses to promote an unmatched name to a
speaker. `sanitize_deadline`, which keeps a date only when a cited segment
actually contains a temporal expression. Raw immutability. The deterministic
floor.

---

## C. The pipeline now

```text
transcript.jsonl (raw)              notes.json (raw, user-authored)
        ↓                                    ↓
   normalize → attribute → converse          │
        ↓                                    │
        └──────────► context.rs ◄────────────┘
                   CANONICAL MEETING CONTEXT
             metadata · participants · glossary
             · notes · transcript, windowed to fit
                          ↓
              Stage A — extract.rs (temp 0.1)
              one pass per window, merged deterministically
                          ↓
                    MeetingFacts
        + rationale · point kind · risks
                          ↓
              length.rs → this meeting's budget
                          ↓
              Stage B — summarize.rs (temp 0.3)
              summary contract + budget + notes-as-emphasis
                          ↓
                    validate.rs
                    ┌─────┴─────┐
                  PASS         FAIL
                    │            ↓
                    │     repair_feedback() names the broken rule
                    │            ↓
                    │     one corrected regeneration
                    │            ↓
                    │        validate ──┬── PASS ──┐
                    │                   └── FAIL ──┤
                    │                              ↓
                    │                   deterministic render
                    ↓                              ↓
                 persist ◄─────────────────────────┘
```

### C1. Canonical meeting context — `processing/context.rs` (new)

One place decides what a model is told about a meeting. Before, Stage A received
a rendered transcript and three loose strings interpolated into a format literal,
and there was nowhere to put anything else.

Two rules hold it honest:

- **Absent is absent.** An optional block with no content is not rendered — not
  as an empty heading, not as the word "None". A model shown
  `# Pre-Meeting Notes\n\nNone.` writes a summary that mentions their absence,
  and a user who never writes notes would see that on every meeting.
- **Nothing is included because it exists.** Fields are there because they change
  how the meeting reads.

It also owns **windowing**: `windows(budget_chars)` splits the transcript into
stretches that fit the model, with a two-segment overlap. One window is returned
whenever the meeting fits, which is the common case. When it does not, the
alternative to a second pass is not truncation — every segment appears in some
window, so a decision taken in the first ten minutes cannot vanish because the
meeting ran for ninety.

### C2. Notes as a source artifact — `types.rs`, `session_store.rs`

`MeetingNotes { during, before }`, persisted to `notes.json` beside
`session.json` and `transcript.jsonl`, committed by rename. `SessionStore` is the
only writer; the processing pipeline reads them and has no code path that could
write one.

`during` is the normal case and the point of the feature: three bullets a person
typed while a ninety-minute call was happening outrank any amount of prompt
tuning at saying which part mattered. `before` is the roughly-1-in-100 case,
tucked behind a disclosure in the UI, never required, never a pipeline stage,
never a summary section.

### C3. Length policy — `processing/length.rs` (new)

`summary_budget(transcript_words, mode)` derives the ceiling from the meeting:
proportional to the transcript, floored so a very short meeting still gets a
usable record, capped by the mode. Mode is a ratio and a ceiling over that
budget, not an absolute size — which is what keeps "Detailed" from meaning
"long": a detailed summary of a three-minute call stays smaller than a concise
summary of a ninety-minute one.

Topic count follows §6's duration table, judged on surviving words rather than
wall-clock, so ninety minutes that were sixty minutes of small talk get the band
their real content earns.

The budget is **stated to the model** in Stage B's user message and used by the
validator, so the model aims at the number it is judged against. Over-budget is a
warning; only a runaway (1.4×) is an error, because replacing a correct 600-word
summary with a fact dump is a much worse outcome for the reader than forty extra
words.

### C4. Schema — `processing/model.rs`

`PROCESSING_VERSION` 3 → 4. All three additions are `#[serde(default)]`, so
existing `processing.json` files load unchanged; they simply carry no rationale,
classify every point as plain discussion, and hold no risks until re-extracted.

- `Decision.rationale: Option<String>` — kept only when the meeting stated a
  reason. A sanitizer drops hollow answers ("not stated", a restatement of the
  decision, anything under three words), because a hollow "because" in a summary
  is worse than no because at all.
- `KeyPoint.kind: KeyPointKind` — discussion, proposal, recommendation,
  disagreement, tradeoff. This exists so a proposal has somewhere honest to live:
  a schema with one slot for "we could launch Friday" and "let's launch Monday"
  is an invitation to file the first as the second.
- `MeetingFacts.risks: Vec<Risk>` — risk, blocker, dependency, constraint, each
  with provenance. A separate collection rather than a flavour of key point,
  because a blocker is what a reader scans for first.

### C5. Model invocation — `providers/mod.rs`, `processing/llm.rs`

- `CompletionOptions { temperature, max_output_tokens, context_tokens, timeout_secs }`
  and `LLMClient::complete_with`, which reports a provider failure as a failure
  instead of substituting canned filler.
- Ollama now sends `options: { temperature, num_ctx, num_predict }`. `num_ctx`
  comes from `ProviderConfig.context_tokens` (default 8192, user-configurable),
  and the pipeline sizes its own windowing from the same number rather than
  assuming one. When Ollama reports that the prompt filled the window, that is
  logged as the truncation risk it is.
- The system prompt is a real field rather than a `[System Instructions: …]`
  prefix on the user prompt, so Relay's rules and the meeting's own words are no
  longer one undifferentiated string.
- Every request carries a timeout.
- `LlmRequest::extraction` runs at 0.1, `LlmRequest::prose` at 0.3, per §11.
- Anthropic and Gemini are routed to their own endpoints with their own body
  shapes and auth headers.

### C6. Summary contract — `processing/summarize.rs`

The system prompt is a numbered hierarchy of sixteen sections rather than a run
of paragraphs: role, objective, source, accuracy, include, exclude, be concrete,
rationale, uncertainty, attribution, depth, structure, output-only, and — only
when they exist — notes, presentation, and the user's own instructions. A small
local model follows a structure it can see; the previous version put the
prohibitions in the middle of prose and they were the part that got lost.

The facts payload changed too: key points are grouped under their topics, and any
point that is not plain discussion carries `"this_was_only_a": "proposal"`.
Stage B cannot organize by topic from a list that already lost the grouping, and
cannot avoid promoting a proposal it was never told was one.

### C7. Validation and repair — `processing/validate.rs`, `processing/mod.rs`

New deterministic checks: required first heading, no preamble, no empty or
placeholder-filled section, no risks section when the facts carry no risks, no
Decisions line that reproduces something the facts recorded as a proposal, and
length against the meeting's budget rather than a constant.

`repair_feedback` maps failed issue codes to corrective instructions. On failure
the pipeline makes **one** corrected attempt with that feedback prepended and the
request otherwise unchanged, revalidates, and only then falls back. The retry is
not the same prompt again: an identical request has no reason to produce a
different answer, and re-rolling the dice is not a repair.

`SummaryArtifact` records `repair_attempted` and `length_budget_words`, because
"needed a second try" and "could not do it" are different quality signals.

---

## D. Files changed, and why

| File | Why |
| --- | --- |
| `providers/mod.rs` | B1, B2, and the cloud mis-routing. Options, window, timeout, per-provider endpoints and body shapes. |
| `meetings_v2/processing/llm.rs` | Lets a stage state its sampling and ask how much prompt the model can read. |
| `meetings_v2/processing/context.rs` *(new)* | The canonical meeting context, and windowing. |
| `meetings_v2/processing/length.rs` *(new)* | B3. Per-meeting budget, stated to the model and used by the validator. |
| `meetings_v2/processing/eval.rs` *(new)* | The quality evaluation set and its scorer. |
| `meetings_v2/processing/model.rs` | B4. Rationale, point kind, risks; `PROCESSING_VERSION` 4. |
| `meetings_v2/processing/extract.rs` | Stage A contract rewritten; windowed extraction and deterministic merge; new fields sanitized. |
| `meetings_v2/processing/summarize.rs` | Stage B contract rewritten; budget and notes; the deterministic renderer carries rationale and risks. |
| `meetings_v2/processing/validate.rs` | Structural checks, budget-aware length, `repair_feedback`. |
| `meetings_v2/processing/mod.rs` | Assembles the context, computes the budget, runs the repair loop. |
| `meetings_v2/processing/modes.rs` | A mode decides depth, never an absolute length. |
| `meetings_v2/processing/conversation.rs` | `render_for_extraction` removed; `context.rs` owns what a model sees. |
| `meetings_v2/types.rs`, `session_store.rs` | `MeetingNotes` and its persistence. |
| `commands.rs`, `lib.rs` | `get_meeting_v2_notes`, `save_meeting_v2_notes`; user instructions into `ProcessingOptions`. |
| `settings/mod.rs` | `summary_instructions`. |
| `src/components/meetings_v2/MeetingNotesTab.tsx` *(new)* | The notes editor. |
| `src/components/meetings_v2/MeetingsV2View.tsx` | The Notes tab, second so it is reachable during a meeting. |
| `src/components/settings/MeetingsSettings.tsx` | Summary instructions. |
| `src/types/index.ts` | Mirrors the schema additions. |

---

## E. Prompt architecture

System and user messages carry different responsibilities, and they are now
different messages rather than one concatenated string.

**Stage A system prompt** — role · objective · source handling (only when notes
exist) · partial-window rules (only when the meeting needs more than one pass) ·
procedure · the six categories · decisions · action items · owners · deadlines ·
risks · uncertainty · hard rules · output.

**Stage A user message** — built by `MeetingContext`: metadata, participants,
glossary, notes, transcript. In that order, because the transcript is the longest
block and should still be in view when the model starts answering.

**Stage B system prompt** — role · objective · source · accuracy · include ·
exclude · be concrete · rationale · uncertainty · attribution · depth ·
structure · output-only, plus notes / presentation / user instructions when they
apply.

**Stage B user message** — the correction (on a repair only), the length budget,
the user's notes as emphasis, then the facts.

---

## F. The rules the system enforces

**Yes.** Preserve concrete information over abstraction. Preserve decisions with
their reasoning. Preserve uncertainty as uncertainty. Preserve explicit
commitments with owner and date where the meeting gave them. Distinguish topic,
opinion, proposal, recommendation, decision, commitment, and open question.
Preserve disagreements and trade-offs that affected the outcome. Attribute where
attribution matters. Respect what a person emphasised in their own notes.

**No.** Never invent a decision, fact, deadline, owner, commitment, conclusion,
risk, or agreement. Never promote a proposal into a decision. Never turn
discussion into an action item. Never infer an owner from proximity. Never derive
a deadline from urgency. Never over-abstract. Never summarize every sentence.
Never reproduce speaker chronology unless the sequence is the point. Never write
generic filler. Never print an empty section or fill one with a placeholder.

Three of these are enforced in code as well as in the prompt, because a prompt
rule with no check behind it is a hope: `qualify.rs` gates every action-item
candidate, `sanitize_deadline` requires a spoken date in a cited segment, and
`resolve_owner` refuses to promote an unmatched name to a speaker.

---

## G. Decision and action semantics

| The meeting said | Category | Where it lands |
| --- | --- | --- |
| "The integration has three blocking bugs" | discussion | `key_points`, kind discussion |
| "We could launch Friday" | proposal | `key_points`, kind proposal — **never** a decision |
| "I think we should use vendor A" | recommendation | `key_points`, kind recommendation |
| "I don't think that works, and here's why" | disagreement | `key_points`, kind disagreement |
| "Slower to build, but less operational risk" | tradeoff | `key_points`, kind tradeoff |
| "Let's launch Monday" | decision | `decisions` |
| "I'll have the build ready by Monday" | commitment | `action_items` |
| "Who signs off on the migration?" | open question | `open_questions` |
| "The script hasn't been reviewed" | blocker | `risks` |

An action item must survive `qualify.rs`, which rejects meeting mechanics, demo
narration, completed work, hypotheticals, vague non-deliverables, ASR fragments,
and anything that does not exist outside the meeting. The test that decides every
candidate: is this still pending after everyone leaves the call?

---

## H. Model configuration

| | Before | Now |
| --- | --- | --- |
| Ollama context window | provider default (4096, or 2048) | `context_tokens`, default 8192, user-set |
| Temperature | provider default (0.8) | Stage A 0.1, Stage B 0.3 |
| Max output tokens | unbounded | 2400 (Stage A); derived from the budget (Stage B) |
| Timeout | none | 300 s |
| Long transcripts | silently truncated by the provider | windowed, one pass each, merged |
| Retries | none | one targeted repair on a validation failure |
| Provider failure | masked as canned filler | reported, and recorded |
| Anthropic / Gemini | posted to `api.openai.com` | their own endpoints and body shapes |
| Streaming | not used | still not used — the pipeline is not interactive |

The model was **not** changed. The audit found the limiting factors to be
context, sampling, schema, and the absence of a repair path — all of which are
now addressed. Whether a larger model helps is a question this work makes
answerable, because `eval.rs` can now measure it.

---

## I. What is validated deterministically

Empty output · below a floor of 8 words · thin relative to the meeting's budget
(warning) · over budget (warning) · far over budget (error) · missing the
required first heading · a preamble addressed to the user · leaked JSON · more
than five consecutive words copied from the transcript · duplicate bullets
(warning) · a named participant who is not a known speaker or entity · a due date
when no action item has one · a risks section when the facts carry no risks · a
Decisions line reproducing something recorded as a proposal · a Decisions line
matching no extracted decision (warning) · sections that are empty or filled with
a placeholder (warning).

Action items are separately validated for empty descriptions, duplicates, owner
consistency, ISO deadline format, and unsourced deadlines; anything with an error
is dropped rather than shown.

Semantic truth is not attempted here. Regex cannot decide whether a summary is
right about a meeting, and pretending otherwise would make the validator both
brittle and falsely reassuring. That question belongs to `eval.rs`.

---

## J. The evaluation set

`processing/eval.rs` holds seven meetings with hand-checked expectations and a
deterministic scorer. Each case records what a good summary must contain, what it
must not say (`forbidden`), and what the meeting genuinely contains but a summary
must still leave out (`noise` — greetings, audio checks, decoder loops).

Scored axes: decision recall, action recall, owner accuracy, deadline accuracy,
rationale preservation, open-question recall, risk recall, detail preservation,
noise suppression, repetition, structure, length. Hallucination has no threshold:
one invented owner, deadline, or decision takes the case to zero, because a
summary that is ninety per cent right about work someone now believes they own is
worse than one that admits it does not know.

The scorer is model-free, so it runs in `cargo test` with no provider and no
run-to-run variance, and the same function can be pointed at real model output.
Two guard tests keep the cases honest: a forbidden claim may not be satisfied by
the transcript itself (the extractive fallback would echo it), and may not be a
tense flip of something the meeting did say.

**Limits worth stating.** The cases are short and synthetic, and the deterministic
floor is extractive, so it scores higher on lexical-overlap axes than a real
provider outage would feel to a reader. The set is a regression gate — is
anything hallucinated, is any owner or date invented, did the rationale survive —
rather than a leaderboard. Real recordings with hand-checked expectations are the
obvious next addition.

---

## K. Before and after

The same meeting, scored. "Before" is reconstructed from the code this replaces,
not written to lose: the old `MeetingFacts` had no field for a decision's reason,
no risks collection, and no way to mark a point as a proposal, so no prompt could
have recovered them — and `render_markdown` emitted exactly these four sections
in this order.

```text
BEFORE  overall 0.70 | decisions 1.00 actions 1.00 owners 1.00 dates 1.00
                       rationale 0.00 open 1.00 risks 1.00 detail 1.00
                     | 68 words | hallucinations: none

AFTER   overall 1.00 | decisions 1.00 actions 1.00 owners 1.00 dates 1.00
                       rationale 1.00 open 1.00 risks 1.00 detail 1.00
                     | 96 words | hallucinations: none
```

```markdown
--- before ---
## Summary

**Topics discussed:** Launch timing

- The payment integration still carries three blocking bugs.
- Shipping on top of the open bugs was judged worse than a weekend of slip.

## Decisions

- Move the launch from Friday to Monday. — Me

## Action Items

- [ ] Update the release calendar — **Speaker 1**
- [ ] Tell support about the new date — **Speaker 1**
```

```markdown
--- after ---
## Overview

**Topics discussed:** Launch timing

## Discussion

### Launch timing

- The payment integration still carries three blocking bugs.
- Trade-off: Shipping on top of the open bugs was judged worse than a weekend of slip.

## Decisions

- Move the launch from Friday to Monday — because QA needs another three days on
  the payment integration (Me)

## Action Items

- [ ] Update the release calendar — **Speaker 1**
- [ ] Tell support about the new date — **Speaker 1**

## Risks & Blockers

- **Blocker:** Three blocking bugs remain in the payment integration.
```

Decisions and actions were already right; the gain is entirely information the
old schema could not carry. The new output is longer here, but not because more
was said about the same things — the extra words are the reason behind the
decision and the blocker, which is exactly what "would I understand this
tomorrow" asks for.

Across the whole set, mean overall score: **0.94 with no model reachable, 1.00
with comprehension.** The floor's losses are concentrated where you would expect
— it never proposes a date, and it does not recognise an unresolved question.

---

## L. What was deliberately not done

- **No second summarization model, no embeddings, no retrieval, no agents.** The
  audit found no Relay problem they solve. The one added pass is per-*window*
  extraction, and only when a meeting does not fit — which is a coverage problem,
  not an architecture preference.
- **No streaming.** Summary generation is not an interactive surface; the UI
  shows a spinner and then a document.
- **No model change.** Context, sampling, schema, and repair were the binding
  constraints. `eval.rs` now makes the model question measurable rather than a
  matter of taste.
- **`qualify.rs` untouched.** It was already the strictest part of the pipeline
  and it was not the problem.
