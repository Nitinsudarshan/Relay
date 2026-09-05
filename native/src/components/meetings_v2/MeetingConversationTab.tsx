import React, { useState, useMemo } from 'react';
import { AudioLines, Check, GitMerge, Info, Pencil, Play, RefreshCw, Users, X } from 'lucide-react';
import type { Conversation, Diarization, Speaker } from '../../types';
import { formatTimestamp, speakerLabel } from './meetingProcessing';
import { MeetingSpeakerEvidenceInspector } from './MeetingSpeakerEvidenceInspector';

interface MeetingConversationTabProps {
  conversation?: Conversation | null;
  speakers: Speaker[];
  /** The acoustic separation the roster came from, if one has run. */
  diarization?: Diarization | null;
  onRenameSpeaker: (speakerId: string, displayName: string | null) => void;
  onMergeSpeaker?: (sourceSpeakerId: string, targetSpeakerId: string) => void;
  onSeek?: (startTimeS: number) => void;
  /** Re-runs acoustic speaker separation over the stored audio. */
  onIdentifySpeakers: (expectedSpeakers: number | null) => void;
  isRenaming: boolean;
  isIdentifying: boolean;
  /** True when the conversation transcript is switched off in settings. */
  isDisabled: boolean;
}

/**
 * How a speaker was established, per §2.3 of
 * `Meeting-rules/meeting_speaker_identification.md`.
 *
 * These states are deliberately not collapsed into "unknown": they call for
 * different actions from the user. A confirmed name needs nothing; an inferred
 * cluster is worth a glance; a channel-only roster means separation never ran.
 */
const ORIGIN_MARKS: Record<Speaker['origin'], { mark: string; title: string }> = {
  MANUAL: { mark: '✓', title: 'You named this speaker' },
  SELF_VOICE_ANCHOR: { mark: '⚓', title: 'Matched local self-voice anchor' },
  CALENDAR: { mark: '📅', title: 'Candidate from calendar' },
  CONTEXTUAL_INFERENCE: { mark: '💬', title: 'Inferred from context' },
  DIARIZATION: {
    mark: '~',
    title: 'A distinct voice was isolated but nobody has named it',
  },
  CHANNEL: {
    mark: '',
    title: 'Told apart by capture channel — your microphone versus everyone else',
  },
};

const SpeakerRow: React.FC<{
  speaker: Speaker;
  otherSpeakers: Speaker[];
  coveragePercent?: number;
  onRename: (newName: string | null) => void;
  onMerge?: (targetSpeakerId: string) => void;
  onInspect?: () => void;
  isInspected?: boolean;
  isRenaming: boolean;
}> = ({
  speaker,
  otherSpeakers,
  coveragePercent,
  onRename,
  onMerge,
  onInspect,
  isInspected,
  isRenaming,
}) => {
  const [editing, setEditing] = useState(false);
  const [merging, setMerging] = useState(false);
  const [draft, setDraft] = useState(speaker.display_name ?? '');

  const commit = () => {
    setEditing(false);
    const trimmed = draft.trim();
    onRename(trimmed.length > 0 ? trimmed : null);
  };

  if (editing) {
    return (
      <div className="flex items-center gap-1.5">
        <input
          autoFocus
          value={draft}
          onChange={(e) => setDraft(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === 'Enter') commit();
            if (e.key === 'Escape') setEditing(false);
          }}
          placeholder={speaker.fallback_label}
          className="w-32 px-2 py-1 rounded-md bg-background border border-input text-xs text-foreground outline-none focus-visible:ring-2 focus-visible:ring-ring"
        />
        <button
          onClick={commit}
          disabled={isRenaming}
          className="p-1 rounded-md bg-emerald-500/10 hover:bg-emerald-500/20 text-emerald-600 dark:text-emerald-400 cursor-pointer"
          aria-label={`Save name for ${speaker.fallback_label}`}
        >
          <Check className="w-3 h-3" />
        </button>
        <button
          onClick={() => setEditing(false)}
          className="p-1 rounded-md bg-muted hover:bg-accent text-muted-foreground cursor-pointer"
          aria-label="Cancel rename"
        >
          <X className="w-3 h-3" />
        </button>
      </div>
    );
  }

  if (merging) {
    return (
      <div className="flex items-center gap-1.5 px-2 py-1 rounded-md bg-accent/40 border border-border text-xs">
        <span className="text-[10px] text-muted-foreground">Merge into:</span>
        <select
          className="text-xs bg-background border border-input rounded px-1.5 py-0.5 text-foreground"
          onChange={(e) => {
            const targetId = e.target.value;
            if (targetId) {
              setMerging(false);
              onMerge?.(targetId);
            }
          }}
          defaultValue=""
        >
          <option value="" disabled>Select speaker…</option>
          {otherSpeakers.map((s) => (
            <option key={s.id} value={s.id}>
              {s.display_name?.trim() || s.fallback_label}
            </option>
          ))}
        </select>
        <button
          onClick={() => setMerging(false)}
          className="p-0.5 text-muted-foreground hover:text-foreground cursor-pointer"
          title="Cancel merge"
        >
          <X className="w-3 h-3" />
        </button>
      </div>
    );
  }

  const origin = ORIGIN_MARKS[speaker.origin] ?? ORIGIN_MARKS.CHANNEL;

  return (
    <div className="flex items-center gap-1">
      <button
        onClick={() => {
          setDraft(speaker.display_name ?? '');
          setEditing(true);
        }}
        className="group flex items-center gap-1.5 px-2.5 py-1 rounded-md bg-card hover:bg-accent border border-border text-xs text-foreground transition-colors cursor-pointer"
        title={`Rename ${speaker.fallback_label} (id: ${speaker.id}) — ${coveragePercent != null ? `${coveragePercent}% speech coverage · ` : ''}${origin.title}`}
      >
        <span>
          {speaker.display_name?.trim() || speaker.fallback_label}
        </span>
        {coveragePercent != null && (
          <span className="text-[10px] text-muted-foreground font-mono">
            {coveragePercent}%
          </span>
        )}
        {origin.mark && (
          <span
            className={`text-[10px] font-mono ${
              speaker.origin === 'MANUAL'
                ? 'text-emerald-600 dark:text-emerald-400'
                : 'text-muted-foreground'
            }`}
            aria-hidden="true"
          >
            {origin.mark}
          </span>
        )}
        {speaker.is_local_user && (
          <span className="text-[9px] font-mono text-primary font-semibold">you</span>
        )}
        <Pencil className="w-3 h-3 text-muted-foreground opacity-0 group-hover:opacity-100 transition-opacity" />
      </button>

      <button
        onClick={onInspect}
        className={`p-1 rounded-md transition-colors cursor-pointer ${
          isInspected
            ? 'bg-primary/10 text-primary font-medium'
            : 'hover:bg-accent text-muted-foreground hover:text-foreground'
        }`}
        title={`Inspect speaker evidence for ${speaker.fallback_label}`}
        aria-label={`Inspect evidence for ${speaker.fallback_label}`}
      >
        <Info className="w-3 h-3" />
      </button>

      {otherSpeakers.length > 0 && onMerge && (
        <button
          onClick={() => setMerging(true)}
          className="p-1 rounded-md hover:bg-accent text-muted-foreground hover:text-primary transition-colors cursor-pointer"
          title={`Merge ${speaker.fallback_label} into another speaker`}
        >
          <GitMerge className="w-3 h-3" />
        </button>
      )}
    </div>
  );
};

/**
 * The readable transcript: chronological, speaker-labelled, sentence grouped.
 */
export const MeetingConversationTab: React.FC<MeetingConversationTabProps> = ({
  conversation,
  speakers,
  diarization,
  onRenameSpeaker,
  onMergeSpeaker,
  onSeek,
  onIdentifySpeakers,
  isRenaming,
  isIdentifying,
  isDisabled,
}) => {
  // Declared before any early return: hooks must run unconditionally.
  const [expectedDraft, setExpectedDraft] = useState<string>('');
  const [inspectedSpeakerId, setInspectedSpeakerId] = useState<string | null>(null);

  if (isDisabled) {
    return (
      <div className="flex-1 flex flex-col items-center justify-center gap-2 p-8 text-center">
        <Users className="w-8 h-8 text-muted-foreground/40" />
        <p className="text-sm font-medium text-foreground">Conversation transcript is off</p>
        <p className="text-xs text-muted-foreground max-w-sm">
          Turn it on in Settings › Meetings. The raw transcript is unaffected either
          way.
        </p>
      </div>
    );
  }

  const turns = conversation?.turns ?? [];

  if (turns.length === 0) {
    return (
      <div className="flex-1 flex flex-col items-center justify-center gap-2 p-8 text-center">
        <Users className="w-8 h-8 text-muted-foreground/40" />
        <p className="text-sm font-medium text-foreground">No conversation yet</p>
        <p className="text-xs text-muted-foreground max-w-sm">
          This appears once the recording is finished and the transcript has been
          processed.
        </p>
      </div>
    );
  }

  const unattributed = conversation?.unattributed_turn_count ?? 0;
  const report = diarization?.report;
  const isChannelOnly = !report || report.cluster_count === 0;
  const parsedExpected = Number.parseInt(expectedDraft, 10);
  const expected =
    Number.isFinite(parsedExpected) && parsedExpected > 0 ? parsedExpected : null;

  const totalSegments = useMemo(
    () => speakers.reduce((sum, s) => sum + s.segment_count, 0),
    [speakers]
  );

  return (
    <div className="flex-1 overflow-y-auto p-6 space-y-4">
      {speakers.length > 0 && (
        <div className="flex items-center gap-2 flex-wrap pb-3 border-b border-border">
          <span className="text-xs font-semibold text-muted-foreground flex items-center gap-1.5">
            <Users className="w-3.5 h-3.5" />
            Speakers
          </span>
          {speakers.map((speaker) => {
            const coveragePercent =
              totalSegments > 0
                ? Math.round((speaker.segment_count / totalSegments) * 100)
                : undefined;
            return (
              <SpeakerRow
                key={speaker.id}
                speaker={speaker}
                otherSpeakers={speakers.filter((s) => s.id !== speaker.id)}
                coveragePercent={coveragePercent}
                isRenaming={isRenaming}
                onRename={(name) => onRenameSpeaker(speaker.id, name)}
                onMerge={
                  onMergeSpeaker
                    ? (targetId) => onMergeSpeaker(speaker.id, targetId)
                    : undefined
                }
                onInspect={() =>
                  setInspectedSpeakerId((cur) => (cur === speaker.id ? null : speaker.id))
                }
                isInspected={inspectedSpeakerId === speaker.id}
              />
            );
          })}

          <div className="flex items-center gap-1.5 ml-auto">
            <input
              type="number"
              min={0}
              max={12}
              value={expectedDraft}
              placeholder="Auto"
              onChange={(e) => setExpectedDraft(e.target.value)}
              className="w-16 px-2 py-1 rounded-md bg-background border border-input text-xs text-foreground outline-none focus-visible:ring-2 focus-visible:ring-ring"
              aria-label="Expected number of speakers"
              title="How many people spoke, if you know. A hint — it cannot add a speaker the recording does not contain."
            />
            <button
              onClick={() => onIdentifySpeakers(expected)}
              disabled={isIdentifying}
              className="flex items-center gap-1 px-2 py-1 rounded-md bg-accent hover:bg-accent/80 text-xs text-foreground font-medium transition-colors disabled:opacity-50 cursor-pointer"
              title="Re-run acoustic separation to find distinct voices"
            >
              <RefreshCw
                className={`w-3 h-3 ${isIdentifying ? 'animate-spin' : ''}`}
              />
              <span>{isChannelOnly ? 'Identify speakers' : 'Re-identify'}</span>
            </button>
          </div>
        </div>
      )}

      {inspectedSpeakerId && (() => {
        const target = speakers.find((s) => s.id === inspectedSpeakerId);
        if (!target) return null;
        return (
          <div className="max-w-md my-2 animate-in fade-in slide-in-from-top-1 duration-150">
            <MeetingSpeakerEvidenceInspector
              speaker={target}
              onClose={() => setInspectedSpeakerId(null)}
              onStartRename={() => onRenameSpeaker(target.id, target.display_name ?? null)}
              onStartMerge={
                onMergeSpeaker && speakers.length > 1
                  ? () => onMergeSpeaker(target.id, speakers.find((s) => s.id !== target.id)!.id)
                  : undefined
              }
            />
          </div>
        );
      })()}

      {report && report.cluster_count > 0 && !report.well_separated && (
        <p className="flex items-start gap-2 text-[11px] px-3 py-2 rounded-lg bg-amber-500/10 border border-amber-500/25 text-amber-800 dark:text-amber-200">
          <Info className="w-3.5 h-3.5 shrink-0 mt-0.5" />
          <span>
            These {report.cluster_count} voices are not cleanly separated — they
            sit about as close to each other as each one does to itself. Treat
            the split as provisional: two similar voices on the same channel are
            beyond what Relay can tell apart without a voiceprint. Setting the
            expected speaker count above usually helps.
          </span>
        </p>
      )}

      {isChannelOnly && speakers.length > 0 && (
        <p className="flex items-start gap-2 text-[11px] text-muted-foreground px-3 py-2 rounded-lg bg-muted/40 border border-border">
          <Info className="w-3.5 h-3.5 shrink-0 mt-0.5 text-primary" />
          <span>
            Speakers here are told apart by capture channel only — your
            microphone versus everyone else — so everyone on the call shares one
            label however many of them there were. <strong>Identify speakers</strong>{' '}
            reads the recorded audio and separates the individual voices.
          </span>
        </p>
      )}

      {unattributed > 0 && (
        <p className="flex items-start gap-2 text-[11px] text-muted-foreground px-3 py-2 rounded-lg bg-muted/40 border border-border">
          <Info className="w-3.5 h-3.5 shrink-0 mt-0.5 text-primary" />
          <span>
            {unattributed} {unattributed === 1 ? 'stretch' : 'stretches'} could not
            be attributed to anyone
            {report && report.unplaced_count > 0
              ? ` — ${report.unplaced_count} of them carried too little voice to place`
              : ''}
            . Relay leaves those blank rather than guessing.
          </span>
        </p>
      )}

      <div className="space-y-3">
        {turns.map((turn) => {
          const label = speakerLabel(speakers, turn.speaker_id);
          const isMe = speakers.find((s) => s.id === turn.speaker_id)?.is_local_user;

          return (
            <div key={turn.id} className="flex flex-col gap-1">
              <div className="flex items-center gap-2">
                <span
                  className={`text-xs font-semibold ${
                    isMe ? 'text-primary font-bold' : turn.speaker_id ? 'text-foreground' : 'text-muted-foreground'
                  }`}
                >
                  {label}
                </span>
                <button
                  type="button"
                  onClick={() => onSeek?.(turn.start_time_s)}
                  className="group flex items-center gap-1 text-[10px] font-mono text-muted-foreground hover:text-primary hover:underline cursor-pointer"
                  title={`Play from ${formatTimestamp(turn.start_time_s)}`}
                >
                  <Play className="w-2.5 h-2.5 opacity-60 group-hover:opacity-100 text-primary" />
                  <span>{formatTimestamp(turn.start_time_s)}</span>
                </button>
              </div>
              <p className="text-sm text-foreground/90 leading-relaxed font-sans select-text pl-0.5">
                {turn.text}
              </p>
            </div>
          );
        })}
      </div>
    </div>
  );
};

