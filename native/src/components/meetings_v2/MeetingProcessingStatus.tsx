import React from 'react';
import { AlertTriangle, Check, Loader2, Minus, RefreshCw } from 'lucide-react';
import type { MeetingProcessing, StageState } from '../../types';
import { processingHeadline } from './meetingProcessing';

interface MeetingProcessingStatusProps {
  processing?: MeetingProcessing | null;
  /** Shown while a stage this component cannot see is in flight. */
  isBusy?: boolean;
  onRetry?: () => void;
}

const TONE_CLASSES: Record<string, string> = {
  idle: 'text-muted-foreground bg-muted/50 border-border',
  busy: 'text-foreground bg-accent border-border',
  ok: 'text-emerald-600 dark:text-emerald-400 bg-emerald-500/10 border-emerald-500/20',
  warn: 'text-amber-600 dark:text-amber-400 bg-amber-500/10 border-amber-500/25',
  error: 'text-destructive bg-destructive/10 border-destructive/25',
};

/** The stages worth showing a user, in pipeline order. */
const VISIBLE_STAGES: { key: keyof MeetingProcessing['stages']; label: string }[] = [
  { key: 'normalization', label: 'Transcript' },
  { key: 'conversation', label: 'Conversation' },
  { key: 'summary', label: 'Summary' },
];

const StageChip: React.FC<{ label: string; stage?: StageState | null }> = ({
  label,
  stage,
}) => {
  const status = stage?.status ?? 'NOT_RUN';
  const icon =
    status === 'SUCCESS' ? (
      <Check className="w-3 h-3 text-emerald-600 dark:text-emerald-400" />
    ) : status === 'FAILED' ? (
      <AlertTriangle className="w-3 h-3 text-destructive" />
    ) : status === 'RUNNING' ? (
      <Loader2 className="w-3 h-3 text-primary animate-spin" />
    ) : (
      <Minus className="w-3 h-3 text-muted-foreground/40" />
    );

  const title =
    status === 'SKIPPED'
      ? stage?.error ?? 'Turned off in settings'
      : stage?.error ?? undefined;

  return (
    <span
      className="flex items-center gap-1.5 text-[11px] text-muted-foreground font-medium"
      title={title}
    >
      {icon}
      <span className={status === 'SKIPPED' ? 'text-muted-foreground/60 line-through' : ''}>
        {label}
      </span>
    </span>
  );
};

/**
 * A compact, honest report of what processing produced.
 */
export const MeetingProcessingStatus: React.FC<MeetingProcessingStatusProps> = ({
  processing,
  isBusy,
  onRetry,
}) => {
  const headline = processingHeadline(processing);
  const tone = isBusy ? 'busy' : headline.tone;
  const label = isBusy ? 'Processing meeting…' : headline.label;

  const summaryStage = processing?.stages.summary;
  const summaryFailed = summaryStage?.status === 'FAILED';
  const summary = processing?.summary;
  const usedFallback = summary?.fallback_used === true;
  const modelRejected = summary?.provider_output_status === 'REJECTED';
  const rejectionCodes = (summary?.rejected_issues ?? [])
    .map((issue) => issue.code)
    .join(', ');

  return (
    <div className="flex flex-col gap-2 px-6 py-2.5 border-b border-border bg-card/40">
      <div className="flex items-center gap-3 flex-wrap">
        <span
          className={`text-[11px] font-semibold px-2 py-0.5 rounded-md border ${
            TONE_CLASSES[tone] ?? TONE_CLASSES.idle
          }`}
        >
          {label}
        </span>

        <div className="flex items-center gap-4">
          {VISIBLE_STAGES.map(({ key, label: stageLabel }) => (
            <StageChip
              key={key}
              label={stageLabel}
              stage={processing?.stages[key]}
            />
          ))}
        </div>

        {summary && (
          <span className="text-[10px] font-mono text-muted-foreground ml-auto">
            {summary.mode.toLowerCase()} ·{' '}
            {usedFallback ? 'no model' : summary.model} · v
            {summary.processing_version}
          </span>
        )}
      </div>

      {summaryFailed && (
        <div className="flex items-center gap-2 text-[11px] text-destructive">
          <AlertTriangle className="w-3.5 h-3.5 shrink-0" />
          <span className="flex-1 min-w-0 truncate">
            Summary unavailable — {summaryStage?.error ?? 'processing did not complete'}
          </span>
          {onRetry && (
            <button
              onClick={onRetry}
              className="flex items-center gap-1 px-2 py-0.5 rounded-md bg-muted hover:bg-accent border border-border text-foreground font-medium transition-colors cursor-pointer shrink-0"
            >
              <RefreshCw className="w-3 h-3" />
              <span>Retry</span>
            </button>
          )}
        </div>
      )}

      {!summaryFailed && usedFallback && (
        <p className="text-[11px] text-amber-600 dark:text-amber-400">
          {modelRejected ? (
            <>
              Generated from fallback because the model output failed validation
              {rejectionCodes ? ` (${rejectionCodes})` : ''}. Regenerate to try the
              model again.
            </>
          ) : (
            <>
              Written without a language model
              {summaryStage?.error ? ` — ${summaryStage.error}` : ''}. The transcript
              is unaffected; retry once a model is available.
            </>
          )}
        </p>
      )}
    </div>
  );
};

