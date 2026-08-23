import React from 'react';
import { Play, Square, RotateCcw } from 'lucide-react';
import { Button } from '@/components/ui/button';
import { Badge } from '@/components/ui/badge';
import { VariantProps } from '../notification-types';
import { SnoozeMenu } from './SnoozeMenu';

export const Executive: React.FC<VariantProps> = ({
  data,
  actions,
  isRecording,
  isDismissed,
  snoozedMinutes,
}) => {
  if (isDismissed) {
    return (
      <div className="w-full max-w-[400px] min-h-[120px] rounded-lg border border-dashed border-border bg-muted/20 p-4 flex flex-col items-center justify-center gap-2 text-center text-xs text-muted-foreground">
        <span>Executive Notification Dismissed</span>
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
    <div className="w-full max-w-[400px] rounded-lg border border-border/80 bg-gradient-to-b from-card to-card/90 p-4 shadow-md select-none flex flex-col justify-between space-y-3">
      {/* Top Brand Bar */}
      <div className="flex items-center justify-between">
        <div className="flex items-center gap-1.5">
          <span className="size-2 rounded-full bg-primary" />
          <span className="text-[11px] font-bold tracking-wider uppercase text-foreground/90 font-mono">
            RELAY EXECUTIVE
          </span>
        </div>
        {snoozedMinutes ? (
          <Badge variant="outline" className="text-[10px] text-amber-500 border-amber-500/40">
            Snoozed {snoozedMinutes}m
          </Badge>
        ) : isRecording ? (
          <Badge className="text-[10px] bg-red-600 text-white font-bold animate-pulse">
            LIVE RECORDING
          </Badge>
        ) : (
          <Badge variant="secondary" className="text-[10px] font-mono uppercase">
            {data.status}
          </Badge>
        )}
      </div>

      {/* Title & Metadata */}
      <div>
        <h3 className="text-base font-extrabold tracking-tight text-foreground line-clamp-1">
          {data.title}
        </h3>
        <div className="flex items-center gap-2 mt-1 text-xs text-muted-foreground">
          <span className="font-semibold text-primary">{data.timeLabel}</span>
          <span>•</span>
          <span>{data.provider}</span>
        </div>
      </div>

      {/* Executive Compact Action Bar */}
      <div className="flex items-center justify-between pt-2 border-t border-border/40">
        <Button
          type="button"
          size="sm"
          variant={isRecording ? 'destructive' : 'default'}
          onClick={actions.onRecord}
          className="h-7 text-xs px-3 font-bold gap-1.5 rounded-md"
        >
          {isRecording ? (
            <>
              <Square className="w-3 h-3 fill-current" /> Stop
            </>
          ) : (
            <>
              <Play className="w-3 h-3 fill-current" /> Start Record
            </>
          )}
        </Button>

        <div className="flex items-center gap-1">
          <SnoozeMenu onSnooze={actions.onSnooze} variant="ghost" className="h-7 text-xs" />
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
