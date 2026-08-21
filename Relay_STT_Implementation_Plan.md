# Relay --- STT Quality & Configuration Implementation Plan

**Purpose:** Improve Relay's local speech-to-text quality by auditing
and hardening the existing Whisper pipeline rather than blindly changing
models.

**Repository:** `D:\Projects\Relay`

**Current production STT decision:** `ggml-small.bin`, loaded through
`whisper-rs`.

**Primary problem:** STT quality is still inconsistent, especially
around Hindi, English/Hindi code-switching, and potentially
audio/language-detection edge cases.

------------------------------------------------------------------------

## 0. Current Relay context

This plan is deliberately built on the current Relay architecture and
the previous implementation work.

Known/current architecture:

``` text
Global PTT hotkey
      ↓
Rust hotkey manager
      ↓
AudioRecorder / cpal
      ↓
Local whisper-rs
      ↓
Transcript
      ├── Voice Note persistence
      └── enigo text injection / Relay action
```

Relay's Rust backend remains the source of truth for capture state. The
PTT pill is a UI/control surface and must not become a second recorder
or second STT pipeline.

The production Whisper model is already promoted to:

``` text
ggml-small.bin
```

with the intended local location:

``` text
%APPDATA%\Relay\models\ggml-small.bin
```

Do **not** downgrade the production model or replace the local STT
architecture as part of this work.

Previous work also introduced language preferences
conceptually/partially across:

-   Primary Dictation Language
-   Languages I Speak
-   Notes Language
-   Output Writing Script

The target bilingual profile used during testing is:

``` json
{
  "language": {
    "primary_dictation_language": "en",
    "spoken_languages": ["en", "hi"],
    "notes_language": "en",
    "output_script": "latin"
  }
}
```

The important distinction is:

-   **spoken language** controls STT
-   **notes language** controls later LLM-generated notes/summaries
-   **output script** controls orthography/transliteration
-   these must not be conflated

------------------------------------------------------------------------

# 1. Objective

The goal is **not** simply to make Whisper use a larger model.

The goal is to establish a production-grade local STT pipeline where we
can answer, with evidence:

1.  What audio reaches Whisper?
2.  At what sample rate and channel configuration?
3.  How is silence handled?
4.  Is VAD present, and where does it run?
5.  Which Whisper model is actually loaded?
6.  Which decoding parameters are actually used?
7.  Which language is actually passed to Whisper?
8.  When is automatic language detection used?
9.  Is `translate` definitely disabled?
10. Can we reproduce poor transcripts from saved WAVs?
11. Can we A/B test different Whisper configurations on the same
    recording?
12. Which configuration gives the best results for English, Hindi and
    Hinglish?
13. What should remain configurable by the user?
14. What should remain an internal implementation detail?

------------------------------------------------------------------------

# 2. Reference architecture to target

Use OpenWhispr, Voxtype and OpenTypeless as implementation references.

Do not copy their architecture wholesale.

The target Relay pipeline should approximately be:

``` text
Microphone
   ↓
Native audio capture
   ↓
Known audio format
   ↓
Optional/controlled VAD
   ↓
Speech segment
   ↓
Whisper / whisper-rs
   ├── explicit language when unambiguous
   ├── auto detection when genuinely multilingual
   ├── translate = false
   ├── controlled decoding
   └── optional initial prompt
   ↓
Raw transcript
   ↓
Optional script/formatting layer
   ↓
Injection / Voice Note / Relay action
```

Hard principle:

> Fix the STT pipeline before using downstream LLM processing to hide
> transcription errors.

------------------------------------------------------------------------

# 3. Implementation sequence

Do the work in this order.

Do **not** combine all phases into one large change.

Each phase should end with tests and a short report before proceeding.

------------------------------------------------------------------------

# Phase 1 --- Baseline and repository audit

## Prompt 1 --- Audit current STT implementation

``` text
You are modifying the current Relay repository.

Before changing anything, inspect the CURRENT HEAD and current working tree.

Objective:
Perform a complete audit of Relay's existing speech-to-text pipeline.

Do not modify production behaviour yet.

Trace the complete path:

global PTT hotkey
→ recording start
→ microphone/audio capture
→ recording stop
→ WAV/sample buffer creation
→ STT invocation
→ Whisper model loading
→ Whisper parameters
→ transcript result
→ voice-note persistence
→ text injection / Relay processing

Inspect at minimum:

- native/src-tauri/src/capture/
- native/src-tauri/src/hotkeys/
- native/src-tauri/src/commands.rs
- native/src-tauri/src/settings/
- native/src/types/
- relevant frontend capture/settings components
- Cargo dependencies related to cpal, whisper-rs, audio processing and VAD

For the STT implementation, report the EXACT current values/code for:

1. model filename and model path resolution
2. model loading lifecycle
3. sample rate
4. number of channels
5. audio sample format
6. audio normalization/scaling
7. silence handling
8. VAD, if any
9. chunking/segmentation
10. max recording duration
11. Whisper sampling strategy
12. temperature
13. best_of
14. beam search, if applicable
15. language
16. translate
17. initial prompt
18. timestamps
19. no-speech threshold
20. entropy/log-probability thresholds
21. suppress-blank / suppress-nst settings
22. thread count
23. GPU/backend configuration
24. model warm-up/reuse
25. error handling
26. whether transcription runs off the UI thread
27. whether the same STT implementation is used by PTT, voice notes and other pipelines

Also identify every place where a language setting is currently:
- stored
- read
- transformed
- passed to Whisper
- ignored

Do not make changes.

End with:

A. Current STT architecture
B. Exact configuration table
C. Suspected quality problems
D. Confirmed problems
E. Files that would need changes
F. Tests that already exist
G. Recommended next phase

Do not claim anything that you did not verify from the current code.
```

### Exit criteria

Do not proceed until you have a concrete configuration table.

------------------------------------------------------------------------

# Phase 2 --- Lock down audio quality

## Prompt 2 --- Audit and harden the audio input

``` text
Continue from the current Relay repository.

Do not change Whisper decoding parameters yet.

Focus exclusively on the audio entering STT.

Audit the microphone pipeline and determine exactly what Whisper receives.

Verify:

- actual microphone sample rate
- actual channel count
- sample format
- resampling path
- conversion to f32
- amplitude normalization
- clipping
- silence
- leading/trailing silence
- buffer ownership
- recording duration
- whether audio is always converted to the format expected by whisper.cpp

The target STT input should be a known, deterministic format appropriate for Whisper:
16 kHz mono floating-point PCM-equivalent samples.

If the current implementation already guarantees this, preserve it and add tests/assertions rather than rewriting it.

If it does not, implement the smallest safe normalization/resampling layer.

Add diagnostics that can be enabled during development without exposing raw microphone audio to the React UI.

Useful diagnostics may include:

- sample count
- duration
- sample rate
- channel count
- RMS
- peak amplitude
- percentage of near-silent samples
- clipping count

Do not stream raw PCM to the frontend.

Add unit tests for:

1. normal audio conversion
2. stereo-to-mono conversion if applicable
3. sample-rate conversion if applicable
4. silence
5. quiet speech
6. loud speech
7. clipping

Run the existing frontend and backend tests.

Report exactly what changed and why.
```

### Exit criteria

You should be able to say:

> "Relay gives Whisper X seconds of 16 kHz mono audio with these
> amplitude characteristics."

------------------------------------------------------------------------

# Phase 3 --- Add/review VAD

## Prompt 3 --- Implement or validate VAD

``` text
Continue from the current Relay repository.

Now investigate Voice Activity Detection.

Use the existing architecture. Do not create a second microphone recorder.

Determine whether Relay currently has VAD in the desktop native STT path.

If VAD is absent, evaluate the smallest reliable local implementation that fits the existing Rust/Tauri architecture.

Use OpenWhispr and Voxtype as references for the role VAD plays, but do not copy their code.

The VAD objective is:

- prevent long silence from being sent to Whisper unnecessarily
- reduce hallucinations on silence/noise
- avoid cutting off the first or last syllables
- preserve natural speech pauses
- work with short PTT recordings
- remain local/offline

Expose internal configuration for development/testing:

- enabled
- speech threshold
- minimum speech duration
- minimum silence duration
- padding before speech
- padding after speech

Do NOT expose all of these to normal users yet.

Add diagnostics so a developer can determine:

- detected speech start
- detected speech end
- total captured duration
- speech duration
- discarded silence duration

Important:
Do not aggressively trim speech.
False-positive silence removal is worse than sending a little extra audio to Whisper.

Add unit tests for:

1. pure silence
2. speech surrounded by silence
3. short pause inside a sentence
4. quiet speech
5. background noise
6. speech followed by a long pause
7. speech beginning immediately after recording starts

Then run backend tests.

Report the selected VAD implementation and exact default parameters.
```

------------------------------------------------------------------------

# Phase 4 --- Make Whisper configuration explicit

## Prompt 4 --- Centralize Whisper decoding configuration

``` text
Continue from the current Relay repository.

Do not change the production model.

Refactor the Whisper configuration so that all important STT parameters have ONE backend source of truth.

Create a clearly named internal configuration structure for Whisper transcription.

It should make explicit, at minimum:

- model path
- language
- translate
- sampling strategy
- best_of
- temperature
- initial prompt
- timestamps
- no-speech handling
- token suppression settings where supported
- thread count where supported

Do not expose every parameter in the user settings UI.

The important requirement is observability:
a developer should be able to determine exactly which configuration was used for a transcription.

Keep:

translate = false

for dictation.

Do not silently fall back to a different model.

Do not silently change the user's selected model.

Preserve custom model paths.

Add unit tests around configuration resolution.

Also add a development diagnostic/log output that reports the effective STT configuration WITHOUT logging transcript content or microphone audio by default.

Run all backend tests.

Report the final effective configuration.
```

------------------------------------------------------------------------

# Phase 5 --- Fix language routing properly

## Prompt 5 --- Implement deterministic language resolution

``` text
Continue from the current Relay repository.

The primary quality problem to solve now is language routing.

Relay must distinguish:

1. Primary Dictation Language
2. Languages I Speak
3. Notes Language
4. Output Writing Script

Only the first two affect STT language selection.

Implement one backend language-resolution function.

Expected behaviour:

Case A:
primary = en
spoken = [en]

→ Whisper language = "en"

Case B:
primary = hi
spoken = [hi]

→ Whisper language = "hi"

Case C:
primary = en
spoken = [en, hi]

→ do NOT hard-lock Whisper to English.
→ use multilingual/automatic decoding.

Case D:
primary = hi
spoken = [hi, en]

→ do NOT hard-lock Whisper to Hindi.
→ allow multilingual decoding.

Case E:
primary = auto

→ Whisper automatic language detection.

Case F:
spoken languages is empty and primary is valid

→ use primary.

Do not use the frontend's local language state as the source of truth.

The selected language must be persisted and reach Rust through the existing settings/IPC path.

Do not implement transliteration in this phase.

Do not change notes language.

Do not change LLM prompts.

Add unit tests for every resolution case above.

Also test backwards compatibility with old settings.json files that have no language section.

Run backend tests and frontend build.

Report the exact effective Whisper language for each test case.
```

------------------------------------------------------------------------

# Phase 6 --- Add reproducible STT diagnostics

## Prompt 6 --- Make STT A/B testing possible

``` text
Continue from the current Relay repository.

We need to stop relying exclusively on live microphone re-speaking to diagnose STT quality.

Extend the existing STT diagnostic capability.

The system must allow a previously recorded WAV file to be transcribed using multiple configurations without requiring a new recording.

The diagnostic should support at minimum:

- production model
- custom model path
- auto language
- English-locked
- Hindi-locked
- current production decoding
- alternative decoding configuration

For each run report:

- model
- model path
- language
- translate
- decoding strategy
- duration
- inference time
- transcript

Do not alter production behaviour.

Keep this diagnostic developer-only.

If the existing diagnose_stt_variants command already exists, extend it rather than creating a duplicate.

Add tests for configuration resolution and command validation.

The output should make it easy to compare:

AUTO
EN
HI

on exactly the same recording.

Run all tests.

Report how to invoke the diagnostic from the current Relay development environment.
```

------------------------------------------------------------------------

# Phase 7 --- Controlled decoding experiments

## Prompt 7 --- A/B test Whisper decoding

``` text
Do not immediately change production defaults.

Using the diagnostic capability, evaluate the current Whisper decoding strategy.

Current production model:
ggml-small.bin

Create a controlled experiment matrix using the same WAV recordings.

Compare:

A. Greedy / current configuration
B. Conservative temperature configuration
C. Alternative supported sampling configuration
D. Initial prompt absent
E. Initial prompt with a small domain vocabulary

Do not introduce speculative parameters that whisper-rs does not actually support.

Do not tune based on a single sentence.

Use multiple recordings:

- short English
- long English
- pure Hindi
- long Hindi
- English + Hindi code-switching
- names
- numbers
- technical terminology
- speech with pauses
- speech in mild background noise

For each recording capture:

- expected transcript
- actual transcript
- language
- configuration
- inference time
- obvious failure mode

Do not promote a configuration because it is faster if transcription quality becomes materially worse.

At the end, recommend one production configuration only if the experiment provides evidence.

If there is no meaningful improvement, leave production defaults unchanged.
```

------------------------------------------------------------------------

# Phase 8 --- Initial prompt/domain vocabulary

## Prompt 8 --- Add optional vocabulary priming

``` text
Continue only if the previous experiment shows that initial_prompt materially improves Relay-specific vocabulary.

Implement a controlled initial-prompt mechanism.

Requirements:

- prompt must be optional
- prompt must not contain a giant list of words
- prompt must be configurable internally
- prompt must not force English for multilingual speech
- prompt must not translate Hindi
- prompt must not become a second semantic processing layer

Start with a small Relay domain vocabulary such as:

Relay
NavGurukul
NGConnect
OpenWhispr
Whisper
Whisper.cpp
Tauri
Rust
Supabase
GitHub

Add tests showing:
- no prompt
- domain prompt
- multilingual prompt

Do not assume prompting improves accuracy.

If results are neutral or worse, keep the feature available internally but do not enable it by default.
```

------------------------------------------------------------------------

# Phase 9 --- Production observability

## Prompt 9 --- Add STT diagnostics without leaking audio

``` text
Add lightweight production-safe STT diagnostics.

Do not log microphone audio.

Do not log full transcripts by default.

For each dictation session, internally record enough metadata to diagnose failures:

- session ID
- model
- model size/name
- language mode
- actual resolved language mode
- recording duration
- speech duration if VAD is enabled
- sample rate
- channel count
- inference duration
- error category
- transcript character/token count

Make diagnostics easy to enable for development.

Avoid sensitive logging.

Do not introduce a telemetry service.

Add tests ensuring diagnostics do not contain raw audio.

Report where the diagnostic information is available.
```

------------------------------------------------------------------------

# Phase 10 --- Keep script conversion separate

## Prompt 10 --- Separate STT from output script

``` text
Do not modify Whisper configuration for this task.

Audit the relationship between:

spoken language
and
output writing script.

Relay may support:

- Hindi spoken language
- native Devanagari output
- Latin/Romanized output

Whisper should remain responsible for transcription, not arbitrary orthographic conversion.

If output_script = native:
return Whisper's native script.

If output_script = latin:
introduce a separate downstream transliteration stage only where required.

Do NOT alter raw STT transcripts.

The raw Voice Note transcript must remain the transcription result.

The transformed/injected text may be different.

Implement this separation only if the current repository requires it.

Add tests proving:

spoken Hindi → raw transcript remains Hindi/Devanagari

and:

output_script = latin → downstream output may be Romanized

Do not change English output.
```

------------------------------------------------------------------------

# Phase 11 --- Regression and production gate

## Prompt 11 --- Full STT regression audit

``` text
The STT implementation is now considered feature-complete for this iteration.

Perform a full audit of the current HEAD.

Verify:

1. Production model remains ggml-small.bin.
2. Custom model paths still work.
3. Model provisioning still works.
4. Language settings persist.
5. Language settings reach Rust.
6. Single-language users get deterministic Whisper language selection.
7. Multilingual users are not incorrectly hard-locked.
8. translate remains false.
9. Audio reaches Whisper in the expected format.
10. VAD does not cut speech.
11. Silence does not create hallucinated text.
12. PTT remains the only recorder.
13. There is still only one STT pipeline.
14. STT does not block the UI.
15. Voice Notes still persist raw transcripts.
16. Injection still works.
17. The PTT pill remains backend-state driven.
18. No raw PCM is sent to React.
19. No microphone audio is logged.
20. Existing non-STT functionality has not regressed.

Run:

npm --prefix native run build

and:

cargo check --no-default-features --manifest-path native/src-tauri/Cargo.toml

cargo test --no-default-features --manifest-path native/src-tauri/Cargo.toml

Run any existing repository lint/typecheck/test commands as well.

Do not claim success for a command that was not actually executed.

Report:
- commands
- results
- files changed
- remaining warnings
- remaining known limitations
- exact production STT configuration
```

------------------------------------------------------------------------

# 4. Manual STT test protocol

Automated tests cannot tell us whether Relay actually sounds good.

Run these manually against the live application.

Use the **same microphone and environment** for all tests.

Do not change the microphone between test groups.

For each test record:

``` text
Test ID
Expected text
Actual text
Language configuration
Model
Result: PASS / PARTIAL / FAIL
Notes
```

------------------------------------------------------------------------

## Test Group A --- English

### A1 --- Short English sentence

Say:

> "I need to finish this report today."

Expected:

> I need to finish this report today.

Check:

-   no missing words
-   no duplicated words
-   correct punctuation if punctuation is expected
-   no translation
-   no hallucination

------------------------------------------------------------------------

### A2 --- Long natural English dictation

Say:

> "I need to review the automation team's progress this afternoon,
> identify the three workflows that are still blocked, speak to the
> owners about the dependencies, and then send a short update to the
> leadership team before the end of the day."

Expected meaning and wording should remain substantially identical.

This is important because a model/configuration can perform well on
five-word sentences while failing on longer contextual speech.

------------------------------------------------------------------------

### A3 --- Technical vocabulary

Say:

> "The Relay desktop application uses Tauri, Rust, whisper-rs, cpal,
> enigo, and a local GGML Whisper model for speech recognition."

Check whether technical terms are preserved rather than replaced with
phonetically similar common words.

------------------------------------------------------------------------

# Test Group B --- Hindi

## B1 --- Hindi in natural speech

Say:

> "कल मुझे टीम के साथ एक मीटिंग करनी है और उसके बाद मुझे रिपोर्ट पूरी करनी है।"

Expected:

> कल मुझे टीम के साथ एक मीटिंग करनी है और उसके बाद मुझे रिपोर्ट पूरी करनी है।

Check specifically:

-   Devanagari rather than Urdu/Arabic script
-   no English translation
-   no dropped words
-   no fabricated words

------------------------------------------------------------------------

## B2 --- Longer Hindi paragraph

Say:

> "आज सुबह मैंने पूरी टीम के साथ प्रोजेक्ट की प्रगति पर चर्चा की। कुछ काम अभी भी
> बाकी हैं, इसलिए मुझे पहले उन लोगों से बात करनी है जिनके टास्क ब्लॉक हो गए हैं। उसके
> बाद मैं शाम तक एक छोटा अपडेट भेज दूंगा।"

Do not judge only character-for-character output.

Judge:

-   semantic fidelity
-   missing phrases
-   repeated phrases
-   script
-   language stability

------------------------------------------------------------------------

# Test Group C --- Hinglish / code-switching

This is the most important Relay-specific test.

## C1 --- Short Hinglish

Say:

> "Kal mujhe team ke saath meeting karni hai."

Expected raw STT should represent the spoken Hindi correctly.

Do not expect Whisper itself to magically produce Romanized Hindi.

------------------------------------------------------------------------

## C2 --- English + Hindi code-switching

Say:

> "I need to finish this report aaj and send it to Bala kal."

Check whether:

-   `aaj` is incorrectly replaced with `today`
-   `kal` is incorrectly replaced with `tomorrow`
-   words are dropped
-   the entire sentence becomes English
-   the language switches into Urdu script

------------------------------------------------------------------------

## C3 --- Longer natural Hinglish

Say:

> "I spoke to the automation team this morning and unke kuch workflows
> abhi bhi blocked hain, so I need to check the dependencies and then
> update Bala before evening."

This should be treated as a **high-priority acceptance test**.

The goal is not perfect Romanization.

The goal is preservation of what was actually spoken.

------------------------------------------------------------------------

## C4 --- More natural conversational Hinglish

Say:

> "Basically mujhe pehle ye check karna hai ki Whisper actually kaunsa
> model load kar raha hai, because agar model sahi hai but audio
> pipeline mein problem hai toh model change karne se kuch solve nahi
> hoga."

This tests:

-   English technical vocabulary
-   Hindi grammar
-   code-switching
-   longer context
-   technical nouns
-   conversational filler

------------------------------------------------------------------------

# Test Group D --- Language settings

Run the same recordings under different settings.

### D1 --- English only

Settings:

``` text
Primary: English
Languages I Speak: English
```

Speak:

> "I need to finish the report before the meeting."

Expected: clean English.

------------------------------------------------------------------------

### D2 --- Hindi only

Settings:

``` text
Primary: Hindi
Languages I Speak: Hindi
```

Speak:

> "मुझे आज शाम तक यह रिपोर्ट पूरी करनी है।"

Expected: Hindi, native script.

------------------------------------------------------------------------

### D3 --- English + Hindi

Settings:

``` text
Primary: English
Languages I Speak: English, Hindi
```

Speak:

> "I need to finish this report aaj and send it to Bala kal."

Expected: no forced English translation and no Urdu-script failure.

------------------------------------------------------------------------

### D4 --- Hindi primary + English secondary

Settings:

``` text
Primary: Hindi
Languages I Speak: Hindi, English
```

Repeat C2 and C4.

The result should remain multilingual rather than being forcibly
Hindi-only.

------------------------------------------------------------------------

### D5 --- Auto

Settings:

``` text
Primary: Auto
```

Run:

1.  English sentence
2.  Hindi sentence
3.  Hinglish sentence

Compare against the explicit-language configurations.

Record whether auto detection creates additional errors.

------------------------------------------------------------------------

# Test Group E --- Silence and noise

## E1 --- Silence

Hold PTT for approximately 5 seconds without speaking.

Expected:

-   no hallucinated sentence
-   no random text
-   graceful empty result
-   no crash

------------------------------------------------------------------------

## E2 --- Short speech with long silence

Say:

> "This is a short test."

Then remain silent for several seconds before releasing.

Expected:

> This is a short test.

No additional hallucinated content.

------------------------------------------------------------------------

## E3 --- Quiet speech

Speak the same sentence at approximately half your normal speaking
volume.

Expected:

-   still recognizable
-   no large increase in hallucination
-   no dropped first/last words

------------------------------------------------------------------------

## E4 --- Background noise

With normal environmental background noise, say:

> "I need to review the project status and send an update this evening."

Compare against the clean-room result.

------------------------------------------------------------------------

# Test Group F --- Speech boundaries

These are specifically for VAD.

## F1 --- Immediate speech

Start speaking almost immediately after pressing PTT:

> "I need to check the deployment status."

Expected: first words are not clipped.

------------------------------------------------------------------------

## F2 --- Delayed speech

Press PTT, wait approximately 1--2 seconds, then say:

> "I need to check the deployment status."

Expected: no hallucination during the silence and no missing first word.

------------------------------------------------------------------------

## F3 --- Internal pause

Say:

> "I need to review the deployment... before I send the update."

Expected: the pause does not cause Whisper/VAD to terminate the
utterance prematurely.

------------------------------------------------------------------------

## F4 --- Final word boundary

Say:

> "Please send the report to Bala."

Release immediately after "Bala."

Expected: `Bala` is not clipped or lost.

------------------------------------------------------------------------

# Test Group G --- Long-form dictation

Use this longer test:

> "Today I want to review everything we have done on Relay so far. We
> started with the basic push-to-talk workflow, then moved the recording
> logic into the Rust backend, added the floating dictation pill, and
> promoted Whisper Small as the production speech-to-text model. The
> next thing I want to solve is transcription quality, especially when I
> switch between English and Hindi in the same sentence. I don't want to
> solve that by simply sending everything through an LLM after
> transcription because the raw transcript itself needs to remain
> accurate."

Expected:

-   coherent transcript
-   no major omissions
-   no repeated clauses
-   no language drift
-   no hallucinated continuation
-   reasonable latency

------------------------------------------------------------------------

# Test Group H --- Domain vocabulary

Say:

> "Relay uses Tauri and Rust on the desktop, whisper-rs for local speech
> recognition, cpal for microphone capture, enigo for text injection,
> and Supabase for some application data."

Repeat three times.

Check whether the same technical terms are consistently recognized.

This test determines whether an initial prompt/domain vocabulary
provides measurable benefit.

------------------------------------------------------------------------

# Test Group I --- Repeatability

Perform **20 consecutive PTT sessions**.

Use a mixture of:

-   English
-   Hindi
-   Hinglish
-   silence
-   short speech
-   long speech

After 20 sessions verify:

-   no stuck recording
-   no stuck processing state
-   no duplicate transcription
-   no duplicate injection
-   no missing injection
-   no duplicate event listeners
-   no stale language setting
-   no model reload unexpectedly occurring on every session
-   no obvious memory growth
-   no degradation in transcription quality
-   no UI freeze

This is a release-gate test.

------------------------------------------------------------------------

# 5. A/B experiment matrix

Create a simple results table:

  Recording       Auto   EN locked   HI locked   Production   Alternative
  --------------- ------ ----------- ----------- ------------ -------------
  English 1                                                   
  English long                                                
  Hindi 1                                                     
  Hindi long                                                  
  Hinglish 1                                                  
  Hinglish long                                               
  Technical                                                   
  Noisy                                                       

Use:

-   `PASS`
-   `PARTIAL`
-   `FAIL`

and add a short explanation.

Do not choose a configuration based on one recording.

------------------------------------------------------------------------

# 6. Production acceptance criteria

The STT work is ready for production V1 only when:

### Model

-   `ggml-small.bin` is consistently used as the production default.
-   Model resolution has one source of truth.
-   No accidental fallback to an older model exists.

### Audio

-   Whisper receives known-format audio.
-   Sample rate and channels are deterministic.
-   Speech is not clipped at boundaries.

### Language

-   English-only users can lock English.
-   Hindi-only users can lock Hindi.
-   Multilingual users are not incorrectly forced into one language.
-   Auto mode remains available.
-   `translate = false`.

### VAD

-   Silence does not generate hallucinated speech.
-   Short pauses do not split sentences.
-   First/last words are preserved.

### Diagnostics

-   A recorded WAV can be re-run through STT.
-   Auto/EN/HI can be compared on identical audio.
-   Effective configuration is observable.

### UX

-   PTT remains unchanged from the user's perspective.
-   Pill remains a consumer of backend state.
-   No second recorder exists.
-   No second STT pipeline exists.

### Safety/privacy

-   No raw microphone audio is logged.
-   No full transcript logging is enabled by default.
-   Everything remains local.

------------------------------------------------------------------------

# 7. What NOT to do during this project

Do not:

-   replace Whisper with a cloud API
-   add a second recorder
-   add a second STT pipeline
-   move microphone capture to React
-   blindly upgrade to Whisper Medium/Large
-   hard-code English
-   hard-code Hindi
-   force `auto` for everyone
-   translate the transcript after Whisper
-   use an LLM to "fix" transcription before measuring raw STT
-   add ten user-facing advanced STT controls
-   change the PTT architecture
-   rewrite the existing injection system unless a regression is
    discovered
-   remove `ggml-small.bin` without evidence
-   claim improved accuracy without A/B testing

------------------------------------------------------------------------

# 8. Recommended implementation order

Execute the prompts in exactly this order:

``` text
1. Audit current STT
        ↓
2. Lock down audio format
        ↓
3. Validate/add VAD
        ↓
4. Centralize Whisper configuration
        ↓
5. Fix language routing
        ↓
6. Build reproducible WAV diagnostics
        ↓
7. A/B test decoding
        ↓
8. Test initial prompt/domain vocabulary
        ↓
9. Add safe diagnostics
        ↓
10. Separate script conversion
        ↓
11. Full regression
```

Do not skip directly to step 7.

The purpose of the sequence is to make it possible to identify whether a
bad transcript originates from:

``` text
Microphone
    ↓
Audio conversion
    ↓
VAD
    ↓
Language detection
    ↓
Whisper decoding
    ↓
Post-processing
    ↓
Injection
```

rather than treating "STT quality" as one opaque problem.

------------------------------------------------------------------------

# 9. Final engineering principle

The most important outcome of this work is not a particular Whisper
parameter.

It is this:

> **Relay must be able to reproduce and explain every STT result.**

For any bad transcription we should be able to take the recorded WAV and
answer:

``` text
Which model?
Which language?
Which audio format?
Which VAD decision?
Which decoder?
Which prompt?
How long did inference take?
What did Whisper actually return?
Was anything transformed afterwards?
```

Once that is true, improving STT becomes an engineering/measurement
problem rather than guesswork.

------------------------------------------------------------------------

# 10. Final IDE completion report

At the end of the entire implementation, ask the IDE to report:

``` text
1. Exact production model
2. Exact model path
3. Audio format
4. VAD implementation and parameters
5. Language-resolution rules
6. Whisper decoding parameters
7. Initial prompt status
8. GPU/CPU backend
9. Model warm-up behaviour
10. Diagnostic tooling
11. Automated test count/results
12. Manual tests completed
13. English results
14. Hindi results
15. Hinglish results
16. Silence/noise results
17. Long-form results
18. 20-session stability results
19. Remaining known weaknesses
20. Recommended next STT improvement
```

Do not claim a manual test passed unless it was actually performed.
