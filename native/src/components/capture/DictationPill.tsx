import React, { useEffect, useRef, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { ProcessedPipelineResult } from '../../types';
import { RelayLogo } from '../common/RelayLogo';
import { Kanban, Sparkles, AlertCircle, CheckCircle2, Mic, Square, Loader2 } from 'lucide-react';
import { cn } from '@/lib/utils';

export type DictationUiState = 'idle' | 'listening' | 'processing' | 'success' | 'error';

interface CaptureStatePayload {
  active: boolean;
  mode: string | null;
  status?: string;
  message?: string;
}

interface DictationPillProps {
  onProcessComplete?: (result: ProcessedPipelineResult) => void;
}

const PROCESSING_CAPTIONS = [
  'transcribing speech...',
  'extracting kanban tasks...',
  'writing note to vault...',
];

const HOVER_EXPAND_DELAY_MS = 150;
const HOVER_COLLAPSE_DELAY_MS = 300;

/**
 * The ONLY push-to-talk visual surface (see docs/decisions.md PTT-013):
 * one native window, one state machine — RESTING (compact, unobtrusive)
 * or EXPANDED (listening/processing/success/error), never both a pill and
 * a separate "listening" indicator window. Ctrl+Space and hovering the
 * resting dot both expand it; releasing the hotkey or moving the mouse
 * away (while idle) collapses it back.
 *
 * Backend capture state remains authoritative for WHAT's happening
 * (listening/processing/success/error, via `capture-state-changed` and
 * `capture-level`); this component only owns whether that gets shown
 * RESTING or EXPANDED, and reports that via `set_pill_expanded` so the
 * native window can resize/reposition to tightly match — never a bigger
 * invisible hit-region than what's actually visible.
 */
export const DictationPill: React.FC<DictationPillProps> = ({ onProcessComplete }) => {
  const [phase, setPhase] = useState<DictationUiState>('idle');
  const [hovering, setHovering] = useState(false);
  const [mode, setMode] = useState<'meeting' | 'scribble'>('meeting');
  const [dictationShortcut, setDictationShortcut] = useState('Ctrl+Space');
  const [audioLevel, setAudioLevel] = useState(0);
  const [captionIndex, setCaptionIndex] = useState(0);
  const [errorMessage, setErrorMessage] = useState<string | null>(null);
  const [successMessage, setSuccessMessage] = useState<string>('Text inserted');

  const captionTimerRef = useRef<ReturnType<typeof setInterval> | null>(null);
  const successTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const hoverEnterTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const hoverLeaveTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  const isExpanded = phase !== 'idle' || hovering;

  // Read initial hotkey settings and capture status
  useEffect(() => {
    invoke<any>('get_settings')
      .then((settings) => {
        if (settings?.hotkeys?.dictation_hotkey) {
          setDictationShortcut(settings.hotkeys.dictation_hotkey);
        }
      })
      .catch(() => {});

    invoke<any>('get_capture_status')
      .then((status) => {
        if (status.active) {
          setPhase('listening');
        }
      })
      .catch(() => {});

    // Sync with backend capture-state-changed events — this is what makes
    // Ctrl+Space (a global OS hotkey with no direct handle to this
    // window) expand the pill: pressing it starts a capture session on
    // the backend, which broadcasts `active: true` here.
    const unlistenState = listen<CaptureStatePayload>('capture-state-changed', ({ payload }) => {
      if (payload.active) {
        setPhase('listening');
        setErrorMessage(null);
      } else if (payload.status === 'TRANSCRIBING' || payload.status === 'PROCESSING') {
        setPhase('processing');
      } else if (payload.status === 'SUCCESS') {
        setPhase('success');
        setSuccessMessage('Text inserted');
        if (successTimerRef.current) clearTimeout(successTimerRef.current);
        successTimerRef.current = setTimeout(() => setPhase('idle'), 2200);
      } else if (payload.status === 'ERROR' || payload.message) {
        setPhase('error');
        setErrorMessage(payload.message || 'Capture processing failed');
      } else {
        setPhase('idle');
      }
    });

    // Sync real RMS microphone audio level
    const unlistenLevel = listen<{ level: number }>('capture-level', ({ payload }) => {
      setAudioLevel(payload.level || 0);
    });

    return () => {
      unlistenState.then((fn) => fn());
      unlistenLevel.then((fn) => fn());
      if (successTimerRef.current) clearTimeout(successTimerRef.current);
      if (hoverEnterTimerRef.current) clearTimeout(hoverEnterTimerRef.current);
      if (hoverLeaveTimerRef.current) clearTimeout(hoverLeaveTimerRef.current);
    };
  }, []);

  // The native window can only be resized/repositioned from Rust — tell it
  // whenever our own RESTING/EXPANDED presentation state changes, so it
  // always tightly matches what's actually on screen.
  useEffect(() => {
    invoke('set_pill_expanded', { expanded: isExpanded }).catch((err) =>
      console.error('Failed to resize dictation pill window', err)
    );
  }, [isExpanded]);

  // Rotating processing caption effect
  useEffect(() => {
    if (phase === 'processing') {
      setCaptionIndex(0);
      captionTimerRef.current = setInterval(() => {
        setCaptionIndex((prev) => (prev + 1) % PROCESSING_CAPTIONS.length);
      }, 1200);
    } else {
      if (captionTimerRef.current) clearInterval(captionTimerRef.current);
    }
    return () => {
      if (captionTimerRef.current) clearInterval(captionTimerRef.current);
    };
  }, [phase]);

  const handleMouseEnter = () => {
    if (hoverLeaveTimerRef.current) clearTimeout(hoverLeaveTimerRef.current);
    if (!hovering) {
      hoverEnterTimerRef.current = setTimeout(() => setHovering(true), HOVER_EXPAND_DELAY_MS);
    }
  };

  const handleMouseLeave = () => {
    if (hoverEnterTimerRef.current) clearTimeout(hoverEnterTimerRef.current);
    hoverLeaveTimerRef.current = setTimeout(() => setHovering(false), HOVER_COLLAPSE_DELAY_MS);
  };

  // Click-to-talk toggle — same recorder/session state machine as the
  // global hotkey (start_capture/stop_capture), never a second one.
  const toggleClickToTalk = async () => {
    if (phase === 'listening') {
      try {
        setPhase('processing');
        const result = await invoke<ProcessedPipelineResult>('stop_capture');
        setPhase('success');
        setSuccessMessage(mode === 'meeting' ? 'Tasks Extracted to Kanban' : 'Saved to Voice Vault');
        if (onProcessComplete) onProcessComplete(result);
        if (successTimerRef.current) clearTimeout(successTimerRef.current);
        successTimerRef.current = setTimeout(() => setPhase('idle'), 2200);
      } catch (err: any) {
        setErrorMessage(err.message || 'Audio capture failed');
        setPhase('error');
      }
    } else if (phase === 'idle') {
      try {
        setErrorMessage(null);
        setPhase('listening');
        await invoke('start_capture', { mode });
      } catch (err: any) {
        setErrorMessage(err.message || 'Failed to start capture');
        setPhase('error');
      }
    }
  };

  return (
    <div
      className="relative inline-flex items-center justify-center select-none"
      onMouseEnter={handleMouseEnter}
      onMouseLeave={handleMouseLeave}
    >
      {!isExpanded ? (
        /* RESTING: compact, unobtrusive — click starts click-to-talk. */
        <button
          type="button"
          onClick={toggleClickToTalk}
          title={`Hold ${dictationShortcut}, or click, to dictate`}
          className="w-11 h-11 rounded-full bg-card/95 backdrop-blur-md border border-border shadow-2xl flex items-center justify-center text-foreground hover:text-primary transition-colors cursor-pointer animate-in fade-in zoom-in-95"
        >
          <RelayLogo className="w-5 h-5" />
        </button>
      ) : (
        /* EXPANDED: listening/processing/success/error UI. */
        <div className="flex items-center gap-3 px-4 py-2 rounded-full bg-card/95 backdrop-blur-md border border-border shadow-2xl transition-all duration-300 animate-in fade-in zoom-in-95">
          <RelayLogo className="w-5 h-5 shrink-0" />

          {/* Phase 1: IDLE (expanded via hover only) */}
          {phase === 'idle' && (
            <div className="flex items-center gap-3">
              <button
                type="button"
                onClick={toggleClickToTalk}
                className="flex items-center gap-2 group cursor-pointer text-xs font-semibold text-foreground hover:text-primary transition-colors"
                title="Click to dictate or hold configured hotkey"
              >
                <div className="w-6 h-6 rounded-full bg-primary/10 text-primary flex items-center justify-center group-hover:scale-110 transition-transform">
                  <Mic className="w-3.5 h-3.5" />
                </div>
                <span>Hold <kbd className="px-1.5 py-0.5 rounded bg-muted border border-border font-mono text-[10px] font-bold text-foreground">{dictationShortcut}</kbd></span>
              </button>

              {/* Mode Switcher */}
              <div className="flex items-center gap-1 bg-muted/60 p-0.5 rounded-full border border-border/50 text-[10px] font-mono">
                <button
                  type="button"
                  onClick={() => setMode('meeting')}
                  className={cn(
                    'px-2 py-0.5 rounded-full transition-colors flex items-center gap-1 cursor-pointer',
                    mode === 'meeting' ? 'bg-primary text-primary-foreground font-bold shadow-xs' : 'text-muted-foreground hover:text-foreground'
                  )}
                >
                  <Kanban className="w-3 h-3" />
                  <span>Meeting</span>
                </button>
                <button
                  type="button"
                  onClick={() => setMode('scribble')}
                  className={cn(
                    'px-2 py-0.5 rounded-full transition-colors flex items-center gap-1 cursor-pointer',
                    mode === 'scribble' ? 'bg-primary text-primary-foreground font-bold shadow-xs' : 'text-muted-foreground hover:text-foreground'
                  )}
                >
                  <Sparkles className="w-3 h-3" />
                  <span>Scribble</span>
                </button>
              </div>
            </div>
          )}

          {/* Phase 2: LISTENING */}
          {phase === 'listening' && (
            <div className="flex items-center gap-3">
              <div className="flex items-center gap-2 text-xs font-bold text-primary">
                <span className="w-2.5 h-2.5 rounded-full bg-primary animate-ping" />
                <span>Listening</span>
              </div>

              {/* Real RMS Audio Level Waveform */}
              <div className="flex items-center gap-1 h-5 px-2 bg-muted/30 rounded-full border border-border/40">
                {[0.5, 0.9, 0.7, 1.0, 0.6, 0.8].map((factor, i) => {
                  const heightPx = Math.max(3, Math.min(18, Math.round(audioLevel * 18 * factor)));
                  return (
                    <span
                      key={i}
                      className="w-1 rounded-full bg-primary transition-all duration-75"
                      style={{ height: `${heightPx}px` }}
                    />
                  );
                })}
              </div>

              <button
                type="button"
                onClick={toggleClickToTalk}
                className="p-1 rounded-full bg-primary/10 hover:bg-primary/20 text-primary transition-colors cursor-pointer"
                title="Stop recording"
              >
                <Square className="w-3.5 h-3.5 fill-current" />
              </button>
            </div>
          )}

          {/* Phase 3: PROCESSING / TRANSCRIBING */}
          {phase === 'processing' && (
            <div className="flex items-center gap-2.5 text-xs text-foreground font-medium">
              <Loader2 className="w-4 h-4 text-primary animate-spin" />
              <span className="font-mono text-muted-foreground text-[11px] animate-pulse">
                {PROCESSING_CAPTIONS[captionIndex]}
              </span>
            </div>
          )}

          {/* Phase 4: SUCCESS */}
          {phase === 'success' && (
            <div className="flex items-center gap-2 text-xs font-bold text-emerald-500 animate-in fade-in duration-200">
              <CheckCircle2 className="w-4 h-4" />
              <span>{successMessage}</span>
            </div>
          )}

          {/* Phase 5: ERROR — kept inline, inside the pill's own bounds
              rather than an overlaid banner: the native window is sized
              tightly to the visible pill, so anything rendered outside
              those bounds would just get clipped instead of shown. */}
          {phase === 'error' && (
            <div className="flex items-center gap-2.5 text-xs max-w-[280px]">
              <AlertCircle className="w-4 h-4 text-destructive shrink-0" />
              <span className="text-foreground/90 truncate flex-1 min-w-0">
                {errorMessage || 'Audio processing failed'}
              </span>
              <button
                type="button"
                onClick={() => setPhase('idle')}
                className="px-2 py-0.5 rounded-full bg-destructive/15 hover:bg-destructive/25 text-destructive text-[10px] font-mono font-bold uppercase tracking-wider shrink-0 transition-colors cursor-pointer"
              >
                Dismiss
              </button>
            </div>
          )}
        </div>
      )}
    </div>
  );
};
