import React, { useState, useEffect, useCallback } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { MeetingReminderEvent, ReminderKind } from '@/types';
import { MeetingNotificationData, MeetingProvider, MeetingStatus } from './notification-types';
import { NativeInspired } from './variants/NativeInspired';
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

export const MeetingReminderToastListener: React.FC = () => {
  const [activeReminder, setActiveReminder] = useState<MeetingReminderEvent | null>(null);
  const [isRecording, setIsRecording] = useState(false);

  const refresh = useCallback(() => {
    invoke<MeetingReminderEvent | null>('get_current_meeting_reminder')
      .then((res) => {
        setActiveReminder(res);
      })
      .catch((err) => {
        console.warn('Could not fetch active meeting reminder:', err);
      });
  }, []);

  useEffect(() => {
    refresh();
    const unlistenPromise = listen('meeting-reminder', refresh);
    return () => {
      unlistenPromise.then((unlisten) => unlisten());
    };
  }, [refresh]);

  if (!activeReminder) return null;

  const handleRecord = async () => {
    const reminder = activeReminder;
    setIsRecording(true);
    try {
      await invoke('start_meeting_recording', { meetingId: reminder.meeting_id });
    } catch (e) {
      console.error('Failed to start meeting recording from toast:', e);
    } finally {
      setIsRecording(false);
      setActiveReminder(null);
    }
  };

  const handleSnooze = async (minutes: number) => {
    const reminder = activeReminder;
    setActiveReminder(null);
    try {
      await invoke('snooze_meeting_reminder', {
        meetingId: reminder.meeting_id,
        kind: reminder.kind,
        minutes,
      });
    } catch (e) {
      console.error('Failed to snooze meeting reminder:', e);
    }
  };

  const handleDismiss = async () => {
    const reminder = activeReminder;
    setActiveReminder(null);
    try {
      await invoke('dismiss_meeting_reminder', {
        meetingId: reminder.meeting_id,
        kind: reminder.kind,
      });
    } catch (e) {
      console.error('Failed to dismiss meeting reminder:', e);
    }
  };

  const notificationData: MeetingNotificationData = {
    title: activeReminder.title,
    status: mapKindToStatus(activeReminder.kind),
    timeLabel: formatKindTimeLabel(activeReminder.kind),
    provider: formatProviderName(activeReminder.provider),
    participants: activeReminder.participants?.length || 1,
  };

  return (
    <div className="fixed top-6 right-6 z-50 max-w-[400px] w-full animate-in slide-in-from-top-4 fade-in duration-300 pointer-events-auto shadow-2xl rounded-md bg-card border border-border">
      <div className="relative">
        <button
          type="button"
          onClick={handleDismiss}
          className="absolute top-2.5 right-2.5 p-1 text-muted-foreground hover:text-foreground rounded transition-colors z-10"
          title="Dismiss notification"
        >
          <X className="w-3.5 h-3.5" />
        </button>
        <NativeInspired
          data={notificationData}
          isRecording={isRecording}
          isDismissed={false}
          snoozedMinutes={null}
          actions={{
            onRecord: handleRecord,
            onSnooze: handleSnooze,
            onDismiss: handleDismiss,
          }}
        />
      </div>
    </div>
  );
};
