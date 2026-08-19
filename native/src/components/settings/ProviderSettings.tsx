import React, { useEffect, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { AppSettings } from '../../types';
import {
  Cpu,
  Cloud,
  CheckCircle,
  Sliders,
  Zap,
  ShieldCheck,
  HardDrive,
  User,
  Trash2,
  Download,
  AlertTriangle,
  Mic,
  Volume2,
  Keyboard
} from 'lucide-react';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { Badge } from '@/components/ui/badge';
import { Switch } from '@/components/ui/switch';
import { TriggerSettings } from './TriggerSettings';

type SettingsSection = 'general' | 'providers' | 'triggers' | 'vault' | 'account' | 'privacy';

const DEFAULT_SETTINGS: AppSettings = {
  provider: {
    active_provider: 'ollama',
    ollama_host: 'http://localhost:11434',
    ollama_model: 'llama3.2:latest',
    cloud_model: 'gpt-4o-mini',
  },
  stt: { whisper_model_path: '' },
  tts: { piper_binary_path: '', piper_voice_path: '' },
  hotkeys: { show_hide_hotkey: 'Ctrl+Shift+Space', dictation_hotkey: 'Ctrl+Space' },
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

  useEffect(() => {
    invoke<AppSettings>('get_settings')
      .then(setSettings)
      .catch((err) => {
        console.error('Failed to load settings', err);
        setError('Could not load saved settings — showing defaults');
      })
      .finally(() => setLoading(false));
  }, []);

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
      <div className="flex-1 flex items-center justify-center bg-card rounded-2xl border border-border text-xs text-muted-foreground">
        Loading settings…
      </div>
    );
  }

  return (
    <div className="flex-1 flex flex-col md:flex-row gap-6 min-h-0 overflow-hidden">
      {/* Settings Sub-Nav Sidebar */}
      <aside className="w-full md:w-56 flex flex-col shrink-0 gap-1 bg-card p-3 rounded-2xl border border-border">
        <div className="px-3 py-2 mb-1">
          <span className="font-mono text-[10px] font-bold tracking-widest text-muted-foreground uppercase">
            SETTINGS DOMAINS
          </span>
        </div>

        <button
          type="button"
          onClick={() => setActiveSection('general')}
          className={`flex items-center gap-2.5 px-3 py-2 rounded-xl text-xs font-medium transition-all text-left ${
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
          className={`flex items-center gap-2.5 px-3 py-2 rounded-xl text-xs font-medium transition-all text-left ${
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
          onClick={() => setActiveSection('triggers')}
          className={`flex items-center gap-2.5 px-3 py-2 rounded-xl text-xs font-medium transition-all text-left ${
            activeSection === 'triggers'
              ? 'bg-accent text-accent-foreground font-semibold shadow-xs'
              : 'text-muted-foreground hover:bg-muted hover:text-foreground'
          }`}
        >
          <Zap className="w-4 h-4 text-primary" />
          <span>Triggers & MCP</span>
        </button>

        <button
          type="button"
          onClick={() => setActiveSection('vault')}
          className={`flex items-center gap-2.5 px-3 py-2 rounded-xl text-xs font-medium transition-all text-left ${
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
          className={`flex items-center gap-2.5 px-3 py-2 rounded-xl text-xs font-medium transition-all text-left ${
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
          className={`flex items-center gap-2.5 px-3 py-2 rounded-xl text-xs font-medium transition-all text-left ${
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
      <main className="flex-1 bg-card rounded-2xl border border-border p-6 overflow-y-auto min-h-0">
        {saved && (
          <div className="mb-4 p-3 rounded-xl bg-success/20 border border-success/40 text-success-foreground text-xs flex items-center justify-between">
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
                    <Input
                      id="show-hide-hotkey"
                      value={settings.hotkeys.show_hide_hotkey}
                      onChange={(e) =>
                        setSettings({ ...settings, hotkeys: { ...settings.hotkeys, show_hide_hotkey: e.target.value } })
                      }
                    />
                  </div>
                  <div>
                    <label htmlFor="dictation-hotkey" className="block text-[11px] text-muted-foreground mb-1">
                      Universal Dictation (hold to talk, types into focused field)
                    </label>
                    <Input
                      id="dictation-hotkey"
                      value={settings.hotkeys.dictation_hotkey}
                      onChange={(e) =>
                        setSettings({ ...settings, hotkeys: { ...settings.hotkeys, dictation_hotkey: e.target.value } })
                      }
                    />
                  </div>
                </div>
                <p className="text-[10px] text-muted-foreground mt-2">
                  Restart Relay after changing hotkeys for them to take effect.
                </p>
              </div>

              <div className="py-3 border-b border-border">
                <div className="flex items-center gap-2 mb-2">
                  <Mic className="w-4 h-4 text-primary" />
                  <p className="text-xs font-semibold text-foreground">Local Speech-to-Text (Whisper)</p>
                </div>
                <label htmlFor="whisper-model-path" className="block text-[11px] text-muted-foreground mb-1">
                  GGML Model Path
                </label>
                <Input
                  id="whisper-model-path"
                  placeholder="C:\\models\\ggml-base.en.bin"
                  value={settings.stt.whisper_model_path || ''}
                  onChange={(e) => setSettings({ ...settings, stt: { whisper_model_path: e.target.value } })}
                />
                <p className="text-[10px] text-muted-foreground mt-1">
                  Download a GGML model from huggingface.co/ggerganov/whisper.cpp — required for meeting/scribble
                  capture, voice chat, and universal dictation.
                </p>
              </div>

              <div className="py-3 border-b border-border">
                <div className="flex items-center gap-2 mb-2">
                  <Volume2 className="w-4 h-4 text-primary" />
                  <p className="text-xs font-semibold text-foreground">Local Text-to-Speech (Piper) — optional</p>
                </div>
                <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
                  <div>
                    <label htmlFor="piper-binary-path" className="block text-[11px] text-muted-foreground mb-1">
                      Piper Binary Path
                    </label>
                    <Input
                      id="piper-binary-path"
                      placeholder="C:\\piper\\piper.exe"
                      value={settings.tts.piper_binary_path || ''}
                      onChange={(e) =>
                        setSettings({ ...settings, tts: { ...settings.tts, piper_binary_path: e.target.value } })
                      }
                    />
                  </div>
                  <div>
                    <label htmlFor="piper-voice-path" className="block text-[11px] text-muted-foreground mb-1">
                      Voice Model Path
                    </label>
                    <Input
                      id="piper-voice-path"
                      placeholder="C:\\piper\\en_US-lessac-medium.onnx"
                      value={settings.tts.piper_voice_path || ''}
                      onChange={(e) =>
                        setSettings({ ...settings, tts: { ...settings.tts, piper_voice_path: e.target.value } })
                      }
                    />
                  </div>
                </div>
                <p className="text-[10px] text-muted-foreground mt-1">
                  Leave blank to skip "speak back" in voice chat — answers still show as text.
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
                <div className="flex bg-muted p-1 rounded-xl border border-border">
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

        {/* TRIGGERS SECTION */}
        {activeSection === 'triggers' && <TriggerSettings />}

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

              <div className="py-3 border-b border-border flex items-center justify-between">
                <div>
                  <p className="text-xs font-semibold text-foreground">Vault Directory Location</p>
                  <p className="text-[11px] text-muted-foreground font-mono">.relay/vault</p>
                </div>
                <Badge variant="outline" className="text-xs font-mono">
                  Local File System
                </Badge>
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

            <div className="p-4 rounded-xl bg-card border border-border flex items-center gap-4">
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
              <div className="p-4 rounded-xl bg-card border border-border flex items-center justify-between">
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
              <div className="p-4 rounded-xl border border-destructive/40 bg-destructive/5 space-y-3">
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
