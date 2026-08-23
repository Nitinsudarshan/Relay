# Maybe Later (Deferred Features & Architecture Backlog)

This document tracks deferred features, rejected/postponed UI patterns, and architecture concepts that have been set aside for future evaluation. 

> [!NOTE]
> When a feature, visual affordance, or architectural idea is identified as speculative, half-implemented, or deferred to keep the current surface clean and reliable, it **must** be documented here rather than left as ghost UI or commented-out code. See [Rules/maybe-later.md](file:///d:/Projects/Relay/Rules/maybe-later.md).

---

## Backlog Items

### 1. In-App Navigation Keyboard Shortcuts (`Alt + 1..4` / `Ctrl + 1..4`)

- **Status**: Deferred (UI indicators removed from Sidenav)
- **Area**: Native App (`native/src/components/common/NativeSidebar.tsx`, `native/src/App.tsx`)
- **Original Context**:
  - The Sidenav previously displayed `⌥1`, `⌥2`, `⌥3`, and `⌥4` next to navigation items ("Voice Note", "Meetings", "Scribbles", "Settings").
  - However, no corresponding `keydown` listeners or Tauri global shortcuts were attached, meaning pressing `Alt + Number` or `Option + Number` did nothing.
  - Furthermore, using the Mac Option symbol (`⌥`) in a Windows-first desktop app created platform inconsistency.
- **Concept & Implementation Blueprint**:
  - Implement a dedicated hook (e.g., `useNavigationShortcuts`) or a `keydown` listener in `App.tsx`.
  - Listen for `Alt + 1` (Voice Note), `Alt + 2` (Meetings), `Alt + 3` (Scribbles), `Alt + 4` (Settings).
  - Ensure shortcuts are bypassed when user focus is inside text fields, textareas, code editors, or modals (`event.target.tagName !== 'INPUT' && event.target.tagName !== 'TEXTAREA'`).
  - Provide platform-accurate visual shortcut badges (`Alt+1` on Windows / Linux, `⌥1` on macOS) or make them configurable in Settings > Hotkeys.

### 2. Post-Migration Legal & Governance Hardening
- **Status**: Deferred (Post-AGPL Migration)
- **Area**: Governance & Legal (`TRADEMARKS.md`, `CONTRIBUTING.md`, `NOTICE`)
- **Concept & Items**:
  - **Formal Trademark / Branding Policy (`TRADEMARKS.md`)**: Establish explicit branding guidelines distinguishing AGPL software licensing from official Relay trademark rights for forks and derivatives.
  - **Contributor License / CLA Policy (`CONTRIBUTING.md`)**: Define contributor guidelines and CLA / DCO expectations before accepting external pull requests.
  - **Dual-Licensing Evaluation**: Evaluate whether to maintain AGPL-only core or establish an AGPL + commercial dual-licensing structure for official cloud/enterprise offerings.
  - **Third-Party Attribution Notice (`NOTICE`)**: Consolidate upstream open-source attribution notices if the asset/crate footprint expands.

