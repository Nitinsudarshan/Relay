import React, { useCallback, useEffect, useRef, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { Square, Loader2, Pause, Play, AlertTriangle } from 'lucide-react';
import { AudioLevels, MeetingSession } from '../../types';
import { MeetingPillSpiral, MeetingPillWaveform } from './MeetingPillMark';

/** Bars in the resting waveform. Four reads as a meter; twelve read as noise. */
const WAVEFORM_BAR_COUNT = 4;
const SILENT_LEVELS = new Array(WAVEFORM_BAR_COUNT).fill(0);

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
 * A meeting runs for an hour, and for that hour this sits on top of whatever the
 * user is actually working in. So at rest it is a vertical capsule showing two
 * things — the mark, and a live level meter — and nothing else. That is enough
 * to answer the only question it needs to: is Relay still hearing this.
 *
 * Controls are not removed, they are deferred: hovering grows the window and
 * reveals the timer, pause and stop. A recording you cannot stop from the
 * indicator that represents it would be a worse pill, not a more minimal one.
 */
export const MeetingRecordingOverlay: React.FC = () => {
  const [session, setSession] = useState<MeetingSession | null>(null);
  const [elapsedSec, setElapsedSec] = useState<number>(0);
  const [levels, setLevels] = useState<number[]>(SILENT_LEVELS);
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
      setLevels(SILENT_LEVELS);
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
      // One meter for both sources: at this size the pill answers "is Relay
      // hearing anything", and whichever side is louder answers it.
      const level = Math.min(1, Math.max(mic_level || 0, sys_level || 0));
      setLevels((prev) => [...prev.slice(1), level]);
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
      <div className="flex items-center gap-2">
        {showControls && (
          <div className="flex items-center gap-1.5 pl-1">
            <span className="font-mono text-[11px] font-semibold tabular-nums text-neutral-300">
              {formatTimer(elapsedSec)}
            </span>
            <button
              onClick={handleTogglePause}
              disabled={isBusy}
              title={isPaused ? 'Resume recording' : 'Pause recording'}
              className="grid place-items-center w-7 h-7 rounded-full bg-neutral-800/90 text-neutral-200 border border-white/10 hover:bg-neutral-700 disabled:opacity-40 focus-visible:outline focus-visible:outline-2 focus-visible:outline-lime-400"
            >
              {isPaused ? (
                <Play className="w-3 h-3 fill-current" />
              ) : (
                <Pause className="w-3 h-3 fill-current" />
              )}
            </button>
            <button
              onClick={handleStop}
              disabled={isBusy}
              title="Stop and save this meeting"
              className="grid place-items-center w-7 h-7 rounded-full bg-red-500/90 text-white border border-red-400/40 hover:bg-red-500 disabled:opacity-40 focus-visible:outline focus-visible:outline-2 focus-visible:outline-lime-400"
            >
              <Square className="w-2.5 h-2.5 fill-current" />
            </button>
          </div>
        )}

        {/* The pill itself. Fixed size, so hovering reveals controls beside it
            rather than moving the mark the user is tracking. */}
        <div
          className="flex flex-col items-center justify-center gap-2 w-[44px] h-[72px] rounded-[18px] bg-[#141414] border border-white/10 ring-1 ring-black/40 shadow-[0_10px_30px_rgba(0,0,0,0.55)]"
          title={session.capture_warning ?? undefined}
        >
          {isFinalizing ? (
            <Loader2 className="w-[22px] h-[22px] animate-spin text-neutral-400" />
          ) : session.capture_warning ? (
            <AlertTriangle className="w-[20px] h-[20px] text-amber-400" />
          ) : (
            <MeetingPillSpiral className={`w-[22px] h-[22px] ${isPaused ? 'text-amber-400' : 'text-neutral-400'}`} />
          )}
          <MeetingPillWaveform levels={levels} muted={isPaused || isFinalizing} />
        </div>
      </div>
    </div>
  );
};
