# Prompt Mode — Archived Product & Architecture Context

> **Status: ARCHIVED**
>
> Prompt Mode has been **removed** from Relay as of v0.15.0.
>
> This document preserves the context behind the feature.
> The future Action / Ask / Transform concept is **not currently implemented**.
> This document is **not** a specification for immediate implementation.
> Any future implementation should be designed independently using this context.

---

## 1. What Prompt Mode Was

Prompt Mode was intended to be a generic AI transformation capability inside Relay.

The basic conceptual flow was:

```
Source Object
      ↓
User Instruction / Prompt
      ↓
      LLM
      ↓
Transformed Output
```

The source could conceptually be different Relay objects, including:

- Voice Notes
- Scribbles
- Meetings
- Transcript content
- Selected text
- Potentially Tasks or other Relay objects in the future

The user could provide an instruction describing what they wanted done with the selected/source content.

Examples:

- Voice note → concise email
- Voice note → scribble
- Scribble → professional update
- Meeting → decisions
- Meeting → action items
- Meeting → follow-up message
- Text → rewrite
- Text → shorten
- Text → expand
- Text → summarize
- Text → extract tasks
- Multiple objects → combined summary
- Custom instruction → arbitrary transformation

The important idea was **not** simply "let users prompt an AI."

The broader idea was:

> "Take something that already exists in Relay and do something useful with it."

---

## 2. The Core Product Idea

The conceptual model was:

```
Object → Intent → Transformation → New Object
```

rather than the conventional:

```
User → Chat → AI Response
```

Prompt Mode was therefore intended to become a generic action/transformation layer over Relay's objects.

The long-term philosophy was:

> "Anything in Relay can be acted upon."

This means Relay could eventually treat its objects as inputs to useful operations.

For example:

```
Voice Note
    ↓
"Turn this into an email"
    ↓
Email/text draft
```

or:

```
Meeting
    ↓
"What do I need to follow up on?"
    ↓
Action items / tasks
```

or:

```
Scribble
    ↓
"Make this concise"
    ↓
Updated scribble/text
```

---

## 3. Capture → Understand → Act

The broader architectural idea behind Prompt Mode was that Relay could evolve into three layers.

### Capture

- Voice Notes
- Meetings
- Dictation
- Scribbles

### Understand

- Transcript
- Topics
- Entities
- Decisions
- Tasks
- People
- Context

### Act

- Summarize
- Rewrite
- Extract
- Transform
- Create
- Connect
- Ask

Prompt Mode was intended to sit primarily at the boundary between **Understand → Act**.

It would allow information Relay already captured and understood to be transformed into useful outputs.

---

## 4. "Prompt" Was Probably the Wrong Product Name

The word "Prompt" describes the mechanism used to communicate with an LLM.

It does not necessarily describe what the user is trying to do.

A user generally does not think:

> "I want to invoke a prompt."

They think:

> "I want to do something with this."

Potential future product concepts considered include:

- Ask Relay
- Actions
- Act on this
- Transform
- Command

The distinction is:

| Layer | Term |
|---|---|
| Internal engineering concept | Prompt |
| Potential user-facing concept | Action / Ask / Act on this / Transform |

"Prompt" may remain a useful internal engineering term, but it should not automatically be assumed to be the correct product-facing name for a future capability.

---

## 5. Prompt Mode Was Not a Chatbot

Prompt Mode should not be thought of simply as another chat interface.

The intended model was **object-centric**.

Instead of:

```
Chat → Prompt → Answer
```

the intended model was:

```
Relay Object → User Intent → Transformation → Relay Object / Useful Output
```

This distinction is important for future product design.

---

## 6. What the Implementation Actually Consisted Of

### Backend (Rust / Tauri)

1. **`settings/mod.rs`**:
   - `PromptItem` struct — a user-defined prompt template (`id`, `name`, `description`, `prompt_body`, `enabled`).
   - `PromptSettings` struct — `enabled: bool` and `prompt_hotkey: String` controlling whether Prompt Mode was active and its global hotkey.
   - `default_prompts()` — three built-in prompt templates: "Summarize into Bullet Points", "Draft Professional Email", "Extract Action Items".
   - Fields on `AppSettings`: `prompt_settings: PromptSettings` and `prompts: Vec<PromptItem>`.
   - Test: `test_prompts_settings_defaults_and_serialization`.

2. **`commands.rs`**:
   - `execute_prompt` Tauri command — resolved a prompt template (by `prompt_id` or inline `prompt_body`), interpolated `{{text}}` with user input, called `LLMClient::new(settings.provider).complete()`, returned the LLM response.
   - `update_hotkeys` — read `prompt_settings` to pass `prompt_hotkey` to `hotkeys::apply_hotkeys`.
   - `save_settings` — re-registered hotkeys including `prompt_hotkey` on settings save.

3. **`hotkeys/mod.rs`**:
   - `register_hotkeys`, `apply_hotkeys`, `try_register_hotkeys` all accepted `prompt_hotkey: Option<&str>`.
   - If prompt mode was enabled and the hotkey was non-empty and different from the dictation hotkey, a third global shortcut was registered.
   - The prompt hotkey called `on_dictation_pressed_with_mode(app, &prompt_state, "prompt")`, reusing the dictation capture pipeline with a `"prompt"` mode string.
   - The old `on_dictation_pressed` function (which hardcoded `"dictation"`) was left as dead code.

4. **`lib.rs`**:
   - Startup read `settings.prompt_settings`, conditionally passed `prompt_hotkey` to `hotkeys::register_hotkeys`.
   - `execute_prompt` was registered in the Tauri invoke handler.

### Frontend (TypeScript / React)

1. **`components/prompts/PromptsPage.tsx`** — full-page Prompt Library UI: list/add/edit/delete/reorder prompts, toggle individual prompts, "Test / Run" button launching the modal.
2. **`components/prompts/PromptTransformModal.tsx`** — modal dialog for executing a prompt: select template, enter custom instruction, call `invoke('execute_prompt')`, show result, copy to clipboard, optionally save as Scribble.
3. **`App.tsx`** — imported `PromptsPage`, added `'prompts'` to `MainTabType`, added "Prompts" hero header, rendered `<PromptsPage />` when active, computed `promptModeEnabled`, fell back from prompts tab if disabled.
4. **`components/common/NativeSidebar.tsx`** — "Prompts" nav item (Wand2 icon, sky-blue), filtered out when `!promptModeEnabled`.
5. **`components/voicenotes/VoiceNotePage.tsx`** — "Transform with Prompt (Wand)" icon button per voice note, opening `PromptTransformModal`.
6. **`components/scribble/ScribbleDetailEditor.tsx`** — "Prompt" button in scribble toolbar, opening `PromptTransformModal`.
7. **`components/settings/ProviderSettings.tsx`** — "Prompt Mode" toggle switch in General Settings, "Prompt Capture Hotkey" recorder in both General and Dictation & Audio sections, `applyPromptHotkey` handler.
8. **`types/index.ts`** — `PromptSettings` and `PromptItem` TypeScript interfaces, `prompt_settings` and `prompts` fields on `AppSettings`.

### Data

- Prompt templates were persisted in `settings.json` as `prompts: [...]` and `prompt_settings: { enabled, prompt_hotkey }`.
- No separate vault storage — prompts lived only in settings.

---

## 7. What Shared Infrastructure Prompt Mode Used (NOT Removed)

Prompt Mode was a **consumer** of shared LLM infrastructure. The following remain intact:

- `LLMClient` / `LLMResponse` / `providers::*` — used by Meetings processing, meeting summaries, scribble enrichment, and other AI features.
- `ProviderConfig` / provider selection — shared across all LLM consumers.
- `HotkeyRecorder.tsx` — shared component for all hotkey configuration.
- `AudioRecorder` / dictation capture pipeline — shared with Universal Dictation and Voice Note capture.
- `on_dictation_pressed_with_mode` — the `"dictation"` mode path is still required; only the `"prompt"` mode caller was removed.
- `HotkeyRegistrationStatus` — still reports dictation and show/hide status.
- `SttEngine` — untouched.
- Meeting processing, summaries, Kanban task extraction — untouched.
- Scribble enrichment and summarization — untouched.

---

## 8. Potential Future Action Model

A future Relay action system could conceptually look like:

```
ACTION
  ├── source
  ├── intent
  ├── context
  ├── output type
  ├── model/provider
  ├── transformation
  └── provenance
```

Conceptually:

```
transform(source, instruction, output_type)
```

**This is FUTURE DESIGN CONTEXT ONLY. Not implemented.**

---

## 9. Why This Idea Could Still Be Valuable

The underlying concept may still be strategically useful for Relay.

Relay contains multiple native object types:

- Voice Note
- Scribble
- Meeting
- Transcript
- Decision
- Action Item
- Task

A future transformation layer could connect these objects:

```
Voice Note ─────┐
Scribble ───────┤
Meeting ────────┤
Transcript ─────┼──→ Action / Transformation → Output
Task ───────────┤
Selection ──────┘
```

Potential outputs: Scribble, Task, Text, Summary, Message Draft, Meeting Artifact, Structured Data.

The interesting opportunity is not "Relay has AI prompting." The interesting opportunity is:

> "Everything captured in Relay can become something else."

---

## 10. Competitive Context

Other products have similar ideas under names such as:

- AI Actions / Custom AI Actions
- Ask AI
- Transformations
- Rewrite / Summarize / Extract

Examples that informed the thinking: OpenWhispr, Workflowy, Skriuw, Metis.

The potentially differentiated aspect is how deeply such transformations could integrate with Relay's own object model, provenance, meetings, tasks, scribbles, and local-first architecture.

---

## 11. Relationship to Meetings

A meeting can contain:

```
Meeting
├── Transcript
├── Conversation
├── Topics
├── Decisions
├── Action Items
├── Open Questions
└── Summary
```

A future action layer could allow operations such as:

- "What do I need to follow up on?"
- "Draft the follow-up email."
- "Give me the decisions."
- "Turn these action items into tasks."
- "Summarize this for someone who wasn't there."
- "What did I commit to?"
- "What questions are still unresolved?"

The important architectural principle is that a future action system should ideally use Relay's structured meeting representation and evidence chain rather than blindly sending the entire raw transcript to an LLM.

---

## 12. Provenance and Evidence

Derived meeting information retains links back to source transcript segments. A possible future chain:

```
Output → Transformation → Source Object → Source Segments → Original Transcript / Recording
```

This could allow Relay to answer "Why did Relay produce this?" and allow a user to navigate back to the underlying evidence.

Future AI transformations should ideally be grounded and explainable.

---

## 13. Why Prompt Mode Was Removed

The current decision is: **Remove Prompt Mode completely.**

The reason is that the current dedicated "Prompt Mode" concept/UX did not sufficiently justify existing as a standalone product mode.

The underlying idea of "take an existing Relay object and do something useful with it" may still be valuable. However, that idea should be revisited separately and deliberately rather than keeping the current Prompt Mode implementation alive.

Therefore:

- **Prompt Mode** = removed
- **Object transformation / Actions** = possible future design direction (not implemented)

---

## 14. Future Design Principle

If Relay revisits this concept later, the product should not automatically recreate "Prompt Mode."

Instead, evaluate a simpler user-facing model such as:

- **Actions**
- **Ask Relay**
- **Act on this**
- **Transform**

The underlying implementation could still use prompts internally. The user-facing concept and engineering mechanism do not have to have the same name.

---

## 15. Files Removed

### Backend (Rust)
- `settings/mod.rs`: `PromptSettings`, `PromptItem`, `default_prompt_hotkey`, `default_prompt_mode_enabled`, `default_prompt_enabled`, `default_prompts`, and the `prompt_settings` / `prompts` fields on `AppSettings`.
- `commands.rs`: `execute_prompt` command, prompt hotkey logic in `update_hotkeys` and `save_settings`.
- `hotkeys/mod.rs`: `prompt_hotkey` parameter on `register_hotkeys`, `apply_hotkeys`, `try_register_hotkeys`; the prompt hotkey registration block; the dead `on_dictation_pressed` function.
- `lib.rs`: `prompt_config` variable, `prompt_hotkey` conditional logic, `execute_prompt` in invoke handler registration.
- Settings test: `test_prompts_settings_defaults_and_serialization`.

### Frontend (TypeScript / React)
- `components/prompts/PromptsPage.tsx` — entire file deleted.
- `components/prompts/PromptTransformModal.tsx` — entire file deleted.
- `App.tsx`: `PromptsPage` import, `'prompts'` tab type, prompt hero header, `promptModeEnabled` logic, prompt tab redirect effect, `promptModeEnabled` prop.
- `components/common/NativeSidebar.tsx`: `Wand2` import, "Prompts" nav item, `promptModeEnabled` prop, `.filter(...)` for prompt visibility.
- `components/voicenotes/VoiceNotePage.tsx`: `PromptTransformModal` import, Wand2 import, prompt modal state, `promptModeEnabled`, `handleOpenPromptModal`, Wand button JSX, modal JSX.
- `components/scribble/ScribbleDetailEditor.tsx`: `PromptTransformModal` import, Wand2 import, prompt modal state, `promptModeEnabled`, Prompt button JSX, modal JSX.
- `components/settings/ProviderSettings.tsx`: `Wand2` import, `applyPromptHotkey` handler, "Prompt Mode" toggle section in General Settings, "Prompt Capture Hotkey" section in both General and Dictation & Audio.
- `types/index.ts`: `PromptSettings`, `PromptItem` interfaces, `prompt_settings` and `prompts` fields on `AppSettings`.

---

*Archived: v0.15.0 — Prompt Mode removal.*
