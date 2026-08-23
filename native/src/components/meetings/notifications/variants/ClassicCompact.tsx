import React from 'react';
import { Calendar, Play, X, Square, RotateCcw } from 'lucide-react';
import { Button } from '@/components/ui/button';
import { Badge } from '@/components/ui/badge';
import { VariantProps } from '../notification-types';
import { SnoozeMenu } from './SnoozeMenu';

export const ClassicCompact: React.FC<VariantProps> = ({
  data,
  actions,
  isRecording,
  isDismissed,
  snoozedMinutes,
}) => {
  if (isDismissed) {
    return (
      <div className="w-full max-w-[400px] min-h-[120px] rounded-lg border border-dashed border-border bg-muted/20 p-4 flex flex-col items-center justify-center gap-2 text-center text-xs text-muted-foreground">
        <span>Notification Dismissed (Simulated)</span>
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

  return (
    <div className="w-full max-w-[400px] rounded-lg border border-border bg-card p-4 shadow-sm select-none flex flex-col justify-between space-y-3">
      {/* Top Header Row */}
      <div className="flex items-center justify-between">
        <span className="text-[11px] font-semibold text-muted-foreground uppercase tracking-wider">
          Relay
        </span>
        {snoozedMinutes ? (
          <Badge variant="outline" className="text-[10px] text-amber-500 border-amber-500/30">
            Snoozed {snoozedMinutes}m
          </Badge>
        ) : isRecording ? (
          <Badge className="text-[10px] bg-red-600 text-white animate-pulse gap-1">
            <span className="size-1.5 rounded-full bg-white" />
            RECORDING
          </Badge>
        ) : null}
      </div>

      {/* Meeting Details */}
      <div className="flex items-start gap-3">
        <div className="p-2 rounded-md bg-muted text-foreground shrink-0 mt-0.5">
          <Calendar className="w-4 h-4 text-primary" />
        </div>
        <div className="space-y-0.5 min-w-0 flex-1">
          <h4 className="text-sm font-bold text-foreground truncate">{data.title}</h4>
          <p className="text-xs text-muted-foreground font-medium">{data.timeLabel}</p>
          <p className="text-[11px] text-muted-foreground">{data.provider}</p>
        </div>
      </div>

      {/* Action Row */}
      <div className="flex items-center justify-between pt-1 border-t border-border/50">
        <Button
          type="button"
          size="sm"
          variant={isRecording ? 'destructive' : 'default'}
          onClick={actions.onRecord}
          className="h-7 text-xs font-semibold gap-1.5"
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
