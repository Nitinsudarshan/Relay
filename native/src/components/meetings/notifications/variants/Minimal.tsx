import React from 'react';
import { Calendar, RotateCcw } from 'lucide-react';
import { Button } from '@/components/ui/button';
import { VariantProps } from '../notification-types';
import { SnoozeMenu } from './SnoozeMenu';

export const Minimal: React.FC<VariantProps> = ({
  data,
  actions,
  isRecording,
  isDismissed,
  snoozedMinutes,
}) => {
  if (isDismissed) {
    return (
      <div className="w-full max-w-[400px] min-h-[100px] rounded border border-dashed border-border bg-muted/20 p-3 flex flex-col items-center justify-center gap-1.5 text-center text-xs text-muted-foreground">
        <span>Minimal Notification Dismissed</span>
        <Button
          type="button"
          variant="outline"
          size="sm"
          onClick={actions.onDismiss}
          className="h-6 text-[11px] gap-1"
        >
          <RotateCcw className="w-3 h-3" /> Restore Preview
        </Button>
      </div>
    );
  }

  return (
    <div className="w-full max-w-[400px] rounded border border-border/80 bg-card p-3 shadow-xs select-none flex flex-col justify-between space-y-2">
      {/* Minimal Header */}
      <div className="flex items-center gap-2">
        <Calendar className="w-3.5 h-3.5 text-muted-foreground shrink-0" />
        <h4 className="text-xs font-bold text-foreground truncate flex-1">{data.title}</h4>
        <span className="text-[10px] text-muted-foreground font-mono shrink-0">
          {data.timeLabel} • {data.provider}
        </span>
      </div>

      {/* Subtle Controls */}
      <div className="flex items-center justify-end gap-2 pt-1">
        <button
          type="button"
          onClick={actions.onRecord}
          className={`text-xs font-bold transition-colors ${
            isRecording
              ? 'text-red-500 hover:text-red-400'
              : 'text-primary hover:text-primary/80'
          }`}
        >
          {isRecording ? 'Stop Recording' : 'Record'}
        </button>

        <span className="text-border">•</span>

        <SnoozeMenu
          onSnooze={actions.onSnooze}
          variant="ghost"
          size="sm"
          className="h-6 text-xs px-1 text-muted-foreground hover:text-foreground"
        />

        <span className="text-border">•</span>

        <button
          type="button"
          onClick={actions.onDismiss}
          className="text-xs text-muted-foreground hover:text-foreground transition-colors"
        >
          Dismiss
        </button>
      </div>
    </div>
  );
};
