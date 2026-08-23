import { useState, useEffect, useCallback, useRef } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { Meeting, CalendarMeetingEvent, MeetingListItem } from '../../types';

export interface MeetingGroup {
  label: string;
  items: MeetingListItem[];
}

export function useMeetingList() {
  const [groups, setGroups] = useState<MeetingGroup[]>([]);
  const [loading, setLoading] = useState(true);
  const [rawData, setRawData] = useState<{ meetings: Meeting[]; calendarEvents: CalendarMeetingEvent[] }>({
    meetings: [],
    calendarEvents: [],
  });

  const processMeetings = (meetings: Meeting[], calendarEvents: CalendarMeetingEvent[]) => {
    // Deduplicate: drop calendar events whose id matches a meeting's calendar_event_id
    const meetingCalIds = new Set(meetings.map(m => m.calendar_event_id).filter(Boolean));
    const dedupedCalendarEvents = calendarEvents.filter(e => !meetingCalIds.has(e.id));

    const list: MeetingListItem[] = [];

    // Project Meetings
    for (const m of meetings) {
      let durationMinutes = null;
      if (m.actual_start && m.actual_end) {
        const start = new Date(m.actual_start).getTime();
        const end = new Date(m.actual_end).getTime();
        durationMinutes = Math.round((end - start) / 60000);
      }
      
      list.push({
        id: m.id,
        sourceKind: 'meeting',
        title: m.title,
        startsAt: m.scheduled_start || m.created_at,
        endsAt: m.scheduled_end || null,
        provider: m.provider,
        participantCount: m.participants ? m.participants.length : 0,
        status: m.status,
        durationMinutes,
        calendarEventId: m.calendar_event_id || null,
      });
    }

    // Project Calendar Events
    for (const e of dedupedCalendarEvents) {
      list.push({
        id: e.id,
        sourceKind: 'calendar',
        title: e.title,
        startsAt: e.scheduled_start,
        endsAt: e.scheduled_end,
        provider: e.provider,
        participantCount: e.participants ? e.participants.length : 0,
        status: 'scheduled',
        durationMinutes: null,
        calendarEventId: e.id,
      });
    }

    // Sort newest first
    list.sort((a, b) => {
      const timeA = new Date(a.startsAt || 0).getTime();
      const timeB = new Date(b.startsAt || 0).getTime();
      return timeB - timeA;
    });

    // Grouping
    const now = new Date();
    const today = new Date(now.getFullYear(), now.getMonth(), now.getDate());
    const yesterday = new Date(today);
    yesterday.setDate(yesterday.getDate() - 1);
    
    // Start of week (Sunday)
    const thisWeekStart = new Date(today);
    thisWeekStart.setDate(thisWeekStart.getDate() - thisWeekStart.getDay());
    
    const groupMap = new Map<string, MeetingListItem[]>();

    const getGroupLabel = (dateStr: string | null) => {
      if (!dateStr) return 'Past';
      const d = new Date(dateStr);
      const dStart = new Date(d.getFullYear(), d.getMonth(), d.getDate());
      
      if (dStart.getTime() === today.getTime()) return 'Today';
      if (dStart.getTime() === yesterday.getTime()) return 'Yesterday';
      if (dStart.getTime() >= thisWeekStart.getTime() && dStart.getTime() < today.getTime()) return 'This week';
      
      // For future dates (e.g. scheduled)
      if (dStart.getTime() > today.getTime()) {
        const nextWeekStart = new Date(thisWeekStart);
        nextWeekStart.setDate(nextWeekStart.getDate() + 7);
        if (dStart.getTime() < nextWeekStart.getTime()) return 'This week';
        return d.toLocaleDateString(undefined, { month: 'long', year: 'numeric' });
      }

      return d.toLocaleDateString(undefined, { month: 'long', year: 'numeric' });
    };

    for (const item of list) {
      const label = getGroupLabel(item.startsAt);
      if (!groupMap.has(label)) {
        groupMap.set(label, []);
      }
      groupMap.get(label)!.push(item);
    }

    // Order of groups: Today, Yesterday, This week, then months (which should already be ordered because list is sorted newest first)
    const resultGroups: MeetingGroup[] = [];
    const priorityLabels = ['Today', 'Yesterday', 'This week'];
    
    for (const pl of priorityLabels) {
      if (groupMap.has(pl)) {
        resultGroups.push({ label: pl, items: groupMap.get(pl)! });
        groupMap.delete(pl);
      }
    }
    
    for (const [label, items] of groupMap.entries()) {
      resultGroups.push({ label, items });
    }

    setGroups(resultGroups);
  };

  const refresh = useCallback(async () => {
    setLoading(true);
    try {
      const loadedMeetings = await invoke<Meeting[]>('get_meetings');
      
      let loadedCal: CalendarMeetingEvent[] = [];
      const authStatus = await invoke<{ connected: boolean; status: string }>('get_calendar_connection_status');
      if (authStatus.connected && authStatus.status === 'connected') {
        try {
          loadedCal = await invoke<CalendarMeetingEvent[]>('get_upcoming_calendar_events');
        } catch (e) {
          console.warn('Failed to load calendar events', e);
        }
      }

      setRawData({ meetings: loadedMeetings, calendarEvents: loadedCal });
      processMeetings(loadedMeetings, loadedCal);
    } catch (err) {
      console.error('Failed to load meetings list:', err);
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    refresh();
  }, [refresh]);

  useEffect(() => {
    const unlisten = listen<Meeting>('meeting-updated', ({ payload }) => {
      setRawData((prev) => {
        const { meetings, calendarEvents } = prev;
        const newMeetings = [...meetings];
        const index = newMeetings.findIndex(m => m.id === payload.id);
        
        if (index >= 0) {
          newMeetings[index] = payload;
        } else {
          newMeetings.unshift(payload);
        }
        
        processMeetings(newMeetings, calendarEvents);
        return { meetings: newMeetings, calendarEvents };
      });
    });

    return () => {
      unlisten.then(fn => fn());
    };
  }, []);

  return { groups, loading, refresh, rawData };
}
