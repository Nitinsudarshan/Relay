import React, { useCallback, useEffect, useState } from 'react';
import { listen } from '@tauri-apps/api/event';
import { invoke } from '@tauri-apps/api/core';
import { getCurrentWindow } from '@tauri-apps/api/window';
import { MeetingReminderPayload, ReminderKind } from '../../types';
import { Video, Calendar, Clock, Disc, X, BellOff, Users } from 'lucide-react';
import { Button } from '@/components/ui/button';
import { Badge } from '@/components/ui/badge';

export const MeetingReminderWindow: React.FC = () => {
  const [reminder, setReminder] = useState<MeetingReminderPayload | null>(null);
  const [busy, setBusy] = useState(false);
  const [isClosing, setIsClosing] = useState(false);

  // 1. Show Protocol & Handshake: Pull pending data + subscribe to event + signal ready
  useEffect(() => {
    let isMounted = true;

    // Pull pending reminder from backend
    invoke<MeetingReminderPayload | null>('get_pending_meeting_reminder')
      .then((data) => {
        if (isMounted && data) {
          setIsClosing(false);
          setReminder(data);
        }
      })
      .catch((err) => {
        console.warn('[MeetingReminderWindow] Failed to get pending reminder:', err);
      });

    // Subscribe to push events
    const unlistenPromise = listen<MeetingReminderPayload>('meeting-reminder', (event) => {
      if (isMounted && event.payload) {
        setIsClosing(false);
        setReminder(event.payload);
      }
    });

    // Signal frontend readiness to Rust
    invoke('meeting_reminder_ready').catch((err) => {
      console.warn('[MeetingReminderWindow] Failed to signal ready:', err);
    });

    return () => {
      isMounted = false;
      unlistenPromise.then((unlisten) => unlisten());
    };
  }, []);

  // 2. Hover Interactivity signals to backend auto-dismiss timer
  const handleMouseEnter = useCallback(() => {
    invoke('meeting_reminder_hover_changed', { hovered: true }).catch(() => {});
  }, []);

  const handleMouseLeave = useCallback(() => {
    invoke('meeting_reminder_hover_changed', { hovered: false }).catch(() => {});
  }, []);

  // 3. Action Execution wrapper: smooth slide-out animation, then hides window and fires IPC
  const executeAction = async (action: (r: MeetingReminderPayload) => Promise<void>) => {
    const current = reminder;
    if (!current || busy) return;

    setBusy(true);
    setIsClosing(true);

    // Wait 220ms for smooth exit animation
    await new Promise((resolve) => setTimeout(resolve, 220));

    try {
      await getCurrentWindow().hide();
    } catch (e) {
      console.warn('Failed to hide window:', e);
    }
    setReminder(null);
    setIsClosing(false);

    try {
      await action(current);
    } catch (err) {
      console.error('[MeetingReminderWindow] Action failed:', err);
    } finally {
      setBusy(false);
    }
  };

  const handleStartRecording = () =>
    executeAction((r) =>
      invoke('start_meeting_recording', { meetingId: r.meeting_id })
    );

  const handleSnooze = (minutes = 5) =>
    executeAction((r) =>
      invoke('snooze_meeting_reminder', {
        meetingId: r.meeting_id,
        kind: r.kind,
        minutes,
      })
    );

  const handleDismiss = () =>
    executeAction((r) =>
      invoke('dismiss_meeting_reminder', {
        meetingId: r.meeting_id,
        kind: r.kind,
      })
    );

  if (!reminder) {
    return null;
  }

  const getProviderIcon = (provider: string) => {
    switch (provider.toLowerCase()) {
      case 'google_meet':
      case 'google meet':
      case 'zoom':
      case 'teams':
      case 'webex':
        return <Video className="w-3.5 h-3.5 text-blue-500" />;
      default:
        return <Calendar className="w-3.5 h-3.5 text-primary" />;
    }
  };

  const getKindBadge = (kind: ReminderKind) => {
    switch (kind) {
      case 'upcoming':
        return (
          <Badge
            variant="outline"
            className="text-[10px] h-4.5 px-1.5 font-medium bg-blue-500/10 text-blue-500 border-blue-500/20 gap-1"
          >
            <Clock className="w-2.5 h-2.5" />
            <span>Starts Soon</span>
          </Badge>
        );
      case 'unrecorded':
        return (
          <Badge
            variant="outline"
            className="text-[10px] h-4.5 px-1.5 font-medium bg-amber-500/10 text-amber-500 border-amber-500/20 gap-1"
          >
            <Disc className="w-2.5 h-2.5 animate-pulse" />
            <span>In Progress</span>
          </Badge>
        );
      case 'detected':
        return (
          <Badge
            variant="outline"
            className="text-[10px] h-4.5 px-1.5 font-medium bg-emerald-500/10 text-emerald-500 border-emerald-500/20 gap-1"
          >
            <span className="w-1.5 h-1.5 rounded-full bg-emerald-500 animate-ping" />
            <span>Detected</span>
          </Badge>
        );
    }
  };

  return (
    <div
      className="w-full h-full p-2 select-none box-border flex items-center justify-center bg-transparent"
      onMouseEnter={handleMouseEnter}
      onMouseLeave={handleMouseLeave}
      data-tauri-drag-region
    >
      <div
        className={`w-[400px] rounded-xl border border-border/80 bg-card/95 backdrop-blur-xl p-3.5 shadow-2xl transition-all duration-200 ease-out hover:border-primary/40 relative overflow-hidden transform ${
          isClosing
            ? 'opacity-0 translate-x-4 scale-95 pointer-events-none'
            : 'opacity-100 translate-x-0 scale-100 animate-in fade-in slide-in-from-top-3'
        }`}
      >
        {/* Ambient background glow */}
        <div className="absolute -right-12 -top-12 w-28 h-28 bg-primary/10 rounded-full blur-2xl pointer-events-none" />

        {/* Top Header Row */}
        <div className="flex items-center justify-between gap-2 mb-2">
          <div className="flex items-center gap-1.5 min-w-0">
            <div className="p-1 rounded-md bg-secondary/80 flex items-center justify-center shrink-0">
              {getProviderIcon(reminder.provider)}
            </div>
            <span className="text-xs font-semibold text-foreground truncate">
              {reminder.provider_name}
            </span>
            {getKindBadge(reminder.kind)}
          </div>

          <button
            type="button"
            onClick={handleDismiss}
            disabled={busy}
            className="p-1 rounded-md text-muted-foreground hover:text-foreground hover:bg-secondary/80 transition-colors shrink-0"
            title="Dismiss reminder"
            aria-label="Dismiss reminder"
          >
            <X className="w-3.5 h-3.5" />
          </button>
        </div>

        {/* Meeting Title & Time Subtitle */}
        <div className="mb-3 space-y-0.5">
          <h3
            className="text-xs font-bold text-foreground line-clamp-1 leading-snug"
            title={reminder.title}
          >
            {reminder.title}
          </h3>
          <div className="flex items-center gap-2 text-[11px] text-muted-foreground">
            <span>{reminder.time_label}</span>
            {reminder.participants && reminder.participants.length > 0 && (
              <>
                <span className="text-border">•</span>
                <span className="flex items-center gap-1">
                  <Users className="w-3 h-3 text-muted-foreground/70" />
                  <span>{reminder.participants.length}</span>
                </span>
              </>
            )}
          </div>
        </div>

        {/* Action Button Grid: Primary Record, Snooze 5m, Dismiss */}
        <div className="grid grid-cols-3 gap-2 pt-1 border-t border-border/50">
          <Button
            size="sm"
            variant="default"
            onClick={handleStartRecording}
            disabled={busy}
            className="col-span-1 h-7.5 text-xs font-semibold bg-red-600 hover:bg-red-700 text-white shadow-xs gap-1.5 px-2.5 transition-transform active:scale-95"
          >
            <Disc className="w-3 h-3 fill-current" />
            <span>Record</span>
          </Button>

          <Button
            size="sm"
            variant="secondary"
            onClick={() => handleSnooze(5)}
            disabled={busy}
            className="col-span-1 h-7.5 text-xs font-medium bg-secondary hover:bg-secondary/80 text-foreground px-2 gap-1.5 transition-transform active:scale-95"
          >
            <Clock className="w-3 h-3 text-muted-foreground" />
            <span>Snooze 5m</span>
          </Button>

          <Button
            size="sm"
            variant="outline"
            onClick={handleDismiss}
            disabled={busy}
            className="col-span-1 h-7.5 text-xs font-medium text-muted-foreground hover:text-foreground hover:bg-destructive/10 hover:border-destructive/30 px-2 gap-1.5 transition-transform active:scale-95"
          >
            <BellOff className="w-3 h-3" />
            <span>Dismiss</span>
          </Button>
        </div>
      </div>
    </div>
  );
};
