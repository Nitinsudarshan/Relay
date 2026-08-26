# Competitive teardown: how meeting-notes apps build the pipeline

**Prepared for:** Relay (local-first, bot-free, Tauri/Rust + React)
**Date:** 27 August 2026
**Scope:** capture, transcription, speaker handling, summarization, and task extraction across eight comparable products, followed by a gap analysis from Relay's position.

---

## 1. Method and framing

Each app is assessed on the six pipeline stages Relay cares about: capture, transcription, speaker attribution, comprehension, generation, and correction. The question throughout is not "is this app good" but "what did they solve that Relay has not, and is their solution portable to a local-first Windows app."

Two products are close architectural siblings and worth reading first: **Meetily** (Rust/Tauri, Ollama) and **OpenWhispr** (local Whisper + sherpa-onnx, ships voice fingerprinting today). Two are the UX benchmark: **Granola** and **Circleback**. **Otter** is the reference implementation for speaker enrollment. **Vowen** has the best correction UX. The rest fill in context.

---

## 2. Individual teardowns

### 2.1 Granola

**Capture.** Bot-free, device-level: microphone plus system audio, on macOS and Windows. Works with any conferencing app because it never touches a platform API. Audio is transcribed in real time and then deleted — only the transcript and notes persist.

**Speaker handling.** This is the most instructive part for Relay. Where proper speaker tags are unavailable, Granola falls back to labelling the transcript **"Me" and "Them"**, derived directly from microphone input versus system audio. On top of that, the model infers speaker names from contextual clues in the transcript, and the user can correct a name inline in the notes or by asking Granola Chat to fix it. The mobile app does on-device speaker recognition for in-person meetings.

**Comprehension.** Note-first. You type rough notes during the call; at the end, "Enhance notes" uses your notes as anchors and searches the transcript to fill in context around them. If you type nothing, you get a generic summary. Output shape is controlled by templates.

**Retrieval.** Granola Chat answers questions across all meetings with source-linked citations.

**Worth taking:** the Me/Them channel fallback; contextual name inference with one-click correction; notes-as-anchor; source-linked citations.
**Not applicable:** transcription is cloud, and audio deletion is their privacy story — Relay's is stronger by default.

### 2.2 Littlebird

**Capture.** Same bot-free system-audio model, but the notetaker is one feature inside a broader context-memory product that also reads the active window on your screen.

**The differentiator.** "Prep for meeting" pulls context from past meetings, emails, and company history before a call starts. Meeting notes are not isolated documents — they feed a memory layer you can query.

**Integration.** Ships an MCP server so other AI tools (Claude, Cursor) can query the meeting memory. Answers include links to the sources used.

**Worth taking:** pre-meeting context injection is the single highest-leverage idea here. Loading prior meetings with the same attendees gives you the participant list, the project glossary, and the open action items from last time — all before the model sees a word of transcript. The MCP server is also a natural fit for a developer-facing tool.
**Not applicable:** screen reading. Wrong privacy posture for Relay.

### 2.3 Otter

**Capture.** Cloud-based, calendar-connected, bot-based (OtterPilot joins Zoom/Meet/Teams). The opposite of Relay architecturally.

**Speaker handling — the reference implementation.** Otter separates two things Relay must also separate:

1. *Diarization* runs automatically after recording stops and produces generic `Speaker 1`, `Speaker 2` labels — meaning "a distinct voice was detected but not matched to anyone known."
2. *Identification* happens when the user tags a speaker once. That tag trains a voiceprint stored against the account and shared across the workspace, so the same person is auto-labelled in future meetings. Manually tagged speakers get a green check to distinguish confirmed from inferred.

Tagging one `Speaker #` propagates to every segment of that cluster in the conversation. Users can pre-enrol by importing old recordings and labelling them. A separate custom-vocabulary list handles jargon and names.

For scheduled meetings where participant names are known from the calendar, accuracy is materially better. Reported accuracy sits in the high-80s to mid-90s under good conditions and degrades with crosstalk and similar voices — speaker ID is widely described as the weakest link in the whole category.

**Worth taking:** the entire enrollment loop, essentially unchanged. Diarize → generic labels → one human tag → persistent embedding → auto-label next time. Also the workspace-level speaker library and the confirmed-vs-inferred visual distinction.

### 2.4 Circleback

**Capture.** No-bot desktop and mobile, plus optional bot. Handles in-person.

**The differentiator.** Structured output is the product. Action items are extracted, **assigned to owners automatically**, and pushed into other tools via automations and webhooks. Automatic speaker identification by name is what makes the assignment possible — the two features are inseparable. Over 100 languages, with a stated focus on technical terms and accents.

**Worth taking:** treating owner assignment as a first-class output rather than a string in a bullet, and having a webhook/automation exit path. Relay's to-do checkboxes are currently a dead end — nothing leaves the app.
**Caveat:** independent reviews flag that transcription in echoey or noisy in-person settings still needs checking before you trust the output.

### 2.5 Hyprnote → anarlog / char

**Status note.** Hyprnote was renamed to char, then the project split: `char` is the team's commercial productivity app, and **anarlog** is the MIT-licensed, local-first open-source notetaker that continues the original Hyprnote line. If you were referencing Hyprnote as a benchmark, anarlog is the current repo.

**Architecture.** Local-first, offline by default, Whisper plus a local LLM, app data in local SQLite, bring-your-own-model via Ollama or cloud APIs. Extension system explicitly modelled on VS Code. Integrates with Apple Calendar, Contacts, and Obsidian.

**The telling detail.** In the current productisation, **speaker identification sits in the paid hosted tier**, alongside hosted transcription and calendar connections. The local free tier does not have it.

**Worth taking:** the plugin architecture, the template system for summary shape, and Calendar + Contacts integration as a name source.
**What it tells you:** on-device speaker ID is hard enough that the closest open-source competitor moved it behind a paywall. That is both a warning about effort and an opening — shipping it locally and free is a real differentiator.

### 2.6 Meetily

**Architecture.** The closest match to Relay's stack: single self-contained Tauri app, Rust backend, JS frontend, Parakeet/Whisper for transcription, Ollama recommended for summarization with Claude/Groq/OpenRouter/OpenAI as alternatives. Runs on macOS and Windows.

**Capture detail worth copying.** Simultaneous microphone and system audio capture with **intelligent ducking and clipping prevention**. This is a real engineering problem Relay will hit — when both streams are live, the system stream can clip and the mic can bleed.

**Other features.** Import existing audio files to transcribe, and re-transcribe any recorded meeting with a different model or language. Hardware acceleration enabled at build time.

**Speaker handling — read the fine print.** Diarization is prominent in the marketing copy, but the repo notes it was **planned for the PRO tier**, not shipped in the community edition. Same pattern as anarlog.

**Worth taking:** dual-capture with ducking; re-transcribe-with-a-different-model as a first-class action (invaluable when you find a Hinglish meeting was mangled); the provider abstraction layer.

### 2.7 OpenWhispr

**Architecture.** Electron, React 19, TypeScript, better-sqlite3, whisper.cpp, **sherpa-onnx**, shadcn/ui. Started as dictation, expanded into meeting transcription and notes. MIT licensed. macOS, Windows, Linux.

**The important bit.** It ships **live speaker identification and voice fingerprinting** today, running locally through sherpa-onnx and ONNX Runtime, with all core features — transcription, reasoning, diarization, semantic search — working against either local or cloud models. There is a documented platform caveat: on Intel Macs, live speaker ID and fingerprinting are unavailable because ONNX Runtime stopped shipping macOS x86_64 binaries at 1.24.

**Worth taking:** this is your concrete implementation path. sherpa-onnx has Rust bindings, runs on Windows, and gives you both segmentation and speaker embeddings without a Python dependency — which matters because pyannote would drag a Python runtime into a Tauri app. The platform caveat is also a useful reminder to feature-detect rather than assume.

### 2.8 Vowen and Whisper Notes (correction UX and honesty)

**Vowen** has the best-documented diarization UX in the category:

- A diarization toggle presented at meeting start, not buried in settings.
- An optional **expected number of speakers** hint to improve clustering accuracy.
- Generic labels after transcription, then a mapping step to real names.
- **Name autocomplete** drawn from names used before across all notes, plus other speakers in the current note, so recurring attendees are one tap.
- **Merge for split speakers** — when one person gets clustered as both Speaker 2 and Speaker 4, you collapse them into one.

It also documents its platform limits plainly: on-device pyannote is macOS only; Windows requires a diarization-capable cloud model.

**Whisper Notes** runs diarization fully on-device and is unusually honest about it. Labels appear after the recording ends, not during. Tapping a sentence seeks playback to that moment. They maintain a fixed two-file benchmark scored at 10ms resolution against hand-checked ground truth, publish the numbers, and describe the one failure mode they could not engineer away — two similar voices in a reverberant room with frequent short interjections.

**Worth taking:** every element of Vowen's mapping UX, especially merge-split-speakers, which is the single most common diarization failure. From Whisper Notes: post-hoc rather than live diarization, click-to-seek, and keeping a small fixed benchmark you re-run on every change.

### 2.9 Legal note: voiceprints are regulated biometric data

This changes how the feature must be designed, not whether to build it.

Illinois BIPA lists voiceprints as biometric identifiers and allows individuals to sue directly, with fixed statutory damages and no requirement to prove harm. A 2026 lawsuit against Fireflies.ai alleges exactly this: that its speaker-recognition function creates and retains voiceprints of meeting participants — including people unaware the tool was running — without the notice, written consent, and retention policy BIPA requires. GDPR follows a similar line: a recording is ordinary personal data, but once technical processing creates a voiceprint for unique identification, Article 9 special-category rules generally apply.

The distinction that matters technically: **diarization that only separates voices within a single recording is not the trigger. Persisting an identity template across recordings is.** That is precisely the line Relay would cross by adding a speaker library.

Relay's position is defensible — local-only storage, no cloud transmission, no vendor data plane — but the design must still be: enrollment strictly opt-in and per-person, never default-on, with visible retention and one-click deletion of any stored voiceprint.

---

## 3. Cross-cutting patterns

Ten things that show up in nearly every mature implementation:

1. **Bot-free dual-stream capture is table stakes.** Mic and system audio, simultaneously, with ducking.
2. **Two-tier speaker model.** A cheap always-on channel split (Me/Them), plus optional real diarization. Nobody relies on diarization alone.
3. **Diarization and identification are separate features.** Clustering is automatic; naming is human, once.
4. **Names persist across meetings** through an embedding store — this is what turns a one-time chore into a system that improves.
5. **Calendar and contacts are the primary name source**, not the audio. Accuracy jumps when participants are known in advance.
6. **Diarization runs post-hoc**, after the recording ends. Live labelling is a stretch goal.
7. **Correction UX is a first-class feature**: rename, merge split clusters, autocomplete from prior names.
8. **Custom vocabulary is a shipped product feature**, not an internal implementation detail.
9. **Human notes as anchors** beat better prompts. Granola, anarlog, and Littlebird all converged on this independently.
10. **Everything links back to a source** — a transcript span, a timestamp, a seekable moment in the audio.

---

## 4. Gap analysis for Relay

| # | Capability | Best reference | Relay today | Priority |
|---|---|---|---|---|
| 1 | Deterministic transcript preprocessor (loop collapse, tag stripping) | — nobody documents this; it's the silent prerequisite | Missing — asked of the LLM | **P0** |
| 2 | Me/Them channel split | Granola | Missing | **P0** |
| 3 | Calendar + contacts as name/metadata source | Otter, anarlog, Littlebird | Missing | **P0** |
| 4 | Custom vocabulary / glossary | Otter | Referenced in rules, not built | **P0** |
| 5 | Post-hoc diarization, on-device | OpenWhispr (sherpa-onnx), Whisper Notes | Missing | **P1** |
| 6 | Speaker enrollment + persistent library | Otter | Missing | **P1** |
| 7 | Correction UX: rename, merge, autocomplete | Vowen | Missing | **P1** |
| 8 | Owner assignment as structured data, not a string | Circleback | Missing | **P1** |
| 9 | Two-stage comprehend-then-write summarization | — nobody documents it; inferable from Granola's quality | Specified in rules, not implemented | **P1** |
| 10 | Ducking / clipping prevention on dual capture | Meetily | Unknown | **P1** |
| 11 | Evidence links back to transcript timestamp | Granola, Whisper Notes | Missing | **P2** |
| 12 | Re-transcribe an existing recording with a different model | Meetily | Missing | **P2** |
| 13 | Pre-meeting context injection from past meetings | Littlebird | Missing | **P2** |
| 14 | Summary templates the user can pick or write | Granola, anarlog | Rules files are fixed | **P2** |
| 15 | Fixed benchmark re-run on every change | Whisper Notes | Missing | **P2** |
| 16 | Notes-as-anchor during the meeting | Granola, anarlog, Littlebird | Missing | **P2** |
| 17 | Webhook / automation exit path | Circleback | Missing | **P3** |
| 18 | MCP server over meeting memory | Littlebird, OpenWhispr | Missing | **P3** |
| 19 | Plugin/extension architecture | anarlog | Missing | **P3** |
| 20 | Biometric consent + retention UI | — Fireflies' cautionary tale | Not yet needed; required before #6 ships | **P1, gated** |

### Where Relay can genuinely win

Three openings the research makes visible:

**Local speaker identification, free.** Both open-source competitors — anarlog and Meetily — put diarization behind a paid tier. OpenWhispr ships it locally and is the exception. If Relay ships on-device diarization plus enrollment on Windows, it occupies ground the closest competitors have vacated.

**Degraded and multilingual transcripts.** Nobody in this set handles Hinglish-through-Whisper well. Circleback claims accent handling; everyone else quietly assumes clean English. Your Stage 0 normalizer plus a glossary plus source-language transcription is a real differentiator for the Indian market specifically, and it is work the incumbents have no incentive to do.

**Windows-first.** Vowen's on-device diarization is macOS only. Whisper Notes is Apple-only. anarlog leads with Homebrew. Relay running properly on a Dell G15 is not a small thing.

---

## 5. Recommended sequence

**Now (unblocks everything else)**
1. Stage 0 normalizer in Rust; fix Whisper decode params (`condition_on_previous_text=False`, compression-ratio threshold, temperature fallback).
2. Me/Them channel split — 80% of speaker value for a fraction of the effort.
3. Calendar integration for title, date, and attendee list.
4. Glossary table, seeded from attendees and corrections.

**Next**
5. sherpa-onnx diarization, post-hoc, behind a per-meeting toggle with an optional speaker-count hint.
6. Speaker mapping UI: generic labels → name autocomplete → merge split clusters.
7. Structured JSON output with owner and evidence span; render checkboxes from it.
8. Two-call comprehend-then-write summarization.

**Then**
9. Speaker enrollment and persistent library — only after the consent and deletion UI exists.
10. Benchmark harness; click-to-seek; re-transcribe action.

---

## Sources

- Granola transcription docs — https://docs.granola.ai/help-center/taking-notes/transcription
- Granola, what is an AI notepad — https://www.granola.ai/blog/what-is-an-ai-notepad
- Littlebird meeting notes — https://littlebird.ai/features/meeting-notes
- TechCrunch on Littlebird — https://techcrunch.com/2026/03/23/littlebird-raises-11m-to-capture-context-from-your-computer-so-you-can-query-your-data/
- Otter, tagging speaker names — https://help.otter.ai/hc/en-us/articles/360048465453-Tagging-speaker-names-in-a-conversation
- Otter, speaker identification best practices — https://help.otter.ai/hc/en-us/articles/37817241040535-Best-Practices-to-Maximize-Speaker-Identification
- anarlog (formerly Hyprnote) — https://github.com/fastrepl/anarlog and https://anarlog.so/
- Meetily — https://github.com/Zackriya-Solutions/meetily
- OpenWhispr — https://github.com/OpenWhispr/openwhispr
- Vowen diarization docs — https://docs.vowen.ai/meeting-notes/diarization
- Whisper Notes on-device diarization — https://whispernotes.app/blog/whisper-speaker-diarization
- Circleback review — https://tooliverse.ai/tools/circleback
- Epstein Becker Green on the Fireflies BIPA suit — https://www.ebglaw.com/insights/publications/ai-meeting-assistants-and-biometric-privacy-lessons-from-the-fireflies-ai-lawsuit
