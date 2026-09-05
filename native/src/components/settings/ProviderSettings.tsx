import React, { useEffect, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import {
  AppSettings,
  LanguageSettings,
  MainTabType,
  VaultLocationInfo,
  RelayAccount,
  AudioDeviceInfo,
  OllamaModelDetails,
  SttModelsOverview,
} from '../../types';
import {
  Cpu,
  Cloud,
  CheckCircle,
  Sliders,
  ShieldCheck,
  HardDrive,
  User,
  Trash2,
  Download,
  AlertTriangle,
  AlertCircle,
  RefreshCw,
  Mic,
  Keyboard,
  Globe,
  Languages,
  Sparkles,
  BookOpen,
  Users,
  Volume2,
  Terminal,
  Check,
  Layers,
  Power,
  Clipboard,
  MessageCircle,
  Activity,
  ChevronDown,
  ChevronUp,
  type LucideIcon,
} from 'lucide-react';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { Badge } from '@/components/ui/badge';
import { Switch } from '@/components/ui/switch';
import { HotkeyRecorder } from './HotkeyRecorder';
import { TrashSettings } from './TrashSettings';
import { AccountSettings } from './AccountSettings';
import { DeveloperSettingsView } from './DeveloperSettingsView';
import { DictionarySnippetsSettings } from './DictionarySnippetsSettings';
import { CaptureSettingsView } from './CaptureSettingsView';
import { MeetingsSettings } from './MeetingsSettings';
import { TalkbackSettingsView, DEFAULT_TALKBACK_SETTINGS } from './TalkbackSettingsView';

export type SettingsSection =
  | 'account'
  | 'general'
  | 'dictation'
  | 'dictionary'
  | 'meetings'
  | 'capture'
  | 'talkback'
  | 'languages'
  | 'advanced'
  | 'privacy'
  | 'trash'
  | 'developer';

interface SettingsNavItem {
  id: SettingsSection;
  label: string;
  icon: LucideIcon;
  /** Non-default tint, for the two sections that are not everyday preferences. */
  accent?: string;
}

/**
 * The settings sections, in the order they are shown.
 *
 * One list rather than twelve near-identical buttons: the hand-written version
 * had drifted — its numbering ran 0,1,2,3,4,5,6,6,5,6,7,8 and two unrelated
 * sections shared the same icon, which is exactly the drift a list cannot have.
 * `capture` is labelled "Web Capture" because `Captures` is now a whole surface
 * with several modes, and only the browser bridge is configured here.
 */
const SETTINGS_NAV: SettingsNavItem[] = [
  { id: 'account', label: 'Account & Identity', icon: User },
  { id: 'general', label: 'General', icon: Sliders },
  { id: 'dictation', label: 'Dictation & Audio', icon: Mic },
  { id: 'dictionary', label: 'Dictionary & Snippets', icon: BookOpen },
  { id: 'meetings', label: 'Meetings', icon: Users },
  { id: 'capture', label: 'Web Capture', icon: Globe },
  { id: 'talkback', label: 'Talkback', icon: MessageCircle },
  { id: 'languages', label: 'Languages & Script', icon: Languages },
  { id: 'advanced', label: 'AI Models & STT', icon: Cpu },
  { id: 'privacy', label: 'Privacy & Vault', icon: ShieldCheck },
  { id: 'trash', label: 'Trash & Deleted', icon: Trash2, accent: 'text-amber-500' },
  { id: 'developer', label: 'Developer', icon: Terminal, accent: 'text-amber-500' },
];

const DEFAULT_LANGUAGE_SETTINGS: LanguageSettings = {
  primary_dictation_language: 'en',
  spoken_languages: ['en'],
  notes_language: 'en',
  output_script: 'latin',
};

const WHISPER_SUPPORTED_LANGUAGES = [
  { code: 'auto', name: 'Auto-detect / Multilingual (Hinglish)' },
  { code: 'en', name: 'English' },
  { code: 'hi', name: 'Hindi' },
  { code: 'kn', name: 'Kannada' },
  { code: 'ta', name: 'Tamil' },
  { code: 'te', name: 'Telugu' },
  { code: 'mr', name: 'Marathi' },
  { code: 'bn', name: 'Bengali' },
  { code: 'gu', name: 'Gujarati' },
  { code: 'ml', name: 'Malayalam' },
  { code: 'pa', name: 'Punjabi' },
  { code: 'ur', name: 'Urdu' },
  { code: 'es', name: 'Spanish' },
  { code: 'fr', name: 'French' },
  { code: 'de', name: 'German' },
  { code: 'it', name: 'Italian' },
  { code: 'pt', name: 'Portuguese' },
  { code: 'ru', name: 'Russian' },
  { code: 'ja', name: 'Japanese' },
  { code: 'zh', name: 'Chinese (Mandarin)' },
  { code: 'ko', name: 'Korean' },
  { code: 'ar', name: 'Arabic' },
  { code: 'nl', name: 'Dutch' },
  { code: 'tr', name: 'Turkish' },
  { code: 'vi', name: 'Vietnamese' },
  { code: 'id', name: 'Indonesian' },
  { code: 'pl', name: 'Polish' },
  { code: 'uk', name: 'Ukrainian' },
  { code: 'sv', name: 'Swedish' },
];

const DEFAULT_SETTINGS: AppSettings = {
  provider: {
    active_provider: 'ollama',
    ollama_host: 'http://localhost:11434',
    ollama_model: 'llama3.2:latest',
    cloud_model: 'gpt-4o-mini',
  },
  stt: { whisper_model_path: '' },
  tts: { piper_binary_path: '', piper_voice_path: '' },
  hotkeys: {
    show_hide_hotkey: 'Ctrl+Shift+Space',
    dictation_hotkey: 'Ctrl+Space',
    toggle_to_talk: false,
    capture_hotkey: 'Ctrl+Shift+C',
  },
  ui: { pill_position: 'bottom_center' },
  vault: { directory: null },
  language: DEFAULT_LANGUAGE_SETTINGS,
  diagnostics: {
    allow_anonymous_diagnostics: true,
    first_run_completed: false,
  },
  sound: {
    dictation_sounds: true,
  },
  clipboard: {
    auto_paste: true,
    copy_to_clipboard: true,
  },
  startup: {
    launch_at_login: false,
    start_minimized: false,
  },
  audio_input: {
    prefer_builtin_mic: true,
    selected_device: null,
    keep_microphone_warm: 'off',
    auto_learn_words: true,
  },
  talkback: DEFAULT_TALKBACK_SETTINGS,
  dictionary: ['Relay', 'Whisper', 'Tauri', 'Rust', 'Supabase', 'LanceDB', 'Ollama'],
  snippets: [],
};

interface ProviderSettingsProps {
  initialSection?: SettingsSection;
  onNavigateTab?: (tab: MainTabType) => void;
}

export const ProviderSettings: React.FC<ProviderSettingsProps> = ({
  initialSection = 'general',
  onNavigateTab,
}) => {
  const [activeSection, setActiveSection] = useState<SettingsSection>(initialSection);

  useEffect(() => {
    if (initialSection) {
      setActiveSection(initialSection);
    }
  }, [initialSection]);
  const [settings, setSettings] = useState<AppSettings>(DEFAULT_SETTINGS);
  const [loading, setLoading] = useState(true);
  const [saved, setSaved] = useState(false);
  const [error, setError] = useState('');

  // Audio input devices
  const [audioDevices, setAudioDevices] = useState<AudioDeviceInfo[]>([]);
  const [loadingDevices, setLoadingDevices] = useState(false);

  type OllamaStatus =
    | { state: 'checking' }
    | { state: 'running' }
    | { state: 'started' }
    | { state: 'not_installed' }
    | { state: 'unreachable'; message: string };
  const [ollamaStatus, setOllamaStatus] = useState<OllamaStatus>({ state: 'checking' });

  // LLM Model discovery state
  const [ollamaModels, setOllamaModels] = useState<OllamaModelDetails[]>([]);
  const [loadingOllamaModels, setLoadingOllamaModels] = useState(false);
  const [customLlmMode, setCustomLlmMode] = useState(false);

  // STT Model discovery state
  const [sttOverview, setSttOverview] = useState<SttModelsOverview | null>(null);
  const [loadingSttModels, setLoadingSttModels] = useState(false);
  const [customSttMode, setCustomSttMode] = useState(false);

  const checkLocalLlm = async (overrideHost?: string) => {
    setOllamaStatus({ state: 'checking' });
    try {
      const status = await invoke<OllamaStatus>('ensure_local_llm_ready');
      setOllamaStatus(status);
      await fetchOllamaModels(overrideHost);
    } catch (err) {
      console.error('Failed to check local Ollama status', err);
      setOllamaStatus({ state: 'unreachable', message: 'Could not reach the backend' });
    }
  };

  const fetchOllamaModels = async (host?: string) => {
    setLoadingOllamaModels(true);
    try {
      const models = await invoke<OllamaModelDetails[]>('get_available_llm_models', {
        host: host || settings.provider.ollama_host || null,
      });
      setOllamaModels(models || []);
    } catch (err) {
      setOllamaModels([]);
    } finally {
      setLoadingOllamaModels(false);
    }
  };

  const fetchSttModels = async () => {
    setLoadingSttModels(true);
    try {
      const overview = await invoke<SttModelsOverview>('get_available_stt_models');
      setSttOverview(overview);
    } catch (err) {
      console.error('Failed to query STT models', err);
    } finally {
      setLoadingSttModels(false);
    }
  };

  type SttModelStatus =
    | { state: 'checking' }
    | { state: 'ready'; path: string }
    | { state: 'failed'; message: string };
  const [sttModelStatus, setSttModelStatus] = useState<SttModelStatus>({ state: 'checking' });

  const checkSttModel = async () => {
    setSttModelStatus({ state: 'checking' });
    try {
      const status = await invoke<SttModelStatus>('ensure_stt_model_ready');
      setSttModelStatus(status);
      if (status.state === 'ready') {
        setSettings((prev) => ({ ...prev, stt: { ...prev.stt, whisper_model_path: status.path } }));
      }
      await fetchSttModels();
    } catch (err) {
      console.error('Failed to check local Whisper model status', err);
      setSttModelStatus({ state: 'failed', message: 'Could not reach the backend' });
    }
  };

  const loadAudioDevices = async () => {
    setLoadingDevices(true);
    try {
      const devs = await invoke<AudioDeviceInfo[]>('get_audio_devices');
      setAudioDevices(devs || []);
    } catch (err) {
      console.error('Failed to query audio devices', err);
    } finally {
      setLoadingDevices(false);
    }
  };

  const [vaultLocation, setVaultLocation] = useState<VaultLocationInfo | null>(null);
  const [vaultBusy, setVaultBusy] = useState(false);
  const [vaultError, setVaultError] = useState('');

  // Relay Account state for Privacy & Destructive controls
  const [account, setAccount] = useState<RelayAccount | null>(null);
  const [deleteAccountModalOpen, setDeleteAccountModalOpen] = useState(false);
  const [deleteAccountAck, setDeleteAccountAck] = useState(false);
  const [deleteAccountInput, setDeleteAccountInput] = useState('');
  const [deletingAccount, setDeletingAccount] = useState(false);
  const [deleteAccountSuccess, setDeleteAccountSuccess] = useState<string | null>(null);
  const [deleteAccountError, setDeleteAccountError] = useState<string | null>(null);

  // Clear Vault double confirmation state
  const [clearVaultModalOpen, setClearVaultModalOpen] = useState(false);
  const [clearVaultAck, setClearVaultAck] = useState(false);
  const [clearVaultInput, setClearVaultInput] = useState('');
  const [clearingVault, setClearingVault] = useState(false);

  const loadAccountState = async () => {
    try {
      const acc = await invoke<RelayAccount>('get_account_state');
      setAccount(acc);
    } catch (err) {
      console.error('Failed to read account state for privacy controls', err);
    }
  };

  const handleDeleteAccount = async () => {
    if (!deleteAccountAck || deleteAccountInput.trim().toUpperCase() !== 'DELETE ACCOUNT') {
      return;
    }
    try {
      setDeletingAccount(true);
      setDeleteAccountError(null);
      const updated = await invoke<RelayAccount>('delete_relay_account');
      setAccount(updated);
      window.dispatchEvent(new CustomEvent('relay-account-changed', { detail: updated }));
      setDeleteAccountSuccess('Relay Cloud Account was deleted. All local markdown notes, scribbles, audio, and vectors remain 100% untouched.');
      setDeleteAccountModalOpen(false);
      setDeleteAccountAck(false);
      setDeleteAccountInput('');
      setTimeout(() => setDeleteAccountSuccess(null), 7000);
    } catch (err: unknown) {
      console.error('Failed to delete account:', err);
      const msg = typeof err === 'string' ? err : (err as { message?: string })?.message || 'Failed to delete account.';
      setDeleteAccountError(msg);
    } finally {
      setDeletingAccount(false);
    }
  };

  const handleDisconnectSync = async () => {
    try {
      const updated = await invoke<RelayAccount>('sign_out_account');
      setAccount(updated);
      window.dispatchEvent(new CustomEvent('relay-account-changed', { detail: updated }));
    } catch (err) {
      console.error('Failed to disconnect sync:', err);
    }
  };

  const loadVaultLocation = async () => {
    try {
      setVaultLocation(await invoke<VaultLocationInfo>('get_vault_location'));
    } catch (err) {
      console.error('Failed to read Vault Directory Location', err);
      setVaultError('Could not determine where the vault is stored');
    }
  };

  const handleChooseVaultFolder = async () => {
    setVaultBusy(true);
    setVaultError('');
    try {
      const picked = await invoke<string | null>('choose_vault_folder');
      if (!picked) return;
      setVaultLocation(await invoke<VaultLocationInfo>('set_vault_location', { path: picked }));
    } catch (err: any) {
      console.error('Failed to set Vault Directory Location', err);
      setVaultError(err?.message || "Couldn't use that folder — choose another.");
    } finally {
      setVaultBusy(false);
    }
  };

  useEffect(() => {
    invoke<AppSettings>('get_settings')
      .then((loaded) => {
        setSettings({
          ...DEFAULT_SETTINGS,
          ...loaded,
          clipboard: {
            ...DEFAULT_SETTINGS.clipboard!,
            ...(loaded.clipboard || {}),
          },
          startup: {
            ...DEFAULT_SETTINGS.startup!,
            ...(loaded.startup || {}),
          },
          audio_input: {
            ...DEFAULT_SETTINGS.audio_input!,
            ...(loaded.audio_input || {}),
          },
          sound: {
            ...DEFAULT_SETTINGS.sound!,
            ...(loaded.sound || {}),
          },
          language: {
            ...DEFAULT_LANGUAGE_SETTINGS,
            ...(loaded.language || {}),
            spoken_languages:
              loaded.language?.spoken_languages && loaded.language.spoken_languages.length > 0
                ? loaded.language.spoken_languages
                : (loaded.language as any)?.spokenLanguages || DEFAULT_LANGUAGE_SETTINGS.spoken_languages,
            primary_dictation_language:
              loaded.language?.primary_dictation_language ||
              (loaded.language as any)?.primaryDictationLanguage ||
              DEFAULT_LANGUAGE_SETTINGS.primary_dictation_language,
            notes_language:
              loaded.language?.notes_language ||
              (loaded.language as any)?.notesLanguage ||
              DEFAULT_LANGUAGE_SETTINGS.notes_language,
            output_script:
              loaded.language?.output_script ||
              (loaded.language as any)?.outputScript ||
              DEFAULT_LANGUAGE_SETTINGS.output_script,
          },
        });
      })
      .catch((err) => {
        console.error('Failed to load settings', err);
        setError('Could not load saved settings — showing defaults');
      })
      .finally(() => setLoading(false));

    const unlistenPromise = listen<AppSettings>('settings-changed', ({ payload }) => {
      if (payload) {
        setSettings((prev) => ({
          ...prev,
          ...payload,
        }));
      }
    });

    return () => {
      unlistenPromise.then((unlisten) => unlisten());
    };
  }, []);

  useEffect(() => {
    if (!loading && activeSection === 'general') {
      loadVaultLocation();
    }
    if (!loading && activeSection === 'advanced') {
      if (settings.provider.active_provider === 'ollama') {
        checkLocalLlm();
        fetchOllamaModels();
      }
      checkSttModel();
      fetchSttModels();
    }
    if (!loading && activeSection === 'privacy') {
      loadAccountState();
    }
  }, [loading, activeSection, settings.provider.active_provider]);

  const applyHotkey = async (field: 'show_hide_hotkey' | 'dictation_hotkey', accelerator: string) => {
    const updatedHotkeys = { ...settings.hotkeys, [field]: accelerator };
    const updatedSettings = { ...settings, hotkeys: updatedHotkeys };
    setSettings(updatedSettings);
    try {
      await invoke('update_hotkeys', { hotkeys: updatedHotkeys });
      setError('');
    } catch (err: any) {
      console.error('Failed to apply hotkey', err);
      setError(err?.message || 'Failed to apply hotkey — it may already be in use by another app');
    }
  };



  const handleSaveDirect = async () => {
    try {
      await invoke('save_settings', { settings });
      setSaved(true);
      setError('');
      setTimeout(() => setSaved(false), 2000);
      fetchSttModels();
      if (settings.provider.active_provider === 'ollama') {
        fetchOllamaModels();
      }
    } catch (err) {
      console.error('Failed to save settings', err);
      setError('Failed to save settings');
    }
  };

  const handleSave = async (e: React.FormEvent) => {
    e.preventDefault();
    await handleSaveDirect();
  };

  // Find default or active microphone
  const defaultDevice = audioDevices.find((d) => d.is_default) || audioDevices[0];
  const activeDeviceName = settings.audio_input?.selected_device || defaultDevice?.name || 'Default Microphone Array';

  if (loading) {
    return (
      <div className="flex-1 flex items-center justify-center bg-card rounded-lg border border-border text-xs text-muted-foreground">
        Loading settings…
      </div>
    );
  }

  return (
    <div className="flex-1 flex flex-col md:flex-row gap-6 min-h-0 overflow-hidden">
      {/* Settings Sub-Nav Sidebar */}
      <aside className="w-full md:w-56 flex flex-col shrink-0 gap-1 bg-card p-3 rounded-lg border border-border">
        <div className="px-3 py-2 mb-1">
          <span className="font-mono text-[10px] font-bold tracking-widest text-muted-foreground uppercase">
            SETTINGS
          </span>
        </div>

        {SETTINGS_NAV.map((item) => {
          const Icon = item.icon;
          const active = activeSection === item.id;
          return (
            <button
              key={item.id}
              type="button"
              onClick={() => setActiveSection(item.id)}
              aria-current={active ? 'page' : undefined}
              className={`flex items-center gap-2.5 px-3 py-2 rounded-lg text-xs font-medium transition-all text-left ${
                active
                  ? 'bg-accent text-accent-foreground font-semibold shadow-xs'
                  : 'text-muted-foreground hover:bg-muted hover:text-foreground'
              }`}
            >
              <Icon className={`w-4 h-4 ${item.accent ?? 'text-primary'}`} />
              <span>{item.label}</span>
            </button>
          );
        })}
      </aside>

      {/* Main Settings Content Area */}
      <main className="flex-1 bg-card rounded-lg border border-border p-6 overflow-y-auto min-h-0">
        {saved && (
          <div className="mb-4 p-3 rounded-lg bg-emerald-500/10 border border-emerald-500/30 text-emerald-600 dark:text-emerald-400 text-xs flex items-center justify-between">
            <span className="flex items-center gap-2">
              <CheckCircle className="w-4 h-4 text-emerald-500" />
              Settings updated successfully
            </span>
            <Badge variant="outline" className="text-[10px] font-mono border-emerald-500/30 text-emerald-500">
              Persisted
            </Badge>
          </div>
        )}

        {error && <p className="mb-4 text-xs text-amber-500">{error}</p>}

        {/* 0. ACCOUNT & IDENTITY SECTION */}
        {activeSection === 'account' && (
          <AccountSettings
            settings={settings}
            onUpdateSettings={setSettings}
          />
        )}

        {/* 1. GENERAL SECTION */}
        {activeSection === 'general' && (
          <form onSubmit={handleSave} className="space-y-6">
            <div>
              <p className="font-mono text-[10px] font-bold text-muted-foreground uppercase tracking-widest mb-1">
                GENERAL CONFIGURATION
              </p>
              <h2 className="text-lg font-bold text-foreground">Desktop App & Startup Defaults</h2>
            </div>

            <div className="space-y-4">
              {/* Show/Hide Global Hotkey */}
              <div className="py-3 border-b border-border">
                <div className="flex items-center gap-2 mb-2">
                  <Keyboard className="w-4 h-4 text-primary" />
                  <p className="text-xs font-semibold text-foreground">Show/Hide Hotkey</p>
                </div>
                <div className="max-w-md">
                  <label htmlFor="show-hide-hotkey" className="block text-[11px] text-muted-foreground mb-1">
                    Show/Hide Relay window (anywhere in the OS)
                  </label>
                  <HotkeyRecorder
                    id="show-hide-hotkey"
                    value={settings.hotkeys.show_hide_hotkey}
                    onCapture={(accelerator) => applyHotkey('show_hide_hotkey', accelerator)}
                  />
                </div>
                <p className="text-[10px] text-muted-foreground mt-2">
                  Click the box, then press your desired key combination — it takes effect immediately.
                </p>
              </div>

              {/* Startup Group (OpenWhispr Style) */}
              <div className="py-3 border-b border-border space-y-3">
                <div className="flex items-center gap-2">
                  <Power className="w-4 h-4 text-primary" />
                  <div>
                    <p className="text-xs font-semibold text-foreground">Startup</p>
                    <p className="text-[11px] text-muted-foreground">Control how Relay behaves when it launches</p>
                  </div>
                </div>

                <div className="p-3.5 rounded-lg bg-muted/40 border border-border space-y-3">
                  <div className="flex items-center justify-between">
                    <div>
                      <p className="text-xs font-medium text-foreground">Launch at login</p>
                      <p className="text-[11px] text-muted-foreground">Start Relay in the background when you log in</p>
                    </div>
                    <Switch
                      checked={settings.startup?.launch_at_login ?? false}
                      onCheckedChange={async (checked) => {
                        const updated: AppSettings = {
                          ...settings,
                          startup: {
                            ...settings.startup,
                            launch_at_login: checked,
                            start_minimized: settings.startup?.start_minimized ?? false,
                          },
                        };
                        setSettings(updated);
                        try {
                          await invoke('save_settings', { settings: updated });
                        } catch (err) {
                          console.error('Failed to update launch at login', err);
                        }
                      }}
                    />
                  </div>

                  <div className="h-px bg-border/60" />

                  <div className="flex items-center justify-between">
                    <div>
                      <p className="text-xs font-medium text-foreground">Start minimized</p>
                      <p className="text-[11px] text-muted-foreground">Launch without showing the main control panel window</p>
                    </div>
                    <Switch
                      checked={settings.startup?.start_minimized ?? false}
                      onCheckedChange={async (checked) => {
                        const updated: AppSettings = {
                          ...settings,
                          startup: {
                            ...settings.startup,
                            launch_at_login: settings.startup?.launch_at_login ?? false,
                            start_minimized: checked,
                          },
                        };
                        setSettings(updated);
                        try {
                          await invoke('save_settings', { settings: updated });
                        } catch (err) {
                          console.error('Failed to update start minimized', err);
                        }
                      }}
                    />
                  </div>
                </div>
              </div>

              {/* Floating Pill Position */}
              <div className="py-3 border-b border-border">
                <p className="text-xs font-semibold text-foreground mb-1">Pill Screen Position</p>
                <p className="text-[11px] text-muted-foreground mb-2">
                  Which edge of your screen the floating dictation pill anchors to
                </p>
                <div className="flex bg-muted p-1 rounded-lg border border-border w-fit">
                  {(
                    [
                      { value: 'bottom_left', label: 'Bottom Left' },
                      { value: 'bottom_center', label: 'Bottom Center' },
                      { value: 'bottom_right', label: 'Bottom Right' },
                    ] as const
                  ).map((opt) => (
                    <button
                      key={opt.value}
                      type="button"
                      onClick={async () => {
                        const updated = { ...settings, ui: { ...settings.ui, pill_position: opt.value } };
                        setSettings(updated);
                        try {
                          await invoke('set_pill_position', { position: opt.value });
                        } catch (err) {
                          console.error('Failed to set pill position', err);
                        }
                      }}
                      className={`px-3 py-1 text-xs font-medium rounded-lg transition-all ${
                        settings.ui.pill_position === opt.value
                          ? 'bg-card text-foreground font-semibold shadow-xs'
                          : 'text-muted-foreground'
                      }`}
                    >
                      {opt.label}
                    </button>
                  ))}
                </div>
              </div>



              {/* Vault Directory Location */}
              <div className="py-3 flex items-center justify-between gap-3">
                <div className="min-w-0">
                  <div className="flex items-center gap-2 mb-1">
                    <HardDrive className="w-4 h-4 text-primary" />
                    <p className="text-xs font-semibold text-foreground">Vault Directory Location</p>
                  </div>
                  <p className="text-[11px] text-muted-foreground font-mono truncate">
                    {vaultLocation?.path || 'Loading…'}
                  </p>
                  {vaultLocation && !vaultLocation.configured && (
                    <p className="text-[10px] text-muted-foreground mt-0.5">Using the default location</p>
                  )}
                  {vaultError && <p className="text-[10px] text-destructive mt-0.5">{vaultError}</p>}
                </div>
                <div className="flex items-center gap-2 shrink-0">
                  {vaultLocation?.accessible === false && (
                    <Badge variant="outline" className="text-xs font-mono border-destructive/50 text-destructive">
                      Inaccessible
                    </Badge>
                  )}
                  <Button
                    type="button"
                    size="sm"
                    variant="outline"
                    className="text-xs h-7"
                    disabled={vaultBusy}
                    onClick={handleChooseVaultFolder}
                  >
                    Choose Folder
                  </Button>
                </div>
              </div>

              <Button type="submit" size="sm" variant="default" className="mt-2">
                Save General Settings
              </Button>
            </div>
          </form>
        )}

        {/* 2. DICTATION & AUDIO SECTION */}
        {activeSection === 'dictation' && (
          <form onSubmit={handleSave} className="space-y-6">
            <div>
              <p className="font-mono text-[10px] font-bold text-muted-foreground uppercase tracking-widest mb-1">
                UNIVERSAL DICTATION & AUDIO HARDWARE
              </p>
              <h2 className="text-lg font-bold text-foreground">Microphone, Clipboard & Sound Behavior</h2>
            </div>

            <div className="space-y-4">
              {/* Universal Dictation Hotkey */}
              <div className="py-3 border-b border-border">
                <div className="flex items-center gap-2 mb-2">
                  <Keyboard className="w-4 h-4 text-primary" />
                  <p className="text-xs font-semibold text-foreground">Universal Dictation Hotkey</p>
                </div>
                <div className="max-w-md">
                  <label htmlFor="dictation-hotkey" className="block text-[11px] text-muted-foreground mb-1">
                    Dictate anywhere (types directly into active focused field)
                  </label>
                  <HotkeyRecorder
                    id="dictation-hotkey"
                    value={settings.hotkeys.dictation_hotkey}
                    onCapture={(accelerator) => applyHotkey('dictation_hotkey', accelerator)}
                  />
                </div>
                <p className="text-[10px] text-muted-foreground mt-2">
                  Press and hold (or toggle) to speak into any text box across your operating system.
                </p>
              </div>



              {/* Toggle-to-Talk Switch */}
              <div className="py-3 border-b border-border flex items-center justify-between">
                <div>
                  <p className="text-xs font-semibold text-foreground">Toggle-to-Talk Mode</p>
                  <p className="text-[11px] text-muted-foreground">
                    Press dictation hotkey once to start recording, press again to finish — instead of holding the key.
                  </p>
                </div>
                <Switch
                  checked={settings.hotkeys.toggle_to_talk}
                  onCheckedChange={async (checked) => {
                    const updated = {
                      ...settings,
                      hotkeys: { ...settings.hotkeys, toggle_to_talk: checked },
                    };
                    setSettings(updated);
                    try {
                      await invoke('save_settings', { settings: updated });
                    } catch (err) {
                      console.error('Failed to toggle toggle-to-talk mode', err);
                    }
                  }}
                />
              </div>

              {/* Clipboard Group (OpenWhispr Style) */}
              <div className="py-3 border-b border-border space-y-3">
                <div className="flex items-center gap-2">
                  <Clipboard className="w-4 h-4 text-primary" />
                  <p className="text-xs font-semibold text-foreground">Clipboard</p>
                </div>
                <div className="p-3.5 rounded-lg bg-muted/40 border border-border space-y-3">
                  <div className="flex items-center justify-between">
                    <div>
                      <p className="text-xs font-medium text-foreground">Automatic pasting</p>
                      <p className="text-[11px] text-muted-foreground">
                        Automatically paste transcribed text into the active app when dictation finishes
                      </p>
                    </div>
                    <Switch
                      checked={settings.clipboard?.auto_paste ?? true}
                      onCheckedChange={async (checked) => {
                        const updated: AppSettings = {
                          ...settings,
                          clipboard: {
                            ...settings.clipboard,
                            auto_paste: checked,
                            copy_to_clipboard: settings.clipboard?.copy_to_clipboard ?? true,
                          },
                        };
                        setSettings(updated);
                        try {
                          await invoke('save_settings', { settings: updated });
                        } catch (err) {
                          console.error('Failed to update auto paste', err);
                        }
                      }}
                    />
                  </div>

                  <div className="h-px bg-border/60" />

                  <div className="flex items-center justify-between">
                    <div>
                      <p className="text-xs font-medium text-foreground">Keep transcription in clipboard</p>
                      <p className="text-[11px] text-muted-foreground">
                        Keep dictated text in your clipboard so you can paste it manually if needed
                      </p>
                    </div>
                    <Switch
                      checked={settings.clipboard?.copy_to_clipboard ?? true}
                      onCheckedChange={async (checked) => {
                        const updated: AppSettings = {
                          ...settings,
                          clipboard: {
                            ...settings.clipboard,
                            auto_paste: settings.clipboard?.auto_paste ?? true,
                            copy_to_clipboard: checked,
                          },
                        };
                        setSettings(updated);
                        try {
                          await invoke('save_settings', { settings: updated });
                        } catch (err) {
                          console.error('Failed to update copy to clipboard', err);
                        }
                      }}
                    />
                  </div>
                </div>
              </div>

              {/* Microphone Hardware & Warm-up (OpenWhispr Style) */}
              <div className="py-3 border-b border-border space-y-3">
                <div className="flex items-center gap-2">
                  <Mic className="w-4 h-4 text-primary" />
                  <div>
                    <p className="text-xs font-semibold text-foreground">Microphone</p>
                    <p className="text-[11px] text-muted-foreground">Select which input device to use for dictation</p>
                  </div>
                </div>

                <div className="p-3.5 rounded-lg bg-muted/40 border border-border space-y-3">
                  <div className="flex items-center justify-between">
                    <div>
                      <p className="text-xs font-medium text-foreground">Prefer Built-in Microphone</p>
                      <p className="text-[11px] text-muted-foreground">
                        External microphones may cause latency or reduced transcription quality
                      </p>
                    </div>
                    <Switch
                      checked={settings.audio_input?.prefer_builtin_mic ?? true}
                      onCheckedChange={async (checked) => {
                        const updated: AppSettings = {
                          ...settings,
                          audio_input: {
                            ...settings.audio_input,
                            prefer_builtin_mic: checked,
                            selected_device: settings.audio_input?.selected_device,
                            keep_microphone_warm: settings.audio_input?.keep_microphone_warm || 'off',
                            auto_learn_words: settings.audio_input?.auto_learn_words ?? true,
                          },
                        };
                        setSettings(updated);
                        try {
                          await invoke('save_settings', { settings: updated });
                        } catch (err) {
                          console.error('Failed to update prefer builtin mic', err);
                        }
                      }}
                    />
                  </div>

                  {/* Active Input Device Badge (Green Banner) */}
                  <div className="p-2.5 rounded-lg bg-emerald-500/10 dark:bg-emerald-950/30 border border-emerald-500/30 text-emerald-800 dark:text-emerald-300 text-xs flex items-center gap-2 font-medium">
                    <Mic className="w-4 h-4 text-emerald-600 dark:text-emerald-400 shrink-0" />
                    <span>Using: {activeDeviceName}</span>
                  </div>

                  <div className="h-px bg-border/60" />

                  {/* Keep Microphone Warm */}
                  <div className="flex items-center justify-between">
                    <div>
                      <p className="text-xs font-medium text-foreground">Keep Microphone Warm</p>
                      <p className="text-[11px] text-muted-foreground">
                        After a dictation ends, keep the microphone open briefly so the next one starts instantly with nothing clipped
                      </p>
                    </div>
                    <select
                      value={settings.audio_input?.keep_microphone_warm || 'off'}
                      onChange={async (e) => {
                        const val = e.target.value;
                        const updated: AppSettings = {
                          ...settings,
                          audio_input: {
                            ...settings.audio_input,
                            prefer_builtin_mic: settings.audio_input?.prefer_builtin_mic ?? true,
                            keep_microphone_warm: val,
                            auto_learn_words: settings.audio_input?.auto_learn_words ?? true,
                          },
                        };
                        setSettings(updated);
                        try {
                          await invoke('save_settings', { settings: updated });
                        } catch (err) {
                          console.error('Failed to save keep mic warm', err);
                        }
                      }}
                      className="h-8 rounded-md bg-background border border-input px-2.5 py-1 text-xs text-foreground focus:outline-none focus:ring-1 focus:ring-ring"
                    >
                      <option value="off">Off</option>
                      <option value="15s">15 seconds</option>
                      <option value="30s">30 seconds</option>
                      <option value="1m">1 minute</option>
                      <option value="5m">5 minutes</option>
                    </select>
                  </div>
                </div>
              </div>

              {/* Auto-learn from corrections (OpenWhispr Style) */}
              <div className="py-3 border-b border-border space-y-3">
                <p className="text-xs font-semibold text-foreground">Auto-learn from corrections</p>
                <div className="p-3.5 rounded-lg bg-muted/40 border border-border flex items-center justify-between">
                  <div>
                    <p className="text-xs font-medium text-foreground">Auto-learn from corrections</p>
                    <p className="text-[11px] text-muted-foreground">
                      When you correct a transcription in the target app, the corrected word is automatically added to your dictionary.
                    </p>
                  </div>
                  <Switch
                    checked={settings.audio_input?.auto_learn_words ?? true}
                    onCheckedChange={async (checked) => {
                      const updated: AppSettings = {
                        ...settings,
                        audio_input: {
                          ...settings.audio_input,
                          prefer_builtin_mic: settings.audio_input?.prefer_builtin_mic ?? true,
                          keep_microphone_warm: settings.audio_input?.keep_microphone_warm || 'off',
                          auto_learn_words: checked,
                        },
                      };
                      setSettings(updated);
                      try {
                        await invoke('save_settings', { settings: updated });
                      } catch (err) {
                        console.error('Failed to update auto learn words', err);
                      }
                    }}
                  />
                </div>
              </div>

              {/* Sound Effects */}
              <div className="py-3 border-b border-border space-y-3">
                <div className="flex items-center gap-2">
                  <Volume2 className="w-4 h-4 text-primary" />
                  <p className="text-xs font-semibold text-foreground">Sound Effects</p>
                </div>
                <div className="flex items-center justify-between">
                  <div>
                    <p className="text-xs font-medium text-foreground">Dictation sounds</p>
                    <p className="text-[11px] text-muted-foreground">
                      Play a tone when recording starts and stops
                    </p>
                  </div>
                  <Switch
                    checked={settings.sound?.dictation_sounds ?? true}
                    onCheckedChange={async (checked) => {
                      const updated: AppSettings = {
                        ...settings,
                        sound: {
                          ...settings.sound,
                          dictation_sounds: checked,
                        },
                      };
                      setSettings(updated);
                      try {
                        await invoke('save_settings', { settings: updated });
                      } catch (err) {
                        console.error('Failed to toggle dictation sounds', err);
                      }
                    }}
                  />
                </div>
              </div>

              <Button type="submit" size="sm" variant="default" className="mt-2">
                Save Dictation Settings
              </Button>
            </div>
          </form>
        )}

        {/* 3. DICTIONARY & SNIPPETS SECTION */}
        {activeSection === 'dictionary' && (
          <DictionarySnippetsSettings
            settings={settings}
            onUpdateSettings={setSettings}
            onSaveDirect={handleSaveDirect}
          />
        )}

        {/* 4. LANGUAGES & SCRIPT SECTION */}
        {activeSection === 'languages' && (
          <form onSubmit={handleSave} className="space-y-6">
            <div>
              <p className="font-mono text-[10px] font-bold text-muted-foreground uppercase tracking-widest mb-1">
                MULTILINGUAL & ORTHOGRAPHY CONFIGURATION
              </p>
              <h2 className="text-lg font-bold text-foreground">Language & Writing Script Preferences</h2>
            </div>

            <div className="space-y-4">
              <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
                {/* Primary Dictation Language */}
                <div>
                  <label htmlFor="primary-dictation-lang" className="block text-xs font-medium text-foreground mb-1">
                    Primary Dictation Language
                  </label>
                  <p className="text-[10px] text-muted-foreground mb-1.5">
                    Default language for push-to-talk and quick speech-to-text.
                  </p>
                  <select
                    id="primary-dictation-lang"
                    value={settings.language?.primary_dictation_language || 'en'}
                    onChange={(e) => {
                      const newPrimary = e.target.value;
                      const currentSpoken = settings.language?.spoken_languages || ['en'];
                      const updatedSpoken = currentSpoken.includes(newPrimary)
                        ? currentSpoken
                        : [...currentSpoken, newPrimary];
                      setSettings({
                        ...settings,
                        language: {
                          ...settings.language,
                          primary_dictation_language: newPrimary,
                          spoken_languages: updatedSpoken,
                        },
                      });
                    }}
                    className="w-full h-9 rounded-lg bg-background border border-input px-3 py-1 text-xs text-foreground focus:outline-none focus:ring-1 focus:ring-ring"
                  >
                    {WHISPER_SUPPORTED_LANGUAGES.map((lang) => (
                      <option key={lang.code} value={lang.code}>
                        {lang.name} ({lang.code})
                      </option>
                    ))}
                  </select>
                </div>

                {/* Output Writing Script */}
                <div>
                  <label className="block text-xs font-medium text-foreground mb-1">
                    Output Writing Script
                  </label>
                  <p className="text-[10px] text-muted-foreground mb-1.5">
                    Controls alphabet used, independent of spoken language.
                  </p>
                  <div className="flex bg-muted p-1 rounded-lg border border-border">
                    {[
                      { value: 'latin', label: 'Latin / English' },
                      { value: 'native', label: 'Native Script' },
                    ].map((opt) => (
                      <button
                        key={opt.value}
                        type="button"
                        onClick={() =>
                          setSettings({
                            ...settings,
                            language: {
                              ...settings.language,
                              output_script: opt.value,
                            },
                          })
                        }
                        className={`flex-1 px-3 py-1 text-xs font-medium rounded-lg transition-all ${
                          (settings.language?.output_script || 'latin') === opt.value
                            ? 'bg-card text-foreground font-semibold shadow-xs'
                            : 'text-muted-foreground hover:text-foreground'
                        }`}
                      >
                        {opt.label}
                      </button>
                    ))}
                  </div>
                </div>
              </div>

              {/* Languages I Speak (Multi-select) */}
              <div className="pt-2">
                <label className="block text-xs font-medium text-foreground mb-1">
                  Languages I Speak (Spoken Profile)
                </label>
                <p className="text-[10px] text-muted-foreground mb-2">
                  Select all languages you commonly speak. Relay recognizes and transcribes speech across your spoken languages profile.
                </p>

                {/* Selected language chips */}
                <div className="flex flex-wrap gap-1.5 mb-2.5 min-h-[32px] p-2 rounded-lg bg-muted/40 border border-border items-center">
                  {(settings.language?.spoken_languages || ['en']).map((code) => {
                    const langObj = WHISPER_SUPPORTED_LANGUAGES.find((l) => l.code === code);
                    const label = langObj ? `${langObj.name} (${code})` : code;
                    const isPrimary = settings.language?.primary_dictation_language === code;
                    return (
                      <Badge
                        key={code}
                        variant="secondary"
                        className="text-[11px] font-medium py-1 px-2.5 gap-1.5 rounded-lg border border-border/80 flex items-center bg-card text-foreground"
                      >
                        <span>{label}</span>
                        {isPrimary && (
                          <span className="text-[9px] uppercase tracking-wider text-primary font-bold">(Primary)</span>
                        )}
                        {(settings.language?.spoken_languages || []).length > 1 && (
                          <button
                            type="button"
                            onClick={() => {
                              const current = settings.language?.spoken_languages || ['en'];
                              const updated = current.filter((c) => c !== code);
                              setSettings({
                                ...settings,
                                language: {
                                  ...settings.language,
                                  spoken_languages: updated.length > 0 ? updated : ['en'],
                                },
                              });
                            }}
                            className="text-muted-foreground hover:text-destructive ml-0.5"
                            title="Remove language"
                          >
                            ×
                          </button>
                        )}
                      </Badge>
                    );
                  })}
                </div>

                {/* Quick-add toggle badges */}
                <div className="flex items-center gap-1.5 flex-wrap">
                  <span className="text-[10px] text-muted-foreground mr-1">Quick add:</span>
                  {WHISPER_SUPPORTED_LANGUAGES.slice(0, 10).map((lang) => {
                    const isSelected = (settings.language?.spoken_languages || ['en']).includes(lang.code);
                    if (isSelected) return null;
                    return (
                      <button
                        key={lang.code}
                        type="button"
                        onClick={() => {
                          const current = settings.language?.spoken_languages || ['en'];
                          setSettings({
                            ...settings,
                            language: {
                              ...settings.language,
                              spoken_languages: [...current, lang.code],
                            },
                          });
                        }}
                        className="px-2 py-0.5 text-[10px] rounded-lg border border-border bg-background text-muted-foreground hover:text-foreground hover:border-primary/50 transition-colors"
                      >
                        + {lang.name}
                      </button>
                    );
                  })}
                </div>
              </div>

              {/* Notes & Summarization Language */}
              <div className="py-3 border-t border-border">
                <label htmlFor="notes-lang" className="block text-xs font-semibold text-foreground mb-1">
                  Notes & Summarization Language
                </label>
                <p className="text-[11px] text-muted-foreground mb-2">
                  Language used by local/cloud LLM when synthesizing structured voice notes and summaries.
                </p>
                <select
                  id="notes-lang"
                  value={settings.language?.notes_language || 'en'}
                  onChange={(e) =>
                    setSettings({
                      ...settings,
                      language: {
                        ...settings.language,
                        notes_language: e.target.value,
                      },
                    })
                  }
                  className="max-w-md w-full h-9 rounded-lg bg-background border border-input px-3 py-1 text-xs text-foreground focus:outline-none focus:ring-1 focus:ring-ring"
                >
                  {WHISPER_SUPPORTED_LANGUAGES.map((lang) => (
                    <option key={lang.code} value={lang.code}>
                      {lang.name} ({lang.code})
                    </option>
                  ))}
                </select>
              </div>

              <Button type="submit" size="sm" variant="default" className="mt-2">
                Save Language Settings
              </Button>
            </div>
          </form>
        )}

        {/* 5. AI MODELS & STT SECTION */}
        {activeSection === 'advanced' && (
          <div className="space-y-6 animate-in fade-in-50">
            {/* Dedicated Diagnostics Redirect Banner */}
            <div className="p-4 rounded-lg border border-primary/20 bg-primary/5 flex flex-col sm:flex-row sm:items-center justify-between gap-3">
              <div className="flex items-start gap-3">
                <div className="p-2 rounded-lg bg-primary/10 text-primary shrink-0 mt-0.5 sm:mt-0">
                  <Activity className="w-4 h-4" />
                </div>
                <div>
                  <p className="text-xs font-bold text-foreground">Need Technical Testing or Observability?</p>
                  <p className="text-[11px] text-muted-foreground mt-0.5 leading-relaxed">
                    Live audio telemetry, VAD decisions, decoding diagnostics, and STT accuracy benchmarking have moved to the dedicated Diagnostics page.
                  </p>
                </div>
              </div>
              {onNavigateTab && (
                <Button
                  type="button"
                  variant="outline"
                  size="sm"
                  onClick={() => onNavigateTab('diagnostics')}
                  className="text-xs gap-1.5 shrink-0 self-start sm:self-auto border-primary/30 text-primary hover:bg-primary/10"
                >
                  <Activity className="w-3.5 h-3.5" />
                  Open Diagnostics
                </Button>
              )}
            </div>

            <form onSubmit={handleSave} className="space-y-6">
              <div>
                <p className="font-mono text-[10px] font-bold text-muted-foreground uppercase tracking-widest mb-1">
                  AI INTELLIGENCE & SPEECH ENGINE
                </p>
                <h2 className="text-lg font-bold text-foreground">Model Configuration & Selection</h2>
                <p className="text-xs text-muted-foreground mt-0.5">
                  Configure local or cloud LLM intelligence and universal speech-to-text models.
                </p>
              </div>

              <div className="space-y-6">
                {/* 1. ACTIVE LLM BACKEND */}
                <div className="p-4 rounded-lg border border-border bg-card/60 space-y-4">
                  <div className="flex flex-col sm:flex-row sm:items-center justify-between gap-2 pb-3 border-b border-border/60">
                    <div>
                      <p className="text-xs font-semibold text-foreground">Active LLM Execution Backend</p>
                      <p className="text-[11px] text-muted-foreground">100% Local Ollama ($0) vs OpenAI / Gemini / Claude Cloud API</p>
                    </div>
                    <div className="flex bg-muted p-1 rounded-lg border border-border shrink-0 self-start sm:self-auto">
                      <button
                        type="button"
                        onClick={() =>
                          setSettings({ ...settings, provider: { ...settings.provider, active_provider: 'ollama' } })
                        }
                        className={`px-3 py-1 text-xs font-medium rounded-lg transition-all ${
                          settings.provider.active_provider === 'ollama'
                            ? 'bg-card text-foreground font-semibold shadow-xs'
                            : 'text-muted-foreground'
                        }`}
                      >
                        Local Ollama
                      </button>
                      <button
                        type="button"
                        onClick={() =>
                          setSettings({ ...settings, provider: { ...settings.provider, active_provider: 'cloud_openai' } })
                        }
                        className={`px-3 py-1 text-xs font-medium rounded-lg transition-all ${
                          settings.provider.active_provider !== 'ollama'
                            ? 'bg-card text-foreground font-semibold shadow-xs'
                            : 'text-muted-foreground'
                        }`}
                      >
                        Cloud API
                      </button>
                    </div>
                  </div>

                  {/* Local Ollama Options */}
                  {settings.provider.active_provider === 'ollama' ? (
                    <div className="space-y-4">
                      {/* Host & Health */}
                      <div className="space-y-2">
                        <label htmlFor="ollama-host" className="block text-xs font-medium text-foreground">
                          Ollama Host Endpoint
                        </label>
                        <div className="flex gap-2">
                          <Input
                            id="ollama-host"
                            value={settings.provider.ollama_host}
                            onChange={(e) =>
                              setSettings({ ...settings, provider: { ...settings.provider, ollama_host: e.target.value } })
                            }
                            placeholder="http://localhost:11434"
                            className="text-xs"
                          />
                          <Button
                            type="button"
                            size="sm"
                            variant="outline"
                            onClick={() => checkLocalLlm(settings.provider.ollama_host)}
                            className="text-xs gap-1.5 shrink-0"
                          >
                            <RefreshCw className={`w-3.5 h-3.5 ${ollamaStatus.state === 'checking' ? 'animate-spin' : ''}`} />
                            Scan Models
                          </Button>
                        </div>

                        {/* Status readout */}
                        <div className="flex items-center gap-2 text-xs pt-1">
                          {ollamaStatus.state === 'checking' && (
                            <Badge variant="outline" className="text-[10px] font-mono">Checking local Ollama…</Badge>
                          )}
                          {ollamaStatus.state === 'running' && (
                            <Badge variant="emerald" className="text-[10px] font-mono">Ollama running ✓</Badge>
                          )}
                          {ollamaStatus.state === 'started' && (
                            <Badge variant="emerald" className="text-[10px] font-mono">Relay started Ollama for you ✓</Badge>
                          )}
                          {ollamaStatus.state === 'not_installed' && (
                            <Badge variant="outline" className="text-[10px] font-mono border-amber-500/50 text-amber-500">
                              Ollama isn't installed — install Ollama once to run locally
                            </Badge>
                          )}
                          {ollamaStatus.state === 'unreachable' && (
                            <Badge variant="outline" className="text-[10px] font-mono border-destructive/50 text-destructive">
                              {ollamaStatus.message || 'Ollama is unreachable'}
                            </Badge>
                          )}
                        </div>
                      </div>

                      {/* Active & Available Models */}
                      <div className="space-y-3 pt-2">
                        <div className="flex items-center justify-between">
                          <label className="block text-xs font-semibold text-foreground">
                            Active LLM Model
                          </label>
                          {/* Readiness badge for currently selected model */}
                          {(() => {
                            const isInstalled = ollamaModels.some(
                              (m) => m.name === settings.provider.ollama_model || m.model === settings.provider.ollama_model
                            );
                            if (ollamaStatus.state === 'checking') {
                              return <Badge variant="outline" className="text-[10px] font-mono">↻ Checking</Badge>;
                            }
                            if (ollamaStatus.state === 'running' || ollamaStatus.state === 'started') {
                              if (isInstalled) {
                                return <Badge variant="emerald" className="text-[10px] font-mono">✓ Ready · Ollama</Badge>;
                              } else {
                                return (
                                  <Badge variant="outline" className="text-[10px] font-mono border-amber-500/50 text-amber-500">
                                    ⚠ Model not found
                                  </Badge>
                                );
                              }
                            }
                            return (
                              <Badge variant="outline" className="text-[10px] font-mono border-destructive/50 text-destructive">
                                ✕ Backend unavailable
                              </Badge>
                            );
                          })()}
                        </div>

                        {/* Active Model Summary Card */}
                        <div className="p-3 rounded-lg border border-primary/30 bg-primary/5 flex items-center justify-between">
                          <div className="space-y-0.5">
                            <span className="text-[10px] font-mono uppercase tracking-wider text-muted-foreground font-semibold">
                              Current Selection
                            </span>
                            <p className="text-sm font-bold text-foreground font-mono">
                              {settings.provider.ollama_model || 'llama3.2:latest'}
                            </p>
                          </div>
                          {ollamaModels.some((m) => m.name === settings.provider.ollama_model) ? (
                            <Badge variant="emerald" className="text-[10px] font-mono">
                              Installed
                            </Badge>
                          ) : (
                            <Badge variant="outline" className="text-[10px] font-mono border-amber-500/50 text-amber-500">
                              Not in registry
                            </Badge>
                          )}
                        </div>

                        {/* Available Models Picker */}
                        <div className="space-y-2">
                          <span className="text-[11px] font-medium text-foreground">
                            Available Models from Ollama ({ollamaModels.length})
                          </span>

                          {loadingOllamaModels ? (
                            <div className="p-4 text-center text-xs text-muted-foreground border border-dashed rounded-lg">
                              Scanning models from Ollama…
                            </div>
                          ) : ollamaModels.length > 0 ? (
                            <div className="grid grid-cols-1 sm:grid-cols-2 gap-2">
                              {ollamaModels.map((m) => {
                                const isSelected = settings.provider.ollama_model === m.name;
                                return (
                                  <button
                                    key={m.name}
                                    type="button"
                                    onClick={() =>
                                      setSettings({
                                        ...settings,
                                        provider: { ...settings.provider, ollama_model: m.name },
                                      })
                                    }
                                    className={`p-2.5 rounded-lg border text-left transition-all flex items-start justify-between ${
                                      isSelected
                                        ? 'border-primary bg-primary/10 text-foreground shadow-xs'
                                        : 'border-border bg-muted/20 text-muted-foreground hover:border-border/80 hover:text-foreground'
                                    }`}
                                  >
                                    <div className="space-y-1 min-w-0 pr-2">
                                      <div className="flex items-center gap-1.5">
                                        <span className="text-xs font-semibold font-mono truncate">{m.name}</span>
                                        {isSelected && (
                                          <Check className="w-3.5 h-3.5 text-primary shrink-0" />
                                        )}
                                      </div>
                                      <div className="flex items-center gap-1.5 flex-wrap">
                                        {m.parameter_size && (
                                          <Badge variant="outline" className="text-[9px] px-1 py-0 font-mono">
                                            {m.parameter_size}
                                          </Badge>
                                        )}
                                        {m.quantization_level && (
                                          <Badge variant="outline" className="text-[9px] px-1 py-0 font-mono">
                                            {m.quantization_level}
                                          </Badge>
                                        )}
                                      </div>
                                    </div>
                                    <span className="text-[10px] font-mono text-muted-foreground shrink-0">
                                      {m.size ? `${(m.size / (1024 * 1024 * 1024)).toFixed(1)} GB` : ''}
                                    </span>
                                  </button>
                                );
                              })}
                            </div>
                          ) : (
                            <div className="p-3.5 rounded-lg border border-border bg-muted/20 text-xs text-muted-foreground space-y-1">
                              <p className="font-semibold text-foreground">No installed models found in Ollama.</p>
                              <p className="text-[11px]">
                                Run <code className="px-1.5 py-0.5 rounded bg-muted border border-border font-mono text-foreground">ollama pull llama3.2</code> in your terminal, or enter a model name manually below.
                              </p>
                            </div>
                          )}

                          {/* Manual / Custom Model toggle */}
                          <div className="pt-2">
                            <button
                              type="button"
                              onClick={() => setCustomLlmMode(!customLlmMode)}
                              className="text-[11px] text-muted-foreground hover:text-foreground flex items-center gap-1"
                            >
                              {customLlmMode ? <ChevronUp className="w-3.5 h-3.5" /> : <ChevronDown className="w-3.5 h-3.5" />}
                              <span>{customLlmMode ? 'Hide advanced model input' : 'Specify custom / unpulled model name manually…'}</span>
                            </button>

                            {customLlmMode && (
                              <div className="mt-2 space-y-1 animate-in fade-in-50">
                                <label htmlFor="custom-ollama-model" className="block text-[11px] text-muted-foreground">
                                  Manual Model Name
                                </label>
                                <Input
                                  id="custom-ollama-model"
                                  value={settings.provider.ollama_model}
                                  onChange={(e) =>
                                    setSettings({
                                      ...settings,
                                      provider: { ...settings.provider, ollama_model: e.target.value },
                                    })
                                  }
                                  placeholder="e.g. qwen2.5:7b, gemma3:4b"
                                  className="text-xs"
                                />
                              </div>
                            )}
                          </div>
                        </div>
                      </div>
                    </div>
                  ) : (
                    /* Cloud API Options */
                    <div className="space-y-4">
                      {/* Cloud Provider Select */}
                      <div className="space-y-1.5">
                        <label className="block text-xs font-medium text-foreground">
                          Cloud Provider
                        </label>
                        <div className="grid grid-cols-3 gap-2">
                          {[
                            { id: 'cloud_openai', label: 'OpenAI' },
                            { id: 'cloud_gemini', label: 'Google Gemini' },
                            { id: 'cloud_anthropic', label: 'Anthropic Claude' },
                          ].map((prov) => (
                            <button
                              key={prov.id}
                              type="button"
                              onClick={() =>
                                setSettings({
                                  ...settings,
                                  provider: {
                                    ...settings.provider,
                                    active_provider: prov.id as any,
                                    cloud_model:
                                      prov.id === 'cloud_gemini'
                                        ? 'gemini-2.0-flash'
                                        : prov.id === 'cloud_anthropic'
                                        ? 'claude-3-5-sonnet-20241022'
                                        : 'gpt-4o-mini',
                                  },
                                })
                              }
                              className={`p-2 rounded-lg border text-xs font-semibold transition-all ${
                                settings.provider.active_provider === prov.id
                                  ? 'border-primary bg-primary/10 text-foreground'
                                  : 'border-border bg-card/50 text-muted-foreground hover:border-border/80'
                              }`}
                            >
                              {prov.label}
                            </button>
                          ))}
                        </div>
                      </div>

                      {/* API Key */}
                      <div>
                        <label htmlFor="cloud-api-key" className="block text-xs font-medium text-foreground mb-1">
                          API Secret Key
                        </label>
                        <Input
                          id="cloud-api-key"
                          type="password"
                          value={settings.provider.cloud_api_key || ''}
                          onChange={(e) =>
                            setSettings({ ...settings, provider: { ...settings.provider, cloud_api_key: e.target.value } })
                          }
                          placeholder="sk-..."
                          className="text-xs"
                        />
                      </div>

                      {/* Cloud Model Selector */}
                      <div className="space-y-2">
                        <label className="block text-xs font-medium text-foreground">
                          Cloud Model Selection
                        </label>
                        <div className="grid grid-cols-3 gap-2">
                          {(settings.provider.active_provider === 'cloud_gemini'
                            ? ['gemini-2.0-flash', 'gemini-1.5-pro', 'gemini-1.5-flash']
                            : settings.provider.active_provider === 'cloud_anthropic'
                            ? ['claude-3-5-sonnet-20241022', 'claude-3-5-haiku-20241022', 'claude-3-opus-20240229']
                            : ['gpt-4o-mini', 'gpt-4o', 'o3-mini']
                          ).map((mName) => (
                            <button
                              key={mName}
                              type="button"
                              onClick={() =>
                                setSettings({
                                  ...settings,
                                  provider: { ...settings.provider, cloud_model: mName },
                                })
                              }
                              className={`p-2 rounded-lg border text-xs font-mono transition-all ${
                                settings.provider.cloud_model === mName
                                  ? 'border-primary bg-primary/10 text-foreground font-bold'
                                  : 'border-border bg-card/50 text-muted-foreground hover:border-border/80'
                              }`}
                            >
                              {mName}
                            </button>
                          ))}
                        </div>

                        <div>
                          <Input
                            id="cloud-model-custom"
                            value={settings.provider.cloud_model || ''}
                            onChange={(e) =>
                              setSettings({ ...settings, provider: { ...settings.provider, cloud_model: e.target.value } })
                            }
                            placeholder="Custom model name..."
                            className="text-xs mt-1"
                          />
                        </div>
                      </div>
                    </div>
                  )}
                </div>

                {/* 2. SPEECH-TO-TEXT MODEL (WHISPER) */}
                <div className="p-4 rounded-lg border border-border bg-card/60 space-y-4">
                  <div className="flex items-center justify-between pb-3 border-b border-border/60">
                    <div className="flex items-center gap-2">
                      <Mic className="w-4 h-4 text-primary" />
                      <div>
                        <p className="text-xs font-semibold text-foreground">Speech-to-Text Model (Whisper)</p>
                        <p className="text-[11px] text-muted-foreground">On-device acoustic transcription via GGML Whisper</p>
                      </div>
                    </div>
                    {/* Active STT Model Status Badge */}
                    {sttOverview?.models.find((m) => m.path === sttOverview.active_model_path)?.status === 'ready' ? (
                      <Badge variant="emerald" className="text-[10px] font-mono">✓ Model ready · Whisper</Badge>
                    ) : sttModelStatus.state === 'checking' ? (
                      <Badge variant="outline" className="text-[10px] font-mono">↻ Checking</Badge>
                    ) : (
                      <Badge variant="outline" className="text-[10px] font-mono border-amber-500/50 text-amber-500">
                        ⚠ Model missing
                      </Badge>
                    )}
                  </div>

                  {/* Active STT Model Readout */}
                  <div className="p-3 rounded-lg border border-primary/30 bg-primary/5 flex items-center justify-between">
                    <div className="space-y-0.5 min-w-0 pr-2">
                      <span className="text-[10px] font-mono uppercase tracking-wider text-muted-foreground font-semibold">
                        Active STT Model
                      </span>
                      <p className="text-sm font-bold text-foreground font-mono truncate">
                        {sttOverview?.active_model_name || 'Whisper Small (Default)'}
                      </p>
                      <p className="text-[10px] text-muted-foreground font-mono truncate">
                        {sttOverview?.active_model_path || '%APPDATA%\\Relay\\models\\ggml-small.bin'}
                      </p>
                    </div>
                    <Badge variant="emerald" className="text-[10px] font-mono shrink-0">
                      Active
                    </Badge>
                  </div>

                  {/* Available STT Models List */}
                  <div className="space-y-2">
                    <div className="flex items-center justify-between">
                      <span className="text-[11px] font-medium text-foreground">
                        Available STT Models on Disk ({sttOverview?.models.length || 0})
                      </span>
                      <Button
                        type="button"
                        variant="ghost"
                        size="sm"
                        onClick={fetchSttModels}
                        disabled={loadingSttModels}
                        className="text-[11px] h-6 px-2 gap-1"
                      >
                        <RefreshCw className={`w-3 h-3 ${loadingSttModels ? 'animate-spin' : ''}`} />
                        Scan
                      </Button>
                    </div>

                    <div className="grid grid-cols-1 sm:grid-cols-2 gap-2">
                      {sttOverview?.models.map((m) => {
                        const isActive = m.path === sttOverview.active_model_path;
                        return (
                          <div
                            key={m.filename}
                            className={`p-2.5 rounded-lg border text-xs space-y-1 ${
                              isActive
                                ? 'border-primary bg-primary/10 text-foreground'
                                : 'border-border bg-muted/20 text-muted-foreground'
                            }`}
                          >
                            <div className="flex items-center justify-between">
                              <span className="font-semibold text-foreground flex items-center gap-1.5">
                                {m.name}
                                {isActive && <Check className="w-3.5 h-3.5 text-primary" />}
                              </span>
                              <Badge
                                variant={m.status === 'ready' ? 'emerald' : 'outline'}
                                className="text-[9px] font-mono"
                              >
                                {m.status === 'ready' ? '✓ Ready' : '⚠ Missing'}
                              </Badge>
                            </div>
                            <div className="text-[10px] font-mono text-muted-foreground flex items-center justify-between">
                              <span>{m.filename}</span>
                              <span>{m.size_bytes ? `${(m.size_bytes / (1024 * 1024)).toFixed(0)} MB` : ''}</span>
                            </div>
                          </div>
                        );
                      })}
                    </div>
                  </div>

                  {/* Performance Profile Toggle */}
                  <div className="space-y-2 pt-2">
                    <label className="block text-xs font-semibold text-foreground">
                      Universal Dictation Performance Profile
                    </label>
                    <div className="grid grid-cols-1 sm:grid-cols-2 gap-2">
                      <button
                        type="button"
                        onClick={() => {
                          setSettings({
                            ...settings,
                            stt: {
                              ...settings.stt,
                              dictation_quality: 'fast',
                              dictationQuality: 'fast',
                              whisper_model_path: null,
                            },
                          });
                        }}
                        className={`p-3 rounded-lg border text-left transition-all ${
                          (settings.stt.dictation_quality ?? 'fast') === 'fast' &&
                          !settings.stt.whisper_model_path
                            ? 'border-primary bg-primary/10 text-foreground shadow-xs'
                            : 'border-border bg-card/50 text-muted-foreground hover:border-border/80'
                        }`}
                      >
                        <div className="flex items-center justify-between mb-1">
                          <span className="text-xs font-bold text-foreground">Fast (Base Model)</span>
                          <Badge variant="emerald" className="text-[9px] px-1.5 py-0">~0.8s</Badge>
                        </div>
                        <p className="text-[11px] text-muted-foreground leading-snug">
                          3x lower latency using Base model (39M params). Recommended for conversational speech.
                        </p>
                      </button>

                      <button
                        type="button"
                        onClick={() => {
                          setSettings({
                            ...settings,
                            stt: {
                              ...settings.stt,
                              dictation_quality: 'accurate',
                              dictationQuality: 'accurate',
                              whisper_model_path: null,
                            },
                          });
                        }}
                        className={`p-3 rounded-lg border text-left transition-all ${
                          settings.stt.dictation_quality === 'accurate' &&
                          !settings.stt.whisper_model_path
                            ? 'border-primary bg-primary/10 text-foreground shadow-xs'
                            : 'border-border bg-card/50 text-muted-foreground hover:border-border/80'
                        }`}
                      >
                        <div className="flex items-center justify-between mb-1">
                          <span className="text-xs font-bold text-foreground">Accurate (Small Model)</span>
                          <Badge variant="outline" className="text-[9px] px-1.5 py-0">~2.4s</Badge>
                        </div>
                        <p className="text-[11px] text-muted-foreground leading-snug">
                          Maximum vocabulary fidelity (244M params). Recommended for complex technical monologues.
                        </p>
                      </button>
                    </div>
                  </div>

                  {/* Custom Model Path (Advanced) */}
                  <div className="pt-2">
                    <button
                      type="button"
                      onClick={() => setCustomSttMode(!customSttMode)}
                      className="text-[11px] text-muted-foreground hover:text-foreground flex items-center gap-1"
                    >
                      {customSttMode ? <ChevronUp className="w-3.5 h-3.5" /> : <ChevronDown className="w-3.5 h-3.5" />}
                      <span>{customSttMode ? 'Hide custom model path' : 'Use external or custom GGML model file…'}</span>
                    </button>

                    {customSttMode && (
                      <div className="mt-2 space-y-1 animate-in fade-in-50">
                        <label htmlFor="whisper-model-path" className="block text-[11px] text-muted-foreground">
                          Custom GGML Model Path (leave empty to use Relay managed models)
                        </label>
                        <Input
                          id="whisper-model-path"
                          placeholder="e.g. C:\models\ggml-medium.bin"
                          value={settings.stt.whisper_model_path || ''}
                          onChange={(e) =>
                            setSettings({
                              ...settings,
                              stt: { ...settings.stt, whisper_model_path: e.target.value },
                            })
                          }
                          className="text-xs"
                        />
                      </div>
                    )}
                  </div>
                </div>

                <div className="flex items-center justify-between pt-2">
                  <Button type="submit" size="sm" variant="default" className="text-xs">
                    Save Engine Settings
                  </Button>
                  {saved && (
                    <span className="text-xs font-medium text-emerald-500 animate-in fade-in-50">
                      Settings saved successfully ✓
                    </span>
                  )}
                </div>
              </div>
            </form>
          </div>
        )}

        {/* 6. PRIVACY & VAULT SECTION */}
        {activeSection === 'privacy' && (
          <div className="space-y-6 animate-in fade-in-50">
            <div>
              <p className="font-mono text-[10px] font-bold text-muted-foreground uppercase tracking-widest mb-1">
                DATA CONTROL & PRIVACY BOUNDARIES
              </p>
              <h2 className="text-lg font-bold text-foreground">Data Ownership & Vault Isolation</h2>
            </div>

            {deleteAccountSuccess && (
              <div className="p-3.5 rounded-lg border border-emerald-500/30 bg-emerald-500/10 text-emerald-600 dark:text-emerald-400 text-xs flex items-center gap-2">
                <CheckCircle className="w-4 h-4 shrink-0" />
                <span>{deleteAccountSuccess}</span>
              </div>
            )}

            {deleteAccountError && (
              <div className="p-3.5 rounded-lg border border-destructive/30 bg-destructive/10 text-destructive text-xs flex items-center gap-2">
                <AlertCircle className="w-4 h-4 shrink-0" />
                <span>{deleteAccountError}</span>
              </div>
            )}

            <div className="space-y-4">
              {/* Privacy Overview */}
              <div className="p-4 rounded-lg bg-muted/40 border border-border space-y-2">
                <div className="flex items-center gap-2 text-primary font-semibold text-xs">
                  <ShieldCheck className="w-4 h-4" />
                  <span>100% Local-First Processing</span>
                </div>
                <p className="text-[11px] text-muted-foreground leading-relaxed">
                  Relay operates locally on your machine. Voice transcriptions, raw audio recordings, markdown notes,
                  and LanceDB vectors stay inside your local directory. No third-party tracking or telemetry is collected.
                </p>
              </div>

              {/* Safe Export Action */}
              <div className="p-4 rounded-lg bg-card border border-border flex items-center justify-between">
                <div>
                  <p className="text-xs font-bold text-foreground">Export All Vault Data</p>
                  <p className="text-[11px] text-muted-foreground">Download full backup of notes, tasks, and LanceDB embeddings</p>
                </div>
                <Button
                  variant="default"
                  size="sm"
                  className="gap-2 text-xs"
                  onClick={async () => {
                    const dir = vaultLocation?.path;
                    if (dir) {
                      try {
                        await invoke('open_vault_in_explorer');
                      } catch {
                        alert(`Your vault is stored at: ${dir}`);
                      }
                    }
                  }}
                >
                  <Download className="w-4 h-4" />
                  <span>Explore Vault Folder</span>
                </Button>
              </div>

              {/* Destructive Actions Section */}
              <div className="p-4 rounded-lg border border-destructive/40 bg-destructive/5 space-y-4">
                <div className="flex items-center gap-2 text-destructive font-bold text-xs">
                  <AlertTriangle className="w-4 h-4 shrink-0" />
                  <span>Irreversible Data Reset & Account Actions</span>
                </div>

                {/* 1. Delete Relay Cloud Account */}
                <div className="py-2.5 border-t border-destructive/20 flex flex-col sm:flex-row sm:items-center justify-between gap-3">
                  <div className="space-y-0.5">
                    <div className="flex items-center gap-2">
                      <p className="text-xs font-semibold text-foreground">Delete Relay Cloud Account</p>
                      <Badge variant="outline" className="text-[9px] px-1.5 py-0 border-destructive/30 text-destructive font-mono">
                        {account?.authenticated ? 'Cloud Linked' : 'Local Only'}
                      </Badge>
                    </div>
                    <p className="text-[11px] text-muted-foreground max-w-md">
                      Deletes your cloud account record and clears OS Keyring tokens.
                      <strong className="text-foreground ml-1">Account ≠ Vault: Your local markdown notes, recordings, and scribbles remain 100% on this PC.</strong>
                    </p>
                  </div>
                  <Button
                    variant="outline"
                    size="sm"
                    className="border-destructive/60 text-destructive hover:bg-destructive/10 gap-1.5 text-xs shrink-0"
                    onClick={() => {
                      setDeleteAccountModalOpen(true);
                      setDeleteAccountAck(false);
                      setDeleteAccountInput('');
                      setDeleteAccountError(null);
                    }}
                    disabled={!account?.authenticated}
                  >
                    <User className="w-3.5 h-3.5" />
                    <span>Delete Cloud Account</span>
                  </Button>
                </div>

                {/* 2. Clear Local Vault & Index */}
                <div className="py-2.5 border-t border-destructive/20 flex flex-col sm:flex-row sm:items-center justify-between gap-3">
                  <div className="space-y-0.5">
                    <p className="text-xs font-semibold text-foreground">Clear Local Vault & Index</p>
                    <p className="text-[11px] text-muted-foreground max-w-md">
                      Permanently wipes stored markdown files, voice notes, scribbles, and the LanceDB vector database from local disk.
                    </p>
                  </div>
                  <Button
                    variant="outline"
                    size="sm"
                    className="border-destructive/60 text-destructive hover:bg-destructive/10 gap-1.5 text-xs shrink-0"
                    onClick={() => {
                      setClearVaultModalOpen(true);
                      setClearVaultAck(false);
                      setClearVaultInput('');
                    }}
                  >
                    <Trash2 className="w-3.5 h-3.5" />
                    <span>Clear Local Vault</span>
                  </Button>
                </div>

                {/* 3. Disconnect Sync */}
                {account?.authenticated && (
                  <div className="py-2.5 border-t border-destructive/20 flex flex-col sm:flex-row sm:items-center justify-between gap-3">
                    <div className="space-y-0.5">
                      <p className="text-xs font-semibold text-foreground">Disconnect Hybrid Cloud Sync</p>
                      <p className="text-[11px] text-muted-foreground max-w-md">
                        Signs out of your Relay identity and returns the application to 100% offline local-only operating mode.
                      </p>
                    </div>
                    <Button
                      variant="outline"
                      size="sm"
                      className="border-border text-muted-foreground hover:text-foreground gap-1.5 text-xs shrink-0"
                      onClick={handleDisconnectSync}
                    >
                      <Cloud className="w-3.5 h-3.5" />
                      <span>Disconnect Sync</span>
                    </Button>
                  </div>
                )}
              </div>
            </div>

            {/* Modal: Delete Relay Account Double Confirmation */}
            {deleteAccountModalOpen && (
              <div className="fixed inset-0 z-50 flex items-center justify-center bg-background/80 backdrop-blur-sm p-4 animate-in fade-in-50">
                <div className="w-full max-w-md bg-card border border-destructive/50 rounded-lg p-6 shadow-2xl space-y-5">
                  <div className="flex items-start gap-3">
                    <div className="p-2 rounded-lg bg-destructive/10 text-destructive shrink-0">
                      <AlertTriangle className="w-5 h-5" />
                    </div>
                    <div className="space-y-1">
                      <h3 className="text-sm font-bold text-foreground">Delete Relay Cloud Account</h3>
                      <p className="text-xs text-muted-foreground leading-relaxed">
                        Step 1 of 2: Review destruction scope.
                      </p>
                    </div>
                  </div>

                  <div className="p-3.5 rounded-lg border border-border bg-muted/30 space-y-2 text-xs">
                    <div className="flex items-center gap-1.5 font-semibold text-destructive">
                      <span>What will be deleted:</span>
                    </div>
                    <ul className="list-disc list-inside space-y-1 text-muted-foreground pl-1 text-[11px]">
                      <li>Your Relay cloud profile and registration in Supabase</li>
                      <li>Secure OAuth credentials stored in your OS Keyring</li>
                      <li>Google Calendar synchronization association</li>
                    </ul>

                    <div className="pt-2 border-t border-border/60">
                      <div className="flex items-center gap-1.5 font-semibold text-emerald-600 dark:text-emerald-400">
                        <Check className="w-3.5 h-3.5" />
                        <span>What is PRESERVED (Account ≠ Vault):</span>
                      </div>
                      <p className="text-[11px] text-muted-foreground mt-0.5 leading-relaxed">
                        All your local Markdown files, Voice Notes, Scribbles, Audio files, and Vector index remain 100% untouched on this device.
                      </p>
                    </div>
                  </div>

                  {/* Double Confirmation Step */}
                  <div className="space-y-3 pt-1">
                    <label className="flex items-start gap-2 text-xs text-foreground cursor-pointer select-none">
                      <input
                        type="checkbox"
                        checked={deleteAccountAck}
                        onChange={(e) => setDeleteAccountAck(e.target.checked)}
                        className="mt-0.5 rounded border-border text-destructive focus:ring-destructive"
                      />
                      <span className="text-[11px] leading-tight text-muted-foreground">
                        I understand this permanently removes my cloud account and disconnects this installation.
                      </span>
                    </label>

                    <div className="space-y-1.5">
                      <label className="text-[11px] font-semibold text-muted-foreground">
                        Type <span className="font-mono text-destructive font-bold">DELETE ACCOUNT</span> to confirm:
                      </label>
                      <Input
                        value={deleteAccountInput}
                        onChange={(e) => setDeleteAccountInput(e.target.value)}
                        placeholder="DELETE ACCOUNT"
                        className="h-8 text-xs font-mono"
                      />
                    </div>
                  </div>

                  <div className="flex items-center justify-end gap-2 pt-2 border-t border-border">
                    <Button
                      size="sm"
                      variant="ghost"
                      className="text-xs h-8"
                      onClick={() => setDeleteAccountModalOpen(false)}
                      disabled={deletingAccount}
                    >
                      Cancel
                    </Button>
                    <Button
                      size="sm"
                      variant="destructive"
                      className="text-xs h-8 gap-1.5"
                      onClick={handleDeleteAccount}
                      disabled={
                        !deleteAccountAck ||
                        deleteAccountInput.trim().toUpperCase() !== 'DELETE ACCOUNT' ||
                        deletingAccount
                      }
                    >
                      {deletingAccount ? <RefreshCw className="w-3.5 h-3.5 animate-spin" /> : null}
                      <span>{deletingAccount ? 'Deleting Account...' : 'Permanently Delete Account'}</span>
                    </Button>
                  </div>
                </div>
              </div>
            )}

            {/* Modal: Clear Vault Double Confirmation */}
            {clearVaultModalOpen && (
              <div className="fixed inset-0 z-50 flex items-center justify-center bg-background/80 backdrop-blur-sm p-4 animate-in fade-in-50">
                <div className="w-full max-w-md bg-card border border-destructive/50 rounded-lg p-6 shadow-2xl space-y-5">
                  <div className="flex items-start gap-3">
                    <div className="p-2 rounded-lg bg-destructive/10 text-destructive shrink-0">
                      <AlertTriangle className="w-5 h-5" />
                    </div>
                    <div className="space-y-1">
                      <h3 className="text-sm font-bold text-destructive">Wipe Local Vault & Vectors</h3>
                      <p className="text-xs text-muted-foreground leading-relaxed">
                        Double Confirmation Required for Destructive Action.
                      </p>
                    </div>
                  </div>

                  <div className="p-3.5 rounded-lg border border-destructive/30 bg-destructive/5 space-y-2 text-xs">
                    <p className="font-semibold text-destructive">
                      WARNING: This action is IRREVERSIBLE.
                    </p>
                    <p className="text-[11px] text-muted-foreground leading-relaxed">
                      All local markdown notes, scribbles, audio recordings, and vector index tables in your vault directory will be deleted from your disk.
                    </p>
                  </div>

                  <div className="space-y-3 pt-1">
                    <label className="flex items-start gap-2 text-xs text-foreground cursor-pointer select-none">
                      <input
                        type="checkbox"
                        checked={clearVaultAck}
                        onChange={(e) => setClearVaultAck(e.target.checked)}
                        className="mt-0.5 rounded border-border text-destructive focus:ring-destructive"
                      />
                      <span className="text-[11px] leading-tight text-muted-foreground">
                        I understand that all local notes, scribbles, and vectors will be permanently destroyed.
                      </span>
                    </label>

                    <div className="space-y-1.5">
                      <label className="text-[11px] font-semibold text-muted-foreground">
                        Type <span className="font-mono text-destructive font-bold">CLEAR VAULT</span> to confirm:
                      </label>
                      <Input
                        value={clearVaultInput}
                        onChange={(e) => setClearVaultInput(e.target.value)}
                        placeholder="CLEAR VAULT"
                        className="h-8 text-xs font-mono"
                      />
                    </div>
                  </div>

                  <div className="flex items-center justify-end gap-2 pt-2 border-t border-border">
                    <Button
                      size="sm"
                      variant="ghost"
                      className="text-xs h-8"
                      onClick={() => setClearVaultModalOpen(false)}
                      disabled={clearingVault}
                    >
                      Cancel
                    </Button>
                    <Button
                      size="sm"
                      variant="destructive"
                      className="text-xs h-8 gap-1.5"
                      onClick={() => {
                        setClearingVault(true);
                        setTimeout(() => {
                          setClearingVault(false);
                          setClearVaultModalOpen(false);
                          setClearVaultAck(false);
                          setClearVaultInput('');
                        }, 1000);
                      }}
                      disabled={
                        !clearVaultAck ||
                        clearVaultInput.trim().toUpperCase() !== 'CLEAR VAULT' ||
                        clearingVault
                      }
                    >
                      <span>{clearingVault ? 'Clearing...' : 'Permanently Wipe Vault'}</span>
                    </Button>
                  </div>
                </div>
              </div>
            )}
          </div>
        )}

        {/* 7. TRASH SECTION */}
        {activeSection === 'meetings' && (
          <MeetingsSettings
            settings={settings}
            onChange={async (next) => {
              setSettings(next);
              try {
                await invoke('save_settings', { settings: next });
              } catch (err) {
                console.error('Failed to save meeting settings', err);
              }
            }}
          />
        )}

        {activeSection === 'capture' && <CaptureSettingsView />}

        {activeSection === 'talkback' && (
          <TalkbackSettingsView
            settings={settings}
            onChange={async (next) => {
              setSettings(next);
              try {
                await invoke('save_settings', { settings: next });
              } catch (err) {
                console.error('Failed to save Talkback settings', err);
              }
            }}
          />
        )}

        {activeSection === 'trash' && <TrashSettings />}

        {/* 8. DEVELOPER SECTION */}
        {activeSection === 'developer' && <DeveloperSettingsView />}
      </main>
    </div>
  );
};
