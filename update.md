# Push-to-Talk Pill Refinement Update Report (v0.3.7)

## What Changed
- **Top Clipping Fix**: Increased Rust `overlay.rs`'s `EXPANDED_SIZE` height to `150.0` and `POPOVER_SIZE` height to `420.0`. The top floating `Hold to record [Ctrl] [Space]` hint bar now sits comfortably with complete vertical clearance and zero horizontal top slicing.
- **Relay Light Theme Color Palette**: Re-themed main pill, notch, waveform bars, keyboard hint bar, and popover dropdown using Relay's native light mode design system:
  - Surface: Pure white card background (`#ffffff`).
  - Text: Slate-900 typography (`#0f172a`).
  - Primary Accent: Relay primary blue (`#2563eb` / `hsl(221, 83%, 53%)`) for waveform bars, active prompt sparkle mode, and toggle switches.
  - Borders & Badges: Slate-200 border (`#e2e8f0`) and Slate-100 key badges (`#f1f5f9`).

---

## Files Modified

- `native/src/components/capture/DictationPill.tsx`
- `native/src/components/capture/PillSettingsPopover.tsx`
- `native/src-tauri/src/overlay.rs`
- `VERSION`
- `CHANGELOG.md`
- `update.md`

---

## Verification Results

1. **Frontend Production Build**: `npm run build` completed with code `0` (built in 2.20s).
2. **Rust Backend Check**: `cargo check` completed with code `0` (0 warnings).
3. **Knowledge Graph**: `graphify update .` completed with code `0`.
