# Push-to-Talk Pill Update Report (v0.4.0)

## What Changed
- **STT Model Validation & Error Resolution**:
  - Updated `ensure_stt_model_ready` in `commands.rs` to verify that `whisper_model_path` actually exists on disk before declaring `Ready`.
  - Simplified the error string from `No local Whisper model configured. Set a GGML model path (e.g. ggml-base.en.bin) in Provider Settings.` to clean, concise text (`Set Whisper model path in Provider Settings.`).
- **Interactive Error Action**:
  - Added click handler to the error banner in `DictationPill.tsx`: clicking the red error message immediately opens the Settings dropdown popover.
  - Added `max-w-[260px]` text truncation to prevent long error strings from causing pill overflow.

---

## Files Modified

- `native/src/components/capture/DictationPill.tsx`
- `native/src-tauri/src/capture/stt.rs`
- `native/src-tauri/src/commands.rs`
- `VERSION`
- `CHANGELOG.md`
- `update.md`

---

## Verification Results

1. **Frontend Production Build**: `npm run build` completed with code `0` (built in 2.19s).
2. **Rust Backend Check**: `cargo check` completed with code `0` (0 warnings).
3. **Knowledge Graph**: `graphify update .` completed with code `0`.
