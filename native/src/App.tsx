import React, { useState, useEffect } from 'react';
import { HomePage } from './components/home/HomePage';
import { VoiceNotePage } from './components/voicenotes/VoiceNotePage';
import { ScribbleViewer } from './components/scribble/ScribbleViewer';
import { MeetingsV2View } from './components/meetings_v2/MeetingsV2View';
import { TalkbackPage } from './components/talkback/TalkbackPage';
import { FilesPage } from './components/files/FilesPage';
import { CapturesPage } from './components/captures/CapturesPage';
import { KnowledgeGraphPage } from './components/knowledge/KnowledgeGraphPage';

import { ProviderSettings, type SettingsSection } from './components/settings/ProviderSettings';
import { DiagnosticsPage } from './components/diagnostics/DiagnosticsPage';
import { ThemeToggle } from './components/ThemeToggle';
import { ChangelogModal } from './components/common/ChangelogModal';
import { WelcomeModal } from './components/common/WelcomeModal';
import { AccountExplanationModal } from './components/common/AccountExplanationModal';
import { RelayAccount, RelayProfile, DeveloperSettings, AppSettings, MainTabType } from './types';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import {
  isPermissionGranted,
  requestPermission,
} from '@tauri-apps/plugin-notification';
import { NativeSidebar } from './components/common/NativeSidebar';
import {
  Mic,
  MessageCircle,
  Sparkles,
  FileText,
  Settings,
  Sidebar as SidebarIcon,
  ChevronRight,
  Activity,
  Globe,
  Home,
  Network,
} from 'lucide-react';
import { Button } from '@/components/ui/button';
import { PageHeader } from './components/common/PageHeader';
import type { CaptureMethod } from './components/captures/CaptureHubPage';

export type { MainTabType };

const TAB_LABELS: Record<MainTabType, string> = {
  home: 'Home',
  capture: 'Voice Note',
  meetings: 'Meetings',
  scribble: 'Scribbles',
  graph: 'Knowledge Graph',
  files: 'Files',
  captures: 'Captures',
  talkback: 'Talkback',
  diagnostics: 'Diagnostics',
  settings: 'Settings',
};

export const App: React.FC = () => {
  const [activeTab, setActiveTab] = useState<MainTabType>('home');
  const [settingsSection, setSettingsSection] = useState<SettingsSection | undefined>(undefined);
  /**
   * A capture mode requested from Home. Captures opens straight onto it, and it
   * is cleared by any other navigation so the Capture tab is not sticky.
   */
  const [captureMethod, setCaptureMethod] = useState<CaptureMethod | null>(null);
  /** A scribble the Knowledge Graph asked the workspace to reveal. */
  const [focusScribbleId, setFocusScribbleId] = useState<string | null>(null);
  const [sidebarOpen, setSidebarOpen] = useState(false);
  const [changelogOpen, setChangelogOpen] = useState(false);
  const [appVersion, setAppVersion] = useState<string>('0.9.0');
  const [account, setAccount] = useState<RelayAccount | null>(null);
  const [profile, setProfile] = useState<RelayProfile | null>(null);
  const [settings, setSettings] = useState<AppSettings | null>(null);
  const [welcomeOpen, setWelcomeOpen] = useState(false);
  const [explanationOpen, setExplanationOpen] = useState(false);



  /**
   * Every tab change goes through here.
   *
   * A one-shot request from another surface — a capture mode from a Home card, a
   * scribble the graph wants revealed — is cleared on the next navigation, so
   * arriving at a surface from the sidebar never lands on someone else's intent.
   */
  const navigateTo = (
    tab: MainTabType,
    intent: {
      captureMethod?: CaptureMethod;
      focusScribbleId?: string;
      section?: SettingsSection;
    } = {},
  ) => {
    setActiveTab(tab);
    setCaptureMethod(intent.captureMethod ?? null);
    setFocusScribbleId(intent.focusScribbleId ?? null);
    if (tab === 'settings') {
      setSettingsSection(intent.section);
    }
  };

  const refreshAccountAndSettings = async () => {
    try {
      const [ver, acc, prof, devSetts, appSetts] = await Promise.all([
        invoke<string>('get_app_version'),
        invoke<RelayAccount>('get_account_state'),
        invoke<RelayProfile>('get_relay_profile'),
        invoke<DeveloperSettings>('get_developer_settings'),
        invoke<AppSettings>('get_settings'),
      ]);
      if (ver) setAppVersion(ver);
      if (acc) setAccount(acc);
      if (prof) setProfile(prof);
      if (appSetts) setSettings(appSetts);

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

    // 2. Listen for backend Tauri account, profile, settings, & navigation events
    const handleNavigate = (payload: unknown) => {
      if (typeof payload === 'string') {
        if (payload in TAB_LABELS) {
          setActiveTab(payload as MainTabType);
          setCaptureMethod(null);
          setFocusScribbleId(null);
          if (payload === 'settings') {
            setSettingsSection(undefined);
          }
        }
      } else if (payload && typeof payload === 'object') {
        const obj = payload as { tab?: MainTabType; section?: SettingsSection };
        if (obj.tab && obj.tab in TAB_LABELS) {
          setActiveTab(obj.tab);
          setCaptureMethod(null);
          setFocusScribbleId(null);
        }
        if (obj.section) {
          setSettingsSection(obj.section);
        }
      }
    };

    const unlistenNavigate = listen<unknown>('navigate-tab', (event) => {
      if (event.payload) {
        handleNavigate(event.payload);
      }
    });

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

    const unlistenSettings = listen<AppSettings>('settings-changed', (event) => {
      if (event.payload) {
        setSettings(event.payload);
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

    const handleDomNavigate = (e: Event) => {
      const customEvent = e as CustomEvent<unknown>;
      if (customEvent.detail) {
        handleNavigate(customEvent.detail);
      }
    };

    window.addEventListener('relay-account-changed', handleDomAccountChange);
    window.addEventListener('relay-profile-changed', handleDomProfileChange);
    window.addEventListener('relay-navigate-tab', handleDomNavigate);

    return () => {
      unlistenNavigate.then((unlisten) => unlisten());
      unlistenAccount.then((unlisten) => unlisten());
      unlistenProfile.then((unlisten) => unlisten());
      unlistenSettings.then((unlisten) => unlisten());
      window.removeEventListener('relay-account-changed', handleDomAccountChange);
      window.removeEventListener('relay-profile-changed', handleDomProfileChange);
      window.removeEventListener('relay-navigate-tab', handleDomNavigate);
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
      case 'home': {
        const name = profile?.display_name && profile.display_name !== 'Local User'
          ? profile.display_name.split(' ')[0]
          : null;
        return (
          <PageHeader
            badge={{ label: 'Home', icon: Home, variant: 'emerald' }}
            title={name ? 'Welcome back,' : 'Everything Relay'}
            highlightText={name ? `${name}.` : 'has captured.'}
            description="Start a capture, or pick up what you already said. Every count below is read from your local vault."
            glowColor="emerald"
          />
        );
      }
      case 'capture':
        return (
          <PageHeader
            badge={{ label: 'Capture Surface', icon: Mic, variant: 'emerald' }}
            title="Voice"
            highlightText="Notes"
            description="Everything you dictate, captured in one truthful history."
            glowColor="emerald"
          />
        );
      case 'meetings':
        return (
          <PageHeader
            badge={{ label: 'Meeting Intelligence', icon: Mic, variant: 'purple' }}
            title="Crash-resilient"
            highlightText="transcripts & memory."
            description="Dual microphone and system audio capture with 30-second incremental persistence and AI extraction."
            glowColor="purple"
          />
        );
      case 'scribble':
        return (
          <PageHeader
            badge={{ label: 'Knowledge Layer', icon: Sparkles, variant: 'default' }}
            title="Connected thoughts,"
            highlightText="living knowledge."
            description="Every atomic thought Relay holds, with the ideas it connects to and the source it came from. New thoughts are captured on the Captures surface."
            glowColor="primary"
          />
        );
      case 'graph':
        return (
          <PageHeader
            badge={{ label: 'Knowledge Layer', icon: Network, variant: 'default' }}
            title="How everything"
            highlightText="connects."
            description="Scribbles, topics, entities and sources as one Obsidian-compatible graph. Drag to rearrange, double-click a thought to open it in Scribbles."
            glowColor="primary"
          />
        );
      case 'captures':
        return (
          <PageHeader
            badge={{ label: 'Capture Surface', icon: Globe, variant: 'default' }}
            title="Everything you capture,"
            highlightText="as text you own."
            description="Type or paste a thought, or open a page the browser extension sent here. Captured pages are stored as external source material with their provenance — never as instructions to Relay's AI."
            glowColor="primary"
          />
        );
      case 'files':
        return (
          <PageHeader
            badge={{ label: 'Document Vault', icon: FileText, variant: 'default' }}
            title="Imported"
            highlightText="documents & knowledge."
            description="Bring PDF, Word, Markdown and Text files into Relay without touching your original files. Summarize, enrich, and ask Talkback questions about them."
            glowColor="primary"
          />
        );
      case 'talkback':
        return (
          <PageHeader
            badge={{ label: 'Conversational Layer', icon: MessageCircle, variant: 'emerald' }}
            title="Think with"
            highlightText="what Relay knows."
            description="Ask about your own Voice Notes, Scribbles and Meetings out loud. Answers about your history come from your own capture, with the sources shown."
            glowColor="emerald"
          />
        );
      case 'diagnostics':
        return (
          <PageHeader
            badge={{ label: 'System Observability', icon: Activity, variant: 'purple' }}
            title="Inspect & test"
            highlightText="Relay's engines."
            description="Real-time telemetry, audio & VAD inspection, speech-to-text accuracy tests, and LLM latency benchmarks."
            glowColor="purple"
          />
        );
      case 'settings':
        return (
          <PageHeader
            badge={{ label: 'Preferences & Vault', icon: Settings, variant: 'purple' }}
            title="How Relay"
            highlightText="behaves."
            description="Configure local LLMs, triggers, privacy bounds, and manage 30-day trash recovery."
            glowColor="purple"
          />
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
        setActiveTab={(tab) => navigateTo(tab)}
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
        <main className="flex-1 min-w-0 p-4 md:p-6 overflow-y-auto flex flex-col bg-background">
          {renderHeroHeader()}

          {activeTab === 'home' && (
            <HomePage
              account={account}
              settings={settings}
              appVersion={appVersion}
              onNavigate={(tab) => navigateTo(tab)}
              onStartCapture={(method) => navigateTo('captures', { captureMethod: method })}
              onOpenSettings={(section) => navigateTo('settings', { section })}
              onOpenChangelog={() => setChangelogOpen(true)}
            />
          )}

          {activeTab === 'capture' && <VoiceNotePage />}

          {activeTab === 'meetings' && <MeetingsV2View />}

          {activeTab === 'scribble' && (
            <ScribbleViewer
              focusScribbleId={focusScribbleId}
              onStartCapture={() => navigateTo('captures', { captureMethod: 'text' })}
            />
          )}

          {activeTab === 'graph' && (
            <KnowledgeGraphPage
              onOpenScribble={(id) => navigateTo('scribble', { focusScribbleId: id })}
            />
          )}

          {activeTab === 'files' && <FilesPage onNavigateTab={(tab) => navigateTo(tab)} />}

          {activeTab === 'captures' && (
            <CapturesPage
              initialCaptureMethod={captureMethod}
              onNavigateTab={(tab) => navigateTo(tab)}
              onOpenCaptureSettings={() => navigateTo('settings', { section: 'capture' })}
              onOpenScribble={(id) => navigateTo('scribble', { focusScribbleId: id })}
            />
          )}

          {activeTab === 'talkback' && <TalkbackPage />}

          {activeTab === 'diagnostics' && (
            <DiagnosticsPage onNavigateTab={(tab) => navigateTo(tab)} />
          )}

          {activeTab === 'settings' && (
            <ProviderSettings
              initialSection={settingsSection}
              onNavigateTab={(tab) => navigateTo(tab)}
            />
          )}
        </main>
      </div>
    </div>
  );
};
