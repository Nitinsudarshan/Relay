import React, { useState, useEffect } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { DeveloperSettings, NotificationSurfaceMode } from '../../types';
import { Terminal, RefreshCw, Check, Bell, CalendarClock, Play, SearchCode, Monitor, BellRing, Layers } from 'lucide-react';
import { Button } from '@/components/ui/button';
import { Switch } from '@/components/ui/switch';
import { Badge } from '@/components/ui/badge';

export const DeveloperSettingsView: React.FC = () => {
  const [devSettings, setDevSettings] = useState<DeveloperSettings>({
    force_onboarding_on_launch: false,
    notification_surface_mode: 'both',
  });
  const [loading, setLoading] = useState(true);
  const [saving, setSaving] = useState(false);
  const [savedFeedback, setSavedFeedback] = useState(false);

  useEffect(() => {
    loadDevSettings();
  }, []);

  const loadDevSettings = async () => {
    try {
      setLoading(true);
      const res = await invoke<DeveloperSettings>('get_developer_settings');
      setDevSettings(res);
    } catch (err) {
      console.error('Failed to load developer settings:', err);
    } finally {
      setLoading(false);
    }
  };

  const handleToggleForceOnboarding = async (checked: boolean) => {
    try {
      setSaving(true);
      const res = await invoke<DeveloperSettings>('set_developer_force_onboarding', {
        enabled: checked,
      });
      setDevSettings(res);
      setSavedFeedback(true);
      setTimeout(() => setSavedFeedback(false), 2000);
    } catch (err) {
      console.error('Failed to update developer onboarding setting:', err);
    } finally {
      setSaving(false);
    }
  };

  const handleSetSurfaceMode = async (mode: NotificationSurfaceMode) => {
    try {
      setSaving(true);
      const res = await invoke<DeveloperSettings>('set_developer_notification_surface_mode', {
        mode,
      });
      setDevSettings(res);
      setSavedFeedback(true);
      setTimeout(() => setSavedFeedback(false), 2000);
    } catch (err) {
      console.error('Failed to update notification surface mode:', err);
    } finally {
      setSaving(false);
    }
  };

  const handleTriggerMockReminder = async (kind: string) => {
    try {
      await invoke('trigger_mock_meeting_reminder', { kind });
    } catch (err) {
      console.error(`Failed to trigger mock reminder (${kind}):`, err);
    }
  };

  const handleCheckDetection = async () => {
    try {
      const res = await invoke('debug_detect_conferencing_windows');
      console.log('Window-detection signal (raw, unresolved):', res);
      alert(JSON.stringify(res, null, 2));
    } catch (err) {
      console.error('Failed to check detection:', err);
      alert('Error: ' + err);
    }
  };

  return (
    <div className="space-y-6 animate-in fade-in-50 duration-200">
      <div className="border-b border-border/40 pb-5">
        <div className="flex items-center gap-3 mb-1.5">
          <div className="flex items-center gap-2">
            <Terminal className="w-5 h-5 text-amber-500" />
            <h2 className="text-xl font-bold tracking-tight text-foreground">Developer Settings</h2>
          </div>
          <Badge variant="outline" className="text-[10px] font-mono border-amber-500/30 text-amber-500 bg-amber-500/5 uppercase">
            Internal / Testing
          </Badge>
        </div>
        <p className="text-xs text-muted-foreground leading-relaxed max-w-2xl">
          Diagnostic overrides for testing Relay lifecycle, transitions, and onboarding workflows.
          <strong className="text-foreground ml-1">These switches do not delete your saved notes, scribbles, or authentication credentials.</strong>
        </p>
      </div>

      {/* Notification Surface Mode Selector */}
      <div className="p-5 rounded-lg border border-border/80 bg-card/60 backdrop-blur-xs space-y-4">
        <div className="space-y-1">
          <div className="flex items-center gap-2">
            <h3 className="text-sm font-semibold text-foreground flex items-center gap-2">
              <Layers className="w-4 h-4 text-primary" />
              Meeting Notification Surface Mode
            </h3>
            {savedFeedback && (
              <Badge variant="secondary" className="text-[10px] gap-1 bg-emerald-500/10 text-emerald-500 border-emerald-500/20">
                <Check className="w-3 h-3" />
                <span>Saved</span>
              </Badge>
            )}
          </div>
          <p className="text-xs text-muted-foreground leading-relaxed max-w-xl">
            Choose which surfaces display meeting reminders when triggered.
          </p>
        </div>

        <div className="grid grid-cols-1 sm:grid-cols-3 gap-2.5 pt-1">
          {/* Both (Default) */}
          <button
            type="button"
            onClick={() => handleSetSurfaceMode('both')}
            disabled={loading || saving}
            className={`p-3 rounded-lg border text-left transition-all flex flex-col justify-between gap-2 ${
              devSettings.notification_surface_mode === 'both'
                ? 'border-primary bg-primary/10 text-foreground ring-1 ring-primary/40 shadow-xs'
                : 'border-border/60 bg-background/50 hover:bg-secondary/40 text-muted-foreground hover:text-foreground'
            }`}
          >
            <div className="flex items-center justify-between w-full">
              <div className="flex items-center gap-2">
                <Layers className="w-4 h-4 text-primary" />
                <span className="text-xs font-semibold">Both (Default)</span>
              </div>
              {devSettings.notification_surface_mode === 'both' && (
                <div className="w-2 h-2 rounded-full bg-primary" />
              )}
            </div>
            <p className="text-[11px] leading-tight text-muted-foreground">
              Shows the app overlay window plus native Windows OS toast.
            </p>
          </button>

          {/* Tauri Overlay Only */}
          <button
            type="button"
            onClick={() => handleSetSurfaceMode('tauri')}
            disabled={loading || saving}
            className={`p-3 rounded-lg border text-left transition-all flex flex-col justify-between gap-2 ${
              devSettings.notification_surface_mode === 'tauri'
                ? 'border-primary bg-primary/10 text-foreground ring-1 ring-primary/40 shadow-xs'
                : 'border-border/60 bg-background/50 hover:bg-secondary/40 text-muted-foreground hover:text-foreground'
            }`}
          >
            <div className="flex items-center justify-between w-full">
              <div className="flex items-center gap-2">
                <Monitor className="w-4 h-4 text-blue-500" />
                <span className="text-xs font-semibold">Tauri Overlay Only</span>
              </div>
              {devSettings.notification_surface_mode === 'tauri' && (
                <div className="w-2 h-2 rounded-full bg-primary" />
              )}
            </div>
            <p className="text-[11px] leading-tight text-muted-foreground">
              Only shows the floating desktop overlay card window.
            </p>
          </button>

          {/* System Notification Only */}
          <button
            type="button"
            onClick={() => handleSetSurfaceMode('system')}
            disabled={loading || saving}
            className={`p-3 rounded-lg border text-left transition-all flex flex-col justify-between gap-2 ${
              devSettings.notification_surface_mode === 'system'
                ? 'border-primary bg-primary/10 text-foreground ring-1 ring-primary/40 shadow-xs'
                : 'border-border/60 bg-background/50 hover:bg-secondary/40 text-muted-foreground hover:text-foreground'
            }`}
          >
            <div className="flex items-center justify-between w-full">
              <div className="flex items-center gap-2">
                <BellRing className="w-4 h-4 text-amber-500" />
                <span className="text-xs font-semibold">System Toast Only</span>
              </div>
              {devSettings.notification_surface_mode === 'system' && (
                <div className="w-2 h-2 rounded-full bg-primary" />
              )}
            </div>
            <p className="text-[11px] leading-tight text-muted-foreground">
              Only dispatches native Windows OS toast notifications.
            </p>
          </button>
        </div>
      </div>

      {/* Onboarding Replay Override Section */}
      <div className="p-5 rounded-lg border border-border/80 bg-card/60 backdrop-blur-xs space-y-4">
        <div className="flex items-start justify-between gap-4">
          <div className="space-y-1">
            <div className="flex items-center gap-2">
              <h3 className="text-sm font-semibold text-foreground">Show onboarding on every launch</h3>
            </div>
            <p className="text-xs text-muted-foreground leading-relaxed max-w-xl">
              Development testing only. Replays onboarding every time Relay starts without deleting your saved profile or data.
            </p>
            <div className="pt-1 text-[11px] text-muted-foreground/80">
              When enabled, Relay will present the 2-step onboarding modal (Personalization name prompt and Google/Local selection) on every startup for iterative UX testing.
            </div>
          </div>

          <div className="flex items-center gap-2 shrink-0 pt-0.5">
            {saving && <RefreshCw className="w-3.5 h-3.5 animate-spin text-muted-foreground" />}
            <Switch
              checked={devSettings.force_onboarding_on_launch}
              onCheckedChange={handleToggleForceOnboarding}
              disabled={loading || saving}
            />
          </div>
        </div>
      </div>

      {/* Mock Meeting Reminders Section */}
      <div className="p-5 rounded-lg border border-border/80 bg-card/60 backdrop-blur-xs space-y-4">
        <div className="space-y-1">
          <h3 className="text-sm font-semibold text-foreground flex items-center gap-2">
            <Bell className="w-4 h-4 text-primary" />
            Test Meeting Reminders
          </h3>
          <p className="text-xs text-muted-foreground leading-relaxed max-w-xl">
            Simulate meeting reminder payloads to test the popup and OS notification system without actually scheduling or joining a meeting.
          </p>
        </div>
        
        <div className="flex flex-wrap items-center gap-3 pt-2">
          <Button
            variant="outline"
            size="sm"
            onClick={handleCheckDetection}
            className="text-xs h-8 gap-1.5 border-purple-500/30 hover:bg-purple-500/10 hover:text-purple-500"
          >
            <SearchCode className="w-3.5 h-3.5" />
            Check Window Detection
          </Button>
          <Button
            variant="outline"
            size="sm"
            onClick={() => handleTriggerMockReminder('upcoming')}
            className="text-xs h-8 gap-1.5 border-blue-500/30 hover:bg-blue-500/10 hover:text-blue-500"
          >
            <CalendarClock className="w-3.5 h-3.5" />
            T-2 Min (Upcoming)
          </Button>
          <Button
            variant="outline"
            size="sm"
            onClick={() => handleTriggerMockReminder('unrecorded')}
            className="text-xs h-8 gap-1.5 border-orange-500/30 hover:bg-orange-500/10 hover:text-orange-500"
          >
            <Play className="w-3.5 h-3.5" />
            T+5 Min (Unrecorded)
          </Button>
          <Button
            variant="outline"
            size="sm"
            onClick={() => handleTriggerMockReminder('detected')}
            className="text-xs h-8 gap-1.5 border-emerald-500/30 hover:bg-emerald-500/10 hover:text-emerald-500"
          >
            <Bell className="w-3.5 h-3.5" />
            Ad-hoc (Detected)
          </Button>
        </div>
      </div>
    </div>
  );
};
