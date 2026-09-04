import React, { useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import {
  AlertTriangle,
  Check,
  ChevronDown,
  Loader2,
  Mic,
  Play,
  X,
} from 'lucide-react';
import type { MeetingSelfTestCheck, MeetingSelfTestReport } from '../../types';

/** One check row, with its purpose behind a disclosure. */
const CheckRow: React.FC<{ check: MeetingSelfTestCheck }> = ({ check }) => {
  const [open, setOpen] = useState(false);

  return (
    <li
      className={`rounded-lg border ${
        check.passed
          ? 'border-border bg-card'
          : 'border-destructive/40 bg-destructive/5'
      }`}
    >
      <button
        type="button"
        onClick={() => setOpen((v) => !v)}
        aria-expanded={open}
        className="w-full flex items-start gap-2 px-3 py-2 text-left cursor-pointer focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring rounded-lg"
      >
        <span className="shrink-0 mt-0.5">
          {check.passed ? (
            <Check
              className="w-3.5 h-3.5 text-emerald-600 dark:text-emerald-400"
              aria-label="passed"
            />
          ) : (
            <X className="w-3.5 h-3.5 text-destructive" aria-label="failed" />
          )}
        </span>
        <span className="flex-1 min-w-0">
          <span className="block text-xs font-semibold text-foreground">
            {check.name}
          </span>
          <span className="block text-[11px] text-muted-foreground mt-0.5 font-mono">
            {check.detail}
          </span>
        </span>
        <span className="shrink-0 flex items-center gap-1.5 text-[10px] font-mono text-muted-foreground">
          {check.duration_ms} ms
          <ChevronDown
            className={`w-3 h-3 transition-transform ${open ? '' : '-rotate-90'}`}
            aria-hidden="true"
          />
        </span>
      </button>
      {open && (
        <p className="px-3 pb-2.5 pl-8 text-[11px] text-muted-foreground max-w-prose">
          {check.purpose}
        </p>
      )}
    </li>
  );
};

/**
 * Diagnostics › Meeting Pipeline.
 *
 * Runs the meeting pipeline's checks here rather than trusting CI, because the
 * failure they cover is machine-dependent: whether room tone becomes four
 * minutes of "Thank you." turns on this microphone's noise floor and on which
 * Whisper model is installed. A green CI run says the logic is right; a green
 * run here says it is working on this machine.
 *
 * Nothing it does touches a recording, the vault, or settings.
 */
export const MeetingPipelineDiagnostics: React.FC = () => {
  const [report, setReport] = useState<MeetingSelfTestReport | null>(null);
  const [isRunning, setIsRunning] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const run = async () => {
    setIsRunning(true);
    setError(null);
    try {
      setReport(await invoke<MeetingSelfTestReport>('run_meeting_pipeline_selftest'));
    } catch (err) {
      console.error('Meeting pipeline self-test failed:', err);
      setError(
        typeof err === 'object' && err !== null && 'message' in err
          ? String((err as { message: unknown }).message)
          : String(err),
      );
    } finally {
      setIsRunning(false);
    }
  };

  return (
    <div className="rounded-lg border border-border bg-card/40 overflow-hidden">
      <div className="flex items-center justify-between gap-3 px-4 py-3 border-b border-border">
        <span className="text-xs font-bold text-foreground flex items-center gap-2">
          <Mic className="w-4 h-4 text-primary" aria-hidden="true" />
          Meeting Pipeline Checks
        </span>
        <div className="flex items-center gap-2">
          {report && (
            <span
              className={`text-[11px] font-mono px-2 py-0.5 rounded-md border ${
                report.failed === 0
                  ? 'text-emerald-600 dark:text-emerald-400 bg-emerald-500/10 border-emerald-500/20'
                  : 'text-destructive bg-destructive/10 border-destructive/25'
              }`}
            >
              {report.passed}/{report.checks.length} passed · {report.duration_ms} ms
            </span>
          )}
          <button
            type="button"
            onClick={run}
            disabled={isRunning}
            className="flex items-center gap-1.5 px-3 py-1.5 rounded-md bg-primary hover:bg-primary/90 text-primary-foreground text-xs font-semibold transition-colors disabled:opacity-50 cursor-pointer focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
          >
            {isRunning ? (
              <Loader2 className="w-3.5 h-3.5 animate-spin" />
            ) : (
              <Play className="w-3.5 h-3.5" />
            )}
            {report ? 'Run again' : 'Run checks'}
          </button>
        </div>
      </div>

      <div className="p-4 space-y-3">
        <p className="text-[11px] text-muted-foreground max-w-prose">
          Runs the speech gate, the hallucination screen and the speaker
          separation against synthesized audio — and, where a Whisper model is
          installed, asks that model to transcribe thirty seconds of room tone so
          you can see for yourself what it invents and that none of it reaches a
          transcript. Nothing here reads or writes a recording.
        </p>

        {error && (
          <p className="flex items-start gap-1.5 text-[11px] text-destructive">
            <AlertTriangle className="w-3.5 h-3.5 shrink-0 mt-px" aria-hidden="true" />
            {error}
          </p>
        )}

        {report?.whisper_on_silence && (
          <div className="rounded-lg border border-amber-500/25 bg-amber-500/5 px-3 py-2 space-y-1">
            <p className="text-[11px] font-semibold text-amber-800 dark:text-amber-200">
              Your Whisper model, given thirty seconds of room tone, produced:
            </p>
            <p className="text-[11px] font-mono text-muted-foreground max-h-24 overflow-y-auto select-text">
              {report.whisper_on_silence}
            </p>
            <p className="text-[11px] text-muted-foreground">
              This is the hallucination the pipeline exists to catch. The check
              above says whether the gate stopped it reaching a transcript.
            </p>
          </div>
        )}

        {report && !report.whisper_checked && (
          <p className="text-[11px] text-muted-foreground">
            No Whisper model is configured, so the checks that use one were
            skipped. The rest ran.
          </p>
        )}

        {report && (
          <ul className="space-y-1.5">
            {report.checks.map((check) => (
              <CheckRow key={check.id} check={check} />
            ))}
          </ul>
        )}

        {!report && !isRunning && (
          <p className="text-[11px] text-muted-foreground/70 italic">
            Not run yet.
          </p>
        )}
      </div>
    </div>
  );
};
