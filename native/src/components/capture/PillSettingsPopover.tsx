import React from 'react';
import { AppSettings } from '../../types';
import { WhisperStatusInfo, OllamaStatusInfo, HotkeyStatusInfo, CleanupStyle, SpeechLanguage } from './PillTypes';
import { 
  Sparkles, 
  CheckCircle2, 
  AlertTriangle, 
  Download, 
  Keyboard, 
  RefreshCw,
  Cpu,
  Edit3,
  Globe
} from 'lucide-react';
import { cn } from '@/lib/utils';

interface PillSettingsPopoverProps {
  settings: AppSettings | null;
  autoPaste: boolean;
  onToggleAutoPaste: (val: boolean) => void;
  textTransform: boolean;
  onToggleTextTransform: (val: boolean) => void;
  cleanupStyle: CleanupStyle;
  onChangeCleanupStyle: (style: CleanupStyle) => void;
  promptMode: boolean;
  onTogglePromptMode: () => void;
  language: SpeechLanguage;
  onChangeLanguage: (lang: SpeechLanguage) => void;
  whisperStatus: WhisperStatusInfo;
  ollamaStatus: OllamaStatusInfo;
  hotkeyStatus: HotkeyStatusInfo;
  onRefreshStatuses: () => void;
  onDownloadWhisper: () => void;
}

export const PillSettingsPopover: React.FC<PillSettingsPopoverProps> = ({
  autoPaste,
  onToggleAutoPaste,
  textTransform,
  onToggleTextTransform,
  cleanupStyle,
  onChangeCleanupStyle,
  promptMode,
  onTogglePromptMode,
  language,
  onChangeLanguage,
  whisperStatus,
  ollamaStatus,
  hotkeyStatus,
  onRefreshStatuses,
  onDownloadWhisper,
}) => {
  const isLlmReady = ollamaStatus.status === 'ready' || ollamaStatus.status === 'cloud_active';

  return (
    <div className="w-[420px] bg-[#faf8f5] dark:bg-[#1a1918] text-[#2c2a29] dark:text-[#f0ede8] border border-[#e6e0d5] dark:border-[#2e2c29] shadow-2xl rounded-2xl p-4 flex flex-col gap-3 animate-in fade-in zoom-in-95 duration-200 select-none font-sans text-xs">
      
      {/* 1. Auto-paste after dictation */}
      <div className="flex items-center justify-between py-1">
        <span className="font-medium text-[#3a3836] dark:text-[#e3dfd8]">
          Auto-paste after dictation
        </span>
        <button
          type="button"
          onClick={() => onToggleAutoPaste(!autoPaste)}
          className={cn(
            'relative inline-flex h-5 w-9 shrink-0 cursor-pointer rounded-full border-2 border-transparent transition-colors duration-200 ease-in-out focus:outline-none',
            autoPaste ? 'bg-[#292726] dark:bg-[#e0dad0]' : 'bg-[#d8d3c9] dark:bg-[#3d3b38]'
          )}
        >
          <span
            className={cn(
              'pointer-events-none inline-block h-4 w-4 transform rounded-full bg-white dark:bg-[#1a1918] shadow-md ring-0 transition duration-200 ease-in-out',
              autoPaste ? 'translate-x-4' : 'translate-x-0'
            )}
          />
        </button>
      </div>

      {/* 2. Text transform */}
      <div className="flex items-center justify-between py-1">
        <span className="font-medium text-[#3a3836] dark:text-[#e3dfd8]">
          Text transform
        </span>
        <button
          type="button"
          onClick={() => onToggleTextTransform(!textTransform)}
          className={cn(
            'relative inline-flex h-5 w-9 shrink-0 cursor-pointer rounded-full border-2 border-transparent transition-colors duration-200 ease-in-out focus:outline-none',
            textTransform ? 'bg-[#292726] dark:bg-[#e0dad0]' : 'bg-[#d8d3c9] dark:bg-[#3d3b38]'
          )}
        >
          <span
            className={cn(
              'pointer-events-none inline-block h-4 w-4 transform rounded-full bg-white dark:bg-[#1a1918] shadow-md ring-0 transition duration-200 ease-in-out',
              textTransform ? 'translate-x-4' : 'translate-x-0'
            )}
          />
        </button>
      </div>

      <div className="h-px bg-[#e6e0d5] dark:bg-[#2e2c29] my-0.5" />

      {/* 3. Cleanup style dropdown */}
      <div className="flex items-center justify-between py-1">
        <div className="flex items-center gap-1.5 text-[#3a3836] dark:text-[#e3dfd8]">
          <Edit3 className="w-3.5 h-3.5 text-[#8c867a]" />
          <span className="font-medium">Cleanup style</span>
        </div>
        <select
          value={cleanupStyle}
          onChange={(e) => onChangeCleanupStyle(e.target.value as CleanupStyle)}
          className="bg-[#f0ebe1] dark:bg-[#262423] border border-[#d8d2c4] dark:border-[#383533] rounded-md px-2 py-1 text-[11px] font-medium text-[#2c2a29] dark:text-[#f0ede8] focus:outline-none cursor-pointer"
        >
          <option value="faithful">Faithful</option>
          <option value="clean">Clean</option>
          <option value="professional">Professional</option>
          <option value="concise">Concise</option>
        </select>
      </div>

      {/* 4. Prompt mode toggle */}
      <div className="flex items-start justify-between py-1">
        <div className="flex flex-col">
          <span className="font-medium text-[#3a3836] dark:text-[#e3dfd8]">
            Prompt mode
          </span>
          <span className="text-[10px] text-[#8c867a] leading-tight">
            Rewrite speech into a prompt
          </span>
        </div>
        {isLlmReady ? (
          <button
            type="button"
            onClick={onTogglePromptMode}
            className={cn(
              'relative inline-flex h-5 w-9 shrink-0 cursor-pointer rounded-full border-2 border-transparent transition-colors duration-200 ease-in-out focus:outline-none mt-0.5',
              promptMode ? 'bg-[#c26238] dark:bg-[#d97345]' : 'bg-[#d8d3c9] dark:bg-[#3d3b38]'
            )}
          >
            <span
              className={cn(
                'pointer-events-none inline-block h-4 w-4 transform rounded-full bg-white dark:bg-[#1a1918] shadow-md ring-0 transition duration-200 ease-in-out',
                promptMode ? 'translate-x-4' : 'translate-x-0'
              )}
            />
          </button>
        ) : (
          <span className="text-[10px] text-amber-600 font-mono flex items-center gap-1">
            <AlertTriangle className="w-3 h-3" /> Requires LLM
          </span>
        )}
      </div>

      <div className="h-px bg-[#e6e0d5] dark:bg-[#2e2c29] my-0.5" />

      {/* 5. Language selector */}
      <div className="flex items-center justify-between py-1">
        <div className="flex items-center gap-1.5 text-[#3a3836] dark:text-[#e3dfd8]">
          <Globe className="w-3.5 h-3.5 text-[#8c867a]" />
          <span className="font-medium">Language</span>
        </div>
        <select
          value={language}
          onChange={(e) => onChangeLanguage(e.target.value as SpeechLanguage)}
          className="bg-[#f0ebe1] dark:bg-[#262423] border border-[#d8d2c4] dark:border-[#383533] rounded-md px-2 py-1 text-[11px] font-medium text-[#2c2a29] dark:text-[#f0ede8] focus:outline-none cursor-pointer"
        >
          <option value="english">English</option>
          <option value="hinglish">Hinglish</option>
          <option value="auto">Auto-detect</option>
        </select>
      </div>

      {/* Verified Status Footer */}
      <div className="border-t border-[#e6e0d5] dark:border-[#2e2c29] pt-2 flex flex-col gap-1 text-[10px]">
        <div className="flex items-center justify-between text-[#8c867a]">
          <span className="font-mono uppercase tracking-wider">Verification</span>
          <button
            type="button"
            onClick={onRefreshStatuses}
            className="hover:text-[#2c2a29] dark:hover:text-[#f0ede8] transition-colors"
            title="Refresh dependencies"
          >
            <RefreshCw className="w-3 h-3" />
          </button>
        </div>

        <div className="flex items-center justify-between text-[#595550] dark:text-[#a8a39a]">
          <span className="flex items-center gap-1"><Cpu className="w-3 h-3" /> Whisper STT</span>
          {whisperStatus.status === 'ready' ? (
            <span className="text-emerald-600 dark:text-emerald-400 font-medium flex items-center gap-1">
              <CheckCircle2 className="w-3 h-3" /> Ready
            </span>
          ) : (
            <button
              type="button"
              onClick={onDownloadWhisper}
              className="text-[#c26238] font-bold underline flex items-center gap-1 cursor-pointer"
            >
              <Download className="w-3 h-3" /> Setup Model
            </button>
          )}
        </div>

        <div className="flex items-center justify-between text-[#595550] dark:text-[#a8a39a]">
          <span className="flex items-center gap-1"><Sparkles className="w-3 h-3" /> LLM Status</span>
          {isLlmReady ? (
            <span className="text-emerald-600 dark:text-emerald-400 font-medium flex items-center gap-1">
              <CheckCircle2 className="w-3 h-3" /> Connected
            </span>
          ) : (
            <span className="text-amber-600 font-medium flex items-center gap-1">
              <AlertTriangle className="w-3 h-3" /> Not Configured
            </span>
          )}
        </div>

        <div className="flex items-center justify-between text-[#595550] dark:text-[#a8a39a]">
          <span className="flex items-center gap-1"><Keyboard className="w-3 h-3" /> Global Hotkey</span>
          <span className="font-mono bg-[#e6e0d5] dark:bg-[#2a2827] px-1.5 py-0.5 rounded text-[10px] text-[#2c2a29] dark:text-[#f0ede8]">
            {hotkeyStatus.hotkey}
          </span>
        </div>
      </div>
    </div>
  );
};
