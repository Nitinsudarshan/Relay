import React from 'react';
import { AlertTriangle, Calendar, Clock, FileText, Layers, Users } from 'lucide-react';
import type {
  MeetingMetadata,
  MeetingSession,
  Participant,
  SpeakerMethod,
} from '../../types';
import { formatDuration } from './meetingsViewState';

interface MeetingMetadataHeaderProps {
  /** Counted metadata, once the meeting has been prepared. */
  metadata?: MeetingMetadata | null;
  /** The source record, used while metadata does not exist yet. */
  session: MeetingSession;
  /** Live elapsed seconds while this meeting is still recording. */
  liveElapsedSeconds?: number | null;
  /** Words, from the derived transcript when there is one. */
  wordCount: number;
}

/** What established the roster, in the fewest words that stay honest. */
const SPEAKER_METHOD_NOTE: Record<SpeakerMethod, string | null> = {
  NONE: null,
  CHANNEL: 'told apart by capture channel only',
  DIARIZATION: 'separated by voice',
};

/**
 * A chip for one participant.
 *
 * The distinctions §2.3 of `Meeting-rules/meeting_speaker_identification.md`
 * requires are all visible here: a confirmed name carries a check, an inferred
 * one does not, and somebody who was named but never heard is marked as such
 * rather than presented as a speaker.
 */
const ParticipantChip: React.FC<{ participant: Participant }> = ({ participant }) => {
  const spoke = participant.speaking_seconds > 0;
  const share = Math.round(participant.share_of_talk * 100);

  return (
    <span
      className={`inline-flex items-center gap-1 px-2 py-0.5 rounded-md border text-[11px] ${
        participant.is_local_user
          ? 'border-primary/40 bg-primary/10 text-foreground'
          : spoke
            ? 'border-border bg-card text-foreground'
            : 'border-dashed border-border bg-transparent text-muted-foreground'
      }`}
      title={
        spoke
          ? `${formatDuration(participant.speaking_seconds)} of talking (${share}%), ` +
            `${participant.turn_count} turn${participant.turn_count === 1 ? '' : 's'} — ` +
            originNote(participant)
          : `Named in this meeting but never heard — ${originNote(participant)}`
      }
    >
      <span>{participant.label}</span>
      {participant.is_confirmed && (
        <span
          className="text-emerald-600 dark:text-emerald-400 font-mono"
          aria-label="confirmed"
        >
          ✓
        </span>
      )}
      {participant.is_named && !participant.is_confirmed && (
        <span className="text-muted-foreground font-mono" aria-label="unconfirmed">
          ~
        </span>
      )}
      {spoke && share > 0 && (
        <span className="text-muted-foreground font-mono">{share}%</span>
      )}
      {!spoke && <span className="text-muted-foreground">mentioned</span>}
    </span>
  );
};

function originNote(participant: Participant): string {
  switch (participant.origin) {
    case 'LOCAL_USER':
      return 'you, from the microphone';
    case 'CHANNEL':
      return 'everyone on the call shares this label; separate voices to split it';
    case 'DIARIZATION':
      return 'a distinct voice in the recording';
    case 'SELF_INTRODUCED':
      return 'introduced themselves in the meeting';
    case 'MENTIONED':
      return 'named in the meeting, not matched to a voice';
    case 'STATED':
      return 'you said they were here';
    case 'INVITED':
      return 'invited, from the calendar';
  }
}

/**
 * The meeting's own facts, above everything derived from it.
 *
 * Every value here is counted or measured. That is what makes it safe to put
 * at the top: a reader can trust it in a way they cannot trust a generated
 * sentence, and a 44-minute meeting whose transcript lost four minutes is a
 * different document from one that did not.
 */
export const MeetingMetadataHeader: React.FC<MeetingMetadataHeaderProps> = ({
  metadata,
  session,
  liveElapsedSeconds,
  wordCount,
}) => {
  const duration = liveElapsedSeconds ?? metadata?.duration_seconds ?? session.duration_seconds;
  const startedAt = metadata?.started_at ?? session.started_at ?? session.created_at;
  const participants = metadata?.participants ?? [];
  const health = metadata?.health;
  const methodNote = metadata ? SPEAKER_METHOD_NOTE[metadata.speaker_method] : null;

  const lostSeconds = health?.rejected_seconds ?? 0;
  const withheldSpans = health
    ? Object.values(health.withheld_on_read).reduce((a, b) => a + b, 0)
    : 0;

  return (
    <div className="space-y-2">
      <div className="flex flex-wrap items-center gap-x-3 gap-y-1 text-xs text-muted-foreground">
        <span className="inline-flex items-center gap-1">
          <Calendar className="w-3 h-3" aria-hidden="true" />
          {new Date(startedAt).toLocaleString([], {
            dateStyle: 'medium',
            timeStyle: 'short',
          })}
        </span>
        <span className="inline-flex items-center gap-1">
          <Clock className="w-3 h-3" aria-hidden="true" />
          {formatDuration(duration)}
          {session.paused_seconds > 1 && (
            <span className="text-muted-foreground/70">
              {' '}
              (+{formatDuration(session.paused_seconds)} paused)
            </span>
          )}
        </span>
        <span className="inline-flex items-center gap-1">
          <Users className="w-3 h-3" aria-hidden="true" />
          {participants.length > 0
            ? `${metadata?.speaking_participant_count ?? 0} spoke${
                participants.length > (metadata?.speaking_participant_count ?? 0)
                  ? ` of ${participants.length}`
                  : ''
              }`
            : 'no speakers yet'}
        </span>
        <span className="inline-flex items-center gap-1">
          <FileText className="w-3 h-3" aria-hidden="true" />
          {wordCount.toLocaleString()} words
        </span>
        <span className="inline-flex items-center gap-1">
          <Layers className="w-3 h-3" aria-hidden="true" />
          {session.chunk_count} chunks
        </span>
      </div>

      {participants.length > 0 && (
        <div className="flex flex-wrap items-center gap-1.5">
          {participants.map((participant) => (
            <ParticipantChip
              key={participant.speaker_id ?? participant.label}
              participant={participant}
            />
          ))}
          {methodNote && (
            <span className="text-[10px] text-muted-foreground/80 italic">
              {methodNote}
            </span>
          )}
        </div>
      )}

      {health && (lostSeconds > 0 || health.failed_chunk_count > 0 || withheldSpans > 0) && (
        <p className="flex items-start gap-1.5 text-[11px] text-amber-800 dark:text-amber-200">
          <AlertTriangle className="w-3.5 h-3.5 shrink-0 mt-px" aria-hidden="true" />
          <span>
            {lostSeconds > 0 && (
              <>
                {formatDuration(lostSeconds)} of this recording produced no usable
                speech and was discarded
              </>
            )}
            {lostSeconds > 0 && (health.failed_chunk_count > 0 || withheldSpans > 0)
              ? '; '
              : ''}
            {health.failed_chunk_count > 0 && (
              <>
                {health.failed_chunk_count} chunk
                {health.failed_chunk_count === 1 ? '' : 's'} failed to transcribe
              </>
            )}
            {health.failed_chunk_count > 0 && withheldSpans > 0 ? '; ' : ''}
            {withheldSpans > 0 && (
              <>
                {withheldSpans} span{withheldSpans === 1 ? '' : 's'} recorded before
                hallucination screening existed are withheld from the summary
                ({health.withheld_word_count.toLocaleString()} words)
              </>
            )}
            . The audio and raw transcript are unchanged.
          </span>
        </p>
      )}
    </div>
  );
};
