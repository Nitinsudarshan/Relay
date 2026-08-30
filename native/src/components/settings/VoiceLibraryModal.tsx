import React, { useState, useMemo, useEffect } from 'react';
import { Input } from '@/components/ui/input';
import { Button } from '@/components/ui/button';
import { Badge } from '@/components/ui/badge';
import {
  Search,
  Check,
  Download,
  Volume2,
  Square,
  Sparkles,
  Loader2,
  Globe,
  ShieldCheck,
  X,
} from 'lucide-react';
import type { CatalogueVoice } from '../../types';

interface VoiceLibraryModalProps {
  open: boolean;
  onClose: () => void;
  catalogue: CatalogueVoice[];
  activeVoiceId: string | null;
  onSelectVoice: (voiceId: string) => Promise<void>;
  onTestVoice: (voiceId?: string) => Promise<void>;
  onStopTest: () => void;
  isPlayingTest: boolean;
  busyVoiceId: string | null;
}

const REGION_FLAGS: Record<string, string> = {
  en_US: '🇺🇸',
  en_GB: '🇬🇧',
  hi_IN: '🇮🇳',
  es_ES: '🇪🇸',
  fr_FR: '🇫🇷',
  de_DE: '🇩🇪',
};

const formatSize = (bytes: number): string => {
  if (!bytes) return '';
  if (bytes < 1024 * 1024) return `${Math.round(bytes / 1024)} KB`;
  return `${(bytes / (1024 * 1024)).toFixed(0)} MB`;
};

/** Extracts region code from voice ID, e.g. en_US from en_US-amy-medium */
const getRegionCode = (id: string): string => {
  const match = id.match(/^([a-z]{2}_[A-Z]{2})/);
  return match ? match[1] : 'en_US';
};

/** Translates technical quality tier into user-friendly description */
const getQualityLabel = (id: string): { label: string; badge: string } => {
  if (id.includes('high')) {
    return {
      label: 'High Quality',
      badge: 'border-purple-500/30 text-purple-600 dark:text-purple-400 bg-purple-500/10',
    };
  }
  if (id.includes('low') || id.includes('x_low')) {
    return {
      label: 'Fast',
      badge: 'border-blue-500/30 text-blue-600 dark:text-blue-400 bg-blue-500/10',
    };
  }
  return {
    label: 'Balanced',
    badge: 'border-emerald-500/30 text-emerald-600 dark:text-emerald-400 bg-emerald-500/10',
  };
};

/** Infers gender/character attribute for filtering */
const getVoiceGender = (voice: CatalogueVoice): 'Female' | 'Male' | 'Neutral' => {
  const desc = (voice.description + ' ' + voice.displayName).toLowerCase();
  if (
    desc.includes('female') ||
    desc.includes('amy') ||
    desc.includes('alba') ||
    desc.includes('lessac') ||
    desc.includes('cori') ||
    desc.includes('siwis')
  ) {
    return 'Female';
  }
  if (
    desc.includes('male') ||
    desc.includes('ryan') ||
    desc.includes('alan') ||
    desc.includes('pratham') ||
    desc.includes('sharvard') ||
    desc.includes('thorsten')
  ) {
    return 'Male';
  }
  return 'Neutral';
};

export const VoiceLibraryModal: React.FC<VoiceLibraryModalProps> = ({
  open,
  onClose,
  catalogue,
  activeVoiceId,
  onSelectVoice,
  onTestVoice,
  onStopTest,
  isPlayingTest,
  busyVoiceId,
}) => {
  const [search, setSearch] = useState('');
  const [selectedLanguage, setSelectedLanguage] = useState<string>('all');
  const [selectedGender, setSelectedGender] = useState<string>('all');
  const [selectedStatus, setSelectedStatus] = useState<string>('all');

  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.key === 'Escape' && open) {
        onClose();
      }
    };
    window.addEventListener('keydown', handleKeyDown);
    return () => window.removeEventListener('keydown', handleKeyDown);
  }, [open, onClose]);

  // Extract all unique languages from catalogue
  const availableLanguages = useMemo(() => {
    const langs = new Set<string>();
    catalogue.forEach((v) => {
      if (v.languageLabel) langs.add(v.languageLabel);
    });
    return Array.from(langs);
  }, [catalogue]);

  const filteredVoices = useMemo(() => {
    return catalogue.filter((voice) => {
      const matchesSearch =
        search === '' ||
        voice.displayName.toLowerCase().includes(search.toLowerCase()) ||
        voice.languageLabel.toLowerCase().includes(search.toLowerCase()) ||
        voice.description.toLowerCase().includes(search.toLowerCase());

      const matchesLang =
        selectedLanguage === 'all' || voice.languageLabel === selectedLanguage;

      const gender = getVoiceGender(voice);
      const matchesGender =
        selectedGender === 'all' || gender.toLowerCase() === selectedGender.toLowerCase();

      const matchesStatus =
        selectedStatus === 'all' ||
        (selectedStatus === 'installed' && voice.installed) ||
        (selectedStatus === 'downloadable' && !voice.installed);

      return matchesSearch && matchesLang && matchesGender && matchesStatus;
    });
  }, [catalogue, search, selectedLanguage, selectedGender, selectedStatus]);

  if (!open) return null;

  return (
    <div
      className="fixed inset-0 z-50 flex items-center justify-center bg-black/60 backdrop-blur-xs p-4 animate-in fade-in duration-200"
      data-testid="voice-library-modal"
    >
      <div className="bg-card border border-border shadow-2xl rounded-2xl w-[90vw] max-w-3xl max-h-[85vh] flex flex-col overflow-hidden text-foreground">
        {/* Header */}
        <div className="p-5 pb-4 border-b border-border bg-muted/20">
          <div className="flex items-center justify-between">
            <div className="flex items-center gap-2.5">
              <div className="w-8 h-8 rounded-lg bg-primary/10 text-primary flex items-center justify-center border border-primary/20">
                <Volume2 className="w-4 h-4" />
              </div>
              <div>
                <h2 className="text-base font-semibold text-foreground">Voice Catalogue</h2>
                <p className="text-xs text-muted-foreground mt-0.5">
                  Curated neural voices running 100% locally on your computer.
                </p>
              </div>
            </div>

            <Button
              variant="ghost"
              size="icon"
              className="h-8 w-8 text-muted-foreground hover:text-foreground"
              onClick={onClose}
              aria-label="Close"
            >
              <X className="w-4 h-4" />
            </Button>
          </div>

          {/* Search & Filter Bar */}
          <div className="mt-4 flex flex-col sm:flex-row items-stretch sm:items-center gap-2.5">
            <div className="relative flex-1">
              <Search className="w-4 h-4 absolute left-3 top-1/2 -translate-y-1/2 text-muted-foreground" />
              <Input
                placeholder="Search by voice name, language or accent…"
                value={search}
                onChange={(e) => setSearch(e.target.value)}
                className="pl-9 h-9 text-xs bg-background"
              />
            </div>

            {/* Language filter pills */}
            <div className="flex items-center gap-1 overflow-x-auto pb-1 sm:pb-0 scrollbar-none">
              <Button
                variant={selectedLanguage === 'all' ? 'secondary' : 'ghost'}
                size="sm"
                onClick={() => setSelectedLanguage('all')}
                className="h-8 px-2.5 text-[11px] shrink-0"
              >
                All Languages
              </Button>
              {availableLanguages.map((lang) => (
                <Button
                  key={lang}
                  variant={selectedLanguage === lang ? 'secondary' : 'ghost'}
                  size="sm"
                  onClick={() => setSelectedLanguage(lang)}
                  className="h-8 px-2.5 text-[11px] shrink-0"
                >
                  {lang}
                </Button>
              ))}
            </div>
          </div>
        </div>

        {/* Voice List Body */}
        <div className="flex-1 overflow-y-auto p-5 space-y-3 min-h-0 bg-background/50">
          {filteredVoices.length === 0 ? (
            <div className="py-12 flex flex-col items-center justify-center text-center text-muted-foreground">
              <Volume2 className="w-8 h-8 opacity-30 mb-2" />
              <p className="text-sm font-medium">No voices match your filters</p>
              <p className="text-xs text-muted-foreground/80 mt-1">
                Try clearing your search or switching language filters.
              </p>
            </div>
          ) : (
            <div className="grid grid-cols-1 md:grid-cols-2 gap-3">
              {filteredVoices.map((voice) => {
                const isActive = activeVoiceId === voice.id;
                const region = getRegionCode(voice.id);
                const flag = REGION_FLAGS[region] || '🌐';
                const quality = getQualityLabel(voice.id);
                const gender = getVoiceGender(voice);
                const isBusy = busyVoiceId === voice.id;

                return (
                  <div
                    key={voice.id}
                    className={`relative rounded-xl border p-4 transition-all flex flex-col justify-between gap-3 ${
                      isActive
                        ? 'border-primary/50 bg-primary/5 shadow-xs ring-1 ring-primary/30'
                        : 'border-border bg-card hover:border-border/80 hover:shadow-xs'
                    }`}
                  >
                    {/* Top row: Name, Flag, Recommended Badge */}
                    <div className="space-y-1.5">
                      <div className="flex items-start justify-between gap-2">
                        <div className="flex items-center gap-2">
                          <span className="text-base select-none" title={voice.languageLabel}>
                            {flag}
                          </span>
                          <div>
                            <p className="text-xs font-semibold text-foreground flex items-center gap-1.5">
                              <span>{voice.displayName}</span>
                              {voice.recommended && (
                                <Badge
                                  variant="secondary"
                                  className="text-[9px] font-medium h-4 px-1.5 bg-primary/10 text-primary border-primary/20"
                                >
                                  Recommended
                                </Badge>
                              )}
                            </p>
                            <p className="text-[11px] text-muted-foreground">
                              {voice.languageLabel} · {gender}
                            </p>
                          </div>
                        </div>

                        <Badge
                          variant="outline"
                          className={`text-[9px] font-medium h-4 px-1.5 ${quality.badge}`}
                        >
                          {quality.label}
                        </Badge>
                      </div>

                      <p className="text-xs text-muted-foreground line-clamp-2 leading-relaxed">
                        {voice.description}
                      </p>
                    </div>

                    {/* Bottom row: Status & Actions */}
                    <div className="flex items-center justify-between pt-2 border-t border-border/60 mt-auto">
                      <div className="flex items-center gap-1.5 text-[11px] text-muted-foreground">
                        {isActive ? (
                          <span className="flex items-center gap-1 text-primary font-medium">
                            <Check className="w-3.5 h-3.5" />
                            Active voice
                          </span>
                        ) : voice.installed ? (
                          <span className="flex items-center gap-1 text-muted-foreground">
                            <ShieldCheck className="w-3.5 h-3.5 text-emerald-500" />
                            Installed
                          </span>
                        ) : (
                          <span className="text-[10px] font-mono text-muted-foreground/80">
                            {formatSize(voice.downloadBytes || 63_000_000)} download
                          </span>
                        )}
                      </div>

                      <div className="flex items-center gap-1.5">
                        {isActive && (
                          <Button
                            variant="outline"
                            size="sm"
                            onClick={() => (isPlayingTest ? onStopTest() : onTestVoice(voice.id))}
                            className="h-7 px-2.5 text-[11px] gap-1"
                          >
                            {isPlayingTest ? (
                              <>
                                <Square className="w-3 h-3 text-primary animate-pulse" />
                                Stop
                              </>
                            ) : (
                              <>
                                <Volume2 className="w-3 h-3 text-muted-foreground" />
                                Test
                              </>
                            )}
                          </Button>
                        )}

                        {!isActive && (
                          <Button
                            variant={voice.installed ? 'secondary' : 'default'}
                            size="sm"
                            disabled={isBusy}
                            onClick={() => void onSelectVoice(voice.id)}
                            className="h-7 px-2.5 text-[11px] gap-1"
                          >
                            {isBusy ? (
                              <>
                                <Loader2 className="w-3 h-3 animate-spin" />
                                Setting up…
                              </>
                            ) : voice.installed ? (
                              <>
                                <Check className="w-3 h-3" />
                                Set Active
                              </>
                            ) : (
                              <>
                                <Download className="w-3 h-3" />
                                Download &amp; Use
                              </>
                            )}
                          </Button>
                        )}
                      </div>
                    </div>
                  </div>
                );
              })}
            </div>
          )}
        </div>

        {/* Footer info note */}
        <div className="p-3.5 px-5 border-t border-border bg-muted/20 flex items-center justify-between text-[11px] text-muted-foreground">
          <div className="flex items-center gap-1.5">
            <ShieldCheck className="w-3.5 h-3.5 text-emerald-500 shrink-0" />
            <span>Pinned SHA-256 integrity verified on-demand for every model download.</span>
          </div>
          <Button variant="ghost" size="sm" onClick={onClose} className="h-7 text-xs">
            Done
          </Button>
        </div>
      </div>
    </div>
  );
};
