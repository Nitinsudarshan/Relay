import React, { useState, useEffect } from 'react';
import { VoiceNotePage } from './components/voicenotes/VoiceNotePage';
import { ScribbleViewer } from './components/scribble/ScribbleViewer';
import { MeetingsV2View } from './components/meetings_v2/MeetingsV2View';
import { ProviderSettings } from './components/settings/ProviderSettings';
import { ThemeToggle } from './components/ThemeToggle';
import { RelayLogo } from './components/common/RelayLogo';
import { ChangelogModal } from './components/common/ChangelogModal';
import { WelcomeModal } from './components/common/WelcomeModal';
import { AccountExplanationModal } from './components/common/AccountExplanationModal';
import { ProcessedPipelineResult, RelayAccount, RelayProfile, DeveloperSettings, AppSettings } from './types';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { getCurrentWindow } from '@tauri-apps/api/window';
import {
  isPermissionGranted,
  requestPermission,
} from '@tauri-apps/plugin-notification';
import { NativeSidebar } from './components/common/NativeSidebar';
import {
  Mic,
  Calendar,
  Sparkles,
  Settings,
  Sidebar as SidebarIcon,
  ChevronRight,
} from 'lucide-react';
import { Badge } from '@/components/ui/badge';
import { Button } from '@/components/ui/button';

export type MainTabType =
  | 'capture'
  | 'meetings'
  | 'scribble'
  | 'settings';

const TAB_LABELS: Record<MainTabType, string> = {
  capture: 'Voice Note',
  meetings: 'Meetings',
  scribble: 'Scribbles',
  settings: 'Settings',
};

export const App: React.FC = () => {
  const [activeTab, setActiveTab] = useState<MainTabType>('capture');
  const [lastResult, setLastResult] = useState<ProcessedPipelineResult | null>(null);
  const [sidebarOpen, setSidebarOpen] = useState(false);
  const [changelogOpen, setChangelogOpen] = useState(false);
  const [appVersion, setAppVersion] = useState<string>('0.9.0');
  const [account, setAccount] = useState<RelayAccount | null>(null);
  const [profile, setProfile] = useState<RelayProfile | null>(null);
  const [welcomeOpen, setWelcomeOpen] = useState(false);
  const [explanationOpen, setExplanationOpen] = useState(false);

  const refreshAccountAndSettings = async () => {
    try {
      const [ver, acc, prof, devSetts] = await Promise.all([
        invoke<string>('get_app_version'),
        invoke<RelayAccount>('get_account_state'),
        invoke<RelayProfile>('get_relay_profile'),
        invoke<DeveloperSettings>('get_developer_settings'),
      ]);
      if (ver) setAppVersion(ver);
      if (acc) setAccount(acc);
      if (prof) setProfile(prof);

      // Onboarding visibility: developer override forces replay, or first-run incomplete
      const shouldShowOnboarding = devSetts?.force_onboarding_on_launch || !prof?.onboarding_completed;
      if (shouldShowOnboarding) {
        setWelcomeOpen(true);
      }
    } catch (err) {
      console.warn('Could not load initial profile/settings:', err);
    }
  };

  useEffect(() => {
    refreshAccountAndSettings();

    // 1. Check Native OS Notification Permissions
    // Note: On desktop (Windows), permission is granted unconditionally, and OS toasts
    // are used as a display-only fallback signal. Interactive controls live in the
    // app-owned meeting-reminder overlay window.
    const setupNotifications = async () => {
      try {
        let granted = await isPermissionGranted();
        if (!granted) {
          const permission = await requestPermission();
          granted = permission === 'granted';
        }
        console.info('[notifications] Native OS notification permission status:', granted ? 'granted' : 'denied');
      } catch (err) {
        console.error('[notifications] Failed to initialize notification permissions:', err);
      }
    };

    setupNotifications();

    // 2. Listen for backend Tauri account & profile events
    const unlistenAccount = listen<RelayAccount>('account-changed', (event) => {
      if (event.payload) {
        setAccount(event.payload);
      }
    });

    const unlistenProfile = listen<RelayProfile>('profile-changed', (event) => {
      if (event.payload) {
        setProfile(event.payload);
      }
    });

    // 3. Listen for DOM custom events
    const handleDomAccountChange = (e: Event) => {
      const customEvent = e as CustomEvent<RelayAccount>;
      if (customEvent.detail) {
        setAccount(customEvent.detail);
      }
    };

    const handleDomProfileChange = (e: Event) => {
      const customEvent = e as CustomEvent<RelayProfile>;
      if (customEvent.detail) {
        setProfile(customEvent.detail);
      }
    };

    window.addEventListener('relay-account-changed', handleDomAccountChange);
    window.addEventListener('relay-profile-changed', handleDomProfileChange);

    return () => {
      unlistenAccount.then((unlisten) => unlisten());
      unlistenProfile.then((unlisten) => unlisten());
      window.removeEventListener('relay-account-changed', handleDomAccountChange);
      window.removeEventListener('relay-profile-changed', handleDomProfileChange);
    };
  }, []);

  const handleWelcomeGoogle = async (displayName: string) => {
    try {
      await invoke('update_profile_display_name', { displayName });
      const acc = await invoke<RelayAccount>('start_google_sign_in');
      const updatedProfile = await invoke<RelayProfile>('complete_profile_onboarding', {
        displayName,
        accountMode: 'local',
      });
      setProfile(updatedProfile);
      setAccount(acc);
      setWelcomeOpen(false);
      setExplanationOpen(true);
    } catch (err) {
      console.error('Failed to complete Google onboarding:', err);
      throw err;
    }
  };

  const handleWelcomeLocally = async (displayName: string) => {
    try {
      const updatedProfile = await invoke<RelayProfile>('complete_profile_onboarding', {
        displayName,
        accountMode: 'local',
      });
      setProfile(updatedProfile);
      setWelcomeOpen(false);
    } catch (err) {
      console.error('Failed to complete local onboarding:', err);
      throw err;
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
        return null; // MeetingsV2View has its own self-contained top bar
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
      {/* Navigation Sidebar (sidebar-07 icon-collapsible pattern) */}
      <NativeSidebar
        isOpen={sidebarOpen}
        onToggle={() => setSidebarOpen(!sidebarOpen)}
        activeTab={activeTab}
        setActiveTab={setActiveTab}
        account={account}
        profile={profile}
        appVersion={appVersion}
        onOpenChangelog={() => setChangelogOpen(true)}
        onOpenWelcome={() => setWelcomeOpen(true)}
        onOpenExplanation={() => setExplanationOpen(true)}
      />

      {/* Welcome First-Launch Onboarding Modal */}
      <WelcomeModal
        isOpen={welcomeOpen}
        initialDisplayName={profile?.display_name && profile.display_name !== 'Local User' ? profile.display_name : ''}
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

          {activeTab === 'meetings' && <MeetingsV2View />}

          {activeTab === 'scribble' && <ScribbleViewer />}

          {activeTab === 'settings' && <ProviderSettings />}
        </main>
      </div>
    </div>
  );
};
