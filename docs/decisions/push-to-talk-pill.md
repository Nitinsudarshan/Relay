# Decision Log — Push-to-Talk Pill Redesign & Interaction Refinement

## Context
Relay's push-to-talk (PTT) interface required an interaction and visual refinement inspired by Oscar's minimalist desktop pill experience while retaining 100% of Relay's existing Rust backend, local Whisper, global hotkey registration, and LLM transformation capabilities.

---

## Decisions

### Decision 1: Oscar-Style Edge-Attached Horizontal Notch
- **Choice**: Replace the legacy large circular logo dot with a slim, horizontal notch attached directly to the screen edge when idle/hidden (`64px x 10px`).
- **Why**: Eliminates screen clutter and prevents the pill from acting like an annoying floating button over user content.
- **Alternatives Considered**: Keeping a floating circular badge.
- **Reason Rejected**: Floating circular badge frequently swallowed mouse clicks meant for underlying application windows and looked out of place.

### Decision 2: Removal of Heavy "RELAY" Branding Text
- **Choice**: Remove "RELAY" brand text from the collapsed pill label. Optionally display the active foreground application name (e.g. `● Chrome` or `● Snipping Tool`) or a simple state dot `● Click to dictate`.
- **Why**: Desktop utilities should be quiet and functional, prioritizing utility over branding clutter.

### Decision 3: Floating Hotkey Hint Bar (`Hold to record [Ctrl] [Space]`)
- **Choice**: Add an off-white floating hint pill directly above the main pill upon activation/hover displaying `Hold to record [Ctrl] [Space]`.
- **Why**: Matches Oscar's visual reference while providing clear keyboard affordance without forcing text into the main pill body.

### Decision 4: Smooth Hover State Transitions with Intent & Grace Delays
- **Choice**: 120ms enter intent delay, 1000ms hover-out grace delay, smooth width/opacity/shadow transitions.
- **Why**: Prevents sudden UI jumping and allows easy navigation between pill controls and the settings popover.

### Decision 5: Real Settings in Dropdown Surface
- **Choice**: Include Auto-paste (Toggle), Text transform (Toggle), Cleanup style (Faithful/Clean/Professional/Concise), Prompt mode (Toggle with "Rewrite speech into a prompt"), and Speech Language.
- **Why**: All dropdown controls map directly to Relay's capabilities without fake or simulated state.

---

## Trade-Offs & Mitigation
- **Native Window Sizing**: Dynamic window resizing in Tauri requires sending IPC calls (`set_pill_window_mode`) when transitioning between NOTCH, COLLAPSED, EXPANDED, and POPOVER modes.
- **Mitigation**: Rust overlay subsystem uses transparent, hit-region-optimized window bounding boxes to prevent dead-zone click interception.
