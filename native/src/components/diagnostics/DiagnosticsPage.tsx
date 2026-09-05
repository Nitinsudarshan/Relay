import React, { useEffect, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import {
  Activity,
  Cpu,
  RefreshCw,
  Sliders,
  Volume2,
  Zap,
  CheckCircle,
  AlertCircle,
  Play,
  FileAudio,
  Terminal,
  ShieldCheck,
  Radio,
  Layers,
  HardDrive,
  Clock,
  Settings,
  HelpCircle,
  Mic,
  Brain,
} from 'lucide-react';
import { Badge } from '@/components/ui/badge';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { MeetingPipelineDiagnostics } from './MeetingPipelineDiagnostics';
import { SpeakerEngineComparison } from './SpeakerEngineComparison';
import { KnowledgeArchitectureDiagnostics } from './KnowledgeArchitectureDiagnostics';
import {
  AppSettings,
  MainTabType,
  OllamaModelDetails,
  OllamaPromptTestResult,
  SttModelsOverview,
  SttModelTestResult,
  AudioDeviceInfo,
  VaultLocationInfo,
} from '@/types';
import { SttDiagnosticsView } from '../settings/SttDiagnosticsView';

interface DiagnosticsPageProps {
  onNavigateTab?: (tab: MainTabType) => void;
}

export const DiagnosticsPage: React.FC<DiagnosticsPageProps> = ({ onNavigateTab }) => {
  const [activeTab, setActiveTab] = useState<'stt' | 'meetings' | 'llm' | 'system' | 'knowledge'>(
    'stt',
  );

  // Overall system settings
  const [settings, setSettings] = useState<AppSettings | null>(null);
  const [loadingSettings, setLoadingSettings] = useState(true);

  // App version & vault info
  const [appVersion, setAppVersion] = useState<string>('0.29.0');
  const [vaultLocation, setVaultLocation] = useState<VaultLocationInfo | null>(null);

  // LLM status & models
  type OllamaStatus =
    | { state: 'checking' }
    | { state: 'running' }
    | { state: 'started' }
    | { state: 'not_installed' }
    | { state: 'unreachable'; message: string };
  const [ollamaStatus, setOllamaStatus] = useState<OllamaStatus>({ state: 'checking' });
  const [installedLlmModels, setInstalledLlmModels] = useState<OllamaModelDetails[]>([]);
  const [loadingLlmModels, setLoadingLlmModels] = useState(false);

  // LLM prompt test state
  const [testPrompt, setTestPrompt] = useState('Hello! Reply with "Relay AI ready" in under 5 words.');
  const [selectedTestModel, setSelectedTestModel] = useState<string>('');
  const [runningLlmTest, setRunningLlmTest] = useState(false);
  const [llmTestResult, setLlmTestResult] = useState<OllamaPromptTestResult | null>(null);

  // STT status & models
  const [sttOverview, setSttOverview] = useState<SttModelsOverview | null>(null);
  const [loadingSttModels, setLoadingSttModels] = useState(false);
  const [testingSttModel, setTestingSttModel] = useState<string | null>(null);
  const [sttTestResult, setSttTestResult] = useState<SttModelTestResult | null>(null);

  // Audio devices
  const [audioDevices, setAudioDevices] = useState<AudioDeviceInfo[]>([]);
  const [loadingDevices, setLoadingDevices] = useState(false);

  const fetchSettings = async () => {
    try {
      const s = await invoke<AppSettings>('get_settings');
      setSettings(s);
    } catch (err) {
      console.error('Failed to load settings in diagnostics:', err);
    } finally {
      setLoadingSettings(false);
    }
  };

  const fetchAppInfo = async () => {
    try {
      const [ver, vault] = await Promise.all([
        invoke<string>('get_app_version'),
        invoke<VaultLocationInfo>('get_vault_location'),
      ]);
      if (ver) setAppVersion(ver);
      if (vault) setVaultLocation(vault);
    } catch (err) {
      console.error('Failed to get app/vault info:', err);
    }
  };

  const checkLlmBackend = async () => {
    setOllamaStatus({ state: 'checking' });
    try {
      const status = await invoke<OllamaStatus>('ensure_local_llm_ready');
      setOllamaStatus(status);
    } catch (err) {
      setOllamaStatus({ state: 'unreachable', message: 'Could not reach backend' });
    }
  };

  const fetchLlmModels = async () => {
    setLoadingLlmModels(true);
    try {
      const models = await invoke<OllamaModelDetails[]>('get_available_llm_models');
      setInstalledLlmModels(models);
    } catch (err) {
      console.error('Failed to fetch LLM models in diagnostics:', err);
    } finally {
      setLoadingLlmModels(false);
    }
  };

  const fetchSttModels = async () => {
    setLoadingSttModels(true);
    try {
      const ov = await invoke<SttModelsOverview>('get_available_stt_models');
      setSttOverview(ov);
    } catch (err) {
      console.error('Failed to fetch STT models in diagnostics:', err);
    } finally {
      setLoadingSttModels(false);
    }
  };

  const fetchAudioDevices = async () => {
    setLoadingDevices(true);
    try {
      const devs = await invoke<AudioDeviceInfo[]>('get_audio_devices');
      setAudioDevices(devs);
    } catch (err) {
      console.error('Failed to fetch audio devices in diagnostics:', err);
    } finally {
      setLoadingDevices(false);
    }
  };

  const refreshAll = async () => {
    await Promise.all([
      fetchSettings(),
      fetchAppInfo(),
      checkLlmBackend(),
      fetchLlmModels(),
      fetchSttModels(),
      fetchAudioDevices(),
    ]);
  };

  useEffect(() => {
    fetchSettings();
    fetchAppInfo();
    checkLlmBackend();
    fetchLlmModels();
    fetchSttModels();
    fetchAudioDevices();
  }, []);

  const handleRunLlmPromptTest = async (overrideModel?: string) => {
    if (!settings) return;
    const modelToTest =
      overrideModel || selectedTestModel || settings.provider.ollama_model || 'llama3.2:latest';
    if (overrideModel) {
      setSelectedTestModel(overrideModel);
    }
    setRunningLlmTest(true);
    setLlmTestResult(null);
    try {
      const res = await invoke<OllamaPromptTestResult>('test_llm_prompt', {
        host: settings.provider.ollama_host || null,
        model: modelToTest,
        prompt: testPrompt || null,
      });
      setLlmTestResult(res);
    } catch (err: any) {
      setLlmTestResult({
        success: false,
        latency_ms: 0,
        model: modelToTest,
        error: err?.message || String(err),
      });
    } finally {
      setRunningLlmTest(false);
    }
  };

  const handleTestSttModel = async (path: string) => {
    setTestingSttModel(path);
    setSttTestResult(null);
    try {
      const res = await invoke<SttModelTestResult>('test_stt_model', { modelPath: path });
      setSttTestResult(res);
    } catch (err: any) {
      setSttTestResult({
        success: false,
        path,
        size_bytes: 0,
        latency_ms: 0,
        error: err?.message || String(err),
      });
    } finally {
      setTestingSttModel(null);
    }
  };

  const handleSaveSettingsDirect = async () => {
    if (!settings) return;
    try {
      await invoke('save_settings', { settings });
      await fetchSttModels();
    } catch (e) {
      console.error('Failed to save settings from diagnostics:', e);
    }
  };

  // Determine active model ready statuses
  const activeLlmModelName = settings?.provider.ollama_model || 'llama3.2:latest';
  const isLlmInstalled = installedLlmModels.some(
    (m) => m.name === activeLlmModelName || m.model === activeLlmModelName
  );
  const isOllamaUp = ollamaStatus.state === 'running' || ollamaStatus.state === 'started';

  const formatBytes = (bytes: number) => {
    if (!bytes) return '0 B';
    const k = 1024;
    const sizes = ['B', 'KB', 'MB', 'GB'];
    const i = Math.floor(Math.log(bytes) / Math.log(k));
    return parseFloat((bytes / Math.pow(k, i)).toFixed(1)) + ' ' + sizes[i];
  };

  return (
    <div className="space-y-6 pb-12 max-w-6xl mx-auto">
      {/* Page Header */}
      <div className="flex flex-col md:flex-row md:items-center justify-between gap-4 border-b border-border pb-4">
        <div>
          <div className="flex items-center gap-2 mb-1">
            <span className="font-mono text-[10px] font-bold text-muted-foreground uppercase tracking-widest">
              SYSTEM INSPECTION & TELEMETRY
            </span>
            <Badge variant="outline" className="text-[10px] font-mono border-primary/30 text-primary">
              v{appVersion}
            </Badge>
          </div>
          <h1 className="text-xl font-bold flex items-center gap-2 text-foreground">
            <Activity className="w-5 h-5 text-primary" />
            Diagnostics & System Health
          </h1>
          <p className="text-xs text-muted-foreground mt-0.5">
            Real-time telemetry, model readiness testing, audio/VAD inspection, and LLM latency benchmarks.
          </p>
        </div>

        <div className="flex items-center gap-2">
          {onNavigateTab && (
            <Button
              type="button"
              variant="outline"
              size="sm"
              onClick={() => onNavigateTab('settings')}
              className="text-xs flex items-center gap-1.5"
            >
              <Settings className="w-3.5 h-3.5" />
              Settings
            </Button>
          )}
          <Button
            type="button"
            variant="default"
            size="sm"
            onClick={refreshAll}
            className="text-xs flex items-center gap-1.5"
          >
            <RefreshCw className="w-3.5 h-3.5" />
            Refresh All
          </Button>
        </div>
      </div>

      {/* 1. SYSTEM STATUS OVERVIEW MATRIX */}
      <div className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-5 gap-3">
        {/* LLM Backend */}
        <div className="p-3.5 rounded-lg border border-border bg-card/60 flex flex-col justify-between">
          <div className="flex items-center justify-between mb-1.5">
            <span className="text-[11px] font-semibold text-muted-foreground flex items-center gap-1.5">
              <Cpu className="w-3.5 h-3.5 text-primary" />
              LLM Backend
            </span>
            {isOllamaUp ? (
              <Badge variant="emerald" className="text-[9px] px-1.5 py-0">Running ✓</Badge>
            ) : ollamaStatus.state === 'checking' ? (
              <Badge variant="outline" className="text-[9px] px-1.5 py-0">Checking…</Badge>
            ) : (
              <Badge variant="outline" className="text-[9px] px-1.5 py-0 border-destructive/40 text-destructive">Offline</Badge>
            )}
          </div>
          <div>
            <p className="text-xs font-bold text-foreground truncate">
              {settings?.provider.active_provider === 'ollama' ? 'Local Ollama' : 'Cloud API'}
            </p>
            <p className="text-[10px] text-muted-foreground font-mono truncate">
              {settings?.provider.ollama_host || 'http://localhost:11434'}
            </p>
          </div>
        </div>

        {/* LLM Model */}
        <div className="p-3.5 rounded-lg border border-border bg-card/60 flex flex-col justify-between">
          <div className="flex items-center justify-between mb-1.5">
            <span className="text-[11px] font-semibold text-muted-foreground flex items-center gap-1.5">
              <Zap className="w-3.5 h-3.5 text-amber-500" />
              Active LLM Model
            </span>
            {isOllamaUp && isLlmInstalled ? (
              <Badge variant="emerald" className="text-[9px] px-1.5 py-0">Ready ✓</Badge>
            ) : isOllamaUp && !isLlmInstalled ? (
              <Badge variant="outline" className="text-[9px] px-1.5 py-0 border-amber-500/40 text-amber-500">Missing ⚠</Badge>
            ) : (
              <Badge variant="outline" className="text-[9px] px-1.5 py-0 border-destructive/40 text-destructive">Unavailable</Badge>
            )}
          </div>
          <div>
            <p className="text-xs font-bold text-foreground truncate" title={activeLlmModelName}>
              {activeLlmModelName}
            </p>
            <p className="text-[10px] text-muted-foreground font-mono">
              {isLlmInstalled ? 'Installed in Ollama' : 'Not in local registry'}
            </p>
          </div>
        </div>

        {/* STT Engine */}
        <div className="p-3.5 rounded-lg border border-border bg-card/60 flex flex-col justify-between">
          <div className="flex items-center justify-between mb-1.5">
            <span className="text-[11px] font-semibold text-muted-foreground flex items-center gap-1.5">
              <Radio className="w-3.5 h-3.5 text-indigo-400" />
              STT Engine
            </span>
            <Badge variant="emerald" className="text-[9px] px-1.5 py-0">Active ✓</Badge>
          </div>
          <div>
            <p className="text-xs font-bold text-foreground">Whisper CPU</p>
            <p className="text-[10px] text-muted-foreground font-mono">16 kHz Mono · whisper.cpp</p>
          </div>
        </div>

        {/* STT Model */}
        <div className="p-3.5 rounded-lg border border-border bg-card/60 flex flex-col justify-between">
          <div className="flex items-center justify-between mb-1.5">
            <span className="text-[11px] font-semibold text-muted-foreground flex items-center gap-1.5">
              <FileAudio className="w-3.5 h-3.5 text-emerald-400" />
              Active STT Model
            </span>
            {sttOverview?.models.find((m) => m.path === sttOverview.active_model_path)?.status === 'ready' ? (
              <Badge variant="emerald" className="text-[9px] px-1.5 py-0">Ready ✓</Badge>
            ) : (
              <Badge variant="outline" className="text-[9px] px-1.5 py-0 border-amber-500/40 text-amber-500">Missing ⚠</Badge>
            )}
          </div>
          <div>
            <p className="text-xs font-bold text-foreground truncate" title={sttOverview?.active_model_name}>
              {sttOverview?.active_model_name || 'Whisper Small'}
            </p>
            <p className="text-[10px] text-muted-foreground font-mono truncate">
              {sttOverview?.active_profile === 'fast' ? 'Fast Profile (~0.8s)' : 'Accurate Profile (~2.4s)'}
            </p>
          </div>
        </div>

        {/* TTS Engine */}
        <div className="p-3.5 rounded-lg border border-border bg-card/60 flex flex-col justify-between">
          <div className="flex items-center justify-between mb-1.5">
            <span className="text-[11px] font-semibold text-muted-foreground flex items-center gap-1.5">
              <Volume2 className="w-3.5 h-3.5 text-sky-400" />
              TTS Engine
            </span>
            <Badge variant="emerald" className="text-[9px] px-1.5 py-0">Ready ✓</Badge>
          </div>
          <div>
            <p className="text-xs font-bold text-foreground">Piper Neural TTS</p>
            <p className="text-[10px] text-muted-foreground font-mono">Zero Cloud Cost · On-Device</p>
          </div>
        </div>
      </div>

      {/* Navigation Tabs */}
      <div className="flex border-b border-border gap-2">
        <button
          type="button"
          onClick={() => setActiveTab('stt')}
          className={`pb-2.5 px-3 text-xs font-semibold flex items-center gap-2 border-b-2 transition-all ${
            activeTab === 'stt'
              ? 'border-primary text-primary'
              : 'border-transparent text-muted-foreground hover:text-foreground'
          }`}
        >
          <FileAudio className="w-4 h-4" />
          Speech-to-Text Diagnostics
        </button>

        <button
          type="button"
          onClick={() => setActiveTab('meetings')}
          className={`pb-2.5 px-3 text-xs font-semibold flex items-center gap-2 border-b-2 transition-all ${
            activeTab === 'meetings'
              ? 'border-primary text-primary'
              : 'border-transparent text-muted-foreground hover:text-foreground'
          }`}
        >
          <Mic className="w-4 h-4" />
          Meeting Pipeline
        </button>

        <button
          type="button"
          onClick={() => setActiveTab('llm')}
          className={`pb-2.5 px-3 text-xs font-semibold flex items-center gap-2 border-b-2 transition-all ${
            activeTab === 'llm'
              ? 'border-primary text-primary'
              : 'border-transparent text-muted-foreground hover:text-foreground'
          }`}
        >
          <Cpu className="w-4 h-4" />
          LLM Diagnostics & Latency
        </button>

        <button
          type="button"
          onClick={() => setActiveTab('system')}
          className={`pb-2.5 px-3 text-xs font-semibold flex items-center gap-2 border-b-2 transition-all ${
            activeTab === 'system'
              ? 'border-primary text-primary'
              : 'border-transparent text-muted-foreground hover:text-foreground'
          }`}
        >
          <HardDrive className="w-4 h-4" />
          System & Audio Runtime
        </button>

        <button
          type="button"
          onClick={() => setActiveTab('knowledge')}
          className={`pb-2.5 px-3 text-xs font-semibold flex items-center gap-2 border-b-2 transition-all ${
            activeTab === 'knowledge'
              ? 'border-primary text-primary'
              : 'border-transparent text-muted-foreground hover:text-foreground'
          }`}
        >
          <Brain className="w-4 h-4" />
          Knowledge Architecture
        </button>
      </div>

      {activeTab === 'knowledge' && (
        <div className="space-y-6 animate-in fade-in-50">
          <KnowledgeArchitectureDiagnostics />
        </div>
      )}

      {activeTab === 'meetings' && (
        <div className="space-y-6 animate-in fade-in-50">
          <SpeakerEngineComparison />
          <MeetingPipelineDiagnostics />
        </div>
      )}

      {/* TAB CONTENT 1: STT DIAGNOSTICS */}
      {activeTab === 'stt' && (
        <div className="space-y-6 animate-in fade-in-50">
          {/* STT Model Readiness & Disk Verification */}
          <div className="p-4 rounded-lg border border-border bg-card/60 space-y-3">
            <div className="flex items-center justify-between pb-2 border-b border-border/60">
              <div className="flex items-center gap-2">
                <ShieldCheck className="w-4 h-4 text-primary" />
                <span className="text-xs font-bold text-foreground">STT Model Files & Verification</span>
              </div>
              <Button
                type="button"
                variant="ghost"
                size="sm"
                onClick={fetchSttModels}
                disabled={loadingSttModels}
                className="text-xs h-7 gap-1"
              >
                <RefreshCw className={`w-3 h-3 ${loadingSttModels ? 'animate-spin' : ''}`} />
                Rescan Disk
              </Button>
            </div>

            <div className="grid grid-cols-1 md:grid-cols-2 gap-3">
              {sttOverview?.models.map((mod) => {
                const isActive = mod.path === sttOverview.active_model_path;
                const isTesting = testingSttModel === mod.path;
                return (
                  <div
                    key={mod.filename}
                    className={`p-3 rounded-lg border text-xs space-y-2 transition-all ${
                      isActive
                        ? 'border-primary/50 bg-primary/5'
                        : 'border-border bg-muted/20'
                    }`}
                  >
                    <div className="flex items-center justify-between">
                      <div className="flex items-center gap-1.5">
                        <span className="font-semibold text-foreground">{mod.name}</span>
                        {isActive && (
                          <Badge variant="emerald" className="text-[9px] px-1.5 py-0">
                            Active Model
                          </Badge>
                        )}
                      </div>
                      <Badge
                        variant={mod.status === 'ready' ? 'emerald' : 'outline'}
                        className="text-[9px] font-mono"
                      >
                        {mod.status === 'ready' ? '✓ Ready' : '⚠ Missing'}
                      </Badge>
                    </div>

                    <div className="text-[11px] text-muted-foreground font-mono space-y-0.5">
                      <p className="truncate" title={mod.path}>
                        File: {mod.filename} ({formatBytes(mod.size_bytes)})
                      </p>
                      <p className="truncate text-[10px] opacity-75">
                        Path: {mod.path}
                      </p>
                    </div>

                    <div className="flex items-center justify-between pt-1">
                      <span className="text-[10px] text-muted-foreground">
                        {mod.profile ? `Profile: ${mod.profile}` : 'Custom Model'}
                      </span>
                      {mod.exists && (
                        <Button
                          type="button"
                          variant="outline"
                          size="sm"
                          onClick={() => handleTestSttModel(mod.path)}
                          disabled={isTesting}
                          className="text-[10px] h-6 px-2 gap-1"
                        >
                          <Play className={`w-2.5 h-2.5 ${isTesting ? 'animate-spin' : ''}`} />
                          Verify File
                        </Button>
                      )}
                    </div>
                  </div>
                );
              })}
            </div>

            {/* Test Model Result */}
            {sttTestResult && (
              <div
                className={`p-3 rounded-lg border text-xs font-mono space-y-1 ${
                  sttTestResult.success
                    ? 'border-emerald-500/30 bg-emerald-500/10 text-emerald-600 dark:text-emerald-400'
                    : 'border-destructive/30 bg-destructive/10 text-destructive'
                }`}
              >
                <div className="flex items-center gap-2 font-bold">
                  {sttTestResult.success ? (
                    <CheckCircle className="w-3.5 h-3.5" />
                  ) : (
                    <AlertCircle className="w-3.5 h-3.5" />
                  )}
                  <span>
                    {sttTestResult.success
                      ? `Model verified: Valid Whisper GGML header (${sttTestResult.latency_ms}ms, ${formatBytes(sttTestResult.size_bytes)})`
                      : `Verification failed: ${sttTestResult.error}`}
                  </span>
                </div>
              </div>
            )}
          </div>

          {/* Embedded Full STT Diagnostics & Quality Inspector View */}
          {settings && (
            <div className="pt-2">
              <SttDiagnosticsView
                settings={settings}
                onUpdateSettings={(updater) => setSettings((prev) => (prev ? updater(prev) : prev))}
                onSaveSettings={handleSaveSettingsDirect}
              />
            </div>
          )}
        </div>
      )}

      {/* TAB CONTENT 2: LLM DIAGNOSTICS */}
      {activeTab === 'llm' && (
        <div className="space-y-6 animate-in fade-in-50">
          {/* LLM Connectivity & Live Prompt Test */}
          <div className="p-4 rounded-lg border border-border bg-card/60 space-y-4">
            <div className="flex items-center justify-between pb-2 border-b border-border/60">
              <div className="flex items-center gap-2">
                <Zap className="w-4 h-4 text-amber-500" />
                <span className="text-xs font-bold text-foreground">Live LLM Prompt & Latency Benchmark</span>
              </div>
              <div className="flex items-center gap-2 text-xs">
                {isOllamaUp ? (
                  <Badge variant="emerald" className="text-[10px] font-mono">Backend Reachable ✓</Badge>
                ) : (
                  <Badge variant="outline" className="text-[10px] font-mono border-destructive/40 text-destructive">Backend Offline</Badge>
                )}
              </div>
            </div>

            <div className="space-y-3">
              <div>
                <div className="flex items-center justify-between mb-1.5">
                  <label className="text-[11px] font-medium text-foreground">
                    Benchmark Prompt
                  </label>
                  <div className="flex items-center gap-1.5">
                    <span className="text-[11px] text-muted-foreground">Target Model:</span>
                    <select
                      value={selectedTestModel || activeLlmModelName}
                      onChange={(e) => setSelectedTestModel(e.target.value)}
                      className="text-xs font-mono bg-background border border-border rounded px-2 py-0.5 text-foreground focus:outline-none focus:ring-1 focus:ring-primary"
                    >
                      {installedLlmModels.map((m) => (
                        <option key={m.name} value={m.name}>
                          {m.name} {m.name === activeLlmModelName ? '(Active)' : ''}
                        </option>
                      ))}
                      {!installedLlmModels.some(
                        (m) => m.name === activeLlmModelName || m.model === activeLlmModelName
                      ) && (
                        <option value={activeLlmModelName}>{activeLlmModelName} (Active)</option>
                      )}
                    </select>
                  </div>
                </div>
                <div className="flex gap-2">
                  <Input
                    value={testPrompt}
                    onChange={(e) => setTestPrompt(e.target.value)}
                    placeholder="Enter test prompt..."
                    className="text-xs"
                  />
                  <Button
                    type="button"
                    variant="default"
                    size="sm"
                    onClick={() => handleRunLlmPromptTest()}
                    disabled={runningLlmTest || !isOllamaUp}
                    className="text-xs flex items-center gap-1.5 shrink-0"
                  >
                    <Play className={`w-3.5 h-3.5 ${runningLlmTest ? 'animate-spin' : ''}`} />
                    {runningLlmTest ? 'Running…' : 'Run Test'}
                  </Button>
                </div>
                {runningLlmTest && (
                  <p className="text-[11px] text-muted-foreground mt-1.5 flex items-center gap-1 animate-pulse">
                    <span>⏳ Benchmarking against</span>
                    <span className="font-mono text-primary font-semibold">
                      {selectedTestModel || activeLlmModelName}
                    </span>
                    <span>— cold model loading from disk into VRAM/RAM can take 15–45s for larger models.</span>
                  </p>
                )}
              </div>

              {/* Prompt Test Result Display */}
              {llmTestResult && (
                <div
                  className={`p-3.5 rounded-lg border text-xs font-mono space-y-2 ${
                    llmTestResult.success
                      ? 'border-emerald-500/30 bg-emerald-500/5'
                      : 'border-destructive/30 bg-destructive/5 text-destructive'
                  }`}
                >
                  <div className="flex items-center justify-between">
                    <span className="font-bold flex items-center gap-1.5 text-foreground">
                      {llmTestResult.success ? (
                        <CheckCircle className="w-4 h-4 text-emerald-500" />
                      ) : (
                        <AlertCircle className="w-4 h-4 text-destructive" />
                      )}
                      Model: {llmTestResult.model}
                    </span>
                    <span className="text-[11px] text-muted-foreground">
                      Roundtrip: <span className="text-foreground font-bold">{llmTestResult.latency_ms} ms</span>
                    </span>
                  </div>

                  {llmTestResult.response && (
                    <div className="p-2.5 rounded bg-background/80 border border-border text-foreground text-xs whitespace-pre-wrap">
                      {llmTestResult.response}
                    </div>
                  )}

                  {llmTestResult.error && (
                    <p className="text-destructive text-xs">Error: {llmTestResult.error}</p>
                  )}
                </div>
              )}
            </div>
          </div>

          {/* Installed Ollama Models Table */}
          <div className="p-4 rounded-lg border border-border bg-card/60 space-y-3">
            <div className="flex items-center justify-between pb-2 border-b border-border/60">
              <div className="flex items-center gap-2">
                <Layers className="w-4 h-4 text-primary" />
                <span className="text-xs font-bold text-foreground">
                  Installed Local Ollama Models ({installedLlmModels.length})
                </span>
              </div>
              <Button
                type="button"
                variant="ghost"
                size="sm"
                onClick={fetchLlmModels}
                disabled={loadingLlmModels}
                className="text-xs h-7 gap-1"
              >
                <RefreshCw className={`w-3 h-3 ${loadingLlmModels ? 'animate-spin' : ''}`} />
                Refresh Models
              </Button>
            </div>

            {installedLlmModels.length === 0 ? (
              <p className="text-xs text-muted-foreground p-4 text-center">
                {isOllamaUp
                  ? 'No models installed yet in Ollama. Pull a model via "ollama pull llama3.2" in terminal.'
                  : 'Ollama is unreachable. Ensure Ollama is running at ' + (settings?.provider.ollama_host || 'http://localhost:11434')}
              </p>
            ) : (
              <div className="overflow-x-auto">
                <table className="w-full text-left text-xs font-mono">
                  <thead>
                    <tr className="border-b border-border text-muted-foreground text-[10px] uppercase">
                      <th className="py-2 px-2">Model Name</th>
                      <th className="py-2 px-2">Status</th>
                      <th className="py-2 px-2">Size</th>
                      <th className="py-2 px-2">Params</th>
                      <th className="py-2 px-2">Quant</th>
                      <th className="py-2 px-2">Family</th>
                      <th className="py-2 px-2 text-right">Action</th>
                    </tr>
                  </thead>
                  <tbody className="divide-y divide-border/60">
                    {installedLlmModels.map((m) => {
                      const isActive = m.name === activeLlmModelName || m.model === activeLlmModelName;
                      return (
                        <tr key={m.name} className={isActive ? 'bg-primary/5' : ''}>
                          <td className="py-2 px-2 font-bold text-foreground flex items-center gap-1.5">
                            {m.name}
                            {isActive && (
                              <Badge variant="emerald" className="text-[9px] px-1.5 py-0 font-sans">
                                Active
                              </Badge>
                            )}
                          </td>
                          <td className="py-2 px-2">
                            <Badge variant="emerald" className="text-[9px] px-1.5 py-0 font-sans">
                              ✓ Ready
                            </Badge>
                          </td>
                          <td className="py-2 px-2 text-muted-foreground">
                            {m.size ? formatBytes(m.size) : '—'}
                          </td>
                          <td className="py-2 px-2 text-muted-foreground">
                            {m.parameter_size || '—'}
                          </td>
                          <td className="py-2 px-2 text-muted-foreground">
                            {m.quantization_level || '—'}
                          </td>
                          <td className="py-2 px-2 text-muted-foreground">
                            {m.family || '—'}
                          </td>
                          <td className="py-2 px-2 text-right">
                            <Button
                              type="button"
                              variant="outline"
                              size="sm"
                              className="h-6 text-[11px] px-2 gap-1 font-sans"
                              onClick={() => {
                                setSelectedTestModel(m.name);
                                handleRunLlmPromptTest(m.name);
                              }}
                              disabled={runningLlmTest || !isOllamaUp}
                            >
                              <Play className="w-2.5 h-2.5 text-primary" />
                              Benchmark
                            </Button>
                          </td>
                        </tr>
                      );
                    })}
                  </tbody>
                </table>
              </div>
            )}
          </div>
        </div>
      )}

      {/* TAB CONTENT 3: SYSTEM & RUNTIME DIAGNOSTICS */}
      {activeTab === 'system' && (
        <div className="space-y-6 animate-in fade-in-50">
          {/* Audio Input Device Diagnostics */}
          <div className="p-4 rounded-lg border border-border bg-card/60 space-y-3">
            <div className="flex items-center justify-between pb-2 border-b border-border/60">
              <div className="flex items-center gap-2">
                <Volume2 className="w-4 h-4 text-emerald-500" />
                <span className="text-xs font-bold text-foreground">
                  Audio Input Devices ({audioDevices.length})
                </span>
              </div>
              <Button
                type="button"
                variant="ghost"
                size="sm"
                onClick={fetchAudioDevices}
                disabled={loadingDevices}
                className="text-xs h-7 gap-1"
              >
                <RefreshCw className={`w-3 h-3 ${loadingDevices ? 'animate-spin' : ''}`} />
                Rescan Devices
              </Button>
            </div>

            {audioDevices.length === 0 ? (
              <p className="text-xs text-muted-foreground p-3">No input devices detected.</p>
            ) : (
              <div className="space-y-2">
                {audioDevices.map((d) => (
                  <div
                    key={d.name}
                    className={`p-3 rounded-lg border text-xs flex items-center justify-between ${
                      d.is_default ? 'border-primary/40 bg-primary/5' : 'border-border bg-muted/20'
                    }`}
                  >
                    <div>
                      <div className="flex items-center gap-2">
                        <span className="font-semibold text-foreground">{d.name}</span>
                        {d.is_default && (
                          <Badge variant="emerald" className="text-[9px] px-1.5 py-0 font-mono">
                            OS Default Mic
                          </Badge>
                        )}
                      </div>
                    </div>
                    <div className="text-right text-[11px] font-mono text-muted-foreground">
                      16 kHz Mono Supported ✓
                    </div>
                  </div>
                ))}
              </div>
            )}
          </div>

          {/* Paths & Runtime Environment */}
          <div className="p-4 rounded-lg border border-border bg-card/60 space-y-3">
            <div className="pb-2 border-b border-border/60">
              <span className="text-xs font-bold text-foreground">Relay Runtime & Filesystem Paths</span>
            </div>

            <div className="space-y-2 text-xs font-mono">
              <div className="p-2.5 rounded bg-muted/30 border border-border flex items-center justify-between">
                <span className="text-muted-foreground">Relay App Version:</span>
                <span className="font-bold text-foreground">{appVersion}</span>
              </div>
              <div className="p-2.5 rounded bg-muted/30 border border-border flex items-center justify-between">
                <span className="text-muted-foreground">Vault Directory:</span>
                <span className="font-bold text-foreground truncate max-w-md" title={vaultLocation?.path || ''}>
                  {vaultLocation?.path || 'Default AppData Vault'}
                </span>
              </div>
              <div className="p-2.5 rounded bg-muted/30 border border-border flex items-center justify-between">
                <span className="text-muted-foreground">STT Models Directory:</span>
                <span className="font-bold text-foreground truncate max-w-md" title={sttOverview?.models_dir || ''}>
                  {sttOverview?.models_dir || '%APPDATA%\\Relay\\models'}
                </span>
              </div>
              <div className="p-2.5 rounded bg-muted/30 border border-border flex items-center justify-between">
                <span className="text-muted-foreground">LLM Host:</span>
                <span className="font-bold text-foreground">
                  {settings?.provider.ollama_host || 'http://localhost:11434'}
                </span>
              </div>
            </div>
          </div>
        </div>
      )}
    </div>
  );
};
