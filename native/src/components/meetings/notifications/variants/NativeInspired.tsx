import React from 'react';
import { Bell, Play, Square, RotateCcw } from 'lucide-react';
import { Button } from '@/components/ui/button';
import { VariantProps } from '../notification-types';
import { SnoozeMenu } from './SnoozeMenu';

export const NativeInspired: React.FC<VariantProps> = ({
  data,
  actions,
  isRecording,
  isDismissed,
  snoozedMinutes,
}) => {
  if (isDismissed) {
    return (
      <div className="w-full max-w-[400px] min-h-[110px] rounded-md border border-dashed border-border bg-muted/20 p-4 flex flex-col items-center justify-center gap-2 text-center text-xs text-muted-foreground">
        <span>Native-Inspired Notification Dismissed</span>
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
    <div className="w-full max-w-[400px] rounded-md border border-border bg-card p-3 shadow-xs select-none flex flex-col justify-between space-y-2.5">
      {/* Windows Toast Style App Header */}
      <div className="flex items-center justify-between text-[11px]">
        <div className="flex items-center gap-1.5 font-medium text-muted-foreground">
          <Bell className="w-3 h-3 text-primary shrink-0" />
          <span>Relay</span>
        </div>
        <span className="text-[10px] text-muted-foreground font-mono">
          {snoozedMinutes ? `Snoozed ${snoozedMinutes}m` : 'Just now'}
        </span>
      </div>

      {/* Title & Body Copy */}
      <div className="space-y-0.5">
        <h4 className="text-xs font-bold text-foreground truncate">{data.title}</h4>
        <p className="text-[11px] text-muted-foreground">
          {data.timeLabel} • {data.provider}
        </p>
      </div>

      {/* Windows Toast Native Button Row */}
      <div className="flex items-center gap-1.5 pt-1">
        <Button
          type="button"
          size="sm"
          variant={isRecording ? 'destructive' : 'default'}
          onClick={actions.onRecord}
          className="h-7 text-xs flex-1 font-medium gap-1 rounded-sm"
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

        <SnoozeMenu
          onSnooze={actions.onSnooze}
          variant="outline"
          size="sm"
          className="h-7 text-xs flex-1 justify-center rounded-sm"
        />

        <Button
          type="button"
          variant="outline"
          size="sm"
          onClick={actions.onDismiss}
          className="h-7 text-xs flex-1 text-muted-foreground hover:text-foreground font-medium rounded-sm"
        >
          Dismiss
        </Button>
      </div>
    </div>
  );
};
