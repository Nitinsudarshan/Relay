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
  AlertTriangle, 
  Check, 
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

const HOVER_EXPAND_DELAY_MS = 150;
const HOVER_COLLAPSE_DELAY_MS = 1000;

export const DictationPill: React.FC<DictationPillProps> = ({ onProcessComplete }) => {
  const [phase, setPhase] = useState<PillState>('collapsed');
  const [hovering, setHovering] = useState(false);
  const [popoverOpen, setPopoverOpen] = useState(false);
  
  // Settings & Toggles
  const [autoPaste, setAutoPaste] = useState(true);
  const [textTransform, setTextTransform] = useState(true);
  const [cleanupStyle, setCleanupStyle] = useState<CleanupStyle>('faithful');
  const [promptMode, setPromptMode] = useState(false);
  const [language, setLanguage] = useState<SpeechLanguage>('english');
  const [dictationShortcut, setDictationShortcut] = useState('Ctrl+Space');

  // Audio Level & Captions
  const [audioLevel, setAudioLevel] = useState(0);
  const [captionIndex, setCaptionIndex] = useState(0);
  const [errorMessage, setErrorMessage] = useState<string | null>(null);
  const [warningMessage, setWarningMessage] = useState<string | null>(null);
  const [successMessage, setSuccessMessage] = useState<string>('Inserted into document');
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

  // Real-time Theme Syncing (Light / Dark / System)
  useEffect(() => {
    const applyTheme = (mode: string) => {
      const isSystemDark = window.matchMedia('(prefers-color-scheme: dark)').matches;
      const isDark = mode === 'dark' || (mode === 'system' && isSystemDark);
      if (isDark) {
        document.documentElement.classList.add('dark');
      } else {
        document.documentElement.classList.remove('dark');
      }
    };

    const initialTheme = localStorage.getItem('relay-theme') || 'system';
    applyTheme(initialTheme);

    const unlistenThemePromise = listen<string>('relay-theme-changed', ({ payload }) => {
      if (payload) applyTheme(payload);
    });

    const handleStorageChange = (e: StorageEvent) => {
      if (e.key === 'relay-theme') {
        applyTheme(e.newValue || 'system');
      }
    };
    window.addEventListener('storage', handleStorageChange);

    const mediaQuery = window.matchMedia('(prefers-color-scheme: dark)');
    const handleMediaChange = () => {
      const current = localStorage.getItem('relay-theme') || 'system';
      if (current === 'system') applyTheme('system');
    };
    mediaQuery.addEventListener('change', handleMediaChange);

    return () => {
      unlistenThemePromise.then((unlisten) => unlisten());
      window.removeEventListener('storage', handleStorageChange);
      mediaQuery.removeEventListener('change', handleMediaChange);
    };
  }, []);

  const refreshDependencies = async () => {
    try {
      const appSettings = await invoke<AppSettings>('get_settings');
      setSettings(appSettings);
      if (appSettings?.hotkeys?.dictation_hotkey) {
        setDictationShortcut(appSettings.hotkeys.dictation_hotkey);
        setHotkeyStatus({ status: 'registered', hotkey: appSettings.hotkeys.dictation_hotkey });
      }

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

    const unlistenState = listen<CaptureStatePayload>('capture-state-changed', ({ payload }) => {
      if (payload.active) {
        setPhase('listening');
        setErrorMessage(null);
        setWarningMessage(null);
      } else if (payload.status === 'TRANSCRIBING' || payload.status === 'PROCESSING') {
        setPhase('processing');
      } else if (payload.status === 'SUCCESS') {
        setPhase('success');
        setSuccessMessage(promptMode ? 'Prompt inserted into document' : 'Inserted into document');
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

  // Update Rust native window geometry
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

  // Hover Handlers
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
        setSuccessMessage('Inserted into document');
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
    activeApp: 'Relay',
  };

  return (
    <div
      className="absolute inset-0 flex items-end justify-center select-none font-sans overflow-hidden bg-transparent"
      onMouseEnter={handleMouseEnter}
      onMouseLeave={handleMouseLeave}
    >
      {/* Dev Diagnostic HUD */}
      {showDiagnostics && (
        <div className="absolute top-2 left-1/2 -translate-x-1/2 bg-slate-950/95 text-emerald-400 font-mono text-[10px] p-2 rounded-lg border border-emerald-500/30 shadow-xl whitespace-nowrap z-50 flex flex-col gap-0.5">
          <div className="flex items-center gap-1 font-bold border-b border-emerald-500/20 pb-0.5">
            <Bug className="w-3 h-3 text-emerald-400" /> Relay Inspection HUD
          </div>
          <div>State: <span className="text-white">{diagnosticsInfo.state}</span> | Window: <span className="text-white">{diagnosticsInfo.windowMode}</span></div>
          <div>STT: <span className="text-white">{diagnosticsInfo.sttStatus}</span> | LLM: <span className="text-white">{diagnosticsInfo.llmStatus}</span></div>
          <div>Hotkey: <span className="text-white">{diagnosticsInfo.hotkeyStatus}</span></div>
        </div>
      )}

      {/* Hit zone for cursor hit testing */}
      <div
        className={cn(
          'absolute bottom-0 z-10 transition-all bg-black/[0.001]',
          !isExpanded
            ? 'left-1/2 -translate-x-1/2 w-[140px] h-[16px] pointer-events-auto'
            : 'left-0 w-full h-full pointer-events-auto'
        )}
      />

      {/* Edge Handle / Notch — 25% wider (~96px), rounded-t-lg top part, neutral dark background in dark mode */}
      <div
        className={cn(
          'absolute left-1/2 bottom-0 -translate-x-1/2 transition-all duration-200 pointer-events-none z-20',
          'bg-white dark:bg-[#171717] border border-slate-200 dark:border-[#262626] border-b-0 shadow-[0_-1px_6px_rgba(0,0,0,0.15)]',
          !hovering ? 'w-[96px] h-[6px] rounded-t-lg' : 'w-[110px] h-[7px] rounded-t-xl shadow-[0_-2px_12px_rgba(37,99,235,0.3)]',
          isExpanded && 'opacity-0 pointer-events-none'
        )}
      />

      {/* Success Toast */}
      {phase === 'success' && (
        <div className="absolute left-1/2 bottom-[82px] -translate-x-1/2 bg-blue-600 dark:bg-blue-500 text-white px-3.5 py-1.5 rounded-lg text-xs font-semibold tracking-wide flex items-center gap-1.5 shadow-lg pointer-events-none z-40 animate-in fade-in slide-in-from-bottom-2 duration-200">
          <Check className="w-3.5 h-3.5 stroke-[2.5]" />
          <span>{successMessage}</span>
        </div>
      )}

      {/* Keyboard hint bar (floating above main pill, matching application dark card #171717) */}
      {isExpanded && !popoverOpen && phase !== 'success' && (
        <div className="absolute left-1/2 bottom-[70px] -translate-x-1/2 bg-white/95 dark:bg-[#171717]/95 border border-slate-200 dark:border-[#262626] shadow-md rounded-lg px-3 py-1.5 whitespace-nowrap flex items-center gap-2 text-xs text-slate-700 dark:text-neutral-300 pointer-events-none z-30 animate-in fade-in slide-in-from-bottom-1 duration-150">
          <span className="text-slate-500 dark:text-neutral-400 font-medium">Hold to record</span>
          <span className="flex items-center gap-1">
            <kbd className="font-mono text-[10px] px-1.5 py-0.5 rounded-md bg-slate-100 dark:bg-[#262626] text-slate-800 dark:text-neutral-200 border border-slate-200 dark:border-[#404040] font-semibold shadow-xs">
              Ctrl
            </kbd>
            <kbd className="font-mono text-[10px] px-1.5 py-0.5 rounded-md bg-slate-100 dark:bg-[#262626] text-slate-800 dark:text-neutral-200 border border-slate-200 dark:border-[#404040] font-semibold shadow-xs">
              Space
            </kbd>
          </span>
        </div>
      )}

      {/* Main Relay Pill Surface (Process label removed, dark theme matching #171717) */}
      <div
        className={cn(
          'absolute left-1/2 bottom-[16px] -translate-x-1/2 transition-all duration-200 ease-out pointer-events-none z-30',
          !isExpanded
            ? 'translate-y-[28px] scale-75 opacity-0'
            : 'translate-y-0 scale-100 opacity-100 pointer-events-auto'
        )}
      >
        <div
          onClick={(e) => {
            if ((e.target as HTMLElement).closest('.pill-actions')) return;
            toggleClickToTalk();
          }}
          className="inline-flex items-center gap-0 pl-4 pr-2 h-[44px] rounded-xl bg-white dark:bg-[#171717] border border-slate-200 dark:border-[#262626] shadow-[0_16px_40px_rgba(15,23,42,0.15),0_2px_8px_rgba(15,23,42,0.08)] dark:shadow-[0_16px_40px_rgba(0,0,0,0.5)] text-slate-900 dark:text-neutral-100 cursor-pointer"
        >
          <div className="flex items-center min-w-[120px] pr-2.5 h-[22px]">
            {/* Phase 1: IDLE / READY — Click to dictate */}
            {(phase === 'collapsed' || phase === 'expanded') && (
              <div className="flex items-center gap-2 min-w-0">
                <span className="text-xs font-semibold text-slate-900 dark:text-neutral-100 tracking-tight whitespace-nowrap">
                  {promptMode ? 'Click to prompt' : 'Click to dictate'}
                </span>
              </div>
            )}

            {/* Phase 2: RECORDING — 15 Relay primary blue waveform bars */}
            {phase === 'listening' && (
              <div className="flex items-center gap-[2.5px] h-[22px] shrink-0">
                {[0.35, 0.55, 0.85, 0.45, 0.7, 0.95, 0.6, 0.4, 0.75, 0.55, 0.3, 0.6, 0.85, 0.5, 0.7].map((h, i) => {
                  const scale = 0.34 + audioLevel * 0.8;
                  const heightPx = Math.max(3, Math.round(h * 22 * scale));
                  return (
                    <span
                      key={i}
                      className="w-[2.5px] bg-blue-600 dark:bg-blue-400 rounded-sm transition-all duration-75 origin-center"
                      style={{ height: `${heightPx}px` }}
                    />
                  );
                })}
              </div>
            )}

            {/* Phase 3: PROCESSING */}
            {phase === 'processing' && (
              <div className="flex items-center gap-2 max-w-[220px] overflow-hidden">
                <span className="w-2.5 h-2.5 rounded-full border-[1.4px] border-slate-300 dark:border-neutral-700 border-t-blue-600 dark:border-t-blue-400 animate-spin shrink-0" />
                <span className="font-mono text-[10.5px] tracking-wide text-slate-600 dark:text-neutral-400 whitespace-nowrap animate-in fade-in duration-200">
                  {PROCESSING_CAPTIONS[captionIndex]}
                </span>
              </div>
            )}

            {/* Phase 4: ERROR */}
            {phase === 'error' && (
              <div className="flex items-center gap-1.5 text-rose-600 dark:text-rose-400 text-xs font-medium">
                <AlertTriangle className="w-3.5 h-3.5 shrink-0" />
                <span className="truncate">{errorMessage || 'no input'}</span>
              </div>
            )}

            {/* Phase 5: WARNING */}
            {phase === 'warning' && (
              <div className="flex items-center gap-1.5 text-amber-600 dark:text-amber-400 text-xs font-medium">
                <AlertTriangle className="w-3.5 h-3.5 shrink-0" />
                <span className="truncate">{warningMessage || 'Sign in to enable AI'}</span>
              </div>
            )}
          </div>

          <div className="w-px h-[20px] bg-slate-200 dark:bg-[#262626] shrink-0" />

          {/* Action Buttons: Prompt Mode + Settings Chevron */}
          <div className="flex items-center gap-1 pl-1.5 shrink-0 pill-actions">
            <button
              type="button"
              onClick={(e) => {
                e.stopPropagation();
                handleTogglePromptMode();
              }}
              className={cn(
                'w-7 h-7 rounded-lg border-none bg-transparent flex items-center justify-center cursor-pointer transition-colors p-0',
                promptMode
                  ? 'text-blue-600 dark:text-blue-400 bg-blue-50 dark:bg-blue-950/40 font-bold'
                  : 'text-slate-400 dark:text-neutral-400 hover:text-slate-800 dark:hover:text-neutral-100 hover:bg-slate-100 dark:hover:bg-[#262626]'
              )}
              title="Prompt mode — rewrite speech into a prompt"
              aria-label="Prompt mode"
            >
              <Sparkles className="w-3.5 h-3.5 stroke-[1.5]" />
            </button>

            <button
              type="button"
              onClick={(e) => {
                e.stopPropagation();
                setPopoverOpen((prev) => !prev);
              }}
              className={cn(
                'w-7 h-7 rounded-lg border-none bg-transparent text-slate-700 dark:text-neutral-300 flex items-center justify-center cursor-pointer transition-colors p-0 hover:bg-slate-100 dark:hover:bg-[#262626]',
                popoverOpen && 'bg-slate-100 dark:bg-[#262626] text-slate-900 dark:text-neutral-100'
              )}
              title="Settings"
              aria-label="Settings"
            >
              {popoverOpen ? (
                <ChevronUp className="w-3.5 h-3.5 stroke-[2]" />
              ) : (
                <ChevronDown className="w-3.5 h-3.5 stroke-[2]" />
              )}
            </button>
          </div>
        </div>
      </div>

      {/* Settings Popover Dropdown */}
      {popoverOpen && (
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
      )}
    </div>
  );
};
