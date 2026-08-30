# Talkback — measurements

Written against **v0.18.0**. Everything below is either a number this
machine actually produced or an explicit statement that it was **not
measured**. Nothing here is estimated and presented as measured.

## The honest headline

**Relay's end-to-end Talkback latency has not been measured.** The three
components that dominate it — Whisper STT, the LLM, and Piper TTS — need a
Whisper model, a running Ollama, and a Piper binary respectively, and none
of the three exists in the Linux CI container this work was built in. The
brief asked for measurements on the real target hardware (Windows), and
that is the one thing that cannot be done from here.

What *was* measured is the code this change adds: retrieval and phrase
chunking. Those are the parts whose cost was previously unknown, and they
turn out to be small enough not to matter, which is itself the useful
result.

## Environment

| | |
|---|---|
| CPU | Intel Xeon @ 2.30 GHz, 4 vCPU |
| RAM | 15 GB |
| OS | Ubuntu 24.04 (Linux 6.18) — **not** Relay's Windows target |
| Toolchain | rustc 1.98.0, `--release` |

Reproduce with:

```bash
cd native/src-tauri
cargo test --release -- --ignored --nocapture
```

## Retrieval

One full pass — gather-equivalent scoring, IDF, one-hop expansion,
deduplication and budget-trimming — over a synthetic corpus of ~3.6 kB
documents, mixed Voice Notes and MeetingFacts, with a six-word query.

| Corpus | Per query |
|---|---|
| 100 docs | **0.33 ms** |
| 500 docs | **1.42 ms** |
| 1,000 docs | **2.66 ms** |
| 5,000 docs | **16.00 ms** |

Linear in corpus size, as a full scan must be. The number that matters:
**at 1,000 documents retrieval costs under 3 ms**, which is roughly 0.2%
of a 1.5-second turn budget. Retrieval is not the latency problem, and
optimising it before measuring the model would be the wrong work.

The caveat this table does *not* cover is I/O. These runs score
already-loaded documents; `sources::gather_candidates` reads and parses
every note and meeting from disk on each turn, and that cost is
**unmeasured** — it is filesystem-bound and would be dominated by the
user's disk and vault size. If a real vault shows this to be the
bottleneck, the fix is an index, not a cache in front of a linear scan.

## Phrase buffer

Character-by-character streaming — the worst case a token stream can
produce — through `PhraseBuffer`.

| | |
|---|---|
| Throughput | **0.28 µs/char** |
| 410,000 chars | 115 ms total, 6,000 phrases released |

A 400-character spoken answer therefore costs about **0.11 ms** to chunk.
Against a synthesis step measured in hundreds of milliseconds, the buffer
is free — which is the point: it exists to *reduce* time-to-first-audio,
and would be self-defeating if it added meaningful latency.

## What is not measured, and what it would take

| Stage | Status | What is needed |
|---|---|---|
| STT (`whisper-rs`, `ggml-base`/`ggml-small`) | **Not measured** | A downloaded model and real speech. Relay's existing `capture/evaluation.rs` harness already measures this path for dictation; Talkback reuses the same `StreamingTranscriber`, so the dictation numbers (~0.8 s fast / ~2.4 s accurate, per `capture/stt.rs`) are the closest existing evidence — **for a different window size**, so they are indicative, not a Talkback measurement. |
| Retrieval I/O | **Not measured** | A populated vault on the target disk. |
| LLM first token | **Not measured** | A running Ollama with the configured model. The streaming path is unit-tested against captured provider frames (`providers::parse_stream_line`), so correctness is verified; timing is not. |
| TTS first audio | **Not measured** | A Piper binary and voice model. |
| End-to-end turn | **Not measured** | All of the above, on Windows. |

The instrumentation to capture every one of these on the real machine
**does** ship: `TurnMetrics` records `stt_ms`, `retrieval_ms`,
`llm_first_token_ms`, `llm_total_ms`, `tts_first_audio_ms` and `total_ms`
per turn, emits them on `talkback-metrics`, and the Talkback page renders
the last turn's numbers in its sidebar. The first real conversation on a
Windows machine fills this table in.

## The TTS comparison the brief asked for

Piper, Kokoro and Chatterbox were **not** benchmarked, because benchmarking
them here would have meant reporting numbers from a Linux container with no
GPU as though they said something about a Windows laptop. What was done
instead is the work that has to come first: `TtsProvider` exists, Piper sits
behind it, and a second provider can be added and measured without touching
anything else.

The evidence gathered *for* that comparison — licences, weight licences,
runtime dependencies, language coverage, streaming support and the Windows
packaging risk for each — is in [`RESEARCH.md` §B](RESEARCH.md#b-open-source-technology-matrix).
The one finding that would have survived any benchmark: **Chatterbox needs
a PyTorch runtime**, which disqualifies it as a Tauri dependency regardless
of how good it sounds.

## Latency targets, for when the numbers exist

From the voice-agent literature (`RESEARCH.md` §A), and adjusted for a
fully local CPU stack:

| Measurement | Hosted-agent norm | Relay's target |
|---|---|---|
| Barge-in (speech → TTS flush) | < 150 ms | < 250 ms — bounded by the 100 ms frame plus one in-flight phrase |
| Turn gap (silence → first audio) | 200–450 ms | **1.5–2 s warm path** |
| Unacceptable | > 1500 ms | > 4 s |

Relay will not hit 450 ms locally, and claiming otherwise would be
dishonest: the 700 ms end-of-turn hangover alone spends more than that
budget before STT starts. The 1.5–2 s target is what a local
Whisper + local LLM + Piper chain can plausibly reach, and the metrics
above are how that gets confirmed rather than asserted.
