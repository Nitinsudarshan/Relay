import React, { useEffect, useRef, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { AppSettings, LanguageSettings, PillPosition, ProcessedPipelineResult } from '../../types';
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
import { cn, applyThemeWithoutTransition } from '@/lib/utils';
import { playDictationStartSound, playDictationStopSound } from '@/lib/soundEffects';

export function getPillLanguageFromSettings(lang?: LanguageSettings): SpeechLanguage {
  if (!lang) return 'auto';
  const primary = (lang.primary_dictation_language || '').trim().toLowerCase();
  const spoken = (lang.spoken_languages || []).map((s) => s.trim().toLowerCase());

  if (primary === 'auto' || !primary) return 'auto';
  if (spoken.includes('en') && spoken.includes('hi')) return 'hinglish';
  if (primary === 'hi') return 'hindi';
  if (primary === 'es') return 'es';
  if (primary === 'en') return 'english';
  return (primary as SpeechLanguage);
}

interface CaptureStatePayload {
  active: boolean;
  mode: string | null;
  status?: string;
  message?: string;
}

interface DictationPillProps {
  onProcessComplete?: (result: ProcessedPipelineResult) => void;
}

const HOVER_EXPAND_DELAY_MS = 150;
const HOVER_COLLAPSE_DELAY_MS = 1000;

// One rolling level sample per bar — a real scrolling waveform (each bar is
// its own recent-history sample) rather than one scalar level reused to
// scale a fixed decorative shape, which at silence still rendered as a
// non-zero pattern instead of going flat.
const WAVEFORM_BAR_COUNT = 15;
const WAVEFORM_AMPLITUDE = 2;
const SILENT_LEVEL_HISTORY = new Array(WAVEFORM_BAR_COUNT).fill(0);

function renderHotkeyKeys(shortcutStr: string) {
  if (!shortcutStr) return null;
  const normalized = shortcutStr
    .replace(/CommandOrControl/i, 'Ctrl')
    .replace(/Control/i, 'Ctrl')
    .replace(/Cmd/i, 'Ctrl');

  const keys = normalized.split('+').map((k) => k.trim()).filter(Boolean);

  return (
    <span className="inline-flex items-center gap-1">
      {keys.map((key, idx) => (
        <kbd
          key={idx}
          className="font-mono text-[10px] px-1.5 py-0.5 rounded-lg bg-slate-100 dark:bg-[#262626] text-slate-800 dark:text-neutral-200 border border-slate-200 dark:border-[#404040] font-semibold shadow-xs"
        >
          {key}
        </kbd>
      ))}
    </span>
  );
}

export const DictationPill: React.FC<DictationPillProps> = ({ onProcessComplete }) => {
  const [phase, setPhase] = useState<PillState>('collapsed');
  const [hovering, setHovering] = useState(false);
  const [popoverOpen, setPopoverOpen] = useState(false);
  
  // Settings & Toggles
  const [autoPaste, setAutoPaste] = useState(true);
  const [textTransform, setTextTransform] = useState(true);
  const [cleanupStyle, setCleanupStyle] = useState<CleanupStyle>('faithful');
  const [language, setLanguage] = useState<SpeechLanguage>('auto');
  const [dictationShortcut, setDictationShortcut] = useState('Ctrl+Space');

  // Audio Level & Mode-Aware Status
  const [levelHistory, setLevelHistory] = useState<number[]>(SILENT_LEVEL_HISTORY);
  const [captureMode, setCaptureMode] = useState<string | null>(null);
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

  const successTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const hoverEnterTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const hoverLeaveTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const settingsRef = useRef<AppSettings | null>(null);
  const isRecordingRef = useRef<boolean>(false);

  const isExpanded = (phase !== 'collapsed' && phase !== 'hidden_notch' && phase !== 'error' && phase !== 'warning') || hovering || popoverOpen;

  const handleLanguageChange = async (newLang: SpeechLanguage) => {
    setLanguage(newLang);

    let currentSettings = settings;
    if (!currentSettings) {
      try {
        currentSettings = await invoke<AppSettings>('get_settings');
      } catch {
        return;
      }
    }

    let primary = 'en';
    let spoken = ['en'];

    if (newLang === 'auto') {
      primary = 'auto';
      spoken = ['en'];
    } else if (newLang === 'hindi') {
      primary = 'hi';
      spoken = ['hi'];
    } else if (newLang === 'hinglish') {
      primary = 'en';
      spoken = ['en', 'hi'];
    } else if (newLang === 'english') {
      primary = 'en';
      spoken = ['en'];
    } else if (newLang === 'es') {
      primary = 'es';
      spoken = ['es'];
    } else {
      primary = newLang;
      spoken = [newLang];
    }

    const updatedLanguage: LanguageSettings = {
      ...currentSettings.language,
      primary_dictation_language: primary,
      spoken_languages: spoken,
      notes_language: currentSettings.language?.notes_language || 'en',
      output_script: currentSettings.language?.output_script || 'latin',
    };

    const updatedSettings: AppSettings = {
      ...currentSettings,
      language: updatedLanguage,
    };

    setSettings(updatedSettings);
    settingsRef.current = updatedSettings;

    try {
      await invoke('save_settings', { settings: updatedSettings });
    } catch (err) {
      console.error('Failed to save language settings from pill', err);
    }
  };

  const handleToggleDictationSounds = async (enabled: boolean) => {
    let currentSettings = settings || settingsRef.current;
    if (!currentSettings) {
      try {
        currentSettings = await invoke<AppSettings>('get_settings');
      } catch {
        return;
      }
    }

    const updatedSettings: AppSettings = {
      ...currentSettings,
      sound: {
        ...currentSettings.sound,
        dictation_sounds: enabled,
      },
    };

    setSettings(updatedSettings);
    settingsRef.current = updatedSettings;

    try {
      await invoke('save_settings', { settings: updatedSettings });
    } catch (err) {
      console.error('Failed to save sound settings from pill', err);
    }
  };

  const handleToggleAutoPaste = async (val: boolean) => {
    setAutoPaste(val);
    let currentSettings = settings || settingsRef.current;
    if (!currentSettings) {
      try {
        currentSettings = await invoke<AppSettings>('get_settings');
      } catch {
        return;
      }
    }

    const updatedSettings: AppSettings = {
      ...currentSettings,
      clipboard: {
        ...currentSettings.clipboard,
        auto_paste: val,
        copy_to_clipboard: currentSettings.clipboard?.copy_to_clipboard ?? true,
      },
    };

    setSettings(updatedSettings);
    settingsRef.current = updatedSettings;

    try {
      await invoke('save_settings', { settings: updatedSettings });
    } catch (err) {
      console.error('Failed to save auto-paste setting from pill', err);
    }
  };

  // Real-time Theme Syncing (Light / Dark / System)
  useEffect(() => {
    const applyTheme = (mode: string) => {
      const isSystemDark = window.matchMedia('(prefers-color-scheme: dark)').matches;
      const isDark = mode === 'dark' || (mode === 'system' && isSystemDark);
      applyThemeWithoutTransition(isDark);
    };

    const initialTheme = localStorage.getItem('relay-theme') || 'system';
    applyTheme(initialTheme);

    const unlistenThemePromise = listen<string>('relay-theme-changed', ({ payload }) => {
      if (payload) applyTheme(payload);
    });

    const unlistenPositionPromise = listen<string>('pill-position-changed', ({ payload }) => {
      if (payload) {
        setSettings((prev) =>
          prev ? { ...prev, ui: { ...prev.ui, pill_position: payload as PillPosition } } : prev
        );
      }
    });

    const unlistenSettingsPromise = listen<AppSettings>('settings-changed', ({ payload }) => {
      if (payload) {
        setSettings(payload);
        settingsRef.current = payload;
        if (payload.language) {
          setLanguage(getPillLanguageFromSettings(payload.language));
        }
        if (payload.clipboard) {
          setAutoPaste(payload.clipboard.auto_paste ?? true);
        }
        if (payload.hotkeys?.dictation_hotkey) {
          setDictationShortcut(payload.hotkeys.dictation_hotkey);
        }
      }
    });

    const unlistenClipboardCopy = listen<string>('dictation-clipboard-copy', ({ payload }) => {
      // The native Rust backend sets the OS clipboard directly via arboard.
      // If the webview happens to be focused, sync web clipboard silently without noisy DOMExceptions.
      if (payload && typeof document !== 'undefined' && document.hasFocus()) {
        navigator.clipboard.writeText(payload).catch(() => {});
      }
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
      unlistenPositionPromise.then((unlisten) => unlisten());
      unlistenSettingsPromise.then((unlisten) => unlisten());
      unlistenClipboardCopy.then((unlisten) => unlisten());
      window.removeEventListener('storage', handleStorageChange);
      mediaQuery.removeEventListener('change', handleMediaChange);
    };
  }, []);

  const refreshDependencies = async () => {
    try {
      const appSettings = await invoke<AppSettings>('get_settings');
      setSettings(appSettings);
      settingsRef.current = appSettings;
      if (appSettings?.language) {
        setLanguage(getPillLanguageFromSettings(appSettings.language));
      }
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
          isRecordingRef.current = true;
          setPhase('listening');
        }
      })
      .catch(() => {});

    const unlistenState = listen<CaptureStatePayload>('capture-state-changed', ({ payload }) => {
      if (payload.mode) {
        setCaptureMode(payload.mode);
      }
      if (payload.active) {
        if (!isRecordingRef.current) {
          isRecordingRef.current = true;
          if (settingsRef.current?.sound?.dictation_sounds ?? true) {
            playDictationStartSound();
          }
        }
        setPhase('listening');
        setErrorMessage(null);
        setWarningMessage(null);
        // Fresh recording, fresh waveform — never carry the previous
        // session's trailing history into a new one.
        setLevelHistory(SILENT_LEVEL_HISTORY);
      } else {
        if (isRecordingRef.current) {
          isRecordingRef.current = false;
          if (settingsRef.current?.sound?.dictation_sounds ?? true) {
            playDictationStopSound();
          }
        }
        if (payload.status === 'TRANSCRIBING' || payload.status === 'PROCESSING') {
          setPhase('processing');
        } else if (payload.status === 'SUCCESS') {
          setPhase('success');
          const isDictation = payload.mode === 'dictation' || captureMode === 'dictation';
          if (isDictation) {
            setSuccessMessage('Text inserted');
          } else {
            setSuccessMessage('Voice note saved');
          }
          if (successTimerRef.current) clearTimeout(successTimerRef.current);
          successTimerRef.current = setTimeout(() => setPhase('collapsed'), 2200);
        } else if (payload.status === 'FOCUS_CHANGED') {
          setPhase('success');
          setSuccessMessage(payload.message || 'Copied to clipboard (focus moved)');
          if (successTimerRef.current) clearTimeout(successTimerRef.current);
          successTimerRef.current = setTimeout(() => setPhase('collapsed'), 3200);
        } else if (payload.status === 'ERROR' || payload.message) {
          setPhase('error');
          setErrorMessage(payload.message || 'Capture processing failed');
        } else if (payload.status === 'NO_SPEECH') {
          // Recording stopped but no usable audio was ever received — never a
          // transcription/processing result to show, just back to idle.
          console.debug('[Dictation] Recording stopped with no audio input');
          setPhase('collapsed');
        } else {
          setPhase('collapsed');
        }
      }
    });

    const unlistenLevel = listen<{ level: number }>('capture-level', ({ payload }) => {
      const level = payload.level || 0;
      setLevelHistory((prev) => [...prev.slice(1), level]);
    });

    // Reflects the real OS-level registration outcome (emitted by Rust on
    // every register/re-register) instead of the optimistic default above —
    // a hotkey can fail to register (e.g. a conflict with another app or
    // the OS's own IME binding on that combination), and that must be
    // visible here rather than silently claimed as "registered".
    const unlistenHotkeyStatus = listen<{
      dictation_hotkey: string;
      dictation_registered: boolean;
      dictation_error?: string | null;
    }>('hotkey-status-changed', ({ payload }) => {
      setHotkeyStatus({
        status: payload.dictation_registered ? 'registered' : 'conflict',
        hotkey: payload.dictation_hotkey,
        message: payload.dictation_error || undefined,
      });
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
      unlistenHotkeyStatus.then((fn) => fn());
      window.removeEventListener('keydown', handleKeyDown);
      if (successTimerRef.current) clearTimeout(successTimerRef.current);
      if (hoverEnterTimerRef.current) clearTimeout(hoverEnterTimerRef.current);
      if (hoverLeaveTimerRef.current) clearTimeout(hoverLeaveTimerRef.current);
    };
  }, [captureMode]);

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



  const toggleClickToTalk = async () => {
    if (phase === 'listening') {
      try {
        // Deliberately no optimistic `setPhase('processing')` here — the
        // backend only emits a TRANSCRIBING status (which flips this to
        // 'processing' via the capture-state-changed listener above) once it
        // has confirmed real audio was captured. A silent/empty recording
        // emits NO_SPEECH instead and this call resolves to `null`, so the
        // pill must never claim to be transcribing before that's known.
        const result = await invoke<ProcessedPipelineResult | null>('stop_capture');
        if (result) {
          setPhase('success');
          setSuccessMessage('Voice note saved');
          if (onProcessComplete) onProcessComplete(result);
          if (successTimerRef.current) clearTimeout(successTimerRef.current);
          successTimerRef.current = setTimeout(() => setPhase('collapsed'), 2200);
        } else {
          setPhase('collapsed');
        }
      } catch (err: any) {
        setErrorMessage(err.message || 'Audio capture failed');
        setPhase('error');
      }
    } else {
      try {
        setErrorMessage(null);
        setWarningMessage(null);
        setCaptureMode('voice_note');
        // No optimistic setPhase('listening') here either — the native
        // recorder is the source of truth for whether capture actually
        // started. Claiming "listening" before start_capture resolves (or
        // when it fails) is exactly the two-sources-of-truth pattern that
        // let the pill show recording state independent of whether
        // anything was actually recording. The capture-state-changed
        // listener above flips this to 'listening' once Rust confirms it.
        await invoke('start_capture', { mode: 'voice_note' });
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
    activeApp: 'Relay',
  };

  const pillPos = settings?.ui?.pill_position || 'bottom_center';
  const isLeft = pillPos === 'bottom_left';
  const isRight = pillPos === 'bottom_right';

  return (
    <div
      className={cn(
        "absolute inset-0 flex items-end select-none font-sans overflow-hidden bg-transparent",
        popoverOpen && "pointer-events-auto",
        isLeft ? "justify-start" : isRight ? "justify-end" : "justify-center"
      )}
      onMouseEnter={handleMouseEnter}
      onMouseLeave={handleMouseLeave}
    >
      {/* Dev Diagnostic HUD */}
      {showDiagnostics && (
        <div
          className={cn(
            "absolute top-2 bg-slate-950/95 text-emerald-400 font-mono text-[10px] p-2 rounded-lg border border-emerald-500/30 shadow-xl whitespace-nowrap z-50 flex flex-col gap-0.5",
            isLeft ? "left-4" : isRight ? "right-4" : "left-1/2 -translate-x-1/2"
          )}
        >
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
            ? (isLeft ? 'left-0 w-[140px] h-[16px] pointer-events-auto' : isRight ? 'right-0 w-[140px] h-[16px] pointer-events-auto' : 'left-1/2 -translate-x-1/2 w-[140px] h-[16px] pointer-events-auto')
            : 'left-0 w-full h-full pointer-events-auto'
        )}
      />

      {/* Edge Handle / Notch — 25% wider (~96px), rounded-t-lg top part, neutral dark background in dark mode */}
      <div
        className={cn(
          'absolute bottom-0 transition-all duration-200 pointer-events-none z-20',
          isLeft ? 'left-4' : isRight ? 'right-4' : 'left-1/2 -translate-x-1/2',
          'bg-white dark:bg-[#171717] border border-slate-200 dark:border-[#262626] border-b-0 shadow-[0_-1px_6px_rgba(0,0,0,0.15)]',
          !hovering ? 'w-[96px] h-[6px] rounded-t-lg' : 'w-[110px] h-[7px] rounded-t-xl shadow-[0_-2px_12px_rgba(37,99,235,0.3)]',
          isExpanded && 'opacity-0 pointer-events-none'
        )}
      />

      {/* Keyboard & Contextual Hint Bar (floating above main pill) */}
      {(hovering || phase === 'listening' || phase === 'expanded') &&
        !popoverOpen &&
        phase !== 'processing' &&
        phase !== 'success' &&
        phase !== 'error' &&
        phase !== 'warning' && (
          <div
            className={cn(
              "absolute bottom-[70px] bg-white/95 dark:bg-[#171717]/95 border border-slate-200 dark:border-[#262626] shadow-md rounded-lg px-3 py-1.5 whitespace-nowrap flex items-center gap-1.5 text-xs text-slate-700 dark:text-neutral-300 pointer-events-none z-30 animate-in fade-in slide-in-from-bottom-1 duration-150",
              isLeft ? "left-4" : isRight ? "right-4" : "left-1/2 -translate-x-1/2"
            )}
          >
            {(phase as PillState) === 'listening' ? (
              settings?.hotkeys?.toggle_to_talk ? (
                <span className="flex items-center gap-1">
                  <span className="text-slate-500 dark:text-neutral-400 font-medium">Press</span>
                  {renderHotkeyKeys(dictationShortcut)}
                  <span className="text-slate-500 dark:text-neutral-400 font-medium">to stop</span>
                </span>
              ) : (
                <span className="text-slate-500 dark:text-neutral-400 font-medium">
                  Release to stop
                </span>
              )
            ) : settings?.hotkeys?.toggle_to_talk ? (
              <span className="flex items-center gap-1">
                <span className="text-slate-500 dark:text-neutral-400 font-medium">Press</span>
                {renderHotkeyKeys(dictationShortcut)}
                <span className="text-slate-500 dark:text-neutral-400 font-medium">to record</span>
              </span>
            ) : (
              <span className="flex items-center gap-1">
                <span className="text-slate-500 dark:text-neutral-400 font-medium">Hold</span>
                {renderHotkeyKeys(dictationShortcut)}
                <span className="text-slate-500 dark:text-neutral-400 font-medium">to record</span>
              </span>
            )}
          </div>
        )}

      {/* Main Relay Pill Surface (Process label removed, dark theme matching #171717) */}
      <div
        className={cn(
          'absolute bottom-[16px] transition-all duration-200 ease-out pointer-events-none z-30',
          isLeft ? 'left-4' : isRight ? 'right-4' : 'left-1/2 -translate-x-1/2',
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
          className="inline-flex items-center gap-0 pl-4 pr-2 h-[44px] rounded-lg bg-white dark:bg-[#171717] border border-slate-200 dark:border-[#262626] shadow-[0_16px_40px_rgba(15,23,42,0.15),0_2px_8px_rgba(15,23,42,0.08)] dark:shadow-[0_16px_40px_rgba(0,0,0,0.5)] text-slate-900 dark:text-neutral-100 cursor-pointer"
        >
          <div className="flex items-center w-[120px] shrink-0 pr-2.5 h-[22px] overflow-hidden">
            {/* Phase 1: IDLE / READY — Click to dictate */}
            {(phase === 'collapsed' || phase === 'expanded') && (
              <div className="flex items-center gap-2 w-full overflow-hidden">
                <span className="text-xs font-semibold text-slate-900 dark:text-neutral-100 tracking-tight whitespace-nowrap truncate">
                  Click to dictate
                </span>
              </div>
            )}

            {/* Phase 2: RECORDING — 15 bars, each a real recent-history
                audio-level sample (a scrolling waveform), not one scalar
                scaling a fixed decorative shape. Silence keeps every sample
                near 0, so the whole row collapses to its hairline minimum
                instead of showing a predetermined pattern. */}
            {phase === 'listening' && (
              <div className="flex items-center gap-[2.5px] h-[22px] shrink-0 overflow-hidden">
                {levelHistory.map((level, i) => {
                  const heightPx = Math.min(
                    22,
                    Math.max(3, Math.round(level * 22 * WAVEFORM_AMPLITUDE))
                  );
                  return (
                    <span
                      key={i}
                      className="w-[2.5px] bg-blue-600 dark:bg-blue-400 rounded-sm transition-all duration-75 origin-center shrink-0"
                      style={{ height: `${heightPx}px` }}
                    />
                  );
                })}
              </div>
            )}

            {/* Phase 3: PROCESSING */}
            {phase === 'processing' && (
              <div className="flex items-center gap-2 w-full overflow-hidden">
                <span className="w-2.5 h-2.5 rounded-full border-[1.4px] border-slate-300 dark:border-neutral-700 border-t-blue-600 dark:border-t-blue-400 animate-spin shrink-0" />
                <span className="font-mono text-[10.5px] tracking-wide text-slate-600 dark:text-neutral-400 whitespace-nowrap truncate animate-in fade-in duration-200">
                  Transcribing...
                </span>
              </div>
            )}

            {/* Phase 4: ERROR */}
            {phase === 'error' && (
              <div 
                onClick={(e) => {
                  e.stopPropagation();
                  invoke('open_settings_window').catch(console.error);
                }}
                className="flex items-center gap-1.5 w-full overflow-hidden text-rose-600 dark:text-rose-400 text-xs font-medium cursor-pointer hover:underline"
                title="Click to open settings in main window"
              >
                <AlertTriangle className="w-3.5 h-3.5 shrink-0" />
                <span className="truncate">{errorMessage || 'Set Whisper model in Settings'}</span>
              </div>
            )}

            {/* Phase 5: WARNING */}
            {phase === 'warning' && (
              <div 
                onClick={(e) => {
                  e.stopPropagation();
                  invoke('open_settings_window').catch(console.error);
                }}
                className="flex items-center gap-1.5 w-full overflow-hidden text-amber-600 dark:text-amber-400 text-xs font-medium cursor-pointer hover:underline"
                title="Click to open settings in main window"
              >
                <AlertTriangle className="w-3.5 h-3.5 shrink-0" />
                <span className="truncate">{warningMessage || 'Sign in to enable AI'}</span>
              </div>
            )}

            {/* Phase 6: SUCCESS */}
            {phase === 'success' && (
              <div className="flex items-center gap-1.5 w-full overflow-hidden text-blue-600 dark:text-blue-400 text-xs font-semibold animate-in fade-in duration-200">
                <Check className="w-3.5 h-3.5 stroke-[2.5] shrink-0" />
                <span className="truncate">{successMessage}</span>
              </div>
            )}
          </div>

          <div className="w-px h-[20px] bg-slate-200 dark:bg-[#262626] shrink-0" />

          {/* Action Buttons: Settings Chevron */}
          <div className="flex items-center gap-1 pl-1.5 shrink-0 pill-actions">
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
          onToggleAutoPaste={handleToggleAutoPaste}
          textTransform={textTransform}
          onToggleTextTransform={setTextTransform}
          onToggleDictationSounds={handleToggleDictationSounds}
          cleanupStyle={cleanupStyle}
          onChangeCleanupStyle={setCleanupStyle}
          language={language}
          onChangeLanguage={handleLanguageChange}
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
