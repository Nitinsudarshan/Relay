import React from 'react';
import { MeetingStatus, MeetingProvider } from './notification-types';
import { Button } from '@/components/ui/button';
import { RotateCcw, Sliders } from 'lucide-react';

interface ControlsProps {
  status: MeetingStatus;
  setStatus: (status: MeetingStatus) => void;
  provider: MeetingProvider;
  setProvider: (provider: MeetingProvider) => void;
  onResetAll: () => void;
}

export const MeetingNotificationControls: React.FC<ControlsProps> = ({
  status,
  setStatus,
  provider,
  setProvider,
  onResetAll,
}) => {
  const statuses: { label: string; value: MeetingStatus }[] = [
    { label: 'Upcoming', value: 'upcoming' },
    { label: 'Detected', value: 'detected' },
    { label: 'In Progress', value: 'in-progress' },
  ];

  const providers: MeetingProvider[] = [
    'Google Meet',
    'Zoom',
    'Teams',
    'Webex',
    'In Person',
  ];

  return (
    <div className="rounded-lg border border-border bg-card p-4 shadow-sm space-y-3">
      <div className="flex items-center justify-between">
        <div className="flex items-center gap-2 text-xs font-bold text-foreground">
          <Sliders className="w-4 h-4 text-primary" />
          <span>Interactive Compare Controls</span>
        </div>
        <Button
          type="button"
          variant="outline"
          size="sm"
          onClick={onResetAll}
          className="h-7 text-xs gap-1.5"
        >
          <RotateCcw className="w-3 h-3" /> Reset all
        </Button>
      </div>

      <div className="grid grid-cols-1 md:grid-cols-2 gap-4 text-xs">
        {/* Meeting State Switcher */}
        <div>
          <label className="text-[11px] font-semibold text-muted-foreground block mb-1.5">
            Meeting State
          </label>
          <div className="flex items-center gap-1.5 bg-muted/40 p-1 rounded-md border border-border">
            {statuses.map((st) => (
              <button
                key={st.value}
                type="button"
                onClick={() => setStatus(st.value)}
                className={`flex-1 h-7 rounded text-xs font-medium transition-all ${
                  status === st.value
                    ? 'bg-card text-foreground font-bold shadow-xs border border-border/60'
                    : 'text-muted-foreground hover:text-foreground'
                }`}
              >
                {st.label}
              </button>
            ))}
          </div>
        </div>

        {/* Meeting Provider Selector */}
        <div>
          <label className="text-[11px] font-semibold text-muted-foreground block mb-1.5">
            Meeting Provider
          </label>
          <select
            value={provider}
            onChange={(e) => setProvider(e.target.value as MeetingProvider)}
            className="w-full h-9 px-3 rounded-md border border-border bg-card text-foreground text-xs focus:outline-none focus:ring-1 focus:ring-primary"
          >
            {providers.map((prov) => (
              <option key={prov} value={prov}>
                {prov}
              </option>
            ))}
          </select>
        </div>
      </div>
    </div>
  );
};
