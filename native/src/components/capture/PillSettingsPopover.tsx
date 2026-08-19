import React, { useState } from 'react';
import { AppSettings } from '../../types';
import { WhisperStatusInfo, OllamaStatusInfo, HotkeyStatusInfo, CleanupStyle, SpeechLanguage } from './PillTypes';
import { ChevronRight, ChevronLeft, Edit3, Globe, Sparkles } from 'lucide-react';
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

const LANG_LABELS: Record<string, string> = {
  auto: 'Auto-detect',
  english: 'English (US)',
  hinglish: 'Hinglish',
  hindi: 'Hindi',
  es: 'Español',
};

const STYLE_LABELS: Record<string, string> = {
  faithful: 'Faithful',
  polished: 'Polished',
  clean: 'Clean',
  concise: 'Concise',
  professional: 'Professional',
};

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
}) => {
  const [page, setPage] = useState<'main' | 'style' | 'lang'>('main');

  return (
    <div
      className="absolute left-1/2 bottom-[76px] -translate-x-1/2 w-[260px] bg-[#faf8f3] border border-black/10 rounded-[14px] p-1.5 text-[#1a1816] text-[13px] text-left shadow-[0_16px_40px_rgba(26,24,22,0.45),0_2px_8px_rgba(26,24,22,0.25),inset_0_1px_0_#faf8f3] animate-in fade-in slide-in-from-bottom-2 duration-180 select-none z-50 font-sans"
      onClick={(e) => e.stopPropagation()}
    >
      {page === 'main' && (
        <>
          {/* 1. Auto-paste after dictation */}
          <div className="flex items-center justify-between px-3 py-2">
            <span>Auto-paste after dictation</span>
            <button
              type="button"
              onClick={() => onToggleAutoPaste(!autoPaste)}
              className={cn(
                'relative w-8 h-[18px] rounded-full border-none cursor-pointer transition-colors duration-150 p-0',
                autoPaste ? 'bg-[#b8623d]' : 'bg-black/12'
              )}
              aria-label="Toggle Auto-paste"
            >
              <span
                className={cn(
                  'absolute top-[2px] w-3.5 h-3.5 rounded-full bg-[#faf8f3] shadow-[0_1px_2px_rgba(0,0,0,0.2)] transition-all duration-150',
                  autoPaste ? 'left-[16px]' : 'left-[2px]'
                )}
              />
            </button>
          </div>

          {/* 2. Text transform */}
          <div className="flex items-center justify-between px-3 py-2">
            <span>Text transform</span>
            <button
              type="button"
              onClick={() => onToggleTextTransform(!textTransform)}
              className={cn(
                'relative w-8 h-[18px] rounded-full border-none cursor-pointer transition-colors duration-150 p-0',
                textTransform ? 'bg-[#b8623d]' : 'bg-black/12'
              )}
              aria-label="Toggle Text transform"
            >
              <span
                className={cn(
                  'absolute top-[2px] w-3.5 h-3.5 rounded-full bg-[#faf8f3] shadow-[0_1px_2px_rgba(0,0,0,0.2)] transition-all duration-150',
                  textTransform ? 'left-[16px]' : 'left-[2px]'
                )}
              />
            </button>
          </div>

          <div className="h-px bg-black/10 my-0.5" />

          {/* 3. Cleanup style row (opens sub-page) */}
          <div
            onClick={() => setPage('style')}
            className="flex items-center gap-2.5 px-3 py-2.5 cursor-pointer rounded-lg hover:bg-[#b8623d]/10 transition-colors"
          >
            <Edit3 className="w-3.5 h-3.5 text-black/55 shrink-0" />
            <span className="flex-1 text-black/50 text-[12px]">Cleanup style</span>
            <span className="text-[#1a1816] text-xs font-medium inline-flex items-center gap-1">
              <span>{STYLE_LABELS[cleanupStyle] || cleanupStyle}</span>
              <ChevronRight className="w-3 h-3 text-black/55" />
            </span>
          </div>

          {/* 4. Prompt mode toggle */}
          <div className="flex items-center justify-between px-3 py-2">
            <div className="flex flex-col gap-0.5">
              <span>Prompt mode</span>
              <span className="text-[10px] font-normal text-black/50 tracking-normal">
                Rewrite speech into a prompt
              </span>
            </div>
            <button
              type="button"
              onClick={onTogglePromptMode}
              className={cn(
                'relative w-8 h-[18px] rounded-full border-none cursor-pointer transition-colors duration-150 p-0 shrink-0',
                promptMode ? 'bg-[#b8623d]' : 'bg-black/12'
              )}
              aria-label="Toggle Prompt Mode"
            >
              <span
                className={cn(
                  'absolute top-[2px] w-3.5 h-3.5 rounded-full bg-[#faf8f3] shadow-[0_1px_2px_rgba(0,0,0,0.2)] transition-all duration-150',
                  promptMode ? 'left-[16px]' : 'left-[2px]'
                )}
              />
            </button>
          </div>

          <div className="h-px bg-black/10 my-0.5" />

          {/* 5. Language row (opens sub-page) */}
          <div
            onClick={() => setPage('lang')}
            className="flex items-center gap-2.5 px-3 py-2.5 cursor-pointer rounded-lg hover:bg-[#b8623d]/10 transition-colors"
          >
            <Globe className="w-3.5 h-3.5 text-black/55 shrink-0" />
            <span className="flex-1 text-black/50 text-[12px]">Language</span>
            <span className="text-[#1a1816] text-xs font-medium inline-flex items-center gap-1">
              <span>{LANG_LABELS[language] || language}</span>
              <ChevronRight className="w-3 h-3 text-black/55" />
            </span>
          </div>
        </>
      )}

      {/* Language Sub-Page */}
      {page === 'lang' && (
        <div className="bg-[#faf8f3] rounded-lg p-0.5 flex flex-col gap-0.5">
          <button
            type="button"
            onClick={() => setPage('main')}
            className="flex items-center gap-2 px-2.5 py-2 cursor-pointer text-black/55 text-xs font-sans rounded-md hover:bg-[#b8623d]/10 transition-colors w-full text-left"
          >
            <ChevronLeft className="w-3.5 h-3.5 stroke-[2]" />
            <span>Back</span>
          </button>
          {[
            { id: 'auto', name: 'Auto-detect' },
            { id: 'english', name: 'English (US)' },
            { id: 'hinglish', name: 'Hinglish' },
            { id: 'hindi', name: 'Hindi' },
            { id: 'es', name: 'Español' },
          ].map((item) => (
            <div
              key={item.id}
              onClick={() => {
                onChangeLanguage(item.id as SpeechLanguage);
                setPage('main');
              }}
              className={cn(
                'flex items-center justify-between px-3 py-2 cursor-pointer rounded-lg text-xs transition-colors',
                language === item.id
                  ? 'bg-[#b8623d]/15 text-[#b8623d] font-medium'
                  : 'hover:bg-[#b8623d]/10 text-[#1a1816]'
              )}
            >
              <span>{item.name}</span>
            </div>
          ))}
        </div>
      )}

      {/* Cleanup Style Sub-Page */}
      {page === 'style' && (
        <div className="bg-[#faf8f3] rounded-lg p-0.5 flex flex-col gap-0.5">
          <button
            type="button"
            onClick={() => setPage('main')}
            className="flex items-center gap-2 px-2.5 py-2 cursor-pointer text-black/55 text-xs font-sans rounded-md hover:bg-[#b8623d]/10 transition-colors w-full text-left"
          >
            <ChevronLeft className="w-3.5 h-3.5 stroke-[2]" />
            <span>Back</span>
          </button>
          {[
            { id: 'faithful', name: 'Faithful' },
            { id: 'polished', name: 'Polished' },
            { id: 'clean', name: 'Clean' },
            { id: 'concise', name: 'Concise' },
          ].map((item) => (
            <div
              key={item.id}
              onClick={() => {
                onChangeCleanupStyle(item.id as CleanupStyle);
                setPage('main');
              }}
              className={cn(
                'flex items-center justify-between px-3 py-2 cursor-pointer rounded-lg text-xs transition-colors',
                cleanupStyle === item.id
                  ? 'bg-[#b8623d]/15 text-[#b8623d] font-medium'
                  : 'hover:bg-[#b8623d]/10 text-[#1a1816]'
              )}
            >
              <span>{item.name}</span>
            </div>
          ))}
        </div>
      )}
    </div>
  );
};
