import React from 'react';
import { Play, Square, RotateCcw, Video, Mic } from 'lucide-react';
import { Button } from '@/components/ui/button';
import { Badge } from '@/components/ui/badge';
import { VariantProps } from '../notification-types';
import { SnoozeMenu } from './SnoozeMenu';

export const FloatingCard: React.FC<VariantProps> = ({
  data,
  actions,
  isRecording,
  isDismissed,
  snoozedMinutes,
}) => {
  if (isDismissed) {
    return (
      <div className="w-full max-w-[400px] min-h-[120px] rounded-xl border border-dashed border-border bg-muted/20 p-4 flex flex-col items-center justify-center gap-2 text-center text-xs text-muted-foreground">
        <span>Floating Card Dismissed</span>
        <Button
          type="button"
          variant="outline"
          size="sm"
          onClick={actions.onDismiss}
          className="h-7 text-xs gap-1.5 rounded-full"
        >
          <RotateCcw className="w-3 h-3" /> Restore Preview
        </Button>
      </div>
    );
  }

  return (
    <div className="w-full max-w-[400px] rounded-xl border border-border/60 bg-card p-4 shadow-lg hover:shadow-xl transition-all select-none flex flex-col justify-between space-y-3">
      {/* Top Bar with Provider Avatar Icon */}
      <div className="flex items-center justify-between">
        <div className="flex items-center gap-2">
          <div className="size-7 rounded-lg bg-primary/10 text-primary flex items-center justify-center border border-primary/20">
            {data.provider.includes('Meet') || data.provider.includes('Zoom') ? (
              <Video className="w-3.5 h-3.5" />
            ) : (
              <Mic className="w-3.5 h-3.5" />
            )}
          </div>
          <div>
            <span className="text-xs font-bold text-foreground">{data.provider}</span>
            <span className="text-[10px] text-muted-foreground block">{data.timeLabel}</span>
          </div>
        </div>

        {snoozedMinutes ? (
          <Badge variant="outline" className="text-[10px] text-amber-500 rounded-full">
            Snoozed {snoozedMinutes}m
          </Badge>
        ) : isRecording ? (
          <Badge className="text-[10px] bg-red-600 text-white rounded-full animate-pulse">
            Recording
          </Badge>
        ) : (
          <Badge variant="secondary" className="text-[10px] rounded-full">
            Relay Assistant
          </Badge>
        )}
      </div>

      {/* Title */}
      <div>
        <h4 className="text-sm font-semibold text-foreground line-clamp-1">{data.title}</h4>
      </div>

      {/* Floating Pill Action Footer */}
      <div className="flex items-center justify-between pt-1">
        <Button
          type="button"
          size="sm"
          variant={isRecording ? 'destructive' : 'default'}
          onClick={actions.onRecord}
          className="h-7 text-xs rounded-full px-3 font-semibold gap-1.5"
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
          <SnoozeMenu onSnooze={actions.onSnooze} variant="secondary" className="h-7 text-xs rounded-full" />
          <Button
            type="button"
            variant="ghost"
            size="sm"
            onClick={actions.onDismiss}
            className="h-7 text-xs text-muted-foreground hover:text-foreground rounded-full"
          >
            Dismiss
          </Button>
        </div>
      </div>
    </div>
  );
};
