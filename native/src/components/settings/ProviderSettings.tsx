import React, { useEffect, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { AppSettings } from '../../types';
import { Cpu, Cloud, CheckCircle, Mic, Volume2, Keyboard } from 'lucide-react';
import { Card, CardHeader, CardTitle, CardDescription, CardContent } from '@/components/ui/card';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { Badge } from '@/components/ui/badge';

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
  const [settings, setSettings] = useState<AppSettings>(DEFAULT_SETTINGS);
  const [loading, setLoading] = useState(true);
  const [saved, setSaved] = useState(false);
  const [error, setError] = useState('');

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
      <Card className="h-full flex items-center justify-center border-slate-800 text-xs text-slate-500">
        Loading settings…
      </Card>
    );
  }

  return (
    <Card className="h-full flex flex-col border-slate-800">
      <CardHeader className="flex-row items-center justify-between pb-3 space-y-0">
        <div className="flex items-center gap-2">
          <Cpu className="w-5 h-5 text-blue-400" />
          <div>
            <CardTitle>Provider, Voice & Hotkey Settings</CardTitle>
            <CardDescription>
              LLM engine, local speech-to-text/text-to-speech, and global hotkeys
            </CardDescription>
          </div>
        </div>
        {saved && (
          <Badge variant="emerald" className="gap-1 px-2.5 py-1">
            <CheckCircle className="w-3.5 h-3.5" /> Saved
          </Badge>
        )}
      </CardHeader>

      <CardContent className="flex-1 overflow-y-auto pr-1">
        <form onSubmit={handleSave} className="space-y-5">
          {error && <p className="text-xs text-amber-400">{error}</p>}

          {/* Provider Toggle */}
          <div>
            <label className="block text-xs font-medium text-slate-300 mb-2">Active Provider Engine</label>
            <div className="grid grid-cols-2 gap-3">
              <button
                type="button"
                onClick={() =>
                  setSettings({ ...settings, provider: { ...settings.provider, active_provider: 'ollama' } })
                }
                className={`p-3.5 rounded-xl border text-left flex items-start gap-3 transition-all ${
                  settings.provider.active_provider === 'ollama'
                    ? 'bg-blue-600/20 border-blue-500 text-slate-100 shadow-md shadow-blue-500/10'
                    : 'bg-slate-950 border-slate-800 text-slate-400 hover:border-slate-700'
                }`}
              >
                <Cpu className="w-5 h-5 text-blue-400 shrink-0 mt-0.5" />
                <div>
                  <div className="font-semibold text-xs text-slate-200">Local Ollama ($0 Free)</div>
                  <div className="text-[11px] text-slate-400">100% offline, privacy-first local LLM</div>
                </div>
              </button>

              <button
                type="button"
                onClick={() =>
                  setSettings({ ...settings, provider: { ...settings.provider, active_provider: 'cloud_openai' } })
                }
                className={`p-3.5 rounded-xl border text-left flex items-start gap-3 transition-all ${
                  settings.provider.active_provider !== 'ollama'
                    ? 'bg-purple-600/20 border-purple-500 text-slate-100 shadow-md shadow-purple-500/10'
                    : 'bg-slate-950 border-slate-800 text-slate-400 hover:border-slate-700'
                }`}
              >
                <Cloud className="w-5 h-5 text-purple-400 shrink-0 mt-0.5" />
                <div>
                  <div className="font-semibold text-xs text-slate-200">Cloud API (Optional)</div>
                  <div className="text-[11px] text-slate-400">OpenAI / Gemini / Anthropic</div>
                </div>
              </button>
            </div>
          </div>

          {settings.provider.active_provider === 'ollama' ? (
            <div className="bg-slate-950 rounded-xl p-4 border border-slate-800 space-y-3">
              <h4 className="text-xs font-semibold text-slate-300">Local Ollama Settings</h4>
              <div className="grid grid-cols-2 gap-3">
                <div>
                  <label className="block text-[11px] text-slate-400 mb-1">Host Endpoint</label>
                  <Input
                    type="text"
                    value={settings.provider.ollama_host}
                    onChange={(e) =>
                      setSettings({ ...settings, provider: { ...settings.provider, ollama_host: e.target.value } })
                    }
                  />
                </div>
                <div>
                  <label className="block text-[11px] text-slate-400 mb-1">Model Name</label>
                  <Input
                    type="text"
                    value={settings.provider.ollama_model}
                    onChange={(e) =>
                      setSettings({ ...settings, provider: { ...settings.provider, ollama_model: e.target.value } })
                    }
                  />
                </div>
              </div>
            </div>
          ) : (
            <div className="bg-slate-950 rounded-xl p-4 border border-slate-800 space-y-3">
              <h4 className="text-xs font-semibold text-slate-300">Cloud API Credentials</h4>
              <div>
                <label className="block text-[11px] text-slate-400 mb-1">API Key</label>
                <Input
                  type="password"
                  placeholder="sk-..."
                  value={settings.provider.cloud_api_key || ''}
                  onChange={(e) =>
                    setSettings({ ...settings, provider: { ...settings.provider, cloud_api_key: e.target.value } })
                  }
                />
              </div>
              <div>
                <label className="block text-[11px] text-slate-400 mb-1">Model</label>
                <Input
                  type="text"
                  value={settings.provider.cloud_model || ''}
                  onChange={(e) =>
                    setSettings({ ...settings, provider: { ...settings.provider, cloud_model: e.target.value } })
                  }
                />
              </div>
            </div>
          )}

          {/* Speech-to-Text */}
          <div className="bg-slate-950 rounded-xl p-4 border border-slate-800 space-y-3">
            <h4 className="text-xs font-semibold text-slate-300 flex items-center gap-1.5">
              <Mic className="w-3.5 h-3.5 text-emerald-400" />
              Local Speech-to-Text (Whisper)
            </h4>
            <div>
              <label className="block text-[11px] text-slate-400 mb-1">GGML Model Path</label>
              <Input
                type="text"
                placeholder="C:\\models\\ggml-base.en.bin"
                value={settings.stt.whisper_model_path || ''}
                onChange={(e) => setSettings({ ...settings, stt: { whisper_model_path: e.target.value } })}
              />
              <p className="text-[10px] text-slate-500 mt-1">
                Download a GGML model from huggingface.co/ggerganov/whisper.cpp — required for meeting/scribble
                capture, voice chat, and universal dictation.
              </p>
            </div>
          </div>

          {/* Text-to-Speech */}
          <div className="bg-slate-950 rounded-xl p-4 border border-slate-800 space-y-3">
            <h4 className="text-xs font-semibold text-slate-300 flex items-center gap-1.5">
              <Volume2 className="w-3.5 h-3.5 text-purple-400" />
              Local Text-to-Speech (Piper) — optional
            </h4>
            <div className="grid grid-cols-2 gap-3">
              <div>
                <label className="block text-[11px] text-slate-400 mb-1">Piper Binary Path</label>
                <Input
                  type="text"
                  placeholder="C:\\piper\\piper.exe"
                  value={settings.tts.piper_binary_path || ''}
                  onChange={(e) =>
                    setSettings({ ...settings, tts: { ...settings.tts, piper_binary_path: e.target.value } })
                  }
                />
              </div>
              <div>
                <label className="block text-[11px] text-slate-400 mb-1">Voice Model Path</label>
                <Input
                  type="text"
                  placeholder="C:\\piper\\en_US-lessac-medium.onnx"
                  value={settings.tts.piper_voice_path || ''}
                  onChange={(e) =>
                    setSettings({ ...settings, tts: { ...settings.tts, piper_voice_path: e.target.value } })
                  }
                />
              </div>
            </div>
            <p className="text-[10px] text-slate-500">
              Leave blank to skip "speak back" in voice chat — answers still show as text.
            </p>
          </div>

          {/* Hotkeys */}
          <div className="bg-slate-950 rounded-xl p-4 border border-slate-800 space-y-3">
            <h4 className="text-xs font-semibold text-slate-300 flex items-center gap-1.5">
              <Keyboard className="w-3.5 h-3.5 text-amber-400" />
              Global Hotkeys
            </h4>
            <div className="grid grid-cols-2 gap-3">
              <div>
                <label className="block text-[11px] text-slate-400 mb-1">Show/Hide Relay</label>
                <Input
                  type="text"
                  value={settings.hotkeys.show_hide_hotkey}
                  onChange={(e) =>
                    setSettings({ ...settings, hotkeys: { ...settings.hotkeys, show_hide_hotkey: e.target.value } })
                  }
                />
              </div>
              <div>
                <label className="block text-[11px] text-slate-400 mb-1">Universal Dictation (hold to talk)</label>
                <Input
                  type="text"
                  value={settings.hotkeys.dictation_hotkey}
                  onChange={(e) =>
                    setSettings({ ...settings, hotkeys: { ...settings.hotkeys, dictation_hotkey: e.target.value } })
                  }
                />
              </div>
            </div>
            <p className="text-[10px] text-slate-500">Restart Relay after changing hotkeys for them to take effect.</p>
          </div>

          <Button type="submit" size="sm" variant="default">
            Save Configuration
          </Button>
        </form>
      </CardContent>
    </Card>
  );
};
