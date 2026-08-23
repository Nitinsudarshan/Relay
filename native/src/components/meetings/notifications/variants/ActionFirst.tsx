import React from 'react';
import { Play, Square, RotateCcw } from 'lucide-react';
import { Button } from '@/components/ui/button';
import { VariantProps } from '../notification-types';
import { SnoozeMenu } from './SnoozeMenu';

export const ActionFirst: React.FC<VariantProps> = ({
  data,
  actions,
  isRecording,
  isDismissed,
  snoozedMinutes,
}) => {
  if (isDismissed) {
    return (
      <div className="w-full max-w-[400px] min-h-[120px] rounded-lg border border-dashed border-border bg-muted/20 p-4 flex flex-col items-center justify-center gap-2 text-center text-xs text-muted-foreground">
        <span>Action-First Notification Dismissed</span>
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
    <div className="w-full max-w-[400px] rounded-lg border border-border bg-card p-3 shadow-sm select-none flex flex-col justify-between space-y-2.5">
      {/* Compact Header: Title + Provider inline */}
      <div className="flex items-center justify-between">
        <span className="text-[11px] font-bold uppercase tracking-wider text-muted-foreground">
          Relay Alert
        </span>
        {snoozedMinutes && (
          <span className="text-[10px] text-amber-500 font-mono">Snoozed {snoozedMinutes}m</span>
        )}
      </div>

      <div className="space-y-0.5">
        <h4 className="text-xs font-bold text-foreground truncate">{data.title}</h4>
        <p className="text-[11px] text-muted-foreground truncate">
          {data.timeLabel} • {data.provider}
        </p>
      </div>

      {/* Dominant Action Row */}
      <div className="grid grid-cols-3 gap-2 pt-1 border-t border-border/40">
        <Button
          type="button"
          size="sm"
          variant={isRecording ? 'destructive' : 'default'}
          onClick={actions.onRecord}
          className="h-8 text-xs font-bold gap-1 rounded-md"
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
          className="h-8 text-xs font-semibold justify-center"
        />

        <Button
          type="button"
          variant="secondary"
          size="sm"
          onClick={actions.onDismiss}
          className="h-8 text-xs text-muted-foreground hover:text-foreground font-semibold"
        >
          Dismiss
        </Button>
      </div>
    </div>
  );
};
