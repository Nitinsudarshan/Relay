/**
 * Derived state for the meetings list and detail view.
 *
 * These were inline in `MeetingsV2View`, where they could not be exercised
 * without mounting a 1,000-line component and its whole Tauri surface. They
 * are pure functions of the data the view already holds, so they live here and
 * are tested directly.
 */
import type {
  LiveTranscriptUpdate,
  MeetingSession,
  MeetingState,
  TranscriptSegment,
} from '../../types';

/**
 * States in which a session still owns the recorder.
 *
 * The distinction that matters: a session in one of these is the *active* one —
 * the view must keep reconciling it against the backend and must not treat it
 * as a finished record. Everything else is history.
 */
export const ACTIVE_STATES: readonly MeetingState[] = [
  'STARTING',
  'RECORDING',
  'PAUSED',
  'STOPPING',
  'FINALIZING',
];

/** Whether a session still owns the recorder. */
export const isActiveState = (state?: MeetingState | null): boolean =>
  !!state && ACTIVE_STATES.includes(state);

/**
 * Words in the durable transcript.
 *
 * Only `SUCCESS` segments count: an `EMPTY` or `FAILED` chunk produced no
 * words, and counting its (absent or partial) text would report progress the
 * user does not have.
 */
export const countWords = (segments: TranscriptSegment[]): number =>
  segments.reduce((acc, seg) => {
    if (seg.status === 'SUCCESS' && seg.text) {
      return acc + seg.text.trim().split(/\s+/).filter(Boolean).length;
    }
    return acc;
  }, 0);

/** Words in the live (not yet durable) stream. */
export const countLiveWords = (updates: LiveTranscriptUpdate[]): number =>
  updates.reduce((acc, update) => {
    if (update.text) {
      return acc + update.text.trim().split(/\s+/).filter(Boolean).length;
    }
    return acc;
  }, 0);

/**
 * The word count to show for a recording in progress.
 *
 * The maximum of three sources rather than a sum: durable segments, the live
 * stream, and the backend's own count all describe the *same* speech at
 * different stages of settling, so adding them would double-count. The largest
 * is the one furthest along.
 */
export const activeWordCount = (
  segments: TranscriptSegment[],
  liveUpdates: LiveTranscriptUpdate[],
  backendWordCount?: number | null,
): number =>
  Math.max(countWords(segments), countLiveWords(liveUpdates), backendWordCount ?? 0);

/**
 * Which session the detail pane should render.
 *
 * The active session wins when it is the selected one, because it carries live
 * state the list copy does not. Falling back to the active session when the
 * selection matches nothing keeps a recording visible rather than showing an
 * empty pane.
 */
export const resolveSelectedSession = (
  sessions: MeetingSession[],
  selectedSessionId: string | null,
  activeSession: MeetingSession | null,
): MeetingSession | undefined =>
  (activeSession?.id === selectedSessionId ? activeSession : undefined) ||
  sessions.find((s) => s.id === selectedSessionId) ||
  activeSession ||
  undefined;

/** `<m>m <s>s`, the list's compact duration. */
export const formatDuration = (secs: number): string => {
  const safe = Math.max(0, secs);
  const m = Math.floor(safe / 60);
  const s = Math.floor(safe % 60);
  return `${m}m ${s}s`;
};
