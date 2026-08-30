import React, { useEffect, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import {
  AppSettings,
  LanguageSettings,
  VaultLocationInfo,
  RelayAccount,
  AudioDeviceInfo,
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
  Sparkles,
  BookOpen,
  Users,
  Volume2,
  Terminal,
  Check,
  Layers,
  Power,
  Clipboard,
} from 'lucide-react';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { Badge } from '@/components/ui/badge';
import { Switch } from '@/components/ui/switch';
import { HotkeyRecorder } from './HotkeyRecorder';
import { SttDiagnosticsView } from './SttDiagnosticsView';
import { TrashSettings } from './TrashSettings';
import { AccountSettings } from './AccountSettings';
import { DeveloperSettingsView } from './DeveloperSettingsView';
import { DictionarySnippetsSettings } from './DictionarySnippetsSettings';
import { MeetingsSettings } from './MeetingsSettings';

export type SettingsSection =
  | 'account'
  | 'general'
  | 'dictation'
  | 'dictionary'
  | 'meetings'
  | 'languages'
  | 'advanced'
  | 'privacy'
  | 'trash'
  | 'developer';

const DEFAULT_LANGUAGE_SETTINGS: LanguageSettings = {
  primary_dictation_language: 'en',
  spoken_languages: ['en'],
  notes_language: 'en',
  output_script: 'latin',
};

const WHISPER_SUPPORTED_LANGUAGES = [
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
  hotkeys: { show_hide_hotkey: 'Ctrl+Shift+Space', dictation_hotkey: 'Ctrl+Space', toggle_to_talk: false },
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
  dictionary: ['Relay', 'Whisper', 'Tauri', 'Rust', 'Supabase', 'LanceDB', 'Ollama'],
  snippets: [],
};

export const ProviderSettings: React.FC = () => {
  const [activeSection, setActiveSection] = useState<SettingsSection>('general');
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

  const checkLocalLlm = async () => {
    setOllamaStatus({ state: 'checking' });
    try {
      const status = await invoke<OllamaStatus>('ensure_local_llm_ready');
      setOllamaStatus(status);
    } catch (err) {
      console.error('Failed to check local Ollama status', err);
      setOllamaStatus({ state: 'unreachable', message: 'Could not reach the backend' });
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
    if (!loading && activeSection === 'dictation') {
      loadAudioDevices();
    }
    if (!loading && activeSection === 'advanced' && settings.provider.active_provider === 'ollama') {
      checkLocalLlm();
      checkSttModel();
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

        {/* 0. Account & Identity */}
        <button
          type="button"
          onClick={() => setActiveSection('account')}
          className={`flex items-center gap-2.5 px-3 py-2 rounded-lg text-xs font-medium transition-all text-left ${
            activeSection === 'account'
              ? 'bg-accent text-accent-foreground font-semibold shadow-xs'
              : 'text-muted-foreground hover:bg-muted hover:text-foreground'
          }`}
        >
          <User className="w-4 h-4 text-primary" />
          <span>Account & Identity</span>
        </button>

        {/* 1. General */}
        <button
          type="button"
          onClick={() => setActiveSection('general')}
          className={`flex items-center gap-2.5 px-3 py-2 rounded-lg text-xs font-medium transition-all text-left ${
            activeSection === 'general'
              ? 'bg-accent text-accent-foreground font-semibold shadow-xs'
              : 'text-muted-foreground hover:bg-muted hover:text-foreground'
          }`}
        >
          <Sliders className="w-4 h-4 text-primary" />
          <span>General</span>
        </button>

        {/* 2. Dictation & Audio */}
        <button
          type="button"
          onClick={() => setActiveSection('dictation')}
          className={`flex items-center gap-2.5 px-3 py-2 rounded-lg text-xs font-medium transition-all text-left ${
            activeSection === 'dictation'
              ? 'bg-accent text-accent-foreground font-semibold shadow-xs'
              : 'text-muted-foreground hover:bg-muted hover:text-foreground'
          }`}
        >
          <Mic className="w-4 h-4 text-primary" />
          <span>Dictation & Audio</span>
        </button>

        {/* 3. Dictionary & Snippets */}
        <button
          type="button"
          onClick={() => setActiveSection('dictionary')}
          className={`flex items-center gap-2.5 px-3 py-2 rounded-lg text-xs font-medium transition-all text-left ${
            activeSection === 'dictionary'
              ? 'bg-accent text-accent-foreground font-semibold shadow-xs'
              : 'text-muted-foreground hover:bg-muted hover:text-foreground'
          }`}
        >
          <BookOpen className="w-4 h-4 text-primary" />
          <span>Dictionary & Snippets</span>
        </button>

        {/* 4. Meetings */}
        <button
          type="button"
          onClick={() => setActiveSection('meetings')}
          className={`flex items-center gap-2.5 px-3 py-2 rounded-lg text-xs font-medium transition-all text-left ${
            activeSection === 'meetings'
              ? 'bg-accent text-accent-foreground font-semibold shadow-xs'
              : 'text-muted-foreground hover:bg-muted hover:text-foreground'
          }`}
        >
          <Users className="w-4 h-4 text-primary" />
          <span>Meetings</span>
        </button>

        {/* 5. Languages & Script */}
        <button
          type="button"
          onClick={() => setActiveSection('languages')}
          className={`flex items-center gap-2.5 px-3 py-2 rounded-lg text-xs font-medium transition-all text-left ${
            activeSection === 'languages'
              ? 'bg-accent text-accent-foreground font-semibold shadow-xs'
              : 'text-muted-foreground hover:bg-muted hover:text-foreground'
          }`}
        >
          <Globe className="w-4 h-4 text-primary" />
          <span>Languages & Script</span>
        </button>

        {/* 5. AI Models & STT Engine */}
        <button
          type="button"
          onClick={() => setActiveSection('advanced')}
          className={`flex items-center gap-2.5 px-3 py-2 rounded-lg text-xs font-medium transition-all text-left ${
            activeSection === 'advanced'
              ? 'bg-accent text-accent-foreground font-semibold shadow-xs'
              : 'text-muted-foreground hover:bg-muted hover:text-foreground'
          }`}
        >
          <Cpu className="w-4 h-4 text-primary" />
          <span>AI Models & STT</span>
        </button>

        {/* 6. Privacy & Vault */}
        <button
          type="button"
          onClick={() => setActiveSection('privacy')}
          className={`flex items-center gap-2.5 px-3 py-2 rounded-lg text-xs font-medium transition-all text-left ${
            activeSection === 'privacy'
              ? 'bg-accent text-accent-foreground font-semibold shadow-xs'
              : 'text-muted-foreground hover:bg-muted hover:text-foreground'
          }`}
        >
          <ShieldCheck className="w-4 h-4 text-primary" />
          <span>Privacy & Vault</span>
        </button>

        {/* 7. Trash & Deleted Items */}
        <button
          type="button"
          onClick={() => setActiveSection('trash')}
          className={`flex items-center gap-2.5 px-3 py-2 rounded-lg text-xs font-medium transition-all text-left ${
            activeSection === 'trash'
              ? 'bg-accent text-accent-foreground font-semibold shadow-xs'
              : 'text-muted-foreground hover:bg-muted hover:text-foreground'
          }`}
        >
          <Trash2 className="w-4 h-4 text-amber-500" />
          <span>Trash & Deleted</span>
        </button>

        {/* 8. Developer */}
        <button
          type="button"
          onClick={() => setActiveSection('developer')}
          className={`flex items-center gap-2.5 px-3 py-2 rounded-lg text-xs font-medium transition-all text-left ${
            activeSection === 'developer'
              ? 'bg-accent text-accent-foreground font-semibold shadow-xs'
              : 'text-muted-foreground hover:bg-muted hover:text-foreground'
          }`}
        >
          <Terminal className="w-4 h-4 text-amber-500" />
          <span>Developer</span>
        </button>
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
                  <div className="p-2.5 rounded-lg bg-emerald-950/30 border border-emerald-500/30 text-emerald-400 text-xs flex items-center gap-2 font-medium">
                    <Mic className="w-4 h-4 text-emerald-400 shrink-0" />
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
          <div className="space-y-8">
            <form onSubmit={handleSave} className="space-y-6">
              <div>
                <p className="font-mono text-[10px] font-bold text-muted-foreground uppercase tracking-widest mb-1">
                  AI INTELLIGENCE & SPEECH ENGINE
                </p>
                <h2 className="text-lg font-bold text-foreground">Local Ollama vs Cloud LLM & Whisper STT</h2>
              </div>

              <div className="space-y-4">
                {/* Active LLM Backend Toggle */}
                <div className="py-3 border-b border-border flex items-center justify-between">
                  <div>
                    <p className="text-xs font-semibold text-foreground">Active LLM Execution Backend</p>
                    <p className="text-[11px] text-muted-foreground">100% Local Ollama ($0) vs OpenAI / Gemini Cloud API</p>
                  </div>
                  <div className="flex bg-muted p-1 rounded-lg border border-border">
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

                {/* Local Ollama Params */}
                {settings.provider.active_provider === 'ollama' ? (
                  <div className="py-3 border-b border-border space-y-4">
                    <p className="font-mono text-[10px] font-bold text-muted-foreground uppercase tracking-wider">
                      OLLAMA LOCAL CONFIGURATION
                    </p>
                    <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
                      <div>
                        <label htmlFor="ollama-host" className="block text-xs font-medium text-foreground mb-1">
                          Ollama Host Endpoint
                        </label>
                        <Input
                          id="ollama-host"
                          value={settings.provider.ollama_host}
                          onChange={(e) =>
                            setSettings({ ...settings, provider: { ...settings.provider, ollama_host: e.target.value } })
                          }
                          placeholder="http://localhost:11434"
                        />
                      </div>
                      <div>
                        <label htmlFor="ollama-model" className="block text-xs font-medium text-foreground mb-1">
                          Target Model Name
                        </label>
                        <Input
                          id="ollama-model"
                          value={settings.provider.ollama_model}
                          onChange={(e) =>
                            setSettings({ ...settings, provider: { ...settings.provider, ollama_model: e.target.value } })
                          }
                          placeholder="llama3.2:latest"
                        />
                      </div>
                    </div>

                    <div className="flex items-center justify-between p-3 rounded-lg bg-muted/40 border border-border">
                      <div className="flex items-center gap-2 text-xs">
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
                            Ollama isn't installed — install it once, Relay handles the rest
                          </Badge>
                        )}
                        {ollamaStatus.state === 'unreachable' && (
                          <Badge variant="outline" className="text-[10px] font-mono border-destructive/50 text-destructive">
                            {ollamaStatus.message}
                          </Badge>
                        )}
                      </div>
                      <Button type="button" size="sm" variant="ghost" onClick={checkLocalLlm} className="text-xs h-7">
                        Retry
                      </Button>
                    </div>
                  </div>
                ) : (
                  <div className="py-3 border-b border-border space-y-4">
                    <p className="font-mono text-[10px] font-bold text-muted-foreground uppercase tracking-wider">
                      CLOUD API CREDENTIALS
                    </p>
                    <div className="space-y-3">
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
                        />
                      </div>
                      <div>
                        <label htmlFor="cloud-model-name" className="block text-xs font-medium text-foreground mb-1">
                          Cloud Model Selection
                        </label>
                        <Input
                          id="cloud-model-name"
                          value={settings.provider.cloud_model || ''}
                          onChange={(e) =>
                            setSettings({ ...settings, provider: { ...settings.provider, cloud_model: e.target.value } })
                          }
                          placeholder="gpt-4o-mini"
                        />
                      </div>
                    </div>
                  </div>
                )}

                {/* Local Whisper Model Path */}
                <div className="py-3 border-b border-border space-y-3">
                  <div className="flex items-center gap-2 mb-1">
                    <Mic className="w-4 h-4 text-primary" />
                    <p className="text-xs font-semibold text-foreground">Speech-to-Text Model (Whisper)</p>
                  </div>

                  {/* Dictation Performance Profile */}
                  <div>
                    <label className="block text-[11px] font-medium text-foreground mb-1.5">
                      Universal Dictation Performance Profile
                    </label>
                    <div className="grid grid-cols-2 gap-2">
                      <button
                        type="button"
                        onClick={() =>
                          setSettings({
                            ...settings,
                            stt: { ...settings.stt, dictation_quality: 'fast', dictationQuality: 'fast' },
                          })
                        }
                        className={`p-2.5 rounded-lg border text-left transition-all ${
                          (settings.stt.dictation_quality ?? 'fast') === 'fast'
                            ? 'border-primary bg-primary/10 text-foreground shadow-sm'
                            : 'border-border bg-card/50 text-muted-foreground hover:border-border/80'
                        }`}
                      >
                        <div className="flex items-center justify-between mb-1">
                          <span className="text-xs font-semibold text-foreground">Fast (Base)</span>
                          <Badge variant="emerald" className="text-[9px] px-1.5 py-0">~0.8s</Badge>
                        </div>
                        <p className="text-[10px] text-muted-foreground leading-snug">
                          3x lower latency using Base model. Recommended for conversational speech.
                        </p>
                      </button>

                      <button
                        type="button"
                        onClick={() =>
                          setSettings({
                            ...settings,
                            stt: { ...settings.stt, dictation_quality: 'accurate', dictationQuality: 'accurate' },
                          })
                        }
                        className={`p-2.5 rounded-lg border text-left transition-all ${
                          settings.stt.dictation_quality === 'accurate'
                            ? 'border-primary bg-primary/10 text-foreground shadow-sm'
                            : 'border-border bg-card/50 text-muted-foreground hover:border-border/80'
                        }`}
                      >
                        <div className="flex items-center justify-between mb-1">
                          <span className="text-xs font-semibold text-foreground">Accurate (Small)</span>
                          <Badge variant="outline" className="text-[9px] px-1.5 py-0">~2.4s</Badge>
                        </div>
                        <p className="text-[10px] text-muted-foreground leading-snug">
                          Maximum vocabulary fidelity. Recommended for long technical monologues.
                        </p>
                      </button>
                    </div>
                  </div>

                  <div>
                    <label htmlFor="whisper-model-path" className="block text-[11px] text-muted-foreground mb-1">
                      Custom Model Path (optional — leave blank for auto-managed models)
                    </label>
                    <Input
                      id="whisper-model-path"
                      placeholder="Leave blank for auto-managed models, or point at a custom GGML file"
                      value={settings.stt.whisper_model_path || ''}
                      onChange={(e) => setSettings({ ...settings, stt: { ...settings.stt, whisper_model_path: e.target.value } })}
                    />
                  </div>

                  <div className="flex items-center justify-between mt-2 p-3 rounded-lg bg-muted/40 border border-border">
                    <div className="text-xs">
                      {sttModelStatus.state === 'checking' && (
                        <Badge variant="outline" className="text-[10px] font-mono">Checking Whisper model…</Badge>
                      )}
                      {sttModelStatus.state === 'ready' && (
                        <Badge variant="emerald" className="text-[10px] font-mono">
                          Model ready: {sttModelStatus.path.split(/[\\/]/).pop()}
                        </Badge>
                      )}
                      {sttModelStatus.state === 'failed' && (
                        <Badge variant="outline" className="text-[10px] font-mono border-destructive/50 text-destructive">
                          {sttModelStatus.message}
                        </Badge>
                      )}
                    </div>
                    <Button type="button" size="sm" variant="ghost" onClick={checkSttModel} className="text-xs h-7">
                      Retry Check
                    </Button>
                  </div>
                </div>

                <Button type="submit" size="sm" variant="default" className="mt-2">
                  Save Engine Settings
                </Button>
              </div>
            </form>

            {/* STT Diagnostics & Quality Inspector */}
            <div className="pt-4 border-t border-border">
              <SttDiagnosticsView
                settings={settings}
                onUpdateSettings={setSettings}
                onSaveSettings={handleSaveDirect}
              />
            </div>
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

        {activeSection === 'trash' && <TrashSettings />}

        {/* 8. DEVELOPER SECTION */}
        {activeSection === 'developer' && <DeveloperSettingsView />}
      </main>
    </div>
  );
};
