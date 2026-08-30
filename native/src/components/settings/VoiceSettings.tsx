import React, { useCallback, useEffect, useRef, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import {
  CheckCircle2,
  AlertTriangle,
  Download,
  Play,
  Square,
  Volume2,
  Globe,
  Sparkles,
  BookOpen,
} from 'lucide-react';
import { Button } from '@/components/ui/button';
import { Badge } from '@/components/ui/badge';
import { VoiceLibraryModal } from './VoiceLibraryModal';
import type { InstallProgress, PiperOrigin, TtsStatus } from '../../types';

/** How Relay found the engine, in words rather than a path. Advanced only. */
const ORIGIN_LABEL: Record<PiperOrigin, string> = {
  configured: 'Set up by Relay',
  managed: "In Relay's voice folder",
  bundled: 'Shipped with Relay',
  system_path: 'Found on your system PATH',
};

/** "24 MB". Sizes are approximate and shown to set expectations, not to audit. */
const formatSize = (bytes: number): string => {
  if (!bytes) return '';
  if (bytes < 1024 * 1024) return `${Math.round(bytes / 1024)} KB`;
  return `${(bytes / (1024 * 1024)).toFixed(0)} MB`;
};

interface VoiceSettingsProps {
  heading?: string;
  onStatusChange?: (status: TtsStatus) => void;
}

/**
 * Local voice setup — curated neural voice library, running offline.
 *
 * The product question this answers is "make Relay speak", not "where did
 * you put piper.exe". Downloading the engine, fetching a voice, verifying
 * checksums and proving it can actually speak are Relay's job; the user
 * selects a voice and it runs with pinned security.
 */
export const VoiceSettings: React.FC<VoiceSettingsProps> = ({
  heading,
  onStatusChange,
}) => {
  const [status, setStatus] = useState<TtsStatus | null>(null);
  const [loading, setLoading] = useState(true);
  const [progress, setProgress] = useState<InstallProgress | null>(null);
  const [busy, setBusy] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [playing, setPlaying] = useState(false);
  const [showAdvanced, setShowAdvanced] = useState(false);
  const [libraryOpen, setLibraryOpen] = useState(false);
  const audioRef = useRef<HTMLAudioElement | null>(null);

  const installing = progress !== null;

  const refresh = useCallback(async () => {
    try {
      const next = await invoke<TtsStatus>('get_tts_status');
      setStatus(next);
      onStatusChange?.(next);
    } catch (err) {
      console.error('Could not read voice status', err);
    } finally {
      setLoading(false);
    }
  }, [onStatusChange]);

  useEffect(() => {
    void refresh();
    const unlisten = listen<InstallProgress>('voice-install-progress', (event) => {
      if (event.payload) setProgress(event.payload);
    });
    return () => {
      void unlisten.then((stop) => stop());
      audioRef.current?.pause();
      audioRef.current = null;
    };
  }, [refresh]);

  const stopTest = () => {
    audioRef.current?.pause();
    audioRef.current = null;
    setPlaying(false);
  };

  const setup = async (voiceId?: string) => {
    stopTest();
    setError(null);
    setProgress({
      stage: 'preparing',
      label: 'Preparing…',
      receivedBytes: 0,
      overall: 0,
    });
    try {
      const next = await invoke<TtsStatus>('install_local_voice', {
        voiceId: voiceId ?? null,
      });
      setStatus(next);
      onStatusChange?.(next);
    } catch (err) {
      setError(errorText(err));
    } finally {
      setProgress(null);
      void refresh();
    }
  };

  const cancel = async () => {
    try {
      await invoke('cancel_voice_install');
    } catch (err) {
      console.warn('Could not cancel voice setup', err);
    }
  };

  const testVoice = async () => {
    stopTest();
    setBusy('test');
    setError(null);
    try {
      const wav = await invoke<string>('test_tts_voice');
      const audio = new Audio(`data:audio/wav;base64,${wav}`);
      audioRef.current = audio;
      audio.onended = () => setPlaying(false);
      audio.onerror = () => {
        setPlaying(false);
        setError('The voice was generated but could not be played.');
      };
      setPlaying(true);
      await audio.play();
    } catch (err) {
      setError(errorText(err));
      setPlaying(false);
    } finally {
      setBusy(null);
    }
  };

  if (loading) {
    return (
      <div className="rounded-xl border border-border p-4 text-xs text-muted-foreground">
        Checking local voice…
      </div>
    );
  }

  if (!status) {
    return (
      <div className="rounded-xl border border-destructive/40 bg-destructive/10 p-4 text-xs text-destructive">
        Could not read the voice configuration.
      </div>
    );
  }

  return (
    <section className="space-y-3" data-testid="voice-settings">
      {heading && <h3 className="text-sm font-semibold text-foreground">{heading}</h3>}

      <div className="rounded-xl border border-border overflow-hidden">
        {installing ? (
          <InstallingPanel progress={progress} onCancel={() => void cancel()} />
        ) : status.ready ? (
          <ReadyPanel
            status={status}
            playing={playing}
            busy={busy}
            onTest={() => void testVoice()}
            onStopTest={stopTest}
            onChangeVoice={(id) => void setup(id)}
            onOpenLibrary={() => setLibraryOpen(true)}
          />
        ) : (
          <SetupPanel
            status={status}
            onSetup={() => void setup()}
            onOpenLibrary={() => setLibraryOpen(true)}
          />
        )}

        {error && (
          <p className="px-4 py-2.5 text-[11px] text-destructive bg-destructive/10 border-t border-destructive/20">
            {error}
          </p>
        )}
      </div>

      {/* Voice Library Modal */}
      {status.catalogue && status.catalogue.length > 0 && (
        <VoiceLibraryModal
          open={libraryOpen}
          onClose={() => setLibraryOpen(false)}
          catalogue={status.catalogue}
          activeVoiceId={selectedId(status) ?? null}
          onSelectVoice={async (voiceId) => {
            setLibraryOpen(false);
            await setup(voiceId);
          }}
          onTestVoice={async () => {
            await testVoice();
          }}
          onStopTest={stopTest}
          isPlayingTest={playing}
          busyVoiceId={busy}
        />
      )}

      {/* Implementation detail, on request only. */}
      {!installing && (
        <details
          className="rounded-lg border border-border bg-muted/30 px-3 py-2"
          open={showAdvanced}
          onToggle={(event) => setShowAdvanced(event.currentTarget.open)}
        >
          <summary className="text-[11px] font-medium text-muted-foreground cursor-pointer">
            Advanced
          </summary>
          {showAdvanced && (
            <dl className="mt-2 space-y-1.5 text-[10px] font-mono">
              <AdvancedRow label="Engine" value={status.ready ? 'Piper' : 'None'} />
              {status.engineVersion && (
                <AdvancedRow label="Version" value={status.engineVersion} />
              )}
              {status.binaryOrigin && (
                <AdvancedRow label="Source" value={ORIGIN_LABEL[status.binaryOrigin]} />
              )}
              {status.binaryPath && (
                <AdvancedRow label="Program" value={status.binaryPath} wrap />
              )}
              {status.voicePath && (
                <AdvancedRow label="Voice file" value={status.voicePath} wrap />
              )}
              <AdvancedRow label="Folder" value={status.voicesDir} wrap />
            </dl>
          )}
        </details>
      )}
    </section>
  );
};

/** Before setup: what this is, and one button. */
const SetupPanel: React.FC<{
  status: TtsStatus;
  onSetup: () => void;
  onOpenLibrary: () => void;
}> = ({ status, onSetup, onOpenLibrary }) => {
  const recommended = status.recommendedVoice;

  return (
    <div className="p-4 space-y-3">
      <div className="flex items-start gap-3">
        <Volume2 className="w-5 h-5 mt-0.5 shrink-0 text-muted-foreground" />
        <div className="space-y-1">
          <p className="text-sm font-semibold text-foreground">Make Relay speak</p>
          <p className="text-[11px] leading-relaxed text-muted-foreground">
            Talkback can read its answers aloud using a voice that runs entirely
            on this computer. Relay does not send the text it speaks to any
            service. One-time setup, then it works offline.
          </p>
        </div>
      </div>

      {status.canInstall && recommended ? (
        <>
          <div className="rounded-lg border border-border bg-muted/40 px-3 py-2.5">
            <p className="text-[10px] uppercase tracking-wide text-muted-foreground">
              Recommended voice
            </p>
            <p className="mt-0.5 text-xs font-medium text-foreground">
              {recommended.displayName}
            </p>
            <p className="text-[11px] text-muted-foreground">
              {recommended.description}
              {status.downloadBytes > 0 &&
                ` · about ${formatSize(status.downloadBytes)} to download`}
            </p>
          </div>

          <div className="flex flex-col sm:flex-row gap-2">
            <Button className="flex-1 gap-2" onClick={onSetup}>
              <Download className="w-4 h-4" />
              Download &amp; Set Up
            </Button>
            {status.catalogue && status.catalogue.length > 1 && (
              <Button variant="outline" onClick={onOpenLibrary} className="gap-1.5 text-xs">
                <Globe className="w-3.5 h-3.5" />
                Browse Catalogue ({status.catalogue.length})
              </Button>
            )}
          </div>
          <p className="text-[10px] text-center text-muted-foreground">
            You can change the voice afterwards.
          </p>
        </>
      ) : (
        <div className="rounded-lg border border-amber-500/30 bg-amber-500/10 px-3 py-2.5">
          <p className="flex items-center gap-1.5 text-[11px] font-semibold text-amber-700 dark:text-amber-400">
            <AlertTriangle className="w-3.5 h-3.5 shrink-0" />
            Automatic setup unavailable
          </p>
          <p className="mt-1 text-[11px] leading-relaxed text-muted-foreground">
            {status.installBlockedReason ??
              "Relay can't set up a voice on this computer."}{' '}
            Talkback still answers in text.
          </p>
        </div>
      )}
    </div>
  );
};

/** During setup: what is happening, how far along, and a way out. */
const InstallingPanel: React.FC<{
  progress: InstallProgress;
  onCancel: () => void;
}> = ({ progress, onCancel }) => {
  const itemFraction =
    progress.totalBytes && progress.totalBytes > 0
      ? progress.receivedBytes / progress.totalBytes
      : null;
  const isDownload =
    progress.stage === 'downloading_engine' || progress.stage === 'downloading_voice';

  return (
    <div className="p-4 space-y-3" data-testid="voice-installing">
      <p className="text-sm font-semibold text-foreground">Setting up local voice</p>

      <div className="space-y-1.5">
        <div className="flex items-baseline justify-between gap-2">
          <span className="text-[11px] text-foreground">
            {progress.item ?? progress.label}
          </span>
          {isDownload && itemFraction !== null && (
            <span className="text-[10px] font-mono text-muted-foreground">
              {Math.round(itemFraction * 100)}%
            </span>
          )}
        </div>
        <Meter
          fraction={isDownload ? (itemFraction ?? 0) : 1}
          indeterminate={!isDownload}
          label={progress.label}
        />
      </div>

      <div className="space-y-1.5">
        <div className="flex items-baseline justify-between gap-2">
          <span className="text-[10px] uppercase tracking-wide text-muted-foreground">
            Overall
          </span>
          <span className="text-[10px] font-mono text-muted-foreground">
            {Math.round(progress.overall * 100)}%
          </span>
        </div>
        <Meter fraction={progress.overall} label="Overall setup progress" />
      </div>

      <Button variant="outline" size="sm" className="w-full h-7 text-[11px]" onClick={onCancel}>
        Cancel
      </Button>
    </div>
  );
};

/** After setup: it works, here is how it sounds, here is how to change it. */
const ReadyPanel: React.FC<{
  status: TtsStatus;
  playing: boolean;
  busy: string | null;
  onTest: () => void;
  onStopTest: () => void;
  onChangeVoice: (id: string) => void;
  onOpenLibrary: () => void;
}> = ({ status, playing, busy, onTest, onStopTest, onChangeVoice, onOpenLibrary }) => {
  const current =
    status.catalogue.find((v) => v.installed && v.id === selectedId(status)) ??
    status.catalogue.find((v) => v.id === selectedId(status));

  return (
    <div>
      <div className="flex items-center gap-2.5 px-4 py-3 bg-emerald-500/10 border-b border-emerald-500/20">
        <CheckCircle2 className="w-4 h-4 text-emerald-500 shrink-0" />
        <div className="min-w-0 flex-1">
          <p className="text-xs font-semibold text-foreground">Local voice ready</p>
          <p className="text-[11px] text-muted-foreground truncate">
            {current?.displayName ?? status.voiceLabel ?? 'Installed'}
            {current?.recommended ? ' — Recommended' : ''}
          </p>
        </div>
      </div>

      <div className="p-4 space-y-3">
        {status.catalogue.length > 1 && (
          <div className="space-y-1.5">
            <div className="flex items-center justify-between">
              <span className="text-[11px] font-medium text-muted-foreground">Voice</span>
              <button
                type="button"
                onClick={onOpenLibrary}
                className="text-[11px] text-primary hover:underline flex items-center gap-1 font-medium"
              >
                <Globe className="w-3 h-3" />
                Browse Voice Library ({status.catalogue.length})
              </button>
            </div>
            <select
              value={selectedId(status) ?? ''}
              aria-label="Voice"
              onChange={(event) => onChangeVoice(event.target.value)}
              className="w-full bg-background border border-input rounded-md px-2 py-1.5 text-xs outline-none focus-visible:ring-2 focus-visible:ring-ring"
            >
              {status.catalogue.map((voice) => (
                <option key={voice.id} value={voice.id}>
                  {voice.displayName}
                  {voice.recommended ? ' — Recommended' : ''}
                  {voice.installed ? '' : ` (${formatSize(voice.downloadBytes)} download)`}
                </option>
              ))}
            </select>
          </div>
        )}

        <div className="flex gap-2">
          {playing ? (
            <Button
              variant="secondary"
              size="sm"
              className="flex-1 h-7 gap-1.5 text-[11px]"
              onClick={onStopTest}
            >
              <Square className="w-3 h-3 text-primary animate-pulse" />
              Stop
            </Button>
          ) : (
            <Button
              size="sm"
              className="flex-1 h-7 gap-1.5 text-[11px]"
              disabled={busy !== null}
              onClick={onTest}
            >
              <Play className="w-3 h-3" />
              {busy === 'test' ? 'Speaking…' : 'Test voice'}
            </Button>
          )}

          <Button
            variant="outline"
            size="sm"
            className="h-7 text-[11px] gap-1"
            onClick={onOpenLibrary}
          >
            <Globe className="w-3 h-3" />
            Library
          </Button>
        </div>

        <p className="text-[10px] leading-relaxed text-muted-foreground">
          Your voice is generated on this computer. Relay does not upload the text
          it speaks to a speech service. (Answers themselves come from whichever
          AI provider you have configured.)
        </p>
      </div>
    </div>
  );
};

/** Which catalogue voice the installed file corresponds to. */
const selectedId = (status: TtsStatus): string | undefined =>
  status.catalogue.find((v) => status.voiceLabel === v.id)?.id ??
  status.catalogue.find((v) => v.installed)?.id;

const Meter: React.FC<{
  fraction: number;
  label: string;
  indeterminate?: boolean;
}> = ({ fraction, label, indeterminate }) => (
  <div
    className="h-1.5 w-full rounded-full bg-muted overflow-hidden"
    role="progressbar"
    aria-label={label}
    aria-valuenow={indeterminate ? undefined : Math.round(fraction * 100)}
    aria-valuemin={0}
    aria-valuemax={100}
  >
    <div
      className={`h-full rounded-full bg-primary transition-[width] duration-300 ${
        indeterminate ? 'animate-pulse' : ''
      }`}
      style={{ width: `${Math.round(Math.min(1, Math.max(0, fraction)) * 100)}%` }}
    />
  </div>
);

const AdvancedRow: React.FC<{ label: string; value: string; wrap?: boolean }> = ({
  label,
  value,
  wrap,
}) => (
  <div className="flex gap-2">
    <dt className="w-20 shrink-0 text-muted-foreground">{label}</dt>
    <dd className={`text-foreground ${wrap ? 'break-all' : 'truncate'}`}>{value}</dd>
  </div>
);

/** Tauri `CommandError` is `{ code, message }`; anything else is best effort. */
const errorText = (err: unknown): string => {
  if (err && typeof err === 'object' && 'message' in err) {
    return String((err as { message: unknown }).message);
  }
  return String(err);
};
