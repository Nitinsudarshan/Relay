import React, { useState } from 'react';
import { ProviderSettings as ProviderSettingsType } from '../../types';
import { Cpu, Cloud, CheckCircle } from 'lucide-react';
import { Card, CardHeader, CardTitle, CardDescription, CardContent } from '@/components/ui/card';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { Badge } from '@/components/ui/badge';

export const ProviderSettings: React.FC = () => {
  const [settings, setSettings] = useState<ProviderSettingsType>({
    active_provider: 'ollama',
    ollama_host: 'http://localhost:11434',
    ollama_model: 'llama3.2:latest',
    cloud_model: 'gpt-4o-mini',
  });

  const [saved, setSaved] = useState(false);

  const handleSave = (e: React.FormEvent) => {
    e.preventDefault();
    setSaved(true);
    setTimeout(() => setSaved(false), 2000);
  };

  return (
    <Card className="h-full flex flex-col border-slate-800">
      <CardHeader className="flex-row items-center justify-between pb-3 space-y-0">
        <div className="flex items-center gap-2">
          <Cpu className="w-5 h-5 text-blue-400" />
          <div>
            <CardTitle>LLM & Provider Configuration</CardTitle>
            <CardDescription>
              Toggle between 100% free local execution (Ollama) and cloud APIs
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
        <form onSubmit={handleSave} className="space-y-4">
          {/* Provider Toggle */}
          <div>
            <label className="block text-xs font-medium text-slate-300 mb-2">Active Provider Engine</label>
            <div className="grid grid-cols-2 gap-3">
              <button
                type="button"
                onClick={() => setSettings({ ...settings, active_provider: 'ollama' })}
                className={`p-3.5 rounded-xl border text-left flex items-start gap-3 transition-all ${
                  settings.active_provider === 'ollama'
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
                onClick={() => setSettings({ ...settings, active_provider: 'cloud_openai' })}
                className={`p-3.5 rounded-xl border text-left flex items-start gap-3 transition-all ${
                  settings.active_provider !== 'ollama'
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

          {/* Ollama Options */}
          {settings.active_provider === 'ollama' && (
            <div className="bg-slate-950 rounded-xl p-4 border border-slate-800 space-y-3">
              <h4 className="text-xs font-semibold text-slate-300">Local Ollama Settings</h4>
              <div className="grid grid-cols-2 gap-3">
                <div>
                  <label className="block text-[11px] text-slate-400 mb-1">Host Endpoint</label>
                  <Input
                    type="text"
                    value={settings.ollama_host}
                    onChange={(e) => setSettings({ ...settings, ollama_host: e.target.value })}
                  />
                </div>
                <div>
                  <label className="block text-[11px] text-slate-400 mb-1">Model Name</label>
                  <Input
                    type="text"
                    value={settings.ollama_model}
                    onChange={(e) => setSettings({ ...settings, ollama_model: e.target.value })}
                  />
                </div>
              </div>
            </div>
          )}

          {/* Cloud Options */}
          {settings.active_provider !== 'ollama' && (
            <div className="bg-slate-950 rounded-xl p-4 border border-slate-800 space-y-3">
              <h4 className="text-xs font-semibold text-slate-300">Cloud API Credentials</h4>
              <div>
                <label className="block text-[11px] text-slate-400 mb-1">API Key</label>
                <Input
                  type="password"
                  placeholder="sk-..."
                  value={settings.cloud_api_key || ''}
                  onChange={(e) => setSettings({ ...settings, cloud_api_key: e.target.value })}
                />
              </div>
              <div>
                <label className="block text-[11px] text-slate-400 mb-1">Model</label>
                <Input
                  type="text"
                  value={settings.cloud_model || ''}
                  onChange={(e) => setSettings({ ...settings, cloud_model: e.target.value })}
                />
              </div>
            </div>
          )}

          <Button type="submit" size="sm" variant="default">
            Save Configuration
          </Button>
        </form>
      </CardContent>
    </Card>
  );
};
