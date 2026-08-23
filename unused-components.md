# Unused & Dormant Components Audit (`unused-components.md`)

This document tracks orphaned, unreferenced, or dormant UI components across both surfaces (**Native Desktop App** and **Web Dashboard**). 

> [!NOTE]
> Components listed here exist in the source tree but are not imported or rendered by any active pages, layouts, or main navigation handlers. Keeping this index ensures clarity on what code can be safely refactored, integrated in future phases, or pruned.

---

## 1. Native Desktop App (`native/`)

### 1.1 `KanbanBoard`
- **File**: [`native/src/components/kanban/KanbanBoard.tsx`](file:///d:/Projects/Relay/native/src/components/kanban/KanbanBoard.tsx)
- **Description**: Full-featured Kanban Board UI component with task cards, columns (To Do, In Progress, Done), and action triggers.
- **Status**: **Unused / Dormant**.
- **Context**: The Windows native desktop app currently presents four core surfaces (`Voice Note`, `Meetings`, `Scribbles`, and `Settings`). The Kanban board view is not imported in [`App.tsx`](file:///d:/Projects/Relay/native/src/App.tsx) or attached to any navigation item.

### 1.2 `ChatPanel`
- **File**: [`native/src/components/chat/ChatPanel.tsx`](file:///d:/Projects/Relay/native/src/components/chat/ChatPanel.tsx)
- **Description**: Conversational AI chat sidebar/panel component with message history and prompt input.
- **Status**: **Unused / Dormant**.
- **Context**: Relay handles AI interactions contextually within Voice Notes, Meeting summaries, and Scribble graphs rather than through a dedicated standalone chat window.

### 1.3 `PTTWidget`
- **File**: [`native/src/components/capture/PTTWidget.tsx`](file:///d:/Projects/Relay/native/src/components/capture/PTTWidget.tsx)
- **Description**: Standalone Push-To-Talk widget component wrapper.
- **Status**: **Unused**.
- **Context**: PTT triggers and state management are directly handled inside [`DictationPill.tsx`](file:///d:/Projects/Relay/native/src/components/capture/DictationPill.tsx) (the floating widget) and [`VoiceNotePage.tsx`](file:///d:/Projects/Relay/native/src/components/voicenotes/VoiceNotePage.tsx).

### 1.4 `ScribbleComposer`
- **File**: [`native/src/components/scribble/ScribbleComposer.tsx`](file:///d:/Projects/Relay/native/src/components/scribble/ScribbleComposer.tsx)
- **Description**: Standalone modal/form component for composing new scribbles.
- **Status**: **Unused**.
- **Context**: [`ScribbleViewer.tsx`](file:///d:/Projects/Relay/native/src/components/scribble/ScribbleViewer.tsx) uses an integrated inline modal and delegator to [`ScribbleDetailEditor.tsx`](file:///d:/Projects/Relay/native/src/components/scribble/ScribbleDetailEditor.tsx), bypassing `ScribbleComposer.tsx`.

---

## 2. Web Dashboard (`web/`)

### 2.1 `search-form`
- **File**: [`web/src/components/search-form.tsx`](file:///d:/Projects/Relay/web/src/components/search-form.tsx)
- **Description**: Search input form component with icon styling.
- **Status**: **Unused**.
- **Context**: Header and sidebar layout use direct search inputs or omit top search bar.

### 2.2 `Input` (Legacy Standalone Component)
- **File**: [`web/src/components/Input.tsx`](file:///d:/Projects/Relay/web/src/components/Input.tsx)
- **Description**: Standalone custom Input text field component.
- **Status**: **Unused**.
- **Context**: All web components use shadcn UI's standardized `@/components/ui/input.tsx` component instead.

### 2.3 `nav-secondary`
- **File**: [`web/src/components/nav-secondary.tsx`](file:///d:/Projects/Relay/web/src/components/nav-secondary.tsx)
- **Description**: Secondary sidebar navigation group for supporting links (e.g., Support, Feedback).
- **Status**: **Unused**.
- **Context**: [`app-sidebar.tsx`](file:///d:/Projects/Relay/web/src/components/app-sidebar.tsx) currently renders primary navigation (`NavMain`) and quick vault items (`NavProjects`), without rendering secondary nav links.

### 2.4 `mini-loader`
- **File**: [`web/src/components/mini-loader.tsx`](file:///d:/Projects/Relay/web/src/components/mini-loader.tsx)
- **Description**: Compact inline loading spinner component.
- **Status**: **Unused**.
- **Context**: Web dashboard uses [`loading-view.tsx`](file:///d:/Projects/Relay/web/src/components/loading-view.tsx) for route loading and `skeleton.tsx` for component skeleton states.

---

## Summary Matrix

| Surface | Component | File Path | Current Status | Recommendation |
| :--- | :--- | :--- | :--- | :--- |
| **Native** | `KanbanBoard` | [`KanbanBoard.tsx`](file:///d:/Projects/Relay/native/src/components/kanban/KanbanBoard.tsx) | Dormant | Retain for future task view integration or move to `maybe_later.md` |
| **Native** | `ChatPanel` | [`ChatPanel.tsx`](file:///d:/Projects/Relay/native/src/components/chat/ChatPanel.tsx) | Dormant | Retain for future inline AI assistant sidebar |
| **Native** | `PTTWidget` | [`PTTWidget.tsx`](file:///d:/Projects/Relay/native/src/components/capture/PTTWidget.tsx) | Unused | Prune or merge into `DictationPill.tsx` |
| **Native** | `ScribbleComposer` | [`ScribbleComposer.tsx`](file:///d:/Projects/Relay/native/src/components/scribble/ScribbleComposer.tsx) | Unused | Prune in favor of `ScribbleDetailEditor.tsx` |
| **Web** | `search-form` | [`search-form.tsx`](file:///d:/Projects/Relay/web/src/components/search-form.tsx) | Unused | Prune or integrate into top header search |
| **Web** | `Input` | [`Input.tsx`](file:///d:/Projects/Relay/web/src/components/Input.tsx) | Unused | Safe to remove (redundant with `ui/input.tsx`) |
| **Web** | `nav-secondary` | [`nav-secondary.tsx`](file:///d:/Projects/Relay/web/src/components/nav-secondary.tsx) | Unused | Retain for future help/support sidebar links |
| **Web** | `mini-loader` | [`mini-loader.tsx`](file:///d:/Projects/Relay/web/src/components/mini-loader.tsx) | Unused | Safe to remove (redundant with `loading-view.tsx`) |
