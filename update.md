# Push-to-Talk Pill Refinement Update Report (v0.3.9)

## What Changed
- **Removed Snipping Tool Process Label**: Removed `● SNIPPING TOOL` / process indicator block from the dictation pill.
- **Application Dark Theme Color Synchronization**: Synchronized the dictation pill, edge handle, floating hint bar, key badges, and settings popover to use Relay's exact neutral dark theme palette:
  - Surface: Card background `#171717` (matching Relay's dark theme dashboard cards).
  - Borders: Neutral dark border `#262626`.
  - Badges & Hover: Neutral dark `#262626` / `#404040`.
  - Typography: Neutral 100 `#fafafa` foreground text and `#a3a3a3` muted text.

---

## Files Modified

- `native/src/components/capture/DictationPill.tsx`
- `native/src/components/capture/PillSettingsPopover.tsx`
- `VERSION`
- `CHANGELOG.md`
- `update.md`

---

## Verification Results

1. **Frontend Production Build**: `npm run build` completed with code `0` (built in 1.97s).
2. **Rust Backend Check**: `cargo check` completed with code `0` (0 warnings).
3. **Knowledge Graph**: `graphify update .` completed with code `0`.
