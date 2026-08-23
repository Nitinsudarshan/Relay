import React, { useCallback, useEffect, useState } from 'react';
import { listen } from '@tauri-apps/api/event';
import { invoke } from '@tauri-apps/api/core';
import { getCurrentWindow } from '@tauri-apps/api/window';
import { X } from 'lucide-react';
import { Button } from '../ui/button';
import { MeetingReminderEvent, ReminderKind } from '../../types';

/// One logical reminder, one interactive surface. This window shows
/// whichever reminder the backend queue says is earliest-and-currently-due
/// (`get_current_meeting_reminder`) — never a locally-cached "last event
/// received," since a second reminder firing while this one is still up
/// must not make the first one silently disappear (Decision 45, Broken #2).
/// An OS notification fires alongside this from the same backend event;
/// this window is the only place with actual controls (§4.2).
export const MeetingReminderWindow: React.FC = () => {
  const [reminder, setReminder] = useState<MeetingReminderEvent | null>(null);
  const [busy, setBusy] = useState(false);

  const refresh = useCallback(() => {
    invoke<MeetingReminderEvent | null>('get_current_meeting_reminder')
      .then(setReminder)
      .catch(console.error);
  }, []);

  useEffect(() => {
    const currentWindow = getCurrentWindow();
    if (!reminder) {
      currentWindow.hide().catch(console.error);
    } else {
      currentWindow.show().catch(console.error);
    }
  }, [reminder]);

  useEffect(() => {
    refresh();
    const unlisten = listen('meeting-reminder', refresh);
    return () => {
      unlisten.then((fn) => fn());
    };
  }, [refresh]);

  const withBusyGuard = async (action: () => Promise<void>) => {
    if (!reminder || busy) return;
    setBusy(true);
    try {
      await action();
    } catch (e) {
      console.error('Meeting reminder action failed', e);
    } finally {
      setBusy(false);
      refresh();
    }
  };

  const handleStartRecording = () =>
    withBusyGuard(() => invoke('start_meeting_recording', { meetingId: reminder!.meeting_id }));

  const handleSnooze = () =>
    withBusyGuard(() =>
      invoke('snooze_meeting_reminder', {
        meetingId: reminder!.meeting_id,
        kind: reminder!.kind,
        minutes: 5,
      })
    );

  const handleDismiss = () =>
    withBusyGuard(() =>
      invoke('dismiss_meeting_reminder', { meetingId: reminder!.meeting_id, kind: reminder!.kind })
    );

  if (!reminder) return null;

  const headerText = headerTextForKind(reminder.kind);

  return (
    <div className="flex flex-col bg-background/95 backdrop-blur-md border border-border shadow-xl rounded-xl h-full w-full select-none overflow-hidden" data-tauri-drag-region>
      <div className="flex items-center justify-between px-4 py-2 border-b border-border/50 bg-muted/30" data-tauri-drag-region>
        <span className="text-xs font-semibold uppercase tracking-wider text-muted-foreground" data-tauri-drag-region>
          {headerText}
        </span>
        <button
          onClick={handleDismiss}
          disabled={busy}
          className="text-muted-foreground hover:text-foreground p-1 -mr-1 rounded-full transition-colors disabled:opacity-50"
          title="Dismiss"
        >
          <X className="w-4 h-4" />
        </button>
      </div>

      <div className="flex flex-col flex-1 px-4 py-3 justify-between">
        <div>
          <h3 className="text-sm font-medium line-clamp-1 text-foreground" title={reminder.title}>
            {reminder.title || 'Untitled Meeting'}
          </h3>
          <p className="text-xs text-muted-foreground mt-0.5">
            {formatProvider(reminder.provider)}
            {reminder.participants.length > 0 && ` • ${reminder.participants.length} participants`}
          </p>
        </div>

        <div className="flex items-center justify-end space-x-2 mt-auto">
          <Button
            variant="ghost"
            size="sm"
            className="h-7 text-xs"
            onClick={handleSnooze}
            disabled={busy}
          >
            Remind me in 5 min
          </Button>
          <Button
            variant="default"
            size="sm"
            className="h-7 text-xs bg-red-600 hover:bg-red-700 text-white"
            onClick={handleStartRecording}
            disabled={busy}
          >
            Start Recording
          </Button>
        </div>
      </div>
    </div>
  );
};

function headerTextForKind(kind: ReminderKind): string {
  switch (kind) {
    case 'upcoming':
      return 'Upcoming Meeting';
    case 'unrecorded':
      return 'Meeting in Progress';
    case 'detected':
      return 'Meeting Detected';
    default:
      return 'Meeting Reminder';
  }
}

function formatProvider(provider: string): string {
  const known: Record<string, string> = {
    google_meet: 'Google Meet',
    zoom: 'Zoom',
    teams: 'Teams',
    webex: 'Webex',
    in_person: 'In Person',
  };
  return known[provider] ?? 'Meeting';
}
