import React from 'react';
import { MeetingGroup } from './useMeetingList';
import { Input } from '../ui/input';
import { Button } from '../ui/button';
import { Search, Plus, CalendarDays, Calendar } from 'lucide-react';
import { Badge } from '../ui/badge';
import { MeetingListItem } from '../../types';

interface MeetingListRailProps {
  groups: MeetingGroup[];
  searchQuery: string;
  onSearchChange: (q: string) => void;
  selectedId: string | null;
  onSelect: (item: MeetingListItem) => void;
  onNewMeeting: () => void;
  onCalendarSync: () => void;
  calendarConnected: boolean;
}

export const MeetingListRail: React.FC<MeetingListRailProps> = ({
  groups,
  searchQuery,
  onSearchChange,
  selectedId,
  onSelect,
  onNewMeeting,
  onCalendarSync,
  calendarConnected,
}) => {
  return (
    <div className="w-80 md:w-96 flex flex-col shrink-0 bg-card rounded-xl border border-border overflow-hidden shadow-xs h-full">
      {/* Header / Search */}
      <div className="p-3 border-b border-border/80 flex flex-col gap-3">
        <div className="flex items-center justify-between gap-2">
          <Button
            variant="outline"
            size="sm"
            onClick={onCalendarSync}
            className="flex-1 text-xs h-8 gap-1.5"
          >
            <CalendarDays className={`w-3.5 h-3.5 ${calendarConnected ? 'text-green-500' : 'text-blue-500'}`} />
            <span>{calendarConnected ? 'Calendar' : 'Sync'}</span>
          </Button>

          <Button
            size="sm"
            onClick={onNewMeeting}
            className="flex-1 text-xs h-8 gap-1.5 shadow-xs"
          >
            <Plus className="w-3.5 h-3.5" />
            <span>New</span>
          </Button>
        </div>

        <div className="relative">
          <Search className="w-3.5 h-3.5 text-muted-foreground absolute left-2.5 top-1/2 -translate-y-1/2" />
          <Input
            value={searchQuery}
            onChange={(e) => onSearchChange(e.target.value)}
            placeholder="Search meetings..."
            className="pl-8 h-8 text-xs"
          />
        </div>
      </div>

      {/* List */}
      <div className="flex-1 overflow-y-auto p-2 space-y-4">
        {groups.length === 0 ? (
          <div className="text-center py-12 text-xs text-muted-foreground space-y-2">
            <Calendar className="w-8 h-8 text-muted-foreground/30 mx-auto" />
            <p>No meetings found.</p>
          </div>
        ) : (
          groups.map((g) => (
            <div key={g.label}>
              <h3 className="px-2 text-[10px] font-bold text-muted-foreground uppercase tracking-wider font-mono mb-2">
                {g.label}
              </h3>
              <div className="space-y-1">
                {g.items.map((m) => {
                  const isSelected = m.id === selectedId;
                  const time = new Date(m.startsAt || 0).toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' });
                  
                  return (
                    <button
                      key={m.id}
                      onClick={() => onSelect(m)}
                      className={`w-full text-left p-2 rounded-lg cursor-pointer transition-all flex flex-col gap-1 border shadow-none ${
                        isSelected
                          ? 'bg-primary/10 border-primary/40'
                          : 'bg-transparent border-transparent hover:bg-muted/50'
                      }`}
                    >
                      <div className="flex items-center justify-between">
                        <Badge variant={m.sourceKind === 'calendar' ? 'outline' : 'secondary'} className={`text-[9px] uppercase font-mono py-0 px-1.5 border-border ${m.sourceKind === 'calendar' ? 'text-blue-500 border-blue-500/30' : ''}`}>
                          {m.provider.replace('_', ' ')}
                        </Badge>
                        <span className="text-[10px] text-muted-foreground font-mono">{time}</span>
                      </div>
                      <h4 className={`text-xs font-bold line-clamp-1 ${isSelected ? 'text-foreground' : 'text-muted-foreground'}`}>
                        {m.title}
                      </h4>
                      {m.sourceKind === 'meeting' && m.durationMinutes && (
                        <div className="flex items-center text-[10px] text-muted-foreground pt-0.5">
                          <span className="capitalize">{m.status}</span>
                          <span className="mx-1.5">•</span>
                          <span>{m.durationMinutes} min</span>
                        </div>
                      )}
                    </button>
                  );
                })}
              </div>
            </div>
          ))
        )}
      </div>
    </div>
  );
};
