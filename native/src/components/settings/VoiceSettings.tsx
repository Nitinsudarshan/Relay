import React, { useCallback, useEffect, useRef, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import {
  CheckCircle2,
  AlertTriangle,
  FolderOpen,
  Play,
  RefreshCw,
  Square,
} from 'lucide-react';
import { Button } from '@/components/ui/button';
import type { PiperOrigin, TtsStatus } from '../../types';

/** How Relay found the executable, in words rather than a path. */
const ORIGIN_LABEL: Record<PiperOrigin, string> = {
  configured: 'You chose this',
  managed: "Found in Relay's voice folder",
  bundled: 'Shipped with Relay',
  system_path: 'Found on your system PATH',
};

interface VoiceSettingsProps {
  /** Rendered above the card; omit inside a section that already has one. */
  heading?: string;
  /** Called whenever readiness changes, so a parent can react. */
  onStatusChange?: (status: TtsStatus) => void;
}

/**
 * Local voice configuration — the answer to "how do I make Talkback speak?".
 *
 * A single reusable card rather than fields scattered through Settings:
 * every question a user has about spoken answers (is it on, which engine,
 * which voice, what is wrong, how do I fix it, does it sound right) is
 * answered in one place, backed by one `get_tts_status` call.
 *
 * Deliberately exposes no pipeline internals. Phrase length, queue depth
 * and synthesis timeouts are decisions with reasons recorded in
 * `docs/talkback/ARCHITECTURE.md`, not preferences.
 */
export const VoiceSettings: React.FC<VoiceSettingsProps> = ({
  heading,
  onStatusChange,
}) => {
  const [status, setStatus] = useState<TtsStatus | null>(null);
  const [loading, setLoading] = useState(true);
  const [busy, setBusy] = useState<string | null>(null);
  const [testError, setTestError] = useState<string | null>(null);
  const [playing, setPlaying] = useState(false);
  const audioRef = useRef<HTMLAudioElement | null>(null);

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
    // Stop any test playback if the user navigates away mid-sentence.
    return () => {
      audioRef.current?.pause();
      audioRef.current = null;
    };
  }, [refresh]);

  const apply = async (
    label: string,
    patch: { binaryPath?: string; voicePath?: string },
  ) => {
    setBusy(label);
    setTestError(null);
    try {
      const next = await invoke<TtsStatus>('set_tts_configuration', {
        binaryPath: patch.binaryPath ?? null,
        voicePath: patch.voicePath ?? null,
      });
      setStatus(next);
      onStatusChange?.(next);
    } catch (err) {
      setTestError(errorText(err));
    } finally {
      setBusy(null);
    }
  };

  const browse = async (which: 'binary' | 'voice') => {
    setBusy(which);
    try {
      const command =
        which === 'binary' ? 'browse_for_piper_binary' : 'browse_for_piper_voice';
      const picked = await invoke<string | null>(command);
      if (!picked) return;
      await apply(
        which,
        which === 'binary' ? { binaryPath: picked } : { voicePath: picked },
      );
    } catch (err) {
      setTestError(errorText(err));
    } finally {
      setBusy(null);
    }
  };

  const openFolder = async () => {
    setBusy('folder');
    try {
      // Creates the folders as a side effect, so "put files here" points
      // at somewhere that exists.
      await invoke<string>('prepare_tts_folders');
      await refresh();
    } catch (err) {
      setTestError(errorText(err));
    } finally {
      setBusy(null);
    }
  };

  const stopTest = () => {
    audioRef.current?.pause();
    audioRef.current = null;
    setPlaying(false);
  };

  const testVoice = async () => {
    stopTest();
    setBusy('test');
    setTestError(null);
    try {
      const wav = await invoke<string>('test_tts_voice');
      const audio = new Audio(`data:audio/wav;base64,${wav}`);
      audioRef.current = audio;
      audio.onended = () => setPlaying(false);
      audio.onerror = () => {
        setPlaying(false);
        setTestError('The voice synthesized but the audio could not be played.');
      };
      setPlaying(true);
      await audio.play();
    } catch (err) {
      setTestError(errorText(err));
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
      {heading && (
        <h3 className="text-sm font-semibold text-foreground">{heading}</h3>
      )}

      <div className="rounded-xl border border-border overflow-hidden">
        {/* Status header — the one line that answers "can it speak?". */}
        <div className="flex items-center justify-between gap-3 px-4 py-3 bg-muted/40 border-b border-border">
          <div className="flex items-center gap-2.5 min-w-0">
            {status.ready ? (
              <CheckCircle2 className="w-4 h-4 text-emerald-500 shrink-0" />
            ) : (
              <AlertTriangle className="w-4 h-4 text-amber-500 shrink-0" />
            )}
            <div className="min-w-0">
              <p className="text-xs font-semibold text-foreground">
                {status.ready ? 'Ready' : 'Not configured'}
              </p>
              <p className="text-[11px] text-muted-foreground truncate">
                {status.ready
                  ? 'Talkback can speak its answers.'
                  : 'Talkback will answer in text only.'}
              </p>
            </div>
          </div>
          <Button
            variant="ghost"
            size="icon"
            onClick={() => void refresh()}
            aria-label="Re-check voice setup"
            className="h-8 w-8 shrink-0"
          >
            <RefreshCw className="w-3.5 h-3.5" />
          </Button>
        </div>

        <dl className="divide-y divide-border">
          <Row label="Voice engine">
            <span className="text-xs text-foreground">
              {status.ready ? 'Local Piper' : 'None'}
            </span>
          </Row>

          <Row label="Program">
            {status.binaryPath ? (
              <div className="min-w-0">
                <p className="text-xs text-foreground truncate" title={status.binaryPath}>
                  {status.binaryPath}
                </p>
                {status.binaryOrigin && (
                  <p className="text-[10px] text-muted-foreground">
                    {ORIGIN_LABEL[status.binaryOrigin]}
                  </p>
                )}
              </div>
            ) : (
              <span className="text-xs text-muted-foreground">Not found</span>
            )}
            <Button
              variant="outline"
              size="sm"
              className="h-7 text-[11px] shrink-0"
              disabled={busy !== null}
              onClick={() => void browse('binary')}
            >
              Browse…
            </Button>
          </Row>

          <Row label="Voice">
            {status.availableVoices.length > 0 ? (
              <select
                value={status.voicePath ?? ''}
                onChange={(event) =>
                  void apply('voice', { voicePath: event.target.value })
                }
                aria-label="Voice model"
                disabled={busy !== null}
                className="flex-1 min-w-0 bg-background border border-input rounded-md px-2 py-1 text-xs outline-none focus-visible:ring-2 focus-visible:ring-ring"
              >
                <option value="">Choose a voice…</option>
                {status.availableVoices.map((voice) => (
                  <option key={voice.path} value={voice.path}>
                    {voice.label}
                    {voice.has_config ? '' : ' — missing .onnx.json'}
                  </option>
                ))}
              </select>
            ) : (
              <span className="text-xs text-muted-foreground">
                No voices found
              </span>
            )}
            <Button
              variant="outline"
              size="sm"
              className="h-7 text-[11px] shrink-0"
              disabled={busy !== null}
              onClick={() => void browse('voice')}
            >
              Browse…
            </Button>
          </Row>
        </dl>

        {/* Problems, in the backend's words — each one names its own fix. */}
        {status.problems.length > 0 && (
          <ul className="px-4 py-3 space-y-1.5 bg-amber-500/5 border-t border-amber-500/20">
            {status.problems.map((problem) => (
              <li
                key={problem}
                className="flex items-start gap-2 text-[11px] text-amber-700 dark:text-amber-400"
              >
                <AlertTriangle className="w-3 h-3 mt-0.5 shrink-0" />
                <span>{problem}</span>
              </li>
            ))}
          </ul>
        )}

        {testError && (
          <p className="px-4 py-2 text-[11px] text-destructive bg-destructive/10 border-t border-destructive/20">
            {testError}
          </p>
        )}

        <div className="flex flex-wrap items-center gap-2 px-4 py-3 border-t border-border">
          {playing ? (
            <Button variant="secondary" size="sm" className="h-7 gap-1.5 text-[11px]" onClick={stopTest}>
              <Square className="w-3 h-3" />
              Stop
            </Button>
          ) : (
            <Button
              size="sm"
              className="h-7 gap-1.5 text-[11px]"
              disabled={!status.ready || busy !== null}
              onClick={() => void testVoice()}
            >
              <Play className="w-3 h-3" />
              {busy === 'test' ? 'Speaking…' : 'Test voice'}
            </Button>
          )}
          <Button
            variant="outline"
            size="sm"
            className="h-7 gap-1.5 text-[11px]"
            disabled={busy !== null}
            onClick={() => void openFolder()}
          >
            <FolderOpen className="w-3 h-3" />
            Create voice folder
          </Button>
        </div>
      </div>

      {/* Setup instructions. Shown until it works, because that is exactly
          when they are needed and never afterwards. */}
      {!status.ready && (
        <details className="rounded-lg border border-border bg-muted/30 px-3 py-2.5" open>
          <summary className="text-xs font-medium text-foreground cursor-pointer">
            Setting up a local voice
          </summary>
          <ol className="mt-2 space-y-2 text-[11px] text-muted-foreground list-decimal list-inside leading-relaxed">
            <li>
              Download Piper from{' '}
              <span className="font-mono text-foreground">
                github.com/OHF-Voice/piper1-gpl
              </span>{' '}
              and put <span className="font-mono text-foreground">{status.executableName}</span>{' '}
              in:
              <code className="mt-1 block break-all rounded bg-background border border-border px-2 py-1 font-mono text-[10px] text-foreground">
                {status.installDir}
              </code>
            </li>
            <li>
              Download a voice — both the{' '}
              <span className="font-mono text-foreground">.onnx</span> model and its{' '}
              <span className="font-mono text-foreground">.onnx.json</span> file, which
              Piper needs together — and put them in:
              <code className="mt-1 block break-all rounded bg-background border border-border px-2 py-1 font-mono text-[10px] text-foreground">
                {status.voicesDir}
              </code>
            </li>
            <li>
              Press <span className="text-foreground">Test voice</span>. Relay finds both
              automatically — nothing else to configure.
            </li>
          </ol>
          <p className="mt-2 text-[11px] text-muted-foreground">
            Everything stays on your machine. Relay never uploads what it speaks.
          </p>
        </details>
      )}
    </section>
  );
};

const Row: React.FC<{ label: string; children: React.ReactNode }> = ({
  label,
  children,
}) => (
  <div className="flex items-center gap-3 px-4 py-2.5">
    <dt className="w-24 shrink-0 text-[11px] font-medium text-muted-foreground">
      {label}
    </dt>
    <dd className="flex flex-1 items-center gap-2 min-w-0">{children}</dd>
  </div>
);

/** Tauri `CommandError` is `{ code, message }`; anything else is best effort. */
const errorText = (err: unknown): string => {
  if (err && typeof err === 'object' && 'message' in err) {
    return String((err as { message: unknown }).message);
  }
  return String(err);
};
