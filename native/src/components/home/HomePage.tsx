import React, { useCallback, useEffect, useMemo, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';

import { HomeCaptureShortcuts } from './HomeCaptureShortcuts';
import { HomeLibraryStats } from './HomeLibraryStats';
import { HomeRecentActivity } from './HomeRecentActivity';
import { HomeSystemPanel } from './HomeSystemPanel';
import {
  buildHomeStats,
  buildHomeVitals,
  buildRecentActivity,
  emptySnapshot,
  type HomeSnapshot,
  type HomeSurface,
} from './homeStats';

import type { CaptureMethod } from '@/components/captures/CaptureHubPage';
import type { SettingsSection } from '@/components/settings/ProviderSettings';
import type {
  AppSettings,
  CaptureBridgeStatus,
  KnowledgeTelemetrySnapshot,
  MeetingSession,
  RelayAccount,
  Scribble,
  VaultFile,
  VaultLocationInfo,
  VaultNote,
} from '@/types';

export interface HomePageProps {
  account: RelayAccount | null;
  /** Read from `App`'s already-loaded settings rather than fetched a second time. */
  settings: AppSettings | null;
  appVersion: string;
  onNavigate: (surface: HomeSurface) => void;
  /** Opens `Captures › Capture` on the named mode. */
  onStartCapture: (method: CaptureMethod) => void;
  onOpenSettings: (section: SettingsSection) => void;
  onOpenChangelog: () => void;
}

/**
 * Relay's landing surface.
 *
 * It owns the reads and nothing else: the vault lists come in here, the
 * derivations happen in `homeStats.ts`, and the four sections below are handed
 * finished props. Home deliberately performs no capture of its own — every card
 * hands the user to the surface that does, so there is one implementation of each
 * capture mode rather than two.
 */
export const HomePage: React.FC<HomePageProps> = ({
  account,
  settings,
  appVersion,
  onNavigate,
  onStartCapture,
  onOpenSettings,
  onOpenChangelog,
}) => {
  const [snapshot, setSnapshot] = useState<HomeSnapshot>(emptySnapshot);
  const [vaultLocation, setVaultLocation] = useState<VaultLocationInfo | null>(null);
  const [bridge, setBridge] = useState<CaptureBridgeStatus | null>(null);
  const [loading, setLoading] = useState(true);
  /** Frozen per load so every relative timestamp on the page agrees. */
  const [nowMs, setNowMs] = useState(() => Date.now());

  const load = useCallback(async () => {
    // Each read is independent and local; a surface that fails degrades to zero
    // rather than blanking the page, because a missing meeting index should not
    // hide the voice notes that did load.
    const [voiceNotes, scribbles, meetings, vaultFiles, captures, telemetry, location, bridgeStatus] =
      await Promise.all([
        invoke<VaultNote[]>('get_voice_notes').catch(() => []),
        invoke<Scribble[]>('get_scribbles').catch(() => []),
        invoke<MeetingSession[]>('list_meetings_v2').catch(() => []),
        invoke<VaultFile[]>('get_vault_files').catch(() => []),
        invoke<VaultFile[]>('get_captures').catch(() => []),
        invoke<KnowledgeTelemetrySnapshot>('get_knowledge_telemetry').catch(() => null),
        invoke<VaultLocationInfo>('get_vault_location').catch(() => null),
        invoke<CaptureBridgeStatus>('get_capture_bridge_status').catch(() => null),
      ]);

    setSnapshot({
      voiceNotes: voiceNotes ?? [],
      scribbles: scribbles ?? [],
      meetings: meetings ?? [],
      // `get_vault_files` spans both trees; captures are counted on their own.
      files: (vaultFiles ?? []).filter((f) => !f.capture),
      captures: captures ?? [],
      telemetry,
    });
    setVaultLocation(location);
    setBridge(bridgeStatus);
    setNowMs(Date.now());
    setLoading(false);
  }, []);

  useEffect(() => {
    load();
  }, [load]);

  // Anything the backend saves while Home is open changes a number on it.
  useEffect(() => {
    const subscriptions = [
      listen('scribble-saved', () => load()),
      listen('scribble-enriched', () => load()),
      listen('voice-note-saved', () => load()),
      listen('meeting-session-state-changed', () => load()),
      listen('capture-processed', () => load()),
    ];

    return () => {
      subscriptions.forEach((s) => s.then((unlisten) => unlisten()));
    };
  }, [load]);

  const stats = useMemo(() => buildHomeStats(snapshot, nowMs), [snapshot, nowMs]);
  const vitals = useMemo(() => buildHomeVitals(snapshot), [snapshot]);
  const activity = useMemo(() => buildRecentActivity(snapshot, 7), [snapshot]);

  return (
    <div className="flex-1 flex flex-col gap-5 min-w-0 overflow-y-auto pb-10">
      <HomeCaptureShortcuts
        dictationHotkey={settings?.hotkeys?.dictation_hotkey ?? null}
        bridgeRunning={bridge ? bridge.running : null}
        bridgePort={bridge?.port ?? null}
        onNavigate={onNavigate}
        onStartCapture={onStartCapture}
      />

      <div className="grid grid-cols-1 xl:grid-cols-3 gap-5 min-w-0">
        <div className="xl:col-span-2 min-w-0">
          <HomeLibraryStats
            stats={stats}
            vitals={vitals}
            loading={loading}
            onNavigate={onNavigate}
          />
        </div>

        <div className="min-w-0">
          <HomeRecentActivity
            items={activity}
            loading={loading}
            nowMs={nowMs}
            onNavigate={onNavigate}
          />
        </div>
      </div>

      <HomeSystemPanel
        settings={settings}
        account={account}
        vaultLocation={vaultLocation}
        appVersion={appVersion}
        onOpenSettings={onOpenSettings}
        onOpenChangelog={onOpenChangelog}
      />
    </div>
  );
};
