import React, { useCallback, useEffect, useMemo, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import {
  Calendar,
  CalendarDays,
  Check,
  Clock,
  ExternalLink,
  MapPin,
  Pencil,
  Play,
  Plus,
  RefreshCw,
  Search,
  Sparkles,
  Trash2,
  Users,
  Video,
  X,
} from 'lucide-react';
import { Button } from '@/components/ui/button';
import { Badge } from '@/components/ui/badge';
import { EmptyState } from '@/components/common/EmptyState';
import { CalendarAccount, CalendarEvent } from '../../types';
import { formatRelativeTimeToMeeting, getMeetingJoinUrl } from './UpcomingMeetingModal';

export const CALENDAR_COLORS = [
  { name: 'Blue', hex: '#3b82f6' },
  { name: 'Emerald', hex: '#10b981' },
  { name: 'Purple', hex: '#8b5cf6' },
  { name: 'Amber', hex: '#f59e0b' },
  { name: 'Rose', hex: '#ef4444' },
  { name: 'Cyan', hex: '#06b6d4' },
  { name: 'Pink', hex: '#ec4899' },
  { name: 'Lime', hex: '#84cc16' },
];

interface CalendarViewModalProps {
  isOpen: boolean;
  onClose: () => void;
  onSelectEvent: (evt: CalendarEvent) => void;
  onStartRecording: (evt: CalendarEvent) => void;
  onJoinAndRecord: (evt: CalendarEvent) => void;
  isStarting?: boolean;
}

export const CalendarViewModal: React.FC<CalendarViewModalProps> = ({
  isOpen,
  onClose,
  onSelectEvent,
  onStartRecording,
  onJoinAndRecord,
  isStarting = false,
}) => {
  const [accounts, setAccounts] = useState<CalendarAccount[]>([]);
  const [events, setEvents] = useState<CalendarEvent[]>([]);
  const [isLoading, setIsLoading] = useState<boolean>(false);
  const [isSyncing, setIsSyncing] = useState<boolean>(false);
  const [searchQuery, setSearchQuery] = useState<string>('');
  const [activeFilter, setActiveFilter] = useState<'all' | 'today' | 'tomorrow' | 'week'>('all');

  // Add Calendar dialog state
  const [isAddingCalendar, setIsAddingCalendar] = useState<boolean>(false);
  const [newCalendarName, setNewCalendarName] = useState<string>('');
  const [newCalendarColor, setNewCalendarColor] = useState<string>(CALENDAR_COLORS[0].hex);
  const [isConnecting, setIsConnecting] = useState<boolean>(false);
  const [actionError, setActionError] = useState<string | null>(null);

  // Edit Calendar state
  const [editingAccountId, setEditingAccountId] = useState<string | null>(null);
  const [editName, setEditName] = useState<string>('');
  const [editColor, setEditColor] = useState<string>(CALENDAR_COLORS[0].hex);

  // Load accounts and events
  const loadData = useCallback(async () => {
    setIsLoading(true);
    setActionError(null);
    try {
      const accs = await invoke<CalendarAccount[]>('list_calendar_accounts');
      setAccounts(accs);
      const evts = await invoke<CalendarEvent[]>('get_upcoming_calendar_events', { days: 14 });
      setEvents(evts);
    } catch (err: unknown) {
      console.error('Failed to load calendar accounts/events:', err);
    } finally {
      setIsLoading(false);
    }
  }, []);

  useEffect(() => {
    if (isOpen) {
      void loadData();
    }
  }, [isOpen, loadData]);

  // Handle escape key
  useEffect(() => {
    if (!isOpen) return;
    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.key === 'Escape' && !isConnecting && !isStarting) {
        onClose();
      }
    };
    window.addEventListener('keydown', handleKeyDown);
    return () => window.removeEventListener('keydown', handleKeyDown);
  }, [isOpen, isConnecting, isStarting, onClose]);

  const handleSyncAll = async () => {
    if (isSyncing) return;
    setIsSyncing(true);
    setActionError(null);
    try {
      await invoke('sync_calendar_accounts', { id: null });
      await loadData();
    } catch (err: unknown) {
      console.error('Failed to sync calendars:', err);
      setActionError('Sync failed. Check your network or credentials.');
    } finally {
      setIsSyncing(false);
    }
  };

  const handleAddCalendar = async () => {
    if (!newCalendarName.trim() || isConnecting) return;
    setIsConnecting(true);
    setActionError(null);
    try {
      await invoke<CalendarAccount>('add_google_calendar_account', {
        name: newCalendarName.trim(),
        color: newCalendarColor,
      });
      setIsAddingCalendar(false);
      setNewCalendarName('');
      await loadData();
    } catch (err: unknown) {
      console.error('Failed to connect Google Calendar:', err);
      setActionError('Failed to connect Google account. Please try again.');
    } finally {
      setIsConnecting(false);
    }
  };

  const handleToggleAccount = async (account: CalendarAccount) => {
    try {
      const updated = await invoke<CalendarAccount>('update_calendar_account', {
        id: account.id,
        name: null,
        color: null,
        enabled: !account.enabled,
      });
      setAccounts((prev) => prev.map((a) => (a.id === updated.id ? updated : a)));
      const evts = await invoke<CalendarEvent[]>('get_upcoming_calendar_events', { days: 14 });
      setEvents(evts);
    } catch (err: unknown) {
      console.error('Failed to toggle calendar:', err);
      setActionError('Failed to update calendar status.');
    }
  };

  const handleSaveEdit = async (accountId: string) => {
    if (!editName.trim()) return;
    try {
      const updated = await invoke<CalendarAccount>('update_calendar_account', {
        id: accountId,
        name: editName.trim(),
        color: editColor,
        enabled: null,
      });
      setAccounts((prev) => prev.map((a) => (a.id === updated.id ? updated : a)));
      setEditingAccountId(null);
      const evts = await invoke<CalendarEvent[]>('get_upcoming_calendar_events', { days: 14 });
      setEvents(evts);
    } catch (err: unknown) {
      console.error('Failed to save calendar edit:', err);
      setActionError('Failed to save changes.');
    }
  };

  const handleDisconnect = async (accountId: string) => {
    try {
      await invoke('disconnect_calendar_account', { id: accountId });
      setAccounts((prev) => prev.filter((a) => a.id !== accountId));
      const evts = await invoke<CalendarEvent[]>('get_upcoming_calendar_events', { days: 14 });
      setEvents(evts);
    } catch (err: unknown) {
      console.error('Failed to disconnect calendar:', err);
      setActionError('Failed to disconnect account.');
    }
  };

  const handleOpenGoogleCalendar = async (accountEmail?: string) => {
    const url = accountEmail
      ? `https://calendar.google.com/calendar/u/${encodeURIComponent(accountEmail)}/r`
      : 'https://calendar.google.com/calendar/r';
    try {
      await invoke('open_external_url', { url });
    } catch (err) {
      window.open(url, '_blank');
    }
  };

  // Filter events based on activeFilter and searchQuery
  const filteredEvents = useMemo(() => {
    const now = new Date();
    const todayStr = now.toDateString();

    const tomorrow = new Date(now);
    tomorrow.setDate(tomorrow.getDate() + 1);
    const tomorrowStr = tomorrow.toDateString();

    const sevenDaysLater = new Date(now);
    sevenDaysLater.setDate(sevenDaysLater.getDate() + 7);

    return events.filter((evt) => {
      const start = new Date(evt.starts_at);
      if (isNaN(start.getTime())) return false;

      // Filter by quick tab
      if (activeFilter === 'today' && start.toDateString() !== todayStr) {
        return false;
      }
      if (activeFilter === 'tomorrow' && start.toDateString() !== tomorrowStr) {
        return false;
      }
      if (activeFilter === 'week') {
        if (start.getTime() < now.getTime() - 2 * 60 * 60 * 1000 || start.getTime() > sevenDaysLater.getTime()) {
          return false;
        }
      }

      // Filter by search query
      if (searchQuery.trim()) {
        const q = searchQuery.toLowerCase().trim();
        const matchTitle = evt.title.toLowerCase().includes(q);
        const matchCalendar = evt.calendar_name?.toLowerCase().includes(q);
        const matchAttendee = evt.attendees.some((a) => a.name.toLowerCase().includes(q));
        const matchLoc = evt.location?.toLowerCase().includes(q);
        if (!matchTitle && !matchCalendar && !matchAttendee && !matchLoc) {
          return false;
        }
      }

      return true;
    });
  }, [events, activeFilter, searchQuery]);

  // Group events by day
  const groupedEvents = useMemo(() => {
    const groups: Record<string, { label: string; date: Date; items: CalendarEvent[] }> = {};
    const todayStr = new Date().toDateString();
    const tomorrow = new Date();
    tomorrow.setDate(tomorrow.getDate() + 1);
    const tomorrowStr = tomorrow.toDateString();

    for (const evt of filteredEvents) {
      const d = new Date(evt.starts_at);
      if (isNaN(d.getTime())) continue;
      const key = d.toISOString().split('T')[0];

      if (!groups[key]) {
        let label = d.toLocaleDateString(undefined, {
          weekday: 'long',
          month: 'short',
          day: 'numeric',
        });
        if (d.toDateString() === todayStr) {
          label = `Today · ${label}`;
        } else if (d.toDateString() === tomorrowStr) {
          label = `Tomorrow · ${label}`;
        }
        groups[key] = { label, date: d, items: [] };
      }
      groups[key].items.push(evt);
    }

    return Object.values(groups).sort((a, b) => a.date.getTime() - b.date.getTime());
  }, [filteredEvents]);

  // Derived metrics for top summary bar (aligned with Home page library vitals)
  const metrics = useMemo(() => {
    const todayStr = new Date().toDateString();
    const todayCount = events.filter((e) => {
      const d = new Date(e.starts_at);
      return !isNaN(d.getTime()) && d.toDateString() === todayStr;
    }).length;

    const activeAccCount = accounts.filter((a) => a.enabled).length;

    // Find next event relative string
    const nowMs = Date.now();
    const upcoming = events
      .filter((e) => {
        const d = new Date(e.starts_at);
        return !isNaN(d.getTime()) && d.getTime() > nowMs;
      })
      .sort((a, b) => new Date(a.starts_at).getTime() - new Date(b.starts_at).getTime())[0];

    const nextRelStr = upcoming ? formatRelativeTimeToMeeting(upcoming.starts_at, upcoming.ends_at) : null;

    return {
      todayCount,
      activeAccCount,
      totalCount: events.length,
      nextEvent: upcoming,
      nextRelStr,
    };
  }, [accounts, events]);

  if (!isOpen) return null;

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center p-4 overflow-y-auto">
      {/* Backdrop */}
      <div
        className="fixed inset-0 bg-black/60 backdrop-blur-sm transition-opacity animate-in fade-in duration-150"
        onClick={() => !isConnecting && !isStarting && onClose()}
      />

      {/* Main Modal Card - Sized to 90vw width and 90vh height, matching Home page aesthetic */}
      <div className="relative bg-card text-card-foreground border border-border rounded-lg shadow-2xl w-[90vw] max-w-[90vw] h-[90vh] max-h-[90vh] flex flex-col overflow-hidden z-10 animate-in fade-in zoom-in-95 duration-200">
        {/* Header - Aligned with Home page typography and hierarchy */}
        <div className="p-4 px-6 border-b border-border flex items-center justify-between gap-4 shrink-0 bg-card">
          <div className="flex items-center gap-3">
            <div className="p-2 rounded-lg bg-primary/10 text-primary shrink-0 border border-primary/20">
              <CalendarDays className="w-5 h-5" />
            </div>
            <div>
              <span className="text-[10px] font-mono uppercase tracking-widest text-muted-foreground block">
                Meetings &amp; Schedule
              </span>
              <div className="flex items-center gap-2 mt-0.5">
                <h2 className="text-base font-extrabold text-foreground">Calendar View</h2>
                <Badge variant="outline" className="text-[10px] font-mono border-border px-1.5 py-0">
                  {filteredEvents.length} event{filteredEvents.length === 1 ? '' : 's'}
                </Badge>
              </div>
            </div>
          </div>

          <div className="flex items-center gap-2">
            <Button
              variant="outline"
              size="sm"
              onClick={handleSyncAll}
              disabled={isSyncing}
              className="text-xs h-8 gap-1.5 border-border hover:bg-muted/40 font-medium"
              title="Sync all connected calendars"
            >
              <RefreshCw className={`w-3.5 h-3.5 ${isSyncing ? 'animate-spin' : ''}`} />
              <span>Sync All</span>
            </Button>

            <Button
              variant="outline"
              size="sm"
              onClick={() => handleOpenGoogleCalendar()}
              className="text-xs h-8 gap-1.5 border-border text-muted-foreground hover:text-foreground hover:bg-muted/40 font-medium"
              title="Open Google Calendar in default browser"
            >
              <ExternalLink className="w-3.5 h-3.5" />
              <span>Google Calendar</span>
            </Button>

            <button
              type="button"
              onClick={onClose}
              className="text-muted-foreground hover:text-foreground p-1.5 rounded-lg hover:bg-muted transition-colors ml-1"
              aria-label="Close"
            >
              <X className="w-5 h-5" />
            </button>
          </div>
        </div>

        {/* Top Overview Metrics Strip - Exactly aligned with Home page vital rows */}
        <div className="border-b border-border bg-card/60 divide-y divide-border sm:divide-y-0 sm:flex sm:divide-x shrink-0">
          <div className="flex items-center gap-3 p-3 px-6 sm:flex-1 min-w-0">
            <Calendar className="w-4 h-4 text-primary shrink-0" />
            <div className="min-w-0">
              <p className="text-sm font-extrabold text-foreground leading-none">
                {accounts.length} Connected
              </p>
              <p className="text-[10px] text-muted-foreground truncate mt-0.5">
                {metrics.activeAccCount} active calendar accounts
              </p>
            </div>
          </div>

          <div className="flex items-center gap-3 p-3 px-6 sm:flex-1 min-w-0">
            <Clock className="w-4 h-4 text-emerald-500 shrink-0" />
            <div className="min-w-0">
              <p className="text-sm font-extrabold text-foreground leading-none">
                {metrics.todayCount} Today
              </p>
              <p className="text-[10px] text-muted-foreground truncate mt-0.5">
                events scheduled today
              </p>
            </div>
          </div>

          <div className="flex items-center gap-3 p-3 px-6 sm:flex-1 min-w-0">
            <CalendarDays className="w-4 h-4 text-indigo-400 shrink-0" />
            <div className="min-w-0">
              <p className="text-sm font-extrabold text-foreground leading-none">
                {metrics.totalCount} Upcoming
              </p>
              <p className="text-[10px] text-muted-foreground truncate mt-0.5">
                in the next 14 days
              </p>
            </div>
          </div>

          <div className="flex items-center gap-3 p-3 px-6 sm:flex-1 min-w-0">
            <Video className="w-4 h-4 text-sky-500 shrink-0" />
            <div className="min-w-0">
              <p className="text-sm font-extrabold text-foreground leading-none truncate">
                {metrics.nextRelStr ? metrics.nextRelStr : 'None'}
              </p>
              <p className="text-[10px] text-muted-foreground truncate mt-0.5" title={metrics.nextEvent?.title}>
                {metrics.nextEvent ? `next: ${metrics.nextEvent.title}` : 'next scheduled call'}
              </p>
            </div>
          </div>
        </div>

        {/* Action Error Banner */}
        {actionError && (
          <div className="p-2.5 px-6 bg-destructive/10 text-destructive text-xs border-b border-destructive/20 flex items-center justify-between">
            <span>{actionError}</span>
            <button onClick={() => setActionError(null)} className="hover:underline text-[11px]">
              Dismiss
            </button>
          </div>
        )}

        {/* Body Split View */}
        <div className="flex-1 flex min-h-0 overflow-hidden">
          {/* Left Column: Calendars Management (Styled like Home page shortcut cards) */}
          <aside className="w-80 lg:w-88 border-r border-border bg-card/40 flex flex-col shrink-0">
            <div className="p-3.5 px-5 border-b border-border flex items-center justify-between">
              <h3 className="text-[10px] font-mono uppercase tracking-widest text-muted-foreground">
                My Calendars ({accounts.length})
              </h3>
              <Button
                variant="outline"
                size="sm"
                onClick={() => {
                  setIsAddingCalendar(true);
                  setNewCalendarName('');
                  const nextColor =
                    CALENDAR_COLORS[accounts.length % CALENDAR_COLORS.length]?.hex || CALENDAR_COLORS[0].hex;
                  setNewCalendarColor(nextColor);
                }}
                className="h-6 px-2 text-[11px] gap-1 text-primary border-border hover:bg-primary/10"
              >
                <Plus className="w-3 h-3" />
                <span>Add</span>
              </Button>
            </div>

            {/* Add Calendar Form Dialog inside sidebar */}
            {isAddingCalendar && (
              <div className="p-3.5 m-3 rounded-lg bg-card border border-border shadow-xs space-y-3 animate-in fade-in zoom-in-98 duration-150">
                <div className="flex items-center justify-between">
                  <span className="text-[10px] font-mono uppercase tracking-widest text-muted-foreground">
                    Connect Account
                  </span>
                  <button
                    onClick={() => setIsAddingCalendar(false)}
                    className="text-muted-foreground hover:text-foreground"
                  >
                    <X className="w-3.5 h-3.5" />
                  </button>
                </div>

                <div className="space-y-1">
                  <label className="text-[10px] font-mono uppercase tracking-widest text-muted-foreground">
                    Calendar Name
                  </label>
                  <input
                    type="text"
                    placeholder="e.g. Work, Personal, School"
                    value={newCalendarName}
                    onChange={(e) => setNewCalendarName(e.target.value)}
                    className="w-full text-xs px-2.5 py-1.5 rounded-md bg-background border border-border focus:border-primary focus:outline-none"
                    autoFocus
                  />
                </div>

                <div className="space-y-1">
                  <label className="text-[10px] font-mono uppercase tracking-widest text-muted-foreground">
                    Color Accent
                  </label>
                  <div className="flex items-center gap-1.5 flex-wrap pt-0.5">
                    {CALENDAR_COLORS.map((c) => (
                      <button
                        key={c.hex}
                        type="button"
                        onClick={() => setNewCalendarColor(c.hex)}
                        className={`w-5 h-5 rounded-full transition-transform flex items-center justify-center ${
                          newCalendarColor === c.hex ? 'ring-2 ring-foreground scale-110' : 'hover:scale-105'
                        }`}
                        style={{ backgroundColor: c.hex }}
                        title={c.name}
                      >
                        {newCalendarColor === c.hex && <Check className="w-3 h-3 text-white" />}
                      </button>
                    ))}
                  </div>
                </div>

                <div className="flex items-center justify-end gap-2 pt-1">
                  <Button
                    variant="ghost"
                    size="sm"
                    onClick={() => setIsAddingCalendar(false)}
                    className="h-7 px-2 text-xs"
                    disabled={isConnecting}
                  >
                    Cancel
                  </Button>
                  <Button
                    variant="default"
                    size="sm"
                    onClick={handleAddCalendar}
                    disabled={!newCalendarName.trim() || isConnecting}
                    className="h-7 px-3 text-xs bg-primary text-primary-foreground font-medium"
                  >
                    {isConnecting ? (
                      <RefreshCw className="w-3 h-3 animate-spin" />
                    ) : (
                      'Authorize'
                    )}
                  </Button>
                </div>
              </div>
            )}

            {/* List of Accounts */}
            <div className="flex-1 overflow-y-auto p-3 space-y-2">
              {accounts.length === 0 ? (
                <div className="p-6 text-center text-muted-foreground space-y-2">
                  <Calendar className="w-6 h-6 mx-auto text-muted-foreground/50" />
                  <p className="text-xs font-medium">No calendars connected yet</p>
                  <p className="text-[11px] text-muted-foreground">
                    Add your work, personal, or school Google Calendars to sync your schedule.
                  </p>
                  <Button
                    variant="outline"
                    size="sm"
                    onClick={() => setIsAddingCalendar(true)}
                    className="text-xs h-7 gap-1 mt-1 border-border"
                  >
                    <Plus className="w-3 h-3" />
                    <span>Connect Google Calendar</span>
                  </Button>
                </div>
              ) : (
                accounts.map((account) => {
                  if (editingAccountId === account.id) {
                    return (
                      <div
                        key={account.id}
                        className="p-3 rounded-lg bg-card border border-border space-y-2.5 shadow-xs"
                      >
                        <div className="space-y-1">
                          <label className="text-[10px] font-mono uppercase tracking-widest text-muted-foreground">
                            Edit Name
                          </label>
                          <input
                            type="text"
                            value={editName}
                            onChange={(e) => setEditName(e.target.value)}
                            className="w-full text-xs px-2.5 py-1.5 rounded-md bg-background border border-border focus:border-primary focus:outline-none"
                            autoFocus
                          />
                        </div>
                        <div className="space-y-1">
                          <label className="text-[10px] font-mono uppercase tracking-widest text-muted-foreground">
                            Color
                          </label>
                          <div className="flex items-center gap-1.5 flex-wrap">
                            {CALENDAR_COLORS.map((c) => (
                              <button
                                key={c.hex}
                                type="button"
                                onClick={() => setEditColor(c.hex)}
                                className={`w-4 h-4 rounded-full transition-transform flex items-center justify-center ${
                                  editColor === c.hex ? 'ring-2 ring-foreground scale-110' : 'hover:scale-105'
                                }`}
                                style={{ backgroundColor: c.hex }}
                                title={c.name}
                              >
                                {editColor === c.hex && <Check className="w-2.5 h-2.5 text-white" />}
                              </button>
                            ))}
                          </div>
                        </div>
                        <div className="flex items-center justify-end gap-1.5 pt-1">
                          <Button
                            variant="ghost"
                            size="sm"
                            onClick={() => setEditingAccountId(null)}
                            className="h-6 px-2 text-xs"
                          >
                            Cancel
                          </Button>
                          <Button
                            variant="default"
                            size="sm"
                            onClick={() => handleSaveEdit(account.id)}
                            className="h-6 px-2.5 text-xs"
                          >
                            Save
                          </Button>
                        </div>
                      </div>
                    );
                  }

                  return (
                    <div
                      key={account.id}
                      className={`group p-3 rounded-lg border border-border text-left flex items-center justify-between gap-2.5 transition-all ${
                        account.enabled
                          ? 'bg-card hover:bg-muted/40 hover:border-primary/50'
                          : 'bg-muted/20 border-dashed opacity-60'
                      }`}
                    >
                      <div className="flex items-center gap-2.5 min-w-0 flex-1">
                        {/* Custom color toggle checkbox */}
                        <button
                          type="button"
                          onClick={() => handleToggleAccount(account)}
                          className="w-4 h-4 rounded-md flex items-center justify-center shrink-0 border border-border transition-colors"
                          style={{
                            backgroundColor: account.enabled ? account.color : 'transparent',
                            borderColor: account.color,
                          }}
                          title={account.enabled ? 'Disable calendar' : 'Enable calendar'}
                        >
                          {account.enabled && <Check className="w-3 h-3 text-white stroke-3" />}
                        </button>

                        <div className="min-w-0 flex-1">
                          <div className="flex items-center gap-1.5">
                            <span className="text-xs font-bold text-foreground truncate">
                              {account.name}
                            </span>
                            <span
                              className="w-2 h-2 rounded-full shrink-0"
                              style={{ backgroundColor: account.color }}
                            />
                          </div>
                          <div className="text-[10px] text-muted-foreground truncate font-mono mt-0.5">
                            {account.account_email || 'Connected'}
                          </div>
                        </div>
                      </div>

                      {/* Hotkey-styled Status badge or hover actions */}
                      <div className="flex items-center gap-1 shrink-0">
                        <div className="group-hover:hidden">
                          <span className="font-mono text-[9px] bg-background/80 px-1.5 py-0.5 rounded-md border border-border text-muted-foreground">
                            {account.enabled ? 'Active' : 'Muted'}
                          </span>
                        </div>

                        {/* Hover Actions */}
                        <div className="hidden group-hover:flex items-center gap-0.5 transition-opacity">
                          <button
                            type="button"
                            onClick={() => {
                              setEditingAccountId(account.id);
                              setEditName(account.name);
                              setEditColor(account.color);
                            }}
                            className="p-1 rounded text-muted-foreground hover:text-foreground hover:bg-muted"
                            title="Edit calendar name or color"
                          >
                            <Pencil className="w-3.5 h-3.5" />
                          </button>
                          <button
                            type="button"
                            onClick={() => handleDisconnect(account.id)}
                            className="p-1 rounded text-muted-foreground hover:text-destructive hover:bg-destructive/10"
                            title="Disconnect calendar"
                          >
                            <Trash2 className="w-3.5 h-3.5" />
                          </button>
                        </div>
                      </div>
                    </div>
                  );
                })
              )}
            </div>

            {/* Sidebar Bottom Sync Indicator */}
            {accounts.length > 0 && (
              <div className="p-3 px-4 border-t border-border text-[10px] font-mono text-muted-foreground flex items-center justify-between bg-card/60">
                <span>{accounts.filter((a) => a.enabled).length} of {accounts.length} active</span>
                <button
                  onClick={handleSyncAll}
                  disabled={isSyncing}
                  className="flex items-center gap-1 hover:text-foreground font-medium"
                >
                  <RefreshCw className={`w-3 h-3 ${isSyncing ? 'animate-spin' : ''}`} />
                  <span>Sync</span>
                </button>
              </div>
            )}
          </aside>

          {/* Right Column: Schedule / Agenda View */}
          <main className="flex-1 flex flex-col min-w-0 bg-background/40 overflow-hidden">
            {/* Filter Bar & Search */}
            <div className="p-3 px-6 border-b border-border flex items-center justify-between gap-4 flex-wrap bg-card/50">
              {/* Filter Tabs */}
              <div className="flex items-center gap-1 p-0.5 bg-muted/40 rounded-lg border border-border">
                {(['all', 'today', 'tomorrow', 'week'] as const).map((filter) => (
                  <button
                    key={filter}
                    onClick={() => setActiveFilter(filter)}
                    className={`px-3 py-1 rounded-md text-xs font-medium capitalize transition-colors ${
                      activeFilter === filter
                        ? 'bg-card text-foreground shadow-2xs font-semibold'
                        : 'text-muted-foreground hover:text-foreground'
                    }`}
                  >
                    {filter === 'week' ? 'Next 7 Days' : filter}
                  </button>
                ))}
              </div>

              {/* Search Bar */}
              <div className="relative w-64">
                <Search className="w-3.5 h-3.5 absolute left-2.5 top-1/2 -translate-y-1/2 text-muted-foreground" />
                <input
                  type="text"
                  placeholder="Search meetings, attendees..."
                  value={searchQuery}
                  onChange={(e) => setSearchQuery(e.target.value)}
                  className="w-full text-xs pl-8 pr-3 py-1.5 rounded-lg bg-card border border-border focus:border-primary focus:outline-none placeholder:text-muted-foreground"
                />
                {searchQuery && (
                  <button
                    onClick={() => setSearchQuery('')}
                    className="absolute right-2.5 top-1/2 -translate-y-1/2 text-muted-foreground hover:text-foreground"
                  >
                    <X className="w-3 h-3" />
                  </button>
                )}
              </div>
            </div>

            {/* Events List Scrollable */}
            <div className="flex-1 overflow-y-auto p-6 space-y-6">
              {isLoading && events.length === 0 ? (
                <div className="p-16 text-center text-muted-foreground flex flex-col items-center justify-center gap-2">
                  <RefreshCw className="w-6 h-6 animate-spin text-primary" />
                  <p className="text-xs font-medium">Loading synced calendars...</p>
                </div>
              ) : groupedEvents.length === 0 ? (
                <EmptyState
                  icon={Calendar}
                  title={searchQuery ? 'No matching meetings' : 'No upcoming meetings scheduled'}
                  description={
                    searchQuery
                      ? 'No events matched your search terms.'
                      : accounts.length === 0
                      ? 'Connect your Google Calendar accounts (Work, Personal, School) to sync and manage your meetings in one place.'
                      : 'There are no upcoming events scheduled in this period across your enabled calendars.'
                  }
                  action={
                    accounts.length === 0 ? (
                      <Button
                        variant="default"
                        size="sm"
                        onClick={() => setIsAddingCalendar(true)}
                        className="text-xs gap-1.5 bg-primary text-primary-foreground font-semibold"
                      >
                        <Plus className="w-3.5 h-3.5" />
                        <span>Connect First Calendar</span>
                      </Button>
                    ) : undefined
                  }
                  className="my-8"
                />
              ) : (
                groupedEvents.map((group) => (
                  <section key={group.label} className="space-y-2.5">
                    {/* Sticky Date Caption Header */}
                    <div className="sticky top-0 z-10 bg-background/95 backdrop-blur-xs py-1 flex items-center gap-2">
                      <h4 className="text-[10px] font-mono uppercase tracking-widest text-muted-foreground flex items-center gap-2">
                        <span>{group.label}</span>
                        <span className="text-[10px] font-mono text-muted-foreground/70">
                          ({group.items.length})
                        </span>
                      </h4>
                      <div className="h-px bg-border flex-1 ml-2" />
                    </div>

                    <div className="space-y-2.5">
                      {group.items.map((evt) => {
                        const startDate = new Date(evt.starts_at);
                        const endDate = new Date(evt.ends_at);
                        const timeStr = isNaN(startDate.getTime())
                          ? ''
                          : `${startDate.toLocaleTimeString([], {
                              hour: '2-digit',
                              minute: '2-digit',
                            })} – ${
                              isNaN(endDate.getTime())
                                ? ''
                                : endDate.toLocaleTimeString([], {
                                    hour: '2-digit',
                                    minute: '2-digit',
                                  })
                            }`;
                        const joinUrl = getMeetingJoinUrl(evt);
                        const relativeTime = formatRelativeTimeToMeeting(evt.starts_at, evt.ends_at);
                        const calColor = evt.calendar_color || '#3b82f6';

                        return (
                          <div
                            key={evt.id}
                            className="group rounded-lg border border-border bg-card p-3.5 hover:bg-muted/40 hover:border-primary/50 transition-all text-left flex flex-col md:flex-row md:items-center justify-between gap-3 relative"
                          >
                            {/* Color Accent Indicator Strip */}
                            <div
                              className="absolute left-0 top-2 bottom-2 w-1 rounded-r-full"
                              style={{ backgroundColor: calColor }}
                            />

                            {/* Left Content Area */}
                            <div className="space-y-1 min-w-0 flex-1 pl-2">
                              {/* Tags and Badges Line */}
                              <div className="flex items-center gap-2 flex-wrap">
                                {evt.calendar_name && (
                                  <span
                                    className="font-mono text-[9px] px-1.5 py-0.5 rounded-md border flex items-center gap-1 font-semibold"
                                    style={{
                                      backgroundColor: `${calColor}15`,
                                      borderColor: `${calColor}40`,
                                      color: calColor,
                                    }}
                                  >
                                    <span
                                      className="w-1.5 h-1.5 rounded-full"
                                      style={{ backgroundColor: calColor }}
                                    />
                                    <span>{evt.calendar_name}</span>
                                  </span>
                                )}

                                {timeStr && (
                                  <span className="font-mono text-[10px] text-muted-foreground flex items-center gap-1">
                                    <Clock className="w-3 h-3 text-muted-foreground/70" />
                                    <span>{timeStr}</span>
                                  </span>
                                )}

                                {relativeTime && (
                                  <span className="font-mono text-[9px] bg-primary/10 border border-primary/20 text-primary px-1.5 py-0.5 rounded-md font-semibold">
                                    {relativeTime}
                                  </span>
                                )}

                                {joinUrl && (
                                  <span className="font-mono text-[9px] bg-muted/60 border border-border text-muted-foreground px-1.5 py-0.5 rounded-md flex items-center gap-1">
                                    <Video className="w-2.5 h-2.5 text-blue-500" />
                                    <span>Video Call</span>
                                  </span>
                                )}

                                {evt.attendees.length > 0 && (
                                  <span className="font-mono text-[9px] text-muted-foreground flex items-center gap-1">
                                    <Users className="w-2.5 h-2.5 text-muted-foreground/70" />
                                    <span>{evt.attendees.length}</span>
                                  </span>
                                )}
                              </div>

                              {/* Title */}
                              <h5 className="text-xs font-bold text-foreground truncate">
                                {evt.title}
                              </h5>

                              {/* Description / Location Preview */}
                              {(evt.location || evt.description) && (
                                <p className="text-[10px] text-muted-foreground truncate">
                                  {evt.location || evt.description?.replace(/\n/g, ' ').slice(0, 120)}
                                </p>
                              )}
                            </div>

                            {/* Right Action Controls */}
                            <div className="flex items-center gap-1.5 shrink-0 pl-2 md:pl-0">
                              <Button
                                variant="ghost"
                                size="sm"
                                onClick={() => onSelectEvent(evt)}
                                className="h-7 px-2.5 text-xs text-muted-foreground hover:text-foreground"
                              >
                                Details
                              </Button>

                              <Button
                                variant="outline"
                                size="sm"
                                onClick={() => onStartRecording(evt)}
                                disabled={isStarting}
                                className="h-7 px-2.5 text-xs gap-1 border-border hover:bg-primary/10 hover:text-primary hover:border-primary/50 font-medium"
                              >
                                <Play className="w-3 h-3 fill-current" />
                                <span>Record</span>
                              </Button>

                              {joinUrl && (
                                <Button
                                  variant="default"
                                  size="sm"
                                  onClick={() => onJoinAndRecord(evt)}
                                  disabled={isStarting}
                                  className="h-7 px-3 text-xs gap-1 font-semibold bg-primary text-primary-foreground shadow-xs"
                                >
                                  <Video className="w-3 h-3" />
                                  <span>Join &amp; Record</span>
                                </Button>
                              )}
                            </div>
                          </div>
                        );
                      })}
                    </div>
                  </section>
                ))
              )}
            </div>
          </main>
        </div>
      </div>
    </div>
  );
};
