import React from 'react';
import { Play, Square, RotateCcw, X, AlertCircle } from 'lucide-react';
import { Button } from '@/components/ui/button';
import { Badge } from '@/components/ui/badge';
import { VariantProps } from '../notification-types';
import { SnoozeMenu } from './SnoozeMenu';

export const StatusFirst: React.FC<VariantProps> = ({
  data,
  actions,
  isRecording,
  isDismissed,
  snoozedMinutes,
}) => {
  if (isDismissed) {
    return (
      <div className="w-full max-w-[400px] min-h-[120px] rounded-lg border border-dashed border-border bg-muted/20 p-4 flex flex-col items-center justify-center gap-2 text-center text-xs text-muted-foreground">
        <span>Status-First Notification Dismissed</span>
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

  const getStatusColor = () => {
    switch (data.status) {
      case 'upcoming':
        return 'bg-amber-500/10 text-amber-500 border-amber-500/30';
      case 'detected':
        return 'bg-blue-500/10 text-blue-500 border-blue-500/30';
      case 'in-progress':
        return 'bg-emerald-500/10 text-emerald-500 border-emerald-500/30';
      default:
        return 'bg-muted text-muted-foreground';
    }
  };

  return (
    <div className="w-full max-w-[400px] rounded-lg border border-border bg-card p-4 shadow-sm select-none flex flex-col justify-between space-y-3">
      {/* Prominent Status Indicator Header */}
      <div className="flex items-center justify-between">
        <div className="flex items-center gap-2">
          <span className="size-2.5 rounded-full bg-emerald-500 animate-ping" />
          <Badge variant="outline" className={`text-xs font-bold uppercase tracking-wider ${getStatusColor()}`}>
            ● {data.status}
          </Badge>
        </div>

        <button
          type="button"
          onClick={actions.onDismiss}
          className="text-muted-foreground hover:text-foreground p-1 rounded-md transition-colors"
          aria-label="Dismiss notification"
        >
          <X className="w-4 h-4" />
        </button>
      </div>

      {/* Main Content */}
      <div className="space-y-1">
        <h4 className="text-base font-bold text-foreground line-clamp-1">{data.title}</h4>
        <div className="flex items-center gap-2 text-xs text-muted-foreground font-medium">
          <span className="text-foreground font-semibold">{data.timeLabel}</span>
          <span>•</span>
          <span>{data.provider}</span>
        </div>
      </div>

      {/* Action Row */}
      <div className="flex items-center justify-between pt-1 border-t border-border/50">
        <Button
          type="button"
          size="sm"
          variant={isRecording ? 'destructive' : 'default'}
          onClick={actions.onRecord}
          className="h-7 text-xs font-bold gap-1.5 rounded-md px-3"
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
          {snoozedMinutes && (
            <span className="text-[10px] text-amber-500 font-mono">({snoozedMinutes}m)</span>
          )}
        </div>
      </div>
    </div>
  );
};
