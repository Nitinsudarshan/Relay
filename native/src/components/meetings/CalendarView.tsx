import React, { useMemo } from 'react';
import { Meeting, CalendarMeetingEvent } from '../../types';
import { Calendar, Clock, Video, Users } from 'lucide-react';
import { Badge } from '@/components/ui/badge';

interface CalendarViewProps {
  meetings: Meeting[];
  calendarEvents: CalendarMeetingEvent[];
  onSelectCalendarEvent: (evt: CalendarMeetingEvent) => void;
  onSelectMeeting: (m: Meeting) => void;
}

export const CalendarView: React.FC<CalendarViewProps> = ({
  meetings,
  calendarEvents,
  onSelectCalendarEvent,
  onSelectMeeting,
}) => {
  const upcomingEvents = useMemo(() => {
    return [...calendarEvents].sort((a, b) => {
      return new Date(a.scheduled_start).getTime() - new Date(b.scheduled_start).getTime();
    });
  }, [calendarEvents]);

  return (
    <div className="flex-1 flex flex-col items-center justify-center p-8 bg-card/30">
      <div className="max-w-2xl w-full space-y-6">
        <div className="text-center space-y-2">
          <Calendar className="w-12 h-12 text-muted-foreground/30 mx-auto" />
          <h2 className="text-lg font-semibold text-foreground">Upcoming Schedule</h2>
          <p className="text-sm text-muted-foreground">Select an upcoming meeting to start capturing notes, or create a new one.</p>
        </div>

        {upcomingEvents.length > 0 ? (
          <div className="space-y-3">
            {upcomingEvents.slice(0, 5).map((evt) => {
              const startDate = new Date(evt.scheduled_start);
              const formattedTime = startDate.toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' });
              const isToday = new Date().toDateString() === startDate.toDateString();
              
              return (
                <div
                  key={evt.id}
                  onClick={() => onSelectCalendarEvent(evt)}
                  className="p-4 rounded-xl bg-card border border-border/80 hover:border-primary/40 transition-all flex items-center justify-between gap-4 cursor-pointer shadow-xs"
                >
                  <div className="flex flex-col items-center justify-center min-w-[60px] bg-muted/40 rounded-lg p-2 border border-border/40">
                    <span className="text-[10px] font-bold uppercase tracking-wider text-muted-foreground">
                      {isToday ? 'TODAY' : startDate.toLocaleDateString(undefined, { weekday: 'short' })}
                    </span>
                    <span className="text-sm font-semibold text-foreground">{formattedTime}</span>
                  </div>
                  
                  <div className="flex-1 min-w-0">
                    <h4 className="text-sm font-bold text-foreground truncate">{evt.title}</h4>
                    <div className="flex items-center gap-3 text-xs text-muted-foreground mt-1">
                      {evt.provider && (
                        <span className="flex items-center gap-1">
                          <Video className="w-3.5 h-3.5" />
                          <span className="capitalize">{evt.provider.replace('_', ' ')}</span>
                        </span>
                      )}
                      {evt.participants.length > 0 && (
                        <span className="flex items-center gap-1">
                          <Users className="w-3.5 h-3.5" />
                          <span>{evt.participants.length} Participant{evt.participants.length !== 1 ? 's' : ''}</span>
                        </span>
                      )}
                    </div>
                  </div>
                </div>
              );
            })}
            
            {upcomingEvents.length > 5 && (
              <div className="text-center py-2 text-xs text-muted-foreground font-medium">
                + {upcomingEvents.length - 5} more upcoming events
              </div>
            )}
          </div>
        ) : (
          <div className="p-8 text-center border border-dashed border-border rounded-xl">
            <p className="text-sm text-muted-foreground">No upcoming meetings scheduled.</p>
          </div>
        )}
      </div>
    </div>
  );
};
