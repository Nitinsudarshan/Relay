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

## 0.1 Revision — architecture review incorporated

The first draft of this plan treated meeting identity, detection, and
notifications more simply than the domain actually allows. A review before
implementation started caught six real issues; all six are adopted in §4.1
and §4.2 below:

1. `detection_key` is a **deduplication fingerprint, never identity** —
   `provider:title:date` can collide two unrelated real meetings (a generic
   window title like "Zoom Meeting", or two distinct meetings that happen
   to share a name on the same day).
2. `meetings/lifecycle.rs` (renamed `resolver.rs`) is a **reconciliation
   layer**, not a CRUD factory — a calendar signal and a later window
   signal for the same real-world meeting must converge on one `Meeting`,
   not silently create two that turn out to be duplicates.
3. **Detection confidence gates persistence** — a window-title heuristic
   match is weaker evidence than a calendar event and shouldn't immediately
   become a vault record; it graduates from candidate to confirmed first.
4. **The OS notification and the interactive popup are one logical
   reminder**, not a redundant surface to delete outright — the actual bug
   was that they weren't coordinated, not that both existed.
5. **"Missed" is mostly a passive data state**, not something to actively
   resurface on wake — surfacing a stale reminder late can be more
   annoying than not reminding at all.
6. **The reminder engine reacts to meeting-record changes**; the polling
   loops that feed it (calendar sync, window detection) are the clock, not
   where the reminder decision logic lives.

The data flow this plan now targets:

```
Calendar signal ──┐
                   ├──►  Resolver  ──► persisted Meeting ──┬──► Reminders (reactive)
Window signal   ──┘   (reconciles,                          ├──► Meetings page
                        confidence-gates)                    └──► Recorder (shared AudioRecorder)
                                                                       │
                                                                       ▼
                                                                   Scribble
```

---

## 1. Why remove instead of patch

Decision 45 found the reminder popup's primary action fails for every
reminder kind (a meeting ID that never resolves to a saved record), a second
reminder silently erases the first, the popup freezes at its first on-screen
position, and several smaller integrity gaps. Patching each in place would
leave the same fragile foundation — a meeting ID that only sometimes refers
to something real — for the next feature to trip over again. This plan
instead fixes the foundation once — a meeting becomes a persisted,
first-class record as soon as Relay has sufficient evidence one exists,
never deferred to record-time (see §4.1) — and rebuilds the layer above it clean,
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

### 4.1 Meeting identity — a reconciliation layer, not independent CRUD

This is the fix for Decision 45's Broken #1 (the reminder popup's "Start
Recording" action failing for every kind). The root cause was that two
signals — calendar sync and window detection — each independently minted
their own incompatible ID shape for what might be the same real meeting.
The fix isn't "both call the same lookup function" — it's a resolver that
converges signals onto one persisted `Meeting`, with an identity hierarchy
strict enough that generic titles can't merge unrelated meetings, and a
confidence gate so a window-title heuristic doesn't spam the vault with
candidates that were never really meetings.

- **`meetings/detection.rs`** (new — the rebuilt window-detection signal
  from the removed `meetings/mod.rs` code, unchanged Win32 `EnumWindows`
  approach and provider/title helpers): also computes a real `confidence`
  for each match — `DetectedMeetingPayload.confidence` already exists on
  the struct today, confirmed via a repo-wide search to be set but never
  read anywhere, so this gives an existing, currently-decorative field a
  real purpose. A bare "Zoom Meeting"/"Meet - Google Meet" title scores
  lower than one with an actual topic/participant in it.
- **`meetings/resolver.rs`** (new — renamed from an earlier `lifecycle.rs`
  draft to describe what it actually does): the single entry point every
  signal goes through,
  `resolve_meeting_signal(signal: MeetingSignal) -> ResolvedMeeting`, where
  `ResolvedMeeting` is either `Persisted(Meeting)` or a `Candidate` not yet
  written to the vault. Matching hierarchy, most authoritative first:
  - **Calendar signal**: (1) exact `calendar_event_id` match against an
    existing `Meeting`; (2) same Google recurrence/series identifier, for a
    recurring series' next occurrence; (3) a time+title fallback only if
    neither ID is available, and only within a tight scheduled-time window
    — never title alone. Any match updates the existing `Meeting`; no match
    creates one. Calendar events are persisted immediately regardless of
    confidence — a scheduled event is already real, explicit evidence,
    independent of whether the user actually attends.
  - **Window-detection signal**: (1) a meeting URL/ID extracted from the
    signal, if available — in practice this rarely comes from the bare OS
    window title itself (Zoom/Meet don't put the meeting code in their
    title bar), so this tier mostly fires when correlating against a
    **calendar** event's own `meeting_url` (already a field on
    `CalendarMeetingEvent`), not the window title in isolation; (2)
    otherwise, match against an existing `Meeting` by provider + temporal
    proximity (within a few minutes of `scheduled_start`/`scheduled_end`,
    or of another already-confirmed detection) + normalized title
    similarity — a match here *corroborates* an existing meeting (e.g. sets
    `actual_start`) rather than creating a second one; (3) no match → held
    as an in-memory **candidate**, keyed by a short-lived `detection_key`
    (provider + cleaned title + rough time bucket) used only to recognize
    "still the same candidate I saw 10 seconds ago" — never as identity;
    (4) a candidate **graduates to a persisted `Meeting`** once confidence
    is high enough (a later iteration: sustained across repeated polls, not
    one instantaneous hit) or the user acts on it directly from a
    "Detected" reminder. Below that bar nothing is written to the vault —
    a stray browser tab with "zoom" in its title never becomes vault
    clutter.
- **Known limitation, accepted rather than solved here**: title-similarity
  matching can misjudge — two distinct meetings with near-identical names
  could be merged, or a real second occurrence could be treated as
  unrelated. There is no unlink/re-split UI in this pass; a wrong merge is
  recoverable by hand-editing the resulting `Meeting` today. A correction
  UI is a reasonable future addition, not built speculatively here.
- Both the calendar-sync loop and the window-detection loop call the
  resolver immediately on a new signal, not lazily at reminder- or
  record-time. A confirmed meeting shows up in the list (as "Upcoming" or
  "Detected" — see §4.4) as soon as it's persisted, which also means
  recording can be started from the list itself, not only from a popup.
- **`Meeting`'s new fields** split identity/linkage from detection
  provenance, rather than one flat `detection_key`:

  ```rust
  pub struct Meeting {
      pub id: String,
      // Identity / linkage
      pub series_id: Option<String>,          // existing — Relay's own MeetingSeries grouping
      pub calendar_event_id: Option<String>,  // existing — exact match, tier 1
      pub calendar_series_id: Option<String>, // new — Google's recurrence/series id, tier 2
      // Detection provenance — kept even after confirmation, so "why did
      // Relay create this meeting?" never requires reverse-engineering it
      // from the title
      pub detection_source: Option<String>,   // "calendar_sync" | "window_detector"
      pub detection_key: Option<String>,      // short-lived dedup fingerprint only — never identity
      pub detection_confidence: Option<f32>,  // gives the existing, unused confidence field a purpose
      pub detected_at: Option<String>,
      // ...unchanged: title, provider, scheduled/actual times, status,
      // participants, recording_path, transcript, notes, summary,
      // decisions, action_items, questions, candidate_scribbles, timestamps
  }
  ```

### 4.2 Reminders — event-driven, one logical reminder across two surfaces

Fixes Decision 45's Broken #2, Refactor #2/#3, and Improve #1–5:

- **States**: `Pending → Fired → {Snoozed(until) | Dismissed | Actioned | Expired}`,
  one `ReminderEvent` per (meeting, kind) in a `Vec`, never a single
  overwritable slot — closes Broken #2 directly. `Expired` means the fire
  window passed with no interaction; it's data, not automatically an
  interruption (see below).
- **Reactive, not a monolithic poll**: `meetings/reminders.rs` recomputes
  due reminders when told a meeting record changed — a callback/event from
  the calendar-sync loop or the resolver — rather than a fixed tick that
  re-derives everything from scratch. The two signal loops keep their own,
  independently appropriate cadences (window detection needs a short
  interval, since it's the only way to notice a call started, close to
  today's 15s; calendar sync can stay coarse, matching `calendar.rs`'s
  existing ~5-minute cache) — they're the clock, not where the reminder
  decision lives. This doesn't eliminate polling entirely: window
  enumeration and calendar freshness have no push mechanism available to a
  desktop app without much larger infrastructure (OS event hooks, calendar
  webhooks), so a timer still exists at the signal layer — what changes is
  that the reminder engine no longer redoes that work itself on every tick.
- **Recording gate is per-meeting** (Refactor #3): `is_recording_this_meeting(id)`
  replaces the old global `is_recording_meeting()` check.
- **One logical reminder, two coordinated surfaces — not a surface to
  delete** (Improve #3, revised): the existing OS notification
  (`tauri-plugin-notification`, confirmed a real dependency already called
  four times in today's `scheduler.rs` — not aspirational) and the
  interactive popup both render from the same `ReminderEvent`, so neither
  can drift from the other. The notification stays informational and, at
  minimum, focuses/raises the popup when clicked; the popup remains the
  only place with actual controls. Whether `tauri-plugin-notification`
  supports richer inline actions directly on Windows is worth checking
  during implementation, not assumed here.
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
- **`Expired` is passive, and only actively resurfaced when the underlying
  meeting might still be actionable** (Improve #2, revised): e.g. an
  `Unrecorded` reminder whose meeting hasn't reached `scheduled_end` yet is
  still worth a nudge; one for a meeting that ended an hour ago is not.
  Otherwise `Expired` just shows as a quiet badge on that meeting's row
  next time the list is open — never an interruption fired on wake, which
  can land more annoying than helpful.
- **One shared recording entry point**, `startMeetingRecording(meetingId)`
  (frontend), called identically from the popup's button, the meetings
  list, and the tray item. It always resolves any pending/fired reminder
  for that meeting as a side effect — this is what fixes both Refactor #1
  (two disconnected start-recording paths) and Improve #5 (acknowledging a
  meeting in the list not dismissing its popup) with one function, not two
  separate patches. The tray's "Start Recording" item (currently a no-op —
  Decision 45, Broken #3b) is wired to call it with whichever meeting is
  soonest/active.
- The dead `MEETING_DETECTED_EVENT`/`check_meeting_detection` pair (Broken
  #3a) is deleted, not reintroduced. `switch-to-meetings-tab` (Broken #3c)
  now carries the meeting ID in its payload, and the handler opens that
  meeting's detail pane, not just the tab.

*Research pattern applied here*: Hyprnote's own notification design gates a
detected-meeting popup to only interrupt when the main window is hidden.
Since Relay already asks the user before ever recording (see §4.5), the
popup only ever needs to *inform*, not *gate consent* — so this plan
borrows the hidden-window-only-interrupt idea for the `Detected` kind
specifically (no calendar backing it, the noisiest kind), applied to the
*popup's* interruptiveness — independent of whether the OS notification
still fires, which it does, per the revision above.

### 4.3 Recording — unchanged invariant, restated for this rebuild

`start_meeting_recording` (rebuilt, same name) takes the real meeting ID
the resolver already guaranteed is persisted, and calls
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
  sync/status flow `CalendarSyncModal.tsx` already implements. **Planned
  fix**: extract a single presentational `CalendarConnectionCard` (props:
  status, handlers) used by both `CalendarSyncModal` and the settings
  section, so there is one calendar-auth UI implementation, not two that
  can drift apart. **Status: NOT DONE — deferred.** Both copies work
  correctly today, so this is a maintainability cleanup rather than a bug
  fix, and it was left for its own pass rather than rushed alongside the
  functional work. The duplication is still present.
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
`meetings/detection.rs`, `meetings/resolver.rs`, `meetings/reminders.rs`

**Backend — kept as-is**
`meetings/calendar.rs` (calendar auth/sync only), `oauth/*`, `identity/*`,
`capture/*`, `overlay.rs` lines 1–173

**Backend — rewritten in place**
`vault/meeting.rs` (adds `calendar_series_id`, `detection_source`,
`detection_key`, `detection_confidence`, `detected_at` — drops nothing from
the existing notes/transcript/decisions/action-items shape, that part isn't
broken), `settings/mod.rs`'s `MeetingSettings`, the meetings-related
sections of `commands.rs` and `lib.rs`

**Backend — deleted**
`meetings/scheduler.rs`, `overlay.rs` lines 175–212 (folded into the rebuilt
reminder window function elsewhere in the same file)

**Frontend — rewritten in place**
`MeetingPage.tsx`, `MeetingListRail.tsx`, `MeetingDetailPane.tsx`,
`MeetingDetailView.tsx`, `useMeetingList.ts`, `MeetingReminderWindow.tsx`,
relevant sections of `App.tsx`, `main.tsx`, `NativeSidebar.tsx`
(repointed, not restructured), `ProviderSettings.tsx`'s Meetings section

**Deferred, not built**
`CalendarConnectionCard.tsx` (would be shared by `CalendarSyncModal.tsx`
and the settings section — see §4.5; the duplication it would remove is
still present)

---

## 6. Traceability — every Decision 45 item, and how this plan resolves it

| # | Decision 45 item | Resolved by |
| --- | --- | --- |
| Broken 1 | Record action fails for every reminder kind | §4.1 — a meeting ID is only ever real: minted by the resolver once persisted, never a synthetic string a downstream command has to guess at |
| Broken 2 | Second reminder erases the first permanently | §4.2 — queue, not a single slot; resolved only once actually seen/acted on |
| Broken 3 | Dead event/command, tray no-op, dropped tab-switch payload | §4.2 — dead pair deleted; tray and tab-switch both call the one shared recording entry point |
| Refactor 1 | Two start-recording paths, one doesn't clear state | §4.2 — one `startMeetingRecording` function, used everywhere |
| Refactor 2 | Popup position frozen after first show | §4.2 — reuses the pill's recompute-on-every-show anchor logic |
| Refactor 3 | Global, not per-meeting, recording gate | §4.2 — `is_recording_this_meeting(id)` |
| Refactor 4 | Settings/type drift, `auto_record` unread | §4.5 — all real settings exposed; `auto_record` removed on evidence |
| Improve 1 | No real snooze | §4.2 — "remind me in 5 min" as one of two popup actions |
| Improve 2 | Missed windows drop reminders silently | §4.2 — revised: `Expired` is passive data, actively resurfaced only if the meeting might still be actionable |
| Improve 3 | Duplicate OS toast + popup | §4.2 — revised: kept as one logical reminder driven by one `ReminderEvent`, not reduced to a single surface |
| Improve 4 | Popup steals focus during screen-share | §4.2 — fullscreen-aware `.setFocus()` suppression |
| Improve 5 | Acknowledging in list doesn't dismiss popup | §4.2 — side effect of the shared recording entry point |

Plus issues found during this investigation and its pre-implementation
review, not in Decision 45 itself:

| Source | Item | Resolved by |
| --- | --- | --- |
| This plan's own audit | Duplicate calendar-connect UI in `ProviderSettings.tsx` | **Deferred, not fixed** — see §4.5 |
| This plan's own audit | Two decorative settings toggles with no backend wiring | §4.5 — removed |
| Architecture review | `detection_key` used as identity, not just dedup | §4.1 — tiered identity hierarchy; key is fingerprint-only |
| Architecture review | Calendar and window signals could create duplicate `Meeting`s | §4.1 — resolver reconciles both onto one record |
| Architecture review | No confidence gate before persisting a detected meeting | §4.1 — candidate → confirmed graduation |
| Architecture review | Reminder engine repeats full sync/enum/match every tick | §4.2 — reactive recompute off meeting-record-change events |
| Architecture review | `Meeting`'s new field was one flat, ungrouped `detection_key` | §4.1 — split into identity/linkage vs. detection provenance |

---

## 7. Explicitly out of scope

No file under `native/src-tauri/src/oauth/`, `identity/`, `capture/`,
`vault/scribble.rs`, `triggers/`, `mcp/`, `tts/`, `providers/`, or `web/` is
modified. `overlay.rs` lines 1–173, everything under
`native/src/components/capture/`, and the Dictation Pill's settings are not
touched. No settings field outside `MeetingSettings` changes shape.

---

## 8. Implementation sequence

1. **Backend foundation** — `vault/meeting.rs`'s new identity/detection
   fields; `meetings/detection.rs` (with real `confidence` scoring);
   `meetings/resolver.rs`'s signal-matching hierarchy and candidate →
   confirmed graduation. Unit tests for idempotency (same calendar event
   synced twice; same window match polled twice) *and* for reconciliation
   (a calendar signal followed by a corroborating window signal updates one
   record, never creates two) before anything else is built on top.
2. **Reminder engine** — `meetings/reminders.rs`, the queue, the
   `Pending/Fired/Snoozed/Dismissed/Actioned/Expired` state machine wired
   reactively to resolver/calendar-sync change events, per-meeting
   recording gate. Unit tests for queue ordering, the never-overwrite
   guarantee, and that `Expired` alone never triggers an interruptive
   surface.
3. **Commands & wiring** — rebuild the meetings/calendar-adjacent commands in
   `commands.rs`, delete the dead pair, rewire `lib.rs` (scheduler spawn →
   reminder engine spawn, tray item, `invoke_handler` list).
4. **Popup** — rebuilt `overlay.rs` reminder window (reusing pill anchor
   logic) + rebuilt `MeetingReminderWindow.tsx` (two actions, no toast).
5. **Meetings page** — rebuilt `MeetingListRail.tsx` (state groups),
   `MeetingDetailPane.tsx` (Summary/Transcript tabs), `useMeetingList.ts`,
   `MeetingPage.tsx`.
6. **Settings** — remove the two decorative toggles and `auto_record`,
   expose all three real reminder settings. (The `CalendarConnectionCard`
   extraction originally listed here is deferred — see §4.5.)
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
- Manual: a calendar-synced meeting later corroborated by a window-detection
  signal (join the actual call) stays one `Meeting`, not two.
- Manual: a browser tab with "Zoom" in its title, never actually joined,
  does not create a persisted meeting.
- Manual: waking the laptop long after a meeting ended shows that
  meeting's reminder as an `Expired` badge in the list, not an interruptive
  popup/notification.
- Manual: popup reappears correctly positioned after switching monitors.
- Manual: recording from the meetings list clears a live popup for that
  meeting; recording from the tray does the same.
- Manual: disconnecting/reconnecting Google Calendar still works unchanged
  (regression check on the untouched auth path).
- Manual: Dictation Pill (global hotkey + click-to-talk) unaffected.
