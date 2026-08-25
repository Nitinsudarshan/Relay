import React, { useEffect, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { Square, Loader2, Mic, Volume2 } from 'lucide-react';
import { AudioLevels, MeetingSession } from '../../types';

// Matching DictationPill waveform bar parameters
const WAVEFORM_BAR_COUNT = 12;
const SILENT_LEVEL_HISTORY = new Array(WAVEFORM_BAR_COUNT).fill(0);

export const MeetingRecordingOverlay: React.FC = () => {
  const [session, setSession] = useState<MeetingSession | null>(null);
  const [elapsedSec, setElapsedSec] = useState<number>(0);
  const [micHistory, setMicHistory] = useState<number[]>(SILENT_LEVEL_HISTORY);
  const [sysHistory, setSysHistory] = useState<number[]>(SILENT_LEVEL_HISTORY);
  const [isStopping, setIsStopping] = useState<boolean>(false);

  // 1. Fetch active session on mount
  useEffect(() => {
    invoke<MeetingSession | null>('get_active_meeting_v2')
      .then((res) => {
        if (res) {
          setSession(res);
        }
      })
      .catch((err) => console.error('Failed to get active meeting:', err));

    // Listen for session state changes
    const unlistenState = listen<MeetingSession>('meeting-session-state-changed', (event) => {
      setSession(event.payload);
      if (event.payload.state === 'STOPPING' || event.payload.state === 'FINALIZING') {
        setIsStopping(true);
      }
      if (event.payload.state === 'COMPLETED' || event.payload.state === 'IDLE') {
        setIsStopping(false);
        setSession(null);
        setElapsedSec(0);
        setMicHistory(SILENT_LEVEL_HISTORY);
        setSysHistory(SILENT_LEVEL_HISTORY);
      }
    });

    // Listen for real-time live audio energy levels
    // Mic wave: newest sample enters at left [0] and scrolls left-to-right
    // Sys wave: newest sample enters at right [last] and scrolls right-to-left
    const unlistenLevels = listen<AudioLevels>('meeting-audio-levels', (event) => {
      const { mic_level, sys_level } = event.payload;
      const mic = Math.min(1.0, mic_level || 0);
      const sys = Math.min(1.0, sys_level || 0);

      setMicHistory((prev) => [mic, ...prev.slice(0, prev.length - 1)]);
      setSysHistory((prev) => [...prev.slice(1), sys]);
    });

    return () => {
      unlistenState.then((f) => f());
      unlistenLevels.then((f) => f());
    };
  }, []);

  // 2. Authoritative Wall-Clock Timer (derived from session start timestamp)
  useEffect(() => {
    if (!session || session.state === 'COMPLETED' || session.state === 'IDLE') {
      return;
    }

    const startMs = session.started_at
      ? new Date(session.started_at).getTime()
      : Date.now() - (session.duration_seconds || 0) * 1000;

    const updateElapsed = () => {
      const diffSec = Math.max(0, Math.floor((Date.now() - startMs) / 1000));
      setElapsedSec(diffSec);
    };

    updateElapsed();
    const timer = setInterval(updateElapsed, 500);
    return () => clearInterval(timer);
  }, [session?.id, session?.started_at, session?.state]);

  const formatTimer = (totalSeconds: number) => {
    const hrs = Math.floor(totalSeconds / 3600);
    const mins = Math.floor((totalSeconds % 3600) / 60);
    const secs = totalSeconds % 60;
    if (hrs > 0) {
      return `${String(hrs).padStart(2, '0')}:${String(mins).padStart(2, '0')}:${String(secs).padStart(2, '0')}`;
    }
    return `${String(mins).padStart(2, '0')}:${String(secs).padStart(2, '0')}`;
  };

  const handleStop = async () => {
    if (isStopping) return;
    setIsStopping(true);
    try {
      await invoke('stop_meeting_v2');
      setSession(null);
      setElapsedSec(0);
      setIsStopping(false);
    } catch (err) {
      console.error('Failed to stop meeting recording:', err);
      setIsStopping(false);
    }
  };

  return (
    <div className="w-full h-full flex items-center justify-center p-1 bg-transparent select-none">
      <div className="inline-flex items-center gap-3 px-4 h-[44px] rounded-lg bg-white dark:bg-[#171717] border border-slate-200 dark:border-[#262626] shadow-[0_16px_40px_rgba(0,0,0,0.5)] text-slate-900 dark:text-neutral-100 ring-1 ring-black/30">
        {/* Left: Recording Status Badge */}
        <div className="flex items-center gap-2 shrink-0">
          <div className="relative flex items-center justify-center">
            <span className="w-2.5 h-2.5 rounded-full bg-red-500 animate-ping absolute" />
            <span className="w-2.5 h-2.5 rounded-full bg-red-500 relative" />
          </div>
          <span className="text-[11px] font-bold tracking-wider uppercase text-red-500 dark:text-red-400">
            {isStopping ? 'Finalizing' : 'Recording'}
          </span>
        </div>

        {/* Center: Authoritative Timer */}
        <div className="font-mono text-xs font-semibold tracking-wider text-slate-800 dark:text-neutral-200 tabular-nums shrink-0">
          {formatTimer(elapsedSec)}
        </div>

        {/* Dual Opposing Waveform Container (DictationPill styling) */}
        <div className="flex items-center gap-2.5 px-3 py-1 bg-slate-100 dark:bg-black/40 rounded-md border border-slate-200 dark:border-white/5 shrink-0">
          {/* Extreme Left: Mic Icon & Label */}
          <div className="flex items-center gap-1 text-[10px] font-bold uppercase tracking-wider text-emerald-600 dark:text-emerald-400" title="Your Microphone Input (Propagates Left to Right)">
            <Mic className="w-3.5 h-3.5 stroke-[2.2]" />
            <span className="text-[9px] opacity-80">MIC</span>
          </div>

          {/* Left-to-Right Scrolling Wave (Mic) */}
          <div className="flex items-center gap-[2.5px] h-[22px] justify-start shrink-0">
            {micHistory.map((level, i) => {
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
            {sysHistory.map((level, i) => {
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
          <div className="flex items-center gap-1 text-[10px] font-bold uppercase tracking-wider text-blue-600 dark:text-blue-400" title="Meeting Audio / System Participants (Propagates Right to Left)">
            <span className="text-[9px] opacity-80">SYS</span>
            <Volume2 className="w-3.5 h-3.5 stroke-[2.2]" />
          </div>
        </div>

        {/* Right: Stop / Finalize Button */}
        <button
          onClick={handleStop}
          disabled={isStopping}
          className={`flex items-center gap-1.5 px-3 py-1 rounded-md text-xs font-semibold tracking-wide transition-all shadow-sm active:scale-95 shrink-0 ${
            isStopping
              ? 'bg-slate-200 dark:bg-neutral-800 text-slate-400 dark:text-neutral-500 cursor-not-allowed border border-slate-300 dark:border-neutral-700'
              : 'bg-red-500/90 hover:bg-red-500 text-white border border-red-400/40 hover:shadow-red-500/20'
          }`}
        >
          {isStopping ? (
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
