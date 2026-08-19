import React, { useState, useEffect, useRef } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { ProcessedPipelineResult } from '../../types';
import { RelayLogo } from '../common/RelayLogo';
import {
  Kanban,
  Sparkles,
  AlertCircle,
  CheckCircle2,
  Mic,
  Square,
  Settings,
  Loader2,
  X,
  Volume2,
} from 'lucide-react';
import { Button } from '@/components/ui/button';
import { Badge } from '@/components/ui/badge';
import { Switch } from '@/components/ui/switch';
import { Popover, PopoverContent, PopoverTrigger } from '@/components/ui/popover';
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

export const DictationPill: React.FC<DictationPillProps> = ({ onProcessComplete }) => {
  const [phase, setPhase] = useState<DictationUiState>('idle');
  const [mode, setMode] = useState<'meeting' | 'scribble'>('meeting');
  const [dictationShortcut, setDictationShortcut] = useState('Ctrl+Space');
  const [audioLevel, setAudioLevel] = useState(0);
  const [captionIndex, setCaptionIndex] = useState(0);
  const [errorMessage, setErrorMessage] = useState<string | null>(null);
  const [successMessage, setSuccessMessage] = useState<string>('Text inserted');

  const captionTimerRef = useRef<ReturnType<typeof setInterval> | null>(null);
  const successTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);

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

    // Sync with backend capture-state-changed events
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
    };
  }, []);

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

  // Click-to-talk toggle
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
    <div className="relative flex flex-col items-center select-none z-50 my-2">
      {/* Error Toast Banner */}
      {phase === 'error' && (
        <div className="absolute bottom-full mb-3 z-50 animate-in fade-in slide-in-from-bottom-2 duration-200">
          <div className="flex items-start gap-2.5 px-4 py-2.5 rounded-2xl bg-destructive text-destructive-foreground border border-destructive-foreground/20 shadow-2xl max-w-xs md:max-w-sm text-xs leading-relaxed font-medium">
            <AlertCircle className="w-4 h-4 shrink-0 mt-0.5" />
            <div className="flex-1 min-w-0">
              <p className="font-semibold text-xs">Capture Error</p>
              <p className="text-[11px] text-destructive-foreground/90 mt-0.5 break-words">
                {errorMessage || 'Audio processing failed'}
              </p>
            </div>
            <button
              type="button"
              onClick={() => setPhase('idle')}
              className="px-2 py-0.5 rounded-full bg-destructive-foreground/15 hover:bg-destructive-foreground/25 text-[10px] font-mono font-bold uppercase tracking-wider shrink-0 transition-colors cursor-pointer"
            >
              Dismiss
            </button>
          </div>
        </div>
      )}

      {/* Main Dictation Pill Capsule Container */}
      <div className="flex items-center gap-3 px-4 py-2 rounded-full bg-card/95 backdrop-blur-md border border-border shadow-2xl transition-all duration-300">
        <RelayLogo className="w-5 h-5 shrink-0" />

        {/* Phase 1: IDLE */}
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
      </div>
    </div>
  );
};
