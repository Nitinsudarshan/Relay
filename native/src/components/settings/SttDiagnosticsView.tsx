import React, { useEffect, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import {
  Activity,
  Cpu,
  RefreshCw,
  Sliders,
  Volume2,
  Layers,
  Zap,
  Globe,
  CheckCircle,
  AlertCircle,
  Copy,
  Check,
  Play,
  FileAudio,
  Sparkles,
  Terminal,
  ShieldAlert,
} from 'lucide-react';
import { Badge } from '@/components/ui/badge';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { Switch } from '@/components/ui/switch';
import {
  AppSettings,
  EvaluationResult,
  SttDiagnosticSnapshot,
} from '../../types';

interface SttDiagnosticsViewProps {
  settings: AppSettings;
  onUpdateSettings: (updater: (prev: AppSettings) => AppSettings) => void;
  onSaveSettings: () => Promise<void>;
}

const DEFAULT_RELAY_PROMPT =
  'Relay, Tauri, Rust, Whisper, Supabase, GitHub, Vercel, React, TypeScript, CPAL, whisper-rs';

export const SttDiagnosticsView: React.FC<SttDiagnosticsViewProps> = ({
  settings,
  onUpdateSettings,
  onSaveSettings,
}) => {
  const [snapshot, setSnapshot] = useState<SttDiagnosticSnapshot | null>(null);
  const [loadingSnapshot, setLoadingSnapshot] = useState(false);
  const [copied, setCopied] = useState(false);

  const [testWavPath, setTestWavPath] = useState('');
  const [testVariant, setTestVariant] = useState<
    'baseline' | 'relay_prompt' | 'best_of_3' | 'beam_2' | 'temperature_fallback'
  >('baseline');
  const [testReference, setTestReference] = useState('');
  const [runningEval, setRunningEval] = useState(false);
  const [evalResult, setEvalResult] = useState<EvaluationResult | null>(null);
  const [evalError, setEvalError] = useState('');
  const [corpusList, setCorpusList] = useState<import('../../types').CorpusItem[]>([]);

  const fetchLastDiagnostics = async () => {
    setLoadingSnapshot(true);
    try {
      const res = await invoke<SttDiagnosticSnapshot | null>(
        'get_last_stt_diagnostics'
      );
      setSnapshot(res);
      if (res?.audio_file && !testWavPath) {
        setTestWavPath(res.audio_file);
      }
    } catch (err) {
      console.error('Failed to fetch last STT diagnostics:', err);
    } finally {
      setLoadingSnapshot(false);
    }
  };

  useEffect(() => {
    fetchLastDiagnostics();

    invoke<import('../../types').CorpusItem[]>('get_stt_corpus')
      .then((items) => setCorpusList(items || []))
      .catch((e) => console.error('Failed to load STT corpus:', e));

    const unlistenPromise = listen<SttDiagnosticSnapshot>(
      'stt-diagnostics-updated',
      ({ payload }) => {
        setSnapshot(payload);
      }
    );

    return () => {
      unlistenPromise.then((unlisten) => unlisten());
    };
  }, []);

  const handleCopyTranscript = () => {
    if (snapshot?.transcript) {
      navigator.clipboard.writeText(snapshot.transcript);
      setCopied(true);
      setTimeout(() => setCopied(false), 2000);
    }
  };

  const handleRunEvaluation = async () => {
    if (!testWavPath.trim()) {
      setEvalError('Please provide a valid audio WAV file path.');
      return;
    }
    setRunningEval(true);
    setEvalError('');
    setEvalResult(null);

    try {
      const res = await invoke<EvaluationResult>('run_stt_evaluation', {
        wavPath: testWavPath.trim(),
        variant: testVariant,
        referenceText: testReference.trim() ? testReference.trim() : null,
        customModelPath: settings.stt.whisper_model_path || null,
      });
      setEvalResult(res);
    } catch (err: any) {
      console.error('Evaluation run failed:', err);
      setEvalError(err?.message || String(err));
    } finally {
      setRunningEval(false);
    }
  };

  const handleTogglePrompt = (enabled: boolean) => {
    onUpdateSettings((prev) => ({
      ...prev,
      stt: {
        ...prev.stt,
        enable_initial_prompt: enabled,
        custom_initial_prompt:
          prev.stt.custom_initial_prompt || DEFAULT_RELAY_PROMPT,
      },
    }));
  };

  const handlePromptTextChange = (text: string) => {
    onUpdateSettings((prev) => ({
      ...prev,
      stt: {
        ...prev.stt,
        custom_initial_prompt: text,
      },
    }));
  };

  const handleResetPrompt = () => {
    onUpdateSettings((prev) => ({
      ...prev,
      stt: {
        ...prev.stt,
        custom_initial_prompt: DEFAULT_RELAY_PROMPT,
      },
    }));
  };

  return (
    <div className="space-y-6 text-foreground">
      {/* Header */}
      <div className="flex flex-col md:flex-row md:items-center justify-between gap-4 border-b border-border pb-4">
        <div>
          <p className="font-mono text-[10px] font-bold text-muted-foreground uppercase tracking-widest mb-1">
            SPEECH-TO-TEXT OBSERVABILITY & CONTROL
          </p>
          <h2 className="text-xl font-bold flex items-center gap-2 text-foreground">
            <Activity className="w-5 h-5 text-primary" />
            STT Diagnostics & Quality Inspector
          </h2>
          <p className="text-xs text-muted-foreground mt-1">
            Real-time telemetry, audio telemetry, VAD decisions, and decoding
            diagnostics for speech recognition.
          </p>
        </div>
        <div className="flex items-center gap-2">
          <Button
            type="button"
            variant="outline"
            size="sm"
            onClick={fetchLastDiagnostics}
            disabled={loadingSnapshot}
            className="text-xs flex items-center gap-1.5"
          >
            <RefreshCw
              className={`w-3.5 h-3.5 ${loadingSnapshot ? 'animate-spin' : ''}`}
            />
            Refresh Telemetry
          </Button>
        </div>
      </div>

      {/* Production Model Invariant Status */}
      <div className="p-4 rounded-lg border border-primary/20 bg-primary/5 flex flex-col md:flex-row items-start md:items-center justify-between gap-4">
        <div className="flex items-center gap-3">
          <div className="p-2.5 rounded-lg bg-primary/10 text-primary">
            <Cpu className="w-5 h-5" />
          </div>
          <div>
            <div className="flex items-center gap-2">
              <span className="font-semibold text-sm">Whisper Small (GGML)</span>
              <Badge variant="outline" className="text-[10px] font-mono border-primary/30 text-primary bg-primary/10">
                Production Default (244M)
              </Badge>
              <Badge variant="outline" className="text-[10px] font-mono border-emerald-500/30 text-emerald-500 bg-emerald-500/10">
                16 kHz Mono
              </Badge>
            </div>
            <p className="text-xs text-muted-foreground mt-0.5 font-mono truncate max-w-md">
              {settings.stt.whisper_model_path || '%APPDATA%\\Relay\\models\\ggml-small.bin'}
            </p>
          </div>
        </div>
        <div className="text-right text-xs">
          <span className="text-muted-foreground">Local CPU Backend · Zero Cost</span>
        </div>
      </div>

      {/* LAST TRANSCRIPTION INSPECTOR */}
      <div className="space-y-4">
        <div className="flex items-center justify-between">
          <h3 className="text-sm font-bold flex items-center gap-2">
            <Sliders className="w-4 h-4 text-primary" />
            Last Transcription Snapshot
          </h3>
          {snapshot && (
            <span className="text-[11px] font-mono text-muted-foreground">
              Mode: <span className="text-foreground uppercase font-semibold">{snapshot.session_mode}</span> ·{' '}
              {new Date(Number(snapshot.timestamp_epoch_ms)).toLocaleTimeString()}
            </span>
          )}
        </div>

        {snapshot ? (
          <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-4">
            {/* Card 1: Audio Telemetry */}
            <div className="p-4 rounded-lg border border-border bg-card/60 space-y-3">
              <div className="flex items-center justify-between pb-2 border-b border-border/60">
                <span className="text-xs font-semibold flex items-center gap-1.5">
                  <Volume2 className="w-3.5 h-3.5 text-primary" />
                  Audio Telemetry
                </span>
                <Badge variant="outline" className="text-[10px] font-mono">
                  {snapshot.sample_rate} Hz Mono
                </Badge>
              </div>
              <div className="space-y-1.5 text-xs font-mono">
                <div className="flex justify-between">
                  <span className="text-muted-foreground">Original Duration:</span>
                  <span className="font-semibold">{snapshot.original_duration_seconds.toFixed(2)}s</span>
                </div>
                <div className="flex justify-between">
                  <span className="text-muted-foreground">Processed Audio:</span>
                  <span className="font-semibold">{snapshot.processed_duration_seconds.toFixed(2)}s</span>
                </div>
                <div className="flex justify-between">
                  <span className="text-muted-foreground">RMS Amplitude:</span>
                  <span className="font-semibold">{snapshot.rms.toFixed(4)}</span>
                </div>
                <div className="flex justify-between">
                  <span className="text-muted-foreground">Peak Amplitude:</span>
                  <span className="font-semibold">{snapshot.peak_amplitude.toFixed(4)}</span>
                </div>
                <div className="flex justify-between">
                  <span className="text-muted-foreground">Near-Zero Silence:</span>
                  <span className="font-semibold">{snapshot.near_zero_percent.toFixed(1)}%</span>
                </div>
                <div className="flex justify-between">
                  <span className="text-muted-foreground">Non-Finite Status:</span>
                  <span className={snapshot.has_non_finite ? 'text-rose-500 font-bold' : 'text-emerald-500'}>
                    {snapshot.has_non_finite ? 'Sanitized Non-Finite' : 'Clean Float32'}
                  </span>
                </div>
              </div>
            </div>

            {/* Card 2: VAD Activity */}
            <div className="p-4 rounded-lg border border-border bg-card/60 space-y-3">
              <div className="flex items-center justify-between pb-2 border-b border-border/60">
                <span className="text-xs font-semibold flex items-center gap-1.5">
                  <Layers className="w-3.5 h-3.5 text-primary" />
                  VAD Segmentation
                </span>
                <Badge
                  variant="outline"
                  className={`text-[10px] font-mono ${
                    snapshot.speech_detected
                      ? 'border-emerald-500/30 text-emerald-500 bg-emerald-500/10'
                      : 'border-amber-500/30 text-amber-500 bg-amber-500/10'
                  }`}
                >
                  {snapshot.speech_detected ? 'Speech Detected' : 'No Speech'}
                </Badge>
              </div>
              <div className="space-y-1.5 text-xs font-mono">
                <div className="flex justify-between">
                  <span className="text-muted-foreground">Speech Bounds:</span>
                  <span className="font-semibold">
                    {snapshot.vad_start_seconds.toFixed(2)}s → {snapshot.vad_end_seconds.toFixed(2)}s
                  </span>
                </div>
                <div className="flex justify-between">
                  <span className="text-muted-foreground">Trimmed Duration:</span>
                  <span className="font-semibold">{snapshot.vad_trimmed_duration_seconds.toFixed(2)}s</span>
                </div>
                <div className="flex justify-between">
                  <span className="text-muted-foreground">Silence Removed:</span>
                  <span className="font-semibold">{snapshot.silence_removed_percent.toFixed(1)}%</span>
                </div>
                <div className="flex justify-between">
                  <span className="text-muted-foreground">Adaptive Noise Floor:</span>
                  <span className="font-semibold">{snapshot.noise_floor.toFixed(4)}</span>
                </div>
                <div className="flex justify-between">
                  <span className="text-muted-foreground">Energy Onset Gate:</span>
                  <span className="font-semibold">{snapshot.onset_threshold.toFixed(4)}</span>
                </div>
              </div>
            </div>

            {/* Card 3: Language & Performance */}
            <div className="p-4 rounded-lg border border-border bg-card/60 space-y-3">
              <div className="flex items-center justify-between pb-2 border-b border-border/60">
                <span className="text-xs font-semibold flex items-center gap-1.5">
                  <Globe className="w-3.5 h-3.5 text-primary" />
                  Language & Latency
                </span>
                <Badge variant="outline" className="text-[10px] font-mono">
                  {snapshot.inference_duration_ms} ms
                </Badge>
              </div>
              <div className="space-y-1.5 text-xs font-mono">
                <div className="flex justify-between">
                  <span className="text-muted-foreground">Primary Preference:</span>
                  <span className="font-semibold">{snapshot.primary_dictation_language}</span>
                </div>
                <div className="flex justify-between">
                  <span className="text-muted-foreground">Spoken Languages:</span>
                  <span className="font-semibold">[{snapshot.spoken_languages.join(', ')}]</span>
                </div>
                <div className="flex justify-between">
                  <span className="text-muted-foreground">Resolved Whisper Lang:</span>
                  <span className="font-bold text-primary">
                    {snapshot.resolved_whisper_language ? `"${snapshot.resolved_whisper_language}"` : 'Auto-Detect (Multilingual)'}
                  </span>
                </div>
                <div className="flex justify-between">
                  <span className="text-muted-foreground">Real-Time Factor (RTF):</span>
                  <span className="font-semibold text-emerald-500">{snapshot.real_time_factor.toFixed(2)}x</span>
                </div>
                <div className="flex justify-between">
                  <span className="text-muted-foreground">Segments / Chars:</span>
                  <span className="font-semibold">
                    {snapshot.segment_count} segs / {snapshot.transcript_char_count} chars
                  </span>
                </div>
              </div>
            </div>
          </div>
        ) : (
          <div className="p-8 rounded-lg border border-dashed border-border text-center space-y-2">
            <FileAudio className="w-8 h-8 mx-auto text-muted-foreground opacity-50" />
            <p className="text-xs font-medium text-muted-foreground">
              No transcription session recorded yet since application launch.
            </p>
            <p className="text-[11px] text-muted-foreground">
              Press the dictation hotkey (<span className="font-mono">{settings.hotkeys.dictation_hotkey}</span>) to dictate a phrase.
            </p>
          </div>
        )}

        {/* Emitted Transcript Snippet */}
        {snapshot && (
          <div className="p-4 rounded-lg border border-border bg-muted/20 space-y-2">
            <div className="flex items-center justify-between">
              <span className="text-xs font-bold text-muted-foreground uppercase tracking-wider">
                Verbatim Emitted Transcript
              </span>
              <Button
                type="button"
                variant="ghost"
                size="sm"
                onClick={handleCopyTranscript}
                className="h-7 text-xs flex items-center gap-1 text-muted-foreground hover:text-foreground"
              >
                {copied ? <Check className="w-3 h-3 text-emerald-500" /> : <Copy className="w-3 h-3" />}
                {copied ? 'Copied' : 'Copy'}
              </Button>
            </div>
            <p className="text-sm font-mono p-3 rounded-lg bg-background border border-border text-foreground select-text whitespace-pre-wrap">
              {snapshot.transcript ? snapshot.transcript : <span className="italic text-muted-foreground font-sans">No speech recognized (empty emission).</span>}
            </p>
            {snapshot.error && (
              <p className="text-xs text-rose-500 font-mono mt-1">
                Error: {snapshot.error}
              </p>
            )}
          </div>
        )}
      </div>

      {/* DOMAIN VOCABULARY PROMPTING SETTINGS */}
      <div className="p-5 rounded-lg border border-border bg-card/60 space-y-4">
        <div className="flex items-start justify-between gap-4">
          <div>
            <div className="flex items-center gap-2">
              <Sparkles className="w-4 h-4 text-primary" />
              <h3 className="text-sm font-bold text-foreground">
                Technical Domain Vocabulary Priming
              </h3>
              <Badge variant="outline" className="text-[10px] font-mono">
                Phase 5/6 Verified
              </Badge>
            </div>
            <p className="text-xs text-muted-foreground mt-1 max-w-xl">
              Provides Whisper with an initial context prompt to boost recognition of specialized
              technical terminology (e.g. Tauri, Rust, CPAL, Supabase). Disabled by default.
            </p>
          </div>
          <Switch
            checked={!!settings.stt.enable_initial_prompt}
            onCheckedChange={handleTogglePrompt}
          />
        </div>

        {settings.stt.enable_initial_prompt && (
          <div className="space-y-3 pt-3 border-t border-border/60">
            <div className="flex items-center justify-between">
              <label htmlFor="custom-prompt-input" className="text-xs font-semibold text-foreground">
                Domain Keywords Prompt
              </label>
              <Button
                type="button"
                variant="ghost"
                size="sm"
                onClick={handleResetPrompt}
                className="h-6 text-[11px] text-muted-foreground hover:text-foreground"
              >
                Reset to Relay Standard
              </Button>
            </div>
            <textarea
              id="custom-prompt-input"
              value={settings.stt.custom_initial_prompt || DEFAULT_RELAY_PROMPT}
              onChange={(e) => handlePromptTextChange(e.target.value)}
              rows={2}
              className="w-full p-2.5 text-xs font-mono rounded-lg bg-background border border-border text-foreground focus:outline-hidden focus:ring-1 focus:ring-primary"
              placeholder="Enter comma-separated domain vocabulary..."
            />
            <p className="text-[11px] text-muted-foreground">
              Keep prompts focused and concise. Large dictionaries can induce phonetic hallucinations.
            </p>
          </div>
        )}
      </div>

      {/* STT TEST BENCH / VARIANT COMPARISON */}
      <div className="p-5 rounded-lg border border-border bg-card/60 space-y-4">
        <div>
          <div className="flex items-center gap-2">
            <Terminal className="w-4 h-4 text-primary" />
            <h3 className="text-sm font-bold text-foreground">
              STT Variant Audio Test Bench
            </h3>
            <Badge variant="outline" className="text-[10px] font-mono">
              Safe Non-Mutating
            </Badge>
          </div>
          <p className="text-xs text-muted-foreground mt-1">
            Test and benchmark alternate decoding strategies on identical recorded WAV files without
            affecting your active production settings.
          </p>
        </div>

        {corpusList.length > 0 && (
          <div>
            <label className="block text-[11px] font-semibold text-foreground mb-1">
              Select from Curated Corpus (35 Items across En, Hi, Hinglish, Technical & Silence)
            </label>
            <select
              onChange={(e) => {
                const item = corpusList.find((c) => c.test_id === e.target.value);
                if (item) {
                  setTestWavPath(item.audio_filename);
                  setTestReference(item.reference || '');
                }
              }}
              defaultValue=""
              className="w-full h-8 px-2 text-xs rounded-lg bg-background border border-border text-foreground focus:outline-hidden focus:ring-1 focus:ring-primary"
            >
              <option value="" disabled>-- Select a curated corpus item to auto-populate --</option>
              {corpusList.map((c) => (
                <option key={c.test_id} value={c.test_id}>
                  [{c.category}] {c.test_id}: {c.description} {c.reference ? `("${c.reference.slice(0, 35)}...")` : '(Silence)'}
                </option>
              ))}
            </select>
          </div>
        )}

        <div className="grid grid-cols-1 md:grid-cols-3 gap-3">
          <div className="md:col-span-2">
            <label className="block text-[11px] text-muted-foreground mb-1">
              Audio File Path (.wav)
            </label>
            <Input
              value={testWavPath}
              onChange={(e) => setTestWavPath(e.target.value)}
              placeholder="e.g. D:\Projects\Relay\.relay\config\audio\dictation_xyz.wav"
              className="text-xs font-mono h-8"
            />
          </div>
          <div>
            <label className="block text-[11px] text-muted-foreground mb-1">
              Decoding Variant
            </label>
            <select
              value={testVariant}
              onChange={(e: any) => setTestVariant(e.target.value)}
              className="w-full h-8 px-2 text-xs rounded-lg bg-background border border-border text-foreground focus:outline-hidden focus:ring-1 focus:ring-primary"
            >
              <option value="baseline">Baseline (Greedy, best_of=1)</option>
              <option value="relay_prompt">Relay Prompt (Greedy + Domain Prompt)</option>
              <option value="best_of_3">Best of 3 (Greedy, best_of=3)</option>
              <option value="beam_2">Beam Search (beam_size=2)</option>
              <option value="temperature_fallback">Temperature Fallback (Staged Retry)</option>
            </select>
          </div>
        </div>

        <div>
          <label className="block text-[11px] text-muted-foreground mb-1">
            Ground Truth Reference Text (Optional, to calculate WER / CER)
          </label>
          <Input
            value={testReference}
            onChange={(e) => setTestReference(e.target.value)}
            placeholder="Expected verbatim transcript..."
            className="text-xs font-mono h-8"
          />
        </div>

        <div className="flex justify-end gap-2 pt-2">
          <Button
            type="button"
            onClick={handleRunEvaluation}
            disabled={runningEval || !testWavPath.trim()}
            size="sm"
            className="text-xs flex items-center gap-1.5"
          >
            {runningEval ? (
              <>
                <RefreshCw className="w-3.5 h-3.5 animate-spin" />
                Evaluating...
              </>
            ) : (
              <>
                <Play className="w-3.5 h-3.5" />
                Run Evaluation
              </>
            )}
          </Button>
        </div>

        {evalError && (
          <div className="p-3 rounded-lg bg-rose-500/10 border border-rose-500/30 text-rose-500 text-xs flex items-center gap-2 font-mono">
            <AlertCircle className="w-4 h-4 shrink-0" />
            <span>{evalError}</span>
          </div>
        )}

        {evalResult && (
          <div className="p-4 rounded-lg border border-primary/20 bg-background space-y-3 text-xs font-mono">
            <div className="flex items-center justify-between pb-2 border-b border-border">
              <span className="font-bold text-foreground">
                Evaluation Result: {evalResult.configuration}
              </span>
              <div className="flex items-center gap-2">
                <Badge variant="outline" className="text-[10px]">
                  {evalResult.inference_duration_ms} ms
                </Badge>
                <Badge variant="outline" className="text-[10px] text-emerald-500">
                  RTF: {evalResult.real_time_factor.toFixed(2)}x
                </Badge>
              </div>
            </div>

            <div className="p-2.5 rounded-lg bg-muted/30 border border-border text-foreground">
              <span className="text-muted-foreground text-[10px] block mb-1 uppercase font-sans font-bold">
                Transcript Emission
              </span>
              {evalResult.transcript || <span className="italic text-muted-foreground">Empty transcript</span>}
            </div>

            {evalResult.accuracy && (
              <div className="grid grid-cols-2 md:grid-cols-4 gap-2 pt-1 text-center">
                <div className="p-2 rounded-lg bg-card border border-border">
                  <span className="text-[10px] text-muted-foreground block font-sans">WER</span>
                  <span className="font-bold text-foreground">{(evalResult.accuracy.wer * 100).toFixed(1)}%</span>
                </div>
                <div className="p-2 rounded-lg bg-card border border-border">
                  <span className="text-[10px] text-muted-foreground block font-sans">CER</span>
                  <span className="font-bold text-foreground">{(evalResult.accuracy.cer * 100).toFixed(1)}%</span>
                </div>
                <div className="p-2 rounded-lg bg-card border border-border">
                  <span className="text-[10px] text-muted-foreground block font-sans">Sub / Del / Ins</span>
                  <span className="font-bold text-foreground">
                    {evalResult.accuracy.substitutions} / {evalResult.accuracy.deletions} / {evalResult.accuracy.insertions}
                  </span>
                </div>
                <div className="p-2 rounded-lg bg-card border border-border">
                  <span className="text-[10px] text-muted-foreground block font-sans">Tech Accuracy</span>
                  <span className="font-bold text-emerald-500">
                    {evalResult.accuracy.technical_term_accuracy != null
                      ? `${(evalResult.accuracy.technical_term_accuracy * 100).toFixed(0)}%`
                      : 'N/A'}
                  </span>
                </div>
              </div>
            )}
          </div>
        )}
      </div>
    </div>
  );
};
