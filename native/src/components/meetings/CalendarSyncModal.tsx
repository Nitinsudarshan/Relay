import React, { useState, useEffect } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { CalendarMeetingEvent, CalendarConnectionStatus, GoogleCalendarConfig } from '../../types';
import { Button } from '@/components/ui/button';
import { Badge } from '@/components/ui/badge';
import { Input } from '@/components/ui/input';
import {
  Calendar,
  RefreshCw,
  X,
  CheckCircle2,
  Video,
  Clock,
  Users,
  Plus,
  Unlink,
  ExternalLink,
  ChevronDown,
  ChevronRight,
  ShieldCheck,
  AlertCircle,
  KeyRound,
  HelpCircle,
} from 'lucide-react';

interface CalendarSyncModalProps {
  isOpen: boolean;
  onClose: () => void;
  authStatus: CalendarConnectionStatus;
  calendarEvents: CalendarMeetingEvent[];
  onConnectGoogle: (clientId?: string, clientSecret?: string) => Promise<void>;
  onDisconnectGoogle: () => Promise<void>;
  onSyncNow: () => Promise<void>;
  onImportMeeting: (event: CalendarMeetingEvent) => Promise<void>;
}

export const CalendarSyncModal: React.FC<CalendarSyncModalProps> = ({
  isOpen,
  onClose,
  authStatus,
  calendarEvents,
  onConnectGoogle,
  onDisconnectGoogle,
  onSyncNow,
  onImportMeeting,
}) => {
  const [connecting, setConnecting] = useState(false);
  const [syncing, setSyncing] = useState(false);
  const [disconnecting, setDisconnecting] = useState(false);
  const [importingId, setImportingId] = useState<string | null>(null);
  const [showCustomCreds, setShowCustomCreds] = useState(false);
  const [customClientId, setCustomClientId] = useState('');
  const [customClientSecret, setCustomClientSecret] = useState('');
  const [errorMsg, setErrorMsg] = useState<string | null>(null);

  // Load saved credentials on open
  useEffect(() => {
    if (isOpen) {
      invoke<GoogleCalendarConfig>('get_google_oauth_config')
        .then((cfg) => {
          if (cfg?.client_id) setCustomClientId(cfg.client_id);
          if (cfg?.client_secret) setCustomClientSecret(cfg.client_secret);
        })
        .catch(console.error);
    }
  }, [isOpen]);

  if (!isOpen) return null;

  const handleConnect = async () => {
    setConnecting(true);
    setErrorMsg(null);
    try {
      const cId = customClientId.trim() || undefined;
      const cSecret = customClientSecret.trim() || undefined;

      // Persist config if provided
      if (cId) {
        await invoke('save_google_oauth_config', {
          config: {
            client_id: cId || null,
            client_secret: cSecret || null,
          },
        });
      }

      await onConnectGoogle(cId, cSecret);
    } catch (err: any) {
      setErrorMsg(typeof err === 'string' ? err : err?.message || 'Google OAuth connection failed.');
    } finally {
      setConnecting(false);
    }
  };

  const handleDisconnect = async () => {
    setDisconnecting(true);
    setErrorMsg(null);
    try {
      await onDisconnectGoogle();
    } catch (err: any) {
      setErrorMsg(typeof err === 'string' ? err : err?.message || 'Failed to disconnect Google Calendar.');
    } finally {
      setDisconnecting(false);
    }
  };

  const handleSync = async () => {
    setSyncing(true);
    setErrorMsg(null);
    try {
      await onSyncNow();
    } catch (err: any) {
      setErrorMsg(typeof err === 'string' ? err : err?.message || 'Failed to sync Google Calendar events.');
    } finally {
      setSyncing(false);
    }
  };

  const handleImport = async (event: CalendarMeetingEvent) => {
    setImportingId(event.id);
    try {
      await onImportMeeting(event);
    } finally {
      setImportingId(null);
    }
  };

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/60 backdrop-blur-xs p-4 animate-in fade-in duration-200">
      <div className="bg-card border border-border rounded-xl shadow-xl w-full max-w-2xl overflow-hidden flex flex-col max-h-[85vh]">
        {/* Modal Header */}
        <div className="flex items-center justify-between px-6 py-4 border-b border-border/80 bg-muted/20">
          <div className="flex items-center gap-2.5">
            <div className="p-1.5 rounded-lg bg-blue-500/10 border border-blue-500/30 text-blue-500">
              <Calendar className="w-5 h-5" />
            </div>
            <div>
              <h2 className="text-base font-bold text-foreground">Google Calendar</h2>
              <p className="text-[11px] text-muted-foreground">
                {authStatus.connected
                  ? `Connected as ${authStatus.account_email || 'Google User'}`
                  : 'Connect your calendar to detect upcoming meetings and associate meeting metadata with Relay.'}
              </p>
            </div>
          </div>

          <div className="flex items-center gap-2">
            {authStatus.connected && (
              <Button
                variant="outline"
                size="sm"
                onClick={handleSync}
                disabled={syncing}
                className="text-xs gap-1.5"
              >
                <RefreshCw className={`w-3.5 h-3.5 ${syncing ? 'animate-spin text-primary' : ''}`} />
                <span>{syncing ? 'Syncing…' : 'Sync Now'}</span>
              </Button>
            )}
            <Button
              variant="ghost"
              size="icon"
              onClick={onClose}
              className="h-8 w-8 text-muted-foreground hover:text-foreground"
            >
              <X className="w-4 h-4" />
            </Button>
          </div>
        </div>

        {/* Error Alert if any */}
        {errorMsg && (
          <div className="mx-6 mt-4 p-3 rounded-lg bg-destructive/10 border border-destructive/30 flex items-center gap-2 text-xs text-destructive">
            <AlertCircle className="w-4 h-4 shrink-0" />
            <span>{errorMsg}</span>
          </div>
        )}

        {/* Content Body */}
        <div className="p-6 space-y-4 overflow-y-auto flex-1">
          {!authStatus.connected ? (
            /* DISCONNECTED STATE */
            <div className="space-y-4">
              <div className="p-5 rounded-xl border border-dashed border-border text-center space-y-3 bg-muted/5">
                <div className="w-10 h-10 rounded-full bg-blue-500/10 border border-blue-500/20 text-blue-500 flex items-center justify-center mx-auto">
                  <Calendar className="w-5 h-5" />
                </div>
                <div className="space-y-1 max-w-md mx-auto">
                  <h3 className="text-sm font-bold text-foreground">Connect Google Calendar</h3>
                  <p className="text-xs text-muted-foreground leading-relaxed">
                    Uses read-only calendar permissions to detect upcoming meetings and attach agendas. All data stays strictly local.
                  </p>
                </div>

                <div className="pt-1">
                  <Button
                    onClick={handleConnect}
                    disabled={connecting}
                    className="text-xs bg-blue-600 hover:bg-blue-700 text-white gap-2 shadow-xs px-5 py-2 h-9"
                  >
                    <Calendar className="w-4 h-4" />
                    <span>{connecting ? 'Waiting for Google Authorization…' : 'Authorize & Connect'}</span>
                  </Button>
                </div>
              </div>

              {/* Google OAuth Credentials Configuration */}
              <div className="border border-border/70 rounded-xl overflow-hidden bg-card">
                <button
                  type="button"
                  onClick={() => setShowCustomCreds(!showCustomCreds)}
                  className="w-full p-3 flex items-center justify-between text-xs font-semibold text-foreground hover:bg-muted/20 transition-all select-none text-left"
                >
                  <div className="flex items-center gap-2">
                    <KeyRound className="w-4 h-4 text-blue-500" />
                    <span>Google Cloud OAuth 2.0 Credentials</span>
                  </div>
                  {showCustomCreds ? <ChevronDown className="w-3.5 h-3.5" /> : <ChevronRight className="w-3.5 h-3.5" />}
                </button>

                {showCustomCreds && (
                  <div className="p-4 border-t border-border/70 space-y-3 bg-muted/10">
                    <div className="p-3 rounded-lg bg-blue-500/5 border border-blue-500/20 space-y-1.5 text-[11px] text-muted-foreground">
                      <div className="font-semibold text-foreground flex items-center gap-1.5">
                        <HelpCircle className="w-3.5 h-3.5 text-blue-500" />
                        <span>How to get your free Google OAuth Client ID:</span>
                      </div>
                      <ol className="list-decimal list-inside space-y-1 text-muted-foreground pl-1">
                        <li>
                          Open{' '}
                          <a
                            href="https://console.cloud.google.com/apis/credentials"
                            target="_blank"
                            rel="noreferrer"
                            className="text-blue-500 underline font-medium"
                          >
                            Google Cloud Console Credentials
                          </a>
                        </li>
                        <li>Ensure <strong>Google Calendar API</strong> is enabled in APIs & Services &gt; Library</li>
                        <li>Click <strong>+ Create Credentials &gt; OAuth client ID</strong></li>
                        <li>Select Application type: <strong>Desktop app</strong>, name it <em>Relay</em>, and click Create</li>
                        <li>Paste your <strong>Client ID</strong> and <strong>Client Secret</strong> below:</li>
                      </ol>
                    </div>

                    <div className="space-y-1">
                      <label className="text-[11px] font-semibold text-foreground">Client ID</label>
                      <Input
                        value={customClientId}
                        onChange={(e) => setCustomClientId(e.target.value)}
                        placeholder="e.g. 123456789-xyz.apps.googleusercontent.com"
                        className="text-xs h-8 font-mono"
                      />
                    </div>

                    <div className="space-y-1">
                      <label className="text-[11px] font-semibold text-foreground">Client Secret</label>
                      <Input
                        type="password"
                        value={customClientSecret}
                        onChange={(e) => setCustomClientSecret(e.target.value)}
                        placeholder="e.g. GOCSPX-..."
                        className="text-xs h-8 font-mono"
                      />
                    </div>
                  </div>
                )}
              </div>
            </div>
          ) : (
            /* CONNECTED STATE */
            <div className="space-y-4">
              {/* Account Status Card */}
              <div className="p-4 rounded-xl bg-emerald-500/5 border border-emerald-500/20 flex items-center justify-between gap-4">
                <div className="flex items-center gap-3">
                  <div className="w-8 h-8 rounded-full bg-emerald-500/10 border border-emerald-500/30 text-emerald-500 flex items-center justify-center font-bold text-xs">
                    ✓
                  </div>
                  <div>
                    <div className="flex items-center gap-2">
                      <span className="text-xs font-bold text-foreground">Google Calendar Connected</span>
                      <Badge variant="outline" className="text-[9px] py-0 px-1.5 border-emerald-500/40 text-emerald-500">
                        Active
                      </Badge>
                    </div>
                    <p className="text-[11px] text-muted-foreground">
                      Connected as <span className="font-semibold text-foreground">{authStatus.account_email}</span>
                      {authStatus.last_synced_at && ` • Last synced ${new Date(authStatus.last_synced_at).toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' })}`}
                    </p>
                  </div>
                </div>

                <div className="flex items-center gap-2">
                  <Button
                    size="sm"
                    variant="outline"
                    onClick={handleSync}
                    disabled={syncing}
                    className="text-xs h-8 gap-1.5"
                  >
                    <RefreshCw className={`w-3.5 h-3.5 ${syncing ? 'animate-spin text-primary' : ''}`} />
                    <span>{syncing ? 'Syncing…' : 'Sync Now'}</span>
                  </Button>

                  <Button
                    size="sm"
                    variant="ghost"
                    onClick={handleDisconnect}
                    disabled={disconnecting}
                    className="text-xs h-8 text-destructive hover:bg-destructive/10 gap-1.5"
                  >
                    <Unlink className="w-3.5 h-3.5" />
                    <span>Disconnect</span>
                  </Button>
                </div>
              </div>

              {/* Events List */}
              <div className="space-y-2 pt-2">
                <h4 className="text-xs font-bold text-foreground uppercase tracking-wider font-mono">
                  Upcoming Google Calendar Meetings ({calendarEvents.length})
                </h4>

                {calendarEvents.length === 0 ? (
                  <div className="p-8 text-center border border-dashed border-border rounded-xl space-y-2">
                    <Calendar className="w-8 h-8 text-muted-foreground/40 mx-auto" />
                    <p className="text-xs text-muted-foreground">
                      No upcoming meetings found on your connected Google Calendar.
                    </p>
                    <p className="text-[11px] text-muted-foreground">
                      When events with Google Meet, Zoom, or Teams links are scheduled, they will appear here.
                    </p>
                  </div>
                ) : (
                  calendarEvents.map((evt) => {
                    const startDate = new Date(evt.scheduled_start);
                    const endDate = new Date(evt.scheduled_end);
                    const formattedDate = startDate.toLocaleDateString(undefined, {
                      weekday: 'short',
                      month: 'short',
                      day: 'numeric',
                    });
                    const formattedTime = `${startDate.toLocaleTimeString([], {
                      hour: '2-digit',
                      minute: '2-digit',
                    })} – ${endDate.toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' })}`;

                    return (
                      <div
                        key={evt.id}
                        className="p-4 rounded-xl bg-card border border-border/80 hover:border-primary/40 transition-all flex items-start justify-between gap-4 shadow-xs"
                      >
                        <div className="space-y-1.5 min-w-0 flex-1">
                          <div className="flex items-center gap-2 flex-wrap">
                            <Badge
                              variant="outline"
                              className="text-[10px] uppercase font-mono tracking-wider py-0 px-2 bg-primary/5 text-primary border-primary/30"
                            >
                              {evt.provider.replace('_', ' ')}
                            </Badge>
                            {evt.recurrence_rule && (
                              <Badge
                                variant="secondary"
                                className="text-[10px] py-0 px-2 text-muted-foreground"
                              >
                                {evt.recurrence_rule}
                              </Badge>
                            )}
                          </div>

                          <h4 className="text-sm font-bold text-foreground truncate">{evt.title}</h4>

                          <div className="flex items-center gap-4 text-xs text-muted-foreground flex-wrap">
                            <div className="flex items-center gap-1.5">
                              <Clock className="w-3.5 h-3.5 text-muted-foreground" />
                              <span>
                                {formattedDate}, {formattedTime}
                              </span>
                            </div>
                            {evt.participants.length > 0 && (
                              <div className="flex items-center gap-1.5">
                                <Users className="w-3.5 h-3.5 text-muted-foreground" />
                                <span>{evt.participants.join(', ')}</span>
                              </div>
                            )}
                          </div>

                          {evt.meeting_url && (
                            <div className="flex items-center gap-1 text-[11px] text-blue-500 hover:underline">
                              <Video className="w-3 h-3" />
                              <a
                                href={evt.meeting_url}
                                target="_blank"
                                rel="noreferrer"
                                className="truncate max-w-sm"
                              >
                                {evt.meeting_url}
                              </a>
                            </div>
                          )}
                        </div>

                        <Button
                          size="sm"
                          variant="outline"
                          onClick={() => handleImport(evt)}
                          disabled={importingId === evt.id}
                          className="text-xs shrink-0 gap-1.5 text-primary border-primary/30 hover:bg-primary/10"
                        >
                          <Plus className="w-3.5 h-3.5" />
                          <span>{importingId === evt.id ? 'Importing…' : 'Add to Relay'}</span>
                        </Button>
                      </div>
                    );
                  })
                )}
              </div>
            </div>
          )}
        </div>

        {/* Modal Footer */}
        <div className="p-4 border-t border-border/80 bg-muted/10 flex items-center justify-between text-xs text-muted-foreground">
          <span>Relay reads calendar events to prepare meeting notes without automatic recording.</span>
          <Button size="sm" variant="ghost" onClick={onClose}>
            Close
          </Button>
        </div>
      </div>
    </div>
  );
};
