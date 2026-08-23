import React, { useState, useEffect } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { getCurrentWindow } from '@tauri-apps/api/window';
import {
  Mic,
  Bell,
  ChevronRight,
  X,
  Zap,
  Sliders,
  LayoutGrid,
  Radio,
  Play,
  Layers,
  Info,
  Check,
  Square,
  Users,
  Sparkles,
  ShieldCheck,
  Flame,
  Wand2,
  Monitor,
} from 'lucide-react';
import { Button } from '@/components/ui/button';
import { Badge } from '@/components/ui/badge';

export interface MeetingNotificationOption {
  id: number;
  name: string;
  category: 'Stealth' | 'Minimal' | 'Command Center' | 'AI Assist';
  description: string;
  ctaText: string;
  secondaryCtaText?: string;
  accentColor: string;
}

export const DESIGN_OPTIONS: MeetingNotificationOption[] = [
  {
    id: 1,
    name: 'Compact HUD Bar (rounded-lg)',
    category: 'Stealth',
    description: '400x84px HUD popup with rounded-lg corners, Record primary CTA, Skip secondary CTA, X dismiss, and 5s OS timer.',
    ctaText: 'Record Now',
    secondaryCtaText: 'Skip',
    accentColor: 'from-emerald-500/20 to-blue-500/10',
  },
  {
    id: 2,
    name: 'Compact Quick Dock Widget (rounded-lg)',
    category: 'Minimal',
    description: '400x84px dock card with Webex badge, Record CTA, Skip CTA, X dismiss, and 5s OS auto-dismiss timer.',
    ctaText: 'Record Meeting',
    secondaryCtaText: 'Skip',
    accentColor: 'from-rose-500/20 to-amber-500/10',
  },
  {
    id: 3,
    name: 'Animated Gradient Border Card (rounded-lg)',
    category: 'Command Center',
    description: '400x84px shimmering gradient border card with participant avatars, Start Recording CTA, Skip CTA, and 5s timer.',
    ctaText: 'Start Recording Now',
    secondaryCtaText: 'Skip',
    accentColor: 'from-primary/30 via-violet-500/30 to-emerald-500/30',
  },
  {
    id: 4,
    name: 'Stealth Mini Floating Bar (rounded-lg)',
    category: 'Stealth',
    description: '400x84px compact stealth bar with live detection status, REC CTA, Skip CTA, X close, and 5s OS timer.',
    ctaText: 'REC',
    secondaryCtaText: 'Skip',
    accentColor: 'from-emerald-500/20 to-teal-500/10',
  },
  {
    id: 5,
    name: 'Left-Accent Banner (rounded-lg)',
    category: 'Minimal',
    description: '400x84px red accent card with pulsing indicator, Start Recording CTA, Skip CTA, X close, and 5s OS auto-dismiss.',
    ctaText: 'Start Recording',
    secondaryCtaText: 'Skip',
    accentColor: 'from-red-500/20 via-primary/10 to-transparent',
  },
  {
    id: 6,
    name: 'Waveform Control Bar (rounded-lg)',
    category: 'Stealth',
    description: '400x84px dark tech widget with real-time frequency visualizer bars, Initiate Capture CTA, Skip CTA, and 5s timer.',
    ctaText: 'Initiate Capture',
    secondaryCtaText: 'Skip',
    accentColor: 'from-cyan-500/20 via-purple-500/20 to-pink-500/20',
  },
  {
    id: 7,
    name: 'AI Copilot Quick Toast (rounded-lg)',
    category: 'AI Assist',
    description: '400x84px AI assist toast with preset selector chip, Record & Transcribe CTA, Skip CTA, X close, and 5s timer.',
    ctaText: 'Record & Transcribe',
    secondaryCtaText: 'Skip',
    accentColor: 'from-amber-500/20 to-purple-600/10',
  },
  {
    id: 8,
    name: 'Corner Action Tray (rounded-lg)',
    category: 'Minimal',
    description: '400x84px Teams action tray with attendee count badge, Capture Audio CTA, Skip CTA, X close, and 5s OS timer.',
    ctaText: 'Capture Audio',
    secondaryCtaText: 'Skip',
    accentColor: 'from-blue-600/20 to-indigo-500/10',
  },
  {
    id: 9,
    name: 'Edge-Anchored Mini HUD (rounded-lg)',
    category: 'Stealth',
    description: '400x84px high-contrast edge HUD card with mic status dot, Start STT CTA, Skip CTA, X close, and 5s timer.',
    ctaText: 'Start STT',
    secondaryCtaText: 'Skip',
    accentColor: 'from-emerald-500/20 to-teal-500/10',
  },
  {
    id: 10,
    name: 'Micro Pre-Flight Command Card (rounded-lg)',
    category: 'Command Center',
    description: '400x84px pre-flight check card with mic signal strength check meter, Launch Recording CTA, Skip CTA, and 5s timer.',
    ctaText: 'Launch Recording',
    secondaryCtaText: 'Skip',
    accentColor: 'from-indigo-500/20 via-purple-500/20 to-pink-500/20',
  },
];

export const MeetingNotificationsDesignGallery: React.FC = () => {
  const [viewMode, setViewMode] = useState<'grid' | 'focused'>('grid');
  const [selectedOptionId, setSelectedOptionId] = useState<number>(1);
  const [actionFeedback, setActionFeedback] = useState<string | null>(null);
  const [selectedPreset, setSelectedPreset] = useState<string>('Executive Summary');
  const [inputMode, setInputMode] = useState<'mic' | 'system' | 'both'>('both');
  const [activeSystemTheme, setActiveSystemTheme] = useState<number>(() => {
    const saved = localStorage.getItem('relay_meeting_reminder_theme');
    return saved ? parseInt(saved, 10) : 1;
  });
  const [copiedId, setCopiedId] = useState<number | null>(null);

  useEffect(() => {
    const saved = localStorage.getItem('relay_meeting_reminder_theme');
    if (saved) {
      setActiveSystemTheme(parseInt(saved, 10));
    }
  }, []);

  const activeOption = DESIGN_OPTIONS.find((o) => o.id === selectedOptionId) || DESIGN_OPTIONS[0];

  const triggerActionFeedback = (msg: string) => {
    setActionFeedback(msg);
    setTimeout(() => {
      setActionFeedback(null);
    }, 4000);
  };

  const handleSimulateSystemToast = async (id: number) => {
    localStorage.setItem('relay_meeting_reminder_theme', String(id));
    window.dispatchEvent(new Event('relay-reminder-theme-changed'));
    setActiveSystemTheme(id);

    try {
      await invoke('trigger_mock_meeting_reminder', { kind: 'detected' });
      triggerActionFeedback(`Fired Design Option #${id} (400x84px) to OS System Desktop Window!`);
    } catch (err) {
      console.warn('Could not launch system popup via Tauri:', err);
      triggerActionFeedback(`Set Design Option #${id} as active theme`);
    }
  };

  const handleApplyTheme = (id: number) => {
    localStorage.setItem('relay_meeting_reminder_theme', String(id));
    window.dispatchEvent(new Event('relay-reminder-theme-changed'));
    setActiveSystemTheme(id);
    triggerActionFeedback(`Design Option #${id} set as active system reminder theme`);
  };

  const handleCopyDesignName = (id: number, name: string) => {
    navigator.clipboard.writeText(name).catch(() => {});
    setCopiedId(id);
    setTimeout(() => setCopiedId(null), 2000);
  };

  return (
    <div className="flex-1 flex flex-col space-y-6 select-none max-w-7xl mx-auto w-full pb-12">
      {/* Header Banner */}
      <div className="relative overflow-hidden rounded-lg border border-border bg-card p-6 shadow-sm">
        <div className="absolute top-0 right-0 -mt-12 -mr-12 w-64 h-64 bg-primary/10 rounded-full blur-3xl pointer-events-none" />
        <div className="flex flex-col md:flex-row md:items-center justify-between gap-4 relative z-10">
          <div>
            <div className="flex items-center gap-2 mb-2">
              <Badge variant="outline" className="text-xs font-mono border-purple-500/40 text-purple-400 bg-purple-500/10">
                Components &gt; Meeting &gt; Notifications
              </Badge>
              <Badge variant="secondary" className="text-xs font-mono">
                Standard 400x84px (rounded-lg)
              </Badge>
            </div>
            <h1 className="text-2xl font-extrabold tracking-tight text-foreground flex items-center gap-2.5">
              <Bell className="w-6 h-6 text-purple-500" />
              Meeting Detection System Popups
            </h1>
            <p className="text-xs text-muted-foreground mt-1 max-w-2xl leading-relaxed">
              Every design option is strictly sized to <code className="text-purple-400 bg-purple-500/10 px-1 py-0.5 rounded-md font-mono">400x84px</code> matching the Tauri window size exactly. Features standard <strong className="text-foreground">Record</strong>, <strong className="text-foreground">Skip</strong>, top-right <strong className="text-foreground">X</strong> close, and a <strong className="text-purple-400">5-second OS auto-dismiss countdown timer</strong>.
            </p>
          </div>

          <div className="flex items-center gap-2 shrink-0">
            <div className="bg-muted/50 p-1 rounded-lg border border-border flex items-center">
              <Button
                variant={viewMode === 'grid' ? 'default' : 'ghost'}
                size="sm"
                className="h-8 text-xs gap-1.5 rounded-md"
                onClick={() => setViewMode('grid')}
              >
                <LayoutGrid className="w-3.5 h-3.5" />
                Grid Gallery
              </Button>
              <Button
                variant={viewMode === 'focused' ? 'default' : 'ghost'}
                size="sm"
                className="h-8 text-xs gap-1.5 rounded-md"
                onClick={() => setViewMode('focused')}
              >
                <Layers className="w-3.5 h-3.5" />
                Focused Inspector
              </Button>
            </div>
          </div>
        </div>

        {/* Live Feedback Toast Notification */}
        {actionFeedback && (
          <div className="mt-4 p-3 rounded-lg bg-purple-500/15 border border-purple-500/30 text-purple-300 text-xs font-medium flex items-center justify-between animate-in fade-in slide-in-from-top-2">
            <div className="flex items-center gap-2">
              <Monitor className="w-4 h-4 text-purple-400 animate-pulse" />
              <span>{actionFeedback}</span>
            </div>
            <button type="button" onClick={() => setActionFeedback(null)} className="text-purple-400 hover:text-purple-200">
              <X className="w-3.5 h-3.5" />
            </button>
          </div>
        )}
      </div>

      {/* View Mode: Grid Gallery */}
      {viewMode === 'grid' && (
        <div className="grid grid-cols-1 md:grid-cols-2 gap-6">
          {DESIGN_OPTIONS.map((option) => {
            const isThemeActive = activeSystemTheme === option.id;
            return (
              <div
                key={option.id}
                className={`group relative rounded-lg border transition-all duration-300 flex flex-col bg-card overflow-hidden ${
                  isThemeActive
                    ? 'border-purple-500 shadow-md shadow-purple-500/10 ring-1 ring-purple-500/50'
                    : 'border-border hover:border-border/80 hover:shadow-sm'
                }`}
              >
                {/* Option Header */}
                <div className="px-5 py-3 border-b border-border bg-muted/20 flex items-center justify-between">
                  <div className="flex items-center gap-2.5">
                    <span className="flex size-6 items-center justify-center rounded-lg bg-purple-500/10 text-purple-400 font-mono font-bold text-xs border border-purple-500/30">
                      {option.id}
                    </span>
                    <div>
                      <h3 className="text-xs font-bold text-foreground flex items-center gap-2">
                        {option.name}
                        {isThemeActive && (
                          <Badge className="text-[9px] font-mono px-1.5 py-0 bg-purple-500 text-white border-none rounded-md">
                            ACTIVE SYSTEM THEME
                          </Badge>
                        )}
                      </h3>
                      <span className="text-[10px] text-muted-foreground font-mono">{option.category}</span>
                    </div>
                  </div>

                  <div className="flex items-center gap-1">
                    <Button
                      variant="ghost"
                      size="icon"
                      className="h-7 w-7 text-muted-foreground hover:text-foreground rounded-md"
                      title="Copy Design Name"
                      onClick={() => handleCopyDesignName(option.id, option.name)}
                    >
                      {copiedId === option.id ? <Check className="w-3.5 h-3.5 text-emerald-500" /> : <Info className="w-3.5 h-3.5" />}
                    </Button>
                    <Button
                      variant="outline"
                      size="sm"
                      className="h-7 text-[10px] px-2 font-mono gap-1 rounded-md"
                      onClick={() => {
                        setSelectedOptionId(option.id);
                        setViewMode('focused');
                      }}
                    >
                      Inspect <ChevronRight className="w-3 h-3" />
                    </Button>
                  </div>
                </div>

                {/* Live Card Preview Surface */}
                <div className="p-6 flex-1 flex flex-col justify-center items-center bg-muted/10 relative overflow-hidden min-h-[160px]">
                  <div className="w-[400px] h-[84px]">
                    {renderNotificationCard(option, {
                      preset: selectedPreset,
                      inputMode,
                      onStart: () => triggerActionFeedback(`[Option ${option.id}] Started recording for Zoom Product Sync!`),
                      onSnooze: () => triggerActionFeedback(`[Option ${option.id}] Snoozed reminder for 5 minutes.`),
                      onDismiss: () => triggerActionFeedback(`[Option ${option.id}] Dismissed meeting reminder.`),
                    })}
                  </div>
                </div>

                {/* Option Footer Controls */}
                <div className="px-5 py-3 border-t border-border bg-card flex items-center justify-between text-xs">
                  <p className="text-[11px] text-muted-foreground line-clamp-1 max-w-[50%]">{option.description}</p>
                  <div className="flex items-center gap-2">
                    <Button
                      variant="secondary"
                      size="sm"
                      className="h-7 text-xs px-2.5 font-medium rounded-md gap-1 bg-purple-500/10 text-purple-300 hover:bg-purple-500/20 border border-purple-500/30"
                      onClick={() => handleSimulateSystemToast(option.id)}
                    >
                      <Monitor className="w-3 h-3 text-purple-400" />
                      Simulate OS Popup
                    </Button>
                    <Button
                      variant={isThemeActive ? 'default' : 'outline'}
                      size="sm"
                      className={`h-7 text-xs px-2.5 font-medium rounded-md ${isThemeActive ? 'bg-purple-600 hover:bg-purple-700 text-white' : ''}`}
                      onClick={() => handleApplyTheme(option.id)}
                    >
                      {isThemeActive ? 'Active' : 'Set Active'}
                    </Button>
                  </div>
                </div>
              </div>
            );
          })}
        </div>
      )}

      {/* View Mode: Focused Inspector */}
      {viewMode === 'focused' && (
        <div className="grid grid-cols-1 lg:grid-cols-3 gap-6">
          {/* Option Selector List */}
          <div className="lg:col-span-1 rounded-lg border border-border bg-card p-4 space-y-2 max-h-[600px] overflow-y-auto">
            <div className="px-2 py-1 text-xs font-bold uppercase tracking-wider text-muted-foreground mb-1">
              Select Design Option (400x84px)
            </div>
            {DESIGN_OPTIONS.map((opt) => {
              const isSelected = opt.id === selectedOptionId;
              const isActiveTheme = activeSystemTheme === opt.id;
              return (
                <button
                  key={opt.id}
                  type="button"
                  onClick={() => setSelectedOptionId(opt.id)}
                  className={`w-full text-left p-3 rounded-lg border transition-all cursor-pointer flex items-center justify-between ${
                    isSelected
                      ? 'border-purple-500 bg-purple-500/10 text-foreground font-semibold shadow-xs'
                      : 'border-border/60 bg-muted/20 hover:bg-muted/50 text-muted-foreground'
                  }`}
                >
                  <div className="flex items-center gap-3">
                    <span
                      className={`flex size-6 items-center justify-center rounded-md text-xs font-mono font-bold ${
                        isSelected ? 'bg-purple-500 text-white' : 'bg-muted text-muted-foreground'
                      }`}
                    >
                      {opt.id}
                    </span>
                    <div>
                      <div className="text-xs font-bold text-foreground line-clamp-1">{opt.name}</div>
                      <div className="text-[10px] text-muted-foreground font-mono">{opt.category}</div>
                    </div>
                  </div>
                  {isActiveTheme && (
                    <Badge className="text-[9px] font-mono px-1 py-0 bg-purple-500 text-white rounded-md">ACTIVE</Badge>
                  )}
                </button>
              );
            })}
          </div>

          {/* Interactive Inspection Workspace */}
          <div className="lg:col-span-2 rounded-lg border border-border bg-card p-6 flex flex-col space-y-6">
            <div className="flex items-center justify-between border-b border-border pb-4">
              <div>
                <div className="flex items-center gap-2">
                  <Badge variant="outline" className="text-xs font-mono border-purple-500/40 text-purple-400 rounded-md">
                    Option #{activeOption.id}
                  </Badge>
                  <Badge variant="secondary" className="text-xs font-mono rounded-md">
                    {activeOption.category}
                  </Badge>
                  {activeSystemTheme === activeOption.id && (
                    <Badge className="text-xs font-mono bg-purple-500 text-white rounded-md">Active System Default</Badge>
                  )}
                </div>
                <h2 className="text-xl font-bold text-foreground mt-1">{activeOption.name}</h2>
                <p className="text-xs text-muted-foreground mt-0.5">{activeOption.description}</p>
              </div>

              <div className="flex items-center gap-2">
                <Button
                  variant="secondary"
                  size="sm"
                  className="h-8 text-xs gap-1.5 rounded-md bg-purple-500/10 text-purple-300 hover:bg-purple-500/20 border border-purple-500/30"
                  onClick={() => handleSimulateSystemToast(activeOption.id)}
                >
                  <Monitor className="w-3.5 h-3.5 text-purple-400" />
                  Simulate OS Popup
                </Button>
                <Button
                  variant={activeSystemTheme === activeOption.id ? 'default' : 'outline'}
                  size="sm"
                  className={`h-8 text-xs gap-1.5 rounded-md ${
                    activeSystemTheme === activeOption.id ? 'bg-purple-600 hover:bg-purple-700 text-white' : ''
                  }`}
                  onClick={() => handleApplyTheme(activeOption.id)}
                >
                  <Check className="w-3.5 h-3.5" />
                  {activeSystemTheme === activeOption.id ? 'System Active' : 'Set as System Theme'}
                </Button>
              </div>
            </div>

            {/* Interactive Control Toggles */}
            <div className="p-4 rounded-lg bg-muted/30 border border-border/60 space-y-3">
              <div className="text-xs font-semibold text-foreground flex items-center gap-2">
                <Sliders className="w-4 h-4 text-purple-400" />
                Interactive State Tester &amp; Controls
              </div>
              <div className="grid grid-cols-1 sm:grid-cols-2 gap-3 text-xs">
                <div>
                  <label className="text-[11px] text-muted-foreground block mb-1">AI Output Preset</label>
                  <select
                    value={selectedPreset}
                    onChange={(e) => setSelectedPreset(e.target.value)}
                    className="w-full h-8 px-2.5 rounded-md border border-border bg-background text-foreground text-xs focus:outline-none focus:ring-1 focus:ring-purple-500"
                  >
                    <option value="Executive Summary">Executive Summary</option>
                    <option value="Action Items & Tasks">Action Items &amp; Tasks</option>
                    <option value="Full Transcript Only">Full Transcript Only</option>
                    <option value="Technical Spec Notes">Technical Spec Notes</option>
                  </select>
                </div>

                <div>
                  <label className="text-[11px] text-muted-foreground block mb-1">Audio Source Mode</label>
                  <div className="flex items-center gap-1 h-8">
                    <button
                      type="button"
                      onClick={() => setInputMode('mic')}
                      className={`flex-1 h-full rounded-md text-[11px] font-medium border transition-colors ${
                        inputMode === 'mic'
                          ? 'bg-purple-500/20 border-purple-500 text-purple-300'
                          : 'bg-background border-border text-muted-foreground'
                      }`}
                    >
                      Mic Only
                    </button>
                    <button
                      type="button"
                      onClick={() => setInputMode('system')}
                      className={`flex-1 h-full rounded-md text-[11px] font-medium border transition-colors ${
                        inputMode === 'system'
                          ? 'bg-purple-500/20 border-purple-500 text-purple-300'
                          : 'bg-background border-border text-muted-foreground'
                      }`}
                    >
                      System Audio
                    </button>
                    <button
                      type="button"
                      onClick={() => setInputMode('both')}
                      className={`flex-1 h-full rounded-md text-[11px] font-medium border transition-colors ${
                        inputMode === 'both'
                          ? 'bg-purple-500/20 border-purple-500 text-purple-300'
                          : 'bg-background border-border text-muted-foreground'
                      }`}
                    >
                      Dual Input
                    </button>
                  </div>
                </div>
              </div>
            </div>

            {/* Render Stage */}
            <div className="flex-1 flex flex-col items-center justify-center p-8 bg-muted/20 border border-dashed border-border rounded-lg relative min-h-[220px]">
              <div className="text-[10px] font-mono text-muted-foreground uppercase tracking-widest absolute top-3 left-3">
                Exact Stage Preview (400x84px)
              </div>
              <div className="w-[400px] h-[84px]">
                {renderNotificationCard(activeOption, {
                  preset: selectedPreset,
                  inputMode,
                  onStart: () => triggerActionFeedback(`[Option ${activeOption.id}] Start Recording triggered`),
                  onSnooze: () => triggerActionFeedback(`[Option ${activeOption.id}] Snooze 5 minutes triggered`),
                  onDismiss: () => triggerActionFeedback(`[Option ${activeOption.id}] Notification dismissed`),
                })}
              </div>
            </div>
          </div>
        </div>
      )}
    </div>
  );
};

export interface RenderCardOptions {
  preset: string;
  inputMode: 'mic' | 'system' | 'both';
  onStart: () => void;
  onSnooze: () => void;
  onDismiss: () => void;
}

/** Standardized Card Wrapper: Width 400px, Height 84px, rounded-lg, 5s OS Auto-Dismiss Timer (pauses on hover) */
const NotificationCardWrapper: React.FC<{
  onDismiss: () => void;
  children: React.ReactNode;
}> = ({ onDismiss, children }) => {
  const [hovered, setHovered] = useState(false);
  const [progress, setProgress] = useState(100);

  useEffect(() => {
    if (hovered) return;
    const startTime = Date.now();
    const duration = 5000; // 5-second OS Toast timeout

    const interval = setInterval(() => {
      const elapsed = Date.now() - startTime;
      const remaining = Math.max(0, 100 - (elapsed / duration) * 100);
      setProgress(remaining);

      if (remaining <= 0) {
        clearInterval(interval);
        try {
          getCurrentWindow().hide().catch(() => {});
        } catch (e) {
          // ignore in browser mode
        }
        onDismiss();
      }
    }, 50);

    return () => clearInterval(interval);
  }, [hovered, onDismiss]);

  return (
    <div
      onMouseEnter={() => setHovered(true)}
      onMouseLeave={() => setHovered(false)}
      className="w-[400px] h-[84px] rounded-lg border border-border bg-card shadow-2xl overflow-hidden flex flex-col justify-between select-none relative"
      data-tauri-drag-region
    >
      <div className="flex-1 w-full flex items-center justify-between px-3 py-2 min-h-0" data-tauri-drag-region>
        {children}
      </div>

      {/* 5-Second OS Auto-Dismiss Progress Bar */}
      <div className="w-full h-1 bg-muted/40 overflow-hidden shrink-0">
        <div
          className="h-full bg-purple-500/80 transition-all ease-linear"
          style={{ width: `${progress}%` }}
        />
      </div>
    </div>
  );
};

export function renderNotificationCard(option: MeetingNotificationOption, opts: RenderCardOptions) {
  switch (option.id) {
    case 1:
      // Option 1: Compact HUD Bar (400x84px, rounded-lg)
      return (
        <NotificationCardWrapper onDismiss={opts.onDismiss}>
          <div className="flex items-center gap-2.5 min-w-0">
            <div className="flex items-center gap-1 text-emerald-400 shrink-0">
              <span className="size-2 rounded-full bg-emerald-500 animate-pulse" />
              <Radio className="w-4 h-4" />
            </div>
            <div className="grid min-w-0 text-left">
              <span className="text-xs font-bold truncate text-foreground">Google Meet Detected</span>
              <span className="text-[10px] text-muted-foreground truncate">Design Weekly Review</span>
            </div>
          </div>

          <div className="flex items-center gap-1.5 shrink-0">
            <button
              type="button"
              onClick={opts.onDismiss}
              className="px-2 py-1 rounded-md text-xs font-medium text-muted-foreground hover:text-foreground transition-colors"
            >
              Skip
            </button>
            <Button
              size="sm"
              onClick={opts.onStart}
              className="h-7 text-xs rounded-md bg-emerald-500 hover:bg-emerald-600 text-slate-950 font-bold px-3 gap-1"
            >
              <Play className="w-3 h-3 fill-current" /> Record
            </Button>
            <button
              type="button"
              onClick={opts.onDismiss}
              className="p-1 text-muted-foreground hover:text-foreground rounded-md transition-colors ml-0.5"
              title="Dismiss"
            >
              <X className="w-4 h-4" />
            </button>
          </div>
        </NotificationCardWrapper>
      );

    case 2:
      // Option 2: Compact Quick Dock Widget (400x84px, rounded-lg)
      return (
        <NotificationCardWrapper onDismiss={opts.onDismiss}>
          <div className="flex items-center gap-2.5 min-w-0">
            <div className="size-8 rounded-md bg-emerald-500/10 text-emerald-500 flex items-center justify-center border border-emerald-500/20 shrink-0">
              <Mic className="w-4 h-4" />
            </div>
            <div className="grid min-w-0 text-left">
              <span className="text-xs font-bold text-foreground truncate">Webex Meeting Active</span>
              <span className="text-[10px] text-muted-foreground truncate">1-Click Quick Record</span>
            </div>
          </div>

          <div className="flex items-center gap-1.5 shrink-0">
            <button
              type="button"
              onClick={opts.onDismiss}
              className="px-2 py-1 rounded-md text-xs font-medium text-muted-foreground hover:text-foreground transition-colors"
            >
              Skip
            </button>
            <Button size="sm" onClick={opts.onStart} className="h-7 text-xs bg-red-600 hover:bg-red-700 text-white font-bold gap-1 rounded-md px-3">
              <Square className="w-3 h-3 fill-current text-white" /> Record
            </Button>
            <button
              type="button"
              onClick={opts.onDismiss}
              className="p-1 text-muted-foreground hover:text-foreground rounded-md transition-colors ml-0.5"
              title="Dismiss"
            >
              <X className="w-4 h-4" />
            </button>
          </div>
        </NotificationCardWrapper>
      );

    case 3:
      // Option 3: Animated Gradient Border Card (400x84px, rounded-lg)
      return (
        <NotificationCardWrapper onDismiss={opts.onDismiss}>
          <div className="flex items-center gap-2.5 min-w-0">
            <div className="flex -space-x-2 shrink-0">
              <div className="size-7 rounded-md bg-blue-500 border-2 border-card flex items-center justify-center text-[9px] font-bold text-white">
                JD
              </div>
              <div className="size-7 rounded-md bg-emerald-500 border-2 border-card flex items-center justify-center text-[9px] font-bold text-white">
                SK
              </div>
            </div>
            <div className="grid min-w-0 text-left">
              <span className="text-xs font-bold text-foreground truncate">Client Demo Session</span>
              <span className="text-[10px] text-muted-foreground font-mono truncate">100% Local STT</span>
            </div>
          </div>

          <div className="flex items-center gap-1.5 shrink-0">
            <button
              type="button"
              onClick={opts.onDismiss}
              className="px-2 py-1 rounded-md text-xs font-medium text-muted-foreground hover:text-foreground transition-colors"
            >
              Skip
            </button>
            <Button size="sm" onClick={opts.onStart} className="h-7 text-xs bg-primary hover:bg-primary/90 text-primary-foreground font-bold rounded-md px-3">
              Record
            </Button>
            <button
              type="button"
              onClick={opts.onDismiss}
              className="p-1 text-muted-foreground hover:text-foreground rounded-md transition-colors ml-0.5"
              title="Dismiss"
            >
              <X className="w-4 h-4" />
            </button>
          </div>
        </NotificationCardWrapper>
      );

    case 4:
      // Option 4: Stealth Mini Floating Bar (400x84px, rounded-lg)
      return (
        <NotificationCardWrapper onDismiss={opts.onDismiss}>
          <div className="flex items-center gap-2.5 min-w-0">
            <span className="size-2.5 rounded-full bg-emerald-500 animate-ping shrink-0" />
            <div className="grid min-w-0 text-left">
              <span className="text-xs font-bold font-mono text-foreground truncate">Zoom Meeting Active</span>
              <span className="text-[10px] text-muted-foreground font-mono truncate">Audio Feed Ready</span>
            </div>
          </div>

          <div className="flex items-center gap-1.5 shrink-0">
            <button
              type="button"
              onClick={opts.onDismiss}
              className="px-2 py-1 rounded-md text-xs font-medium text-muted-foreground hover:text-foreground transition-colors"
            >
              Skip
            </button>
            <Button
              size="sm"
              onClick={opts.onStart}
              className="h-7 text-xs font-bold rounded-md bg-emerald-500 text-slate-950 hover:bg-emerald-400 px-3"
            >
              Record
            </Button>
            <button
              type="button"
              onClick={opts.onDismiss}
              className="p-1 text-muted-foreground hover:text-foreground rounded-md transition-colors ml-0.5"
              title="Dismiss"
            >
              <X className="w-4 h-4" />
            </button>
          </div>
        </NotificationCardWrapper>
      );

    case 5:
      // Option 5: Left-Accent Banner (400x84px, rounded-lg)
      return (
        <NotificationCardWrapper onDismiss={opts.onDismiss}>
          <div className="flex items-center gap-2.5 min-w-0">
            <div className="relative flex size-8 items-center justify-center rounded-md bg-red-500/10 text-red-500 shrink-0 border border-red-500/20">
              <span className="absolute size-2 rounded-full bg-red-500 animate-ping" />
              <Mic className="w-4 h-4 relative z-10" />
            </div>
            <div className="grid min-w-0 text-left">
              <span className="text-xs font-bold text-foreground truncate">Product Sync &amp; Specs</span>
              <span className="text-[10px] text-muted-foreground font-mono truncate">3 participants</span>
            </div>
          </div>

          <div className="flex items-center gap-1.5 shrink-0">
            <button
              type="button"
              onClick={opts.onDismiss}
              className="px-2 py-1 rounded-md text-xs font-medium text-muted-foreground hover:text-foreground transition-colors"
            >
              Skip
            </button>
            <Button size="sm" onClick={opts.onStart} className="h-7 text-xs bg-red-600 hover:bg-red-700 text-white font-bold rounded-md px-3">
              Record
            </Button>
            <button
              type="button"
              onClick={opts.onDismiss}
              className="p-1 text-muted-foreground hover:text-foreground rounded-md transition-colors ml-0.5"
              title="Dismiss"
            >
              <X className="w-4 h-4" />
            </button>
          </div>
        </NotificationCardWrapper>
      );

    case 6:
      // Option 6: Waveform Control Bar (400x84px, rounded-lg)
      return (
        <NotificationCardWrapper onDismiss={opts.onDismiss}>
          <div className="flex items-center gap-2 min-w-0">
            <Flame className="w-4 h-4 text-cyan-400 animate-pulse shrink-0" />
            <div className="flex items-center gap-0.5 h-4 w-16 bg-slate-900/80 rounded px-1 border border-cyan-500/20 shrink-0">
              {[40, 75, 30, 90, 60, 100, 45, 80].map((h, i) => (
                <span
                  key={i}
                  className="w-1 bg-cyan-400/80 rounded-full animate-pulse"
                  style={{ height: `${h}%`, animationDelay: `${i * 90}ms` }}
                />
              ))}
            </div>
          </div>

          <div className="flex items-center gap-1.5 shrink-0">
            <button
              type="button"
              onClick={opts.onDismiss}
              className="px-2 py-1 rounded-md text-xs font-medium text-muted-foreground hover:text-foreground transition-colors"
            >
              Skip
            </button>
            <Button
              size="sm"
              onClick={opts.onStart}
              className="h-7 text-xs font-mono bg-cyan-500 hover:bg-cyan-400 text-slate-950 font-bold rounded-md px-3"
            >
              Record
            </Button>
            <button
              type="button"
              onClick={opts.onDismiss}
              className="p-1 text-muted-foreground hover:text-foreground rounded-md transition-colors ml-0.5"
              title="Dismiss"
            >
              <X className="w-4 h-4" />
            </button>
          </div>
        </NotificationCardWrapper>
      );

    case 7:
      // Option 7: AI Copilot Quick Toast (400x84px, rounded-lg)
      return (
        <NotificationCardWrapper onDismiss={opts.onDismiss}>
          <div className="flex items-center gap-2 min-w-0">
            <Wand2 className="w-4 h-4 text-amber-500 shrink-0" />
            <div className="grid min-w-0 text-left">
              <span className="text-xs font-bold text-foreground truncate">Zoom: Standup &amp; Backlog</span>
              <span className="text-[10px] text-amber-400 font-mono truncate">{opts.preset}</span>
            </div>
          </div>

          <div className="flex items-center gap-1.5 shrink-0">
            <button
              type="button"
              onClick={opts.onDismiss}
              className="px-2 py-1 rounded-md text-xs font-medium text-muted-foreground hover:text-foreground transition-colors"
            >
              Skip
            </button>
            <Button size="sm" onClick={opts.onStart} className="h-7 text-xs bg-amber-500 hover:bg-amber-600 text-slate-950 font-bold rounded-md px-3 gap-1">
              <Sparkles className="w-3 h-3 fill-current" /> Record
            </Button>
            <button
              type="button"
              onClick={opts.onDismiss}
              className="p-1 text-muted-foreground hover:text-foreground rounded-md transition-colors ml-0.5"
              title="Dismiss"
            >
              <X className="w-4 h-4" />
            </button>
          </div>
        </NotificationCardWrapper>
      );

    case 8:
      // Option 8: Corner Action Tray (400x84px, rounded-lg)
      return (
        <NotificationCardWrapper onDismiss={opts.onDismiss}>
          <div className="flex items-center gap-2.5 min-w-0">
            <div className="size-8 rounded-md bg-blue-500/10 text-blue-500 flex items-center justify-center border border-blue-500/20 shrink-0">
              <Users className="w-4 h-4" />
            </div>
            <div className="grid min-w-0 text-left">
              <span className="text-xs font-bold text-foreground truncate">Teams: Strategy Review</span>
              <span className="text-[10px] text-muted-foreground truncate">4 attendees • 100% Local</span>
            </div>
          </div>

          <div className="flex items-center gap-1.5 shrink-0">
            <button
              type="button"
              onClick={opts.onDismiss}
              className="px-2 py-1 rounded-md text-xs font-medium text-muted-foreground hover:text-foreground transition-colors"
            >
              Skip
            </button>
            <Button size="sm" onClick={opts.onStart} className="h-7 text-xs bg-blue-600 hover:bg-blue-700 text-white font-bold rounded-md px-3">
              Record
            </Button>
            <button
              type="button"
              onClick={opts.onDismiss}
              className="p-1 text-muted-foreground hover:text-foreground rounded-md transition-colors ml-0.5"
              title="Dismiss"
            >
              <X className="w-4 h-4" />
            </button>
          </div>
        </NotificationCardWrapper>
      );

    case 9:
      // Option 9: Edge-Anchored Mini HUD (400x84px, rounded-lg)
      return (
        <NotificationCardWrapper onDismiss={opts.onDismiss}>
          <div className="flex items-center gap-2 min-w-0">
            <div className="size-2.5 rounded-full bg-emerald-500 animate-ping shrink-0" />
            <div className="grid min-w-0 text-left">
              <span className="text-xs font-bold font-mono text-foreground truncate">Google Meet Active</span>
              <span className="text-[10px] text-emerald-400 font-mono truncate">100% Local STT</span>
            </div>
          </div>

          <div className="flex items-center gap-1.5 shrink-0">
            <button
              type="button"
              onClick={opts.onDismiss}
              className="px-2 py-1 rounded-md text-xs font-medium text-muted-foreground hover:text-foreground transition-colors"
            >
              Skip
            </button>
            <Button size="sm" onClick={opts.onStart} className="h-7 text-xs bg-emerald-500 hover:bg-emerald-400 text-slate-950 font-bold rounded-md px-3">
              Record
            </Button>
            <button
              type="button"
              onClick={opts.onDismiss}
              className="p-1 text-muted-foreground hover:text-foreground rounded-md transition-colors ml-0.5"
              title="Dismiss"
            >
              <X className="w-4 h-4" />
            </button>
          </div>
        </NotificationCardWrapper>
      );

    case 10:
    default:
      // Option 10: Micro Pre-Flight Command Card (400x84px, rounded-lg)
      return (
        <NotificationCardWrapper onDismiss={opts.onDismiss}>
          <div className="flex items-center gap-2 min-w-0">
            <ShieldCheck className="w-4 h-4 text-purple-400 shrink-0" />
            <div className="grid min-w-0 text-left">
              <span className="text-xs font-bold text-foreground truncate">Relay Pre-Flight Check</span>
              <span className="text-[10px] text-emerald-400 font-mono truncate">Mic Signal OK (-12dB)</span>
            </div>
          </div>

          <div className="flex items-center gap-1.5 shrink-0">
            <button
              type="button"
              onClick={opts.onDismiss}
              className="px-2 py-1 rounded-md text-xs font-medium text-muted-foreground hover:text-foreground transition-colors"
            >
              Skip
            </button>
            <Button size="sm" onClick={opts.onStart} className="h-7 text-xs bg-purple-600 hover:bg-purple-700 text-white font-bold gap-1 rounded-md px-3">
              <Zap className="w-3 h-3 fill-current" /> Record
            </Button>
            <button
              type="button"
              onClick={opts.onDismiss}
              className="p-1 text-muted-foreground hover:text-foreground rounded-md transition-colors ml-0.5"
              title="Dismiss"
            >
              <X className="w-4 h-4" />
            </button>
          </div>
        </NotificationCardWrapper>
      );
  }
}
