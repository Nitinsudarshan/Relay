import React, { useState, useEffect, useMemo, useCallback } from 'react';
import { invoke } from '@tauri-apps/api/core';
import {
  Meeting,
  MeetingSeries,
  Scribble,
  CalendarMeetingEvent,
  MeetingProvider,
  CalendarConnectionStatus,
  MeetingListItem,
} from '../../types';
import { MeetingDetailPane } from './MeetingDetailPane';
import { MeetingListRail } from './MeetingListRail';
import { MeetingModal } from './MeetingModal';
import { CalendarSyncModal } from './CalendarSyncModal';
import { useMeetingList } from './useMeetingList';

interface MeetingPageProps {
  onNavigateToScribbles?: (scribbleId?: string) => void;
  /** A meeting to select immediately, e.g. arriving from the reminder
   * popup's "Start Recording" or the tray (see App.tsx's
   * `switch-to-meetings-tab` listener) — this is what actually opens the
   * right meeting instead of just switching to this tab. */
  focusMeetingId?: string | null;
  onFocusMeetingIdConsumed?: () => void;
}

export const MeetingPage: React.FC<MeetingPageProps> = ({
  onNavigateToScribbles,
  focusMeetingId,
  onFocusMeetingIdConsumed,
}) => {
  const [selectedMeetingId, setSelectedMeetingId] = useState<string | null>(null);
  const [searchQuery, setSearchQuery] = useState('');
  
  const [scribbles, setScribbles] = useState<Scribble[]>([]);
  const [seriesList, setSeriesList] = useState<MeetingSeries[]>([]);
  const [calendarAuthStatus, setCalendarAuthStatus] = useState<CalendarConnectionStatus>({
    connected: false,
    status: 'disconnected',
  });
  
  // Modals
  const [isMeetingModalOpen, setIsMeetingModalOpen] = useState(false);
  const [isCalendarModalOpen, setIsCalendarModalOpen] = useState(false);

  const { groups, refresh, rawData } = useMeetingList();

  const loadExtraData = useCallback(async () => {
    try {
      const [loadedSeries, loadedScribbles, loadedAuth] = await Promise.all([
        invoke<MeetingSeries[]>('get_meeting_series'),
        invoke<Scribble[]>('get_scribbles'),
        invoke<CalendarConnectionStatus>('get_calendar_connection_status'),
      ]);
      setSeriesList(loadedSeries);
      setScribbles(loadedScribbles);
      setCalendarAuthStatus(loadedAuth);
    } catch (err) {
      console.error('Failed to load extra data', err);
    }
  }, []);

  useEffect(() => {
    loadExtraData();
  }, [loadExtraData]);

  useEffect(() => {
    if (focusMeetingId) {
      setSelectedMeetingId(focusMeetingId);
      onFocusMeetingIdConsumed?.();
    }
  }, [focusMeetingId, onFocusMeetingIdConsumed]);

  // If no meeting selected, auto-select first one from filtered list
  const filteredGroups = useMemo(() => {
    if (!searchQuery.trim()) return groups;
    const q = searchQuery.toLowerCase();
    
    return groups.map(g => ({
      ...g,
      items: g.items.filter(m => 
        m.title.toLowerCase().includes(q) || 
        m.provider.toLowerCase().includes(q)
      )
    })).filter(g => g.items.length > 0);
  }, [groups, searchQuery]);

  useEffect(() => {
    if (!selectedMeetingId) {
      for (const g of filteredGroups) {
        if (g.items.length > 0) {
          setSelectedMeetingId(g.items[0].id);
          break;
        }
      }
    }
  }, [filteredGroups, selectedMeetingId]);

  const selectedItem = useMemo(() => {
    for (const g of groups) {
      const found = g.items.find(i => i.id === selectedMeetingId);
      if (found) return found;
    }
    return null;
  }, [groups, selectedMeetingId]);

  const linkedScribbles = useMemo(() => {
    if (!selectedMeetingId) return [];
    return scribbles.filter(
      (s) =>
        s.source_type === 'meeting' &&
        s.source_metadata &&
        s.source_metadata.meeting_id === selectedMeetingId
    );
  }, [scribbles, selectedMeetingId]);

  // Actions
  const handleStartRecording = async (meetingId: string) => {
    try {
      await invoke('start_meeting_recording', { meetingId });
    } catch (err) {
      console.error('Failed to start recording', err);
    }
  };

  const handleStopRecording = async (meetingId: string) => {
    try {
      await invoke('stop_meeting_recording', { meetingId });
    } catch (err) {
      console.error('Failed to stop recording', err);
    }
  };

  const handleEnrichMeeting = async (meetingId: string) => {
    try {
      await invoke('trigger_enrich_meeting', { meetingId });
      refresh();
    } catch (err) {
      console.error('Failed to enrich', err);
    }
  };

  const handleSaveScribbleFromMeeting = async (content: string, title?: string, segment?: string) => {
    if (!selectedMeetingId) return;
    try {
      const createdScribble = await invoke<Scribble>('create_scribble_from_meeting', {
        meetingId: selectedMeetingId,
        content,
        title: title || null,
        segment: segment || null,
      });
      setScribbles(prev => [createdScribble, ...prev]);
    } catch (err) {
      console.error('Failed to create scribble', err);
    }
  };

  const handleUpdateMeeting = async (updated: Meeting) => {
    try {
      await invoke('update_meeting', { meeting: updated });
      refresh();
    } catch (err) {
      console.error('Failed to update', err);
    }
  };

  const handleDeleteMeeting = async (meetingId: string) => {
    try {
      await invoke('delete_meeting', { meetingId });
      setSelectedMeetingId(null);
      refresh();
    } catch (err) {
      console.error('Failed to delete', err);
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

      setSelectedMeetingId(newMeeting.id);
      refresh();
    } catch (err) {
      console.error('Failed to create', err);
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
      setSeriesList(prev => [newSeries, ...prev]);
    } catch (err) {
      console.error('Failed to save series', err);
    }
  };

  const handleImportCalendarEvent = async (event: CalendarMeetingEvent) => {
    try {
      // Goes through the same resolver the background engine uses, so
      // clicking this can't race the engine's own ~15s sync into creating
      // a duplicate meeting for the same calendar event.
      const meeting = await invoke<Meeting>('import_calendar_event', { event });
      setSelectedMeetingId(meeting.id);
      refresh();
    } catch (err) {
      console.error('Failed to import', err);
    }
  };

  // Calendar sync modals
  const handleConnectGoogle = async () => {
    const status = await invoke<CalendarConnectionStatus>('start_google_calendar_oauth');
    setCalendarAuthStatus(status);
    refresh();
  };
  const handleDisconnectGoogle = async () => {
    const status = await invoke<CalendarConnectionStatus>('disconnect_google_calendar');
    setCalendarAuthStatus(status);
    refresh();
  };
  const handleSyncGoogle = async () => {
    await invoke<CalendarMeetingEvent[]>('sync_google_calendar');
    refresh();
  };

  return (
    <div className="flex-1 flex gap-4 min-h-0 h-full overflow-hidden">
      <MeetingListRail 
        groups={filteredGroups}
        searchQuery={searchQuery}
        onSearchChange={setSearchQuery}
        selectedId={selectedMeetingId}
        onSelect={(item) => setSelectedMeetingId(item.id)}
        onNewMeeting={() => setIsMeetingModalOpen(true)}
        onCalendarSync={() => setIsCalendarModalOpen(true)}
        calendarConnected={calendarAuthStatus.connected}
      />
      
      <MeetingDetailPane
        item={selectedItem}
        rawMeetings={rawData.meetings}
        rawCalendarEvents={rawData.calendarEvents}
        onImportCalendarEvent={handleImportCalendarEvent}
        linkedScribbles={linkedScribbles}
        onStartRecording={handleStartRecording}
        onStopRecording={handleStopRecording}
        onEnrichMeeting={handleEnrichMeeting}
        onSaveScribbleFromMeeting={handleSaveScribbleFromMeeting}
        onUpdateMeeting={handleUpdateMeeting}
        onDeleteMeeting={handleDeleteMeeting}
        onNavigateToScribble={onNavigateToScribbles}
      />

      <MeetingModal
        isOpen={isMeetingModalOpen}
        onClose={() => setIsMeetingModalOpen(false)}
        onSaveMeeting={handleSaveNewMeeting}
        onSaveSeries={handleSaveNewSeries}
        existingSeries={seriesList}
      />

      <CalendarSyncModal
        isOpen={isCalendarModalOpen}
        onClose={() => setIsCalendarModalOpen(false)}
        authStatus={calendarAuthStatus}
        calendarEvents={rawData.calendarEvents}
        onConnectGoogle={handleConnectGoogle}
        onDisconnectGoogle={handleDisconnectGoogle}
        onSyncNow={handleSyncGoogle}
        onImportMeeting={handleImportCalendarEvent}
      />
    </div>
  );
};
