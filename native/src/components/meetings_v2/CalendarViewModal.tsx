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
  Trash2,
  Users,
  Video,
  X,
} from 'lucide-react';
import { Button } from '@/components/ui/button';
import { Badge } from '@/components/ui/badge';
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
    } catch (err) {
      console.error('Failed to toggle calendar account:', err);
    }
  };

  const handleSaveEdit = async (id: string) => {
    try {
      const updated = await invoke<CalendarAccount>('update_calendar_account', {
        id,
        name: editName.trim() || null,
        color: editColor,
        enabled: null,
      });
      setAccounts((prev) => prev.map((a) => (a.id === updated.id ? updated : a)));
      setEditingAccountId(null);
      const evts = await invoke<CalendarEvent[]>('get_upcoming_calendar_events', { days: 14 });
      setEvents(evts);
    } catch (err) {
      console.error('Failed to update calendar account:', err);
    }
  };

  const handleDisconnect = async (id: string) => {
    try {
      const remaining = await invoke<CalendarAccount[]>('disconnect_calendar_account', { id });
      setAccounts(remaining);
      const evts = await invoke<CalendarEvent[]>('get_upcoming_calendar_events', { days: 14 });
      setEvents(evts);
    } catch (err) {
      console.error('Failed to disconnect calendar account:', err);
    }
  };

  const handleOpenGoogleCalendar = async (account?: CalendarAccount) => {
    try {
      const url = account?.account_email
        ? `https://calendar.google.com/calendar/u/${encodeURIComponent(account.account_email)}/r`
        : 'https://calendar.google.com/';
      await invoke('open_external_url', { url });
    } catch (err) {
      console.error('Failed to open Google Calendar in browser:', err);
    }
  };

  // Filter and group events
  const filteredEvents = useMemo(() => {
    const query = searchQuery.trim().toLowerCase();
    const now = new Date();
    const todayStr = now.toDateString();
    const tomorrow = new Date(now);
    tomorrow.setDate(tomorrow.getDate() + 1);
    const tomorrowStr = tomorrow.toDateString();

    return events.filter((evt) => {
      // 1. Account enabled filter
      if (evt.calendar_id) {
        const acc = accounts.find((a) => a.id === evt.calendar_id);
        if (acc && !acc.enabled) return false;
      }

      // 2. Search query filter
      if (query) {
        const titleMatch = evt.title.toLowerCase().includes(query);
        const descMatch = evt.description?.toLowerCase().includes(query) ?? false;
        const orgMatch = evt.organizer?.toLowerCase().includes(query) ?? false;
        const attMatch = evt.attendees.some((a) => a.name.toLowerCase().includes(query));
        const calMatch = evt.calendar_name?.toLowerCase().includes(query) ?? false;
        if (!titleMatch && !descMatch && !orgMatch && !attMatch && !calMatch) {
          return false;
        }
      }

      // 3. Time filter
      const start = new Date(evt.starts_at);
      if (isNaN(start.getTime())) return false;
      const startDayStr = start.toDateString();

      if (activeFilter === 'today') {
        return startDayStr === todayStr;
      }
      if (activeFilter === 'tomorrow') {
        return startDayStr === tomorrowStr;
      }
      if (activeFilter === 'week') {
        const sevenDaysLater = new Date(now.getTime() + 7 * 24 * 60 * 60 * 1000);
        return start >= now && start <= sevenDaysLater;
      }

      return true;
    });
  }, [events, accounts, searchQuery, activeFilter]);

  // Group filtered events by date
  const groupedEvents = useMemo(() => {
    const groups: { [dateKey: string]: { label: string; date: Date; items: CalendarEvent[] } } = {};
    const now = new Date();
    const todayStr = now.toDateString();
    const tomorrow = new Date(now);
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

  if (!isOpen) return null;

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center p-4 overflow-y-auto">
      {/* Backdrop */}
      <div
        className="fixed inset-0 bg-black/60 backdrop-blur-sm transition-opacity animate-in fade-in duration-150"
        onClick={() => !isConnecting && !isStarting && onClose()}
      />

      {/* Main Modal Card - Sized identically to Files Vault */}
      <div className="relative bg-card text-card-foreground border border-border rounded-xl shadow-2xl w-[80vw] max-w-[80vw] max-h-[88vh] flex flex-col overflow-hidden z-10 animate-in fade-in zoom-in-95 duration-200">
        {/* Header */}
        <div className="p-5 px-6 border-b border-border/60 flex items-center justify-between gap-4 shrink-0 bg-muted/20">
          <div className="flex items-center gap-3">
            <div className="p-2.5 rounded-lg bg-primary/10 text-primary shrink-0">
              <CalendarDays className="w-6 h-6" />
            </div>
            <div>
              <div className="flex items-center gap-2">
                <h2 className="text-xl font-bold text-foreground">Calendar View</h2>
                <Badge variant="outline" className="text-xs font-mono">
                  {filteredEvents.length} event{filteredEvents.length === 1 ? '' : 's'}
                </Badge>
              </div>
              <p className="text-xs text-muted-foreground mt-0.5">
                Synced schedules across your connected accounts (Work, Personal, School).
              </p>
            </div>
          </div>

          <div className="flex items-center gap-2">
            <Button
              variant="outline"
              size="sm"
              onClick={handleSyncAll}
              disabled={isSyncing}
              className="text-xs h-8 gap-1.5 border-border"
              title="Sync all calendars"
            >
              <RefreshCw className={`w-3.5 h-3.5 ${isSyncing ? 'animate-spin' : ''}`} />
              <span>Sync All</span>
            </Button>

            <Button
              variant="outline"
              size="sm"
              onClick={() => handleOpenGoogleCalendar()}
              className="text-xs h-8 gap-1.5 border-border text-muted-foreground hover:text-foreground"
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
          {/* Left Column: Calendars Management */}
          <aside className="w-72 lg:w-80 border-r border-border/60 bg-muted/15 flex flex-col shrink-0">
            <div className="p-3.5 border-b border-border/50 flex items-center justify-between">
              <div className="text-xs font-bold text-foreground flex items-center gap-1.5">
                <Calendar className="w-4 h-4 text-primary" />
                <span>My Calendars ({accounts.length})</span>
              </div>
              <Button
                variant="ghost"
                size="sm"
                onClick={() => {
                  setIsAddingCalendar(true);
                  setNewCalendarName('');
                  const nextColor = CALENDAR_COLORS[accounts.length % CALENDAR_COLORS.length]?.hex || CALENDAR_COLORS[0].hex;
                  setNewCalendarColor(nextColor);
                }}
                className="h-7 px-2 text-xs gap-1 text-primary hover:text-primary hover:bg-primary/10"
              >
                <Plus className="w-3.5 h-3.5" />
                <span>Add</span>
              </Button>
            </div>

            {/* Add Calendar Form Dialog inside sidebar */}
            {isAddingCalendar && (
              <div className="p-3.5 m-2.5 rounded-lg bg-card border border-primary/40 shadow-xs space-y-3 animate-in fade-in zoom-in-98 duration-150">
                <div className="flex items-center justify-between text-xs font-semibold text-foreground">
                  <span>Connect Google Calendar</span>
                  <button
                    onClick={() => setIsAddingCalendar(false)}
                    className="text-muted-foreground hover:text-foreground"
                  >
                    <X className="w-3.5 h-3.5" />
                  </button>
                </div>
                <div className="space-y-1.5">
                  <label className="text-[10px] uppercase font-bold text-muted-foreground tracking-wider">
                    Calendar Name
                  </label>
                  <input
                    type="text"
                    placeholder="e.g. Work, Personal, School"
                    value={newCalendarName}
                    onChange={(e) => setNewCalendarName(e.target.value)}
                    className="w-full text-xs px-2.5 py-1.5 rounded-md bg-muted/50 border border-border focus:border-primary focus:outline-none"
                    autoFocus
                  />
                </div>

                <div className="space-y-1.5">
                  <label className="text-[10px] uppercase font-bold text-muted-foreground tracking-wider">
                    Color
                  </label>
                  <div className="flex items-center gap-1.5 flex-wrap">
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

            {/* Calendars List */}
            <div className="flex-1 overflow-y-auto p-2.5 space-y-1.5">
              {accounts.length === 0 && !isAddingCalendar ? (
                <div className="p-4 text-center text-xs text-muted-foreground space-y-2">
                  <Calendar className="w-8 h-8 text-muted-foreground/40 mx-auto" />
                  <p>No calendars connected yet.</p>
                  <Button
                    variant="outline"
                    size="sm"
                    onClick={() => setIsAddingCalendar(true)}
                    className="text-xs h-8 gap-1.5 border-dashed"
                  >
                    <Plus className="w-3.5 h-3.5" />
                    <span>Connect Calendar</span>
                  </Button>
                </div>
              ) : (
                accounts.map((account) => {
                  const isEditing = editingAccountId === account.id;

                  if (isEditing) {
                    return (
                      <div
                        key={account.id}
                        className="p-3 rounded-lg bg-card border border-primary/40 space-y-2.5 shadow-xs"
                      >
                        <div className="text-[11px] font-semibold text-foreground">Edit Calendar</div>
                        <input
                          type="text"
                          value={editName}
                          onChange={(e) => setEditName(e.target.value)}
                          className="w-full text-xs px-2.5 py-1.5 rounded-md bg-muted/50 border border-border focus:border-primary focus:outline-none"
                        />
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
                            >
                              {editColor === c.hex && <Check className="w-2.5 h-2.5 text-white" />}
                            </button>
                          ))}
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
                      className={`group p-2.5 rounded-lg border transition-all flex items-center justify-between gap-2.5 ${
                        account.enabled
                          ? 'bg-card border-border/80 hover:border-border'
                          : 'bg-muted/30 border-dashed border-border/50 opacity-60'
                      }`}
                    >
                      <div className="flex items-center gap-2.5 min-w-0 flex-1">
                        {/* Color indicator and toggle checkbox */}
                        <button
                          type="button"
                          onClick={() => handleToggleAccount(account)}
                          className="w-4 h-4 rounded-md flex items-center justify-center shrink-0 border border-border/60 transition-colors"
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
                            <span className="text-xs font-semibold text-foreground truncate">
                              {account.name}
                            </span>
                            <span
                              className="w-2 h-2 rounded-full shrink-0"
                              style={{ backgroundColor: account.color }}
                            />
                          </div>
                          <div className="text-[10px] text-muted-foreground truncate font-mono">
                            {account.account_email || 'Connected'}
                          </div>
                        </div>
                      </div>

                      {/* Hover Actions */}
                      <div className="flex items-center gap-0.5 opacity-0 group-hover:opacity-100 transition-opacity">
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
                          <Pencil className="w-3 h-3" />
                        </button>
                        <button
                          type="button"
                          onClick={() => handleDisconnect(account.id)}
                          className="p-1 rounded text-muted-foreground hover:text-destructive hover:bg-destructive/10"
                          title="Disconnect calendar"
                        >
                          <Trash2 className="w-3 h-3" />
                        </button>
                      </div>
                    </div>
                  );
                })
              )}
            </div>

            {/* Sidebar Bottom Sync Indicator */}
            {accounts.length > 0 && (
              <div className="p-3 border-t border-border/50 text-[11px] text-muted-foreground flex items-center justify-between">
                <span>{accounts.filter((a) => a.enabled).length} active</span>
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
          <main className="flex-1 flex flex-col min-w-0 bg-background/50 overflow-hidden">
            {/* Filter Bar & Search */}
            <div className="p-3.5 px-6 border-b border-border/60 flex items-center justify-between gap-4 flex-wrap bg-muted/10">
              {/* Filter Tabs */}
              <div className="flex items-center gap-1 p-0.5 bg-muted/50 rounded-lg border border-border/50">
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
                  className="w-full text-xs pl-8 pr-3 py-1.5 rounded-lg bg-card border border-border/80 focus:border-primary focus:outline-none placeholder:text-muted-foreground"
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
                <div className="p-12 text-center text-muted-foreground flex flex-col items-center justify-center gap-2">
                  <RefreshCw className="w-6 h-6 animate-spin text-primary" />
                  <p className="text-xs font-medium">Loading synced calendars...</p>
                </div>
              ) : groupedEvents.length === 0 ? (
                <div className="p-16 text-center border border-dashed border-border rounded-xl space-y-3 bg-muted/5">
                  <Calendar className="w-10 h-10 text-muted-foreground/40 mx-auto" />
                  <div className="space-y-1">
                    <h3 className="text-sm font-semibold text-foreground">
                      No upcoming meetings found
                    </h3>
                    <p className="text-xs text-muted-foreground max-w-sm mx-auto">
                      {searchQuery
                        ? 'No meetings match your search query.'
                        : accounts.length === 0
                        ? 'Connect your Google Calendar accounts (Work, Personal, School) to see all your meetings in one place.'
                        : 'There are no upcoming events scheduled in this period across your enabled calendars.'}
                    </p>
                  </div>
                  {accounts.length === 0 && (
                    <Button
                      variant="default"
                      size="sm"
                      onClick={() => setIsAddingCalendar(true)}
                      className="text-xs gap-1.5 mt-2 bg-primary text-primary-foreground font-semibold"
                    >
                      <Plus className="w-3.5 h-3.5" />
                      <span>Connect First Calendar</span>
                    </Button>
                  )}
                </div>
              ) : (
                groupedEvents.map((group) => (
                  <div key={group.label} className="space-y-3">
                    <div className="sticky top-0 z-10 bg-background/95 backdrop-blur-xs py-1 text-xs font-bold text-muted-foreground uppercase tracking-wider flex items-center gap-2">
                      <span>{group.label}</span>
                      <span className="text-[10px] text-muted-foreground/60 font-mono">
                        ({group.items.length})
                      </span>
                      <div className="h-px bg-border/60 flex-1 ml-2" />
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
                            className="group relative p-4 rounded-xl bg-card border border-border/70 hover:border-primary/50 transition-all flex flex-col md:flex-row md:items-center justify-between gap-4 shadow-2xs hover:shadow-xs"
                          >
                            {/* Color bar indicator */}
                            <div
                              className="absolute left-0 top-3 bottom-3 w-1 rounded-r-full"
                              style={{ backgroundColor: calColor }}
                            />

                            {/* Left Info */}
                            <div className="space-y-1.5 min-w-0 flex-1 pl-2">
                              <div className="flex items-center gap-2 flex-wrap">
                                {evt.calendar_name && (
                                  <span
                                    className="text-[10px] font-bold px-2 py-0.5 rounded-full border flex items-center gap-1.5"
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

                                {relativeTime && (
                                  <Badge
                                    variant="outline"
                                    className="text-[10px] font-mono border-primary/30 text-primary bg-primary/10"
                                  >
                                    <Clock className="w-2.5 h-2.5 mr-1" />
                                    {relativeTime}
                                  </Badge>
                                )}

                                <span className="text-xs text-muted-foreground font-mono">
                                  {timeStr}
                                </span>
                              </div>

                              <h4
                                onClick={() => onSelectEvent(evt)}
                                className="text-sm font-bold text-foreground hover:text-primary transition-colors cursor-pointer truncate"
                                title={evt.title}
                              >
                                {evt.title}
                              </h4>

                              <div className="flex items-center gap-3 text-xs text-muted-foreground flex-wrap">
                                {joinUrl ? (
                                  <span className="flex items-center gap-1 text-blue-500 font-medium">
                                    <Video className="w-3.5 h-3.5" />
                                    <span>Conferencing Available</span>
                                  </span>
                                ) : evt.location ? (
                                  <span className="flex items-center gap-1 truncate max-w-xs">
                                    <MapPin className="w-3.5 h-3.5 shrink-0" />
                                    <span className="truncate">{evt.location}</span>
                                  </span>
                                ) : null}

                                {evt.attendees && evt.attendees.length > 0 && (
                                  <span className="flex items-center gap-1">
                                    <Users className="w-3.5 h-3.5" />
                                    <span>
                                      {evt.attendees.length} participant
                                      {evt.attendees.length === 1 ? '' : 's'}
                                    </span>
                                  </span>
                                )}

                                {evt.organizer && (
                                  <span className="truncate max-w-xs text-[11px]">
                                    by {evt.organizer}
                                  </span>
                                )}
                              </div>
                            </div>

                            {/* Action Buttons on Card */}
                            <div className="flex items-center gap-2 shrink-0 pt-2 md:pt-0 border-t md:border-t-0 border-border/40">
                              <Button
                                variant="ghost"
                                size="sm"
                                onClick={() => onSelectEvent(evt)}
                                className="text-xs h-8 px-2.5 text-muted-foreground hover:text-foreground"
                              >
                                Details
                              </Button>

                              <Button
                                variant="outline"
                                size="sm"
                                onClick={() => onStartRecording(evt)}
                                disabled={isStarting}
                                className="text-xs h-8 px-3 gap-1.5 border-border hover:bg-muted font-medium"
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
                                  className="text-xs h-8 px-3 gap-1.5 bg-primary text-primary-foreground hover:bg-primary/90 font-semibold"
                                >
                                  <Video className="w-3.5 h-3.5" />
                                  <span>Join & Record</span>
                                </Button>
                              )}
                            </div>
                          </div>
                        );
                      })}
                    </div>
                  </div>
                ))
              )}
            </div>
          </main>
        </div>
      </div>
    </div>
  );
};
