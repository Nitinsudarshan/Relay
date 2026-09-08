# Spike — Does `ort` build, bundle and run on Windows?

**Status**: written, never run. Needs a Windows machine.
**What has been checked**: the spike type-checks against `ort` 2.0.0-rc.13, and
the generated ONNX model has tests that re-parse it. Neither was linked or
executed — the container this was written in cannot reach `cdn.pyke.io`, where
`ort` fetches its prebuilt runtime, so `cargo check --features onnx-spike`
fails there with a proxy 403 that says nothing about Windows.
**Blocks**: `maybe_later.md` item 1 (T4 smart turn detection, Silero VAD, wake
word) and item 3's neighbour, G6 denoise. Both are approved in principle and
neither should be started until this passes.
**Owner**: whoever has the Windows box. It is a half-hour job.

---

## The question

Not "can Rust run an ONNX model" — that is known. The question is whether an
ONNX Runtime linked into **Relay's own Tauri binary** survives being packaged
and installed:

1. **Builds.** Does `ort` compile against the MSVC toolchain Relay is already
   built with, without a second toolchain or a CMake surprise?
2. **Bundles.** Does whatever `ort` needs at runtime — a static library, or
   `onnxruntime.dll` — get carried into the installer that `tauri build`
   produces, or does it only exist beside `target/release/relay.exe`?
3. **Runs.** Does an *installed* Relay, launched from the Start menu on a
   machine that never had a build toolchain, load the runtime and execute a
   graph?

Failure 2 is the one worth spending a spike on. It does not look like a build
error; it looks like a feature that works for the developer and crashes for
everybody else.

## What the spike is

A feature-gated module — `native/src-tauri/src/developer/onnx_spike.rs` — that
runs at launch and writes a JSON report. It is compiled into the real Relay
binary rather than a standalone example on purpose: a standalone `cargo run`
answers question 1 and neither of the others.

It builds its own model rather than loading one. `tiny_add_model()` emits a
valid ONNX graph — `c = a + b` over three floats — as bytes. Relay ships no
model files and a spike is a poor reason to be the first, and this also keeps
the model-distribution question (below) out of the packaging answer.

## Running it

From `native/`:

```powershell
# 1. Builds? — the fastest signal, ~5 minutes cold.
cargo check --manifest-path src-tauri/Cargo.toml --features onnx-spike
#    ORT_SKIP_DOWNLOAD=1 type-checks without fetching the runtime, which is how
#    the spike's own code was validated; it proves nothing about linking.

# 2. Runs in dev?
npm run tauri dev -- --features onnx-spike

# 3. Bundles? — the actual question.
npm run tauri build -- --features onnx-spike
```

Then **install the artifact from `src-tauri/target/release/bundle/`** — the MSI
or the NSIS `-setup.exe`, not the loose `relay.exe` — and launch the installed
app. Ideally on a second machine, or at minimum from the installed location
rather than the build tree.

## Reading the result

The report is written to `%TEMP%\relay-onnx-spike.json` on every launch of a
spike build. It also goes to the log, which on a bundled Windows binary you
will not see — the file is the channel.

```jsonc
{
  "ort_compiled_in": true,        // false = the feature flag did not take
  "exe_dir": "C:\\Program Files\\Relay",
  "dylib_beside_exe": [],         // empty is GOOD — see below
  "model_bytes": 213,
  "inference": {
    "expected": [11.0, 22.0, 33.0],
    "actual":   [11.0, 22.0, 33.0],
    "correct": true,              // the line that decides it
    "elapsed_ms": 12
  },
  "error": null
}
```

**Pass** = `correct: true` from the *installed* app. That is all three
questions answered at once.

| Symptom | What it means | Next move |
|---|---|---|
| `cargo check` fails | Question 1. Read the error; `ort` may want a newer MSVC or a `CMAKE` on PATH. | Note the requirement — it becomes a CI prerequisite. |
| Dev run passes, installed run has `error` mentioning a missing DLL | Question 2, and the reason this spike exists. | Add the dylib to `tauri.conf.json`'s `bundle.resources` (or `externalBin`) and re-run from step 3. Record the recipe. |
| `dylib_beside_exe` is empty and `correct: true` | Static link. The best outcome — nothing to bundle. | Say so in the brief; the packaging risk for G6/T4 is gone. |
| `dylib_beside_exe` lists a DLL and `correct: true` | Works, but the installer is now carrying a file. | Check the installer size delta and that the DLL is signed the way the rest of the bundle is. |
| `correct: false` with numbers | The runtime ran and disagreed with arithmetic. | Not a packaging problem. Report it upstream; the fixture is in the same file. |
| `error` mentions the model, an opset, or a missing input named `a` | The generated fixture, not the runtime. | The encoder has tests (`cargo test onnx_spike`); a failure here means they missed something. |

## Not part of this spike

**Where real models come from.** G6 and T4 need actual weights — Silero VAD,
Smart Turn v3, a denoiser. Relay's README says it bundles no models, and
`large-v3-turbo` now establishes the alternative: download on demand, into the
same place the Whisper models go. Choosing between amending the README and
downloading on demand is a product decision and it comes *after* this passes,
not as part of it.

**Model size and licensing.** Silero VAD is MIT, Smart Turn v3 is open-weights;
both need checking against what Relay actually redistributes once the answer
above is settled.

## Cleaning up

The spike is throwaway. Once it has answered, either delete
`developer::onnx_spike`, the `onnx-spike` feature and the `ort` dependency, or
— if the answer was "works, statically linked" — keep the `ort` line and delete
the rest. Do not leave a launch-time side effect in a shipped configuration:
the feature is off by default and must stay that way.
