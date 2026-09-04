# Meetings: transcript health and speaker identity

**Traced from**: v0.31.0. Read as history, not as instructions — where this
disagrees with the source, the source is right.

This records why four reported failures happened and what replaced the code
that caused them. It is the companion to
[`MEETINGS_INTELLIGENCE_AUDIT.md`](MEETINGS_INTELLIGENCE_AUDIT.md), which
traced the derived-intelligence stages, and to
[`SUMMARY_QUALITY_REBUILD.md`](SUMMARY_QUALITY_REBUILD.md), which traced the
summary. This one is about the layer underneath both: whether the transcript is
real, and whether the speakers are.

---

## 1. Four minutes of "Thank you."

### What was reported

A 44-minute meeting, 90 chunks, 5,661 words. Chunks 11 through 19 — 240
seconds of it — contained nothing but `Thank you.` repeated, several hundred
times. The rest of the transcript was fine.

### Why it happened

Two independent bugs, and the second is why it lasted nine chunks rather than
one.

**The silence gate could not tell a fan from a conversation.** The worker
computed one RMS mean across each 30-second chunk and compared it to a fixed
threshold of 0.005:

```rust
let sum_sq: f32 = chunk.samples.iter().map(|&s| s * s).sum();
let rms = (sum_sq / sample_count.max(1) as f32).sqrt();
if sample_count < 16_000 / 2 || rms < SILENCE_RMS_THRESHOLD { /* skip */ }
```

Steady room tone — a fan, an air conditioner, an open microphone in a quiet
room — sits comfortably above 0.005 for the entire window while containing no
voice whatsoever. So the chunk was decoded. Whisper is a sequence model with no
way to output "nothing was said"; handed thirty seconds of tone it emits the
likeliest continuation it knows, and for a model trained on captioned video
that is subtitle boilerplate.

The measurement was wrong in kind, not in degree. No threshold on a 30-second
mean can separate flat noise from speech, because the mean discards exactly the
information that distinguishes them: speech is *peaky*. `profile_speech`
measures voiced time at 20 ms resolution against the chunk's **own** noise
floor — the 10th-percentile frame energy — so a frame must sit about 9.5 dB
above the room to count. Hiss is flat, so no frame clears it and the chunk
reads as silent however loud its mean. Speech clears it easily.

**The initial prompt carried the loop forward.** The worker passed the tail of
each chunk's text into the next decode as Whisper's `initial_prompt`, with this
comment:

> Every chunk is decoded on a fresh Whisper state, which is what keeps a
> decoder loop in one chunk from poisoning the rest of the meeting. […] a loop
> cannot propagate, because the prompt is capped and the state is still
> discarded.

The comment names the exact mechanism and then draws the wrong conclusion.
Whisper reads `initial_prompt` as *preceding text*, not as configuration.
Prompting chunk *n+1* with `"Thank you. Thank you."` makes the same
continuation overwhelmingly likely — the discarded decoder state is beside the
point. `carry_forward` now refuses any tail that `is_safe_as_prompt` rejects,
and the chain breaks on a gate, a failure, or a rejection.

### What was added

`transcript_health` holds both defences, and they are deliberately separate:
one runs before the decode and one after.

`assess` rejects text no plausible speech could have produced:

| Rule | Signal | Why it is safe |
|---|---|---|
| `RepetitionLoop` | One phrase repeated adjacently, covering ≥85% of the words | A loop is self-evident; the audio does not get a vote |
| `NoSpeech` | Whisper's own probability ≥0.85 | The model's own certainty |
| `FillerOverSilence` | Known subtitle boilerplate, under a second of voice (or the model doubting it over a quiet window) | Absolute voiced time, not a window ratio — see §1.1 |
| `ImplausibleRate` | More than 8 words per voiced second | Auctioneers reach 6 |

The 85% coverage bar is high on purpose. `"we we we should ship it we should
ship it we should ship it tomorrow morning"` is a decoder *stutter* around real
content: the loop covers two thirds of it, and the right treatment is the
normalizer collapsing the repetition and keeping the sentence — which it
already did. Only a segment that is almost entirely one repeated phrase has no
content to protect. A unit test pins that case.

### 1.1 The one rule that was wrong first

`FillerOverSilence` was originally written against the *ratio* of voiced time
to window length. A test caught it: a 30-second chunk holding 1.8 seconds of
speech and the words `"Thank you."` was rejected, because 6% is a low ratio.

But that is a real thing people say at the end of a call. The ratio is not the
signal — **absolute voiced time** is, because the pre-decode gate already
guarantees at least half a second of voice reached the decoder. Under a second
of voice, there was nothing there to say it; two seconds of voice and the words
"thank you" is somebody thanking somebody.

The asymmetry decides it. A stray polite phrase costs a summary nothing; lost
speech is unrecoverable. Both directions are now pinned by tests.

### 1.2 The thirteen meetings that already exist

Those transcripts hold their hallucinated runs stored as `Success`.
`transcript.jsonl` is immutable by design *and* is the evidence the failure
happened, so rewriting it was never an option. `screen_raw_segments` runs the
same screen on the way **out**: the derived transcript comes out clean, the
meeting reports what was withheld and how many words, and not a byte of source
data changes.

For a pre-v2.6 segment there is no measured voiced time to compare against, so
the screen assumes the whole span was voiced. That disables the rules needing
audio evidence and leaves the self-evident ones — which is the conservative
choice, and the right one: a decoder loop is a decoder loop whatever the audio
was.

### 1.3 A rejection is recorded, not hidden

`TranscriptSegmentStatus::Rejected` is distinct from `Empty`. `Empty` means the
recorder heard nothing and never decoded; `Rejected` means Whisper produced
something and it was thrown away. Conflating them would hide the failure this
work exists to make visible.

The rejection carries the reason and the discarded text (capped at 320
characters). The Raw Transcript tab shows both, the second behind a disclosure.
A chunk that silently vanishes is indistinguishable from one that never
existed, and this tab is the pipeline's diagnostic source.

---

## 2. One "Speaker 1" for a room of twenty

### What was reported

A 44-minute meeting with about twenty people in it. The Speakers row showed one
chip: `Speaker 1`.

### Why it happened

`attribute_speakers` had exactly two outcomes by construction:

```rust
fn implied_speaker_id(self) -> Option<&'static str> {
    match self {
        Self::Mic => Some(SPEAKER_ID_ME),
        Self::System => Some(SPEAKER_ID_REMOTE),
        Self::Mixed | Self::Unknown => None,
    }
}
```

This is rung 1 of `Meeting-rules/meeting_speaker_identification.md` and it is
correct as far as it goes — the microphone really is the local user. It is also
the *whole* of what was built, and it cannot represent more than two people,
because there are only two capture channels. Every to-do owned by anyone other
than the local user resolved to the same anonymous bucket.

### What was added

`meetings_v2::diarize` — rung 4. It reads the chunk WAVs the recorder already
wrote, cuts them at the utterance boundaries Whisper already reported, computes
a voice feature vector per utterance, and clusters them. Nothing is
re-recorded, nothing is re-transcribed, and `transcript.jsonl` is not touched:
attribution is a separate layer keyed by utterance id.

**Features are classical, not neural.** MFCC mean and standard deviation over
each utterance, plus a median pitch estimate, computed from a radix-2 FFT and a
mel filterbank written in `features.rs`. A neural speaker embedding (x-vector,
ECAPA) would be materially better; it would also mean shipping an ONNX runtime,
a model download with its own licence question, and the consent flow §6 of the
speaker rules requires — because an embedding stored across meetings *is*
biometric data, which the classical path never creates.

What that buys: separating voices that differ in vocal tract length or
register, and counting how many distinct voices a stretch holds. What it does
not buy: telling two similar voices apart on one channel, or matching a voice
across meetings. Both are written into the module's docs, and
`DiarizationReport::well_separated` is how the UI knows which situation it is
in. `maybe_later.md` item 18 holds the upgrade path.

The zeroth cepstral coefficient is dropped, which is not incidental: it is
total log energy, so keeping it would make somebody leaning toward the
microphone read as a new person.

### 2.1 Two designs that were wrong before the third

**Cosine distance on per-meeting normalized features.** The first draft
centred and variance-normalized every dimension across the meeting, reasoning
that the room, the microphone and the codec are a shared component that should
be removed. Two tests failed: one voice split into two, and a marginal split
reported itself as confident.

The flaw is that *any* data-derived scaling makes the distance scale-free,
which destroys the only judgement that matters — is this the same voice? With a
single speaker, all residuals are tiny jitter, and dividing by a tiny standard
deviation amplifies them into what looks like a room full of people.

The fix is to not centre or scale at all. A shared offset cancels out of a
Euclidean difference anyway, so centring was never doing the work it claimed
to, and fixed per-dimension weights mean a distance means the same thing in
every meeting. `distance` is now per-dimension RMS with fixed weights: MFCC
means at 1.0, standard deviations at 0.5 (weaker evidence — how much a voice
moves depends on what was said), and log pitch at 8.0 (an octave is 0.7 in
natural log, against inter-speaker MFCC differences of several units).

**A distance threshold.** The second draft used an absolute merge threshold.
That has to be right in two directions at once and will not be: set it tight
and one animated speaker becomes three, set it loose and a room of twenty
collapses to one.

The third design reads the **shape of the merge sequence** instead. Merge all
the way down to one cluster, recording each merge distance. Within one speaker
those distances rise smoothly; the first merge that crosses between two
speakers jumps. The jump says how many voices there were, and it is scale-free
— it holds for a quiet recording and a loud one, a close mic and a far one.

An absolute floor is still needed, because a large *ratio* between two tiny
distances is noise. That floor is the only calibrated constant left, and it was
measured rather than chosen. On the module's own fixtures:

| Case | Within-cluster | Between-cluster |
|---|---|---|
| One voice, held as one cluster | 0.40 | — |
| Three voices, correctly split | 0.26 | 5.72 |
| Three voices, forced into one | 3.86 | — |

The two regimes are an order of magnitude apart. `MIN_SPLIT_DISTANCE` sits at
2.0, closer to the same-speaker end because leaving two people merged is the
more legible failure: "Speaker 2 said both of these" is wrong but readable,
whereas a split invents somebody who was never in the room. The constant's doc
comment records this measurement so the next person to touch it knows how to
recalibrate.

### 2.2 Rung 1 still wins for the local user

Where the channel and a cluster disagree about the *local* user, the channel
wins: it is what the audio device reported, and a cluster is an inference about
it.

There is a subtler problem. The local user's voice also arrives through the
loopback in many setups, so it forms a cluster of its own — and without
intersecting the two sources, the user appears twice: once as "Me" from the
channel and once as a `Speaker N` from their own voice.
`local_user_clusters` resolves it by majority: the cluster that most often
coincides with microphone-only audio is the local user's. A test pins it.

Cluster 0 maps to `speaker_1` rather than `speaker_2`, so a name the user gave
before diarization existed survives running it.

---

## 3. A header that did not say who was there

The header read `Recorded on 4 Sept 2026, 3:09 pm • Duration: 44m 43s •
Chunks: 90 • Words: 5661`. Chunks and words are implementation detail;
participants are the thing a reader needs.

`processing::metadata::MeetingMetadata` is the answer, and it is deliberately
separate from `MeetingFacts`. Facts are what a model read out of the transcript
and can be wrong; everything in metadata is counted or measured, which is what
makes it safe to put at the top of a document somebody else will read.

`ParticipantOrigin` keeps §2.3's states distinct rather than collapsing them
into "unknown", because they call for different actions: a confirmed name needs
nothing, an inferred one is worth a glance, a channel-only bucket means
separation never ran, and somebody who was named but never heard is not a
speaker at all.

`TranscriptHealth` is on the meeting rather than in a log because it is the
number that explains a thin summary. Nine rejected chunks is four and a half
minutes that never reached the model, and the user is entitled to see that
rather than wonder why their notes are short.

### 3.1 Names the meeting said out loud

Rung 5. The rules specify a model pass; `processing::names` uses deterministic
patterns instead, and the reason is the failure mode: a model asked "who is
Speaker 2" will always produce a plausible answer. Patterns produce nothing
when there is nothing, which is the required behaviour.

Two confidence levels, kept apart:

- **Self-introduction** ("I'm Pranjali") binds to the speaker of that turn.
- **Direct address** ("Thanks, Ayush") names somebody who is *not* speaking, so
  it never binds to a voice. Guessing which voice a name belongs to is how a
  commitment gets attributed to the wrong person.

Neither is presented as confirmed. A `plausible_name` stop list rejects the
capitalized words that share a name's shape — `Everyone`, `Monday`, `Sorry`,
`Relay`, all-caps acronyms — and a test asserts that a transcript full of them
offers no names at all.

---

## 4. A notes box that asked for the wrong thing

The notes tab was a 14-row textarea and an optional second one. Its own copy
said it "corrects a name the recogniser mangled" — but the only mechanism was
prose in a prompt, which works if and only if a model notices the sentence and
chooses to act on it. And a name does not belong in a summary anyway; it
belongs in the speaker registry.

`MeetingNotes` now carries typed directives with five kinds, each read by the
stage that can act on it:

| Kind | Read by | Effect |
|---|---|---|
| `SpeakerName` | `processing::directives` | Renames in the registry. No model. |
| `Term` | `normalize_transcript` | Joins this run's glossary. No model. |
| `Participant` | `metadata::build_participants` | Listed, marked as not heard. |
| `Agenda` | `MeetingContext::render_notes` | Intent, never evidence of a decision. |
| `Note` | `MeetingContext::render_notes` | Read when summarizing. |

Adding one re-prepares the meeting immediately, because a correction the user
has to press "regenerate" to see is a correction they will assume did not work.
A directive naming a speaker the meeting does not have comes back as
`UnresolvedDirective` and is shown on its own row — silently doing nothing is
the failure mode this replaces.

The paragraph box remains, second and collapsed, for the things that genuinely
are prose. Free-text directives fold into the same prompt block rather than
getting one of their own: to a summarizer, a sentence typed as a directive and
the same sentence typed in the box are the same kind of evidence, and splitting
them would only invite the model to weigh one above the other.

---

## 5. A summary that could not be shared

`processing::share` composes the counted header, the already-validated prose,
the to-dos with owners and deadlines, and the decisions with their rationale
into one Markdown document. It composes only — it writes nothing, so nothing
here can invent.

Two rules, both because this text leaves the app:

- **A degraded meeting says so.** Rejected chunks, a channel-only roster, prose
  rendered without a model, unattributed stretches — each is disclosed. A
  shared summary that hides its own limitations is worse than none, because the
  reader cannot know to ask.
- **Private things stay private by default.** The conversation is off because it
  turns one page into forty; the user's notes are off because they are usually
  working material, not something meant to circulate.

---

## 6. Why the checks are runnable

`meetings_v2::selftest` runs eleven checks from a button in Diagnostics. The
unit tests prove the same properties on CI, so this needs a justification: the
failure is **machine-dependent**. Whether room tone becomes four minutes of
"Thank you." turns on this microphone's noise floor, on which Whisper model is
installed, and on how quiet the room is. A green CI run says the logic is
right; a green self-test says it is working here.

Where a model is configured, the run also asks *that* model to transcribe
thirty seconds of synthesized room tone and reports what came back. If it is
subtitle boilerplate, the user is looking at the exact hallucination this work
exists to catch, produced by their own model, with the check above it saying
the gate stopped it. `resolve_meeting_model_path` was factored out of the
meetings engine specifically so the check asks the same model a real recording
would — a self-test against a different model would be worse than none.

Every check reports the measurement behind its verdict. A check that says only
"passed" cannot be trusted, and a failure with no number cannot be diagnosed; a
test asserts that property across the whole set.

---

## 7. What is still not built

- **Rung 2 — a voice library.** The feature that would create biometric data,
  and §6's consent, management and deletion requirements land with it.
  `maybe_later.md` item 18.
- **Rung 3 — calendar attendees.** No calendar integration exists.
  `MeetingMetadata` and the expected-speaker hint are the surfaces it would
  fill. `maybe_later.md` item 19.
- **Per-source audio tracks.** Diarizing the system-audio stream alone would
  remove the local user's voice from the clustering problem entirely.
  `maybe_later.md` item 3.
- **The "In person" marking** from §2.2, which would disable rung 1 and make
  diarization the primary path for a room sharing one microphone.
