---
trigger: model_decision
description: Requirements and conventions for logging deferred, postponed, or speculative features to maybe_later.md.
---

# Deferred Features & "Maybe Later" Backlog (`maybe_later.md`)

When designing, refactoring, or cleaning up Relay's surfaces (Native Desktop, Rust Backend, Web Dashboard), speculative affordances, half-implemented features, or deprioritized UX enhancements must **never** be left in the code as ghost UI, misleading labels, or dead code.

Instead, they must be cleanly removed from the active surface and documented in [`maybe_later.md`](file:///d:/Projects/Relay/maybe_later.md) at the repository root.

---

## Core Principles

1. **No Misleading UI / Ghost Features**:
   - Never display buttons, shortcut badges, tooltips, or controls in the UI that do not have working backing logic or event listeners.
   - If an element is non-functional or speculative, remove it from the user interface immediately.

2. **Mandatory Documentation in `maybe_later.md`**:
   - When a feature, visual shortcut, or capability is postponed, removed, or identified as a good future improvement, log it in [`maybe_later.md`](file:///d:/Projects/Relay/maybe_later.md).
   - Each entry in `maybe_later.md` must follow the standard structure below.

3. **No Dead / Commented-Out Code**:
   - Do not leave commented-out blocks in active source files as "reminders". Use `maybe_later.md` and git history to retain the rationale and blueprint.

---

## Entry Format for `maybe_later.md`

Every deferred item added to `maybe_later.md` should include:

```markdown
### [Item Number]. [Feature / Concept Name]

- **Status**: Deferred / Postponed / Backlog
- **Area**: Surface / component affected (e.g. `native/src/App.tsx`, `native/src-tauri/src/hotkeys/mod.rs`)
- **Original Context**: Why was this originally introduced, or what was the issue that prompted deferring it?
- **Concept & Implementation Blueprint**:
  - Technical architecture and approach when revisited.
  - Edge cases to handle (e.g., focus management, platform differences, auth boundaries).
```

---

## When to Add an Item to `maybe_later.md`

- **Incomplete / Static UI Affordances**: e.g., shortcut hints with no key listeners, placeholder menu actions, or mock buttons.
- **Platform-Specific Considerations**: Features that require distinct Windows vs macOS vs Linux implementations that are deferred.
- **Architectural Enhancements**: Speculative future optimizations or workflows (e.g. advanced plugin models, complex offline sync edge cases) that should not complicate the active build.
