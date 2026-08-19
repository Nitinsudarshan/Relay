import React, { useState } from 'react';
import { ProviderSettings as ProviderSettingsType } from '../../types';
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
  RefreshCw,
  Key,
  Globe,
  Radio
} from 'lucide-react';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { Badge } from '@/components/ui/badge';
import { Switch } from '@/components/ui/switch';
import { TriggerSettings } from './TriggerSettings';

type SettingsSection = 'general' | 'providers' | 'triggers' | 'vault' | 'account' | 'privacy';

export const ProviderSettings: React.FC = () => {
  const [activeSection, setActiveSection] = useState<SettingsSection>('providers');
  const [saved, setSaved] = useState(false);

  // General & Provider Settings state
  const [sttEngine, setSttEngine] = useState<'whisper' | 'parakeet' | 'cloud'>('whisper');
  const [hotkeyMode, setHotkeyMode] = useState<'hold' | 'toggle'>('hold');
  const [activeProvider, setActiveProvider] = useState<'ollama' | 'cloud'>('ollama');
  const [ollamaHost, setOllamaHost] = useState('http://localhost:11434');
  const [ollamaModel, setOllamaModel] = useState('llama3.2:latest');
  const [cloudApiKey, setCloudApiKey] = useState('');
  const [cloudModel, setCloudModel] = useState('gpt-4o-mini');

  // Vault & Data & Privacy state
  const [autoSaveVault, setAutoSaveVault] = useState(true);
  const [rawAudioKept, setRawAudioKept] = useState(true);

  const handleSave = (e: React.FormEvent) => {
    e.preventDefault();
    setSaved(true);
    setTimeout(() => setSaved(false), 2000);
  };

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

        {/* GENERAL SECTION */}
        {activeSection === 'general' && (
          <div className="space-y-6">
            <div>
              <p className="font-mono text-[10px] font-bold text-muted-foreground uppercase tracking-widest mb-1">
                GENERAL CAPTURE DEFAULTS
              </p>
              <h2 className="text-lg font-bold text-foreground">Global Dictation & Hotkey</h2>
            </div>

            <div className="space-y-4">
              <div className="py-3 border-b border-border flex items-center justify-between">
                <div>
                  <p className="text-xs font-semibold text-foreground">Global Push-to-Talk Hotkey</p>
                  <p className="text-[11px] text-muted-foreground">Trigger audio capture anywhere on system</p>
                </div>
                <kbd className="px-2 py-1 bg-muted rounded border border-border font-mono text-xs">
                  Ctrl+Space
                </kbd>
              </div>

              <div className="py-3 border-b border-border flex items-center justify-between">
                <div>
                  <p className="text-xs font-semibold text-foreground">Hotkey Activation Mode</p>
                  <p className="text-[11px] text-muted-foreground">Hold down vs press to toggle start/stop</p>
                </div>
                <div className="flex bg-muted p-1 rounded-xl border border-border">
                  <button
                    type="button"
                    onClick={() => setHotkeyMode('hold')}
                    className={`px-3 py-1 text-xs font-medium rounded-lg transition-all ${
                      hotkeyMode === 'hold' ? 'bg-card text-foreground font-semibold shadow-xs' : 'text-muted-foreground'
                    }`}
                  >
                    Hold
                  </button>
                  <button
                    type="button"
                    onClick={() => setHotkeyMode('toggle')}
                    className={`px-3 py-1 text-xs font-medium rounded-lg transition-all ${
                      hotkeyMode === 'toggle' ? 'bg-card text-foreground font-semibold shadow-xs' : 'text-muted-foreground'
                    }`}
                  >
                    Toggle
                  </button>
                </div>
              </div>

              <div className="py-3 border-b border-border flex items-center justify-between">
                <div>
                  <p className="text-xs font-semibold text-foreground">Speech-to-Text Model Engine</p>
                  <p className="text-[11px] text-muted-foreground">Local Parakeet / Whisper or Cloud STT</p>
                </div>
                <div className="flex bg-muted p-1 rounded-xl border border-border">
                  <button
                    type="button"
                    onClick={() => setSttEngine('whisper')}
                    className={`px-3 py-1 text-xs font-medium rounded-lg transition-all ${
                      sttEngine === 'whisper' ? 'bg-card text-foreground font-semibold shadow-xs' : 'text-muted-foreground'
                    }`}
                  >
                    Whisper
                  </button>
                  <button
                    type="button"
                    onClick={() => setSttEngine('parakeet')}
                    className={`px-3 py-1 text-xs font-medium rounded-lg transition-all ${
                      sttEngine === 'parakeet' ? 'bg-card text-foreground font-semibold shadow-xs' : 'text-muted-foreground'
                    }`}
                  >
                    Parakeet
                  </button>
                  <button
                    type="button"
                    onClick={() => setSttEngine('cloud')}
                    className={`px-3 py-1 text-xs font-medium rounded-lg transition-all ${
                      sttEngine === 'cloud' ? 'bg-card text-foreground font-semibold shadow-xs' : 'text-muted-foreground'
                    }`}
                  >
                    Cloud STT
                  </button>
                </div>
              </div>
            </div>
          </div>
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
                    onClick={() => setActiveProvider('ollama')}
                    className={`px-3 py-1 text-xs font-medium rounded-lg transition-all ${
                      activeProvider === 'ollama' ? 'bg-card text-foreground font-semibold shadow-xs' : 'text-muted-foreground'
                    }`}
                  >
                    Local Ollama
                  </button>
                  <button
                    type="button"
                    onClick={() => setActiveProvider('cloud')}
                    className={`px-3 py-1 text-xs font-medium rounded-lg transition-all ${
                      activeProvider === 'cloud' ? 'bg-card text-foreground font-semibold shadow-xs' : 'text-muted-foreground'
                    }`}
                  >
                    Cloud API
                  </button>
                </div>
              </div>

              {activeProvider === 'ollama' ? (
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
                        value={ollamaHost}
                        onChange={(e) => setOllamaHost(e.target.value)}
                        placeholder="http://localhost:11434"
                      />
                    </div>
                    <div>
                      <label htmlFor="ollama-model" className="block text-xs font-medium text-foreground mb-1">
                        Target Model Name
                      </label>
                      <Input
                        id="ollama-model"
                        value={ollamaModel}
                        onChange={(e) => setOllamaModel(e.target.value)}
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
                        value={cloudApiKey}
                        onChange={(e) => setCloudApiKey(e.target.value)}
                        placeholder="sk-..."
                      />
                    </div>
                    <div>
                      <label htmlFor="cloud-model-name" className="block text-xs font-medium text-foreground mb-1">
                        Cloud Model Selection
                      </label>
                      <Input
                        id="cloud-model-name"
                        value={cloudModel}
                        onChange={(e) => setCloudModel(e.target.value)}
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

        {/* DATA & PRIVACY SECTION (Exact reference design matching Part 4) */}
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
