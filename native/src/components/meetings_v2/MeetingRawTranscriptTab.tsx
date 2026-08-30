import React from 'react';
import { FileText, Sparkles, Terminal } from 'lucide-react';
import type { LiveTranscriptUpdate, TranscriptSegment } from '../../types';

interface MeetingRawTranscriptTabProps {
  segments: TranscriptSegment[];
  liveUpdates: LiveTranscriptUpdate[];
  isRecording: boolean;
  isPaused: boolean;
  /** Recorded chunks still waiting to be transcribed. Their audio is already saved. */
  backlog: number;
  latestLatency?: number;
}

const CHANNEL_LABEL = (segment: TranscriptSegment): string => {
  if (segment.mic_had_audio && segment.sys_had_audio) return 'mic + system';
  if (segment.mic_had_audio) return 'mic';
  if (segment.sys_had_audio) return 'system';
  return 'no channel data';
};

/**
 * The low-level STT output, exactly as Whisper produced it, one entry per
 * durable 30-second chunk.
 */
export const MeetingRawTranscriptTab: React.FC<MeetingRawTranscriptTabProps> = ({
  segments,
  liveUpdates,
  isRecording,
  isPaused,
  backlog,
  latestLatency,
}) => {
  const finalised = liveUpdates.filter((u) => u.is_final);
  const pending = liveUpdates.find((u) => !u.is_final);

  return (
    <div className="flex-1 overflow-y-auto p-6 space-y-4">
      <div className="flex items-center justify-between pb-2 border-b border-border">
        <h3 className="text-sm font-semibold text-foreground flex items-center gap-2">
          <Terminal className="w-4 h-4 text-muted-foreground" />
          Raw Transcript
          <span className="text-xs font-normal text-muted-foreground">
            ({segments.length} chunk{segments.length === 1 ? '' : 's'})
          </span>
        </h3>
        {isRecording && (
          <div className="flex items-center gap-2">
            {backlog > 0 && (
              <span
                className="text-[10px] text-amber-600 dark:text-amber-400 font-mono px-2 py-0.5 rounded-md bg-amber-500/10 border border-amber-500/20"
                title="Recorded chunks still waiting to be transcribed. Audio is already saved."
              >
                {backlog} chunk{backlog === 1 ? '' : 's'} queued
              </span>
            )}
            <span
              className={`text-[11px] font-medium flex items-center gap-1.5 px-2 py-0.5 rounded-md border ${
                isPaused
                  ? 'text-amber-600 dark:text-amber-400 bg-amber-500/10 border-amber-500/20'
                  : 'text-emerald-600 dark:text-emerald-400 bg-emerald-500/10 border-emerald-500/20 animate-pulse'
              }`}
            >
              <Sparkles className="w-3 h-3" />
              {isPaused ? 'Paused' : 'Live STT Stream'}
              {latestLatency !== undefined && !isPaused ? ` • ${latestLatency}ms` : ''}
            </span>
          </div>
        )}
      </div>

      <p className="text-[11px] text-muted-foreground">
        Unedited speech-to-text output, kept for verification and debugging. Nothing
        Relay derives from this meeting ever changes it.
      </p>

      {/* Live feed: committed utterances plus the one still forming. */}
      {isRecording && (finalised.length > 0 || pending) && (
        <div className="p-4 rounded-lg bg-emerald-500/10 border border-emerald-500/20 flex flex-col gap-2">
          <div className="flex items-center justify-between text-[11px] text-emerald-600 dark:text-emerald-400 font-semibold">
            <span className="flex items-center gap-1.5">
              <span className="w-2 h-2 rounded-full bg-emerald-500 animate-ping" />
              Live Continuous Speech
            </span>
          </div>
          <div className="text-sm text-foreground leading-relaxed font-sans select-text">
            {finalised.map((u) => (
              <span key={u.segment_id} className="mr-1.5">
                {u.text}
              </span>
            ))}
            {pending && (
              <span
                key={pending.segment_id}
                className="mr-1.5 text-emerald-700 dark:text-emerald-300 italic"
                title="Still being transcribed"
              >
                {pending.text}
              </span>
            )}
          </div>
        </div>
      )}

      {segments.length === 0 && liveUpdates.length === 0 ? (
        <div className="py-12 text-center text-muted-foreground text-xs">
          {isRecording
            ? 'Listening for speech on Microphone and System Audio…'
            : 'No speech was recognized in this meeting.'}
        </div>
      ) : (
        <div className="space-y-3">
          {segments.map((seg) => (
            <div
              key={seg.chunk_index}
              className="p-4 rounded-lg bg-card border border-border flex flex-col gap-1.5"
            >
              <div className="flex items-center justify-between text-[11px] text-muted-foreground font-mono">
                <span>
                  seg_{String(seg.chunk_index).padStart(5, '0')} · chunk #
                  {seg.chunk_index + 1} ({Math.floor(seg.start_time_s)}s –{' '}
                  {Math.floor(seg.end_time_s)}s)
                </span>
                <span className="flex items-center gap-2">
                  <span
                    className="text-[9px] text-muted-foreground/80"
                    title="Which capture source was audible in this chunk — the basis for speaker attribution"
                  >
                    {CHANNEL_LABEL(seg)}
                  </span>
                  <span
                    className={`px-1.5 rounded text-[9px] font-bold uppercase ${
                      seg.status === 'SUCCESS'
                        ? 'text-emerald-600 dark:text-emerald-400 bg-emerald-500/10'
                        : 'text-muted-foreground bg-muted'
                    }`}
                  >
                    {seg.status}
                  </span>
                </span>
              </div>
              <p className="text-sm text-foreground leading-relaxed font-mono text-[13px] select-text">
                {seg.text || (
                  <span className="italic text-muted-foreground font-sans">
                    (Silence / No Speech)
                  </span>
                )}
              </p>
            </div>
          ))}
        </div>
      )}
    </div>
  );
};

/** Shown in place of the tab when raw transcript visibility is off in settings. */
export const RawTranscriptHidden: React.FC = () => (
  <div className="flex-1 flex flex-col items-center justify-center gap-2 p-8 text-center">
    <FileText className="w-8 h-8 text-muted-foreground/40" />
    <p className="text-sm font-medium text-foreground">Raw transcript is hidden</p>
    <p className="text-xs text-muted-foreground max-w-sm">
      Turn it back on in Settings › Meetings. Hiding it only affects this view — the
      transcript itself is still on disk and still the source for everything Relay
      derives from this meeting.
    </p>
  </div>
);

