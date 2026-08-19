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
      className="absolute left-1/2 bottom-[76px] -translate-x-1/2 w-[270px] bg-white text-slate-900 border border-slate-200 shadow-2xl rounded-2xl p-2.5 text-xs text-left select-none z-50 font-sans animate-in fade-in slide-in-from-bottom-2 duration-150"
      onClick={(e) => e.stopPropagation()}
    >
      {page === 'main' && (
        <>
          {/* 1. Auto-paste after dictation */}
          <div className="flex items-center justify-between px-3 py-2">
            <span className="font-medium text-slate-800">Auto-paste after dictation</span>
            <button
              type="button"
              onClick={() => onToggleAutoPaste(!autoPaste)}
              className={cn(
                'relative w-8 h-[18px] rounded-full border-none cursor-pointer transition-colors duration-150 p-0',
                autoPaste ? 'bg-blue-600' : 'bg-slate-300'
              )}
              aria-label="Toggle Auto-paste"
            >
              <span
                className={cn(
                  'absolute top-[2px] w-3.5 h-3.5 rounded-full bg-white shadow-sm transition-all duration-150',
                  autoPaste ? 'left-[16px]' : 'left-[2px]'
                )}
              />
            </button>
          </div>

          {/* 2. Text transform */}
          <div className="flex items-center justify-between px-3 py-2">
            <span className="font-medium text-slate-800">Text transform</span>
            <button
              type="button"
              onClick={() => onToggleTextTransform(!textTransform)}
              className={cn(
                'relative w-8 h-[18px] rounded-full border-none cursor-pointer transition-colors duration-150 p-0',
                textTransform ? 'bg-blue-600' : 'bg-slate-300'
              )}
              aria-label="Toggle Text transform"
            >
              <span
                className={cn(
                  'absolute top-[2px] w-3.5 h-3.5 rounded-full bg-white shadow-sm transition-all duration-150',
                  textTransform ? 'left-[16px]' : 'left-[2px]'
                )}
              />
            </button>
          </div>

          <div className="h-px bg-slate-100 my-1" />

          {/* 3. Cleanup style row (opens sub-page) */}
          <div
            onClick={() => setPage('style')}
            className="flex items-center gap-2.5 px-3 py-2 cursor-pointer rounded-lg hover:bg-slate-100 transition-colors"
          >
            <Edit3 className="w-3.5 h-3.5 text-slate-500 shrink-0" />
            <span className="flex-1 text-slate-600 text-xs">Cleanup style</span>
            <span className="text-slate-900 text-xs font-semibold inline-flex items-center gap-1">
              <span>{STYLE_LABELS[cleanupStyle] || cleanupStyle}</span>
              <ChevronRight className="w-3.5 h-3.5 text-slate-400" />
            </span>
          </div>

          {/* 4. Prompt mode toggle */}
          <div className="flex items-center justify-between px-3 py-2">
            <div className="flex flex-col gap-0.5">
              <span className="font-medium text-slate-800">Prompt mode</span>
              <span className="text-[10px] font-normal text-slate-500">
                Rewrite speech into a prompt
              </span>
            </div>
            <button
              type="button"
              onClick={onTogglePromptMode}
              className={cn(
                'relative w-8 h-[18px] rounded-full border-none cursor-pointer transition-colors duration-150 p-0 shrink-0',
                promptMode ? 'bg-blue-600' : 'bg-slate-300'
              )}
              aria-label="Toggle Prompt Mode"
            >
              <span
                className={cn(
                  'absolute top-[2px] w-3.5 h-3.5 rounded-full bg-white shadow-sm transition-all duration-150',
                  promptMode ? 'left-[16px]' : 'left-[2px]'
                )}
              />
            </button>
          </div>

          <div className="h-px bg-slate-100 my-1" />

          {/* 5. Language row (opens sub-page) */}
          <div
            onClick={() => setPage('lang')}
            className="flex items-center gap-2.5 px-3 py-2 cursor-pointer rounded-lg hover:bg-slate-100 transition-colors"
          >
            <Globe className="w-3.5 h-3.5 text-slate-500 shrink-0" />
            <span className="flex-1 text-slate-600 text-xs">Language</span>
            <span className="text-slate-900 text-xs font-semibold inline-flex items-center gap-1">
              <span>{LANG_LABELS[language] || language}</span>
              <ChevronRight className="w-3.5 h-3.5 text-slate-400" />
            </span>
          </div>
        </>
      )}

      {/* Language Sub-Page */}
      {page === 'lang' && (
        <div className="bg-white rounded-lg p-0.5 flex flex-col gap-0.5">
          <button
            type="button"
            onClick={() => setPage('main')}
            className="flex items-center gap-2 px-2.5 py-2 cursor-pointer text-slate-600 text-xs font-sans rounded-md hover:bg-slate-100 transition-colors w-full text-left"
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
                  ? 'bg-blue-50 text-blue-600 font-semibold'
                  : 'hover:bg-slate-100 text-slate-800'
              )}
            >
              <span>{item.name}</span>
            </div>
          ))}
        </div>
      )}

      {/* Cleanup Style Sub-Page */}
      {page === 'style' && (
        <div className="bg-white rounded-lg p-0.5 flex flex-col gap-0.5">
          <button
            type="button"
            onClick={() => setPage('main')}
            className="flex items-center gap-2 px-2.5 py-2 cursor-pointer text-slate-600 text-xs font-sans rounded-md hover:bg-slate-100 transition-colors w-full text-left"
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
                  ? 'bg-blue-50 text-blue-600 font-semibold'
                  : 'hover:bg-slate-100 text-slate-800'
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
