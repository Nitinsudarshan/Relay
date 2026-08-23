import React, { useEffect, useState } from 'react';
import { listen } from '@tauri-apps/api/event';
import { invoke } from '@tauri-apps/api/core';
import { getCurrentWindow } from '@tauri-apps/api/window';
import { X } from 'lucide-react';
import { Button } from '../ui/button';

interface MeetingReminderPayload {
  meeting_id: string;
  title: string;
  provider: string;
  kind: string; // "upcoming" | "unrecorded" | "detected"
  participants: string[];
}

export const MeetingReminderWindow: React.FC = () => {
  const [reminder, setReminder] = useState<MeetingReminderPayload | null>(null);
  
  // Hide the window if no reminder is active
  useEffect(() => {
    const currentWindow = getCurrentWindow();
    if (!reminder) {
      currentWindow.hide().catch(console.error);
    } else {
      currentWindow.show().catch(console.error);
      currentWindow.setFocus().catch(console.error);
    }
  }, [reminder]);

  useEffect(() => {
    // 1. Fetch current active reminder immediately
    invoke<MeetingReminderPayload | null>('get_active_meeting_reminder')
      .then(payload => {
        if (payload) setReminder(payload);
      })
      .catch(console.error);

    // 2. Listen for future reminders
    const unlisten = listen<MeetingReminderPayload>('meeting-reminder', (e) => {
      setReminder(e.payload);
    });

    return () => {
      unlisten.then((fn) => fn());
    };
  }, []);

  const handleStartRecording = async () => {
    if (!reminder) return;
    try {
      await invoke('start_recording_from_reminder', { meetingId: reminder.meeting_id });
      setReminder(null);
    } catch (e) {
      console.error('Failed to start recording from reminder', e);
    }
  };

  const handleDismiss = async (permanent: boolean) => {
    if (!reminder) return;
    try {
      await invoke('dismiss_meeting_reminder', { meetingId: reminder.meeting_id, permanent });
      setReminder(null);
    } catch (e) {
      console.error('Failed to dismiss reminder', e);
    }
  };

  if (!reminder) return null;

  const headerText = 
    reminder.kind === 'upcoming' ? 'Upcoming Meeting' : 
    reminder.kind === 'unrecorded' ? 'Meeting in Progress' : 
    'Meeting Detected';

  return (
    <div className="flex flex-col bg-background/95 backdrop-blur-md border border-border shadow-xl rounded-xl h-full w-full select-none overflow-hidden" data-tauri-drag-region>
      <div className="flex items-center justify-between px-4 py-2 border-b border-border/50 bg-muted/30" data-tauri-drag-region>
        <span className="text-xs font-semibold uppercase tracking-wider text-muted-foreground" data-tauri-drag-region>
          {headerText}
        </span>
        <button 
          onClick={() => handleDismiss(false)}
          className="text-muted-foreground hover:text-foreground p-1 -mr-1 rounded-full transition-colors"
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
            {reminder.provider.charAt(0).toUpperCase() + reminder.provider.slice(1).replace('_', ' ')}
            {reminder.participants.length > 0 && ` • ${reminder.participants.length} participants`}
          </p>
        </div>
        
        <div className="flex items-center justify-end space-x-2 mt-auto">
          <Button 
            variant="ghost" 
            size="sm" 
            className="h-7 text-xs"
            onClick={() => handleDismiss(true)}
          >
            Don't remind me
          </Button>
          <Button 
            variant="default" 
            size="sm" 
            className="h-7 text-xs bg-red-600 hover:bg-red-700 text-white"
            onClick={handleStartRecording}
          >
            Record
          </Button>
        </div>
      </div>
    </div>
  );
};
