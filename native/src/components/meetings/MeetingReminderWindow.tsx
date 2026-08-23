import React, { useCallback, useEffect, useState } from 'react';
import { listen } from '@tauri-apps/api/event';
import { invoke } from '@tauri-apps/api/core';
import { getCurrentWindow } from '@tauri-apps/api/window';
import { X } from 'lucide-react';
import { Button } from '../ui/button';
import { MeetingReminderEvent, ReminderKind } from '../../types';

import {
  DESIGN_OPTIONS,
  renderNotificationCard,
} from './MeetingNotificationsDesignGallery';

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
  const [activeThemeId, setActiveThemeId] = useState<number>(() => {
    const saved = localStorage.getItem('relay_meeting_reminder_theme');
    return saved ? parseInt(saved, 10) : 1;
  });

  const refresh = useCallback(() => {
    invoke<MeetingReminderEvent | null>('get_current_meeting_reminder')
      .then(setReminder)
      .catch(console.error);

    const saved = localStorage.getItem('relay_meeting_reminder_theme');
    if (saved) {
      setActiveThemeId(parseInt(saved, 10));
    }
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
    const handleStorage = () => refresh();
    window.addEventListener('storage', handleStorage);
    window.addEventListener('relay-reminder-theme-changed', handleStorage);
    return () => {
      unlisten.then((fn) => fn());
      window.removeEventListener('storage', handleStorage);
      window.removeEventListener('relay-reminder-theme-changed', handleStorage);
    };
  }, [refresh]);

  const withBusyGuard = async (action: () => Promise<void>) => {
    if (!reminder || busy) return;
    setBusy(true);
    // Hide window immediately when user triggers an action
    getCurrentWindow().hide().catch(console.error);
    setReminder(null);
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

  const handleDismiss = () => {
    getCurrentWindow().hide().catch(console.error);
    setReminder(null);
    withBusyGuard(() =>
      invoke('dismiss_meeting_reminder', { meetingId: reminder!.meeting_id, kind: reminder!.kind })
    );
  };

  if (!reminder) return null;

  const selectedOption =
    DESIGN_OPTIONS.find((o) => o.id === activeThemeId) || DESIGN_OPTIONS[0];

  return (
    <div
      className="size-full bg-transparent p-0 m-0 overflow-hidden select-none flex items-center justify-center"
      data-tauri-drag-region
    >
      {renderNotificationCard(selectedOption, {
        preset: 'Executive Summary',
        inputMode: 'both',
        onStart: handleStartRecording,
        onSnooze: handleSnooze,
        onDismiss: handleDismiss,
      })}
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
