import React, { useEffect, useRef, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { AppSettings, ProcessedPipelineResult } from '../../types';
import { PillSettingsPopover } from './PillSettingsPopover';
import { 
  PillState, 
  WhisperStatusInfo, 
  OllamaStatusInfo, 
  HotkeyStatusInfo, 
  DiagnosticInfo, 
  CleanupStyle, 
  SpeechLanguage 
} from './PillTypes';
import { 
  Sparkles, 
  ChevronDown, 
  ChevronUp, 
  AlertCircle, 
  CheckCircle2, 
  Square, 
  Loader2, 
  Bug 
} from 'lucide-react';
import { cn } from '@/lib/utils';

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

const HOVER_EXPAND_DELAY_MS = 120;
const HOVER_COLLAPSE_DELAY_MS = 1000;

export const DictationPill: React.FC<DictationPillProps> = ({ onProcessComplete }) => {
  const [phase, setPhase] = useState<PillState>('collapsed');
  const [hovering, setHovering] = useState(false);
  const [popoverOpen, setPopoverOpen] = useState(false);
  const [activeApp, setActiveApp] = useState<string>('SNIPPING TOOL');
  
  // Settings & Toggles
  const [autoPaste, setAutoPaste] = useState(true);
  const [textTransform, setTextTransform] = useState(true);
  const [cleanupStyle, setCleanupStyle] = useState<CleanupStyle>('faithful');
  const [promptMode, setPromptMode] = useState(false);
  const [language, setLanguage] = useState<SpeechLanguage>('english');
  const [dictationShortcut, setDictationShortcut] = useState('Ctrl+Space');

  // Recording & Status State
  const [audioLevel, setAudioLevel] = useState(0);
  const [captionIndex, setCaptionIndex] = useState(0);
  const [errorMessage, setErrorMessage] = useState<string | null>(null);
  const [warningMessage, setWarningMessage] = useState<string | null>(null);
  const [successMessage, setSuccessMessage] = useState<string>('Text inserted');
  const [showDiagnostics, setShowDiagnostics] = useState(false);

  // Dependency Verification Statuses
  const [settings, setSettings] = useState<AppSettings | null>(null);
  const [whisperStatus, setWhisperStatus] = useState<WhisperStatusInfo>({ status: 'checking' });
  const [ollamaStatus, setOllamaStatus] = useState<OllamaStatusInfo>({
    status: 'checking',
    host: 'http://localhost:11434',
    model: 'llama3.2:latest',
  });
  const [hotkeyStatus, setHotkeyStatus] = useState<HotkeyStatusInfo>({
    status: 'registered',
    hotkey: 'Ctrl+Space',
  });

  const captionTimerRef = useRef<ReturnType<typeof setInterval> | null>(null);
  const successTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const hoverEnterTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const hoverLeaveTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  const isExpanded = (phase !== 'collapsed' && phase !== 'hidden_notch' && phase !== 'error' && phase !== 'warning') || hovering || popoverOpen;

  // Refresh dependency statuses
  const refreshDependencies = async () => {
    try {
      const appSettings = await invoke<AppSettings>('get_settings');
      setSettings(appSettings);
      if (appSettings?.hotkeys?.dictation_hotkey) {
        setDictationShortcut(appSettings.hotkeys.dictation_hotkey);
        setHotkeyStatus({ status: 'registered', hotkey: appSettings.hotkeys.dictation_hotkey });
      }

      // Check STT
      try {
        const sttResult = await invoke<any>('ensure_stt_model_ready');
        if (sttResult.state === 'ready' || sttResult.Ready) {
          setWhisperStatus({ status: 'ready', modelPath: sttResult.Ready?.path || sttResult.path });
        } else {
          setWhisperStatus({ status: 'download_required', message: 'Whisper model required' });
        }
      } catch {
        setWhisperStatus({ status: 'download_required', message: 'Whisper check failed' });
      }

      // Check Ollama / LLM
      try {
        const llmResult = await invoke<any>('ensure_local_llm_ready');
        if (appSettings.provider.active_provider !== 'ollama') {
          setOllamaStatus({
            status: 'cloud_active',
            host: 'cloud',
            model: appSettings.provider.cloud_model || 'cloud',
          });
        } else if (llmResult === 'Running' || llmResult.state === 'running') {
          setOllamaStatus({
            status: 'ready',
            host: appSettings.provider.ollama_host,
            model: appSettings.provider.ollama_model,
          });
        } else {
          setOllamaStatus({
            status: 'unreachable',
            host: appSettings.provider.ollama_host,
            model: appSettings.provider.ollama_model,
          });
        }
      } catch {
        setOllamaStatus({
          status: 'unreachable',
          host: appSettings?.provider?.ollama_host || 'http://localhost:11434',
          model: appSettings?.provider?.ollama_model || 'llama3.2',
        });
      }
    } catch (err) {
      console.error('Failed to load settings', err);
    }
  };

  const handleDownloadWhisper = async () => {
    setWhisperStatus({ status: 'downloading' });
    try {
      const res = await invoke<any>('ensure_stt_model_ready');
      if (res.state === 'ready' || res.Ready) {
        setWhisperStatus({ status: 'ready', modelPath: res.Ready?.path || res.path });
      } else {
        setWhisperStatus({ status: 'failed', message: 'Download failed' });
      }
    } catch {
      setWhisperStatus({ status: 'failed', message: 'Download failed' });
    }
  };

  useEffect(() => {
    refreshDependencies();

    invoke<any>('get_capture_status')
      .then((status) => {
        if (status.active) {
          setPhase('listening');
        }
      })
      .catch(() => {});

    // Listen for backend capture events
    const unlistenState = listen<CaptureStatePayload>('capture-state-changed', ({ payload }) => {
      if (payload.active) {
        setPhase('listening');
        setErrorMessage(null);
        setWarningMessage(null);
      } else if (payload.status === 'TRANSCRIBING' || payload.status === 'PROCESSING') {
        setPhase('processing');
      } else if (payload.status === 'SUCCESS') {
        setPhase('success');
        setSuccessMessage(promptMode ? 'Prompt transformed & inserted' : 'Text inserted');
        if (successTimerRef.current) clearTimeout(successTimerRef.current);
        successTimerRef.current = setTimeout(() => setPhase('collapsed'), 2200);
      } else if (payload.status === 'ERROR' || payload.message) {
        setPhase('error');
        setErrorMessage(payload.message || 'Capture processing failed');
      } else {
        setPhase('collapsed');
      }
    });

    const unlistenLevel = listen<{ level: number }>('capture-level', ({ payload }) => {
      setAudioLevel(payload.level || 0);
    });

    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.ctrlKey && e.shiftKey && e.key === 'D') {
        setShowDiagnostics((prev) => !prev);
      }
    };
    window.addEventListener('keydown', handleKeyDown);

    return () => {
      unlistenState.then((fn) => fn());
      unlistenLevel.then((fn) => fn());
      window.removeEventListener('keydown', handleKeyDown);
      if (successTimerRef.current) clearTimeout(successTimerRef.current);
      if (hoverEnterTimerRef.current) clearTimeout(hoverEnterTimerRef.current);
      if (hoverLeaveTimerRef.current) clearTimeout(hoverLeaveTimerRef.current);
    };
  }, []);

  // Update Rust overlay window geometry
  useEffect(() => {
    let modeName: 'resting' | 'expanded' | 'popover' = 'resting';
    if (popoverOpen) {
      modeName = 'popover';
    } else if (isExpanded) {
      modeName = 'expanded';
    }

    invoke('set_pill_window_mode', { mode: modeName }).catch(() => {
      invoke('set_pill_expanded', { expanded: isExpanded }).catch(() => {});
    });
  }, [isExpanded, popoverOpen]);

  // Caption timer effect
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

  // Hover Handlers (120ms enter intent, 1000ms leave collapse delay)
  const handleMouseEnter = () => {
    if (hoverLeaveTimerRef.current) clearTimeout(hoverLeaveTimerRef.current);
    if (!hovering) {
      hoverEnterTimerRef.current = setTimeout(() => {
        setHovering(true);
        if (phase === 'collapsed' || phase === 'hidden_notch') setPhase('expanded');
      }, HOVER_EXPAND_DELAY_MS);
    }
  };

  const handleMouseLeave = () => {
    if (hoverEnterTimerRef.current) clearTimeout(hoverEnterTimerRef.current);
    if (popoverOpen || phase === 'listening' || phase === 'processing') return;

    hoverLeaveTimerRef.current = setTimeout(() => {
      setHovering(false);
      if (phase === 'expanded') setPhase('collapsed');
    }, HOVER_COLLAPSE_DELAY_MS);
  };

  const handleTogglePromptMode = () => {
    const isLlmReady = ollamaStatus.status === 'ready' || ollamaStatus.status === 'cloud_active';
    if (!promptMode && !isLlmReady) {
      setWarningMessage('Prompt mode requires a configured LLM');
      setPhase('warning');
      return;
    }
    setPromptMode((prev) => !prev);
    setWarningMessage(null);
  };

  const toggleClickToTalk = async () => {
    if (phase === 'listening') {
      try {
        setPhase('processing');
        const result = await invoke<ProcessedPipelineResult>('stop_capture');
        setPhase('success');
        setSuccessMessage('Text inserted');
        if (onProcessComplete) onProcessComplete(result);
        if (successTimerRef.current) clearTimeout(successTimerRef.current);
        successTimerRef.current = setTimeout(() => setPhase('collapsed'), 2200);
      } catch (err: any) {
        setErrorMessage(err.message || 'Audio capture failed');
        setPhase('error');
      }
    } else {
      try {
        setErrorMessage(null);
        setWarningMessage(null);
        setPhase('listening');
        await invoke('start_capture', { mode: 'scribble' });
      } catch (err: any) {
        setErrorMessage(err.message || 'Failed to start capture');
        setPhase('error');
      }
    }
  };

  const diagnosticsInfo: DiagnosticInfo = {
    state: phase,
    sttStatus: whisperStatus.status,
    llmStatus: ollamaStatus.status,
    hotkeyStatus: hotkeyStatus.status,
    windowMode: popoverOpen ? 'popover' : isExpanded ? 'expanded' : 'resting',
    promptMode,
    activeApp,
  };

  return (
    <div
      className="relative flex flex-col items-center justify-center select-none font-sans"
      onMouseEnter={handleMouseEnter}
      onMouseLeave={handleMouseLeave}
    >
      {/* Dev Diagnostic HUD */}
      {showDiagnostics && (
        <div className="absolute -top-28 left-1/2 -translate-x-1/2 bg-black/90 text-emerald-400 font-mono text-[10px] p-2 rounded-lg border border-emerald-500/30 shadow-xl whitespace-nowrap z-50 flex flex-col gap-0.5">
          <div className="flex items-center gap-1 font-bold border-b border-emerald-500/20 pb-0.5">
            <Bug className="w-3 h-3 text-emerald-400" /> Relay Inspection HUD
          </div>
          <div>State: <span className="text-white">{diagnosticsInfo.state}</span> | Window: <span className="text-white">{diagnosticsInfo.windowMode}</span></div>
          <div>STT: <span className="text-white">{diagnosticsInfo.sttStatus}</span> | LLM: <span className="text-white">{diagnosticsInfo.llmStatus}</span></div>
          <div>Hotkey: <span className="text-white">{diagnosticsInfo.hotkeyStatus}</span> | App: <span className="text-white">{diagnosticsInfo.activeApp}</span></div>
        </div>
      )}

      {/* Floating Hotkey Hint Bar (Rendered above pill on hover/recording, Oscar-style) */}
      {isExpanded && (
        <div className="absolute -top-9 left-1/2 -translate-x-1/2 bg-[#faf8f5]/95 dark:bg-[#1a1918]/95 border border-[#e6e0d5] dark:border-[#2e2c29] shadow-md px-3 py-0.5 rounded-full text-[11px] text-[#595550] dark:text-[#a8a39a] flex items-center gap-1.5 whitespace-nowrap animate-in fade-in slide-in-from-bottom-2 duration-200 z-10">
          <span>Hold to record</span>
          <kbd className="px-1.5 py-0.2 rounded bg-[#eee9df] dark:bg-[#2c2a29] font-mono text-[10px] font-bold text-[#2c2a29] dark:text-[#f0ede8] border border-[#d8d2c4] dark:border-[#383533]">
            Ctrl
          </kbd>
          <kbd className="px-1.5 py-0.2 rounded bg-[#eee9df] dark:bg-[#2c2a29] font-mono text-[10px] font-bold text-[#2c2a29] dark:text-[#f0ede8] border border-[#d8d2c4] dark:border-[#383533]">
            Space
          </kbd>
        </div>
      )}

      {/* Main Surface (Notch vs Oscar Pill) */}
      {!isExpanded ? (
        /* Oscar Edge Horizontal Notch (Resting State) */
        <div
          onClick={toggleClickToTalk}
          className="w-16 h-2 rounded-t-lg bg-[#dfd9cd] dark:bg-[#3d3b38] hover:h-3 transition-all duration-200 cursor-pointer shadow-xs border-t border-x border-[#c8c2b5] dark:border-[#4d4a46]"
          title="Click or hover to expand push-to-talk"
        />
      ) : (
        /* Oscar-Inspired Pill Surface */
        <div
          className={cn(
            'flex items-center justify-between bg-[#faf8f5] dark:bg-[#1a1918] border border-[#e6e0d5] dark:border-[#2e2c29] shadow-xl rounded-full px-4 h-11 w-[430px] transition-all duration-250 ease-out'
          )}
        >
          {/* Left: Active Process / App Indicator (No "RELAY" branding) */}
          <div className="flex items-center gap-2 shrink-0">
            <span className="w-2 h-2 rounded-full bg-[#8c867a] dark:bg-[#999387]" />
            <span className="text-[11px] font-mono font-medium text-[#7a7469] dark:text-[#999387] tracking-wide uppercase max-w-[100px] truncate">
              {activeApp}
            </span>
          </div>

          {/* Center: Main Interactive Action / Recording State */}
          <div className="flex-1 flex items-center justify-center px-2 min-w-0">
            {/* COLLAPSED / EXPANDED IDLE */}
            {(phase === 'collapsed' || phase === 'expanded') && (
              <button
                type="button"
                onClick={toggleClickToTalk}
                className="text-xs font-semibold text-[#2c2a29] dark:text-[#f0ede8] hover:text-[#c26238] transition-colors cursor-pointer truncate"
              >
                {promptMode ? 'Click to prompt' : 'Click to dictate'}
              </button>
            )}

            {/* RECORDING / LISTENING */}
            {phase === 'listening' && (
              <div className="flex items-center gap-3">
                <div className="flex items-center gap-2 text-xs font-bold text-[#c26238] dark:text-[#d97345]">
                  <span className="w-2 h-2 rounded-full bg-[#c26238] dark:bg-[#d97345] animate-ping" />
                  <span>{promptMode ? 'Prompting...' : 'Listening...'}</span>
                </div>

                {/* Real RMS Audio Visualizer Waveform */}
                <div className="flex items-center gap-1 h-4 px-2 bg-[#eee9df]/50 dark:bg-[#262423]/50 rounded-full">
                  {[0.5, 0.9, 0.7, 1.0, 0.6, 0.8].map((factor, i) => {
                    const heightPx = Math.max(3, Math.min(16, Math.round(audioLevel * 16 * factor)));
                    return (
                      <span
                        key={i}
                        className="w-1 rounded-full bg-[#c26238] dark:bg-[#d97345] transition-all duration-75"
                        style={{ height: `${heightPx}px` }}
                      />
                    );
                  })}
                </div>

                <button
                  type="button"
                  onClick={toggleClickToTalk}
                  className="p-1 rounded-full bg-[#f2e6d8] dark:bg-[#382b24] text-[#c26238] dark:text-[#d97345] transition-colors cursor-pointer"
                  title="Stop recording"
                >
                  <Square className="w-3 h-3 fill-current" />
                </button>
              </div>
            )}

            {/* PROCESSING */}
            {phase === 'processing' && (
              <div className="flex items-center gap-2 text-xs text-[#2c2a29] dark:text-[#f0ede8] font-medium truncate">
                <Loader2 className="w-3.5 h-3.5 text-[#c26238] dark:text-[#d97345] animate-spin shrink-0" />
                <span className="font-mono text-[#8c867a] text-[11px] animate-pulse truncate">
                  {PROCESSING_CAPTIONS[captionIndex]}
                </span>
              </div>
            )}

            {/* SUCCESS */}
            {phase === 'success' && (
              <div className="flex items-center gap-1.5 text-xs font-bold text-emerald-600 dark:text-emerald-400 animate-in fade-in duration-200 truncate">
                <CheckCircle2 className="w-3.5 h-3.5 shrink-0" />
                <span className="truncate">{successMessage}</span>
              </div>
            )}

            {/* ERROR */}
            {phase === 'error' && (
              <div className="flex items-center gap-1.5 text-xs text-rose-600 dark:text-rose-400 font-medium truncate">
                <AlertCircle className="w-3.5 h-3.5 shrink-0" />
                <span className="truncate">{errorMessage || 'Something went wrong'}</span>
              </div>
            )}

            {/* WARNING */}
            {phase === 'warning' && (
              <div className="flex items-center gap-1.5 text-xs text-amber-600 dark:text-amber-400 font-medium truncate">
                <AlertCircle className="w-3.5 h-3.5 shrink-0" />
                <span className="truncate">{warningMessage || 'LLM not configured'}</span>
              </div>
            )}
          </div>

          {/* Right Controls: Separator | Sparkle | Chevron */}
          <div className="flex items-center gap-2 shrink-0">
            <div className="h-4 w-px bg-[#e6e0d5] dark:bg-[#2e2c29]" />

            {/* Sparkle (Prompt Mode Button) */}
            <button
              type="button"
              onClick={handleTogglePromptMode}
              className={cn(
                'p-1.5 rounded-full transition-colors cursor-pointer flex items-center justify-center',
                promptMode
                  ? 'bg-[#f5eade] dark:bg-[#382b24] text-[#c26238] dark:text-[#d97345] font-bold shadow-xs'
                  : 'text-[#8c867a] hover:text-[#2c2a29] dark:hover:text-[#f0ede8]'
              )}
              title={promptMode ? 'Prompt Mode Active' : 'Toggle Prompt Mode'}
              aria-label="Toggle Prompt Mode"
            >
              <Sparkles className="w-3.5 h-3.5" />
            </button>

            {/* Chevron (Settings Dropdown) */}
            <button
              type="button"
              onClick={() => setPopoverOpen((prev) => !prev)}
              className={cn(
                'p-1 text-[#8c867a] hover:text-[#2c2a29] dark:hover:text-[#f0ede8] transition-colors cursor-pointer'
              )}
              title="Open Dictation Settings"
              aria-label="Open Dictation settings"
            >
              {popoverOpen ? <ChevronUp className="w-3.5 h-3.5" /> : <ChevronDown className="w-3.5 h-3.5" />}
            </button>
          </div>
        </div>
      )}

      {/* Popover Settings Dropdown Panel */}
      {popoverOpen && (
        <div className="mt-2 z-20">
          <PillSettingsPopover
            settings={settings}
            autoPaste={autoPaste}
            onToggleAutoPaste={setAutoPaste}
            textTransform={textTransform}
            onToggleTextTransform={setTextTransform}
            cleanupStyle={cleanupStyle}
            onChangeCleanupStyle={setCleanupStyle}
            promptMode={promptMode}
            onTogglePromptMode={handleTogglePromptMode}
            language={language}
            onChangeLanguage={setLanguage}
            whisperStatus={whisperStatus}
            ollamaStatus={ollamaStatus}
            hotkeyStatus={hotkeyStatus}
            onRefreshStatuses={refreshDependencies}
            onDownloadWhisper={handleDownloadWhisper}
          />
        </div>
      )}
    </div>
  );
};
