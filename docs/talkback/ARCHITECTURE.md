# Talkback — architecture

The conversational layer over everything Relay has already captured.
Written against **v0.18.0**. This is a *living specification*: if it and the
code disagree, the code is right and this file is a bug.

The research behind these choices is in [`RESEARCH.md`](RESEARCH.md).

---

## 1. What Talkback is

```text
CAPTURE  →  UNDERSTAND  →  REMEMBER  →  CONVERSE
```

Relay already does the first three. Talkback is the fourth, and it is
explicitly **not** a chatbot bolted onto a vault:

- Personal-memory questions are answered **only** from Relay's own data, or
  not at all.
- Every answer carries real provenance — source type, id, title, timestamp.
- Conversation is **ephemeral**. Nothing becomes knowledge without the user
  saying so.
- Talkback owns **no storage of its own**. Voice Notes, Scribbles, Meetings
  and MeetingFacts are the memory.

## 2. Pipeline

```text
                              ┌──────── text input (fallback, same engine)
                              │
 MIC ─→ TURN DETECTOR ─→ STT ─┴─→ TURN NORMALIZATION
                                        │
                                  INTENT ROUTER ─────────────→ TOOL EXECUTOR
                                        │                          │
                                 CONTEXT RETRIEVER            create_voice_note
                          ┌─────────────┼─────────────┐       create_scribble
                    VoiceNotes      Scribbles      Meetings    search_memory
                          │              │          MeetingFacts
                          └─────────────┬┴─────────────┘
                                 CONTEXT ASSEMBLER
                                        │
                                  LLM (streaming)
                                        │
                                 PHRASE BUFFER
                                        │
                                       TTS
                                        │
                              AUDIO QUEUE (WebView)
                                        │
                                  LISTEN AGAIN
```

Interruption cuts across all of it:

```text
SPEAKING → user speaks → cancel token trips → LLM stream dropped,
           TTS abandoned, audio queue cleared → USER_SPEAKING
```

## 3. Why architecture C, not speech-to-speech

Full reasoning in [`RESEARCH.md` §C](RESEARCH.md#c-architecture-comparison).
In one line: a realtime speech-to-speech model has no text turn to attach
`source_id`s to, and **provenance is the product**. The seam for a future
local speech-to-speech engine is kept (`TalkbackEngine` is the only thing
that knows how a turn becomes a response), but V1 is
STT → retrieval → streaming LLM → phrase buffer → TTS.

## 4. Module map

| Module | Responsibility | Pure/testable |
|---|---|---|
| `talkback/state.rs` | The authoritative state machine | Yes — no I/O |
| `talkback/session.rs` | Ephemeral session, turns, source history | Yes |
| `talkback/intent.rs` | Deterministic intent routing | Yes |
| `talkback/retrieval.rs` | Ranking, expansion, dedup, budget | Yes — ranks `CandidateDoc`s |
| `talkback/sources.rs` | Gathers `CandidateDoc`s from the vault/meetings | I/O |
| `talkback/assemble.rs` | System prompt + context block + history | Yes |
| `talkback/chunk.rs` | Phrase buffer for streaming TTS | Yes |
| `talkback/tools.rs` | Tool contracts and execution | Partly |
| `talkback/turn.rs` | Streaming turn detector (energy + hangover) | Yes |
| `talkback/audio.rs` | Talkback-only mic worker (cpal) | No — device I/O |
| `talkback/engine.rs` | Orchestration, cancellation, event emission | Partly |
| `tts/mod.rs` | `TtsProvider` trait, capabilities, resolution | Yes |
| `tts/piper.rs` | Piper provider (was `tts::TtsEngine`) | No — subprocess |

## 5. State machine

Backend-owned. The frontend renders it and never invents it.

```text
OFF ──enable──→ STARTING ──ready──→ LISTENING ⇄ USER_SPEAKING
                    │                   ▲              │ speech end
                    │ fail              │              ▼
                    ▼                   │        TRANSCRIBING
                  ERROR ───────────────┐│              │ transcript
                    ▲                  ││              ▼
                    │            playback done      THINKING
                    │                  ││              │ first phrase
                    │                  │└─────────── SPEAKING
                    │                  │                │ user speaks
                    └── any ── fail ───┘          INTERRUPTED
                                                        │
                                              (→ USER_SPEAKING)
```

`disable` returns to `OFF` from any state. Every transition is a total
function `TalkbackState::apply(event) -> Result<TalkbackState, _>`, so an
illegal transition is a test failure rather than a UI glitch.

## 6. Retrieval

One canonical entry point:

```rust
retrieve(&query) -> RetrievalResult      // query: text, source_types,
                                         // max_results, char_budget, since
```

Stages, in order:

1. **Normalize** — lowercase, strip punctuation, drop stopwords, keep a
   phrase form for exact-match bonus.
2. **Gather candidates** — every Voice Note, Scribble, Meeting summary and
   MeetingFacts row, projected into a uniform `CandidateDoc`.
3. **Score** — IDF-weighted term overlap + title boost + exact-phrase bonus
   + per-source weight + recency decay. Deterministic and unit-tested.
4. **Expand** — one hop from matched Scribbles along `relationships` and
   shared `topics`, at a discounted score. One hop only: full graph
   traversal costs latency and buys noise.
5. **Deduplicate** — by `(source_type, source_id)`, keeping the best score;
   a Meeting and its MeetingFacts collapse to the facts row.
6. **Budget** — trim excerpts to fit `char_budget`, derived from the
   provider's `context_tokens`, never a fixed constant.

**Source weights** encode the Notion/Granola lesson: derived intelligence
beats raw transcript.

```text
MeetingFacts 1.25  >  Scribble 1.10  >  Meeting summary 1.00  >  Voice Note 0.95
```

Voice Notes score lowest not because they matter least but because they are
verbatim dictation — high recall, low signal density.

### Why not vectors, yet

There is no embedding pipeline in Relay (see `RESEARCH.md` §E.1) — the
LanceDB in Decision 6 was never built. Scoring is therefore lexical, and
`score_candidate` is a single function behind which a hybrid lexical +
embedding score can be substituted without touching the rest of the
pipeline. The honest position is: **retrieval is lexical today, and the
seam for semantic retrieval is `retrieval::score_candidate`.**

## 7. Personal-memory policy

`intent.rs` classifies a turn before anything else runs. A
`PersonalMemory` turn ("what did I say about…", "did we decide…", "do you
remember…", "last time…") gets:

- retrieval that is **mandatory**, not best-effort;
- a system prompt that forbids answering from model knowledge;
- when nothing is retrieved, a fixed honest response — *"I couldn't find
  that in your Relay data."* — produced without calling the LLM at all.

That last point matters: the cheapest way to never hallucinate a memory is
not to ask a model to avoid it.

## 8. Streaming and interruption

- **LLM**: `LLMClient::complete_streaming` parses provider stream formats
  (Ollama NDJSON, OpenAI/Anthropic/Gemini SSE) through one pure
  `parse_stream_chunk` function, so every provider is testable offline.
- **Phrase buffer**: deltas accumulate until a sentence boundary that is
  not a decimal or a known abbreviation, with a soft split at clause
  boundaries past ~140 chars and a hard flush at 240. Tokens never reach
  TTS; sentences do.
- **TTS**: one short synthesis per phrase. That is what makes a batch
  engine like Piper behave like a streaming one — time-to-first-audio is
  the cost of the *first sentence*, not the whole answer.
- **Playback**: in the WebView, not in Rust. `rodio` would drag in a second
  `cpal` (see `RESEARCH.md` §E.4), and the Web Audio API already gives
  queueing and instant cancellation.
- **Cancellation**: one `CancellationToken` per turn, checked before each
  LLM chunk, each synthesis, and each emit. Interrupting sets it, clears
  the frontend queue, and starts a new turn.

## 9. Activation

```text
activation_mode: toggle | wake_word
```

V1 implements `toggle` only, and **adds no global hotkey** — Relay already
has two and a third would be one too many. `wake_word` is a value the
settings type accepts and the engine rejects with a clear error, so the
seam is real rather than aspirational. No always-on listener ships.

## 10. Privacy

- Talkback OFF means **no Talkback microphone stream exists** — the cpal
  stream is created on enable and dropped on disable, not merely muted.
- Talkback and dictation cannot hold the microphone at the same time;
  each refuses with a specific error code rather than fighting for the
  device.
- Retrieved context goes wherever the user's configured LLM provider goes.
  With Ollama that is localhost. With a cloud provider it is that vendor —
  which is why the retriever has a `char_budget` and sends excerpts, never
  the vault.
- Observability logs ids, durations and counts. It never logs transcript
  text, retrieved content, or audio.

## 11. What Talkback deliberately does not do

- No second knowledge base, no Talkback notes, no Talkback vector store.
- No new Scribble or Voice Note schema — the tools call the existing
  persistence functions.
- No changes to the meeting recording clock, the 30-second chunk writer,
  crash recovery, Universal Dictation, or the production STT path.
- No destructive or outbound tools (email, calendar, deletion). The tool
  registry has room for them; V1 registers four read/create tools.
