# Rule: Speaker Identification & Labelling

**Applies to:** the speaker-attribution stage of the pipeline, and the model pass that maps voice clusters to real names.
**Consumed by:** `meeting_transcript_summary.md` and `meeting_action_items_tasks.md`, both of which depend on knowing who spoke.

---

## 0. Why this runs before anything else

A to-do without an owner is a note. The difference between "someone will send the email list" and "I need to send the email list by Friday" is the entire value of the feature, and it is decided here, not in the task-extraction prompt. No amount of prompt tuning recovers an owner the transcript never recorded.

Two things are deliberately kept separate throughout:

- **Diarization** — separating the audio into distinct voices. Automatic, no human involved, produces `Speaker 1`, `Speaker 2`. Answers *how many people spoke and when*.
- **Identification** — attaching a real name to a voice. Requires a human once, then persists. Answers *who*.

Conflating them is the most common design error. Diarization can succeed completely while identification fails, and the UI must be able to show that state honestly.

> [!NOTE]
> **What is implemented today** (v0.33.0). Rungs **1**, **4**, **5** and **6**
> are built. Rungs 2 and 3 are not.
>
> **v0.31.0 shipped rung 4 broken**, and the correction is worth reading before
> the description below: a three-person meeting reported one speaker, because
> the split threshold was calibrated against synthetic voices an octave apart
> and sat above every distance real speech produces. See
> `docs/meetings/TRANSCRIPT_AND_SPEAKER_REBUILD.md` §2.0. Two things changed as
> a result — the count is now chosen by scoring partitions rather than by any
> threshold, and §3's identification is a *comparison* of three methods over one
> recording rather than a single implementation judged by holding meetings.
>
> - **Rung 1 — channel** (`processing/speakers.rs`): microphone input is the
>   local user (`speaker_me`), system audio is everyone else. Always on, and it
>   is not overridable by a later rung *for the local user*, because the channel
>   is what the audio device reported and a cluster is an inference about it.
> - **Rung 4 — diarization** (`meetings_v2::diarize`): the recorded chunk WAVs
>   are cut at Whisper's own utterance boundaries, characterised, and clustered
>   into distinct voices, which is what splits the single remote bucket into
>   `Speaker 1`, `Speaker 2`, … Features are MFCC statistics plus a pitch
>   estimate, **not** a neural embedding, so no biometric data is created:
>   features live for the duration of one run and are never stored or matched
>   across meetings. Three engines implement the decision — channel only,
>   whole-recording clustering, and a registry built live as chunks land — and
>   §3's identification command can run all of them over one recording for
>   comparison.
>
>   The limitation is measured rather than asserted: two deliberately similar
>   voices score the same as one voice wandering across a meeting, so they
>   cannot be told apart at all, and Relay merges them rather than risk
>   inventing a speaker. §2.2's expected-speaker count is the override, and it
>   recovers the correct split. `maybe_later.md` item 18 tracks the neural
>   embedding that would resolve it.
> - **Rung 5 — contextual inference** (`processing/names.rs`): deterministic
>   patterns over self-introductions and direct address, deliberately *not* the
>   model pass §4 specifies. A model asked who Speaker 2 is will always produce
>   a plausible answer, and the failure mode here is inventing a name. A
>   self-introduction binds to the speaker who gave it; a direct address is
>   recorded as a mentioned participant and never bound to a voice.
> - **Rung 6 — user correction** (`processing/directives.rs`): a `SpeakerName`
>   directive in the meeting's notes renames the speaker in the registry. It
>   does not yet feed rungs 2 or 3, because neither exists.
>
> **Rung 2 (enrolled voiceprint)** is absent because it is the feature that
> would create biometric data, and §6's consent, management and deletion
> requirements have to land with it — `maybe_later.md` item 18.
> **Rung 3 (calendar attendees)** is absent because Relay has no calendar
> integration; the participant list and the expected-speaker hint are the
> surfaces it would fill — `maybe_later.md` item 19.
>
> §2.1's settings surface accordingly exposes "Speaker identification:
> Automatic / Off", "Separate individual speakers" (rung 4, **on** by default —
> see below) and an expected-speaker count. The voice-library toggles are still
> deliberately absent.
>
> One deliberate departure from §2.1: "Identify individual speakers" defaults
> **on**, where this document says off for CPU cost. The measured cost is a few
> hundred milliseconds over a whole meeting, run once after recording ends; the
> alternative default is a twenty-person meeting reporting one remote speaker,
> which is the failure this rung was built to fix. §2.2's per-meeting override
> exists as the **Identify speakers** action in the conversation tab, with its
> own expected-speaker field.
>
> §2.2's **In person** marking is built as a setting: it disables the local-user
> inference, because the channel split that identifies the person at this
> machine means nothing when every voice arrives on one microphone, and a guess
> there mislabels whoever it lands on.
>
> §2.3's states are all reachable: a name a person typed is marked confirmed, a
> cluster nobody has named shows as `Speaker N`, a roster the clustering is
> unsure of is reported as provisional, and a stretch that could not be placed
> is left blank.


---

## 1. The attribution ladder

Resolve each segment by walking these in order. Stop at the first rung that produces a confident answer.

| Rung | Source | Confidence | Cost |
|---|---|---|---|
| 1 | **Channel** — mic stream vs system stream | Certain for "me" | Free |
| 2 | **Enrolled voiceprint** — embedding match above threshold | High | Cheap |
| 3 | **Calendar attendees + conferencing display names** | High when only one candidate fits | Free |
| 4 | **Diarization cluster** — distinct voice, unnamed | Certain grouping, unknown identity | Moderate |
| 5 | **Contextual inference by the model** — self-introductions, direct address | Medium at best | Cheap |
| 6 | **User correction** | Certain, and feeds rungs 2 and 3 | One tap |

**Rung 1 is mandatory and always on.** Microphone input is the local user; system audio is everyone else. This alone resolves every first-person commitment in the transcript — which is the majority of to-dos that matter to the person using the app. It requires no model, no consent flow, and no ONNX runtime. Ship it before anything else on this list.

Rungs 2 and 4 are optional and gated behind settings. Rung 5 is a model pass, specified in §4.

---

## 2. Setting surface

### 2.1 Global settings — Settings › Meetings › Speakers

| Setting | Type | Default | Notes |
|---|---|---|---|
| Separate my voice from others | toggle | **On** | Rung 1. Cheap and non-biometric. Should rarely be turned off. |
| Identify individual speakers | toggle | **Off** | Enables diarization (rung 4). Off by default because it costs CPU and battery. |
| Run automatically after each meeting | toggle | Off | Requires the above. Otherwise diarization is manual per meeting. |
| Remember voices across meetings | toggle | **Off** | Enables the speaker library (rung 2). **Creates biometric data — see §6.** Must never default to on. |
| Use calendar attendees | toggle | On | Rung 3. |
| Default expected speakers | number, or Auto | Auto | Passed as a clustering hint. |
| Manage speaker library | button | — | List, rename, merge, delete voiceprints. Required by §6. |

### 2.2 Per-meeting override

The global settings are defaults, not commitments. Every recording shows an inline control at start and after the fact:

- A diarization toggle visible when starting a recording, not buried in preferences.
- An **expected speaker count** field, defaulting to the calendar attendee count when known. A correct hint measurably improves clustering.
- For an in-person meeting where the channel split is meaningless (everyone through one mic), the user can mark the meeting **In person**, which disables rung 1 and makes diarization the primary path.

### 2.3 States the UI must be able to show

Do not collapse these into "unknown". They call for different user actions.

| State | Meaning | Label |
|---|---|---|
| Confirmed | User named this cluster, or an enrolled voiceprint matched | Name, with a confirmed marker |
| Inferred | Model assigned a name from context or calendar | Name, visually distinguished as unconfirmed |
| Unnamed cluster | A distinct voice was isolated but not identified | `Speaker 2` |
| Channel only | No diarization run | `Me` / `Them` |
| Unattributed | Segment could not be assigned to any cluster | no label |

Otter's convention of a check mark on manually tagged speakers is worth copying directly — users need to know at a glance which names to trust.

---

## 3. Executable function

Speaker identification is a **command the user can invoke**, not only a background step. It re-runs independently of transcription, because the user often decides they need speakers only once they see the notes.

### 3.1 Invocation points

- Command palette: **Identify speakers in this meeting**
- Meeting detail view: a button in the transcript pane header
- Bulk: **Identify speakers** across a selection of past meetings
- Automatic post-recording, when that setting is on

### 3.2 Behaviour

1. Reject if the audio has been discarded — say so plainly rather than failing silently.
2. Run segmentation and embedding over the stored audio. Post-hoc, not live; the recording must have ended.
3. Cluster embeddings, using the expected-speaker hint if supplied.
4. Match each cluster against the enrolled library, if that setting is on.
5. Emit unmatched clusters as `Speaker N`.
6. Run the contextual inference pass in §4.
7. Open the mapping UI (§5).
8. Persist labels against segment ranges. **Never rewrite the transcript text** — attribution is a separate layer, so it can be re-run, corrected, or discarded without touching the source.
9. Invalidate the derived summary and to-dos, and offer to regenerate. Attribution changes ownership; stale to-dos with the wrong owner are worse than none.

### 3.3 Idempotency

Re-running must be safe and must never silently overwrite a confirmed name. Confirmed labels survive; only inferred and unnamed clusters are recomputed.

---

## 4. Contextual inference pass (the model's job)

Runs after clustering, with clusters and any known attendee list as input. This is the only part of speaker identification a language model should touch.

**Input:** the normalized transcript with cluster labels (`Speaker 1`, `Speaker 2`, …), the calendar attendee list if available, and the speaker library names.

**Output:** JSON only.

```json
[{"cluster": "Speaker 2", "name": "Pranjal", "confidence": "high", "evidence": "addressed by name at 00:14:22"}]
```

### Rules

1. **Assign a name only on direct textual evidence.** Two kinds count: self-introduction ("this is Pranjal"), and direct address immediately before or after that cluster speaks ("Pranjal, you can take him through the other pointers" → the next speaker is Pranjal).
2. **Never assign a name from topic or role.** That the cluster talks about finance does not make them the finance lead. This is the main hallucination route and it produces confident, wrong, plausible answers.
3. **Never assign a name that is not in the attendee list or spoken in the transcript.**
4. **One name per cluster, one cluster per name.** If two clusters both look like the same person, emit neither name and flag them as a possible merge — that is a UI decision, not a model decision.
5. **Abstain freely.** Omitting a cluster is a correct answer. An unnamed `Speaker 3` costs one tap; a wrongly named one silently misassigns every to-do that person owns.
6. **Confidence must be honest.** `high` requires a self-introduction or unambiguous direct address. Everything else is `medium`. Never emit `high` for an inference from a single ambiguous line.
7. **Evidence is mandatory** — a timestamp or short locator for every assignment. No evidence, no assignment.
8. **Translated passages are not evidence.** Machine-translated Hindi/Hinglish detaches pronouns and mangles names; do not assign identity from those stretches.
9. **The mic channel is never inferred.** Rung 1 already resolved it. Do not second-guess it.

Anything the model returns enters the UI as **inferred**, never confirmed. Only a human confirms.

---

## 5. Correction UI

Correction is the feature, not an error path. Three operations, all required:

**Rename.** Tap a label, type a name. Autocomplete from names used before across all meetings, plus other speakers in the current meeting, so recurring attendees are one tap. Renaming propagates to every segment in that cluster immediately.

**Merge.** One person routinely gets split across two clusters — different mic distance, a cold, joining twice. The user selects two labels and collapses them. This is the single most common diarization failure and there must be a one-action fix.

**Split.** Rarer, and more expensive to implement. Two people clustered as one. Acceptable to defer, but the UI should at least let the user reassign individual segments.

Every correction feeds back: into the speaker library if enrollment is on, into the name autocomplete regardless, and into the glossary as a known proper noun so ASR stops mangling it.

---

## 6. Biometric data: consent and retention

Rungs 1, 3, 4, and 5 do not create biometric data. Clustering voices within a single recording separates them without building a persistent identity template.

**Rung 2 does.** A stored voiceprint that identifies a person across recordings is a biometric identifier under Illinois BIPA, and generally special-category data under GDPR Article 9. A 2026 lawsuit against Fireflies.ai turns on exactly this: creating and retaining voiceprints of meeting participants — including people unaware the tool was running — without notice, written consent, and a published retention policy.

Local-only storage is a strong position but not an exemption. Required before the speaker library ships:

- **Opt-in, off by default.** Never enabled by an update.
- **Explicit disclosure at the moment of enrollment**, naming what is stored and for how long — not buried in a privacy policy.
- **Per-person deletion**, with a visible list of everyone in the library.
- **Local storage only.** No voiceprint leaves the device, ever, including in backups and sync.
- **A stated retention period** with automatic expiry after a period of no matches.
- **A plain statement that Relay never transmits or trains on voiceprints.**

This is an engineering constraint, not legal advice — confirm specifics with counsel before shipping enrollment.

---

## 7. Contract with the to-do pass

Attribution exists to serve ownership. The to-do extractor receives, for every segment, one of:

- `me` — resolved by channel or by the user's own enrolled voice
- a named person
- an unnamed cluster (`Speaker 2`)
- `unknown`

And applies these rules:

1. A first-person commitment from a `me` segment produces a to-do owned by **the user**.
2. A first-person commitment from a named segment is owned by **that person**.
3. A first-person commitment from an unnamed cluster is owned by `Unassigned`, and carries the cluster id so the to-do re-owns itself automatically once the user names that speaker.
4. Assignment to a named person ("can you review the guide?") is owned by the addressee, if they accepted — regardless of who was speaking.
5. **Never guess an owner from an unnamed cluster.** `Unassigned` is the correct answer.

The to-do output is grouped so the user's own items come first, under a heading that says so. The practical question a person asks after a meeting is *what do I have to do*, and that should not require scanning past four other people's work.

When attribution is re-run or corrected, to-do ownership is recomputed. Confirmed manual edits to an owner survive.

---

## 8. Self-check

- [ ] Channel split (`me` / `them`) ran, and was not overridden by inference.
- [ ] Every name traces to a self-introduction, a direct address, an enrolled match, or a user action.
- [ ] No name was assigned from topic, role, or seniority.
- [ ] No name appears that was absent from both the attendee list and the transcript.
- [ ] Every inferred name carries evidence and an honest confidence.
- [ ] Inferred and confirmed are visually distinguishable in the UI.
- [ ] No cluster received a name from a machine-translated passage.
- [ ] The transcript text itself was not modified — only the attribution layer.
- [ ] Downstream summary and to-dos were invalidated after any attribution change.
- [ ] If enrollment is on, consent was captured and the library is deletable.
