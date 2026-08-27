# Meeting pipelines, mechanism by mechanism — and where Relay actually stands

**Prepared for:** Relay v0.13.1 (Tauri/Rust + React, Windows-first, local-first, bot-free)
**Date:** 27 August 2026
**Scope:** meetings only. Dictation, scribbles, and the Kanban/vault surfaces are out of scope except where a meeting exits into them.

> **Relationship to `meeting_notes_competitive_teardown.md`.** That document's §4 gap
> table was written against the pre-v0.13.0 codebase. Items 1, 4, 8, 9 and 14 of that
> table have since shipped. This document supersedes §4 and re-grounds every claim
> about Relay in the checked-out code at v0.13.1, with file and line references. The
> teardown's per-app research and its §2.9 legal note still stand and are not repeated.

---

## 1. The frame: seven stages, every product

Every tool in this category — cloud or local, bot or bot-free — is the same seven
stages. Comparing feature lists is noise; comparing what each stage *knows* is
signal.

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

## 2. The one structural fact that explains most of the differences

**Whether a bot joins the call determines where names come from, and almost
everything else follows.**

A bot is a *participant*. That single fact hands the vendor, for free, from the
platform API: the participant roster with real names, join/leave times, the calendar
event, the host, and on some platforms per-participant audio streams. Fireflies is
explicit about the consequence — real participant names on Google Meet and Zoom,
generic `Speaker 1` / `Speaker 2` everywhere else. Their diarization is a *fallback*
for platforms that won't hand over a roster, not the primary mechanism.

A bot-free desktop app is a *tap on the sound card*. It gets one mixed stream and no
roster at all. Names have to be earned, from exactly three possible sources:

1. **Calendar attendees** — cheap, high-accuracy, and the reason every serious
   bot-free tool integrates a calendar first.
2. **Channel provenance** — microphone is you, system audio is everyone else. Free,
   always available, and resolves only two identities.
3. **Diarization + enrollment** — expensive, error-prone, and biometrically
   regulated. Otter's voiceprint loop is the reference; both open-source competitors
   paywalled it.

Relay chose bot-free, which is the right call for its privacy story and the reason it
works on any conferencing platform without an integration. But it means Relay
inherited the *hard* version of stage 3 and today solves it with source 2 only, at the
coarsest possible granularity. That single decision cascades into gaps 1, 2 and 5
below.

---

## 3. Per-app pipelines

Each walkthrough is mechanism, not marketing, and ends with what that pipeline buys
that Relay's does not.

### 3.1 Fathom — bot-based, calendar-driven, three capture fidelities

**Trigger** Calendar-connected; auto-joins the meetings you designate.
**Capture** Three selectable fidelities: full audio+video, audio+transcript, or
transcript-only. The two transcript modes can run without a visible bot. Cloud.
**Transcription** Real-time, cloud ASR, speaker-labelled as it goes.
**Attribution** Platform roster plus diarization for overlapping speech.
**Comprehension → generation** The differentiator is *latency and shape*: transcript,
summary and action items land within minutes of the call ending, in one of a set of
pre-built meeting-type templates.
**Exit** CRM sync, task tools, Zapier.

**Buys them:** capture fidelity as a user-facing dial, and template-per-meeting-type
selected before the call rather than after. Relay has modes and extensions
(`processing/modes.rs`) but they are chosen post-hoc and there is no fidelity dial.

### 3.2 Fireflies — the CRM-shaped pipeline

**Trigger** Calendar detection; the notetaker is auto-added as an invitee.
**Capture** Five ingress paths into one pipeline: bot, desktop background recording,
mobile for in-person, web recorder, and file upload.
**Transcription** Real-time cloud, with **custom vocabulary applied at the
recognizer** — a shipped, user-editable product feature.
**Attribution** Roster-first, diarization second, plus a "correct the label once"
loop.
**Comprehension** Action items with owners; topic trackers that fire on configurable
keyword sets.
**Retrieval** AskFred answers natural-language questions across every meeting and
returns timestamps and speaker attribution with the answer.
**Exit** The deepest exit path in the category: Salesforce/HubSpot/Pipedrive call
logging, Slack, task tools, webhooks.

**Buys them:** (a) many ingress paths, one pipeline — notably **file upload**, which
Relay cannot do at all; (b) vocabulary at the recognizer, not post-hoc; (c) cited,
cross-meeting retrieval; (d) an exit path that makes the meeting a *side effect* of
updating the systems people actually work in.

### 3.3 Otter — the reference implementation for identity

Covered in the teardown §2.3; the mechanism worth restating because it is the exact
shape Relay's own `speakers.rs` is designed to grow into:

Diarize → generic `Speaker N` labels → **one** human tag → persistent voiceprint
against the workspace → auto-label in every future meeting. Manually confirmed
speakers get a visual check distinguishing them from inferred ones. Tagging one
segment propagates across the whole cluster.

**Buys them:** identity that *improves with use*. Relay's registry already has the
right bones — stable ids that are never display names, `SpeakerOrigin::Manual`
outranking `Channel`, renames surviving regeneration (`speakers.rs:110-155`) — and no
mechanism that makes a name persist to the *next* meeting.

### 3.4 Granola — bot-free, note-first, Me/Them

**Capture** Device-level mic + system audio, macOS and Windows, no bot. Audio is
transcribed and then **deleted**; only transcript and notes persist.
**Attribution** Two tiers, and this is the part worth copying: where real speaker tags
are unavailable it labels the transcript **"Me" and "Them"** straight off the
mic-vs-system split — then the *model infers speaker names from context in the
transcript*, and the user corrects a name inline or by asking Granola Chat.
**Comprehension** Note-first. You type rough notes during the call; "Enhance notes"
uses those notes as **anchors** and searches the transcript for context around them.
Type nothing and you get a generic summary.
**Retrieval** Granola Chat, source-linked citations across all meetings.

**Buys them:** the notes-as-anchor mechanic, which is the highest-quality-per-effort
idea in the entire category, and contextual name inference on top of the same channel
split Relay already has.

### 3.5 Jamie — bot-free, transcript-only, EU-resident

**Capture** Device audio, no bot, any platform, in-person included.
**The mechanism worth noting:** once the transcript is generated, **the audio is
deleted**. Data stays in Europe on every plan including free; GDPR + ISO 27001 + DORA
positioning.

**Buys them:** a crisp privacy story — but note the direction of the trade. Granola
and Jamie delete audio *because they had to send it somewhere*. Relay keeps audio
because it never left the machine, which is the stronger position *and* the enabler
for click-to-seek and re-transcription. Relay currently banks neither.

### 3.6 tl;dv / MeetGeek — the analytics and retrieval end

**tl;dv** Bot-based; transcription, summaries, timestamped clip-and-share. Popular
with sales ICs. The interesting stage is 6: sharing a *moment*, not a document.
**MeetGeek** Broadest platform coverage in the set (Zoom, Meet, Teams, Webex,
Discord, Slack huddles, plus in-person via mobile through **the same pipeline**), 60+
languages, meeting analytics over time, AI Chat, and an **MCP server** exposing
meeting memory to Claude/ChatGPT.

**Buys them:** one pipeline for virtual and in-person; cross-meeting analytics as a
product; MCP as the exit path for a developer audience — the cheapest possible
integration surface, and a natural fit for Relay given its user base.

### 3.7 Meetily — Relay's closest architectural sibling

Tauri + Rust + JS, Parakeet/Whisper, Ollama-first with cloud alternatives, macOS and
Windows.

Three mechanisms Relay lacks outright:
- **Dual capture with intelligent ducking and clipping prevention.** Relay does dual
  capture with a soft-saturating mix (`capture.rs:250 soft_mix`, system attenuated to
  0.9 and tanh-ish saturation past unity) — that prevents hard clipping but it is not
  ducking: neither source is attenuated when the other is speaking.
- **Import an existing audio file** and transcribe it.
- **Re-transcribe any recorded meeting with a different model or language.**

Diarization is prominent in its marketing and was planned for the **PRO** tier, not
shipped in the community edition.

### 3.8 anarlog (formerly Hyprnote) — local-first, plugin-shaped

Local Whisper + local LLM, SQLite, bring-your-own-model via Ollama or cloud,
VS-Code-style extension system, Apple Calendar + Contacts + Obsidian integration.

The telling detail: **speaker identification sits in the paid hosted tier.** The local
free tier does not have it.

**Buys them:** Calendar *and Contacts* as a name source, and a plugin architecture
that lets the summary shape be community-authored rather than vendor-authored.

### 3.9 OpenWhispr — the one that ships local speaker ID

Electron, whisper.cpp, **sherpa-onnx**, better-sqlite3. Ships **live speaker
identification and voice fingerprinting today, locally**, with transcription,
reasoning, diarization and semantic search all working against local or cloud models.
Documented caveat: unavailable on Intel Macs because ONNX Runtime stopped shipping
macOS x86_64 binaries after 1.24.

**Buys them:** the existence proof. sherpa-onnx has Rust bindings, runs on Windows,
and gives segmentation plus speaker embeddings with no Python runtime — which is
precisely why it, and not pyannote, is Relay's implementation path when the time
comes.

### 3.10 Whisper Notes / Vowen — the correction UX and the honesty

**Whisper Notes** On-device diarization, labels appear **after** the recording ends
(not during), **tap a sentence to seek playback to that moment**, and a fixed
two-file benchmark scored at 10 ms resolution against hand-checked ground truth, with
published numbers and a stated unfixable failure mode.
**Vowen** Diarization toggle at meeting start rather than buried in settings; an
optional **expected speaker count** hint; generic labels then a mapping step; **name
autocomplete from names used in prior notes**; and **merge for split speakers** —
collapsing one person clustered as both Speaker 2 and Speaker 4. On-device is macOS
only; Windows requires a cloud model.

**Buys them:** every element of the correction UX, of which *merge split clusters* is
the one that matters most because it is the most common diarization failure. And the
benchmark discipline: a fixed fixture set re-scored on every change.

### 3.11 WhisperX / Vosk / Whishper — engines, not products

Not comparable as apps; relevant as components. **WhisperX** is the one to know:
forced alignment gives word-level timestamps and it pairs VAD-based segmentation with
diarization, which is materially better than naive fixed-window chunking at exactly
the boundary problem described in gap 4 below.

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

Per-stage `StageStatus`, `PROCESSING_VERSION` = 2, `RULES_VERSION`, and a processing
log that explains a run without reproducing the meeting.

### What Relay already does better than most of this field

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
3. **Source/derived separation is real and enforced.** Deleting `processing.json`
   loses nothing that cannot be recomputed. Cloud tools have no reason to build this;
   the local competitors have not.
4. **There is always a summary.** No Ollama, no network, model output rejected — the
   deterministic renderer still produces prose. Meetily and anarlog degrade to nothing
   when the local model is down.
5. **Failure is reported honestly.** `provider_output_status`
   (`ACCEPTED|REJECTED|UNAVAILABLE|NOT_ATTEMPTED`), `rejected_issues`, `fallback_used`
   and `SummarySource` are separate fields, so the UI never calls a deterministic
   render an AI summary.
6. **Audio is kept, because it never left.** The privacy-first competitors delete
   audio as their privacy story. Relay's audio is durable on disk — which is the
   precondition for click-to-seek and re-transcription, both of which Relay has not
   yet spent.

---

## 5. Where Relay falls short — ranked by what it costs a real meeting

### Tier 1 — the output is measurably worse than a competitor's for the same recording

**Gap 1. Attribution resolves to "unknown" for most of a real conversation.**

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
booleans — the measurement capture already performs. It changes no WAV format, no
chunk duration, and no clock. It is the single highest value-to-effort item in this
document, and it is still open as OUT-OF-SCOPE ISSUE 1 in the audit.

**Gap 2. There is no name source at all.**

Verified: zero calendar, attendee, or contacts references anywhere in the meetings
code. No calendar integration, no attendee list, no contacts, and — notably — no
attempt at Granola's *contextual name inference*, even though Stage A is the one place
in the pipeline that reads the whole transcript and could name speakers from "thanks,
Pranjali" for free.

The result: every meeting shows "Me" and "Speaker 1" until a human types something,
and typed names do not carry to the next meeting. Otter, anarlog, Littlebird and every
bot-based tool treat calendar/roster as the *primary* name source and audio as the
fallback. Relay has neither the primary nor the fallback.

**Gap 3. Vocabulary is corrected after the fact, never given to the recognizer.**

`normalize.rs:288 apply_glossary` rewrites known terms by edit distance *after*
Whisper has already guessed. An `initial_prompt` path exists
(`stt.rs:239`, `:256 from_settings`) but fires only from a manually typed
`custom_initial_prompt` — it is never auto-seeded from the user's dictionary, the
meeting's attendees, or the topics of related prior meetings. Otter and Fireflies both
ship custom vocabulary applied at the recognizer. Post-hoc repair cannot recover a
name Whisper never produced a near-miss for, and for Hinglish it is exactly the hard
case.

**Gap 4. Chunk boundaries are decoded blind.**

`stt.rs:388-389` creates a **fresh** `WhisperState` per chunk. That is the right call
for hallucination-loop safety — nothing carries over — but it means a sentence
straddling a 30-second boundary is decoded as two independent halves with no overlap
window, no prompt carry, and no VAD-aligned cut. WhisperX's VAD segmentation plus
forced alignment exists precisely to solve this. Cheapest fixes: a small overlap on
the durable chunk with seam deduplication, and/or carrying the tail of the previous
chunk's text as `initial_prompt` (which also composes with gap 3).

### Tier 2 — the meeting cannot be acted on

**Gap 5. Nothing leaves the app.** *(largest product value per unit of effort)*

Relay's action items are already better-shaped data than most competitors hold: owner,
deadline, status, evidence span, confidence — structured objects, persisted, with
`set_meeting_v2_action_item_status` (`commands.rs:1999`) making checkboxes durable. And
the only exit in the codebase is `promote_meeting_v2_to_scribble`
(`commands.rs:2146`) — which produces a *note*, not a task.

No Kanban card, no webhook, no calendar event, no clipboard/markdown share, no MCP.
Circleback's entire product is the exit path; Fireflies' CRM push is why sales teams
pay; MeetGeek ships an MCP server. Relay already has a Kanban board in the same
application and does not connect the two. The data is shaped correctly; only the wire
is missing.

**Gap 6. No retrieval across meetings.**

`related.rs` scores relatedness on demand over derived artifacts (topics 0.35,
entities 0.25, type 0.15, speakers 0.15, title 0.10 — title deliberately unable to
carry a match alone). Good design. But there is no *ask*: no equivalent of Granola
Chat, AskFred, or MeetGeek AI Chat. Relay already ships a grounded voice chat over a
LanceDB vault for scribbles — meetings are not in it. Meetings live in a filesystem
tree with no index.

**Gap 7. Evidence and audio are unreachable from the UI.**

`audio_full.wav` is merged at finalize and then never played — there is no `<audio>`
element anywhere in `meetings_v2/` (the `Play` icons are recorder controls). Action
items carry evidence spans internally and the UI cannot jump to them. Whisper Notes'
tap-a-sentence-to-seek is the reference, and Relay is one of the *few* tools in this
set that still has the audio to seek into.

### Tier 3 — quality cannot be improved or trusted over time

**Gap 8. No notes-as-anchor.** Verified: no notes field in
`MeetingRecordingOverlay.tsx`, nothing to type into during a meeting. Granola, anarlog
and Littlebird converged on this independently, and it is the cheapest quality
multiplier available: a user's own three bullets outperform any amount of prompt
tuning at telling Stage A what mattered. It fits Relay's architecture cleanly — human
notes are a *source* artifact, and Stage A already takes structured input.

**Gap 9. No re-transcribe, no audio import.** Chunks are durable, models are
swappable, and no command exists to re-run either. Meetily ships both. For a
Hinglish-first product where a mangled decode is a routine outcome, re-transcribe is
close to essential — and it is nearly free given the durable chunk store.

**Gap 10. No benchmark.** There is a serious Rust test suite (~2,600 lines across
`processing/tests.rs` and `qualify/tests.rs`), but it tests behaviour on synthetic
input. There is no fixture set of real transcripts with hand-checked expected facts,
so a threshold change in `qualify.rs` or a prompt edit in Stage A cannot be *measured*
— only spot-checked. Whisper Notes maintains and publishes exactly this.

### Tier 4 — correctly deferred

**Gap 11. Diarization + enrollment.** Rightly gated behind a consent and retention UI,
per the BIPA analysis in the teardown §2.9. Two things to hold onto: the market signal
is favourable (anarlog and Meetily both paywalled it; only OpenWhispr ships it locally,
via sherpa-onnx with Rust bindings and no Python), and **fixing gap 1 captures most of
the practical value at a fraction of the cost with zero biometric exposure.** Do gap 1
first regardless.

**Gap 12. No meeting detection.** Manual start only. Every competitor triggers from
the calendar. This is downstream of gap 2 — one calendar integration addresses both.

**Gap 13. No ducking.** `soft_mix` prevents clipping but does not attenuate either
source when the other is active. Meetily ships ducking. Lower priority than it looks:
with a per-second channel track (gap 1), *not* ducking is arguably correct, because
attenuation would corrupt the very energy measurement attribution depends on.

---

## 6. Revised sequence

The old teardown's recommended sequence is now partly done. Given v0.13.1, this is the
order that maximises value per unit of work:

**First — unlock what is already built**
1. **Per-second channel energy track** in `capture.rs`, persisted on the segment.
   `qualify.rs` already computes owners and demotes them for lack of this signal.
   Nothing else in this list has a comparable payoff.
2. **Calendar integration.** One piece of work, three payoffs: attendee names for the
   speaker registry, an `initial_prompt` seed of names and terms, and priors for
   `related.rs`. Also delivers gap 12.
3. **Meeting action item → Kanban card.** The objects are already structured; this is
   a wire, not a feature.

**Next — transcription quality where it actually breaks**
4. Overlap window on durable chunks with seam dedup; `initial_prompt` auto-seeded from
   dictionary + attendees + related-meeting topics.
5. Contextual speaker-name inference in Stage A, surfaced as a *suggestion* the user
   confirms — Granola's mechanic, and it reuses the registry's existing
   `Manual` > `Channel` precedence.
6. In-meeting notes field as a source artifact, fed to Stage A as anchors.

**Then — make the archive worth having**
7. Click-to-seek and re-transcribe-with-another-model. Both are wiring `audio_full.wav`
   and the chunk store to existing surfaces.
8. Meetings into LanceDB; extend the existing grounded chat to cross-meeting questions
   with citations back to a transcript span.
9. Fixture benchmark: a handful of real meetings with hand-checked expected facts,
   re-scored on every change to `qualify.rs` or the Stage A/B prompts.

**Then — the regulated feature**
10. sherpa-onnx post-hoc diarization behind a per-meeting toggle with an optional
    speaker-count hint; the Vowen mapping UX (autocomplete from prior names, merge
    split clusters); consent and one-click deletion UI *before* any embedding is
    persisted.

---

## 7. Sources

App-side mechanisms in §3.1, §3.2, §3.5 and §3.6 from vendor documentation and
third-party reviews retrieved 27 August 2026; §3.3, §3.4, §3.7–§3.11 carry over the
sourcing in `meeting_notes_competitive_teardown.md` §Sources. Every claim about Relay
is from the checked-out tree at v0.13.1 and cites its file.

- Fathom overview — https://www.fathom.ai/overview
- Fathom, bot-free notetakers compared — https://www.fathom.ai/learn/bot-free-note-taker
- Fireflies — https://fireflies.ai/
- Fireflies speaker identification — https://workgpt.com/en/faq/does-fireflies-have-speaker-identification
- Jamie, bot-free notetakers — https://www.meetjamie.ai/en/blog/top-10-bot-free-notetakers-for-microsoft-teams-2025
- Jamie review — https://workgpt.com/en/app-reviews/jamie-ai
- MeetGeek vs tl;dv — https://meetgeek.ai/blog/meetgeek-vs-tldv
- Bot-free notetakers, independent roundup — https://fellow.ai/blog/bot-free-ai-note-takers/
