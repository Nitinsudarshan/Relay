import React, { useState, useEffect, useCallback, useMemo } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import {
  Meeting,
  MeetingSeries,
  Scribble,
  CalendarMeetingEvent,
  MeetingProvider,
  DetectedMeetingPayload,
  CalendarConnectionStatus,
} from '../../types';
import { MeetingDetailView } from './MeetingDetailView';
import { MeetingModal } from './MeetingModal';
import { CalendarSyncModal } from './CalendarSyncModal';
import { CalendarView } from './CalendarView';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { Badge } from '@/components/ui/badge';
import {
  Search,
  Plus,
  Calendar,
  Repeat,
  Video,
  Clock,
  Users,
  ChevronRight,
  ChevronDown,
  RefreshCw,
  Sparkles,
  Layers,
  CheckCircle2,
  Mic,
  CalendarDays,
} from 'lucide-react';

interface MeetingPageProps {
  onNavigateToScribbles?: (scribbleId?: string) => void;
}

type FilterType = 'all' | 'scheduled' | 'completed';

export const MeetingPage: React.FC<MeetingPageProps> = ({ onNavigateToScribbles }) => {
  const [meetings, setMeetings] = useState<Meeting[]>([]);
  const [seriesList, setSeriesList] = useState<MeetingSeries[]>([]);
  const [scribbles, setScribbles] = useState<Scribble[]>([]);
  const [calendarEvents, setCalendarEvents] = useState<CalendarMeetingEvent[]>([]);
  const [calendarAuthStatus, setCalendarAuthStatus] = useState<CalendarConnectionStatus>({
    connected: false,
    status: 'disconnected',
  });
  const [selectedMeetingId, setSelectedMeetingId] = useState<string | null>(null);
  const [searchQuery, setSearchQuery] = useState('');
  const [filterType, setFilterType] = useState<FilterType>('all');
  const [collapsedSeries, setCollapsedSeries] = useState<Set<string>>(new Set());
  const [recordingMeetingId, setRecordingMeetingId] = useState<string | null>(null);

  // Modals
  const [isMeetingModalOpen, setIsMeetingModalOpen] = useState(false);
  const [isCalendarModalOpen, setIsCalendarModalOpen] = useState(false);
  const [loading, setLoading] = useState(true);

  // Refresh all meeting data from backend
  const refreshData = useCallback(async () => {
    try {
      const [loadedMeetings, loadedSeries, loadedScribbles, loadedAuth] = await Promise.all([
        invoke<Meeting[]>('get_meetings'),
        invoke<MeetingSeries[]>('get_meeting_series'),
        invoke<Scribble[]>('get_scribbles'),
        invoke<CalendarConnectionStatus>('get_calendar_connection_status'),
      ]);

      setMeetings(loadedMeetings);
      setSeriesList(loadedSeries);
      setScribbles(loadedScribbles);
      setCalendarAuthStatus(loadedAuth);

      // Only fetch events if connected
      if (loadedAuth.connected && loadedAuth.status === 'connected') {
        try {
          const loadedCal = await invoke<CalendarMeetingEvent[]>('get_upcoming_calendar_events');
          setCalendarEvents(loadedCal);
        } catch (calErr) {
          console.warn('Could not sync calendar events:', calErr);
        }
      } else {
        setCalendarEvents([]);
      }

      // Select first meeting if none selected
      if (loadedMeetings.length > 0 && !selectedMeetingId) {
        setSelectedMeetingId(loadedMeetings[0].id);
      }
    } catch (err) {
      console.error('Failed to load meetings data:', err);
    } finally {
      setLoading(false);
    }
  }, [selectedMeetingId]);

  const handleConnectGoogle = async () => {
    const status = await invoke<CalendarConnectionStatus>('start_google_calendar_oauth');
    setCalendarAuthStatus(status);
    await refreshData();
  };

  const handleDisconnectGoogle = async () => {
    const status = await invoke<CalendarConnectionStatus>('disconnect_google_calendar');
    setCalendarAuthStatus(status);
    setCalendarEvents([]);
  };

  const handleSyncGoogle = async () => {
    const events = await invoke<CalendarMeetingEvent[]>('sync_google_calendar');
    setCalendarEvents(events);
    const status = await invoke<CalendarConnectionStatus>('get_calendar_connection_status');
    setCalendarAuthStatus(status);
  };

  useEffect(() => {
    refreshData();
  }, [refreshData]);

  // Listen to live backend meeting updates
  useEffect(() => {
    const unlistenUpdated = listen<Meeting>('meeting-updated', ({ payload }) => {
      setMeetings((prev) => {
        const index = prev.findIndex((m) => m.id === payload.id);
        if (index >= 0) {
          const next = [...prev];
          next[index] = payload;
          return next;
        }
        return [payload, ...prev];
      });
    });

    const unlistenCapture = listen<{ active: boolean; mode?: string }>('capture-state-changed', ({ payload }) => {
      if (!payload.active) {
        setRecordingMeetingId(null);
      }
    });

    return () => {
      unlistenUpdated.then((fn) => fn());
      unlistenCapture.then((fn) => fn());
    };
  }, []);

  // Periodic active window meeting detection check
  useEffect(() => {
    const interval = setInterval(async () => {
      try {
        await invoke<DetectedMeetingPayload[]>('check_meeting_detection');
      } catch {
        // Silently ignore background poll errors
      }
    }, 15000);
    return () => clearInterval(interval);
  }, []);

  const handleStartRecording = async (meetingId: string) => {
    try {
      setRecordingMeetingId(meetingId);
      await invoke('start_meeting_recording', { meetingId });
    } catch (err) {
      console.error('Failed to start meeting recording:', err);
      setRecordingMeetingId(null);
    }
  };

  const handleStopRecording = async (meetingId: string) => {
    try {
      await invoke('stop_meeting_recording', { meetingId });
      setRecordingMeetingId(null);
    } catch (err) {
      console.error('Failed to stop meeting recording:', err);
    }
  };

  const handleEnrichMeeting = async (meetingId: string) => {
    try {
      const enriched = await invoke<Meeting>('trigger_enrich_meeting', { meetingId });
      setMeetings((prev) => prev.map((m) => (m.id === enriched.id ? enriched : m)));
    } catch (err) {
      console.error('Failed to enrich meeting:', err);
    }
  };

  const handleSaveScribbleFromMeeting = async (
    content: string,
    title?: string,
    segment?: string
  ) => {
    if (!selectedMeetingId) return;
    try {
      const createdScribble = await invoke<Scribble>('create_scribble_from_meeting', {
        meetingId: selectedMeetingId,
        content,
        title: title || null,
        segment: segment || null,
      });

      setScribbles((prev) => [createdScribble, ...prev]);
    } catch (err) {
      console.error('Failed to create scribble from meeting:', err);
    }
  };

  const handleUpdateMeeting = async (updated: Meeting) => {
    try {
      await invoke('update_meeting', { meeting: updated });
      setMeetings((prev) => prev.map((m) => (m.id === updated.id ? updated : m)));
    } catch (err) {
      console.error('Failed to update meeting:', err);
    }
  };

  const handleDeleteMeeting = async (meetingId: string) => {
    try {
      await invoke('delete_meeting', { meetingId });
      setMeetings((prev) => prev.filter((m) => m.id !== meetingId));
      if (selectedMeetingId === meetingId) {
        const remaining = meetings.filter((m) => m.id !== meetingId);
        setSelectedMeetingId(remaining.length > 0 ? remaining[0].id : null);
      }
    } catch (err) {
      console.error('Failed to delete meeting:', err);
    }
  };

  const handleSaveNewMeeting = async (data: {
    title: string;
    provider: MeetingProvider;
    series_id?: string | null;
    scheduled_start?: string;
    participants: string[];
  }) => {
    try {
      const newMeeting = await invoke<Meeting>('create_meeting', {
        title: data.title,
        provider: data.provider,
        seriesId: data.series_id || null,
      });

      if (data.scheduled_start || data.participants.length > 0) {
        newMeeting.scheduled_start = data.scheduled_start || newMeeting.scheduled_start;
        newMeeting.participants = data.participants;
        await invoke('save_meeting', { meeting: newMeeting });
      }

      setMeetings((prev) => [newMeeting, ...prev]);
      setSelectedMeetingId(newMeeting.id);
    } catch (err) {
      console.error('Failed to create meeting:', err);
    }
  };

  const handleSaveNewSeries = async (data: {
    title: string;
    provider?: string;
    recurrence_rule?: string;
  }) => {
    try {
      const newSeries: MeetingSeries = {
        id: `series_${Date.now()}`,
        title: data.title,
        provider: data.provider || null,
        calendar_series_id: null,
        recurrence_rule: data.recurrence_rule || null,
        created_at: new Date().toISOString(),
        updated_at: new Date().toISOString(),
      };

      await invoke('save_meeting_series', { series: newSeries });
      setSeriesList((prev) => [newSeries, ...prev]);
    } catch (err) {
      console.error('Failed to create series:', err);
    }
  };

  const handleImportCalendarEvent = async (event: CalendarMeetingEvent) => {
    try {
      const newMeeting = await invoke<Meeting>('create_meeting', {
        title: event.title,
        provider: event.provider,
        seriesId: event.calendar_series_id || null,
      });

      newMeeting.calendar_event_id = event.id;
      newMeeting.scheduled_start = event.scheduled_start;
      newMeeting.scheduled_end = event.scheduled_end;
      newMeeting.participants = event.participants;
      newMeeting.provider_metadata = { meeting_url: event.meeting_url };

      await invoke('save_meeting', { meeting: newMeeting });
      setMeetings((prev) => [newMeeting, ...prev]);
      setSelectedMeetingId(newMeeting.id);
    } catch (err) {
      console.error('Failed to import calendar meeting:', err);
    }
  };

  const toggleSeriesCollapse = (seriesId: string) => {
    setCollapsedSeries((prev) => {
      const next = new Set(prev);
      if (next.has(seriesId)) next.delete(seriesId);
      else next.add(seriesId);
      return next;
    });
  };

  // Groupings & Filtered lists
  const filteredMeetings = useMemo(() => {
    let list = meetings;
    if (searchQuery.trim()) {
      const q = searchQuery.toLowerCase();
      list = list.filter(
        (m) =>
          m.title.toLowerCase().includes(q) ||
          m.notes.toLowerCase().includes(q) ||
          m.transcript.toLowerCase().includes(q) ||
          m.participants.some((p) => p.toLowerCase().includes(q))
      );
    }
    return list;
  }, [meetings, searchQuery]);

  const standaloneMeetings = useMemo(() => {
    return filteredMeetings.filter((m) => !m.series_id);
  }, [filteredMeetings]);

  const seriesGroupedMeetings = useMemo(() => {
    const map = new Map<string, Meeting[]>();
    for (const m of filteredMeetings) {
      if (m.series_id) {
        if (!map.has(m.series_id)) map.set(m.series_id, []);
        map.get(m.series_id)!.push(m);
      }
    }
    // Sort each series occurrences newest first
    for (const [_, list] of map.entries()) {
      list.sort((a, b) => {
        const aDate = new Date(a.scheduled_start || a.created_at).getTime();
        const bDate = new Date(b.scheduled_start || b.created_at).getTime();
        return bDate - aDate;
      });
    }
    return map;
  }, [filteredMeetings]);

  const selectedMeeting = useMemo(() => {
    return meetings.find((m) => m.id === selectedMeetingId) || null;
  }, [meetings, selectedMeetingId]);

  const linkedScribbles = useMemo(() => {
    if (!selectedMeetingId) return [];
    return scribbles.filter(
      (s) =>
        s.source_type === 'meeting' &&
        s.source_metadata &&
        s.source_metadata.meeting_id === selectedMeetingId
    );
  }, [scribbles, selectedMeetingId]);

  return (
    <div className="flex-1 flex flex-col h-full overflow-hidden space-y-4">
      {/* Top Search & Filter Bar */}
      <div className="flex items-center justify-between gap-3 flex-wrap shrink-0">
        <div className="flex items-center gap-2 flex-1 max-w-md">
          <div className="relative flex-1">
            <Search className="w-4 h-4 text-muted-foreground absolute left-3 top-1/2 -translate-y-1/2" />
            <Input
              value={searchQuery}
              onChange={(e) => setSearchQuery(e.target.value)}
              placeholder="Search meetings, transcripts, action items, participants…"
              className="pl-9 h-9 text-xs"
            />
          </div>
        </div>

        {/* Filter Pills */}
        <div className="flex items-center gap-1.5 bg-muted/40 p-1 rounded-lg border border-border/60">
          <button
            type="button"
            onClick={() => setFilterType('all')}
            className={`px-2.5 py-1 text-xs font-medium rounded-md transition-all ${
              filterType === 'all'
                ? 'bg-card text-foreground font-semibold shadow-xs'
                : 'text-muted-foreground hover:text-foreground'
            }`}
          >
            All ({meetings.length + calendarEvents.length})
          </button>
          <button
            type="button"
            onClick={() => setFilterType('scheduled')}
            className={`px-2.5 py-1 text-xs font-medium rounded-md transition-all flex items-center gap-1 ${
              filterType === 'scheduled'
                ? 'bg-card text-foreground font-semibold shadow-xs'
                : 'text-muted-foreground hover:text-foreground'
            }`}
          >
            <Calendar className="w-3 h-3 text-blue-500" />
            <span>Scheduled ({calendarEvents.length})</span>
          </button>
          <button
            type="button"
            onClick={() => setFilterType('completed')}
            className={`px-2.5 py-1 text-xs font-medium rounded-md transition-all ${
              filterType === 'completed'
                ? 'bg-card text-foreground font-semibold shadow-xs'
                : 'text-muted-foreground hover:text-foreground'
            }`}
          >
            Completed/Recorded ({meetings.length})
          </button>
        </div>

        {/* Primary Action Buttons */}
        <div className="flex items-center gap-2">
          <Button
            variant="outline"
            size="sm"
            onClick={() => setIsCalendarModalOpen(true)}
            className="text-xs h-9 gap-1.5"
          >
            <CalendarDays className={`w-3.5 h-3.5 ${calendarAuthStatus.connected ? 'text-green-500' : 'text-blue-500'}`} />
            <span>{calendarAuthStatus.connected ? 'Calendar Connected' : 'Calendar Sync'}</span>
          </Button>

          <Button
            size="sm"
            onClick={() => setIsMeetingModalOpen(true)}
            className="text-xs h-9 gap-1.5 shadow-xs"
          >
            <Plus className="w-3.5 h-3.5" />
            <span>New Meeting</span>
          </Button>
        </div>
      </div>

      {/* Main Master-Detail Surface */}
      <div className="flex-1 flex gap-4 min-h-0 overflow-hidden">
        {/* Left Master Column (Meeting List & Recurring Series) */}
        <div className="w-80 md:w-96 flex flex-col shrink-0 bg-card rounded-xl border border-border overflow-hidden shadow-xs">
          <div className="p-3 border-b border-border/80 bg-muted/20 flex items-center justify-between">
            <span className="text-xs font-bold text-foreground uppercase tracking-wider font-mono">
              {filterType === 'scheduled' ? 'Scheduled Meetings' : filterType === 'completed' ? 'Completed Meetings' : 'All Meetings'}
            </span>
            <Badge variant="outline" className="text-[10px] font-mono py-0 px-1.5">
              {filterType === 'scheduled' ? calendarEvents.length : filterType === 'completed' ? filteredMeetings.length : calendarEvents.length + filteredMeetings.length}
            </Badge>
          </div>
          <div className="flex-1 overflow-y-auto p-3 space-y-3">
            {(() => {
              // Create combined list
              let displayList: Array<CalendarMeetingEvent | Meeting> = [];
              if (filterType === 'all') {
                displayList = [...calendarEvents, ...filteredMeetings].sort((a, b) => {
                  const aDate = new Date('scheduled_start' in a && typeof a.scheduled_start === 'string' && a.scheduled_start.includes('T') ? a.scheduled_start : ('created_at' in a ? a.created_at : 0)).getTime();
                  const bDate = new Date('scheduled_start' in b && typeof b.scheduled_start === 'string' && b.scheduled_start.includes('T') ? b.scheduled_start : ('created_at' in b ? b.created_at : 0)).getTime();
                  return bDate - aDate;
                });
              } else if (filterType === 'scheduled') {
                displayList = [...calendarEvents].sort((a, b) => {
                  return new Date(a.scheduled_start).getTime() - new Date(b.scheduled_start).getTime();
                });
              } else {
                displayList = [...filteredMeetings].sort((a, b) => {
                  const aDate = new Date(a.scheduled_start || a.created_at).getTime();
                  const bDate = new Date(b.scheduled_start || b.created_at).getTime();
                  return bDate - aDate;
                });
              }

              if (displayList.length === 0) {
                return (
                  <div className="text-center py-12 text-xs text-muted-foreground space-y-2">
                    <Calendar className="w-8 h-8 text-muted-foreground/30 mx-auto" />
                    <p>No meetings found for this filter.</p>
                  </div>
                );
              }

              return displayList.map((item) => {
                const isCalendarEvent = 'scheduled_start' in item && 'provider' in item && !('created_at' in item);
                
                if (isCalendarEvent) {
                  const evt = item as CalendarMeetingEvent;
                  const sTime = new Date(evt.scheduled_start).toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' });
                  return (
                    <div key={evt.id} className="p-3 rounded-lg border border-border hover:border-primary/50 transition-all bg-card space-y-2 shadow-xs">
                      <div className="flex items-center justify-between">
                        <Badge variant="outline" className="text-[9px] uppercase font-mono text-primary border-primary/30 py-0 px-1.5">
                          {evt.provider.replace('_', ' ')}
                        </Badge>
                        <span className="text-[10px] text-muted-foreground font-mono">{sTime}</span>
                      </div>
                      <h4 className="text-xs font-bold text-foreground line-clamp-1">{evt.title}</h4>
                      <Button size="sm" variant="outline" onClick={() => handleImportCalendarEvent(evt)} className="w-full text-xs h-7 gap-1 text-primary border-primary/30">
                        <Plus className="w-3 h-3" />
                        <span>Start Capturing</span>
                      </Button>
                    </div>
                  );
                } else {
                  const m = item as Meeting;
                  const mDate = new Date(m.scheduled_start || m.created_at);
                  const mFormatted = mDate.toLocaleDateString(undefined, { month: 'short', day: 'numeric' });
                  const isSelected = m.id === selectedMeetingId;
                  
                  return (
                    <div
                      key={m.id}
                      onClick={() => setSelectedMeetingId(m.id)}
                      className={`p-3 rounded-lg cursor-pointer transition-all flex flex-col gap-1 border shadow-xs ${
                        isSelected
                          ? 'bg-primary/10 border-primary/40'
                          : 'bg-card border-border/40 hover:border-border hover:bg-muted/20'
                      }`}
                    >
                      <div className="flex items-center justify-between">
                        <Badge variant={m.series_id ? 'secondary' : 'outline'} className="text-[9px] uppercase font-mono py-0 px-1.5 border-border">
                          {m.provider.replace('_', ' ')}
                        </Badge>
                        <span className="text-[10px] text-muted-foreground font-mono">{mFormatted}</span>
                      </div>
                      <h4 className={`text-xs font-bold line-clamp-1 ${isSelected ? 'text-foreground font-extrabold' : 'text-foreground'}`}>
                        {m.title}
                      </h4>
                      <div className="flex items-center justify-between text-[10px] text-muted-foreground pt-1">
                        <span className="capitalize">{m.status}</span>
                        {m.action_items && m.action_items.length > 0 && <span>{m.action_items.length} tasks</span>}
                      </div>
                    </div>
                  );
                }
              });
            })()}
          </div>
        </div>

        {/* Right Detail Column */}
        <div className="flex-1 flex flex-col min-w-0 h-full overflow-hidden">
          {selectedMeeting ? (
            <MeetingDetailView
              meeting={selectedMeeting}
              linkedScribbles={linkedScribbles}
              isRecordingThisMeeting={recordingMeetingId === selectedMeeting.id}
              onStartRecording={handleStartRecording}
              onStopRecording={handleStopRecording}
              onEnrichMeeting={handleEnrichMeeting}
              onSaveScribbleFromMeeting={handleSaveScribbleFromMeeting}
              onUpdateMeeting={handleUpdateMeeting}
              onDeleteMeeting={handleDeleteMeeting}
              onNavigateToScribble={(scId) => onNavigateToScribbles?.(scId)}
            />
          ) : (
            <CalendarView 
              meetings={meetings} 
              calendarEvents={calendarEvents}
              onSelectCalendarEvent={handleImportCalendarEvent}
              onSelectMeeting={(m) => setSelectedMeetingId(m.id)}
            />
          )}
        </div>
      </div>

      {/* New Meeting / Series Modal */}
      <MeetingModal
        isOpen={isMeetingModalOpen}
        onClose={() => setIsMeetingModalOpen(false)}
        onSaveMeeting={handleSaveNewMeeting}
        onSaveSeries={handleSaveNewSeries}
        existingSeries={seriesList}
      />

      {/* Calendar Sync Modal */}
      <CalendarSyncModal
        isOpen={isCalendarModalOpen}
        onClose={() => setIsCalendarModalOpen(false)}
        authStatus={calendarAuthStatus}
        calendarEvents={calendarEvents}
        onConnectGoogle={handleConnectGoogle}
        onDisconnectGoogle={handleDisconnectGoogle}
        onSyncNow={handleSyncGoogle}
        onImportMeeting={handleImportCalendarEvent}
      />
    </div>
  );
};
