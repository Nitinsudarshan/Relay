# Relay — Push-to-Talk Pill Redesign & Interaction Overhaul

## 0. Role / execution mode

You are working directly inside the Relay repository.

Your task is to redesign and implement the desktop Push-to-Talk / Dictation Pill.

Do not start coding immediately.

First inspect the repository and understand:
- current Tauri architecture
- frontend framework and component structure
- existing DictationPill
- FloatingPill
- hotkey implementation
- recording/audio lifecycle
- Whisper/STT integration
- LLM/provider integration
- settings/state management
- desktop window positioning
- taskbar/work-area handling
- current build/package configuration
- existing tests
- existing styling/design system

The existing implementation is the source of truth for actual functionality.

The Oscar screenshots supplied with this task are design and interaction references only.

Do not copy Oscar branding, assets, exact visual identity, proprietary implementation, or code.

The desired result is:
Relay's own Push-to-Talk experience, inspired by the interaction model and progressive disclosure of Oscar.

---

## 1. Primary objective

Replace/refactor the existing Relay floating dictation pill into a polished desktop PTT surface that:
- is extremely minimal when idle
- expands smoothly when approached/hovered
- supports click-to-dictate
- supports the global PTT hotkey
- visually communicates recording/listening
- communicates transcription/processing states
- supports Prompt Mode
- exposes useful settings through a compact popover
- handles taskbar/work-area changes correctly
- never gets hidden behind the Windows taskbar
- does not abruptly disappear when the pointer moves between the pill and its popover
- clearly identifies unavailable/unconfigured dependencies
- never presents fake/dummy functionality as working functionality
- remains usable without an LLM where possible
- preserves the existing Relay backend/capture pipeline

---

## 2. Important reference

The attached Oscar screenshots show the desired interaction philosophy:

- **Resting**: Very small / almost notch-like.
- **Hover**: Expands into a compact horizontal pill.
- **Click**: Starts dictation.
- **Global hotkey**: Activates the same recording flow.
- **Chevron**: Opens a compact settings popover.
- **Sparkle / transform control**: Represents prompt/AI transformation functionality.
- **Settings popover**: Uses progressive disclosure instead of putting every setting permanently inside the pill.

Use these ideas as UX references. Do not reproduce Oscar literally.

---

## 3. First task — repository reconnaissance

Before modifying code, inspect the repository.

Find and document:

### Pill
- Pill
- DictationPill
- FloatingPill
- related components
- CSS/Tailwind/style files
- any overlay/window implementation

### Audio
Find:
- microphone capture
- recording start
- recording stop
- audio stream lifecycle
- waveform generation
- Whisper invocation
- transcription state

### Hotkey
Find:
- global hotkey registration
- Ctrl+Space
- fallback hotkey
- hotkey conflict handling
- accessibility requirements
- retry behaviour

### AI
Find:
- Ollama
- cloud LLM
- provider configuration
- model configuration
- prompt mode
- transformation
- meeting/scribble processing

### Settings
Find the source of truth for:
- auto paste
- transform
- prompt mode
- language
- cleanup style
- provider
- model
- hotkey

### Desktop positioning
Find:
- Tauri window creation
- always-on-top
- transparent window
- frameless window
- monitor detection
- work-area detection
- taskbar handling
- window movement
- resize logic

### Packaging
Inspect:
- package.json
- Tauri configuration
- Windows bundling configuration
- installer configuration
- model/resource packaging
- first-run behaviour

Do not duplicate existing functionality.

---

## 4. Create an implementation plan before coding

Before making code changes, produce an internal implementation plan covering:

Existing architecture  
↓  
Current pill lifecycle  
↓  
Required state machine  
↓  
UI changes  
↓  
Window/positioning changes  
↓  
Settings integration  
↓  
Dependency warnings  
↓  
Testing  
↓  
Packaging validation  

Then implement.

---

## 5. New pill UX

The pill should have these major states:

RESTING  
↓  
HOVER / EXPANDED  
↓  
LISTENING  
↓  
TRANSCRIBING  
↓  
PROCESSING  
↓  
SUCCESS  
↓  
RESTING  

And:

ANY STATE  
↓  
ERROR / WARNING  

---

## 6. RESTING STATE

The idle state should be extremely minimal.

Conceptually:
`◉` or a very small Relay-branded notch/surface.

It should not look like a normal application button. It should feel like: "Relay is quietly available."

### Requirements
- minimal width
- minimal visual noise
- floating
- always-on-top where current architecture requires it
- does not obstruct normal desktop use
- accessible by hover
- accessible by hotkey
- does not require clicking a tiny target to use PTT

Do not remove Relay branding entirely. Use a subtle Relay indicator/logo/state indicator.

---

## 7. HOVER / EXPANDED STATE

On pointer approach/hover, smoothly expand.

Target structure:
```
┌──────────────────────────────────────────────┐
│ • RELAY     Click to dictate        ✦    ˅   │
└──────────────────────────────────────────────┘
```

The exact typography/layout should follow Relay's visual system.

- **Left**: Small state/application indicator (`• RELAY` or appropriate Relay terminology).
- **Main action**: `Click to dictate` (primary interaction).
- **Transform / Prompt action**: Sparkle-style button (`✦`) representing Prompt Mode / transformation.
- **Settings**: Chevron (`⌄`) opens the settings popover.

---

## 8. Do NOT implement hover as two unrelated components

The current implementation conditionally switches between collapsed and expanded structures.

Do not preserve that approach if it prevents smooth animation.

Prefer one visual component whose:
- width
- opacity
- content visibility
- spacing
- border radius
- transform

animate between states. The transition should feel like one object expanding.

---

## 9. Hover animation

The current interaction feels too abrupt. Implement deliberate motion.

- **Enter**: pointer approaches → small intent delay (100–150ms) → expand.
- **Expansion**: Target 200–300ms. Use an appropriate easing curve. Do not use an overly bouncy animation.
- The desired feeling is: calm / premium / desktop-native, not animated UI component.

---

## 10. Hover-out delay

When the pointer leaves the pill: DO NOT immediately collapse it. Wait approximately **1000ms** before collapsing.
If the pointer returns during that period: cancel collapse.

This is especially important when the user moves: `pill` → `settings popover`.
The popover must not disappear while the user is trying to interact with it.
Use a shared hover/interaction region if necessary.

---

## 11. Settings popover

The chevron opens a compact settings popover.

Conceptual design:
```
┌─────────────────────────────────────┐
│ Auto-paste after dictation       ON │
│                                     │
│ Text transform                   ON │
│ ─────────────────────────────────── │
│ ✎ Cleanup style              Faithful⌄│
│                                     │
│ Prompt mode                      OFF │
│ Rewrite speech into a prompt        │
│ ─────────────────────────────────── │
│ ◎ Language                    English⌄│
└─────────────────────────────────────┘
```

*Important*: Do not blindly implement these exact settings. Every setting must map to an actual Relay capability. Inspect the repository first.

---

## 12. Settings rule — NO FAKE CONTROLS

If a UI control represents functionality that is not actually implemented: do not present it as working.

Instead use one of:
- **Option A — warning**: `⚠ Ollama not configured` / `Configure in Settings →`
- **Option B — disabled state**: `Prompt mode` / `Unavailable — configure an LLM`
- **Option C — setup CTA**: `Prompt mode` / `Requires an LLM` / `[Configure]`

Never make a fake toggle that appears functional.

---

## 13. Dummy-data / placeholder detection

Apply this principle throughout the Relay UI. Any component that uses dummy data, placeholder model names, fake status, hardcoded provider state, mock statistics, sample meeting information, fake availability, or simulated backend status must visually communicate that fact during development/testing.

---

## 14. Ollama status MUST be truthful

The check should confirm as appropriate:
- Ollama endpoint is reachable
- Ollama responds successfully
- required API is available
- configured target model exists/is available

- If Ollama is not available: `⚠ Ollama unavailable`
- If Ollama is reachable but model is missing: `⚠ Model not installed`
- If Ollama is available and model exists: `✓ Ollama ready`

Do not report running based solely on configuration.

---

## 15. Whisper status MUST also be truthful

The UI must clearly distinguish:
- Ready: `✓ Whisper ready`
- Download required: `⚠ Whisper model required` / `Download model →`
- Downloading: `Downloading Whisper model` `[██████░░░░ 60%]`
- Failed: `⚠ Whisper download failed` / `Retry`
- Custom model configured: `✓ Custom Whisper model`

---

## 16. Prompt Mode

Prompt Mode should be a first-class interaction. The sparkle button should provide access to it.
- OFF: `Click to dictate ✦`
- ON: `Click to prompt ✦` (or visually distinguishable state)

Prompt mode should only be enabled if the actual processing pipeline exists. If no LLM is configured: `⚠ Prompt mode requires an LLM` / `[Configure LLM]`.

---

## 17. LLM architecture

Relay should distinguish Provider, Model, Availability, Configuration.
The pill itself should not become a model-selection interface. Provider/model configuration belongs in Settings. The pill only needs to communicate whether the requested capability is available.

---

## 18. Recording state

When recording begins, the pill should transition smoothly:
```
┌────────────────────────────────────────┐
│ • RELAY      〰〰〰〰〰〰〰         ■  │
└────────────────────────────────────────┘
```
Waveform should communicate microphone is actively recording. The stop control must be obvious.

---

## 19. Hotkey activation

The existing global hotkey (`Ctrl + Space` or configured) remains authoritative.
On activation: RESTING → LISTENING without requiring hover.

---

## 20. Hotkey conflict

If hotkey cannot be registered, show a meaningful warning with options to change/retry.

---

## 21. Processing states

- Transcription: `• RELAY Transcribing... ◌`
- AI processing: `• RELAY Processing... ◌`
- Success: `• RELAY ✓ Text inserted`
- Error: `• RELAY ⚠ Something went wrong`

---

## 22. State machine

```typescript
type PillState =
  | "resting"
  | "expanded"
  | "listening"
  | "transcribing"
  | "processing"
  | "success"
  | "error"
  | "warning";
```
Visual layer consumes authoritative backend state.

---

## 23. Taskbar positioning

Fix at window-positioning layer. Position relative to monitor work area, above taskbar.

---

## 24. Dynamic taskbar behaviour

Adapt dynamically based on taskbar visibility and autohide state.

---

## 25. Multi-monitor support

Determine monitor floating pill belongs to. Handle monitor changes, DPI scaling, resolution changes correctly.

---

## 26. Window behaviour

Preserve always-on-top, transparency, frameless, click interaction, global hotkey interaction, desktop positioning.

---

## 27. Visual design principles

Clarity > decoration. Function > animation. Oscar-inspired minimalism & progressive disclosure.

---

## 28. Accessibility

Keyboard accessibility, contrast, focus states, accessible labels, aria-labels for icons.

---

## 29. Do not break existing functionality

Preserve capture, Whisper transcription, auto-paste, hotkey, clipboard, meeting capture, scribble capture, LLM processing, settings persistence.

---

## 30. Development-only diagnostics

Add debug/diagnostic window or state view for state, STT, LLM, Hotkey, Taskbar, Work Area details.

---

## 31. Testing matrix

Test all pill states, recording flows, dependency states, hotkey events, OS taskbar/scaling variations.

---

## 32. Important — inspect rather than assume

Verify feature existence before exposing options. Expose only working portions.

---

## 33. Code quality requirements

Centralized state, reusable animation constants, reusable status helpers, typed state.

---

## 34. Suggested component structure

Adapt to existing structure.

---

## 35. Dependency status architecture

Standardized `DependencyStatus` type.

---

## 36. Packaging requirement

Must work in both Dev (`npm run dev:native`) and Packaged Windows installer.

---

## 37. First-run experience

Truthful dependency setup prompt.

---

## 38. Acceptance criteria

UX, OS integration, Dependency truthfulness, Engineering requirements met.

---

## 39. Required final review

Self-review before declaring completion.

---

## 40. Final deliverables

Changed files list, architecture summary, behavior summary, dependency report, test results, known limitations.
