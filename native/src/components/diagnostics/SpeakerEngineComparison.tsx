import React, { useEffect, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { AlertTriangle, Check, Loader2, Play, Users, X } from 'lucide-react';
import type {
  EngineComparison,
  EngineOutcome,
  MeetingSession,
} from '../../types';

/**
 * How confident a roster is, from the numbers the engine reported.
 *
 * Deliberately three states rather than two. "Separated, but not confidently"
 * is the case where two similar voices were split and Relay cannot be sure it
 * was right — collapsing that into either "good" or "bad" is what would make
 * the comparison misleading.
 */
function verdict(outcome: EngineOutcome): {
  tone: 'good' | 'caution' | 'bad';
  text: string;
} {
  if (outcome.error) return { tone: 'bad', text: 'Could not run' };

  const { cluster_count, well_separated, silhouette, singleton_speaker_count } =
    outcome.diarization.report;

  if (cluster_count === 0) return { tone: 'bad', text: 'Found nobody' };
  if (cluster_count === 1) return { tone: 'caution', text: 'One voice' };
  if (well_separated) {
    return {
      tone: 'good',
      text: `${cluster_count} voices, clearly apart`,
    };
  }
  return {
    tone: 'caution',
    text:
      singleton_speaker_count && singleton_speaker_count > 0
        ? `${cluster_count} voices, one heard only once`
        : `${cluster_count} voices, not clearly apart${
            silhouette ? ` (${silhouette.toFixed(2)})` : ''
          }`,
  };
}

const TONE_CLASS = {
  good: 'text-emerald-600 dark:text-emerald-400 bg-emerald-500/10 border-emerald-500/25',
  caution: 'text-amber-700 dark:text-amber-300 bg-amber-500/10 border-amber-500/25',
  bad: 'text-destructive bg-destructive/10 border-destructive/25',
} as const;

const EngineCard: React.FC<{ outcome: EngineOutcome; isActive: boolean }> = ({
  outcome,
  isActive,
}) => {
  const { tone, text } = verdict(outcome);
  const report = outcome.diarization.report;

  return (
    <div
      className={`rounded-lg border p-3 space-y-2 ${
        isActive ? 'border-primary/50 bg-primary/5' : 'border-border bg-card'
      }`}
    >
      <div className="flex items-center justify-between gap-2">
        <span className="text-xs font-semibold text-foreground flex items-center gap-1.5">
          {outcome.label}
          {isActive && (
            <span className="text-[9px] font-mono px-1.5 py-0.5 rounded bg-primary/15 text-primary">
              in use
            </span>
          )}
        </span>
        <span
          className={`text-[10px] font-medium px-1.5 py-0.5 rounded border ${TONE_CLASS[tone]}`}
        >
          {text}
        </span>
      </div>

      <p className="text-[11px] text-muted-foreground">{outcome.summary}</p>

      {outcome.error ? (
        <p className="flex items-start gap-1.5 text-[11px] text-destructive">
          <AlertTriangle className="w-3.5 h-3.5 shrink-0 mt-px" aria-hidden="true" />
          {outcome.error}
        </p>
      ) : (
        <div className="space-y-1 text-[11px] font-mono text-muted-foreground">
          <div className="flex items-center gap-1.5 flex-wrap">
            <span>turns per speaker:</span>
            {outcome.speaker_sizes.length > 0 ? (
              outcome.speaker_sizes.map((size, index) => (
                <span
                  key={index}
                  className="px-1.5 rounded bg-muted text-foreground"
                  title={`Speaker ${index + 1} spoke ${size} time${size === 1 ? '' : 's'}`}
                >
                  {size}
                </span>
              ))
            ) : (
              <span>none</span>
            )}
          </div>
          <div>
            you: {report.local_cluster === null || report.local_cluster === undefined
              ? 'not identified'
              : `speaker ${report.local_cluster + 1}`}
            {report.unplaced_count > 0 && ` · ${report.unplaced_count} unplaced`}
            {report.skipped_count > 0 && ` · ${report.skipped_count} skipped`}
          </div>
          <div>{report.duration_ms} ms</div>
        </div>
      )}
    </div>
  );
};

/**
 * Diagnostics › Meeting Pipeline › speaker engines.
 *
 * A meeting is expensive to produce and cheap to re-analyse. Judging which
 * method separates speakers correctly used to mean holding a new meeting for
 * each one, which is why two wrong implementations shipped. This runs all three
 * over a recording the user already has and already recognises, so the choice
 * is made by looking rather than by guessing.
 *
 * It reads the stored audio and writes nothing, so running it can never make a
 * meeting worse.
 */
export const SpeakerEngineComparison: React.FC = () => {
  const [meetings, setMeetings] = useState<MeetingSession[]>([]);
  const [selected, setSelected] = useState<string>('');
  const [expected, setExpected] = useState<string>('');
  const [comparison, setComparison] = useState<EngineComparison | null>(null);
  const [isRunning, setIsRunning] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    invoke<MeetingSession[]>('list_meetings_v2')
      .then((all) => {
        setMeetings(all);
        if (all.length > 0) setSelected((current) => current || all[0].id);
      })
      .catch((err) => console.error('Failed to list meetings:', err));
  }, []);

  const run = async () => {
    if (!selected) return;
    setIsRunning(true);
    setError(null);
    setComparison(null);
    try {
      const parsed = Number.parseInt(expected, 10);
      setComparison(
        await invoke<EngineComparison>('compare_meeting_v2_speaker_engines', {
          sessionId: selected,
          expectedSpeakers: Number.isFinite(parsed) && parsed > 0 ? parsed : null,
        }),
      );
    } catch (err) {
      console.error('Engine comparison failed:', err);
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
          <Users className="w-4 h-4 text-primary" aria-hidden="true" />
          Speaker Separation — compare methods
        </span>
      </div>

      <div className="p-4 space-y-3">
        <p className="text-[11px] text-muted-foreground max-w-prose">
          Runs all three methods over one recording you already have, so you can
          see which gets your meetings right without holding a new meeting for
          each. Reads the stored audio; changes nothing.
        </p>

        <div className="flex flex-wrap items-end gap-2">
          <label className="flex flex-col gap-1 flex-1 min-w-[14rem]">
            <span className="text-[11px] text-muted-foreground">Recording</span>
            <select
              value={selected}
              onChange={(e) => setSelected(e.target.value)}
              className="w-full text-xs bg-input border border-border rounded-md px-2 py-1.5 text-foreground"
            >
              {meetings.length === 0 && <option value="">No recordings yet</option>}
              {meetings.map((meeting) => (
                <option key={meeting.id} value={meeting.id}>
                  {meeting.title} · {Math.round(meeting.duration_seconds / 60)}m
                </option>
              ))}
            </select>
          </label>

          <label className="flex flex-col gap-1">
            <span className="text-[11px] text-muted-foreground">People</span>
            <input
              type="number"
              min={0}
              max={20}
              value={expected}
              placeholder="Auto"
              onChange={(e) => setExpected(e.target.value)}
              className="w-20 text-xs bg-input border border-border rounded-md px-2 py-1.5 text-foreground"
              aria-label="How many people spoke"
              title="If you know how many people spoke, say so — it is the surest way to get the roster right."
            />
          </label>

          <button
            type="button"
            onClick={run}
            disabled={isRunning || !selected}
            className="flex items-center gap-1.5 px-3 py-1.5 rounded-md bg-primary hover:bg-primary/90 text-primary-foreground text-xs font-semibold transition-colors disabled:opacity-50 cursor-pointer focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
          >
            {isRunning ? (
              <Loader2 className="w-3.5 h-3.5 animate-spin" />
            ) : (
              <Play className="w-3.5 h-3.5" />
            )}
            Compare
          </button>
        </div>

        {error && (
          <p className="flex items-start gap-1.5 text-[11px] text-destructive">
            <AlertTriangle className="w-3.5 h-3.5 shrink-0 mt-px" aria-hidden="true" />
            {error}
          </p>
        )}

        {comparison && (
          <>
            <div className="grid gap-2 md:grid-cols-3">
              {comparison.outcomes.map((outcome) => (
                <EngineCard
                  key={outcome.id}
                  outcome={outcome}
                  isActive={outcome.engine === comparison.active}
                />
              ))}
            </div>
            <p className="text-[11px] text-muted-foreground max-w-prose">
              Count the voices you know were in that meeting and pick the method
              that matches. If none does, set the number of people above and run
              again — saying how many spoke is the surest way to get the roster
              right. Change the method in Settings › Meetings.
            </p>
          </>
        )}

        {!comparison && !isRunning && (
          <p className="text-[11px] text-muted-foreground/70 italic">Not run yet.</p>
        )}
      </div>
    </div>
  );
};

/** Exported for tests: the verdict shown on each engine card. */
export const __verdict = verdict;
