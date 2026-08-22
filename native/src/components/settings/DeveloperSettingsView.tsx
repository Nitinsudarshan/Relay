import React, { useState, useEffect } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { DeveloperSettings } from '../../types';
import { Terminal, RefreshCw, Check } from 'lucide-react';
import { Switch } from '@/components/ui/switch';
import { Badge } from '@/components/ui/badge';

export const DeveloperSettingsView: React.FC = () => {
  const [devSettings, setDevSettings] = useState<DeveloperSettings>({ force_onboarding_on_launch: false });
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

      {/* Onboarding Replay Override Section */}
      <div className="p-5 rounded-xl border border-border/80 bg-card/60 backdrop-blur-xs space-y-4">
        <div className="flex items-start justify-between gap-4">
          <div className="space-y-1">
            <div className="flex items-center gap-2">
              <h3 className="text-sm font-semibold text-foreground">Show onboarding on every launch</h3>
              {savedFeedback && (
                <Badge variant="secondary" className="text-[10px] gap-1 bg-emerald-500/10 text-emerald-500 border-emerald-500/20">
                  <Check className="w-3 h-3" />
                  <span>Saved</span>
                </Badge>
              )}
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
    </div>
  );
};
