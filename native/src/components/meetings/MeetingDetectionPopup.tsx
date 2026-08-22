import React, { useState, useEffect } from 'react';
import { listen } from '@tauri-apps/api/event';
import { DetectedMeetingPayload } from '../../types';
import { Button } from '@/components/ui/button';
import { Badge } from '@/components/ui/badge';
import {
  Video,
  Mic,
  X,
  Calendar,
  Sparkles,
} from 'lucide-react';

interface MeetingDetectionPopupProps {
  onStartMeetingRecording: (meetingId: string) => Promise<void>;
  onCreateAndStartMeeting?: (detected: DetectedMeetingPayload) => Promise<void>;
}

export const MeetingDetectionPopup: React.FC<MeetingDetectionPopupProps> = ({
  onStartMeetingRecording,
  onCreateAndStartMeeting,
}) => {
  const [detectedMeeting, setDetectedMeeting] = useState<DetectedMeetingPayload | null>(null);
  const [dismissedEvents, setDismissedEvents] = useState<Set<string>>(new Set());

  useEffect(() => {
    const unlisten = listen<DetectedMeetingPayload>('meeting-detected', ({ payload }) => {
      if (payload && !dismissedEvents.has(payload.event_id)) {
        setDetectedMeeting(payload);
      }
    });

    return () => {
      unlisten.then((fn) => fn());
    };
  }, [dismissedEvents]);

  if (!detectedMeeting) return null;

  const handleDismiss = () => {
    setDismissedEvents((prev) => new Set(prev).add(detectedMeeting.event_id));
    setDetectedMeeting(null);
  };

  const handleStart = async () => {
    if (onCreateAndStartMeeting) {
      await onCreateAndStartMeeting(detectedMeeting);
    }
    handleDismiss();
  };

  return (
    <div className="fixed bottom-6 right-6 z-50 animate-in slide-in-from-bottom-5 fade-in duration-300 max-w-sm w-full">
      <div className="bg-card/95 backdrop-blur-md border border-primary/40 rounded-2xl shadow-2xl p-4 space-y-3 relative overflow-hidden">
        {/* Glow accent */}
        <div className="absolute -top-10 -right-10 w-24 h-24 bg-primary/20 rounded-full blur-2xl pointer-events-none" />

        {/* Top bar */}
        <div className="flex items-center justify-between">
          <div className="flex items-center gap-2">
            <span className="relative flex h-2 w-2">
              <span className="animate-ping absolute inline-flex h-full w-full rounded-full bg-emerald-400 opacity-75"></span>
              <span className="relative inline-flex rounded-full h-2 w-2 bg-emerald-500"></span>
            </span>
            <span className="text-[11px] font-mono font-bold tracking-wider uppercase text-primary">
              Meeting Detected
            </span>
          </div>

          <button
            type="button"
            onClick={handleDismiss}
            className="text-muted-foreground hover:text-foreground transition-colors p-1"
            title="Dismiss detection"
          >
            <X className="w-3.5 h-3.5" />
          </button>
        </div>

        {/* Meeting Info */}
        <div className="space-y-1">
          <div className="flex items-center gap-1.5 flex-wrap">
            <Badge
              variant="outline"
              className="text-[9px] uppercase font-mono py-0 px-1.5 border-primary/40 bg-primary/10 text-primary"
            >
              {detectedMeeting.provider.replace('_', ' ')}
            </Badge>
            <span className="text-[10px] text-muted-foreground font-mono">
              {detectedMeeting.detection_source === 'calendar' ? 'Calendar Event' : 'Active App Window'}
            </span>
          </div>

          <h4 className="text-xs font-bold text-foreground line-clamp-2 leading-tight">
            {detectedMeeting.title}
          </h4>
        </div>

        {/* Action buttons */}
        <div className="flex items-center gap-2 pt-1">
          <Button
            size="sm"
            onClick={handleStart}
            className="flex-1 text-xs h-8 bg-emerald-600 hover:bg-emerald-700 text-white gap-1.5 font-medium shadow-xs"
          >
            <Mic className="w-3.5 h-3.5" />
            <span>Start Recording</span>
          </Button>

          <Button
            size="sm"
            variant="outline"
            onClick={handleDismiss}
            className="text-xs h-8 text-muted-foreground hover:text-foreground"
          >
            Not this meeting
          </Button>
        </div>
      </div>
    </div>
  );
};
