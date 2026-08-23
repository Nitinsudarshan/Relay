import React, { useCallback, useEffect, useState } from 'react';
import { listen } from '@tauri-apps/api/event';
import { invoke } from '@tauri-apps/api/core';
import { getCurrentWindow } from '@tauri-apps/api/window';
import { MeetingReminderEvent, ReminderKind } from '../../types';
import { MeetingNotificationData, MeetingProvider, MeetingStatus } from './notifications/notification-types';
import { NativeInspired } from './notifications/variants/NativeInspired';
import { X } from 'lucide-react';

function formatProviderName(provider: string): MeetingProvider {
  switch (provider.toLowerCase()) {
    case 'google_meet':
    case 'google meet':
      return 'Google Meet';
    case 'zoom':
      return 'Zoom';
    case 'teams':
      return 'Teams';
    case 'webex':
      return 'Webex';
    default:
      return 'In Person';
  }
}

function formatKindTimeLabel(kind: ReminderKind): string {
  switch (kind) {
    case 'upcoming':
      return 'starts in 5 minutes';
    case 'unrecorded':
      return 'Meeting in progress';
    case 'detected':
      return 'Meeting detected';
    default:
      return 'Meeting alert';
  }
}

function mapKindToStatus(kind: ReminderKind): MeetingStatus {
  switch (kind) {
    case 'upcoming':
      return 'upcoming';
    case 'detected':
      return 'detected';
    case 'unrecorded':
      return 'in-progress';
    default:
      return 'upcoming';
  }
}

export const MeetingReminderWindow: React.FC = () => {
  const [reminder, setReminder] = useState<MeetingReminderEvent | null>(null);
  const [isRecording, setIsRecording] = useState(false);
  const [hovered, setHovered] = useState(false);
  const [progress, setProgress] = useState(100);

  const refresh = useCallback(() => {
    invoke<MeetingReminderEvent | null>('get_current_meeting_reminder')
      .then(setReminder)
      .catch(console.error);
  }, []);

  useEffect(() => {
    refresh();
    const unlistenPromise = listen('meeting-reminder', refresh);
    return () => {
      unlistenPromise.then((unlisten) => unlisten());
    };
  }, [refresh]);

  // Sync window visibility with active reminder state
  useEffect(() => {
    const win = getCurrentWindow();
    if (!reminder) {
      win.hide().catch(console.error);
    } else {
      win.show().catch(console.error);
    }
  }, [reminder]);

  // 5-second auto-dismiss timer (pauses on hover)
  useEffect(() => {
    if (!reminder || hovered) return;
    const startTime = Date.now();
    const duration = 5000;

    const interval = setInterval(() => {
      const elapsed = Date.now() - startTime;
      const remaining = Math.max(0, 100 - (elapsed / duration) * 100);
      setProgress(remaining);

      if (remaining <= 0) {
        clearInterval(interval);
        handleDismiss();
      }
    }, 50);

    return () => clearInterval(interval);
  }, [reminder, hovered]);

  const hideAndRun = (action: (r: MeetingReminderEvent) => Promise<void>) => {
    const captured = reminder;
    if (!captured) return;

    // 1. Hide desktop window immediately
    getCurrentWindow().hide().catch(console.error);
    setReminder(null);

    // 2. Execute Rust backend command with captured reminder data
    action(captured).catch((e) =>
      console.error('Meeting reminder action failed:', e)
    );
  };

  const handleStartRecording = () => {
    setIsRecording(true);
    hideAndRun((r) => invoke('start_meeting_recording', { meetingId: r.meeting_id }));
  };

  const handleSnooze = (minutes: number) =>
    hideAndRun((r) =>
      invoke('snooze_meeting_reminder', {
        meetingId: r.meeting_id,
        kind: r.kind,
        minutes,
      })
    );

  const handleDismiss = () =>
    hideAndRun((r) =>
      invoke('dismiss_meeting_reminder', { meetingId: r.meeting_id, kind: r.kind })
    );

  if (!reminder) return null;

  const notificationData: MeetingNotificationData = {
    title: reminder.title,
    status: mapKindToStatus(reminder.kind),
    timeLabel: formatKindTimeLabel(reminder.kind),
    provider: formatProviderName(reminder.provider),
    participants: reminder.participants?.length || 1,
  };

  return (
    <div
      onMouseEnter={() => setHovered(true)}
      onMouseLeave={() => setHovered(false)}
      className="size-full bg-transparent p-0 m-0 overflow-hidden select-none flex flex-col justify-center items-center relative"
      data-tauri-drag-region
    >
      <div className="w-full max-w-[400px] relative rounded-md bg-card border border-border shadow-2xl overflow-hidden ring-1 ring-primary/20">
        <button
          type="button"
          onClick={handleDismiss}
          className="absolute top-2.5 right-2.5 p-1 text-muted-foreground hover:text-foreground rounded transition-colors z-10"
          title="Dismiss desktop notification"
        >
          <X className="w-3.5 h-3.5" />
        </button>

        <NativeInspired
          data={notificationData}
          isRecording={isRecording}
          isDismissed={false}
          snoozedMinutes={null}
          actions={{
            onRecord: handleStartRecording,
            onSnooze: handleSnooze,
            onDismiss: handleDismiss,
          }}
        />

        {/* 5-second OS Auto-Dismiss Progress Bar */}
        <div className="w-full bg-muted/60 h-1 overflow-hidden">
          <div
            className={`h-full transition-all duration-75 ${
              hovered ? 'bg-amber-500' : 'bg-primary'
            }`}
            style={{ width: `${progress}%` }}
          />
        </div>
      </div>
    </div>
  );
};
