import React, { useCallback, useEffect, useRef, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { Square, Loader2, Mic, Volume2, Pause, Play, AlertTriangle } from 'lucide-react';
import { AudioLevels, MeetingSession } from '../../types';

// Matching DictationPill waveform bar parameters
const WAVEFORM_BAR_COUNT = 12;
const SILENT_LEVEL_HISTORY = new Array(WAVEFORM_BAR_COUNT).fill(0);

/** States in which a session is no longer occupying the pill. */
const TERMINAL_STATES = ['COMPLETED', 'IDLE', 'INTERRUPTED', 'RECOVERED', 'ERROR'];

/**
 * How often the pill reconciles against the backend.
 *
 * The pill is a subscriber, never the owner, of recording state — but events
 * alone are not enough to stay correct: the overlay's webview can be created
 * after a session has already started (missing the start event) and it survives
 * across sessions, so a single missed event used to leave it showing a stale
 * session whose timer kept counting between meetings. Reconciling on a timer
 * makes every such divergence self-healing within one interval.
 */
const RECONCILE_INTERVAL_MS = 1000;
const TIMER_TICK_MS = 250;

export const MeetingRecordingOverlay: React.FC = () => {
  const [session, setSession] = useState<MeetingSession | null>(null);
  const [elapsedSec, setElapsedSec] = useState<number>(0);
  const [micHistory, setMicHistory] = useState<number[]>(SILENT_LEVEL_HISTORY);
  const [sysHistory, setSysHistory] = useState<number[]>(SILENT_LEVEL_HISTORY);
  const [isBusy, setIsBusy] = useState<boolean>(false);

  /**
   * Backend-reported recorded duration plus the local instant it arrived.
   * Elapsed time is interpolated from this rather than from `started_at`, so the
   * display excludes paused intervals and can never drift away from the
   * recording itself.
   */
  const durationAnchor = useRef<{ seconds: number; at: number } | null>(null);

  const applySession = useCallback((next: MeetingSession | null) => {
    if (!next || TERMINAL_STATES.includes(next.state)) {
      durationAnchor.current = null;
      setSession(null);
      setElapsedSec(0);
      setMicHistory(SILENT_LEVEL_HISTORY);
      setSysHistory(SILENT_LEVEL_HISTORY);
      return;
    }

    durationAnchor.current = {
      seconds: next.duration_seconds || 0,
      at: performance.now(),
    };
    setSession(next);
  }, []);

  // Reconcile with the backend on mount and on an interval, and react
  // immediately to state changes in between.
  useEffect(() => {
    let cancelled = false;

    const reconcile = async () => {
      try {
        const active = await invoke<MeetingSession | null>('get_active_meeting_v2');
        if (!cancelled) {
          applySession(active);
        }
      } catch (err) {
        console.error('Failed to reconcile active meeting:', err);
      }
    };

    reconcile();
    const poll = setInterval(reconcile, RECONCILE_INTERVAL_MS);

    const unlistenState = listen<MeetingSession>('meeting-session-state-changed', (event) => {
      applySession(event.payload);
    });

    const unlistenLevels = listen<AudioLevels>('meeting-audio-levels', (event) => {
      const { mic_level, sys_level } = event.payload;
      const mic = Math.min(1.0, mic_level || 0);
      const sys = Math.min(1.0, sys_level || 0);

      // Mic wave: newest sample enters at left [0] and scrolls left-to-right.
      // Sys wave: newest sample enters at right [last] and scrolls right-to-left.
      setMicHistory((prev) => [mic, ...prev.slice(0, prev.length - 1)]);
      setSysHistory((prev) => [...prev.slice(1), sys]);
    });

    return () => {
      cancelled = true;
      clearInterval(poll);
      unlistenState.then((f) => f());
      unlistenLevels.then((f) => f());
    };
  }, [applySession]);

  // Interpolate between reconciliations, and only while actually recording.
  const isRecording = session?.state === 'RECORDING';
  useEffect(() => {
    const update = () => {
      const anchor = durationAnchor.current;
      if (!anchor) {
        setElapsedSec(0);
        return;
      }
      const drift = isRecording ? (performance.now() - anchor.at) / 1000 : 0;
      setElapsedSec(Math.max(0, Math.floor(anchor.seconds + drift)));
    };

    update();
    if (!isRecording) {
      return;
    }
    const timer = setInterval(update, TIMER_TICK_MS);
    return () => clearInterval(timer);
  }, [isRecording, session?.id, session?.state, session?.duration_seconds]);

  const formatTimer = (totalSeconds: number) => {
    const hrs = Math.floor(totalSeconds / 3600);
    const mins = Math.floor((totalSeconds % 3600) / 60);
    const secs = totalSeconds % 60;
    if (hrs > 0) {
      return `${String(hrs).padStart(2, '0')}:${String(mins).padStart(2, '0')}:${String(secs).padStart(2, '0')}`;
    }
    return `${String(mins).padStart(2, '0')}:${String(secs).padStart(2, '0')}`;
  };

  const isPaused = session?.state === 'PAUSED';
  const isFinalizing = session?.state === 'STOPPING' || session?.state === 'FINALIZING';

  const handleTogglePause = async () => {
    if (!session || isBusy || isFinalizing) return;
    setIsBusy(true);
    try {
      // Always name the session: a pill that outlived its meeting must not
      // pause or stop whatever is recording now.
      const command = isPaused ? 'resume_meeting_v2' : 'pause_meeting_v2';
      const updated = await invoke<MeetingSession>(command, { sessionId: session.id });
      applySession(updated);
    } catch (err) {
      console.error('Failed to toggle meeting pause:', err);
    } finally {
      setIsBusy(false);
    }
  };

  const handleStop = async () => {
    if (!session || isBusy || isFinalizing) return;
    setIsBusy(true);
    try {
      await invoke('stop_meeting_v2', { sessionId: session.id });
      applySession(null);
    } catch (err) {
      console.error('Failed to stop meeting recording:', err);
    } finally {
      setIsBusy(false);
    }
  };

  const statusLabel = isFinalizing ? 'Finalizing' : isPaused ? 'Paused' : 'Recording';
  const statusColor = isPaused
    ? 'text-amber-500 dark:text-amber-400'
    : 'text-red-500 dark:text-red-400';
  const dotColor = isPaused ? 'bg-amber-500' : 'bg-red-500';
  const waveMic = isPaused ? SILENT_LEVEL_HISTORY : micHistory;
  const waveSys = isPaused ? SILENT_LEVEL_HISTORY : sysHistory;

  return (
    <div className="w-full h-full flex items-center justify-center p-1 bg-transparent select-none">
      <div className="inline-flex items-center gap-3 px-4 h-[44px] rounded-lg bg-white dark:bg-[#171717] border border-slate-200 dark:border-[#262626] shadow-[0_16px_40px_rgba(0,0,0,0.5)] text-slate-900 dark:text-neutral-100 ring-1 ring-black/30">
        {/* Left: Recording Status Badge */}
        <div className="flex items-center gap-2 shrink-0">
          <div className="relative flex items-center justify-center">
            {!isPaused && (
              <span className={`w-2.5 h-2.5 rounded-full ${dotColor} animate-ping absolute`} />
            )}
            <span className={`w-2.5 h-2.5 rounded-full ${dotColor} relative`} />
          </div>
          <span className={`text-[11px] font-bold tracking-wider uppercase ${statusColor}`}>
            {statusLabel}
          </span>
        </div>

        {/* Center: Authoritative Timer (recorded time, excluding pauses) */}
        <div className="font-mono text-xs font-semibold tracking-wider text-slate-800 dark:text-neutral-200 tabular-nums shrink-0">
          {formatTimer(elapsedSec)}
        </div>

        {session?.capture_warning && (
          <div
            className="flex items-center text-amber-500 dark:text-amber-400 shrink-0"
            title={session.capture_warning}
          >
            <AlertTriangle className="w-3.5 h-3.5" />
          </div>
        )}

        {/* Dual Opposing Waveform Container (DictationPill styling) */}
        <div className="flex items-center gap-2.5 px-3 py-1 bg-slate-100 dark:bg-black/40 rounded-md border border-slate-200 dark:border-white/5 shrink-0">
          {/* Extreme Left: Mic Icon & Label */}
          <div
            className="flex items-center gap-1 text-[10px] font-bold uppercase tracking-wider text-emerald-600 dark:text-emerald-400"
            title="Your Microphone Input (Propagates Left to Right)"
          >
            <Mic className="w-3.5 h-3.5 stroke-[2.2]" />
            <span className="text-[9px] opacity-80">MIC</span>
          </div>

          {/* Left-to-Right Scrolling Wave (Mic) */}
          <div className="flex items-center gap-[2.5px] h-[22px] justify-start shrink-0">
            {waveMic.map((level, i) => {
              const heightPx = Math.max(3, Math.min(22, Math.round(level * 22)));
              const active = level > 0.01;
              return (
                <span
                  key={i}
                  className={`w-[2.5px] rounded-sm transition-all duration-75 origin-center ${
                    active
                      ? 'bg-emerald-500 dark:bg-emerald-400 shadow-xs shadow-emerald-400/40'
                      : 'bg-slate-300 dark:bg-neutral-700'
                  }`}
                  style={{ height: `${heightPx}px` }}
                />
              );
            })}
          </div>

          {/* Center Divider */}
          <div className="w-px h-3.5 bg-slate-300 dark:bg-white/10 mx-0.5" />

          {/* Right-to-Left Scrolling Wave (System Audio) */}
          <div className="flex items-center gap-[2.5px] h-[22px] justify-end shrink-0">
            {waveSys.map((level, i) => {
              const heightPx = Math.max(3, Math.min(22, Math.round(level * 22)));
              const active = level > 0.01;
              return (
                <span
                  key={i}
                  className={`w-[2.5px] rounded-sm transition-all duration-75 origin-center ${
                    active
                      ? 'bg-blue-600 dark:bg-blue-400 shadow-xs shadow-blue-400/40'
                      : 'bg-slate-300 dark:bg-neutral-700'
                  }`}
                  style={{ height: `${heightPx}px` }}
                />
              );
            })}
          </div>

          {/* Extreme Right: System Audio Label & Icon */}
          <div
            className="flex items-center gap-1 text-[10px] font-bold uppercase tracking-wider text-blue-600 dark:text-blue-400"
            title="Meeting Audio / System Participants (Propagates Right to Left)"
          >
            <span className="text-[9px] opacity-80">SYS</span>
            <Volume2 className="w-3.5 h-3.5 stroke-[2.2]" />
          </div>
        </div>

        {/* Pause / Resume */}
        <button
          onClick={handleTogglePause}
          disabled={isBusy || isFinalizing || !session}
          title={isPaused ? 'Resume recording' : 'Pause recording'}
          className={`flex items-center gap-1.5 px-3 py-1 rounded-md text-xs font-semibold tracking-wide transition-all shadow-sm active:scale-95 shrink-0 border ${
            isBusy || isFinalizing || !session
              ? 'bg-slate-200 dark:bg-neutral-800 text-slate-400 dark:text-neutral-500 cursor-not-allowed border-slate-300 dark:border-neutral-700'
              : isPaused
              ? 'bg-emerald-500/90 hover:bg-emerald-500 text-white border-emerald-400/40'
              : 'bg-slate-100 dark:bg-neutral-800 hover:bg-slate-200 dark:hover:bg-neutral-700 text-slate-700 dark:text-neutral-200 border-slate-300 dark:border-neutral-700'
          }`}
        >
          {isPaused ? (
            <>
              <Play className="w-2.5 h-2.5 fill-current" />
              <span>Resume</span>
            </>
          ) : (
            <>
              <Pause className="w-2.5 h-2.5 fill-current" />
              <span>Pause</span>
            </>
          )}
        </button>

        {/* Right: Stop / Finalize Button */}
        <button
          onClick={handleStop}
          disabled={isBusy || isFinalizing}
          className={`flex items-center gap-1.5 px-3 py-1 rounded-md text-xs font-semibold tracking-wide transition-all shadow-sm active:scale-95 shrink-0 ${
            isBusy || isFinalizing
              ? 'bg-slate-200 dark:bg-neutral-800 text-slate-400 dark:text-neutral-500 cursor-not-allowed border border-slate-300 dark:border-neutral-700'
              : 'bg-red-500/90 hover:bg-red-500 text-white border border-red-400/40 hover:shadow-red-500/20'
          }`}
        >
          {isFinalizing ? (
            <>
              <Loader2 className="w-3 h-3 animate-spin" />
              <span>Saving...</span>
            </>
          ) : (
            <>
              <Square className="w-2.5 h-2.5 fill-current" />
              <span>Stop</span>
            </>
          )}
        </button>
      </div>
    </div>
  );
};
