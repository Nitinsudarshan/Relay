# Push-to-Talk Pill Redesign Update Report

## What Changed
- **Overlapping Logo Bug Fix**: Completely removed the circular black Relay logo component that was overlapping and clipping through the middle of the pill text.
- **Oscar-Style Edge Notch**: Replaced floating resting dot with a slim horizontal notch (`w-16 h-2`) attached flush to the screen edge when idle.
- **Floating Hotkey Hint Bar**: Added an off-white floating pill above the main surface (`Hold to record [Ctrl] [Space]`) that appears smoothly on hover/activation.
- **Branding Clean-Up**: Removed "RELAY" brand text, keeping the interface minimal and focused on utility (displaying optional active process name e.g. `● SNIPPING TOOL`).
- **Oscar Visual Surface**: Redesigned the pill with warm off-white and dark theme aesthetics (`#faf8f5`), rounded-full geometry, warm copper sparkle button (`✦`), and dropdown chevron (`⌄`).
- **Real Settings Dropdown**: Connected dropdown options (Auto-paste, Text transform, Cleanup style: Faithful/Clean/Professional/Concise, Prompt mode: "Rewrite speech into a prompt", and Language: English/Hinglish/Auto).
- **Inspection & Decision Log**: Created `docs/inspect/push-to-talk-pill.md` and `docs/decisions/push-to-talk-pill.md`.

---

## Files Modified / Created

- `docs/inspect/push-to-talk-pill.md` [NEW]
- `docs/decisions/push-to-talk-pill.md` [NEW]
- `update.md` [NEW]
- `native/src/components/capture/PillTypes.ts`
- `native/src/components/capture/PillSettingsPopover.tsx`
- `native/src/components/capture/DictationPill.tsx`
- `native/src-tauri/src/overlay.rs`
- `native/src-tauri/src/commands.rs`
- `native/src-tauri/src/lib.rs`

---

## Functionality Preserved

1. **Global Hotkey Registration**: `Ctrl + Space` global shortcut remains authoritative and triggers capture without hover.
2. **Audio Capture Pipeline**: RMS audio level calculation and `start_capture` / `stop_capture` session management preserved.
3. **Local Whisper STT & Models**: Model verification and auto-download path (`ensure_stt_model_ready`) intact.
4. **LLM Transformation & Prompt Mode**: Ollama / Cloud LLM provider connections and prompt mode transformation pipeline preserved.
5. **Clipboard & Injection**: Auto-paste, clipboard restoration, and focus preservation intact.

---

## Tests Performed & Packaging Verification

1. **Frontend Type Check & Vite Production Build**:
   ```powershell
   cd native; npm run build
   ```
   *Result*: Clean exit code `0` (built in 2.21s).

2. **Rust Backend Compilation**:
   ```powershell
   cargo check --manifest-path native/src-tauri/Cargo.toml
   ```
   *Result*: Clean exit code `0` (compiled in 1.78s).

3. **Knowledge Graph Update**:
   ```powershell
   graphify update .
   ```
   *Result*: Clean exit code `0` (updated graphify-out knowledge graph).

---

## Known Limitations & Remaining TODOs

- Active application detection currently defaults to process name formatting (e.g. `SNIPPING TOOL`, `CHROME`); can be extended with native Win32 foreground handle APIs.
