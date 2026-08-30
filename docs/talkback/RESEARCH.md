# Talkback — research pass

Traced from **v0.17.3** (`claude/talkback-agent-design-qvp3av`), researched
2026-08-30. This is a *deep record*: it states what was checked, when, and
what it decided. Read it as history plus the reasoning behind
[`ARCHITECTURE.md`](ARCHITECTURE.md); where the code and this file disagree,
the code is right.

Every "Decision" column below is binding on the implementation. Where a
decision rejects something, the reason is recorded — a rejection with no
reason is how a settled question gets reopened every six months.

---

## 0. The constraint that decides most of this

Relay is **AGPL-3.0-only**, ships as a **Windows desktop app** (Tauri 2 +
Rust + React), and has **no Python runtime** and **no GPU assumption**. Its
one existing native ML dependency is `whisper-rs` → whisper.cpp, built from
source by CMake at `cargo build` time.

So a candidate is only viable if it is one of:

1. a Rust crate (pure Rust, or C/C++ built by `cc`/`cmake`), or
2. an ONNX graph runnable through a Rust ONNX runtime, or
3. an external process the user installs and Relay shells out to.

Anything whose only real distribution is "pip install and run PyTorch" is
not a Relay dependency, however good the model is. That single rule removes
Pipecat, Chatterbox, Parakeet-via-NeMo, and openWakeWord as *dependencies*
before quality is even discussed. They are still worth reading, and several
of them changed the design.

---

## A. Competitive matrix (reference only — no implementation dependency)

| Product | Voice | Memory | Personal context | Retrieval | Interrupt | Wake word | Actions | Visual agent | Persistence | Local/private | Relay takeaway |
|---|---|---|---|---|---|---|---|---|---|---|---|
| **ChatGPT Voice** | Realtime speech-to-speech | Cross-session "memories" | Opt-in, model-managed | Web + memory | Yes | No | Tools/web | Animated orb | Chat history | No | The orb is a *state display*, not a waveform. Copy: a persistent presence with legible LISTENING/THINKING/SPEAKING states. Don't copy: memory the user cannot inspect. |
| **Claude voice** | STT → text model → TTS | Conversation only | Project/context files | Files, connectors | Yes | No | Tools/MCP | Minimal | Chat history | No | Voice and text hit the *same* engine and the same transcript. Copy exactly. |
| **Gemini Live** | Realtime, multimodal | Session | Account data | Search | Yes, aggressive | "Hey Google" | App actions | Animated field | Session | No | Perceived realtime is turn-taking + barge-in latency, not model size. |
| **Perplexity voice** | STT → LLM → TTS | Thread | Thread only | Web search, cited | Yes | No | Few | Minimal | No | Short spoken answers with citations. Copy: brevity by default, sources always available. |
| **Granola** | No conversational voice | Meetings + your typed notes | **Very strong** — your shorthand is context, not a scratchpad | Meeting corpus + calendar | n/a | n/a | Templates, chat-with-notes | No | Persistent notes | No (cloud) | The single most relevant product. Copy: derived meeting intelligence is a *first-class retrieval source*, and the user's own words outrank any prompt tuning. Relay already has this shape in `MeetingNotes` + `MeetingFacts`. |
| **Limitless** (Meta acq. Dec 2025, pendant discontinued) | Ask-your-day voice | Continuous capture | Total | Temporal + semantic | n/a | n/a | Few | No | Everything | Cloud | Copy: *temporal* recall ("what did I say last Tuesday") is a distinct query class needing a date filter, not just similarity. Don't copy: always-on capture — Relay's mic is explicit. |
| **Mem** | Limited | Personal KB | Strong | Semantic | n/a | n/a | Few | No | Yes | Cloud | Copy: one knowledge substrate, many views. Do **not** build a Talkback-only memory store. |
| **Notion AI** | No | Workspace | Docs/meetings | Hybrid | n/a | n/a | Many | No | Yes | Cloud | Copy: separate *capture* from *derived intelligence*, and retrieve the derived layer first. Relay's `MeetingFacts` is exactly that layer. |

**Principles extracted (not features cloned):**

1. The agent is a *presence with states*, not a record button.
2. Voice and text must share one engine, one transcript, one memory.
3. Short answers by default; depth on request.
4. Derived intelligence beats raw transcript as retrieval material.
5. Personal-memory questions and general questions are different query
   classes and must be routed differently.
6. Barge-in is the feature that makes it feel like a conversation.

**Latency expectations, from the voice-agent literature rather than
vibes:** barge-in (user speech → TTS flush) under ~150 ms; turn gap (user
stops → first agent audio) 200–450 ms for a hosted cloud agent; above
~800 ms users notice the pause, above ~1500 ms the conversation reads as
broken. A fully local CPU stack will not hit 450 ms — see
[§C](#c-architecture-comparison) for what Relay targets instead and why.

---

## B. Open-source technology matrix

Licence and status verified 2026-08-30 against the upstream repository or
crates.io, not from secondary write-ups, except where marked *(secondary)*.

| Tool | Category | Licence | Local | Windows | CPU | GPU | Streaming | Languages | Maturity | Integration cost | Relay fit | Decision |
|---|---|---|---|---|---|---|---|---|---|---|---|---|
| **whisper.cpp / `whisper-rs` 0.16** | STT | MIT | Yes | Yes (already shipping) | Yes | Optional | Chunked, not true streaming | 99 incl. Hindi | Shipped in Relay, 400+ tests around it | **Zero** — already integrated | Already the production path; `StreamingTranscriber` already re-decodes a growing utterance buffer | **KEEP** |
| **NVIDIA Parakeet TDT 0.6B v3** | STT | CC-BY-4.0 (weights) | Yes | Via NeMo/PyTorch | Yes | Preferred | Yes | **25 European languages — no Hindi** | Strong, NVIDIA-maintained | High: NeMo/PyTorch, or an unofficial ONNX export | Fails Relay's core English↔Hindi code-switching requirement, and would add a Python runtime | **REJECT** (revisit only if an official ONNX export *with* Hindi lands) |
| **Silero VAD** | VAD | MIT (code and weights) | Yes | Yes | ~1 ms per 30 ms frame, 1 thread | No | Frame-by-frame | 6000+ | Mature, widely deployed | Medium: ONNX runtime + model file | Best-in-class streaming speech gate | **EVALUATE** — architecture accommodates it, V1 does not ship it (see §D) |
| **Pipecat Smart Turn v3** | Semantic turn detection | Open weights + open training data/code | Yes | Yes | ~8M params, <60 ms CPU | No | Per-utterance | Multilingual | New but from the Pipecat team | Medium-high: ONNX + Whisper-tiny-style mel front end | The right answer to "has the user finished a thought" | **EVALUATE** — V2 candidate |
| **Relay's existing energy VAD** | VAD | — (in repo) | Yes | Yes | Trivial | No | Post-hoc over a finished buffer | n/a | Shipped, tested | Zero | Trims silence from a completed recording; **not** a turn detector | **KEEP for capture, INSUFFICIENT for Talkback turn-taking** — see §D |
| **Piper (`rhasspy/piper`)** | TTS | MIT | Yes | Yes | Yes | No | No (batch WAV) | ~30 | **Archived Oct 2025 — read-only** | Zero (already wired) | Relay's shipped TTS; still works, no longer maintained | **KEEP as the V1 baseline provider, flagged as archived** |
| **Piper successor (`OHF-Voice/piper1-gpl`)** | TTS | **GPL-3.0** (relicensed) | Yes | Yes | Yes | No | Partial | ~30 | Active, Open Home Foundation, v1.6.0 Jul 2026 *(secondary)* | Low — same CLI shape, out-of-process | GPL-3.0 is fine *for Relay*: AGPL-3.0 is GPL-3.0-compatible, and Relay shells out to a separate process rather than linking. Relay downloads it at the user's request from the project's own release URL and does not redistribute it. | **KEEP** — what the installer fetches |
| **Kokoro-82M** | TTS | Apache-2.0 (weights **and** code) | Yes | Yes | Yes (real-time-ish) | Optional | Yes, chunked | 8 incl. Hindi | v1.0 Jan 2025, very widely used | Medium: ONNX + a Rust port (`Kokoros`, `kokoroxide`, `kokoro-en`, `tts-rs`) or a bundled CLI | Best quality-per-MB local option; Apache weights are redistributable | **EVALUATE** — second provider behind the trait, benchmark before adopting |
| **Chatterbox / Turbo / Nano** (Resemble AI) | TTS | MIT (code) | Yes | Python only | Nano ~3× realtime on 8 cores *(secondary)* | 8–16 GB VRAM for the full model *(secondary)* | Yes | Multilingual | Active | **Very high** — PyTorch runtime inside a Tauri app | Expressive, agent-oriented; wrong runtime for a Windows desktop installer | **REFERENCE ONLY** for V1; re-evaluate if an ONNX/Rust path appears |
| **openWakeWord** | Wake word | Apache-2.0 | Yes | Python | Yes | No | Yes | Custom | Maintained, Home-Assistant ecosystem | High (Python) | Right idea, wrong runtime | **REFERENCE ONLY** — informs the `activation_mode` seam |
| **microWakeWord** | Wake word | Apache-2.0 | Yes | MCU-targeted | Yes | No | Yes | Custom | OHF partner | High | Optimised for microcontrollers, not desktop | **REFERENCE ONLY** |
| **Pipecat** | Voice-agent framework | BSD-2 | Yes | Python | — | — | Yes | — | 13.4k★, very active | **Prohibitive** — a Python process next to a Rust/Tauri app | Its *architecture* is the value | **REFERENCE ONLY — NOT AN IMPLEMENTATION DEPENDENCY** |
| **LiveKit Agents** | Voice-agent framework + WebRTC | Apache-2.0 | Yes (self-host) | Server-side | — | — | Yes | — | Very active | Prohibitive — WebRTC infra for a single-machine app | Lessons only | **REFERENCE ONLY** |
| **`rodio` 0.22.2** | Rust audio playback | MIT/Apache-2.0 | Yes | Yes | — | — | Yes | — | Mature | **Blocked**: requires `cpal ^0.17`; Relay pins `cpal 0.15` | Two cpal versions → two WASAPI stacks in one process | **REJECT for V1** (verified against crates.io dependency data) |
| **`ort`** (ONNX Runtime bindings) | Inference runtime | Apache-2.0/MIT | Yes | Yes, with a caveat | Yes | Optional | — | — | Mature | Medium: Windows ships an older `onnxruntime.dll` in System32 that shadows the crate's; the `copy-dylibs` fix targets **binary** Cargo targets, and Relay's crate is `staticlib`/`cdylib`/`rlib` | Real packaging risk that must be proven on Windows before any ONNX model ships | **EVALUATE** — gate for Silero/Kokoro/Smart-Turn |
| **`fastembed` 5.x** | Local embeddings | Apache-2.0 | Yes | Yes | Yes | Optional | — | Multilingual models available | Active (v5.16.2, Jun 2026) *(secondary)* | Medium — same `ort` caveat, plus a first-run model download | The credible path to real semantic retrieval | **EVALUATE** — V2, behind the retriever's scoring seam |
| **Ollama `/api/embed`** | Embeddings via existing provider | MIT (Ollama) | Yes | Yes | Yes | Optional | — | Model-dependent | Shipped | **Low** — Relay already talks to Ollama over HTTP | Embeddings with *zero* new native dependencies | **EVALUATE** — the preferred first semantic-retrieval experiment |

### Classification

```text
KEEP            whisper.cpp / whisper-rs      (STT, unchanged)
                Relay energy VAD              (capture trimming, unchanged)
                Piper                         (TTS baseline, now behind a trait)
                piper1-gpl                    (the maintained binary to point at)
                Ollama / LLMClient            (LLM, extended with streaming)

EVALUATE        Kokoro-82M                    (2nd TTS provider — benchmark first)
                Silero VAD (ONNX)             (streaming turn detection — V2)
                Smart Turn v3                 (semantic end-of-turn — V2)
                ort / fastembed               (embeddings — V2, gated on Windows proof)
                Ollama /api/embed             (embeddings without new native deps)

REFERENCE ONLY  Pipecat, LiveKit, Vapi, ChatGPT Voice, Claude voice,
                Gemini Live, Perplexity, Granola, Limitless, Mem, Notion AI,
                OpenAI Realtime/TTS, ElevenLabs, Retell,
                Chatterbox, openWakeWord, microWakeWord

REJECT          Parakeet TDT       — no Hindi; NeMo/PyTorch runtime
                rodio              — cpal 0.17 vs Relay's cpal 0.15
                Any paid SaaS voice/TTS/STT as a required dependency
```

---

## C. Architecture comparison

### A — `STT → LLM → TTS`

Simplest. One LLM call over the raw question. No grounding, no provenance,
no personal memory. This is roughly what `pipeline/chat.rs` does today plus
a naive vault lookup.

*Rejected*: it cannot answer "what did we decide about X", which is the
entire product thesis.

### B — Realtime speech-to-speech

One multimodal model consumes audio and emits audio. Lowest achievable
latency, most natural prosody.

**Why Relay must not start here** — evaluated against the required
dimensions:

| Dimension | Verdict |
|---|---|
| **Latency** | Best-in-class *if* the model is hosted. Locally, no open speech-to-speech model runs at conversational latency on a CPU-only Windows laptop, which is Relay's stated target. |
| **Grounding** | The retrieval step disappears into an opaque audio-to-audio call. Relay's differentiator is that answers come from the user's own capture. |
| **Provenance** | There is no text turn to attach `source_id`s to. "Where did you get that?" becomes unanswerable. |
| **Tool calling** | Weaker and less consistent than text tool-calling across providers; Voice Note and Scribble creation depend on it. |
| **Debugging** | No intermediate transcript, no retrieved-context log, nothing to unit-test. Relay's meeting pipeline learned this lesson already: `MeetingFacts` exists precisely so no model is asked to comprehend and write at the same time. |
| **Local knowledge injection** | Injecting a large retrieved context into a realtime audio session is awkward and provider-specific. |
| **Provider flexibility** | Would hard-bind Talkback to the handful of vendors that offer it — every one of them a paid SaaS, which §2 of the brief forbids as a dependency. |
| **Offline** | Impossible today for this class of model on Relay's hardware. |

*Deferred, not rejected*: the architecture keeps a seam
(`TalkbackProvider::Realtime`) so a future open local speech-to-speech model
can be added as an *alternative* engine, never as the only one.

### C — `STT → intent → retrieval → LLM (streaming) → sentence buffer → TTS (streaming)`

**Chosen.** It is the only one of the three that can be grounded,
provenanced, tool-called, unit-tested, and run entirely offline. Its
weakness is latency, and that weakness is addressable *structurally*
(stream the LLM, chunk to TTS at the first sentence, start speaking before
generation ends) rather than by buying a faster provider.

The hypothesis in the brief was C. It survives verification. The reasoning
above is why, and the reasoning is not "C sounds modern" — it is that B
destroys provenance, which is the product.

---

## D. Should Relay adopt a voice-agent framework?

| | Native Rust/Tauri | Pipecat | LiveKit Agents |
|---|---|---|---|
| Extra runtime | None | CPython + PyTorch-class deps | Python + WebRTC/SFU |
| Windows packaging | Already solved | Ship or require an interpreter | Server component |
| Latency | In-process; no IPC hop | IPC + serialization per frame | Network hop |
| Debugging | One process, one log, `cargo test` | Two languages, two logs | Three moving parts |
| Local inference | whisper.cpp already linked | Re-plumb through Python | Same |
| Control over interruption | Total | Framework-mediated | Framework-mediated |
| Maintenance | Relay's own | Follows a fast-moving upstream | Same |

**Decision: native, with Pipecat's architecture as the reference.** Four
specific ideas are adopted from Pipecat and LiveKit and are visible in the
code:

1. **Frames, not calls.** The pipeline emits typed events
   (`talkback-state`, `talkback-delta`, `talkback-audio`) rather than one
   request/response.
2. **Replaceable service boundaries.** `TtsProvider` is a trait today;
   `SttProvider` and `TurnDetector` have the same shape reserved.
3. **Interruption as a first-class pipeline signal**, not an error path —
   one cancellation token threaded through LLM, TTS, and playback.
4. **Behavioural evaluation.** Relay already has this habit
   (`capture/evaluation.rs`, `meetings_v2/processing/eval.rs`); Talkback
   gets deterministic retrieval and state-machine tests rather than
   "it sounded fine".

Nothing about Pipecat's *code* enters Relay. The framework is a
**REFERENCE ONLY** input.

---

## E. What the audit found that changed the plan

Findings that overrode the brief's starting hypotheses:

1. **There is no vector store.** `docs/decisions.md` Decision 6 commits to
   LanceDB and the README advertises it, but there is **no LanceDB
   dependency in `Cargo.toml`** and no embedding code anywhere.
   `VaultManager::search_notes` is a term-count scan over every note.
   Talkback's retrieval quality is therefore bounded by lexical matching
   until embeddings land — and the honest move is to say so and design the
   seam, not to bolt a vector DB onto a feature that hasn't shipped yet.
   *(README corrected in this change.)*

2. **`pipeline/chat.rs` is unreachable.** Decision 34 deferred Voice Chat;
   `ChatPanel.tsx` was deleted and no command exposes `process_chat` —
   `start_capture(mode: "chat")` is never invoked by the frontend. So
   Talkback is not competing with a live surface: it *replaces* dead code.
   `process_chat` is deleted in this change rather than left to rot beside
   its successor.

3. **A streaming STT worker already exists.** `meetings_v2/live_stt.rs`
   runs a growing-utterance buffer with silence-based commit, driven by
   `LiveAudioFrame { is_speech, discontinuity, … }`. That is *exactly* the
   shape Talkback turn detection needs. It is meeting-owned and must not be
   modified — but its design is copied, and `StreamingTranscriber`
   (in `capture/stt.rs`, not in `meetings_v2/`) is reused directly.

4. **`rodio` is blocked by a version conflict** (`cpal ^0.17` vs Relay's
   `0.15`). Playing TTS in Rust would put two WASAPI stacks in one process.
   Playback therefore stays in the WebView, where the Web Audio API also
   gives interruption for free.

5. **Piper is archived.** The `TtsSettings` doc comment points at a
   repository that went read-only in October 2025. The maintained fork is
   GPL-3.0, which is compatible with Relay's AGPL-3.0 and irrelevant anyway
   because Relay shells out to a user-supplied binary.

---

## F. Sources

- [snakers4/silero-vad](https://github.com/snakers4/silero-vad) — MIT, ONNX, ~1 ms/frame
- [silero-vad-rs](https://crates.io/crates/silero-vad-rs) · [voice_activity_detector](https://crates.io/crates/voice_activity_detector) — Rust ports
- [OHF-Voice/piper1-gpl](https://github.com/OHF-Voice/piper1-gpl) — GPL-3.0 Piper successor
- [hexgrad/Kokoro-82M](https://huggingface.co/hexgrad/Kokoro-82M) — Apache-2.0 weights
- [lucasjinreal/Kokoros](https://github.com/lucasjinreal/Kokoros) · [kokoroxide](https://crates.io/crates/kokoroxide) — Rust Kokoro
- [resemble-ai/chatterbox](https://github.com/resemble-ai/chatterbox) — MIT, PyTorch
- [nvidia/parakeet-tdt-0.6b-v3](https://huggingface.co/nvidia/parakeet-tdt-0.6b-v3) — CC-BY-4.0, 25 European languages
- [pipecat-ai/pipecat](https://github.com/pipecat-ai/pipecat) — BSD-2
- [pipecat-ai/smart-turn](https://github.com/pipecat-ai/smart-turn) · [Smart Turn v3 announcement](https://www.daily.co/blog/announcing-smart-turn-v3-with-cpu-inference-in-just-12ms/)
- [dscripka/openWakeWord](https://github.com/dscripka/openWakeWord) — Apache-2.0
- [RustAudio/rodio](https://github.com/RustAudio/rodio) · [rodio 0.22.2 deps](https://crates.io/api/v1/crates/rodio/0.22.2/dependencies) — requires cpal ^0.17
- [ort](https://crates.io/crates/ort) — ONNX Runtime bindings, Windows System32 DLL caveat
- [fastembed](https://crates.io/crates/fastembed) — Apache-2.0 local embeddings
- [Ollama API](https://github.com/ollama/ollama/blob/main/docs/api.md) — NDJSON streaming, `/api/embed`
- [ggml-org/whisper.cpp](https://github.com/ggml-org/whisper.cpp) · [whisper-rs](https://crates.io/crates/whisper-rs)
- [Granola](https://www.granola.ai/) — meeting context as personal context
- [Voice AI barge-in and turn-taking, 2026](https://futureagi.com/blog/voice-ai-barge-in-turn-taking-2026/) — latency budgets
