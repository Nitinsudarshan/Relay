import React from 'react';
import { MeetingListItem, Meeting, CalendarMeetingEvent } from '../../types';
import { MeetingDetailView } from './MeetingDetailView';
import { Button } from '../ui/button';
import { Calendar, Plus, Users, Link as LinkIcon, AlertCircle } from 'lucide-react';
import { Badge } from '../ui/badge';
import { useCaptureOwnership } from '../../hooks/useCaptureOwnership';

interface MeetingDetailPaneProps {
  item: MeetingListItem | null;
  rawMeetings: Meeting[];
  rawCalendarEvents: CalendarMeetingEvent[];
  onImportCalendarEvent: (evt: CalendarMeetingEvent) => void;
  // Props for MeetingDetailView
  linkedScribbles: any[];
  onStartRecording: (id: string) => Promise<void>;
  onStopRecording: (id: string) => Promise<void>;
  onEnrichMeeting: (id: string) => Promise<void>;
  onSaveScribbleFromMeeting: (content: string, title?: string, segment?: string) => Promise<void>;
  onUpdateMeeting: (m: Meeting) => Promise<void>;
  onDeleteMeeting: (id: string) => Promise<void>;
  onNavigateToScribble?: (id?: string) => void;
}

export const MeetingDetailPane: React.FC<MeetingDetailPaneProps> = ({
  item,
  rawMeetings,
  rawCalendarEvents,
  onImportCalendarEvent,
  linkedScribbles,
  onStartRecording,
  onStopRecording,
  onEnrichMeeting,
  onSaveScribbleFromMeeting,
  onUpdateMeeting,
  onDeleteMeeting,
  onNavigateToScribble,
}) => {
  const ownership = useCaptureOwnership();

  if (!item) {
    return (
      <div className="flex-1 flex items-center justify-center bg-card rounded-xl border border-border shadow-xs h-full text-muted-foreground text-sm">
        <div className="flex flex-col items-center space-y-3">
          <Calendar className="w-10 h-10 opacity-20" />
          <p>Select a meeting to view details</p>
        </div>
      </div>
    );
  }

  if (item.sourceKind === 'meeting') {
    const meeting = rawMeetings.find(m => m.id === item.id);
    if (!meeting) return null;
    
    // We override isRecordingThisMeeting by checking our global ownership hook
    const isRecordingThisMeeting = ownership.active && ownership.mode === 'meeting';
    // Wait, we need to know if THIS meeting is the one being recorded.
    // The previous implementation used recordingMeetingId, but the plan was:
    // "Remove recordingMeetingId from MeetingPage.tsx, relying on the new hook to deduce if the current meeting is being recorded or if the mic is used by another mode".
    // Actually the ownership hook only knows it's a meeting, but not WHICH meeting. 
    // Usually only the ACTIVE meeting has status="recording". Let's check status.
    const isThisMeetingRecording = meeting.status === 'recording' && ownership.ownedByMeeting;

    return (
      <div className="flex-1 flex flex-col min-w-0 h-full overflow-hidden relative">
        {ownership.ownedByOther && (
          <div className="absolute top-0 left-0 right-0 z-50 bg-amber-500/10 border-b border-amber-500/20 px-4 py-2 flex items-center justify-center gap-2">
            <AlertCircle className="w-4 h-4 text-amber-500" />
            <span className="text-xs font-medium text-amber-500">
              Microphone is currently in use by {ownership.mode}. You must stop it before recording this meeting.
            </span>
          </div>
        )}
        <MeetingDetailView
          meeting={meeting}
          linkedScribbles={linkedScribbles}
          isRecordingThisMeeting={isThisMeetingRecording}
          onStartRecording={onStartRecording}
          onStopRecording={onStopRecording}
          onEnrichMeeting={onEnrichMeeting}
          onSaveScribbleFromMeeting={onSaveScribbleFromMeeting}
          onUpdateMeeting={onUpdateMeeting}
          onDeleteMeeting={onDeleteMeeting}
          onNavigateToScribble={onNavigateToScribble}
          disableRecording={ownership.ownedByOther}
        />
      </div>
    );
  }

  // Calendar Event
  const calEvent = rawCalendarEvents.find(e => e.id === item.id);
  if (!calEvent) return null;

  const startDate = new Date(calEvent.scheduled_start);
  const endDate = new Date(calEvent.scheduled_end);
  const durationText = `${startDate.toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' })} - ${endDate.toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' })}`;

  return (
    <div className="flex-1 flex flex-col bg-card rounded-xl border border-border overflow-hidden shadow-xs h-full relative">
      {ownership.ownedByOther && (
        <div className="absolute top-0 left-0 right-0 z-50 bg-amber-500/10 border-b border-amber-500/20 px-4 py-2 flex items-center justify-center gap-2">
          <AlertCircle className="w-4 h-4 text-amber-500" />
          <span className="text-xs font-medium text-amber-500">
            Microphone is currently in use by {ownership.mode}.
          </span>
        </div>
      )}
      <div className="p-6 md:p-8 flex flex-col items-center justify-center h-full max-w-lg mx-auto text-center space-y-6">
        <div className="w-16 h-16 rounded-full bg-blue-500/10 flex items-center justify-center">
          <Calendar className="w-8 h-8 text-blue-500" />
        </div>
        
        <div className="space-y-2">
          <Badge variant="outline" className="uppercase font-mono text-primary border-primary/30 py-0.5 px-2">
            {calEvent.provider.replace('_', ' ')}
          </Badge>
          <h2 className="text-2xl font-extrabold text-foreground tracking-tight">{calEvent.title}</h2>
          <p className="text-sm text-muted-foreground font-mono">{startDate.toLocaleDateString(undefined, { weekday: 'long', month: 'long', day: 'numeric' })} • {durationText}</p>
        </div>

        <div className="flex flex-col gap-2 w-full pt-4 border-t border-border/50 text-left">
          {calEvent.participants.length > 0 && (
            <div className="flex items-center gap-3 text-sm text-muted-foreground">
              <Users className="w-4 h-4" />
              <span>{calEvent.participants.length} participants</span>
            </div>
          )}
          {calEvent.meeting_url && (
            <div className="flex items-center gap-3 text-sm text-muted-foreground">
              <LinkIcon className="w-4 h-4" />
              <a href={calEvent.meeting_url} target="_blank" rel="noreferrer" className="text-primary hover:underline truncate">
                {calEvent.meeting_url}
              </a>
            </div>
          )}
        </div>

        <Button 
          size="lg" 
          onClick={() => onImportCalendarEvent(calEvent)}
          className="w-full gap-2 mt-4"
          disabled={ownership.ownedByOther}
        >
          <Plus className="w-4 h-4" />
          <span>Import and Prepare Meeting</span>
        </Button>
      </div>
    </div>
  );
};
