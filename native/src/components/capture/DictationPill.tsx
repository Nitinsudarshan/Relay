import React, { useState, useEffect, useRef } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { ProcessedPipelineResult } from '../../types';
import { RelayLogo } from '../common/RelayLogo';
import {
  Kanban,
  Sparkles,
  ChevronDown,
  ChevronUp,
  AlertCircle,
  CheckCircle2,
  Cpu,
  Mic,
  Zap,
  SlidersHorizontal,
  RotateCcw,
} from 'lucide-react';
import { Button } from '@/components/ui/button';
import { Badge } from '@/components/ui/badge';
import { Switch } from '@/components/ui/switch';
import { Popover, PopoverContent, PopoverTrigger } from '@/components/ui/popover';

type DictationPhase =
  | 'rest'
  | 'ready'
  | 'expanded'
  | 'recording'
  | 'processing'
  | 'inserted'
  | 'error';

interface DictationPillProps {
  onProcessComplete?: (result: ProcessedPipelineResult) => void;
}

const PROCESSING_CAPTIONS = [
  'transcribing speech...',
  'removing filler words...',
  'extracting kanban tasks...',
  'writing note to vault...',
];

export const DictationPill: React.FC<DictationPillProps> = ({ onProcessComplete }) => {
  const [phase, setPhase] = useState<DictationPhase>('expanded');
  const [mode, setMode] = useState<'meeting' | 'scribble'>('meeting');
  const [captionIndex, setCaptionIndex] = useState(0);
  const [errorMessage, setErrorMessage] = useState<string | null>(null);
  const [settingsOpen, setSettingsOpen] = useState(false);

  // Settings State
  const [useCloudLLM, setUseCloudLLM] = useState(false);
  const [sttEngine, setSttEngine] = useState<'whisper' | 'parakeet'>('whisper');
  const [autoOpenDashboard, setAutoOpenDashboard] = useState(true);

  const hoverTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const leaveTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const captionTimerRef = useRef<ReturnType<typeof setInterval> | null>(null);

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

  // Handle hover hold (~180ms)
  const handleMouseEnter = () => {
    if (leaveTimerRef.current) clearTimeout(leaveTimerRef.current);
    if (phase === 'rest') {
      setPhase('ready');
      hoverTimerRef.current = setTimeout(() => {
        setPhase('expanded');
      }, 180);
    }
  };

  const handleMouseLeave = () => {
    if (hoverTimerRef.current) clearTimeout(hoverTimerRef.current);
    if (phase === 'expanded' || phase === 'ready') {
      if (!settingsOpen) {
        leaveTimerRef.current = setTimeout(() => {
          setPhase('rest');
        }, 300);
      }
    }
  };

  const startRecording = async () => {
    try {
      setErrorMessage(null);
      setPhase('recording');
      await invoke('start_capture', { mode });
    } catch (err: any) {
      console.error('Failed to start recording', err);
      setErrorMessage(err.message || 'Microphone capture error');
      setPhase('error');
    }
  };

  const stopRecording = async () => {
    try {
      setPhase('processing');
      const result = await invoke<ProcessedPipelineResult>('stop_capture');
      setPhase('inserted');
      if (onProcessComplete) onProcessComplete(result);
      setTimeout(() => {
        setPhase('expanded');
      }, 2500);
    } catch (err: any) {
      console.error('Failed to stop recording', err);
      setErrorMessage(err.message || 'LLM pipeline processing error');
      setPhase('error');
    }
  };

  const toggleRecording = () => {
    if (phase === 'recording') {
      stopRecording();
    } else if (phase === 'expanded' || phase === 'ready' || phase === 'rest') {
      startRecording();
    }
  };

  return (
    <div
      className="relative flex flex-col items-center select-none z-50 my-4"
      onMouseEnter={handleMouseEnter}
      onMouseLeave={handleMouseLeave}
    >
      {/* Inserted Success Toast */}
      {phase === 'inserted' && (
        <div className="absolute -top-12 animate-in fade-in slide-in-from-bottom-2 duration-200">
          <Badge
            variant="outline"
            className="bg-primary text-primary-foreground border-primary/20 gap-2 px-3 py-1.5 shadow-lg text-xs font-semibold rounded-full"
          >
            <CheckCircle2 className="w-3.5 h-3.5" />
            <span>Saved to Vault & {mode === 'meeting' ? 'Kanban' : 'Scribbles'}</span>
          </Badge>
        </div>
      )}

      {/* Error Toast */}
      {phase === 'error' && (
        <div className="absolute bottom-full mb-3 z-50 animate-in fade-in slide-in-from-bottom-2 duration-200">
          <div className="flex items-start gap-2.5 px-4 py-2.5 rounded-2xl bg-destructive text-destructive-foreground border border-destructive-foreground/20 shadow-2xl max-w-xs md:max-w-sm text-xs leading-relaxed font-medium">
            <AlertCircle className="w-4 h-4 shrink-0 mt-0.5 text-destructive-foreground" />
            <div className="flex-1 min-w-0">
              <p className="font-semibold text-xs text-destructive-foreground">Capture Error</p>
              <p className="text-[11px] text-destructive-foreground/90 mt-0.5 break-words">
                {errorMessage || 'Audio processing error'}
              </p>
            </div>
            <button
              type="button"
              onClick={() => setPhase('expanded')}
              className="px-2 py-0.5 rounded-full bg-destructive-foreground/15 hover:bg-destructive-foreground/25 text-destructive-foreground text-[10px] font-mono font-bold uppercase tracking-wider shrink-0 transition-colors cursor-pointer"
            >
              Dismiss
            </button>
          </div>
        </div>
      )}

      {/* Handle (rest / ready state) */}
      {(phase === 'rest' || phase === 'ready') && (
        <div
          onClick={() => setPhase('expanded')}
          className={`h-2.5 rounded-full cursor-pointer transition-all duration-300 shadow-md ${
            phase === 'ready'
              ? 'w-28 bg-primary ring-4 ring-primary/20 scale-105'
              : 'w-16 bg-border-strong hover:bg-primary'
          }`}
          title="Hold or click to expand Relay Dictation Pill"
        />
      )}

      {/* Expanded Floating Capsule Body */}
      {phase !== 'rest' && phase !== 'ready' && (
        <div className="inline-flex items-center gap-3 h-12 px-4 rounded-full bg-card border border-border shadow-xl text-card-foreground transition-all duration-300 animate-in fade-in zoom-in-95">
          {/* Logo Mark */}
          <div className="flex items-center gap-1.5 cursor-pointer" onClick={() => setPhase('rest')}>
            <RelayLogo className="w-5 h-5" />
          </div>

          <div className="h-4 w-px bg-border shrink-0" />

          {/* Pill Center Body Content per Phase */}
          <div className="flex items-center gap-2 min-w-[140px] justify-center">
            {/* Expanded / Idle State */}
            {phase === 'expanded' && (
              <button
                type="button"
                onClick={toggleRecording}
                className="flex items-center gap-2 group text-xs font-medium text-foreground hover:text-primary transition-colors cursor-pointer"
              >
                <div className="flex items-center gap-1">
                  {[...Array(5)].map((_, i) => (
                    <span key={i} className="w-1 h-1 rounded-full bg-muted-foreground/50 group-hover:bg-primary" />
                  ))}
                </div>
                <span className="font-semibold tracking-tight">Click to dictate</span>
              </button>
            )}

            {/* Recording State: Live Audio Waveform */}
            {phase === 'recording' && (
              <button
                type="button"
                onClick={toggleRecording}
                className="flex items-center gap-2 group cursor-pointer"
              >
                <div className="flex items-end gap-1 h-5 px-1">
                  <span className="w-1 h-3 bg-primary rounded-full animate-audio-bar-1" />
                  <span className="w-1 h-5 bg-primary rounded-full animate-audio-bar-2" />
                  <span className="w-1 h-2 bg-primary rounded-full animate-audio-bar-3" />
                  <span className="w-1 h-4 bg-primary rounded-full animate-audio-bar-4" />
                  <span className="w-1 h-5 bg-primary rounded-full animate-audio-bar-5" />
                  <span className="w-1 h-3 bg-primary rounded-full animate-audio-bar-2" />
                  <span className="w-1 h-4 bg-primary rounded-full animate-audio-bar-1" />
                </div>
                <span className="text-xs font-mono font-bold text-primary uppercase tracking-wider animate-pulse">
                  REC
                </span>
              </button>
            )}

            {/* Processing State: Rotating Caption */}
            {phase === 'processing' && (
              <div className="flex items-center gap-2 text-xs font-mono text-muted-foreground">
                <RotateCcw className="w-3.5 h-3.5 animate-spin text-primary shrink-0" />
                <span className="truncate max-w-[160px] animate-in fade-in duration-200">
                  {PROCESSING_CAPTIONS[captionIndex]}
                </span>
              </div>
            )}
          </div>

          <div className="h-4 w-px bg-border shrink-0" />

          {/* Controls Right Section */}
          <div className="flex items-center gap-1.5 shrink-0">
            {/* Mode Switch Toggle: Meeting vs Voice Scribble */}
            <Button
              type="button"
              variant="ghost"
              size="icon"
              onClick={() => setMode(mode === 'meeting' ? 'scribble' : 'meeting')}
              title={mode === 'meeting' ? 'Mode: Meeting → Kanban' : 'Mode: Voice Scribble'}
              className={`h-7 w-7 rounded-full transition-colors ${
                mode === 'scribble'
                  ? 'bg-primary text-primary-foreground hover:bg-primary/90'
                  : 'text-muted-foreground hover:text-foreground hover:bg-muted'
              }`}
            >
              {mode === 'meeting' ? <Kanban className="w-3.5 h-3.5" /> : <Sparkles className="w-3.5 h-3.5" />}
            </Button>

            {/* Settings Popover */}
            <Popover open={settingsOpen} onOpenChange={setSettingsOpen}>
              <PopoverTrigger asChild>
                <Button
                  type="button"
                  variant="ghost"
                  size="icon"
                  className="h-7 w-7 rounded-full text-muted-foreground hover:text-foreground hover:bg-muted"
                  title="Pill Settings"
                >
                  <SlidersHorizontal className="w-3.5 h-3.5" />
                </Button>
              </PopoverTrigger>
              <PopoverContent className="w-64 p-3 bg-popover border-border shadow-2xl rounded-2xl text-xs space-y-3">
                <div className="flex items-center justify-between font-bold text-foreground pb-2 border-b border-border">
                  <span className="font-mono text-[10px] uppercase tracking-wider text-muted-foreground">
                    Dictation Engine Settings
                  </span>
                  <Badge variant="outline" className="text-[10px] font-mono">
                    Local Vault
                  </Badge>
                </div>

                {/* Local Ollama vs Cloud Provider */}
                <div className="flex items-center justify-between py-1">
                  <div className="space-y-0.5">
                    <p className="font-medium text-foreground">Cloud LLM Provider</p>
                    <p className="text-[10px] text-muted-foreground">Toggle OpenAI/Gemini vs Ollama</p>
                  </div>
                  <Switch checked={useCloudLLM} onCheckedChange={setUseCloudLLM} />
                </div>

                {/* STT Engine choice */}
                <div className="space-y-1 py-1">
                  <p className="font-medium text-foreground">Speech-to-Text Engine</p>
                  <div className="flex bg-muted p-0.5 rounded-lg border border-border">
                    <button
                      type="button"
                      onClick={() => setSttEngine('whisper')}
                      className={`flex-1 py-1 text-[10px] font-medium rounded-md transition-all ${
                        sttEngine === 'whisper' ? 'bg-card text-foreground font-semibold shadow-xs' : 'text-muted-foreground'
                      }`}
                    >
                      Whisper Local
                    </button>
                    <button
                      type="button"
                      onClick={() => setSttEngine('parakeet')}
                      className={`flex-1 py-1 text-[10px] font-medium rounded-md transition-all ${
                        sttEngine === 'parakeet' ? 'bg-card text-foreground font-semibold shadow-xs' : 'text-muted-foreground'
                      }`}
                    >
                      Parakeet Engine
                    </button>
                  </div>
                </div>

                {/* Hotkey Indicator */}
                <div className="flex items-center justify-between pt-2 border-t border-border text-muted-foreground text-[10px]">
                  <span className="flex items-center gap-1">
                    <Zap className="w-3 h-3 text-primary" /> Global Hotkey
                  </span>
                  <kbd className="px-1.5 py-0.5 bg-muted rounded border border-border font-mono text-[10px]">
                    Ctrl+Space
                  </kbd>
                </div>
              </PopoverContent>
            </Popover>
          </div>
        </div>
      )}
    </div>
  );
};
