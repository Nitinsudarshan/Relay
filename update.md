# Push-to-Talk Pill Update Report (v0.4.1)

## What Changed
- **Unconditional Automatic GGML Whisper Model Downloader**:
  - Removed feature gating from `ensure_default_model` in `stt.rs`.
  - When the app launches or when `ensure_stt_model_ready` runs:
    1. Checks if `settings.stt.whisper_model_path` points to a valid file on disk.
    2. If missing or unconfigured, automatically downloads HuggingFace's default `ggml-tiny.en.bin` model into `%APPDATA%\Relay\models\ggml-tiny.en.bin`.
    3. Saves the path into `settings.json`.
    4. Automatically transitions the dictation pill state to `Ready` (`Click to dictate`), completely eliminating the missing model error banner!

---

## Files Modified

- `native/src-tauri/src/capture/stt.rs`
- `native/src-tauri/src/commands.rs`
- `VERSION`
- `CHANGELOG.md`
- `update.md`

---

## Verification Results

1. **Frontend Production Build**: `npm run build` completed with code `0` (built in 2.13s).
2. **Rust Backend Check**: `cargo check` completed with code `0` (0 warnings).
3. **Knowledge Graph**: `graphify update .` completed with code `0`.
