import React, { useState } from 'react';
import { CalendarMeetingEvent, CalendarConnectionStatus } from '../../types';
import { Button } from '@/components/ui/button';
import { Badge } from '@/components/ui/badge';
import {
  Calendar,
  RefreshCw,
  X,
  Video,
  Clock,
  Users,
  Plus,
  Unlink,
  AlertCircle,
  ShieldCheck,
} from 'lucide-react';

interface CalendarSyncModalProps {
  isOpen: boolean;
  onClose: () => void;
  authStatus: CalendarConnectionStatus;
  calendarEvents: CalendarMeetingEvent[];
  onConnectGoogle: () => Promise<void>;
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
  const [errorMsg, setErrorMsg] = useState<string | null>(null);

  if (!isOpen) return null;

  const handleConnect = async () => {
    setConnecting(true);
    setErrorMsg(null);
    try {
      await onConnectGoogle();
    } catch (err: unknown) {
      console.error('Google Calendar connect error:', err);
      setErrorMsg(
        typeof err === 'string'
          ? err
          : (err as { message?: string })?.message || 'Failed to connect Google Calendar.'
      );
    } finally {
      setConnecting(false);
    }
  };

  const handleDisconnect = async () => {
    setDisconnecting(true);
    setErrorMsg(null);
    try {
      await onDisconnectGoogle();
    } catch (err: unknown) {
      console.error('Google Calendar disconnect error:', err);
      setErrorMsg(
        typeof err === 'string'
          ? err
          : (err as { message?: string })?.message || 'Failed to disconnect Google Calendar.'
      );
    } finally {
      setDisconnecting(false);
    }
  };

  const handleSync = async () => {
    setSyncing(true);
    setErrorMsg(null);
    try {
      await onSyncNow();
    } catch (err: unknown) {
      console.error('Google Calendar sync error:', err);
      setErrorMsg(
        typeof err === 'string'
          ? err
          : (err as { message?: string })?.message || 'Failed to sync Google Calendar events.'
      );
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

  const isConnected = authStatus.connected && authStatus.status === 'connected';
  const isAuthError = authStatus.status === 'auth_error';
  const isNotConfigured = authStatus.status === 'not_configured';

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
                {isConnected
                  ? `Connected as ${authStatus.account_email || 'Google User'}`
                  : isAuthError
                  ? 'Calendar connection needs re-authorization.'
                  : 'Connect your calendar to detect upcoming meetings and associate meeting context with Relay.'}
              </p>
            </div>
          </div>

          <div className="flex items-center gap-2">
            {isConnected && (
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

        {/* User-facing Alert if error occurs */}
        {errorMsg && (
          <div className="mx-6 mt-4 p-3 rounded-lg bg-destructive/10 border border-destructive/30 flex items-center gap-2 text-xs text-destructive">
            <AlertCircle className="w-4 h-4 shrink-0" />
            <span>{errorMsg}</span>
          </div>
        )}

        {/* Content Body */}
        <div className="p-6 space-y-4 overflow-y-auto flex-1">
          {isNotConfigured ? (
            /* 1. NOT CONFIGURED FOR BUILD STATE */
            <div className="p-6 rounded-xl border border-dashed border-border text-center space-y-3 bg-muted/5">
              <div className="w-10 h-10 rounded-full bg-amber-500/10 border border-amber-500/20 text-amber-500 flex items-center justify-center mx-auto">
                <Calendar className="w-5 h-5" />
              </div>
              <div className="space-y-1 max-w-md mx-auto">
                <h3 className="text-sm font-bold text-foreground">Google Calendar isn't available yet</h3>
                <p className="text-xs text-muted-foreground leading-relaxed">
                  Google Calendar hasn't been configured for this Relay installation.
                </p>
              </div>
            </div>
          ) : isAuthError ? (
            /* 2. AUTH ERROR / RECONNECT REQUIRED STATE */
            <div className="p-6 rounded-xl border border-destructive/30 bg-destructive/5 space-y-4 text-center">
              <div className="w-10 h-10 rounded-full bg-destructive/10 border border-destructive/30 text-destructive flex items-center justify-center mx-auto">
                <AlertCircle className="w-5 h-5" />
              </div>
              <div className="space-y-1 max-w-md mx-auto">
                <h3 className="text-sm font-bold text-foreground">Google Calendar needs attention</h3>
                <p className="text-xs text-muted-foreground leading-relaxed">
                  Relay couldn't access your Google Calendar. Please reconnect your account to resume syncing meetings.
                </p>
              </div>
              <div className="flex items-center justify-center gap-3 pt-1">
                <Button
                  onClick={handleConnect}
                  disabled={connecting}
                  className="text-xs bg-blue-600 hover:bg-blue-700 text-white gap-2 shadow-xs px-5 py-2 h-9"
                >
                  <RefreshCw className={`w-3.5 h-3.5 ${connecting ? 'animate-spin' : ''}`} />
                  <span>{connecting ? 'Waiting for Google Authorization…' : 'Reconnect Google Calendar'}</span>
                </Button>
                <Button
                  variant="outline"
                  size="sm"
                  onClick={handleDisconnect}
                  disabled={disconnecting}
                  className="text-xs text-destructive hover:bg-destructive/10 h-9"
                >
                  Disconnect
                </Button>
              </div>
            </div>
          ) : !isConnected ? (
            /* 3. CLEAN DISCONNECTED STATE */
            <div className="p-6 rounded-xl border border-dashed border-border text-center space-y-4 bg-muted/5">
              <div className="w-12 h-12 rounded-full bg-blue-500/10 border border-blue-500/20 text-blue-500 flex items-center justify-center mx-auto">
                <Calendar className="w-6 h-6" />
              </div>
              <div className="space-y-1.5 max-w-md mx-auto">
                <h3 className="text-sm font-bold text-foreground">Connect Google Calendar</h3>
                <p className="text-xs text-muted-foreground leading-relaxed">
                  Connect your calendar to let Relay detect upcoming meetings and associate meeting context with Relay.
                </p>
              </div>

              <div className="pt-2">
                <Button
                  onClick={handleConnect}
                  disabled={connecting}
                  className="text-xs bg-blue-600 hover:bg-blue-700 text-white gap-2 shadow-xs px-6 py-2.5 h-9"
                >
                  <Calendar className="w-4 h-4" />
                  <span>{connecting ? 'Waiting for Google Authorization…' : 'Connect Google Calendar'}</span>
                </Button>
              </div>

              <div className="pt-3 max-w-sm mx-auto flex items-center justify-center gap-1.5 text-[11px] text-muted-foreground/80">
                <ShieldCheck className="w-3.5 h-3.5 text-emerald-500 shrink-0" />
                <span>Relay only reads your calendar to identify relevant meetings. Your meeting data remains local according to Relay's privacy model.</span>
              </div>
            </div>
          ) : (
            /* 4. CONNECTED STATE */
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
                      Connected to your Google Calendar
                      {authStatus.account_email ? ` (${authStatus.account_email})` : ''}
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
