import React from 'react';
import { Check, Circle, X, ShieldCheck, UserCheck, AlertCircle, Sparkles } from 'lucide-react';
import { Speaker, SpeakerAssignment, SpeakerConfidenceLevel } from '../../types';

interface MeetingSpeakerEvidenceInspectorProps {
  speaker: Speaker;
  assignment?: SpeakerAssignment | null;
  onClose: () => void;
  onStartRename?: () => void;
  onStartMerge?: () => void;
}

const CONFIDENCE_BADGE: Record<
  SpeakerConfidenceLevel,
  { label: string; bg: string; text: string; icon: React.ReactNode }
> = {
  CONFIRMED: {
    label: 'Confirmed',
    bg: 'bg-emerald-500/10 dark:bg-emerald-500/20 border-emerald-500/30',
    text: 'text-emerald-700 dark:text-emerald-400',
    icon: <ShieldCheck className="w-3.5 h-3.5 text-emerald-600 dark:text-emerald-400" />,
  },
  HIGH: {
    label: 'High Confidence',
    bg: 'bg-blue-500/10 dark:bg-blue-500/20 border-blue-500/30',
    text: 'text-blue-700 dark:text-blue-400',
    icon: <UserCheck className="w-3.5 h-3.5 text-blue-600 dark:text-blue-400" />,
  },
  LIKELY: {
    label: 'Likely',
    bg: 'bg-sky-500/10 dark:bg-sky-500/20 border-sky-500/30',
    text: 'text-sky-700 dark:text-sky-400',
    icon: <Sparkles className="w-3.5 h-3.5 text-sky-600 dark:text-sky-400" />,
  },
  UNRESOLVED: {
    label: 'Unresolved',
    bg: 'bg-amber-500/10 dark:bg-amber-500/20 border-amber-500/30',
    text: 'text-amber-700 dark:text-amber-400',
    icon: <AlertCircle className="w-3.5 h-3.5 text-amber-600 dark:text-amber-400" />,
  },
  UNKNOWN: {
    label: 'Unknown',
    bg: 'bg-muted border-border',
    text: 'text-muted-foreground',
    icon: <Circle className="w-3.5 h-3.5 text-muted-foreground" />,
  },
};

export const MeetingSpeakerEvidenceInspector: React.FC<MeetingSpeakerEvidenceInspectorProps> = ({
  speaker,
  assignment,
  onClose,
  onStartRename,
  onStartMerge,
}) => {
  const confidenceLevel = assignment?.confidence_level ?? (speaker.is_local_user ? 'CONFIRMED' : 'UNRESOLVED');
  const badge = CONFIDENCE_BADGE[confidenceLevel] ?? CONFIDENCE_BADGE.UNRESOLVED;
  const evidence = assignment?.evidence;

  const simValue = evidence?.similarity != null ? `${(evidence.similarity * 100).toFixed(0)}%` : null;

  return (
    <div className="rounded-lg border border-border bg-card p-4 shadow-sm text-xs space-y-3.5">
      {/* Header */}
      <div className="flex items-start justify-between gap-2 border-b border-border/60 pb-2.5">
        <div>
          <h4 className="font-semibold text-sm text-foreground flex items-center gap-1.5">
            {speaker.display_name?.trim() || speaker.fallback_label}
            {speaker.is_local_user && (
              <span className="text-[10px] font-mono text-primary font-bold px-1.5 py-0.5 rounded bg-primary/10">
                You
              </span>
            )}
          </h4>
          <p className="text-[11px] text-muted-foreground mt-0.5">
            Speaker ID: <code className="font-mono text-[10px]">{speaker.id}</code>
          </p>
        </div>
        <button
          onClick={onClose}
          className="text-muted-foreground hover:text-foreground p-1 rounded hover:bg-accent cursor-pointer"
          aria-label="Close evidence inspector"
        >
          <X className="w-3.5 h-3.5" />
        </button>
      </div>

      {/* Speech Coverage & Identity Confidence */}
      <div className="space-y-2 bg-background/50 rounded-md p-2.5 border border-border/40">
        <div className="flex items-center justify-between">
          <span className="text-muted-foreground font-medium">Speech Coverage</span>
          <span className="font-mono text-[11px] font-semibold text-foreground">
            {speaker.segment_count} segment{speaker.segment_count === 1 ? '' : 's'}
          </span>
        </div>
        <div className="flex items-center justify-between">
          <span className="text-muted-foreground font-medium">Identity Confidence</span>
          <div className={`flex items-center gap-1.5 px-2 py-0.5 rounded-full border text-[11px] font-medium ${badge.bg} ${badge.text}`}>
            {badge.icon}
            <span>{badge.label}</span>
          </div>
        </div>
      </div>

      {/* Multi-Signal Evidence Fusion */}
      <div className="space-y-2">
        <span className="text-muted-foreground font-medium uppercase tracking-wider text-[10px]">
          Multi-Signal Evidence
        </span>
        <div className="space-y-1.5 bg-background/50 rounded-md p-2.5 border border-border/40">
          {/* Voice Similarity */}
          <div className="flex items-center justify-between py-0.5">
            <span className="flex items-center gap-1.5 text-foreground">
              {simValue ? (
                <Check className="w-3.5 h-3.5 text-emerald-500" />
              ) : (
                <Circle className="w-3.5 h-3.5 text-muted-foreground/50" />
              )}
              Voice Similarity
            </span>
            <span className="font-mono text-[11px] text-muted-foreground">
              {simValue ? `${simValue}` : 'None'}
            </span>
          </div>

          {/* Diarization Cluster */}
          <div className="flex items-center justify-between py-0.5">
            <span className="flex items-center gap-1.5 text-foreground">
              {evidence?.cluster_id != null ? (
                <Check className="w-3.5 h-3.5 text-emerald-500" />
              ) : (
                <Circle className="w-3.5 h-3.5 text-muted-foreground/50" />
              )}
              Acoustic Cluster
            </span>
            <span className="font-mono text-[11px] text-muted-foreground">
              {evidence?.cluster_id != null ? `Cluster ${evidence.cluster_id}` : 'None'}
            </span>
          </div>

          {/* Calendar Attendee Candidate */}
          <div className="flex items-center justify-between py-0.5">
            <span className="flex items-center gap-1.5 text-foreground">
              {evidence?.calendar_candidate ? (
                <Check className="w-3.5 h-3.5 text-blue-500" />
              ) : (
                <Circle className="w-3.5 h-3.5 text-muted-foreground/50" />
              )}
              Calendar Candidate
            </span>
            <span className="text-[11px] text-muted-foreground truncate max-w-[140px]" title={evidence?.calendar_candidate ?? ''}>
              {evidence?.calendar_candidate ?? 'None'}
            </span>
          </div>

          {/* Contextual Mention */}
          <div className="flex items-center justify-between py-0.5">
            <span className="flex items-center gap-1.5 text-foreground">
              {evidence?.contextual_mention ? (
                <Check className="w-3.5 h-3.5 text-sky-500" />
              ) : (
                <Circle className="w-3.5 h-3.5 text-muted-foreground/50" />
              )}
              Contextual Mention
            </span>
            <span className="text-[11px] text-muted-foreground truncate max-w-[140px]" title={evidence?.contextual_mention ?? ''}>
              {evidence?.contextual_mention ?? 'None'}
            </span>
          </div>

          {/* Temporal Consistency */}
          <div className="flex items-center justify-between py-0.5">
            <span className="flex items-center gap-1.5 text-foreground">
              {evidence?.temporal_consistency ? (
                <Check className="w-3.5 h-3.5 text-emerald-500" />
              ) : (
                <Circle className="w-3.5 h-3.5 text-muted-foreground/50" />
              )}
              Temporal Continuity
            </span>
            <span className="text-[11px] text-muted-foreground">
              {evidence?.temporal_consistency ?? 'Neutral'}
            </span>
          </div>
        </div>
      </div>

      {/* Evidence Notes */}
      {evidence?.notes && (
        <p className="text-[11px] text-muted-foreground bg-accent/30 rounded p-2 border border-border/30 italic">
          {evidence.notes}
        </p>
      )}

      {/* User Actions */}
      <div className="flex items-center gap-2 pt-1">
        {onStartRename && (
          <button
            onClick={() => {
              onClose();
              onStartRename();
            }}
            className="flex-1 py-1.5 px-2 rounded-md bg-primary text-primary-foreground font-medium text-xs hover:bg-primary/90 transition-colors text-center cursor-pointer"
          >
            Assign / Rename
          </button>
        )}
        {onStartMerge && (
          <button
            onClick={() => {
              onClose();
              onStartMerge();
            }}
            className="py-1.5 px-2 rounded-md bg-accent hover:bg-accent/80 text-foreground font-medium text-xs transition-colors cursor-pointer"
          >
            Merge
          </button>
        )}
      </div>
    </div>
  );
};
