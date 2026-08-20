import React, { useEffect, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { AppSettings, VaultLocationInfo } from '../../types';
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
  Keyboard
} from 'lucide-react';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { Badge } from '@/components/ui/badge';
import { Switch } from '@/components/ui/switch';
import { HotkeyRecorder } from './HotkeyRecorder';

type SettingsSection = 'general' | 'providers' | 'vault' | 'account' | 'privacy';

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
};

export const ProviderSettings: React.FC = () => {
  const [activeSection, setActiveSection] = useState<SettingsSection>('providers');
  const [settings, setSettings] = useState<AppSettings>(DEFAULT_SETTINGS);
  const [loading, setLoading] = useState(true);
  const [saved, setSaved] = useState(false);
  const [error, setError] = useState('');

  // Vault & Data & Privacy state — cosmetic only for now; wiring these up to
  // real vault/export/reset commands is tracked in docs/roadmap.md, not part
  // of this settings-persistence pass.
  const [autoSaveVault, setAutoSaveVault] = useState(true);
  const [rawAudioKept, setRawAudioKept] = useState(true);

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
        setSettings((prev) => ({ ...prev, stt: { whisper_model_path: status.path } }));
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
      .then(setSettings)
      .catch((err) => {
        console.error('Failed to load settings', err);
        setError('Could not load saved settings — showing defaults');
      })
      .finally(() => setLoading(false));
  }, []);

  useEffect(() => {
    if (!loading && activeSection === 'vault') {
      loadVaultLocation();
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [loading, activeSection]);

  useEffect(() => {
    if (!loading && activeSection === 'providers' && settings.provider.active_provider === 'ollama') {
      checkLocalLlm();
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [loading, activeSection, settings.provider.active_provider]);

  useEffect(() => {
    if (!loading && activeSection === 'general') {
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

  const handleSave = async (e: React.FormEvent) => {
    e.preventDefault();
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
            SETTINGS DOMAINS
          </span>
        </div>

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

        <button
          type="button"
          onClick={() => setActiveSection('providers')}
          className={`flex items-center gap-2.5 px-3 py-2 rounded-lg text-xs font-medium transition-all text-left ${
            activeSection === 'providers'
              ? 'bg-accent text-accent-foreground font-semibold shadow-xs'
              : 'text-muted-foreground hover:bg-muted hover:text-foreground'
          }`}
        >
          <Cpu className="w-4 h-4 text-primary" />
          <span>LLM Providers</span>
        </button>

        <button
          type="button"
          onClick={() => setActiveSection('vault')}
          className={`flex items-center gap-2.5 px-3 py-2 rounded-lg text-xs font-medium transition-all text-left ${
            activeSection === 'vault'
              ? 'bg-accent text-accent-foreground font-semibold shadow-xs'
              : 'text-muted-foreground hover:bg-muted hover:text-foreground'
          }`}
        >
          <HardDrive className="w-4 h-4 text-primary" />
          <span>Vault & LanceDB</span>
        </button>

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
          <span>Account & Sync</span>
        </button>

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
          <span>Data & Privacy</span>
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

        {/* GENERAL SECTION */}
        {activeSection === 'general' && (
          <form onSubmit={handleSave} className="space-y-6">
            <div>
              <p className="font-mono text-[10px] font-bold text-muted-foreground uppercase tracking-widest mb-1">
                GENERAL CAPTURE DEFAULTS
              </p>
              <h2 className="text-lg font-bold text-foreground">Global Hotkeys, Dictation & Voice</h2>
            </div>

            <div className="space-y-4">
              <div className="py-3 border-b border-border">
                <div className="flex items-center gap-2 mb-2">
                  <Keyboard className="w-4 h-4 text-primary" />
                  <p className="text-xs font-semibold text-foreground">Global Hotkeys</p>
                </div>
                <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
                  <div>
                    <label htmlFor="show-hide-hotkey" className="block text-[11px] text-muted-foreground mb-1">
                      Show/Hide Relay (anywhere in the OS)
                    </label>
                    <HotkeyRecorder
                      id="show-hide-hotkey"
                      value={settings.hotkeys.show_hide_hotkey}
                      onCapture={(accelerator) => applyHotkey('show_hide_hotkey', accelerator)}
                    />
                  </div>
                  <div>
                    <label htmlFor="dictation-hotkey" className="block text-[11px] text-muted-foreground mb-1">
                      Universal Dictation (types into focused field)
                    </label>
                    <HotkeyRecorder
                      id="dictation-hotkey"
                      value={settings.hotkeys.dictation_hotkey}
                      onCapture={(accelerator) => applyHotkey('dictation_hotkey', accelerator)}
                    />
                  </div>
                </div>
                <p className="text-[10px] text-muted-foreground mt-2">
                  Click a hotkey box, then press the keys you want — it takes effect immediately, no restart needed.
                </p>
              </div>

              <div className="py-3 border-b border-border flex items-center justify-between">
                <div>
                  <p className="text-xs font-semibold text-foreground">Toggle-to-Talk</p>
                  <p className="text-[11px] text-muted-foreground">
                    Press the dictation hotkey once to start recording, press it again to stop — instead of holding
                    it down the whole time. Useful for longer recordings.
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

              <div className="py-3 border-b border-border">
                <p className="text-xs font-semibold text-foreground mb-1">Pill Position</p>
                <p className="text-[11px] text-muted-foreground mb-2">
                  Which edge of the screen the floating pill anchors to
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
                        setSettings({ ...settings, ui: { ...settings.ui, pill_position: opt.value } });
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

              <div className="py-3 border-b border-border">
                <div className="flex items-center gap-2 mb-2">
                  <Mic className="w-4 h-4 text-primary" />
                  <p className="text-xs font-semibold text-foreground">Local Speech-to-Text (Whisper)</p>
                </div>
                <label htmlFor="whisper-model-path" className="block text-[11px] text-muted-foreground mb-1">
                  GGML Model Path (optional — leave blank to use the auto-downloaded default)
                </label>
                <Input
                  id="whisper-model-path"
                  placeholder="Leave blank for the auto-downloaded default, or point at your own model"
                  value={settings.stt.whisper_model_path || ''}
                  onChange={(e) => setSettings({ ...settings, stt: { whisper_model_path: e.target.value } })}
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
                    Retry
                  </Button>
                </div>
                <p className="text-[10px] text-muted-foreground mt-1">
                  Relay downloads a small default Whisper model automatically the first time it's needed — required
                  for meeting/scribble capture, voice chat, and universal dictation. Point this at your own GGML
                  model (huggingface.co/ggerganov/whisper.cpp) for better accuracy or another language.
                </p>
              </div>

              <Button type="submit" size="sm" variant="default" className="mt-2">
                Save General Settings
              </Button>
            </div>
          </form>
        )}

        {/* PROVIDERS SECTION */}
        {activeSection === 'providers' && (
          <form onSubmit={handleSave} className="space-y-6">
            <div>
              <p className="font-mono text-[10px] font-bold text-muted-foreground uppercase tracking-widest mb-1">
                LLM PROVIDERS & MODEL ENGINE
              </p>
              <h2 className="text-lg font-bold text-foreground">AI Intelligence Source</h2>
            </div>

            <div className="space-y-4">
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

              {settings.provider.active_provider === 'ollama' ? (
                <div className="py-4 space-y-4">
                  <p className="font-mono text-[10px] font-bold text-muted-foreground uppercase tracking-wider">
                    OLLAMA LOCAL PARAMS
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
                  <p className="text-[10px] text-muted-foreground">
                    Relay starts Ollama and pulls the model above automatically — no manual{' '}
                    <code className="font-mono">ollama serve</code> or <code className="font-mono">ollama pull</code>{' '}
                    needed once Ollama itself is installed.
                  </p>
                </div>
              ) : (
                <div className="py-4 space-y-4">
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

              <Button type="submit" size="sm" variant="default" className="mt-2">
                Save Provider Settings
              </Button>
            </div>
          </form>
        )}

        {/* VAULT SECTION */}
        {activeSection === 'vault' && (
          <div className="space-y-6">
            <div>
              <p className="font-mono text-[10px] font-bold text-muted-foreground uppercase tracking-widest mb-1">
                LOCAL OBSIDIAN-STYLE VAULT & VECTOR RAG
              </p>
              <h2 className="text-lg font-bold text-foreground">LanceDB & Vault Management</h2>
            </div>

            <div className="space-y-4">
              <div className="py-3 border-b border-border flex items-center justify-between">
                <div>
                  <p className="text-xs font-semibold text-foreground">Auto-save Markdown Frontmatter</p>
                  <p className="text-[11px] text-muted-foreground">Persist structured headers directly to note files</p>
                </div>
                <Switch checked={autoSaveVault} onCheckedChange={setAutoSaveVault} />
              </div>

              <div className="py-3 border-b border-border flex items-center justify-between">
                <div>
                  <p className="text-xs font-semibold text-foreground">Retain Raw Audio Backstop</p>
                  <p className="text-[11px] text-muted-foreground">Keep uncompressed WAV recordings alongside note</p>
                </div>
                <Switch checked={rawAudioKept} onCheckedChange={setRawAudioKept} />
              </div>

              <div className="py-3 border-b border-border flex items-center justify-between gap-3">
                <div className="min-w-0">
                  <p className="text-xs font-semibold text-foreground">Vault Directory Location</p>
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

              <div className="py-3 flex items-center justify-between">
                <div>
                  <p className="text-xs font-semibold text-foreground">Note Retrieval</p>
                  <p className="text-[11px] text-muted-foreground">
                    Keyword-ranked search today — embedded LanceDB vector search is planned (see docs/roadmap.md)
                  </p>
                </div>
                <Badge variant="outline" className="text-xs font-mono">
                  Keyword Search
                </Badge>
              </div>
            </div>
          </div>
        )}

        {/* ACCOUNT SECTION */}
        {activeSection === 'account' && (
          <div className="space-y-6">
            <div>
              <p className="font-mono text-[10px] font-bold text-muted-foreground uppercase tracking-widest mb-1">
                HYBRID CLOUD PROFILE
              </p>
              <h2 className="text-lg font-bold text-foreground">Account & Plan Overview</h2>
            </div>

            <div className="p-4 rounded-lg bg-card border border-border flex items-center gap-4">
              <div className="w-12 h-12 rounded-full bg-primary text-primary-foreground font-extrabold text-lg flex items-center justify-center">
                N
              </div>
              <div className="space-y-0.5">
                <p className="text-sm font-bold text-foreground">Nitin Sudarshan</p>
                <p className="text-xs text-muted-foreground">nitin@example.com</p>
                <div className="flex gap-2 pt-1">
                  <Badge variant="outline" className="text-[10px] font-mono border-primary/30 text-primary">
                    Pro Hybrid Plan
                  </Badge>
                  <Badge variant="outline" className="text-[10px] font-mono">
                    Supabase Cloud Connected
                  </Badge>
                </div>
              </div>
            </div>
          </div>
        )}

        {/* DATA & PRIVACY SECTION */}
        {activeSection === 'privacy' && (
          <div className="space-y-6">
            <div>
              <p className="font-mono text-[10px] font-bold text-muted-foreground uppercase tracking-widest mb-1">
                DATA CONTROL & PRIVACY BOUNDARIES
              </p>
              <h2 className="text-lg font-bold text-foreground">Data & Privacy Control</h2>
            </div>

            <div className="space-y-4">
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
      </main>
    </div>
  );
};
