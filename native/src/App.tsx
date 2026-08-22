import React, { useState, useEffect } from 'react';
import { VoiceNotePage } from './components/voicenotes/VoiceNotePage';
import { MeetingPage } from './components/meetings/MeetingPage';
import { MeetingDetectionPopup } from './components/meetings/MeetingDetectionPopup';
import { ScribbleViewer } from './components/scribble/ScribbleViewer';
import { ProviderSettings } from './components/settings/ProviderSettings';
import { ThemeToggle } from './components/ThemeToggle';
import { RelayLogo } from './components/common/RelayLogo';
import { ChangelogModal } from './components/common/ChangelogModal';
import { WelcomeModal } from './components/common/WelcomeModal';
import { AccountExplanationModal } from './components/common/AccountExplanationModal';
import { ProcessedPipelineResult, DetectedMeetingPayload, Meeting, RelayAccount, AppSettings } from './types';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import {
  Mic,
  Calendar,
  Sparkles,
  Settings,
  ShieldCheck,
  Activity,
  Sidebar as SidebarIcon,
  ChevronRight,
  User,
} from 'lucide-react';
import { Badge } from '@/components/ui/badge';
import { Button } from '@/components/ui/button';

export type MainTabType = 'capture' | 'meetings' | 'scribble' | 'settings';

const TAB_LABELS: Record<MainTabType, string> = {
  capture: 'Voice Note',
  meetings: 'Meetings',
  scribble: 'Scribbles',
  settings: 'Settings',
};

export const App: React.FC = () => {
  const [activeTab, setActiveTab] = useState<MainTabType>('capture');
  const [lastResult, setLastResult] = useState<ProcessedPipelineResult | null>(null);
  const [sidebarOpen, setSidebarOpen] = useState(true);
  const [changelogOpen, setChangelogOpen] = useState(false);
  const [appVersion, setAppVersion] = useState<string>('0.8.2');
  const [account, setAccount] = useState<RelayAccount | null>(null);
  const [welcomeOpen, setWelcomeOpen] = useState(false);
  const [explanationOpen, setExplanationOpen] = useState(false);

  const refreshAccountAndSettings = async () => {
    try {
      const [ver, acc, setts] = await Promise.all([
        invoke<string>('get_app_version'),
        invoke<RelayAccount>('get_account_state'),
        invoke<AppSettings>('get_settings'),
      ]);
      if (ver) setAppVersion(ver);
      if (acc) setAccount(acc);
      if (setts && setts.diagnostics && setts.diagnostics.first_run_completed === false) {
        setWelcomeOpen(true);
      }
    } catch (err) {
      console.warn('Could not load initial account/settings:', err);
    }
  };

  useEffect(() => {
    refreshAccountAndSettings();

    // 1. Listen for backend Tauri account-changed events (sign-in, sign-out, delete-account)
    const unlistenPromise = listen<RelayAccount>('account-changed', (event) => {
      if (event.payload) {
        setAccount(event.payload);
      }
    });

    // 2. Listen for DOM custom events
    const handleDomAccountChange = (e: Event) => {
      const customEvent = e as CustomEvent<RelayAccount>;
      if (customEvent.detail) {
        setAccount(customEvent.detail);
      }
    };
    window.addEventListener('relay-account-changed', handleDomAccountChange);

    return () => {
      unlistenPromise.then((unlisten) => unlisten());
      window.removeEventListener('relay-account-changed', handleDomAccountChange);
    };
  }, []);

  const handleWelcomeGoogle = async () => {
    const acc = await invoke<RelayAccount>('start_google_sign_in');
    await invoke('complete_first_run');
    setAccount(acc);
    setWelcomeOpen(false);
    setExplanationOpen(true);
  };

  const handleWelcomeLocally = async () => {
    await invoke('complete_first_run');
    setWelcomeOpen(false);
  };

  const handleCreateAndStartDetectedMeeting = async (detected: DetectedMeetingPayload) => {
    try {
      const newMeeting = await invoke<Meeting>('create_meeting', {
        title: detected.title,
        provider: detected.provider,
        seriesId: null,
      });

      if (detected.meeting_url || detected.scheduled_start) {
        newMeeting.provider_metadata = { meeting_url: detected.meeting_url };
        newMeeting.scheduled_start = detected.scheduled_start || newMeeting.scheduled_start;
        await invoke('save_meeting', { meeting: newMeeting });
      }

      setActiveTab('meetings');
      await invoke('start_meeting_recording', { meetingId: newMeeting.id });
    } catch (err) {
      console.error('Failed to create and start detected meeting:', err);
    }
  };

  const renderHeroHeader = () => {
    switch (activeTab) {
      case 'capture':
        return (
          <div className="relative rounded-lg border border-border/80 bg-gradient-to-br from-card via-card/95 to-emerald-500/5 p-5 md:p-6 shadow-xs overflow-hidden mb-5 shrink-0">
            <div className="absolute -right-10 -top-10 w-40 h-40 bg-emerald-500/10 rounded-full blur-3xl pointer-events-none" />
            <div className="relative z-10 space-y-1.5">
              <div className="flex items-center gap-2">
                <Badge variant="outline" className="text-[10px] font-mono uppercase tracking-wider text-emerald-500 border-emerald-500/30 bg-emerald-500/5 gap-1.5 py-0.5 px-2">
                  <Mic className="w-3 h-3 text-emerald-500" />
                  <span>Capture Surface</span>
                </Badge>
              </div>
              <h1 className="text-xl md:text-2xl font-extrabold tracking-tight text-foreground">
                Voice <span className="italic text-primary">Notes</span>
              </h1>
              <p className="text-xs text-muted-foreground max-w-2xl leading-relaxed">
                Everything you dictate, captured in one truthful history.
              </p>
            </div>
          </div>
        );
      case 'meetings':
        return (
          <div className="relative rounded-lg border border-border/80 bg-gradient-to-br from-card via-card/95 to-blue-500/5 p-5 md:p-6 shadow-xs overflow-hidden mb-5 shrink-0">
            <div className="absolute -right-10 -top-10 w-40 h-40 bg-blue-500/10 rounded-full blur-3xl pointer-events-none" />
            <div className="relative z-10 space-y-1.5">
              <div className="flex items-center gap-2">
                <Badge variant="outline" className="text-[10px] font-mono uppercase tracking-wider text-blue-500 border-blue-500/30 bg-blue-500/5 gap-1.5 py-0.5 px-2">
                  <Calendar className="w-3 h-3 text-blue-500" />
                  <span>Source & Capture Surface</span>
                </Badge>
              </div>
              <h1 className="text-xl md:text-2xl font-extrabold tracking-tight text-foreground">
                Meetings & <span className="italic text-primary">Conferences</span>
              </h1>
              <p className="text-xs text-muted-foreground max-w-2xl leading-relaxed">
                Capture standalone and recurring meetings, preserve truthful source context, and extract living knowledge.
              </p>
            </div>
          </div>
        );
      case 'scribble':
        return (
          <div className="relative rounded-lg border border-border/80 bg-gradient-to-br from-card via-card/95 to-primary/5 p-5 md:p-6 shadow-xs overflow-hidden mb-5 shrink-0">
            <div className="absolute -right-10 -top-10 w-40 h-40 bg-primary/10 rounded-full blur-3xl pointer-events-none" />
            <div className="relative z-10 space-y-1.5">
              <div className="flex items-center gap-2">
                <Badge variant="outline" className="text-[10px] font-mono uppercase tracking-wider text-primary border-primary/30 bg-primary/5 gap-1.5 py-0.5 px-2">
                  <Sparkles className="w-3 h-3 text-primary" />
                  <span>Knowledge Layer</span>
                </Badge>
              </div>
              <h1 className="text-xl md:text-2xl font-extrabold tracking-tight text-foreground">
                Connected thoughts, <span className="italic text-primary">living</span> knowledge.
              </h1>
              <p className="text-xs text-muted-foreground max-w-2xl leading-relaxed">
                Capture atomic thoughts, connect related ideas, and explore your Obsidian-compatible knowledge graph.
              </p>
            </div>
          </div>
        );
      case 'settings':
        return (
          <div className="relative rounded-lg border border-border/80 bg-gradient-to-br from-card via-card/95 to-purple-500/5 p-5 md:p-6 shadow-xs overflow-hidden mb-5 shrink-0">
            <div className="absolute -right-10 -top-10 w-40 h-40 bg-purple-500/10 rounded-full blur-3xl pointer-events-none" />
            <div className="relative z-10 space-y-1.5">
              <div className="flex items-center gap-2">
                <Badge variant="outline" className="text-[10px] font-mono uppercase tracking-wider text-purple-500 border-purple-500/30 bg-purple-500/5 gap-1.5 py-0.5 px-2">
                  <Settings className="w-3 h-3 text-purple-500" />
                  <span>Preferences & Vault</span>
                </Badge>
              </div>
              <h1 className="text-xl md:text-2xl font-extrabold tracking-tight text-foreground">
                How Relay <span className="italic text-primary">behaves</span>.
              </h1>
              <p className="text-xs text-muted-foreground max-w-2xl leading-relaxed">
                Configure local LLMs, triggers, privacy bounds, and manage 30-day trash recovery.
              </p>
            </div>
          </div>
        );
    }
  };

  return (
    <div className="flex h-screen w-screen bg-background text-foreground overflow-hidden font-sans">
      {/* Non-blocking Meeting Detection Popup Notification */}
      <MeetingDetectionPopup
        onStartMeetingRecording={async (mId) => {
          setActiveTab('meetings');
          await invoke('start_meeting_recording', { meetingId: mId });
        }}
        onCreateAndStartMeeting={handleCreateAndStartDetectedMeeting}
      />

      {/* Navigation Sidebar (Relay 4-item Structure: Voice Note, Meetings, Scribbles, Settings) */}
      <aside
        className={`${
          sidebarOpen ? 'w-64 p-4 border-r border-sidebar-border opacity-100' : 'w-0 p-0 border-none opacity-0 pointer-events-none'
        } transition-all duration-300 bg-sidebar flex flex-col shrink-0 select-none overflow-hidden z-20`}
      >
        {/* Logo Header */}
        <div className="flex items-center gap-3 px-2 py-3 mb-4 border-b border-sidebar-border">
          <div className="flex aspect-square size-9 items-center justify-center rounded-lg bg-card border border-border text-foreground shadow-xs">
            <RelayLogo className="w-5 h-5" />
          </div>
          <div className="grid flex-1 text-left text-sm leading-tight">
            <span className="truncate font-bold tracking-wider text-sidebar-foreground">RELAY</span>
            <span className="truncate text-[10px] text-muted-foreground font-mono uppercase tracking-widest">
              Desktop Native
            </span>
          </div>
        </div>

        {/* Navigation Items (Voice Note, Meetings, Scribbles, Settings) */}
        <nav className="flex-1 space-y-1">
          {/* 1. Voice Note */}
          <button
            onClick={() => setActiveTab('capture')}
            className={`w-full flex items-center justify-between px-3 py-2 rounded-lg text-xs font-medium transition-all ${
              activeTab === 'capture'
                ? 'bg-sidebar-accent text-sidebar-accent-foreground font-semibold shadow-xs'
                : 'text-muted-foreground hover:bg-sidebar-accent/50 hover:text-sidebar-foreground'
            }`}
          >
            <div className="flex items-center gap-2.5">
              <Mic className="w-4 h-4 text-emerald-500" />
              <span>Voice Note</span>
            </div>
            {activeTab === 'capture' && <span className="w-1.5 h-1.5 rounded-full bg-primary shrink-0" />}
          </button>

          {/* 2. Meetings */}
          <button
            onClick={() => setActiveTab('meetings')}
            className={`w-full flex items-center justify-between px-3 py-2 rounded-lg text-xs font-medium transition-all ${
              activeTab === 'meetings'
                ? 'bg-sidebar-accent text-sidebar-accent-foreground font-semibold shadow-xs'
                : 'text-muted-foreground hover:bg-sidebar-accent/50 hover:text-sidebar-foreground'
            }`}
          >
            <div className="flex items-center gap-2.5">
              <Calendar className="w-4 h-4 text-blue-500" />
              <span>Meetings</span>
            </div>
            {activeTab === 'meetings' && <span className="w-1.5 h-1.5 rounded-full bg-primary shrink-0" />}
          </button>

          {/* 3. Scribbles */}
          <button
            onClick={() => setActiveTab('scribble')}
            className={`w-full flex items-center justify-between px-3 py-2 rounded-lg text-xs font-medium transition-all ${
              activeTab === 'scribble'
                ? 'bg-sidebar-accent text-sidebar-accent-foreground font-semibold shadow-xs'
                : 'text-muted-foreground hover:bg-sidebar-accent/50 hover:text-sidebar-foreground'
            }`}
          >
            <div className="flex items-center gap-2.5">
              <Sparkles className="w-4 h-4 text-amber-500" />
              <span>Scribbles</span>
            </div>
            {activeTab === 'scribble' && <span className="w-1.5 h-1.5 rounded-full bg-primary shrink-0" />}
          </button>

          {/* 4. Settings */}
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
          <button
            type="button"
            onClick={() => setActiveTab('settings')}
            className="w-full p-2.5 rounded-lg bg-card hover:bg-card/80 border border-border flex items-center gap-2.5 shadow-xs text-left transition-colors cursor-pointer group"
            title="Open Account & Settings"
          >
            {account?.authenticated && account.profile_image ? (
              <img
                src={account.profile_image}
                alt="Profile"
                className="w-7 h-7 rounded-full border border-primary/30 object-cover shrink-0"
              />
            ) : (
              <div className="w-7 h-7 rounded-full bg-primary/20 text-primary font-bold flex items-center justify-center text-xs shrink-0">
                {account?.authenticated && account.display_name
                  ? account.display_name.charAt(0).toUpperCase()
                  : <User className="w-3.5 h-3.5" />}
              </div>
            )}
            <div className="grid flex-1 leading-tight min-w-0">
              <span className="text-xs font-bold text-foreground truncate group-hover:text-primary transition-colors">
                {account?.authenticated ? account.display_name || account.email : 'Local Mode'}
              </span>
              <span className="text-[10px] text-muted-foreground truncate font-mono">
                {account?.authenticated ? account.email : '100% On-Device'}
              </span>
            </div>
            <Badge variant="outline" className="text-[9px] font-mono px-1.5 py-0 border-primary/30 text-primary">
              {account?.authenticated ? 'Google' : 'Local'}
            </Badge>
          </button>

          <div className="flex items-center justify-between text-[10px] text-muted-foreground px-1">
            <div className="flex items-center gap-1.5 font-medium text-emerald-500">
              <ShieldCheck className="w-3.5 h-3.5" />
              <span>Local Vault</span>
            </div>
            <button
              type="button"
              onClick={() => setChangelogOpen(true)}
              className="flex items-center gap-1 font-mono hover:text-primary transition-colors cursor-pointer group"
              title="View Release Notes & Changelog"
            >
              <Activity className="w-3 h-3 text-primary group-hover:animate-pulse" />
              <span className="underline decoration-dotted underline-offset-2">v{appVersion}</span>
            </button>
          </div>
        </div>
      </aside>

      {/* Welcome First-Launch Onboarding Modal */}
      <WelcomeModal
        isOpen={welcomeOpen}
        onContinueGoogle={handleWelcomeGoogle}
        onContinueLocally={handleWelcomeLocally}
      />

      {/* Account Trust & Privacy Explanation Modal */}
      <AccountExplanationModal
        isOpen={explanationOpen}
        onClose={() => setExplanationOpen(false)}
      />

      {/* Changelog Modal */}
      <ChangelogModal
        open={changelogOpen}
        onClose={() => setChangelogOpen(false)}
        currentVersion={appVersion}
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
              <span className="font-semibold text-foreground">{TAB_LABELS[activeTab]}</span>
            </div>
          </div>

          <div className="flex items-center gap-2">
            <ThemeToggle />
          </div>
        </header>

        {/* View Surface Container */}
        <main className="flex-1 p-4 md:p-6 overflow-y-auto flex flex-col bg-background">
          {renderHeroHeader()}

          {activeTab === 'capture' && <VoiceNotePage />}

          {activeTab === 'meetings' && (
            <MeetingPage onNavigateToScribbles={() => setActiveTab('scribble')} />
          )}

          {activeTab === 'scribble' && <ScribbleViewer />}

          {activeTab === 'settings' && <ProviderSettings />}
        </main>
      </div>
    </div>
  );
};
