import React from 'react';
import { Calendar, Users, User, Play, Square, RotateCcw } from 'lucide-react';
import { Button } from '@/components/ui/button';
import { Badge } from '@/components/ui/badge';
import { VariantProps } from '../notification-types';
import { SnoozeMenu } from './SnoozeMenu';

export const RichContext: React.FC<VariantProps> = ({
  data,
  actions,
  isRecording,
  isDismissed,
  snoozedMinutes,
}) => {
  if (isDismissed) {
    return (
      <div className="w-full max-w-[400px] min-h-[140px] rounded-lg border border-dashed border-border bg-muted/20 p-4 flex flex-col items-center justify-center gap-2 text-center text-xs text-muted-foreground">
        <span>Rich Context Notification Dismissed</span>
        <Button
          type="button"
          variant="outline"
          size="sm"
          onClick={actions.onDismiss}
          className="h-7 text-xs gap-1.5"
        >
          <RotateCcw className="w-3 h-3" /> Restore Preview
        </Button>
      </div>
    );
  }

  const organizer = data.organizer || 'Nitin • Product Team';
  const participants = data.participants || 4;

  return (
    <div className="w-full max-w-[400px] rounded-lg border border-border bg-card p-4 shadow-sm select-none flex flex-col justify-between space-y-3">
      {/* Category & Status Header */}
      <div className="flex items-center justify-between text-[11px] text-muted-foreground">
        <div className="flex items-center gap-1.5 font-medium">
          <Calendar className="w-3.5 h-3.5 text-primary" />
          <span>Relay • Upcoming Meeting</span>
        </div>
        {snoozedMinutes ? (
          <Badge variant="outline" className="text-[10px] text-amber-500">
            Snoozed {snoozedMinutes}m
          </Badge>
        ) : isRecording ? (
          <Badge className="text-[10px] bg-red-600 text-white animate-pulse">Recording</Badge>
        ) : (
          <Badge variant="secondary" className="text-[10px]">
            {data.status}
          </Badge>
        )}
      </div>

      {/* Meeting Title & Detailed Context */}
      <div className="space-y-1">
        <h4 className="text-sm font-bold text-foreground line-clamp-1">{data.title}</h4>
        <p className="text-xs text-primary font-semibold">{data.timeLabel}</p>

        {/* Rich Metadata Chips */}
        <div className="flex items-center gap-3 pt-1 text-[11px] text-muted-foreground">
          <span className="flex items-center gap-1">
            <User className="w-3 h-3" /> {organizer}
          </span>
          <span className="flex items-center gap-1">
            <Users className="w-3 h-3" /> {participants} attendees
          </span>
        </div>
      </div>

      {/* Action Row */}
      <div className="flex items-center justify-between pt-2 border-t border-border/50">
        <Button
          type="button"
          size="sm"
          variant={isRecording ? 'destructive' : 'default'}
          onClick={actions.onRecord}
          className="h-7 text-xs font-semibold gap-1.5 rounded-md px-3"
        >
          {isRecording ? (
            <>
              <Square className="w-3 h-3 fill-current" /> Stop
            </>
          ) : (
            <>
              <Play className="w-3 h-3 fill-current" /> Record
            </>
          )}
        </Button>

        <div className="flex items-center gap-1">
          <SnoozeMenu onSnooze={actions.onSnooze} />
          <Button
            type="button"
            variant="ghost"
            size="sm"
            onClick={actions.onDismiss}
            className="h-7 text-xs text-muted-foreground hover:text-foreground"
          >
            Dismiss
          </Button>
        </div>
      </div>
    </div>
  );
};
