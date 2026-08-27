import React, { useCallback, useEffect, useRef, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { Square, Loader2, Pause, Play, AlertTriangle } from 'lucide-react';
import { AudioLevels, MeetingSession } from '../../types';
import { MeetingPillWaveform } from './MeetingPillMark';

/**
 * Samples held in the waveform, one per bar.
 *
 * The backend emits a level pair every 40 ms and each event shifts the history
 * by one bar, so this is a little under a second of audio on screen — long
 * enough to read as a wave, short enough that it tracks the voice rather than
 * lagging behind it.
 */
const WAVEFORM_BAR_COUNT = 20;
const SILENT_LEVELS: number[] = new Array(WAVEFORM_BAR_COUNT).fill(0);

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

/**
 * The floating meeting recording pill.
 *
 * One self-contained surface: the status dot, the elapsed time, and the live
 * waveform are inside it, and hovering opens pause and stop inside it too
 * rather than floating them alongside. Nothing is attached to the pill; the
 * pill is the whole object.
 *
 * A meeting runs for an hour, and for that hour this sits on top of whatever the
 * user is actually working in — so it stays narrow at rest and the window grows
 * only while the controls are open. The window is transparent but still takes
 * clicks, so unused width is an invisible dead zone, not free space.
 */
export const MeetingRecordingOverlay: React.FC = () => {
  const [session, setSession] = useState<MeetingSession | null>(null);
  const [elapsedSec, setElapsedSec] = useState<number>(0);
  const [micLevels, setMicLevels] = useState<number[]>(SILENT_LEVELS);
  const [sysLevels, setSysLevels] = useState<number[]>(SILENT_LEVELS);
  const [isBusy, setIsBusy] = useState<boolean>(false);
  const [isHovered, setIsHovered] = useState<boolean>(false);

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
      setMicLevels(SILENT_LEVELS);
      setSysLevels(SILENT_LEVELS);
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
      // Both channels shift the same way — newest on the right — so a bar's top
      // and bottom halves are the same instant. Opposing directions would put
      // the two meters on different timelines and the mirrored wave would be
      // showing something that never happened.
      setMicLevels((prev) => [...prev.slice(1), Math.min(1, mic_level || 0)]);
      setSysLevels((prev) => [...prev.slice(1), Math.min(1, sys_level || 0)]);
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

  // The window has to grow before the controls can be seen, so hover state is
  // pushed to the backend rather than handled in CSS alone.
  const setExpanded = useCallback((expanded: boolean) => {
    setIsHovered(expanded);
    invoke('set_meeting_overlay_expanded', { expanded }).catch((err) => {
      console.error('Failed to resize the meeting pill:', err);
    });
  }, []);

  // Never leave the window expanded behind a pill that has gone away.
  useEffect(() => {
    if (!session && isHovered) {
      setExpanded(false);
    }
  }, [session, isHovered, setExpanded]);

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

  if (!session) {
    return <div className="w-full h-full bg-transparent" />;
  }

  const showControls = isHovered && !isFinalizing;

  return (
    <div
      className="w-full h-full flex items-center justify-end pr-1 bg-transparent select-none"
      onMouseEnter={() => setExpanded(true)}
      onMouseLeave={() => setExpanded(false)}
    >
      <div
        className="inline-flex items-center gap-3 h-11 pl-3.5 pr-2 rounded-lg
                   bg-[#141414]/95 backdrop-blur-xl
                   border border-white/10 ring-1 ring-indigo-500/15
                   shadow-[0_10px_30px_rgba(0,0,0,0.55)]"
      >
        {/* Status dot and elapsed recorded time — the only text in the pill. */}
        <div className="flex items-center gap-2 shrink-0">
          <span className="relative flex w-2 h-2">
            {isRecording && (
              <span className="absolute inline-flex w-2 h-2 rounded-full bg-red-500 opacity-70 animate-ping" />
            )}
            <span
              className={`relative inline-flex w-2 h-2 rounded-full ${
                isPaused ? 'bg-amber-400' : isFinalizing ? 'bg-indigo-400' : 'bg-red-500'
              }`}
            />
          </span>
          <span className="font-mono text-[13px] leading-none font-medium tabular-nums text-neutral-100">
            {formatTimer(elapsedSec)}
          </span>
        </div>

        {session.capture_warning && (
          <span
            className="flex items-center shrink-0 text-amber-400"
            title={session.capture_warning}
          >
            <AlertTriangle className="w-3.5 h-3.5" />
          </span>
        )}

        {isFinalizing ? (
          <span className="flex items-center shrink-0 text-neutral-400">
            <Loader2 className="w-4 h-4 animate-spin" />
          </span>
        ) : (
          <div className="shrink-0" title="You above the line, the meeting below it">
            <MeetingPillWaveform mic={micLevels} sys={sysLevels} muted={isPaused} />
          </div>
        )}

        {/* Controls open inside the pill, not beside it. */}
        {showControls && (
          <div className="flex items-center gap-1 shrink-0">
            <button
              onClick={handleTogglePause}
              disabled={isBusy}
              title={isPaused ? 'Resume recording' : 'Pause recording'}
              aria-label={isPaused ? 'Resume recording' : 'Pause recording'}
              className="grid place-items-center w-7 h-7 rounded-md text-neutral-300
                         hover:bg-white/10 disabled:opacity-40 disabled:cursor-not-allowed
                         focus-visible:outline focus-visible:outline-2 focus-visible:outline-indigo-400
                         cursor-pointer"
            >
              {isPaused ? (
                <Play className="w-3.5 h-3.5 fill-current" />
              ) : (
                <Pause className="w-3.5 h-3.5 fill-current" />
              )}
            </button>
            <button
              onClick={handleStop}
              disabled={isBusy}
              title="Stop and save this meeting"
              aria-label="Stop and save this meeting"
              className="grid place-items-center w-7 h-7 rounded-md text-white bg-red-500
                         hover:bg-red-600 disabled:opacity-40 disabled:cursor-not-allowed
                         focus-visible:outline focus-visible:outline-2 focus-visible:outline-indigo-400
                         cursor-pointer"
            >
              <Square className="w-3 h-3 fill-current" />
            </button>
          </div>
        )}
      </div>
    </div>
  );
};
