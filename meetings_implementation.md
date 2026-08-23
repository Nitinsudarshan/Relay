# Meetings — Removal & Rebuild Implementation Plan

## 0. Scope & standing constraints

This plan removes the meetings feature's pages and logic — everything except
Google Calendar auth and its sync functionality — and replaces it with a
rebuild informed by `docs/decisions.md` Decision 45 (the bug audit of commit
`3b2684a`) and field research across ten open-source meeting-notetaker tools.
It is scoped **exclusively to the meetings feature**. Three constraints hold
across every phase below, non-negotiably:

1. **Recording stays on the shared `AudioRecorder` / Dictation Pill.**
   `start_meeting_recording` already calls `state.recorder` — the same
   instance the pill, global hotkey, and click-to-talk use. This plan changes
   *which meeting ID* reaches that call and *what happens around it*; it
   never introduces a second capture path. `native/src/components/capture/*`
   and `overlay.rs`'s pill functions (`ensure_pill_window`, `reposition_pill`,
   `set_expanded`, `set_pill_window_geometry`) are not touched.
2. **The UI stays simple.** Every choice below picks the smaller, plainer
   control over a configurable subsystem — matching the restraint
   Decision 45 and Decisions 21/29 already apply to the pill itself.
3. **Nothing outside the meetings feature is touched.** Scribbles, Voice
   Notes, Kanban, Voice Chat, Triggers, Identity/Google Sign-In, `web/`, and
   the Dictation Pill are out of scope even where their code is adjacent to
   or shares infrastructure with meetings. §8 lists this explicitly.

This document is a plan. No source has been changed yet — implementation is
tracked as the phases in §9, done as a separate, subsequent pass.

---

## 1. Why remove instead of patch

Decision 45 found the reminder popup's primary action fails for every
reminder kind (a meeting ID that never resolves to a saved record), a second
reminder silently erases the first, the popup freezes at its first on-screen
position, and several smaller integrity gaps. Patching each in place would
leave the same fragile foundation — a meeting ID that only sometimes refers
to something real — for the next feature to trip over again. This plan
instead fixes the foundation once (a meeting is created the moment it's
detected, never later — see §4.1) and rebuilds the layer above it clean,
reusing everything that already works (calendar auth, the list-rail/
detail-pane split, the shared recorder).

---

## 2. What survives untouched

### 2.1 Outside the meetings feature entirely — not read, not modified

| Area | Why it's out of scope |
| --- | --- |
| `native/src-tauri/src/oauth/*` (`config.rs`, `flow.rs`, `pkce.rs`, `tokens.rs`) | Shared OAuth infra — `identity/oauth.rs` and `identity/tokens.rs` use the same `GoogleOAuthConfig`/`KeyringTokenStore` for Google Sign-In (`SCOPE_IDENTITY`). Calendar sync uses the same module with `SCOPE_CALENDAR_READONLY`. This is identity infrastructure that calendar happens to consume, not meetings' own code. |
| `native/src-tauri/src/identity/*` | Google Sign-In, installation ID, Supabase account — unrelated. |
| `native/src-tauri/src/capture/*` (`AudioRecorder`, `SttEngine`) | Shared recording/transcription primitive per constraint 1. |
| `native/src-tauri/src/overlay.rs` lines 1–173 (`ensure_pill_window`, `reposition_pill`, `set_expanded`, `set_pill_window_geometry`, `reposition`, `compute_anchor`, `active_monitor`) | Dictation Pill window management. Only lines 175–212 (`REMINDER_WINDOW_LABEL`, `ensure_reminder_window`) are meetings' own and in scope. |
| `native/src/components/capture/*` | Dictation Pill UI. |
| Scribbles, Voice Notes, Kanban, Voice Chat, Triggers/MCP | Untouched; `vault/scribble.rs`'s `Scribble` type is read (a kept meeting command creates one) but not modified. |
| `web/` | Already deferred (Decision 32); not part of this pass either way. |

### 2.2 Kept — Google Calendar auth and its functionality (the explicit exception)

| Item | File | Role |
| --- | --- | --- |
| `get_calendar_connection_status`, `start_google_calendar_oauth`, `disconnect_google_calendar`, `sync_google_calendar`, `get_upcoming_calendar_events` | `commands.rs` | Connect / disconnect / status / sync / list — the calendar auth surface. Unchanged. |
| Token storage, `sync_real_google_calendar_events`, `CalendarConnectionState`/`Status`, `load_calendar_config`/`save_calendar_config`, `get_config_dir` | `meetings/calendar.rs` | Keyring-backed OAuth tokens and the real Google Calendar API fetch. Unchanged. |
| `CalendarMeetingEvent` | `meetings/mod.rs` | The raw event shape calendar sync returns. Unchanged (moves file — see §5 — but the type itself doesn't change). |
| Connect / disconnect / sync / status UI | `CalendarSyncModal.tsx` | Kept as the one calendar-auth UI (see §4.5 for de-duplicating ProviderSettings' second copy of this). |

---

## 3. What gets removed

| Item | File | Why |
| --- | --- | --- |
| `ActiveReminderState`, `start_scheduler`, `dismiss_meeting_reminder`, `start_recording_from_reminder`, `get_active_meeting_reminder`, `trigger_mock_meeting_reminder` | `meetings/scheduler.rs` | The single-slot, permanently-latching reminder engine (Decision 45, Broken #1–2). |
| `REMINDER_WINDOW_LABEL`, `ensure_reminder_window` | `overlay.rs:175-212` | Never repositions after first show (Decision 45, Refactor #2). |
| `Meeting`, `MeetingSeries`, `MeetingActionItem`, the markdown format, the status state machine | `vault/meeting.rs` | Not itself buggy, but its IDs are only ever minted at *record time*, which is the root cause of Broken #1 — redesigned in §4.1 to mint at *detection time* instead. |
| `detect_active_conferencing_windows`, `identify_meeting_provider`, `clean_meeting_window_title`, `DetectedMeetingPayload`, provider constants | `meetings/mod.rs` | Not buggy either, but per the explicit "remove all meetings logic except calendar auth" scope, this is removed and reintroduced as a clean, correctly-wired module in §4.1 rather than kept as-is with a new layer bolted on. |
| `get_meetings`, `get_meeting`, `create_meeting`, `save_meeting`, `update_meeting`, `delete_meeting`, `get_meeting_series`, `save_meeting_series`, `delete_meeting_series`, `start_meeting_recording`, `stop_meeting_recording`, `trigger_enrich_meeting`, `create_scribble_from_meeting` | `commands.rs` | Rebuilt against the new lifecycle module; same responsibilities, corrected wiring. |
| `MEETING_DETECTED_EVENT`, `check_meeting_detection` | `commands.rs:1361,1693-1750` | Already dead — unregistered in `invoke_handler`, nothing listens for the event. Deleted outright, not rebuilt (Decision 45, Broken #3a). |
| Scheduler spawn, tray "Start Recording" no-op, all command registrations above | `lib.rs` | Rewired in §4. |
| `MeetingSettings` | `settings/mod.rs` | Redesigned — see §4.5 for the `auto_record` resolution. |
| `MeetingPage.tsx`, `MeetingListRail.tsx`, `MeetingDetailPane.tsx`, `MeetingDetailView.tsx`, `MeetingReminderWindow.tsx`, `useMeetingList.ts` | `native/src/components/meetings/` | Rebuilt against the corrected backend. |
| The meetings tab's event handling in `App.tsx`, the `#/meeting-reminder` route in `main.tsx` | — | Rebuilt correctly (payload no longer dropped). |

---

## 4. New architecture

### 4.1 One meeting-creation path, triggered at detection — not at record time

This is the fix for Decision 45's Broken #1 (the reminder popup's "Start
Recording" action failing for every kind). The root cause was that a
meeting's real vault ID was only ever minted when a recording actually
started, while calendar sync and window detection each independently
invented their own ID shapes (a raw calendar-event ID; a synthetic
`detected_<provider>_<title>` string) that never matched. The fix is to
never let that gap exist:

- **`meetings/detection.rs`** (new — the rebuilt window-detection signal from
  the removed `meetings/mod.rs` code, unchanged Win32 approach): exposes
  `detect_active_conferencing_windows() -> Vec<WindowMatch>`, pure, no side
  effects, same provider-identification/title-cleaning helpers as before.
- **`meetings/lifecycle.rs`** (new): a single
  `find_or_create_meeting(source: MeetingSource) -> Meeting` entry point,
  where `MeetingSource` is either `Calendar(CalendarMeetingEvent)` or
  `WindowDetected { provider, title }`. It's idempotent:
  - Calendar-sourced: looks up an existing `Meeting` by `calendar_event_id`
    first; creates one with a real `meeting_<uuid>` ID only if none exists.
    Recurring daily sync of the same event never creates a duplicate.
  - Window-detected: looks up an existing `Meeting` with a matching
    `detection_key` (a new `Option<String>` field on `Meeting`, e.g.
    `"{provider}:{title}:{date}"`) within the current day before creating
    one — repeated detection polls of the same open Zoom window don't spawn
    duplicates.
  - **Either path returns a real, already-persisted `Meeting` with a real
    ID.** Nothing downstream — the reminder engine, the popup, the list —
    ever sees a synthetic or not-yet-real ID again.
- Both the calendar sync tick and the window-detection poll call
  `find_or_create_meeting` immediately on a new match, not lazily on
  reminder or record. The meeting shows up in the list (as "Upcoming" or
  "Detected" — see §4.4) the moment it's found, which also means recording
  can be started from the list itself, not only from a popup.

### 4.2 Reminder queue and popup

Fixes Decision 45's Broken #2, Refactor #2/#3, and Improve #1–5.

- **`meetings/reminders.rs`** (new, replaces `scheduler.rs`): reminders are a
  `Vec<ReminderEvent>` keyed by `meeting_id`, each with a `kind`
  (`Upcoming` / `Unrecorded` / `Detected`), a `fire_at`, and a `state`
  (`Pending` → `Shown` → one of `Snoozed(until)` / `Dismissed` / `Actioned`
  / `Missed`). A second reminder queues instead of overwriting — closes
  Broken #2 directly. A kind is marked `Shown`/resolved only once the user
  has actually seen and acted on it, never at fire time.
- **Missed-window catch-up** (Improve #2): each poll also checks for any
  `Pending` reminder whose `fire_at` has passed outside its normal window
  (app was asleep/closed/backgrounded) and surfaces it once as `Missed`,
  visually distinct ("You missed a reminder for…") rather than a live one.
- **Recording gate is per-meeting** (Refactor #3): `is_recording_this_meeting(id)`
  replaces the old global `is_recording_meeting()` check.
- **The popup** (`overlay.rs`'s `ensure_reminder_window`, rebuilt): reuses
  `compute_anchor`/`active_monitor` from the pill's existing code instead of
  hardcoding its own top-right corner, so it's recomputed on every show
  (Refactor #2) instead of frozen at first creation. Skips `.setFocus()`
  when the foreground app is fullscreen (Improve #4) — still shown, just not
  focus-stealing.
- **Two actions only** (constraint 2 — stay simple): "Start Recording" and
  "Remind me in 5 min", replacing "Don't remind me" vs. a functionally
  identical ✕ (Improve #1). No duration picker, no do-not-disturb
  subsystem.
- **One notification surface** (Improve #3): the popup only. The parallel OS
  toast that duplicated it is dropped.
- **One shared recording entry point**, `startMeetingRecording(meetingId)`
  (frontend), called identically from the popup's button, the meetings
  list, and the tray item. It always clears any queued/shown reminder for
  that meeting as a side effect — this is what fixes both Refactor #1 (two
  disconnected start-recording paths) and Improve #5 (acknowledging a
  meeting in the list not dismissing its popup) with one function, not two
  separate patches. The tray's "Start Recording" item (currently a no-op —
  Decision 45, Broken #3b) is wired to call it with whichever meeting is
  soonest/active.
- The dead `MEETING_DETECTED_EVENT`/`check_meeting_detection` pair (Broken
  #3a) is deleted, not reintroduced. `switch-to-meetings-tab` (Broken #3c)
  now carries the meeting ID in its payload, and the handler opens that
  meeting's detail pane, not just the tab.

*Research pattern applied here*: Hyprnote's own notification design gates a
detected-meeting popup to only interrupt when the main window is hidden, and
confirms with a toast when a notification setting is turned on. Since Relay
already asks the user before ever recording (see §4.5), the popup only ever
needs to *inform*, not *gate consent* — so this plan borrows the
hidden-window-only-interrupt idea for the `Detected` kind specifically
(no calendar backing it, the noisiest kind) but doesn't need Hyprnote's
consent-confirmation toast, since consent is already the app's standing
default.

### 4.3 Recording — unchanged invariant, restated for this rebuild

`start_meeting_recording` (rebuilt, same name) takes the real meeting ID
`find_or_create_meeting` already guaranteed exists, and calls
`state.recorder.start("meeting", ...)` exactly as today — no new capture
path. This is what closes Broken #1 concretely: the command was always
correct; it just never received a valid ID before.

### 4.4 Meetings page

Keeps the list-rail/detail-pane split from `78a6136`/`3b2684a` — Decision 45
already validated this against Meetily's and Hyprnote's sidebar → workspace
pattern — rebuilt against the new lifecycle/reminder modules:

- **`MeetingListRail.tsx`** groups by state, not a flat list: **Now**
  (recording, or a reminder currently shown) / **Upcoming** (calendar-matched,
  not started) / **Detected** (window-matched, no calendar link) / **Past**
  (completed). This is deliberately closer to Meetily/Hyprnote's structured
  list than Screenpipe's undifferentiated stream — Decision 45 already flagged
  the latter as something to avoid regressing toward.
- **`MeetingDetailPane.tsx`** gets explicit **Summary** / **Transcript** tabs
  (Decision 45 flagged this as worth finishing, matching Meetily's workspace),
  plus the existing decisions/action-items surface and a "Promote to
  Scribble" action calling the kept, unmodified `Scribble` creation path.
- **`useMeetingList.ts`** rebuilt against the new commands; same
  responsibility (list state for the rail/pane), corrected data.

### 4.5 Settings — de-duplicated, and one setting removed on evidence

Two corrections found during this investigation, beyond Decision 45's own
list, both meetings-scoped:

- **Duplicate calendar-connect UI.** `ProviderSettings.tsx`'s "Meetings"
  section (`handleConnectGoogleCalendar`/`handleDisconnectGoogleCalendar`,
  around line 1196) independently reimplements the exact connect/disconnect/
  sync/status flow `CalendarSyncModal.tsx` already implements. **Fix**:
  extract a single presentational `CalendarConnectionCard` (props: status,
  handlers) used by both `CalendarSyncModal` and the settings section, so
  there is one calendar-auth UI implementation, not two that can drift apart.
- **Two fully decorative settings toggles.** "Speaker Diarization &
  Labeling" (`speakerDiarization`) and "Executive AI Minutes & Insights"
  (`meetingSummaryPrompt`) in `ProviderSettings.tsx` (lines ~1353, 1371) are
  local `useState` only — never part of the persisted `settings` object
  `handleSaveDirect` writes, never read anywhere else in the codebase.
  Diarization specifically isn't implemented at all yet
  (`docs/roadmap.md` item 3). **Fix**: remove both toggles rather than leave
  UI that does nothing — consistent with the project's own stated principle
  of not silently implying something works when it's a stub.
- **`auto_record` is removed, not wired up** — resolving the question
  Decision 45 explicitly left open, on evidence found while reading the
  current settings UI: `ProviderSettings.tsx` already ships an "Explicit
  Capture Consent (Privacy Guard)" control, hardcoded to a permanent
  **"Enforced"** badge (not a toggle) stating *"Relay will never record audio
  silently. Capture only begins when you explicitly press 'Start
  Recording'."* — and `CalendarSyncModal.tsx`'s own footer already tells
  users *"Relay reads calendar events to prepare meeting notes without
  automatic recording."* Implementing `auto_record` would contradict both
  already-shipped product commitments. It's deleted from `MeetingSettings`
  rather than implemented or left dormant.
- The remaining three settings (`remind_before_meeting`, `remind_if_unrecorded`,
  `remind_on_detection`) are all exposed in the UI (today only two of the
  four are — Decision 45, Refactor #4) alongside the new "remind me in 5 min"
  behavior, which needs no separate setting (it's an action, not a mode).

---

## 5. File map

**Backend — new**
`meetings/detection.rs`, `meetings/lifecycle.rs`, `meetings/reminders.rs`

**Backend — kept as-is**
`meetings/calendar.rs` (calendar auth/sync only), `oauth/*`, `identity/*`,
`capture/*`, `overlay.rs` lines 1–173

**Backend — rewritten in place**
`vault/meeting.rs` (adds `detection_key`; drops nothing from the existing
notes/transcript/decisions/action-items shape — that part isn't broken),
`settings/mod.rs`'s `MeetingSettings`, the meetings-related sections of
`commands.rs` and `lib.rs`

**Backend — deleted**
`meetings/scheduler.rs`, `overlay.rs` lines 175–212 (folded into the rebuilt
reminder window function elsewhere in the same file)

**Frontend — rewritten in place**
`MeetingPage.tsx`, `MeetingListRail.tsx`, `MeetingDetailPane.tsx`,
`MeetingDetailView.tsx`, `useMeetingList.ts`, `MeetingReminderWindow.tsx`,
`CalendarSyncModal.tsx` (extracts `CalendarConnectionCard`), relevant
sections of `App.tsx`, `main.tsx`, `NativeSidebar.tsx` (repointed, not
restructured), `ProviderSettings.tsx`'s Meetings section

**Frontend — new**
`CalendarConnectionCard.tsx` (shared by the modal and settings section)

---

## 6. Traceability — every Decision 45 item, and how this plan resolves it

| # | Decision 45 item | Resolved by |
| --- | --- | --- |
| Broken 1 | Record action fails for every reminder kind | §4.1 — meeting IDs are real from the moment of detection, never synthetic |
| Broken 2 | Second reminder erases the first permanently | §4.2 — queue, not a single slot; resolved-not-fired latching |
| Broken 3 | Dead event/command, tray no-op, dropped tab-switch payload | §4.2 — dead pair deleted; tray and tab-switch both call the one shared recording entry point |
| Refactor 1 | Two start-recording paths, one doesn't clear state | §4.2 — one `startMeetingRecording` function, used everywhere |
| Refactor 2 | Popup position frozen after first show | §4.2 — reuses the pill's recompute-on-every-show anchor logic |
| Refactor 3 | Global, not per-meeting, recording gate | §4.2 — `is_recording_this_meeting(id)` |
| Refactor 4 | Settings/type drift, `auto_record` unread | §4.5 — all real settings exposed; `auto_record` removed on evidence |
| Improve 1 | No real snooze | §4.2 — "remind me in 5 min" as one of two popup actions |
| Improve 2 | Missed windows drop reminders silently | §4.2 — `Missed` state surfaced once |
| Improve 3 | Duplicate OS toast + popup | §4.2 — popup is the only surface |
| Improve 4 | Popup steals focus during screen-share | §4.2 — fullscreen-aware `.setFocus()` suppression |
| Improve 5 | Acknowledging in list doesn't dismiss popup | §4.2 — side effect of the shared recording entry point |

Plus two meetings-scoped issues found during this investigation, not in
Decision 45: the duplicate calendar-connect UI and the two decorative
settings toggles (§4.5).

---

## 7. Explicitly out of scope

No file under `native/src-tauri/src/oauth/`, `identity/`, `capture/`,
`vault/scribble.rs`, `triggers/`, `mcp/`, `tts/`, `providers/`, or `web/` is
modified. `overlay.rs` lines 1–173, everything under
`native/src/components/capture/`, and the Dictation Pill's settings are not
touched. No settings field outside `MeetingSettings` changes shape.

---

## 8. Implementation sequence

1. **Backend foundation** — `vault/meeting.rs`'s `detection_key` field;
   `meetings/detection.rs`; `meetings/lifecycle.rs`'s `find_or_create_meeting`.
   Unit tests for idempotency (same calendar event synced twice; same window
   match polled twice) before anything else is built on top.
2. **Reminder engine** — `meetings/reminders.rs`, the queue, missed-window
   catch-up, per-meeting recording gate. Unit tests for queue ordering and
   the never-overwrite guarantee.
3. **Commands & wiring** — rebuild the meetings/calendar-adjacent commands in
   `commands.rs`, delete the dead pair, rewire `lib.rs` (scheduler spawn →
   reminder engine spawn, tray item, `invoke_handler` list).
4. **Popup** — rebuilt `overlay.rs` reminder window (reusing pill anchor
   logic) + rebuilt `MeetingReminderWindow.tsx` (two actions, no toast).
5. **Meetings page** — rebuilt `MeetingListRail.tsx` (state groups),
   `MeetingDetailPane.tsx` (Summary/Transcript tabs), `useMeetingList.ts`,
   `MeetingPage.tsx`.
6. **Settings** — `CalendarConnectionCard` extraction, remove the two
   decorative toggles and `auto_record`, expose all three real reminder
   settings.
7. **Wiring cleanup** — `App.tsx`'s tab/event handling, `main.tsx`'s
   `#/meeting-reminder` route, `NativeSidebar.tsx`'s nav entry repointed.

## 9. Verification checklist

- `cargo test` / `cargo check` pass; new unit tests for idempotent
  meeting creation and reminder-queue ordering.
- `tsc --noEmit` / `vite build` pass.
- Manual: a calendar-matched meeting reminder's "Start Recording" actually
  starts a recording (the concrete repro of Broken #1).
- Manual: two reminders firing within one poll tick both end up visible
  (queue, not overwrite).
- Manual: popup reappears correctly positioned after switching monitors.
- Manual: recording from the meetings list clears a live popup for that
  meeting; recording from the tray does the same.
- Manual: disconnecting/reconnecting Google Calendar still works unchanged
  (regression check on the untouched auth path).
- Manual: Dictation Pill (global hotkey + click-to-talk) unaffected.
