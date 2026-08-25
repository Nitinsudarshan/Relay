# Legacy Meetings System — Archaeological Record & Removal Log

> **DOCUMENT STATUS: PERMANENT HISTORICAL ARCHIVE**  
> This document is **NOT** an implementation document or architecture specification for Meetings V2.  
> It is a permanent archaeological record of Relay's legacy Meetings implementation (v0.1.0 – v0.10.1).  
> Its purpose is to ensure that removing legacy code does not lose knowledge, architectural lessons, or bug discoveries when building Meetings V2 from the ground up.

---

## Table of Contents

1. [Executive Summary & System Overview](#1-executive-summary--system-overview)
2. [Old Architecture Snapshot](#2-old-architecture-snapshot)
3. [Deep-Dive Subsystem Analysis](#3-deep-dive-subsystem-analysis)
   - [3.1 Overall Architecture](#31-overall-architecture)
   - [3.2 Recording Flow](#32-recording-flow)
   - [3.3 Audio Capture Flow](#33-audio-capture-flow)
   - [3.4 Transcription Flow](#34-transcription-flow)
   - [3.5 Meeting Detection Flow](#35-meeting-detection-flow)
   - [3.6 Meeting Lifecycle & State Management](#36-meeting-lifecycle--state-management)
   - [3.7 Meeting UI & Interaction Models](#37-meeting-ui--interaction-models)
   - [3.8 Meeting Persistence & Storage Engine](#38-meeting-persistence--storage-engine)
   - [3.9 Meeting Notifications & Overlay Subsystem](#39-meeting-notifications--overlay-subsystem)
   - [3.10 IPC & Tauri Communication Matrix](#310-ipc--tauri-communication-matrix)
   - [3.11 External Services & APIs](#311-external-services--apis)
   - [3.12 Dependencies & Build Configuration](#312-dependencies--build-configuration)
4. [Recording Pipeline Snapshot](#4-recording-pipeline-snapshot)
5. [Notification Subsystem Snapshot](#5-notification-subsystem-snapshot)
6. [UI Snapshot & Usability Analysis](#6-ui-snapshot--usability-analysis)
7. [Data & Storage Snapshot](#7-data--storage-snapshot)
8. [Dependency Snapshot](#8-dependency-snapshot)
9. [File-by-File Removal Inventory](#9-file-by-file-removal-inventory)
10. [Known Problems in Legacy Meetings](#10-known-problems-in-legacy-meetings)
11. [Lessons for Meetings V2](#11-lessons-for-meetings-v2)
12. [Removed vs Retained Matrix](#12-removed-vs-retained-matrix)
13. [Git & History Reference](#13-git--history-reference)
14. [Cleanup Completed](#14-cleanup-completed)

---

## 1. Executive Summary & System Overview

Relay's legacy Meetings system was developed to provide automatic detection, calendar synchronization, audio capture, transcription, AI enrichment, and knowledge extraction (Scribble creation) for video conferences and in-person discussions.

The implementation spanned:
- **Rust Backend (`native/src-tauri`)**: Win32 window enumeration, Google Calendar OAuth/sync, signal reconciliation, queue-based reminder state machines, Tauri WebView overlay lifecycle management, and markdown frontmatter file persistence.
- **Frontend React UI (`native/src`)**: A split master-detail rail (`MeetingListRail` + `MeetingDetailPane` / `MeetingDetailView`), calendar synchronization modal (`CalendarSyncModal`), manual creation modal (`MeetingModal`), floating meeting reminder overlay window (`MeetingReminderWindow`), and an 8-variant notification preview gallery (`MeetingNotificationGallery`).

While the system established valuable concepts—such as separating detection from identity, reconciling multi-source signals, and promoting meeting insights into permanent knowledge graphs—the physical implementation suffered from structural coupling, non-incremental audio recording, fragile window lifecycle handshakes, and UI overcomplication.

---

## 2. Old Architecture Snapshot

The legacy architecture operated as a multi-stage pipeline converging on a reconciliation layer before driving reminders and recording:

```
┌─────────────────────────┐         ┌───────────────────────────────┐
│   Google Calendar API   │         │  Windows Top-Level EnumWindows│
│  (OAuth 2.0 PKCE Loop)  │         │   (Conferencing Window Scan)  │
└────────────┬────────────┘         └───────────────┬───────────────┘
             │ (Every 15s / Manual Sync)            │ (Every 15s Polling Loop)
             ▼                                      ▼
      Calendar Signal                        Window Match Signal
(ID, Title, Time, Series)              (Provider, Raw Title, Confidence)
             │                                      │
             └───────────────────┬──────────────────┘
                                 │
                                 ▼
                   ┌───────────────────────────┐
                   │   meetings::resolver.rs   │
                   │  (Signal Reconciliation)  │
                   └─────────────┬─────────────┘
                                 │
          ┌──────────────────────┴──────────────────────┐
          ▼                                             ▼
  [High Confidence / Cal]                       [Low Confidence / Generic]
Persisted to Vault Markdown                     Candidate Store (In-Memory)
  (.relay/vault/meetings/)                      Graduates upon >=2 hits
          │
          ▼
┌─────────────────────────────┐
│    meetings::reminders.rs   │
│   (State Machine & Queue)   │
│  Pending/Fired/Snooze/Exp   │
└──────────────┬──────────────┘
               │
               ▼
┌─────────────────────────────┐
│ notification_service.rs     │
│  - 15s Auto-Dismiss Timer   │
│  - Hover Pause / Resume     │
│  - Ready Handshake (3s TO)  │
└──────────────┬──────────────┘
               │
       ┌───────┴────────────────────────┐
       ▼                                ▼
┌────────────────────────────┐    ┌──────────────────────────┐
│ Overlay WebView Window     │    │ Native OS Toast Fallback │
│ (meeting-reminder route)   │    │ (Display-only signal)    │
└──────────────┬─────────────┘    └──────────────────────────┘
               │ User clicks "Record"
               ▼
┌────────────────────────────────────────────────────────────┐
│ commands::start_meeting_recording                          │
│  - Marks reminder `Actioned`                               │
│  - Dismisses overlay                                       │
│  - Switches main UI tab to `meetings`                      │
│  - Invokes shared capture::AudioRecorder (Mode: "meeting") │
└──────────────┬─────────────────────────────────────────────┘
               │ Captures microphone audio (in-memory buffer)
               ▼
┌────────────────────────────────────────────────────────────┐
│ commands::stop_meeting_recording                           │
│  - Writes final WAV to .relay/config/audio/                │
│  - Invokes capture::SttEngine (Whisper offline model)      │
│  - Spawns pipeline::enrich_meeting (LLM extraction)        │
│  - Updates .relay/vault/meetings/{id}.md                   │
└──────────────┬─────────────────────────────────────────────┘
               │
               ▼
┌────────────────────────────────────────────────────────────┐
│ MeetingDetailView & Knowledge Graph                        │
│  - Displays summary, decisions, actions, transcript        │
│  - create_scribble_from_meeting -> .relay/vault/scribbles/ │
└────────────────────────────────────────────────────────────┘
```

---

## 3. Deep-Dive Subsystem Analysis

### 3.1 Overall Architecture
The system was composed of:
1. **Background Engine (`meetings/engine.rs`)**: A `tokio::time::interval` loop ticking every 15 seconds, polling Google Calendar and running native window detection.
2. **Reconciliation Layer (`meetings/resolver.rs`)**: Evaluated incoming signals to prevent duplicate meetings, checking exact calendar event IDs, recurrence series IDs, URL matches, and temporal/title proximity.
3. **Queue-Based Reminder Manager (`meetings/reminders.rs`)**: Tracked reminder states per `(meeting_id, kind)` tuple without destructive overwrites.
4. **Notification Dispatcher (`meetings/notification_service.rs`)**: Managed the display protocol for the floating reminder overlay, auto-dismiss countdowns, hover-pausing, and native toast fallbacks.
5. **Shared Audio Capture (`capture/mod.rs`)**: Reused the core voice-note `AudioRecorder` instance.
6. **File-Based Vault Storage (`vault/meeting.rs`, `vault/mod.rs`)**: Serialized meetings into Markdown files with JSON/YAML frontmatter.
7. **React Desktop Frontends (`native/src`)**: Multi-pane layout in `App.tsx` and standalone overlay in `main.tsx`.

### 3.2 Recording Flow
- **Initiation**: Triggered from `MeetingDetailPane`, `MeetingReminderWindow`, or system tray menu. All converged on `commands::start_meeting_recording`.
- **Pre-Conditions**: Verified that `state.recorder.is_active()` was `false` and that the target meeting existed in the local vault.
- **State Transition**: Set meeting status to `"recording"`, stamped `actual_start`, set `ActiveMeetingRecording` to `Some(meeting_id)`, marked all queue reminders for the meeting as `Actioned`, dismissed the reminder window, and emitted `switch-to-meetings-tab`.
- **Termination**: Triggered via `commands::stop_meeting_recording`. Stopped `AudioRecorder`, obtained raw audio samples, stamped `actual_end`, and set status to `"processing"`.

### 3.3 Audio Capture Flow
- **Capture Source**: Monophonic microphone input captured via `cpal` (default input device).
- **Buffering Strategy**: Audio samples (`f32`) were accumulated entirely in a heap-allocated memory vector (`Vec<f32>`).
- **Disk Persistence**: Audio was **never persisted incrementally to disk** during recording. The WAV file was written only once upon successful stop.
- **Audio Routing**: Captured **microphone input only**. There was no WASAPI loopback capture or system audio routing to capture remote meeting attendees.

### 3.4 Transcription Flow
- **Execution**: Triggered inside `stop_meeting_recording` via `tokio::task::spawn_blocking`.
- **Engine**: Local Whisper engine (`candle` / `sherpa-onnx`) running on CPU/GPU.
- **Payload**: Full `Vec<f32>` sample array passed in a single batch.
- **Segmentation**: No streaming or intermediate chunked transcription.
- **Failure Behavior**: If Whisper failed or encountered out-of-memory errors, the transcript was set to an empty string, status transitioned to `"completed"`, and an error was logged to the console.

### 3.5 Meeting Detection Flow
- **Window Enumeration**: On Windows, invoked Win32 `EnumWindows` and `GetWindowTextW` every 15 seconds.
- **Pattern Matching**: Matched substrings for Zoom (`"zoom meeting"`, `"meeting id"`), Google Meet (`"meet - "`, `" - google meet"`), Microsoft Teams (`"microsoft teams meeting"`), and Webex (`"webex meeting"`).
- **Confidence Scoring**: Generic fallback titles scored `0.55`; specific topic titles scored `0.85`.
- **Candidate Lifecycle**: Generic titles required $\ge 2$ sustained polling hits before graduating to a persisted `Meeting` record.

### 3.6 Meeting Lifecycle
A meeting progressed through the following states:
```
[ scheduled ] ──► [ recording ] ──► [ processing ] ──► [ completed ]
       │                                                     ▲
       ▼                                                     │
 [ detected ] ───────────────────────────────────────────────┘
```
- `scheduled`: Imported from Google Calendar or manually planned.
- `detected`: Discovered via active window detection heuristics.
- `recording`: Microphone capture actively recording.
- `processing`: Audio stopped; STT transcription running.
- `completed`: Notes, transcript, and AI enrichment saved.
- `cancelled`: Meeting dismissed or cancelled.

### 3.7 Meeting State Management
- **Backend Rust State**: `AppState` held `VaultManager`, `AudioRecorder`, `SttEngine`, and mutex-guarded `AppSettings`. Additional state managed by Tauri: `ReminderQueue`, `ActiveMeetingRecording`, `CandidateStore`, and `NotificationService`.
- **Frontend React State**: `useMeetingList` hook maintained reactive groupings (`Today`, `Yesterday`, `This week`, months) and listened to `meeting-updated` Tauri events.

### 3.8 Meeting UI
- **Master-Detail Layout**: `MeetingListRail` on the left (search, filter, calendar status, grouping) and `MeetingDetailPane` on the right.
- **Detail View Tabs**: Notes (with Markdown editor), Decisions & Actions (interactive checkbox toggles), Questions (discussion points), Transcript (full text), and Linked Scribbles.
- **Modals**: `CalendarSyncModal` (Google OAuth and event import) and `MeetingModal` (manual creation).

### 3.9 Meeting Persistence
- **Storage Location**: `.relay/vault/meetings/{id}.md`.
- **Format**: YAML/JSON frontmatter block enclosed by `---` dividers, followed by `# Notes` and `# Transcript` markdown sections.
- **Series Storage**: `.relay/vault/meeting_series/{id}.md`.
- **Trash Handling**: Deletion moved notes to `.relay/vault/trash/meeting_{id}.md` with 30-day recovery metadata.

### 3.10 Meeting Notifications
- **Dual-Surface Pipeline**: Dispatched both an in-app overlay window (`meeting-reminder`) and a native OS toast.
- **Auto-Dismiss & Hover**: 15-second countdown timer. Hovering over the overlay paused the timer; unhovering clamped the remaining time to a 5-second minimum resume floor.
- **Handshake Protocol**: Window remained hidden until the React frontend emitted `meeting_reminder_ready` (with a 3000ms safety timeout).

### 3.11 IPC & Tauri Communication
- Exposed 18 dedicated Tauri commands under `commands.rs`.
- Emitted events: `meeting-updated`, `meeting-reminder`, `switch-to-meetings-tab`, `start-meeting-recording-for`.

### 3.12 External Services & APIs
- **Google Calendar API v3**: `https://www.googleapis.com/calendar/v3/calendars/primary/events`.
- **OAuth 2.0**: Keyring-backed token store with automatic refresh token rotation.
- **Local / Remote LLMs**: Ollama, OpenAI, Anthropic, Gemini for meeting enrichment (`enrich_meeting`).

### 3.13 Dependencies
- `tauri` (v2), `tauri-plugin-notification`, `cpal`, `hound`, `tokio`, `chrono`, `serde`, `serde_json`, `reqwest`, `uuid`, `regex`.

### 3.14 Configuration
- Stored in `.relay/config/settings.json` under the `meetings` object:
  ```json
  "meetings": {
    "remind_before_meeting": true,
    "remind_if_unrecorded": true,
    "remind_on_detection": true,
    "auto_record": false
  }
  ```

### 3.15 Known Limitations & Failure Modes
- Microphone-only recording (no system audio loopback).
- Single in-memory buffer (crash caused total audio loss).
- Monolithic batch transcription (long meetings failed or stalled).
- No incremental persistence.

---

## 4. Recording Pipeline Snapshot

| Stage | Legacy Implementation Detail |
|---|---|
| **Trigger** | User clicked "Record" on Meeting Detail, Reminder Overlay, or Tray icon. |
| **Capture Surface** | Reused shared `AudioRecorder` in `"meeting"` mode. |
| **Audio Buffering** | Buffered in-memory as `Vec<f32>` samples via `cpal`. |
| **Continuous Disk Persistence** | **NONE**. No incremental chunks, temporary files, or SQLite blocks. |
| **Segmentation** | **NONE**. Recorded as one continuous buffer regardless of duration. |
| **Audio Hand-off** | `stop()` returned full sample buffer and wrote single WAV file to `.relay/config/audio/`. |
| **Transcription Timing** | Synchronously/blocking after recording stopped. |
| **Transcript Storage** | Stored in Markdown body under `# Transcript` in `.relay/vault/meetings/{id}.md`. |
| **Finalization** | Async background task ran `enrich_meeting` through LLM. |
| **Audio Capture Failure** | Stopped recording, left empty audio path, set status to `completed`. |
| **Transcription Failure** | Saved meeting with empty transcript and unparsed notes. |
| **Crash Recovery** | **NONE**. If the app crashed during a 60-minute meeting, all audio in RAM was lost. |

---

## 5. Notification Subsystem Snapshot

### 5.1 Architecture & Components
- **Trigger**: `meetings/engine.rs` evaluated `reminders::recompute_reminders()` on every 15s tick.
- **Notification Manager**: `NotificationService` struct managing active reminder mutexes, auto-dismiss tasks, and hover state.
- **Overlay Window**: Tauri WebView window labeled `"meeting-reminder"`, loading `index.html#/meeting-reminder`.
- **Native OS Toast**: Display-only fallback signal triggered via `tauri-plugin-notification`.

### 5.2 Notification Lifecycle Matrix
1. **Upcoming Meeting**: Fired $\le 120$ seconds prior to `scheduled_start`.
2. **Unrecorded Meeting**: Fired between 300 and 420 seconds after `scheduled_start` if meeting was not recording and an active conferencing window was detected.
3. **Detected Meeting**: Fired when active window detection confirmed a conferencing session.
4. **Actions Available**:
   - `Record`: Started recording immediately, dismissed overlay, and switched main window to Meetings tab.
   - `Snooze 5m`: Scheduled a snooze timestamp; suppressed alerts until expired.
   - `Dismiss`: Marked reminder `Dismissed` for that specific kind.

---

## 6. UI Snapshot & Usability Analysis

### 6.1 Route & Layout Hierarchy
- **Main Route (`/`)**: Embedded inside `App.tsx` under tab `'meetings'`.
- **Overlay Route (`/#/meeting-reminder`)**: Rendered `MeetingReminderWindow.tsx` inside isolated borderless WebView.
- **Preview Route (`/` with tab `'components-meeting-notifications'`)**: Rendered `MeetingNotificationGallery.tsx`.

### 6.2 Observed Usability Issues
1. **Visual Density**: The detail pane attempted to render notes, executive summary, Mermaid diagrams, decisions, action items, open questions, transcripts, and linked Scribbles simultaneously.
2. **Ambiguous Recording State**: When recording from the floating pill or tray, the meeting detail view did not always reflect active microphone capture immediately.
3. **Unresponsive Stop on Long Recordings**: Stopping a long meeting blocked the UI while Whisper processed the multi-gigabyte audio buffer on CPU.
4. **Window Flashing**: The dedicated reminder WebView window occasionally exhibited container redraw artifacts or flash-of-white background during show transitions.

---

## 7. Data & Storage Snapshot

### 7.1 Data Structures

#### `Meeting` (Rust: `vault/meeting.rs`, TS: `types/index.ts`)
- `id: String` (format: `meeting_<uuid>`)
- `series_id: Option<String>`
- `title: String`
- `provider: String` (`google_meet`, `zoom`, `teams`, `webex`, `in_person`, `other`)
- `provider_metadata: serde_json::Value` (stores `meeting_url`)
- `calendar_event_id: Option<String>`
- `calendar_series_id: Option<String>`
- `detection_source: Option<String>` (`calendar_sync`, `window_detector`)
- `detection_key: Option<String>`
- `detection_confidence: Option<f32>`
- `detected_at: Option<String>`
- `scheduled_start: Option<String>`
- `scheduled_end: Option<String>`
- `actual_start: Option<String>`
- `actual_end: Option<String>`
- `status: String` (`scheduled`, `detected`, `recording`, `processing`, `completed`, `cancelled`)
- `participants: Vec<String>`
- `recording_path: Option<String>`
- `transcript: String`
- `notes: String`
- `summary: Option<String>`
- `decisions: Vec<String>`
- `action_items: Vec<MeetingActionItem>`
- `questions: Vec<String>`
- `candidate_scribbles: Vec<String>`
- `created_at: String`
- `updated_at: String`

#### `MeetingActionItem`
- `id: String`
- `title: String`
- `assignee: Option<String>`
- `due_date: Option<String>`
- `priority: String` (`high`, `medium`, `low`)
- `status: String` (`todo`, `done`)

#### `MeetingSeries`
- `id: String` (format: `series_<uuid>`)
- `title: String`
- `provider: Option<String>`
- `calendar_series_id: Option<String>`
- `recurrence_rule: Option<String>`
- `created_at: String`
- `updated_at: String`

---

## 8. Dependency Snapshot

| Dependency | Scope / Role | Retained or Removed | Rationale |
|---|---|---|---|
| `tauri` (v2) | Core desktop framework | **RETAINED** | Shared platform foundation. |
| `tauri-plugin-notification` | OS native toasts | **RETAINED** | Used across app for system notifications. |
| `cpal` | Audio stream capture | **RETAINED** | Shared microphone capture for Voice Notes / Dictation. |
| `hound` | WAV encoding | **RETAINED** | Shared audio file serialization. |
| `whisper-rs` / `candle` | Local Speech-to-Text | **RETAINED** | Shared transcription engine. |
| `reqwest` | HTTP client for Google Calendar | **RETAINED** | Shared OAuth & Google Calendar sync. |
| `chrono` | Date & time arithmetic | **RETAINED** | Core utility across backend. |

---

## 9. File-by-File Removal Inventory

### Rust Backend Files (`native/src-tauri/src/`)

#### `native/src-tauri/src/meetings/mod.rs`
- **File Type**: Rust Module Declaration & Data Types.
- **Purpose**: Root module for legacy meetings subsystem. Exported constants, `CalendarMeetingEvent`, and re-exports.
- **Dependencies**: `serde`, submodules (`calendar`, `detection`, `engine`, `notification_service`, `reminders`, `resolver`).
- **Meetings Specific**: Yes (100%).
- **Important Logic**: Provider constants (`PROVIDER_GOOGLE_MEET`, `PROVIDER_ZOOM`, etc.) and `CalendarMeetingEvent` struct definition.
- **Useful for V2**: The clean `CalendarMeetingEvent` definition and provider taxonomy are reusable reference models.
- **Reason for Removal**: Part of legacy meetings module being decommissioned for V2 rebuild.

#### `native/src-tauri/src/meetings/detection.rs`
- **File Type**: Rust Native Window Enumeration.
- **Purpose**: Win32 `EnumWindows` scanning for conferencing apps (Zoom, Meet, Teams, Webex).
- **Dependencies**: Win32 `user32` API (`EnumWindows`, `GetWindowTextW`, `IsWindowVisible`).
- **Meetings Specific**: Yes (100%).
- **Important Logic**: `clean_meeting_window_title` browser suffix stripping, `score_confidence` (0.55 vs 0.85), `identify_meeting_provider`.
- **Useful for V2**: The title cleaning rules and Win32 `EnumWindows` procedure are valuable references.
- **Reason for Removal**: Replaced by clean Meetings V2 window detection pipeline.

#### `native/src-tauri/src/meetings/engine.rs`
- **File Type**: Rust Background Loop & Orchestrator.
- **Purpose**: 15-second background tick polling calendar, window detection, and driving `NotificationService`.
- **Dependencies**: `AppState`, `ReminderQueue`, `CandidateStore`, `NotificationService`, `tokio`.
- **Meetings Specific**: Yes (100%).
- **Important Logic**: Signal polling coordination, auto-triggering resolver and reminder reconciliation.
- **Useful for V2**: Reference for background tick cadence, but orchestration must be rewritten.
- **Reason for Removal**: Monolithic architecture replaced by modular V2 event pipeline.

#### `native/src-tauri/src/meetings/notification_service.rs`
- **File Type**: Rust Notification Lifecycle & Window Controller.
- **Purpose**: Managed display state, hover-pause timers, ready handshakes, and native toast fallbacks for reminders.
- **Dependencies**: `overlay.rs`, `tauri::AppHandle`, `tauri_plugin_notification`.
- **Meetings Specific**: Yes (100%).
- **Important Logic**: `sanitize_and_clamp_text` (bidi-override stripping), 15s auto-dismiss loop, hover minimum resume floor (5000ms).
- **Useful for V2**: Text sanitization algorithms and hover-pause timer math are strong reference patterns.
- **Reason for Removal**: Dedicated webview overlay approach being replaced by streamlined notification architecture.

#### `native/src-tauri/src/meetings/reminders.rs`
- **File Type**: Rust Reminder Queue & State Machine.
- **Purpose**: Managed `ReminderEvent` queue (`Pending -> Fired -> Snoozed / Dismissed / Actioned / Expired`).
- **Dependencies**: `chrono`, `VaultManager`, `MeetingSettings`.
- **Meetings Specific**: Yes (100%).
- **Important Logic**: `recompute_reminders`, `upcoming_fire_time`, `unrecorded_fire_time` active-window gating, `mark_meeting_actioned`.
- **Useful for V2**: Multi-slot queue state machine is a valid model, but should not own recording lifecycle.
- **Reason for Removal**: Legacy state machine replaced by V2 lifecycle coordinator.

#### `native/src-tauri/src/meetings/resolver.rs`
- **File Type**: Rust Signal Reconciliation Layer.
- **Purpose**: Converged calendar and window detection signals onto single persisted `Meeting` records.
- **Dependencies**: `VaultManager`, `CandidateStore`, `chrono`.
- **Meetings Specific**: Yes (100%).
- **Important Logic**: 3-tier matching hierarchy (calendar event ID, series ID, temporal proximity), candidate graduation threshold.
- **Useful for V2**: The signal reconciliation rules prevent duplicate meeting creation and are highly valuable.
- **Reason for Removal**: Legacy implementation being replaced by V2 resolver.

#### `native/src-tauri/src/vault/meeting.rs`
- **File Type**: Rust Storage & Markdown Serialization.
- **Purpose**: Frontmatter parsing and Markdown serialization for `Meeting` and `MeetingSeries`.
- **Dependencies**: `serde`, `chrono`, `uuid`, `Scribble`.
- **Meetings Specific**: Yes (100%).
- **Important Logic**: `format_markdown`, `parse_markdown`, `create_scribble` provenance tracking.
- **Useful for V2**: Scribble creation provenance metadata pattern (`source_type: "meeting"`, `source_metadata: { meeting_id, ... }`).
- **Reason for Removal**: Storage format being upgraded for incremental recording persistence.

#### `native/src-tauri/capabilities/meeting-reminder.json`
- **File Type**: Tauri Security Capability JSON.
- **Purpose**: Scoped IPC permissions for `meeting-reminder` window (`core:default`, `core:event:default`).
- **Dependencies**: Tauri capability schema.
- **Meetings Specific**: Yes.
- **Reason for Removal**: Dedicated reminder window removed in favor of clean architecture.

---

### React Frontend Files (`native/src/components/meetings/`)

#### `native/src/components/meetings/MeetingPage.tsx`
- **File Type**: React Page Container.
- **Purpose**: Main Meetings page holding state, modals, rail, and detail pane.
- **Dependencies**: `useMeetingList`, `MeetingListRail`, `MeetingDetailPane`, `MeetingModal`, `CalendarSyncModal`.
- **Meetings Specific**: Yes.
- **Reason for Removal**: Replaced by clean Meetings V2 UI.

#### `native/src/components/meetings/MeetingDetailPane.tsx`
- **File Type**: React Container / Switcher.
- **Purpose**: Switched between meeting detail view, calendar event import card, or empty state.
- **Dependencies**: `MeetingDetailView`, `useCaptureOwnership`.
- **Meetings Specific**: Yes.
- **Reason for Removal**: Replaced by Meetings V2 detail container.

#### `native/src/components/meetings/MeetingDetailView.tsx`
- **File Type**: React View Component (864 lines).
- **Purpose**: Large multi-tab editor for notes, transcripts, decisions, actions, questions, and scribbles.
- **Dependencies**: `MarkdownView`, `ConfirmationModal`, Lucide icons, Shadcn UI.
- **Meetings Specific**: Yes.
- **Reason for Removal**: UI overcomplication; replaced by streamlined Meetings V2 workspace.

#### `native/src/components/meetings/MeetingListRail.tsx`
- **File Type**: React Sidebar / List Rail.
- **Purpose**: Displayed searchable, grouped list of meetings and upcoming calendar events.
- **Dependencies**: Shadcn UI, Lucide icons.
- **Meetings Specific**: Yes.
- **Reason for Removal**: Replaced by Meetings V2 list rail.

#### `native/src/components/meetings/MeetingModal.tsx`
- **File Type**: React Dialog Modal.
- **Purpose**: Form to manually create ad-hoc meetings and recurring meeting series.
- **Dependencies**: Shadcn Dialog, Input, Select.
- **Meetings Specific**: Yes.
- **Reason for Removal**: Replaced by V2 meeting creation flow.

#### `native/src/components/meetings/MeetingReminderWindow.tsx`
- **File Type**: React Floating Overlay Component.
- **Purpose**: Rendered the popup card for meeting reminders inside the isolated Tauri window.
- **Dependencies**: `@tauri-apps/api`, Lucide icons, Shadcn UI.
- **Meetings Specific**: Yes.
- **Reason for Removal**: Dedicated reminder window removed.

#### `native/src/components/meetings/useMeetingList.ts`
- **File Type**: React Custom Hook.
- **Purpose**: Fetched meetings and calendar events, performed deduplication, grouped items by date.
- **Dependencies**: `@tauri-apps/api/core`, `@tauri-apps/api/event`.
- **Meetings Specific**: Yes.
- **Reason for Removal**: Replaced by V2 meeting state store.

#### `native/src/components/meetings/notifications/` (All 7 Files)
- **Files**: `MeetingNotificationGallery.tsx`, `MeetingNotificationControls.tsx`, `MeetingNotificationPreview.tsx`, `SimulatedDesktopToastOverlay.tsx`, `notification-types.ts`, `variants/NativeInspired.tsx`, `variants/SnoozeMenu.tsx`.
- **File Type**: React Notification Variant Gallery & Preview Components.
- **Purpose**: Prototyping and testing surface for 8 notification designs.
- **Dependencies**: Lucide icons, Tailwind CSS, Shadcn UI.
- **Meetings Specific**: Yes.
- **Reason for Removal**: Prototyping gallery no longer needed in production codebase.

---

## 10. Known Problems in Legacy Meetings

1. **No Incremental Audio Persistence**: Audio was buffered purely in RAM. Any crash, battery depletion, or OS kill during a long meeting resulted in complete data loss.
2. **Microphone-Only Capture**: Failed to capture remote participant audio from Zoom/Meet/Teams due to lack of WASAPI system loopback audio capture.
3. **Monolithic STT Processing**: Stopping a 60-minute meeting forced a massive single-batch Whisper transcription job, locking CPU/GPU threads and causing UI freezes.
4. **Fragile Window Handshake**: The secondary WebView reminder window suffered from race conditions during mount, occasionally failing to show or requiring a fallback timeout.
5. **No Intermediate Crash Recovery**: If transcription failed after audio stop, the meeting note was finalized with an empty transcript with no retry mechanism.
6. **UI State Divergence**: The recording status in the UI could desynchronize from the backend audio capture engine if recording was triggered outside the meeting detail pane.
7. **Overloaded Detail View**: The 864-line `MeetingDetailView` suffered from excessive cognitive load and cluttered tabs.

---

## 11. Lessons for Meetings V2

1. **Audio is the Primary Source of Truth**: Raw audio must be written to disk continuously (in 10-to-30-second chunk files or continuous append stream). Transcripts and notes are derived artifacts.
2. **Crash Recovery from Day One**: If the app crashes, restart must detect orphaned audio segments, recover the session, and resume or finalize transcription automatically.
3. **Dual-Channel Audio Capture**: Must capture both the user's microphone and system output (WASAPI loopback) to record all meeting participants.
4. **Incremental Streaming Transcription**: Audio must be transcribed progressively or segmented into manageable chunks so transcription is nearly complete when the meeting ends.
5. **Single Authoritative Lifecycle Owner**: Meeting state must be owned strictly by the Rust backend state machine. Frontend views must simply render state snapshots.
6. **Decoupled Notifications**: Reminders and notifications must not manage window creation or recording lifecycles directly; they should emit intents to the core coordinator.
7. **Minimal, Focused UI**: Keep the initial workspace simple and distraction-free, focusing on live capture status, progressive transcript preview, and key takeaways.

---

## 12. Removed vs Retained Matrix

| Component / Subsystem | Status | Reason & Future Role |
|---|---|---|
| **Legacy Meeting UI (`MeetingPage`, `DetailView`, `Rail`)** | **REMOVED** | Rebuilding clean, responsive Meetings V2 interface. |
| **Legacy Reminder Overlay (`MeetingReminderWindow`)** | **REMOVED** | Dedicated secondary window replaced by native notification architecture. |
| **Notification Prototyping Gallery (`notifications/*`)** | **REMOVED** | Prototype gallery served its purpose; removed from active tree. |
| **Legacy Window Detection (`meetings/detection.rs`)** | **REMOVED** | Rebuilding as part of clean V2 signal pipeline. |
| **Legacy Meeting Resolver (`meetings/resolver.rs`)** | **REMOVED** | Signal reconciliation logic preserved as reference for V2. |
| **Legacy Vault Markdown Formatter (`vault/meeting.rs`)** | **REMOVED** | Replaced by V2 schema supporting chunked audio storage. |
| **Google Calendar OAuth (`oauth/*`)** | **RETAINED** | Shared infrastructure used by Google Sign-In and Calendar. |
| **Google Calendar Fetcher (`meetings/calendar.rs`)** | **RETAINED** | Keyring token storage and Google Calendar API sync retained. |
| **Shared Audio Recorder (`capture/mod.rs`)** | **RETAINED** | Shared microphone engine used by Voice Notes & Dictation. |
| **Shared STT Engine (`capture/stt/*`)** | **RETAINED** | Shared local Whisper inference engine. |
| **Dictation Pill Overlay (`overlay.rs:1-160`)** | **RETAINED** | Shared global Push-to-Talk and voice capture overlay. |
| **Scribbles Knowledge Layer (`vault/scribble.rs`)** | **RETAINED** | Obsidian-compatible knowledge graph and atomic notes layer. |

---

## 13. Git & History Reference

The following Git commits contain the complete history and evolution of the legacy Meetings implementation for future archaeological reference:

- `0bd789e` — *feat(meetings): app-owned overlay meeting reminder window & native toast demotion (v0.10.0)* (2026-08-25)
- `b26ac84` — *feat(meetings): streamline to native OS notifications and remove custom reminder window* (2026-08-25)
- `95cb1be` — *fix(meetings): display Variant 08 Native Inspired overlay directly on OS desktop floating over Chrome/Google Meet with shadow(false) zero-container-box fix* (2026-08-25)
- `81f5ac1` — *fix(meetings): eliminate duplicate notifications and white container box by enforcing clean Native OS Toast architecture* (2026-08-25)
- `bcb55dc` — *feat(meetings): wire Variant 08 Native Inspired card to OS Desktop floating overlay window* (2026-08-25)
- `4842152` — *feat(meetings): wire Variant 08 Native Inspired notification to live Rust backend IPC & reminder queue* (2026-08-25)
- `fa5d70a` — *feat(meetings): select Variant 08 (Native Inspired) as official notification spec and remove scrapped variants* (2026-08-25)
- `f1b173f` — *feat(meetings): add interactive floating desktop toast simulation overlay for all notification variants* (2026-08-25)
- `36f4285` — *feat(meetings): 8-variant custom meeting notification component gallery* (2026-08-24)
- `d0f53c9` — *refactor(meetings): replace Tauri webview meeting-reminder window with Native OS Toast Notifications (Decision 46)* (2026-08-24)
- `3b2684a` — *feat(meetings): modernize sidenav and update meetings experience (Decision 45 base audit)* (2026-08-23)

---

## 14. Cleanup Completed

| Metric / Category | Count / Status |
|---|---|
| **Files inventoried for removal** | 18 files across Rust and React |
| **React UI components identified** | 14 components (`MeetingPage`, `DetailPane`, `DetailView`, `Rail`, `Modal`, `ReminderWindow`, 8 gallery items) |
| **UI Routes identified** | 2 (`#/meeting-reminder`, `components-meeting-notifications`) |
| **Rust backend modules identified** | 6 files (`detection.rs`, `engine.rs`, `notification_service.rs`, `reminders.rs`, `resolver.rs`, `vault/meeting.rs`) |
| **IPC command handlers cataloged** | 18 handlers in `commands.rs` |
| **Database / storage structures** | 3 structures (`Meeting`, `MeetingActionItem`, `MeetingSeries`) |
| **Dependencies removed** | 0 (all shared dependencies retained for Voice Notes / Core) |
| **Capabilities cataloged** | 1 (`meeting-reminder.json`) |
| **Shared infrastructure retained** | `oauth/*`, `meetings/calendar.rs`, `capture/*`, `overlay.rs` (pill), `vault/scribble.rs` |
| **Build status** | Stable & clean |
| **Archive status** | Complete archaeological record created at `docs/meetings/MEETINGS_LEGACY_REMOVAL.md` |
