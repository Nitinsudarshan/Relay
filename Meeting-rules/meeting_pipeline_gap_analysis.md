# Meeting pipelines, mechanism by mechanism — and where Relay actually stands

**Prepared for:** Relay (Tauri/Rust + React, Windows-first, local-first, bot-free)
**Analysis baseline:** v0.13.1
**Date:** 27 August 2026

> **Status update — Meetings v2.5.** Gaps 1, 3 and 5 are closed and gap 4 is
> partly addressed; see §5.1 for what shipped and what each fix actually does.
>
> **Status update — v0.16.0, the summary quality rebuild.** Gaps 8 and 10 are
> closed: meetings have a notes field written to `notes.json` as a source
> artifact and fed to Stage A, and `processing/eval.rs` holds a fixture set with
> hand-checked expectations and a deterministic scorer, so a change to a prompt
> or a threshold can be measured rather than spot-checked. That audit also found
> a defect this ledger did not: the provider layer sent no `num_ctx`, so Ollama
> silently discarded the front of any transcript past its default window — every
> claim below about what Relay's model "understood" from a long meeting was
> made against a half-read transcript. See
> `docs/meetings/SUMMARY_QUALITY_REBUILD.md`.
>
> The rest of the ledger stands as written.
**Scope:** meetings only, and **bot-free architectures only.** Dictation, scribbles, and
the Kanban/vault surfaces are out of scope except where a meeting exits into them.

> **Why bot-based tools are excluded.** A notetaker that joins the call as a
> participant is handed the participant roster by the conferencing platform: real
> names, join and leave times, the calendar event. That is a different product with a
> different set of problems, and Relay is not building it. Comparing against a pipeline
> that gets names for free tells Relay nothing it can act on, so none of those products
> appear below. Every product here taps the device's own audio, the way Relay does.

> **Relationship to `meeting_notes_competitive_teardown.md`.** That document's §4 gap
> table was written against the pre-v0.13.0 codebase; items 1, 4, 8, 9 and 14 have
> since shipped. This supersedes §4 and re-grounds every claim about Relay in the
> checked-out code at v0.13.1, with file and line references. Be aware that the
> teardown also covers bot-based products, which are out of scope here — but its §2.9
> legal note is architecture-independent and still governs gap 11: diarizing within a
> single recording is not the regulatory trigger, persisting an identity template
> across recordings is.

---

## 1. The frame: seven stages, every product

Every bot-free tool in this category is the same seven stages. Comparing feature lists
is noise; comparing what each stage *knows* is signal.

| # | Stage | The question it answers |
|---|---|---|
| 0 | **Trigger** | How does recording start, and does anything know a meeting is happening? |
| 1 | **Capture** | Where does the audio come from, and what metadata rides along with it? |
| 2 | **Transcription** | Words, with what context and what boundary handling? |
| 3 | **Attribution** | Who said which words, and where does the *name* come from? |
| 4 | **Comprehension** | Transcript → structure (decisions, tasks, topics, owners) |
| 5 | **Generation** | Structure → prose, in a shape someone asked for |
| 6 | **Correction & exit** | Fix what's wrong; get the output somewhere it does work |

Relay's own architecture already thinks in these terms — `processing/mod.rs` runs
normalize → speakers → conversation → extract → summarize → validate as named,
independently-statused stages. That is unusually clean for this category and it makes
the comparison below unusually direct.

---

## 2. The problem every bot-free pipeline shares: four ways a name gets in

A bot-free desktop app is a tap on the sound card. It gets one mixed stream and no
participant list, so a name has to be *earned*. There are exactly four sources, and
which of them a product uses is the sharpest way to tell these pipelines apart.

| # | Source | Who uses it | Cost |
|---|---|---|---|
| 1 | **Calendar + contacts** | anarlog (Apple Calendar + Contacts), Littlebird (prior meetings) | One integration. Highest accuracy per unit of work in the whole category. |
| 2 | **Channel provenance** — mic is you, system audio is everyone else | Granola ("Me" / "Them", at utterance granularity) | Free. Resolves exactly two identities. |
| 3 | **Transcript context** — the model reads "thanks, Pranjali" | Granola (infers names, then one-click correct) | Free where a stage already reads the whole transcript. |
| 4 | **Diarization + enrollment** | OpenWhispr (sherpa-onnx, local, shipped); Whisper Notes (on-device, post-hoc); Vowen (macOS only) | Expensive, error-prone, and biometrically regulated once a template persists across recordings. |

**Relay uses source 2 only, and at the coarsest granularity its capture layer permits.**
Sources 1 and 3 are absent; 3 in particular is nearly free, because Stage A already
reads the entire transcript. Source 4 is correctly deferred behind a consent flow. This
single fact cascades into gaps 1, 2 and 5 below.

Two market signals sit inside that table and are worth reading together. anarlog puts
speaker identification in its **paid hosted tier** — the local free tier does not have
it. Meetily's diarization is prominent in its marketing and was planned for the **PRO**
tier, not shipped in the community edition. OpenWhispr is the exception that proves it
is possible locally. Ground that Relay's two closest open-source competitors have
vacated is ground worth occupying.

---

## 3. Per-app pipelines

Each walkthrough is mechanism, not marketing, and ends with what that pipeline buys
that Relay's does not.

### 3.1 Granola — the closest functional comparison

**Capture.** Device-level mic + system audio, macOS and Windows, no bot. Works with any
conferencing app because it never touches a platform API. Audio is transcribed and then
**deleted**; only transcript and notes persist.

**Attribution.** Two tiers, and this is the part worth copying. Where real speaker tags
are unavailable it labels the transcript **"Me" and "Them"** straight off the
mic-vs-system split — at utterance granularity, not chunk granularity. On top of that
the *model infers speaker names from contextual clues in the transcript*, and the user
corrects a name inline in the notes or by asking Granola Chat to fix it. The mobile app
does on-device speaker recognition for in-person meetings.

**Comprehension.** Note-first. You type rough notes during the call; at the end,
"Enhance notes" uses your notes as **anchors** and searches the transcript to fill in
context around them. Type nothing and you get a generic summary. Output shape is
controlled by templates.

**Retrieval.** Granola Chat answers questions across all meetings with source-linked
citations.

**Buys them:** notes-as-anchor; contextual name inference with one-click correction;
per-utterance channel split; source-linked citations. Three of those four are directly
portable to Relay today.

### 3.2 Circleback — structured output *is* the product

**Capture.** No-bot desktop and mobile; handles in-person.

**The differentiator.** Action items are extracted, **assigned to owners
automatically**, and pushed into other tools via automations and webhooks. Automatic
speaker identification by name is what makes the assignment possible — the two
features are inseparable. Over 100 languages, with a stated focus on technical terms
and accents.

**Caveat.** Independent reviews flag that transcription in echoey or noisy in-person
settings still needs checking before you trust the output.

**Buys them:** owner assignment as a first-class output rather than a string in a
bullet, and a webhook/automation exit path. Relay's action items are *better-shaped
data* than this and have nowhere to go.

### 3.3 Littlebird — the meeting as a memory layer

**Capture.** Same bot-free system-audio model, but the notetaker is one feature inside a
broader context-memory product.

**The differentiator.** "Prep for meeting" pulls context from past meetings and company
history *before* a call starts. Meeting notes are not isolated documents — they feed a
memory layer you can query, and answers include links to the sources used.

**Integration.** Ships an **MCP server** so other AI tools (Claude, Cursor) can query
the meeting memory.

**Buys them:** pre-meeting context injection, which is the single highest-leverage idea
here — loading prior meetings with the same attendees gives you the participant list,
the project glossary, and last time's open action items before the model sees a word of
transcript. The MCP server is also the cheapest possible exit path for a
developer-facing tool.

**Not applicable:** Littlebird also reads the active window on your screen. Wrong
privacy posture for Relay.

### 3.4 Jamie — bot-free, transcript-only, EU-resident

**Capture.** Device audio, no bot, any platform, in-person included.

**The mechanism worth noting.** Once the transcript is generated, **the audio is
deleted**. Data stays in Europe on every plan including free; GDPR + ISO 27001 +
DORA positioning.

**Note the direction of the trade.** Granola and Jamie delete audio *because they had
to send it somewhere first*. Relay keeps audio because it never left the machine, which
is the stronger position *and* the enabler for click-to-seek and re-transcription.
Relay currently banks neither.

### 3.5 Meetily — Relay's closest architectural sibling

Single self-contained Tauri app, Rust backend, JS frontend, Parakeet/Whisper for
transcription, Ollama recommended for summarization with Claude/Groq/OpenRouter/OpenAI
as alternatives. macOS and Windows. Hardware acceleration enabled at build time.

Three mechanisms Relay lacks outright:
- **Dual capture with intelligent ducking and clipping prevention.** Relay does dual
  capture with a soft-saturating mix (`capture.rs:250 soft_mix`, system attenuated to
  0.9 and saturating past unity) — that prevents hard clipping, but it is not ducking:
  neither source is attenuated when the other is speaking.
- **Import an existing audio file** and transcribe it.
- **Re-transcribe any recorded meeting with a different model or language.**

Diarization was planned for the PRO tier, not shipped in the community edition.

**Buys them:** two commands Relay could almost have for free given its durable chunk
store, and a provider abstraction layer.

### 3.6 anarlog (formerly Hyprnote) — local-first, plugin-shaped

**Status note.** Hyprnote was renamed to char, then the project split: `char` is the
team's commercial productivity app, and **anarlog** is the MIT-licensed, local-first
open-source notetaker that continues the original Hyprnote line.

**Architecture.** Local-first, offline by default, Whisper plus a local LLM, app data in
local SQLite, bring-your-own-model via Ollama or cloud APIs. Extension system
explicitly modelled on VS Code. Integrates with **Apple Calendar, Contacts**, and
Obsidian.

**The telling detail.** Speaker identification sits in the **paid hosted tier**,
alongside hosted transcription and calendar connections. The local free tier does not
have it.

**Buys them:** Calendar *and Contacts* as a name source; a template system for summary
shape; a plugin architecture that lets output shape be community-authored rather than
vendor-authored.

### 3.7 OpenWhispr — the one that ships local speaker ID

Electron, React 19, TypeScript, better-sqlite3, whisper.cpp, **sherpa-onnx**,
shadcn/ui. MIT. macOS, Windows, Linux. Started as dictation, expanded into meeting
transcription and notes.

**The important bit.** It ships **live speaker identification and voice fingerprinting
today**, running locally through sherpa-onnx and ONNX Runtime, with all core features —
transcription, reasoning, diarization, semantic search — working against either local or
cloud models. Documented platform caveat: on Intel Macs, live speaker ID and
fingerprinting are unavailable because ONNX Runtime stopped shipping macOS x86_64
binaries at 1.24.

**Buys them:** the existence proof, and Relay's concrete implementation path.
sherpa-onnx has Rust bindings, runs on Windows, and gives both segmentation and speaker
embeddings without a Python dependency — which matters because pyannote would drag a
Python runtime into a Tauri app. The platform caveat is also a useful reminder to
feature-detect rather than assume.

### 3.8 Vowen and Whisper Notes — correction UX and honesty

**Vowen** has the best-documented diarization UX in the category:
- A diarization toggle presented at meeting start, not buried in settings.
- An optional **expected number of speakers** hint to improve clustering accuracy.
- Generic labels after transcription, then a mapping step to real names.
- **Name autocomplete** drawn from names used before across all notes, plus other
  speakers in the current note, so recurring attendees are one tap.
- **Merge for split speakers** — when one person is clustered as both Speaker 2 and
  Speaker 4, you collapse them into one.

It documents its platform limits plainly: on-device pyannote is macOS only.

**Whisper Notes** runs diarization fully on-device and is unusually honest about it.
Labels appear after the recording ends, not during. **Tapping a sentence seeks playback
to that moment.** They maintain a fixed two-file benchmark scored at 10 ms resolution
against hand-checked ground truth, publish the numbers, and describe the one failure
mode they could not engineer away — two similar voices in a reverberant room with
frequent short interjections.

**Buys them:** every element of Vowen's mapping UX, especially merge-split-speakers,
which is the single most common diarization failure. From Whisper Notes: post-hoc rather
than live diarization, click-to-seek, and a small fixed benchmark re-run on every change.

### 3.9 WhisperX / Vosk / Whishper — engines, not products

Not comparable as apps; relevant as components. **WhisperX** is the one to know: forced
alignment gives word-level timestamps, and it pairs VAD-based segmentation with
diarization, which is materially better than naive fixed-window chunking at exactly the
boundary problem described in gap 4 below.

---

## 4. Relay's pipeline as actually built at v0.13.1

Traced from code, not from design docs.

```
════════ SOURCE — immutable, written only by the recorder ════════
 mic (cpal) ─┐
             ├─▶ DualAudioCapture ──┬─▶ Clock A: 30 s chunks ─▶ chunk_%05d.wav
 sys loopback┘   capture.rs         │   worker.rs                (16 kHz mono PCM)
                 · resample→16 k    │                                  │
                 · soft_mix (:250)  │                          Whisper, FRESH state
                 · per-chunk RMS    │                          per chunk (stt.rs:388)
                   → 2 bools        │                                  │
                                    │                          transcript.jsonl
                                    │                          append-only, status
                                    │                          SUCCESS|EMPTY|FAILED
                                    │                          + mic_had_audio,
                                    │                            sys_had_audio
                                    └─▶ Clock B: 1 s frames ─▶ LiveSttWorker
                                        (bounded, droppable)    no_context=true
                                                                → UI only, ephemeral

════════ DERIVED — processing.json + processing_log.jsonl ════════
  normalize      deterministic: filler, ASR tags, loop collapse, glossary
                 by edit-distance (normalize.rs) — raw is never touched
       ↓
  speakers       channel → speaker_id; Mixed/Unknown stay None (speakers.rs)
       ↓
  conversation   speaker-grouped chronological turns (conversation.rs)
       ↓
  extract        Stage A → MeetingFacts JSON (extract.rs)
       ↓           └─▶ qualify: durability → deliverable → commitment gates,
       ↓               owner resolution, scoring, semantic dedup, hard cap
       ↓               (qualify.rs — one call site, both paths)
  summarize      Stage B → Markdown from FACTS ONLY; transcript not in scope
       ↓         mode × extension; deterministic renderer as the floor
  validate       verdict on the prose actually shown; provider_output_status
                 tracked separately from stage status
```

Per-stage `StageStatus`, `PROCESSING_VERSION` = 2, `RULES_VERSION`, and a processing log
that explains a run without reproducing the meeting.

### What Relay already does better than this field

Worth stating plainly, because it changes what the gaps below cost:

1. **The action-item quality gate is in code, not in a prompt.** `qualify.rs` (1,359
   lines + 590 of tests) runs durability → deliverable → commitment gates on
   per-*sentence* evidence, resolves owners, scores, deduplicates semantically, and
   enforces a hard cap — with a single call site so the model path and the cue-based
   path cannot disagree. "I'll just be back in a minute" and "I'll stop sharing" are
   rejected structurally. Every tool in this category produces that garbage; nobody in
   this set documents anything like this defence.
2. **Stage B cannot see the transcript.** A model cannot copy out sentences it was
   never shown. That is a structural guarantee against transcript-reshuffling, not a
   prompt instruction.
3. **Source/derived separation is real and enforced.** Deleting `processing.json` loses
   nothing that cannot be recomputed. The local competitors have not built this.
4. **There is always a summary.** No Ollama, no network, model output rejected — the
   deterministic renderer still produces prose. Meetily and anarlog degrade to nothing
   when the local model is down.
5. **Failure is reported honestly.** `provider_output_status`
   (`ACCEPTED|REJECTED|UNAVAILABLE|NOT_ATTEMPTED`), `rejected_issues`, `fallback_used`
   and `SummarySource` are separate fields, so the UI never calls a deterministic
   render an AI summary.
6. **Audio is kept, because it never left.** The privacy-first competitors delete audio
   as their privacy story. Relay's chunks are durable on disk — the precondition for
   click-to-seek and re-transcription, both of which Relay has not yet spent.

---

## 5. Where Relay falls short — ranked by what it costs a real meeting

### 5.1 Closed in Meetings v2.5

**Gap 1 — attribution resolves at utterance granularity now.** `capture.rs` keeps
the per-source RMS it already measured as a per-second track on the chunk
(`ChannelEnergy`), `stt.rs` returns Whisper's own timed utterance spans instead of
one concatenated string, and `worker.rs` matches each span against the track. A
10 dB dominance margin rejects speaker bleed into the microphone, so an utterance
resolves to one source instead of registering both. Genuine crosstalk still
resolves to no speaker — the module's rule that ambiguity is preserved rather
than guessed is unchanged. Normalized segments are now one per utterance
(`seg_<chunk>_<utterance>`), which is what lets `qualify.rs` keep the owner it
computes instead of demoting it. `PROCESSING_VERSION` is 3.

**Gap 3 — the recognizer gets the vocabulary before it guesses.**
`AppSettings::build_stt_prompt` existed and was called from nowhere; the meetings
path now seeds `initial_prompt` from it, so the user's dictionary reaches Whisper
as well as the post-hoc glossary rewrite. An explicitly configured STT prompt
still wins.

**Gap 4 — partly.** Each chunk still gets a fresh Whisper state, so a decoder
loop cannot propagate. What changed is that the tail of the previous chunk's text
is carried into the next chunk's prompt, restoring cross-boundary context without
restoring shared state. Silence, an empty decode, and a failed decode all clear
the carry, so context never splices across a gap. *Still open:* an overlap window
on the durable chunk with seam deduplication, and VAD-aligned cuts.

**Gap 5 — to-dos leave the app.** `processing::tasks` maps an action item to a
`MeetingTaskDraft` (title, assignee resolved through the speaker registry, due
date from a spoken deadline only, priority from deadline plus extraction
confidence, description carrying meeting and transcript provenance), and
`push_meeting_v2_action_items_to_kanban` turns drafts into `KanbanCard`s. The card
is saved before the to-do is marked as pushed, and `kanban_card_id` on the action
item is what makes "add all" safe to press twice.

*Unchanged and still open:* gaps 2, 6, 7, 8, 9, 10, and the tier-4 deferrals.
Gap 2 is now the highest-value remaining item — the attribution machinery
resolves *which* channel spoke, but nothing yet attaches a **name** to it.



### Tier 1 — the output is measurably worse than a competitor's for the same recording

**Gap 1. Attribution resolves to "unknown" for most of a real conversation.** *(closed in v2.5 — see §5.1)*

`capture.rs:632` computes `mic_had_audio` / `sys_had_audio` as one RMS boolean per
source per **30-second chunk**, and `types.rs:123-126` persists exactly those two
bools. `SegmentChannel::from_flags` maps `(true, true)` → `Mixed` →
`implied_speaker_id() == None`. In any genuine two-way conversation, nearly every
30-second window contains both sources, so most segments carry no speaker at all.

The downstream cost is concrete and lands exactly where it hurts: `qualify.rs`
carefully resolves an action-item owner and then **demotes to `Unassigned` when every
cited segment had both channels live**. The owner field that the whole quality gate is
built around is therefore empty most of the time. Granola gets Me/Them working because
it splits at utterance granularity; Relay measures the same signal and throws away all
but 1 bit per source per 30 s.

*This is not diarization, needs no model, no ONNX, and no consent flow.* The fix is to
carry a per-second channel energy track (~30 pairs per chunk) alongside the existing
booleans — the measurement capture already performs. It changes no WAV format, no chunk
duration, and no clock. It is the single highest value-to-effort item in this document,
and it is still open as OUT-OF-SCOPE ISSUE 1 in the audit.

**Gap 2. Two of the four name sources are absent, and one of them is nearly free.**

Verified: zero calendar, attendee, or contacts references anywhere in the meetings code
(source 1). And no attempt at contextual name inference (source 3) — even though Stage A
is the one place in the pipeline that reads the whole transcript and could name speakers
from "thanks, Pranjali" at no additional cost.

The result: every meeting shows "Me" and "Speaker 1" until a human types something, and
typed names do not carry to the next meeting. anarlog and Littlebird both treat
calendar/contacts as the primary source; Granola gets real names out of the transcript
itself. Relay has neither.

**Gap 3. Vocabulary is corrected after the fact, never given to the recognizer.** *(closed in v2.5 — see §5.1)*

`normalize.rs:288 apply_glossary` rewrites known terms by edit distance *after* Whisper
has already guessed. Whisper's own biasing mechanism — `initial_prompt` — is already
wired (`stt.rs:239`, `:256 from_settings`) but fires only from a manually typed
`custom_initial_prompt`. It is never auto-seeded from the user's dictionary, the
meeting's attendees, or the topics of related prior meetings.

Post-hoc repair cannot recover a name Whisper never produced a near-miss for, and for
Hinglish that is exactly the hard case. This one costs nothing but wiring: the glossary
table and the prompt path both already exist.

**Gap 4. Chunk boundaries are decoded blind.** *(partly closed in v2.5 — see §5.1)*

`stt.rs:388-389` creates a **fresh** `WhisperState` per chunk. That is the right call
for hallucination-loop safety — nothing carries over — but it means a sentence
straddling a 30-second boundary is decoded as two independent halves with no overlap
window, no prompt carry, and no VAD-aligned cut. WhisperX's VAD segmentation plus forced
alignment exists precisely to solve this. Cheapest fixes: a small overlap on the durable
chunk with seam deduplication, and/or carrying the tail of the previous chunk's text as
`initial_prompt` — which composes with gap 3.

### Tier 2 — the meeting cannot be acted on

**Gap 5. Nothing leaves the app.** *(closed in v2.5 — see §5.1)*

Relay's action items are already better-shaped data than the competitors hold: owner,
deadline, status, evidence span, confidence — structured objects, with
`set_meeting_v2_action_item_status` (`commands.rs:1999`) making checkboxes durable. And
the only exit in the codebase is `promote_meeting_v2_to_scribble`
(`commands.rs:2146`) — which produces a *note*, not a task.

No Kanban card, no webhook, no calendar event, no clipboard/markdown share, no MCP.
Circleback's entire product is the exit path; Littlebird and OpenWhispr both expose
their meeting memory over MCP. Relay already has a Kanban board in the same application
and does not connect the two. The data is shaped correctly; only the wire is missing.

**Gap 6. No retrieval across meetings, and no pre-meeting context.**

`related.rs` scores relatedness on demand over derived artifacts (topics 0.35, entities
0.25, type 0.15, speakers 0.15, title 0.10 — title deliberately unable to carry a match
alone). Good design. But there is no *ask*: no equivalent of Granola Chat's
source-linked answers, and nothing like Littlebird's "prep for meeting", which is the
highest-leverage version of this idea — prior meetings with the same attendees hand you
the participant list, the project glossary, and last time's open action items before the
model sees a word of transcript.

Relay already ships a grounded voice chat over a LanceDB vault for scribbles. Meetings
are not in it; they live in a filesystem tree with no index.

**Gap 7. Evidence and audio are unreachable from the UI.**

`audio_full.wav` is merged at finalize and then never played — there is no `<audio>`
element anywhere in `meetings_v2/` (the `Play` icons are recorder controls). Action items
carry evidence spans internally and the UI cannot jump to them. Whisper Notes'
tap-a-sentence-to-seek is the reference, and Relay is one of the *few* tools in this set
that still has the audio to seek into.

### Tier 3 — quality cannot be improved or trusted over time

**Gap 8. No notes-as-anchor.** *(closed in v0.16.0 — a Notes tab writes
`notes.json` beside `session.json`, and `processing/context.rs` feeds it to
Stage A as emphasis rather than as a second transcript.)* As originally found: no
notes field in
`MeetingRecordingOverlay.tsx`, nothing to type into during a meeting. Granola, anarlog
and Littlebird converged on this independently, and it is the cheapest quality
multiplier available: a user's own three bullets outperform any amount of prompt tuning
at telling Stage A what mattered. It fits Relay's architecture cleanly — human notes are
a *source* artifact, and Stage A already takes structured input.

**Gap 9. No re-transcribe, no audio import.** Chunks are durable, models are swappable,
and no command exists to re-run either. Meetily ships both. For a Hinglish-first product
where a mangled decode is a routine outcome, re-transcribe is close to essential — and
it is nearly free given the durable chunk store.

**Gap 10. No benchmark.** *(closed in v0.16.0 — `processing/eval.rs` holds seven
meetings with hand-checked expectations and a model-free scorer over eleven
axes, with hallucination as a hard gate. The cases are still synthetic; real
recordings are the obvious next addition.)* As originally found: there is a
serious Rust test suite (~2,600 lines across
`processing/tests.rs` and `qualify/tests.rs`), but it tests behaviour on synthetic
input. There is no fixture set of real transcripts with hand-checked expected facts, so
a threshold change in `qualify.rs` or a prompt edit in Stage A cannot be *measured* —
only spot-checked. Whisper Notes maintains and publishes exactly this.

### Tier 4 — correctly deferred

**Gap 11. Diarization + enrollment.** Rightly gated behind a consent and retention UI:
diarization within one recording is not the regulatory trigger, but persisting an
identity template across recordings is. Two things to hold onto — the market signal is
favourable (anarlog and Meetily both put it behind a paid tier; only OpenWhispr ships it
locally, via sherpa-onnx with Rust bindings and no Python), and **fixing gap 1 captures
most of the practical value at a fraction of the cost with zero biometric exposure.** Do
gap 1 first regardless.

**Gap 12. No meeting detection.** Manual start only. anarlog and Littlebird both trigger
from the calendar. This is downstream of gap 2 — one calendar integration addresses both.

**Gap 13. No ducking.** `soft_mix` prevents clipping but does not attenuate either
source when the other is active. Meetily ships ducking. Lower priority than it looks:
with a per-second channel track (gap 1), *not* ducking is arguably correct, because
attenuation would corrupt the very energy measurement attribution depends on.

---

## 6. Revised sequence

The old teardown's recommended sequence is now partly done. Given v0.13.1, this is the
order that maximises value per unit of work:

**Done in v2.5**
1. ~~**Per-second channel energy track** in `capture.rs`~~ — shipped, at utterance
   granularity via Whisper's own segment timings.
2. ~~**Meeting action item → Kanban card**~~ — shipped as `processing::tasks` plus
   `push_meeting_v2_action_items_to_kanban`.
3. ~~**Vocabulary at the recognizer**~~ — shipped; `initial_prompt` is seeded from
   the dictionary and carries the previous chunk's tail.

**First — the highest-value remaining item**
1. **Calendar + contacts integration.** One piece of work, three payoffs: attendee names
   for the speaker registry, an `initial_prompt` seed of names and terms, and priors for
   `related.rs`. Also delivers gap 12. Now the top of the list: v2.5 resolves *which
   channel* spoke, but nothing yet attaches a name to it.

**Next — transcription quality where it actually breaks**
4. Overlap window on durable chunks with seam dedup; `initial_prompt` auto-seeded from
   dictionary + attendees + related-meeting topics.
5. Contextual speaker-name inference in Stage A, surfaced as a *suggestion* the user
   confirms — Granola's mechanic, and it reuses the registry's existing
   `Manual` > `Channel` precedence.
6. ~~**In-meeting notes field as a source artifact, fed to Stage A as anchors.**~~
   — shipped in v0.16.0.

**Then — make the archive worth having**
7. Click-to-seek and re-transcribe-with-another-model. Both are wiring
   `audio_full.wav` and the chunk store to existing surfaces.
8. Meetings into LanceDB; extend the existing grounded chat to cross-meeting questions
   with citations back to a transcript span. Then Littlebird-style pre-meeting context
   from the same index.
9. ~~**Fixture benchmark**, re-scored on every change to `qualify.rs` or the
   Stage A/B prompts.~~ — shipped in v0.16.0 as `processing/eval.rs`, on
   synthetic fixtures. Real meetings with hand-checked expected facts remain
   the upgrade.

**Then — the regulated feature**
10. sherpa-onnx post-hoc diarization behind a per-meeting toggle with an optional
    speaker-count hint; the Vowen mapping UX (autocomplete from prior names, merge split
    clusters); consent and one-click deletion UI *before* any embedding is persisted.

---

## 7. Where Relay can genuinely win

**Local speaker identification, free.** Both open-source competitors — anarlog and
Meetily — put diarization behind a paid tier; OpenWhispr is the exception. If Relay
ships on-device attribution plus enrollment on Windows, it occupies ground the closest
competitors have vacated — and gap 1 gets most of the way there without touching a
biometric.

**Degraded and multilingual transcripts.** Nobody in this set handles Hinglish-through-
Whisper well. Circleback claims accent handling; everyone else quietly assumes clean
English. The Stage 0 normalizer plus a glossary *at the recognizer* plus source-language
transcription is a real differentiator for the Indian market specifically, and it is work
the incumbents have no incentive to do.

**Windows-first.** Vowen's on-device diarization is macOS only. Whisper Notes is
Apple-only. anarlog leads with Homebrew. Relay running properly on a Dell G15 is not a
small thing.

---

## 8. Sources

Every claim about Relay is from the checked-out tree at v0.13.1 and cites its file.
App-side mechanisms carry the sourcing in `meeting_notes_competitive_teardown.md`
§Sources, plus the bot-free roundups retrieved 27 August 2026 below.

- Granola transcription docs — https://docs.granola.ai/help-center/taking-notes/transcription
- Granola, what is an AI notepad — https://www.granola.ai/blog/what-is-an-ai-notepad
- Littlebird meeting notes — https://littlebird.ai/features/meeting-notes
- TechCrunch on Littlebird — https://techcrunch.com/2026/03/23/littlebird-raises-11m-to-capture-context-from-your-computer-so-you-can-query-your-data/
- Circleback review — https://tooliverse.ai/tools/circleback
- Jamie, bot-free notetakers — https://www.meetjamie.ai/en/blog/top-10-bot-free-notetakers-for-microsoft-teams-2025
- Jamie review — https://workgpt.com/en/app-reviews/jamie-ai
- Bot-free notetakers, independent roundup — https://fellow.ai/blog/bot-free-ai-note-takers/
- Meetily — https://github.com/Zackriya-Solutions/meetily
- anarlog (formerly Hyprnote) — https://github.com/fastrepl/anarlog and https://anarlog.so/
- OpenWhispr — https://github.com/OpenWhispr/openwhispr
- Vowen diarization docs — https://docs.vowen.ai/meeting-notes/diarization
- Whisper Notes on-device diarization — https://whispernotes.app/blog/whisper-speaker-diarization
