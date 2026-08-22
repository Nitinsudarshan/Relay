import React, { useEffect, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { AppSettings, LanguageSettings, VaultLocationInfo } from '../../types';
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
  Mic,
  Keyboard,
  Globe,
  Activity,
  FileText,
  Sparkles,
  Users,
  Layers,
  FileAudio,
  Check,
} from 'lucide-react';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { Badge } from '@/components/ui/badge';
import { Switch } from '@/components/ui/switch';
import { HotkeyRecorder } from './HotkeyRecorder';
import { SttDiagnosticsView } from './SttDiagnosticsView';
import { TriggerSettings } from './TriggerSettings';
import { TrashSettings } from './TrashSettings';
import { AccountSettings } from './AccountSettings';

export type SettingsSection =
  | 'account'
  | 'general'
  | 'dictation'
  | 'voicenotes'
  | 'scribbles'
  | 'meetings'
  | 'privacy'
  | 'trash'
  | 'advanced';

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
};

export const ProviderSettings: React.FC = () => {
  const [activeSection, setActiveSection] = useState<SettingsSection>('general');
  const [settings, setSettings] = useState<AppSettings>(DEFAULT_SETTINGS);
  const [loading, setLoading] = useState(true);
  const [saved, setSaved] = useState(false);
  const [error, setError] = useState('');

  // Feature Toggles & Preferences (Prepared for upcoming V1 iterations)
  const [autoSaveVault, setAutoSaveVault] = useState(true);
  const [rawAudioKept, setRawAudioKept] = useState(true);
  const [autoExtractTasks, setAutoExtractTasks] = useState(true);
  const [speakerDiarization, setSpeakerDiarization] = useState(false);
  const [meetingSummaryPrompt, setMeetingSummaryPrompt] = useState(true);
  const [scribbleTemplate, setScribbleTemplate] = useState<'structured' | 'minimal' | 'executive'>('structured');

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

  const [vaultLocation, setVaultLocation] = useState<VaultLocationInfo | null>(null);
  const [vaultBusy, setVaultBusy] = useState(false);
  const [vaultError, setVaultError] = useState('');

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
          language: {
            ...DEFAULT_LANGUAGE_SETTINGS,
            ...(payload.language || {}),
          },
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
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [loading, activeSection]);

  useEffect(() => {
    if (!loading && activeSection === 'advanced' && settings.provider.active_provider === 'ollama') {
      checkLocalLlm();
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [loading, activeSection, settings.provider.active_provider]);

  useEffect(() => {
    if (!loading && activeSection === 'advanced') {
      checkSttModel();
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [loading, activeSection]);

  // Hotkeys are applied the moment they're captured — the backend
  // unregisters the old binding and registers the new one live, so there's
  // no need to restart Relay (or even press Save) for them to take effect.
  const applyHotkey = async (field: 'show_hide_hotkey' | 'dictation_hotkey', accelerator: string) => {
    const updatedHotkeys = { ...settings.hotkeys, [field]: accelerator };
    setSettings((prev) => ({ ...prev, hotkeys: updatedHotkeys }));
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

        {/* 2. Dictation */}
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
          <span>Dictation</span>
        </button>

        {/* 3. Voice Notes */}
        <button
          type="button"
          onClick={() => setActiveSection('voicenotes')}
          className={`flex items-center gap-2.5 px-3 py-2 rounded-lg text-xs font-medium transition-all text-left ${
            activeSection === 'voicenotes'
              ? 'bg-accent text-accent-foreground font-semibold shadow-xs'
              : 'text-muted-foreground hover:bg-muted hover:text-foreground'
          }`}
        >
          <FileText className="w-4 h-4 text-primary" />
          <span>Voice Notes</span>
        </button>

        {/* 4. Scribbles */}
        <button
          type="button"
          onClick={() => setActiveSection('scribbles')}
          className={`flex items-center gap-2.5 px-3 py-2 rounded-lg text-xs font-medium transition-all text-left ${
            activeSection === 'scribbles'
              ? 'bg-accent text-accent-foreground font-semibold shadow-xs'
              : 'text-muted-foreground hover:bg-muted hover:text-foreground'
          }`}
        >
          <Sparkles className="w-4 h-4 text-primary" />
          <span>Scribbles</span>
        </button>

        {/* 5. Meetings */}
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

        {/* 6. Privacy */}
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
          <span>Privacy</span>
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

        {/* 8. Advanced */}
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
          <span>Advanced</span>
        </button>
      </aside>

      {/* Main Settings Content Area */}
      <main className="flex-1 bg-card rounded-lg border border-border p-6 overflow-y-auto min-h-0">
        {saved && (
          <div className="mb-4 p-3 rounded-lg bg-success/20 border border-success/40 text-success-foreground text-xs flex items-center justify-between">
            <span className="flex items-center gap-2">
              <CheckCircle className="w-4 h-4 text-emerald-500" />
              Settings updated successfully
            </span>
            <Badge variant="outline" className="text-[10px] font-mono">
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
              <h2 className="text-lg font-bold text-foreground">Desktop App & Vault Defaults</h2>
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
              <div className="py-3 border-b border-border flex items-center justify-between gap-3">
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

              {/* Account & Sync Card */}
              <div className="py-3">
                <div className="flex items-center gap-2 mb-3">
                  <User className="w-4 h-4 text-primary" />
                  <p className="text-xs font-semibold text-foreground">Account & Cloud Profile</p>
                </div>
                <div className="p-3.5 rounded-lg bg-card border border-border flex items-center gap-3.5">
                  <div className="w-10 h-10 rounded-full bg-primary text-primary-foreground font-bold text-sm flex items-center justify-center shrink-0">
                    N
                  </div>
                  <div className="space-y-0.5 min-w-0">
                    <p className="text-xs font-bold text-foreground truncate">Nitin Sudarshan</p>
                    <p className="text-[11px] text-muted-foreground truncate">nitin@example.com</p>
                    <div className="flex gap-1.5 pt-0.5 flex-wrap">
                      <Badge variant="outline" className="text-[9px] font-mono border-primary/30 text-primary px-1.5 py-0">
                        Pro Hybrid
                      </Badge>
                      <Badge variant="outline" className="text-[9px] font-mono px-1.5 py-0">
                        Local Vault Active
                      </Badge>
                    </div>
                  </div>
                </div>
              </div>

              <Button type="submit" size="sm" variant="default" className="mt-2">
                Save General Settings
              </Button>
            </div>
          </form>
        )}

        {/* 2. DICTATION SECTION */}
        {activeSection === 'dictation' && (
          <form onSubmit={handleSave} className="space-y-6">
            <div>
              <p className="font-mono text-[10px] font-bold text-muted-foreground uppercase tracking-widest mb-1">
                UNIVERSAL DICTATION & VOICE CAPTURE
              </p>
              <h2 className="text-lg font-bold text-foreground">Dictation, Languages & Spoken Shortcuts</h2>
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

              {/* Language & Writing Script Preferences */}
              <div className="py-4 border-b border-border space-y-4">
                <div className="flex items-center gap-2">
                  <Globe className="w-4 h-4 text-primary" />
                  <p className="text-xs font-semibold text-foreground">Language & Writing Script Preferences</p>
                </div>
                <p className="text-[11px] text-muted-foreground">
                  Separately configure the languages you speak, your default dictation language, and output writing script.
                </p>

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

                {/* Contextual orthography example card */}
                <div className="p-3 rounded-lg bg-muted/40 border border-border text-[11px] space-y-1.5">
                  <p className="font-semibold text-foreground text-[11px]">
                    Writing Script vs. Spoken Language
                  </p>
                  <p className="text-muted-foreground text-[10px] leading-relaxed">
                    Writing script dictates the alphabet used for transcriptions and notes regardless of spoken language.
                  </p>
                  <div className="grid grid-cols-1 sm:grid-cols-2 gap-2 pt-1 font-mono text-[10px]">
                    <div className="p-2 rounded bg-background border border-border">
                      <span className="text-muted-foreground block text-[9px] font-sans font-medium uppercase tracking-wider mb-0.5">
                        Hindi + Latin Script (Romanized)
                      </span>
                      <span className="text-foreground">"Kal meeting hai"</span>
                    </div>
                    <div className="p-2 rounded bg-background border border-border">
                      <span className="text-muted-foreground block text-[9px] font-sans font-medium uppercase tracking-wider mb-0.5">
                        Hindi + Native Script (Devanagari)
                      </span>
                      <span className="text-foreground">"कल मीटिंग है"</span>
                    </div>
                  </div>
                </div>
              </div>

              {/* Trigger Phrases & Actions */}
              <div className="py-2 border-b border-border">
                <TriggerSettings />
              </div>

              <Button type="submit" size="sm" variant="default" className="mt-2">
                Save Dictation Settings
              </Button>
            </div>
          </form>
        )}

        {/* 3. VOICE NOTES SECTION */}
        {activeSection === 'voicenotes' && (
          <form onSubmit={handleSave} className="space-y-6">
            <div>
              <p className="font-mono text-[10px] font-bold text-muted-foreground uppercase tracking-widest mb-1">
                VOICE NOTES ENGINE
              </p>
              <h2 className="text-lg font-bold text-foreground">Audio Capture, Summarization & Vault Generation</h2>
            </div>

            <div className="space-y-4">
              {/* Notes & Summarization Language */}
              <div className="py-3 border-b border-border">
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

              {/* Auto-save Markdown Frontmatter */}
              <div className="py-3 border-b border-border flex items-center justify-between">
                <div>
                  <p className="text-xs font-semibold text-foreground">Auto-save YAML Frontmatter</p>
                  <p className="text-[11px] text-muted-foreground">
                    Persist structured headers (title, date, tags, speakers) directly into Obsidian-compatible markdown notes.
                  </p>
                </div>
                <Switch checked={autoSaveVault} onCheckedChange={setAutoSaveVault} />
              </div>

              {/* Note Retrieval Index Mode */}
              <div className="py-3 border-b border-border flex items-center justify-between">
                <div>
                  <p className="text-xs font-semibold text-foreground">Note Retrieval Index</p>
                  <p className="text-[11px] text-muted-foreground">
                    Keyword-ranked local search with hybrid vector embeddings backstop.
                  </p>
                </div>
                <Badge variant="outline" className="text-xs font-mono">
                  Keyword Search Active
                </Badge>
              </div>

              {/* Voice Notes V1 Preview & Defaults */}
              <div className="p-4 rounded-lg bg-muted/40 border border-border space-y-3">
                <div className="flex items-center gap-2">
                  <FileAudio className="w-4 h-4 text-primary" />
                  <p className="text-xs font-semibold text-foreground">Voice Note Capture Invariants</p>
                </div>
                <p className="text-[11px] text-muted-foreground leading-relaxed">
                  Voice Notes record with high-fidelity 16kHz mono audio, auto-detect silence boundaries via calibrated VAD,
                  and generate structured markdown files written directly to your local Obsidian vault.
                </p>
                <div className="grid grid-cols-1 sm:grid-cols-2 gap-2 pt-1">
                  <div className="p-2.5 rounded-lg bg-card border border-border text-[11px]">
                    <span className="font-semibold text-foreground block mb-0.5">Dual Surface Access</span>
                    <span className="text-[10px] text-muted-foreground">History view in app + Instant floating pill recording.</span>
                  </div>
                  <div className="p-2.5 rounded-lg bg-card border border-border text-[11px]">
                    <span className="font-semibold text-foreground block mb-0.5">Local Audio Retention</span>
                    <span className="text-[10px] text-muted-foreground">Raw audio preserved in vault alongside markdown notes.</span>
                  </div>
                </div>
              </div>

              <Button type="submit" size="sm" variant="default" className="mt-2">
                Save Voice Notes Settings
              </Button>
            </div>
          </form>
        )}

        {/* 4. SCRIBBLES SECTION */}
        {activeSection === 'scribbles' && (
          <form onSubmit={handleSave} className="space-y-6">
            <div>
              <p className="font-mono text-[10px] font-bold text-muted-foreground uppercase tracking-widest mb-1">
                SCRIBBLES & EXTRACTION
              </p>
              <h2 className="text-lg font-bold text-foreground">Structured Notes & Action Items Engine</h2>
            </div>

            <div className="space-y-4">
              {/* Note Formatting Template */}
              <div className="py-3 border-b border-border">
                <p className="text-xs font-semibold text-foreground mb-1">Structured Note Template</p>
                <p className="text-[11px] text-muted-foreground mb-2">
                  Default markdown template applied when formatting voice thoughts into polished scribbles.
                </p>
                <div className="flex bg-muted p-1 rounded-lg border border-border w-fit">
                  {[
                    { id: 'structured', label: 'Full Structure' },
                    { id: 'minimal', label: 'Minimal Bullets' },
                    { id: 'executive', label: 'Executive Memo' },
                  ].map((tpl) => (
                    <button
                      key={tpl.id}
                      type="button"
                      onClick={() => setScribbleTemplate(tpl.id as any)}
                      className={`px-3 py-1 text-xs font-medium rounded-lg transition-all ${
                        scribbleTemplate === tpl.id
                          ? 'bg-card text-foreground font-semibold shadow-xs'
                          : 'text-muted-foreground hover:text-foreground'
                      }`}
                    >
                      {tpl.label}
                    </button>
                  ))}
                </div>
              </div>

              {/* Kanban Task Extraction */}
              <div className="py-3 border-b border-border flex items-center justify-between">
                <div>
                  <p className="text-xs font-semibold text-foreground">Auto-Extract Kanban Tasks</p>
                  <p className="text-[11px] text-muted-foreground">
                    Detect actionable commitments in voice notes and automatically create Kanban cards.
                  </p>
                </div>
                <Switch checked={autoExtractTasks} onCheckedChange={setAutoExtractTasks} />
              </div>

              {/* Scribbles V1 Feature Preview Card */}
              <div className="p-4 rounded-lg bg-muted/40 border border-border space-y-3">
                <div className="flex items-center gap-2">
                  <Sparkles className="w-4 h-4 text-primary" />
                  <p className="text-xs font-semibold text-foreground">Scribbles V1 Readiness</p>
                </div>
                <p className="text-[11px] text-muted-foreground leading-relaxed">
                  Scribbles combines instant speech transcription with prompt-driven LLM restructuring.
                  Upcoming V1 updates will add customized formatting templates, inline editing, and live sync.
                </p>
                <div className="flex items-center gap-2 text-[10px] text-muted-foreground font-mono">
                  <Badge variant="outline" className="px-2 py-0.5">Raw Audio Backstop: Active</Badge>
                  <Badge variant="outline" className="px-2 py-0.5">Kanban Sync: Ready</Badge>
                </div>
              </div>

              <Button type="submit" size="sm" variant="default" className="mt-2">
                Save Scribble Settings
              </Button>
            </div>
          </form>
        )}

        {/* 5. MEETINGS SECTION */}
        {activeSection === 'meetings' && (
          <form onSubmit={handleSave} className="space-y-6">
            <div>
              <p className="font-mono text-[10px] font-bold text-muted-foreground uppercase tracking-widest mb-1">
                MEETING RECORDER & INTELLIGENCE
              </p>
              <h2 className="text-lg font-bold text-foreground">Long-Form Capture & Diarization Preferences</h2>
            </div>

            <div className="space-y-4">
              {/* Speaker Diarization Switch */}
              <div className="py-3 border-b border-border flex items-center justify-between">
                <div>
                  <p className="text-xs font-semibold text-foreground">Speaker Diarization & Labeling</p>
                  <p className="text-[11px] text-muted-foreground">
                    Distinguish multiple speakers in meeting transcripts (Speaker 1, Speaker 2).
                  </p>
                </div>
                <Switch checked={speakerDiarization} onCheckedChange={setSpeakerDiarization} />
              </div>

              {/* Auto Meeting Minutes & Actions */}
              <div className="py-3 border-b border-border flex items-center justify-between">
                <div>
                  <p className="text-xs font-semibold text-foreground">Generate Meeting Minutes & Next Steps</p>
                  <p className="text-[11px] text-muted-foreground">
                    Automatically extract key decisions, unresolved questions, and assignees from recorded meetings.
                  </p>
                </div>
                <Switch checked={meetingSummaryPrompt} onCheckedChange={setMeetingSummaryPrompt} />
              </div>

              {/* Meetings V1 Feature Card */}
              <div className="p-4 rounded-lg bg-muted/40 border border-border space-y-3">
                <div className="flex items-center gap-2">
                  <Users className="w-4 h-4 text-primary" />
                  <p className="text-xs font-semibold text-foreground">Meetings V1 Preparation</p>
                </div>
                <p className="text-[11px] text-muted-foreground leading-relaxed">
                  Long-form meeting recording captures system audio and microphone streams with continuous chunked Whisper transcription.
                  Full UI and recording controls will arrive in Phase 11B/11C.
                </p>
                <div className="flex items-center gap-2 text-[10px] text-muted-foreground font-mono">
                  <Badge variant="outline" className="px-2 py-0.5">Continuous Buffer: 16kHz</Badge>
                  <Badge variant="outline" className="px-2 py-0.5">Target Folder: /Meetings</Badge>
                </div>
              </div>

              <Button type="submit" size="sm" variant="default" className="mt-2">
                Save Meeting Settings
              </Button>
            </div>
          </form>
        )}

        {/* 6. PRIVACY SECTION */}
        {activeSection === 'privacy' && (
          <div className="space-y-6">
            <div>
              <p className="font-mono text-[10px] font-bold text-muted-foreground uppercase tracking-widest mb-1">
                DATA CONTROL & PRIVACY BOUNDARIES
              </p>
              <h2 className="text-lg font-bold text-foreground">Data Ownership & Vault Isolation</h2>
            </div>

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

              {/* Raw Audio Backstop Retain */}
              <div className="py-3 border-b border-border flex items-center justify-between">
                <div>
                  <p className="text-xs font-semibold text-foreground">Retain Raw Audio Backstop</p>
                  <p className="text-[11px] text-muted-foreground">Keep uncompressed WAV recordings alongside generated markdown notes</p>
                </div>
                <Switch checked={rawAudioKept} onCheckedChange={setRawAudioKept} />
              </div>

              {/* Safe Export Action */}
              <div className="p-4 rounded-lg bg-card border border-border flex items-center justify-between">
                <div>
                  <p className="text-xs font-bold text-foreground">Export All Vault Data</p>
                  <p className="text-[11px] text-muted-foreground">Download full backup of notes, tasks, and LanceDB embeddings</p>
                </div>
                <Button variant="default" size="sm" className="gap-2">
                  <Download className="w-4 h-4" />
                  <span>Export Everything</span>
                </Button>
              </div>

              {/* Destructive Outlined Actions */}
              <div className="p-4 rounded-lg border border-destructive/40 bg-destructive/5 space-y-3">
                <div className="flex items-center gap-2 text-destructive font-bold text-xs">
                  <AlertTriangle className="w-4 h-4 shrink-0" />
                  <span>Irreversible Data Reset Actions</span>
                </div>

                <div className="py-2 border-t border-destructive/20 flex items-center justify-between">
                  <div>
                    <p className="text-xs font-semibold text-foreground">Clear Local Vault & Index</p>
                    <p className="text-[11px] text-muted-foreground">Deletes all stored markdown files and LanceDB table</p>
                  </div>
                  <Button variant="outline" size="sm" className="border-destructive/50 text-destructive hover:bg-destructive/10 gap-1.5 text-xs">
                    <Trash2 className="w-3.5 h-3.5" />
                    <span>Clear Vault</span>
                  </Button>
                </div>

                <div className="py-2 border-t border-destructive/20 flex items-center justify-between">
                  <div>
                    <p className="text-xs font-semibold text-foreground">Disconnect Hybrid Cloud Sync</p>
                    <p className="text-[11px] text-muted-foreground">Reverts app to 100% offline local-only operating mode</p>
                  </div>
                  <Button variant="outline" size="sm" className="border-destructive/50 text-destructive hover:bg-destructive/10 gap-1.5 text-xs">
                    <Cloud className="w-3.5 h-3.5" />
                    <span>Disconnect Sync</span>
                  </Button>
                </div>
              </div>
            </div>
          </div>
        )}

        {/* 7. ADVANCED SECTION */}
        {activeSection === 'advanced' && (
          <div className="space-y-8">
            <form onSubmit={handleSave} className="space-y-6">
              <div>
                <p className="font-mono text-[10px] font-bold text-muted-foreground uppercase tracking-widest mb-1">
                  ADVANCED ENGINE & DIAGNOSTICS
                </p>
                <h2 className="text-lg font-bold text-foreground">AI Intelligence Source & Local STT Engine</h2>
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
                <div className="py-3 border-b border-border">
                  <div className="flex items-center gap-2 mb-2">
                    <Mic className="w-4 h-4 text-primary" />
                    <p className="text-xs font-semibold text-foreground">Speech-to-Text Model (Whisper)</p>
                  </div>
                  <label htmlFor="whisper-model-path" className="block text-[11px] text-muted-foreground mb-1">
                    GGML Model Path (optional — leave blank to use the auto-downloaded default)
                  </label>
                  <Input
                    id="whisper-model-path"
                    placeholder="Leave blank for the auto-downloaded default, or point at your own model"
                    value={settings.stt.whisper_model_path || ''}
                    onChange={(e) => setSettings({ ...settings, stt: { ...settings.stt, whisper_model_path: e.target.value } })}
                  />
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

        {/* 7. TRASH SECTION */}
        {activeSection === 'trash' && <TrashSettings />}
      </main>
    </div>
  );
};
