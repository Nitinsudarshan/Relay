import React, { useState, useEffect } from 'react';
import { PTTWidget } from './components/capture/PTTWidget';
import { ScribbleViewer } from './components/scribble/ScribbleViewer';
import { ProviderSettings } from './components/settings/ProviderSettings';
import { ThemeToggle } from './components/ThemeToggle';
import { RelayLogo } from './components/common/RelayLogo';
import { ChangelogModal } from './components/common/ChangelogModal';
import { ProcessedPipelineResult } from './types';
import { listen } from '@tauri-apps/api/event';
import {
  Mic,
  Sparkles,
  Settings,
  ShieldCheck,
  Activity,
  Sidebar as SidebarIcon,
  ChevronRight,
  HardDrive,
  Cloud,
  User,
  ArrowUpRight
} from 'lucide-react';
import { Badge } from '@/components/ui/badge';
import { Button } from '@/components/ui/button';
import { cn } from '@/lib/utils';

export const App: React.FC = () => {
  const [activeTab, setActiveTab] = useState<
    'capture' | 'scribble' | 'settings'
  >('capture');
  const [lastResult, setLastResult] = useState<ProcessedPipelineResult | null>(null);
  const [sidebarOpen, setSidebarOpen] = useState(true);
  const [changelogOpen, setChangelogOpen] = useState(false);

  const handleProcessComplete = (result: ProcessedPipelineResult) => {
    setLastResult(result);
    if (result.mode === 'scribble') {
      setActiveTab('scribble');
    }
  };

  // The dictation pill defaults to living in its own floating desktop
  // window, separate from this one — this main window has no direct
  // handle to it, so it learns a capture finished (and switches to the
  // Scribble tab) from the backend event instead of a prop callback.
  // Also covers the in-app fallback pill for free.
  useEffect(() => {
    const unlistenPromise = listen<ProcessedPipelineResult>('capture-processed', ({ payload }) =>
      handleProcessComplete(payload)
    );
    const unlistenTabPromise = listen<string>('navigate-tab', ({ payload }) => {
      if (
        payload === 'capture' ||
        payload === 'scribble' ||
        payload === 'settings'
      ) {
        setActiveTab(payload as any);
      }
    });
    return () => {
      unlistenPromise.then((unlisten) => unlisten());
      unlistenTabPromise.then((unlisten) => unlisten());
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);


  const renderHeroHeader = () => {
    switch (activeTab) {
      case 'capture':
        return (
          <div className="mb-6">
            <p className="font-mono text-[10px] font-semibold text-muted-foreground uppercase tracking-widest mb-1">
              RELAY · TODAY
            </p>
            <h1 className="text-2xl md:text-3xl font-extrabold tracking-tight text-foreground">
              Today, <span className="italic text-primary">Nitin</span> captured.
            </h1>
            <p className="text-xs text-muted-foreground mt-1">
              Live push-to-talk voice memory & task extraction dashboard.
            </p>
          </div>
        );
      case 'scribble':
        return (
          <div className="mb-6">
            <p className="font-mono text-[10px] font-semibold text-muted-foreground uppercase tracking-widest mb-1">
              RELAY · SCRIBBLES
            </p>
            <h1 className="text-2xl md:text-3xl font-extrabold tracking-tight text-foreground">
              Raw thoughts, <span className="italic text-primary">polished</span> state.
            </h1>
            <p className="text-xs text-muted-foreground mt-1">
              Structured markdown notes with raw audio backstop intact.
            </p>
          </div>
        );
      case 'settings':
        return (
          <div className="mb-6">
            <p className="font-mono text-[10px] font-semibold text-muted-foreground uppercase tracking-widest mb-1">
              RELAY · SETTINGS
            </p>
            <h1 className="text-2xl md:text-3xl font-extrabold tracking-tight text-foreground">
              How Relay <span className="italic text-primary">behaves</span>.
            </h1>
            <p className="text-xs text-muted-foreground mt-1">
              Configure local LLMs, cloud fallback providers, triggers, and privacy bounds.
            </p>
          </div>
        );
    }
  };

  return (
    <div className="flex h-screen w-screen bg-background text-foreground overflow-hidden font-sans">
      {/* Navigation Sidebar */}
      <aside
        className={`${
          sidebarOpen ? 'w-64 p-4 border-r border-sidebar-border opacity-100' : 'w-0 p-0 border-none opacity-0 pointer-events-none'
        } transition-all duration-300 bg-sidebar flex flex-col shrink-0 select-none overflow-hidden z-20`}
      >
        {/* Logo Header */}
        <div className="flex items-center gap-3 px-2 py-3 mb-4 border-b border-sidebar-border">
          <div className="flex aspect-square size-9 items-center justify-center rounded-xl bg-card border border-border text-foreground shadow-xs">
            <RelayLogo className="w-5 h-5" />
          </div>
          <div className="grid flex-1 text-left text-sm leading-tight">
            <span className="truncate font-bold tracking-wider text-sidebar-foreground">RELAY</span>
            <span className="truncate text-[10px] text-muted-foreground font-mono uppercase tracking-widest">
              Desktop Native
            </span>
          </div>
        </div>

        {/* MENU mono-caps label */}
        <div className="px-3 mb-2">
          <span className="font-mono text-[10px] font-bold tracking-widest text-muted-foreground uppercase">
            MENU
          </span>
        </div>

        {/* Navigation Items with active accent dot */}
        <nav className="flex-1 space-y-1">
          <button
            onClick={() => setActiveTab('capture')}
            className={`w-full flex items-center justify-between px-3 py-2 rounded-lg text-xs font-medium transition-all ${
              activeTab === 'capture'
                ? 'bg-sidebar-accent text-sidebar-accent-foreground font-semibold shadow-xs'
                : 'text-muted-foreground hover:bg-sidebar-accent/50 hover:text-sidebar-foreground'
            }`}
          >
            <div className="flex items-center gap-2.5">
              <Mic className="w-4 h-4" />
              <span>Voice Capture</span>
            </div>
            {activeTab === 'capture' && <span className="w-1.5 h-1.5 rounded-full bg-primary shrink-0" />}
          </button>

          <button
            onClick={() => setActiveTab('scribble')}
            className={`w-full flex items-center justify-between px-3 py-2 rounded-lg text-xs font-medium transition-all ${
              activeTab === 'scribble'
                ? 'bg-sidebar-accent text-sidebar-accent-foreground font-semibold shadow-xs'
                : 'text-muted-foreground hover:bg-sidebar-accent/50 hover:text-sidebar-foreground'
            }`}
          >
            <div className="flex items-center gap-2.5">
              <Sparkles className="w-4 h-4" />
              <span>Scribble Notes</span>
            </div>
            {activeTab === 'scribble' && <span className="w-1.5 h-1.5 rounded-full bg-primary shrink-0" />}
          </button>

          <button
            onClick={() => setActiveTab('settings')}
            className={`w-full flex items-center justify-between px-3 py-2 rounded-lg text-xs font-medium transition-all ${
              activeTab === 'settings'
                ? 'bg-sidebar-accent text-sidebar-accent-foreground font-semibold shadow-xs'
                : 'text-muted-foreground hover:bg-sidebar-accent/50 hover:text-sidebar-foreground'
            }`}
          >
            <div className="flex items-center gap-2.5">
              <Settings className="w-4 h-4" />
              <span>Settings</span>
            </div>
            {activeTab === 'settings' && <span className="w-1.5 h-1.5 rounded-full bg-primary shrink-0" />}
          </button>
        </nav>

        {/* Pinned Account & Hybrid Sync Card Block */}
        <div className="mt-auto pt-4 border-t border-sidebar-border space-y-3">
          <div className="p-2.5 rounded-xl bg-card border border-border flex items-center gap-2.5 shadow-xs">
            <div className="w-7 h-7 rounded-full bg-primary text-primary-foreground font-bold flex items-center justify-center text-xs shrink-0">
              N
            </div>
            <div className="grid flex-1 leading-tight min-w-0">
              <span className="text-xs font-bold text-foreground truncate">Nitin Sudarshan</span>
              <span className="text-[10px] text-muted-foreground truncate">nitin@example.com</span>
            </div>
            <Badge variant="outline" className="text-[9px] font-mono px-1.5 py-0 border-primary/30 text-primary">
              Pro
            </Badge>
          </div>

          <div className="flex items-center justify-between text-[10px] text-muted-foreground px-1">
            <div className="flex items-center gap-1.5 font-medium text-emerald-500">
              <ShieldCheck className="w-3.5 h-3.5" />
              <span>Local Vault ($0)</span>
            </div>
            <button
              type="button"
              onClick={() => setChangelogOpen(true)}
              className="flex items-center gap-1 font-mono hover:text-primary transition-colors cursor-pointer group"
              title="View Release Notes & Changelog"
            >
              <Activity className="w-3 h-3 text-primary group-hover:animate-pulse" />
              <span className="underline decoration-dotted underline-offset-2">v0.4.4</span>
            </button>
          </div>
        </div>
      </aside>

      {/* Changelog Modal */}
      <ChangelogModal
        open={changelogOpen}
        onClose={() => setChangelogOpen(false)}
        currentVersion="0.4.4"
      />

      {/* Main Content Area */}
      <div className="flex-1 flex flex-col min-w-0 overflow-hidden">
        {/* Top Header Bar */}
        <header className="h-14 bg-sidebar border-b border-border px-4 flex items-center justify-between shrink-0 select-none">
          <div className="flex items-center gap-3">
            <Button
              variant="ghost"
              size="icon"
              className="h-8 w-8 text-muted-foreground hover:text-foreground"
              onClick={() => setSidebarOpen(!sidebarOpen)}
              aria-label="Toggle Sidebar Navigation"
            >
              <SidebarIcon className="w-4 h-4" />
            </Button>

            <div className="h-4 w-px bg-border" />

            <div className="flex items-center gap-1.5 text-xs text-muted-foreground font-mono uppercase tracking-wider">
              <span>RELAY</span>
              <ChevronRight className="w-3.5 h-3.5 text-muted-foreground/60" />
              <span className="font-semibold text-foreground">{activeTab}</span>
            </div>
          </div>

          <div className="flex items-center gap-2">
            <Badge variant="outline" className="gap-1.5 text-xs px-2.5 py-1 font-mono border-border">
              <HardDrive className="w-3 h-3 text-primary" /> Local Mode (LanceDB)
            </Badge>
            <ThemeToggle />
          </div>
        </header>

        {/* View Surface Container */}
        <main className="flex-1 p-4 md:p-6 overflow-y-auto flex flex-col bg-background">
          {/* Top Hero Banner */}
          {renderHeroHeader()}

          {/* Always mounted (never conditionally rendered on `activeTab`) —
              when the floating pill is turned off in Settings, this is the
              *only* place the docked pill lives, and it owns the
              capture-state-changed/capture-level listeners that make
              Ctrl+Space (held from anywhere in the OS) show up as a visible
              recording state. Conditionally mounting it per-tab used to
              tear those listeners down the moment the user left the Voice
              Capture tab, silently orphaning any hotkey-triggered session.
              Visibility is CSS-only so the component — and its listeners —
              stay alive across tab switches. */}
          <div
            className={cn(
              'flex-1 flex flex-col max-w-4xl mx-auto w-full',
              activeTab !== 'capture' && 'hidden'
            )}
          >
            <PTTWidget />

            {lastResult && activeTab === 'capture' && (
              <div className="mt-4 flex-1">
                {lastResult.mode === 'scribble' ? (
                  <ScribbleViewer content={lastResult.output_markdown} transcript={lastResult.transcript} />
                ) : (
                  <div className="rounded-xl p-4 border border-border bg-card text-xs font-mono text-foreground shadow-sm">
                    <p className="font-semibold text-emerald-500 mb-1">Result Summary:</p>
                    <p className="text-muted-foreground">{lastResult.output_markdown}</p>
                  </div>
                )}
              </div>
            )}
          </div>

          {activeTab === 'scribble' && (
            <ScribbleViewer
              content={lastResult?.output_markdown || ''}
              transcript={lastResult?.transcript || ''}
            />
          )}
          {activeTab === 'settings' && <ProviderSettings />}
        </main>
      </div>
    </div>
  );
};
