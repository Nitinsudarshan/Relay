import type {
  ActionItem,
  MeetingProcessing,
  MeetingType,
  MeetingSession,
  ProcessingStatus,
  Speaker,
  SummaryMode,
} from '../../types';

/**
 * Resolution helpers for derived meeting data.
 *
 * The rule these encode: derived views reference speaker *ids*, and names are
 * resolved here at render time. That is why renaming a speaker updates the
 * conversation and every action-item owner without regenerating anything, and
 * without touching the raw transcript.
 */

/** What to show for a speaker id. Never invents a human name. */
export const speakerLabel = (
  speakers: Speaker[],
  speakerId?: string | null,
): string => {
  if (!speakerId) return 'Unknown speaker';
  const speaker = speakers.find((s) => s.id === speakerId);
  if (!speaker) return 'Unknown speaker';
  const name = speaker.display_name?.trim();
  return name && name.length > 0 ? name : speaker.fallback_label;
};

/**
 * The display name for an action item's owner.
 * @param item - The action item whose owner is being resolved
 */
export const ownerLabel = (item: ActionItem, speakers: Speaker[]): string => {
  switch (item.owner_type) {
    case 'ME':
    case 'SPEAKER':
      return item.owner_speaker_id
        ? speakerLabel(speakers, item.owner_speaker_id)
        : 'Unassigned';
    case 'EXTERNAL':
      return item.owner_label?.trim() || 'Unassigned';
    case 'GROUP':
      return 'The group';
    default:
      return 'Unassigned';
  }
};

/**
 * The title to show for a meeting: the extracted one when it exists, otherwise
 * the recorder's own. Derived data is preferred for display but never written
 * back over the source record.
 */
export const meetingTitle = (
  session: MeetingSession,
  processing?: MeetingProcessing | null,
): string => processing?.facts?.title?.trim() || session.title;

/** `mm:ss`, or `h:mm:ss` past an hour. */
export const formatTimestamp = (seconds: number): string => {
  const total = Math.max(0, Math.round(seconds));
  const hours = Math.floor(total / 3600);
  const minutes = Math.floor((total % 3600) / 60);
  const secs = total % 60;
  const pad = (n: number) => String(n).padStart(2, '0');
  return hours > 0
    ? `${hours}:${pad(minutes)}:${pad(secs)}`
    : `${minutes}:${pad(secs)}`;
};

/**
 * Display label for a meeting type. Matches what the backend's index command
 * sends, so a type coming from an event and one coming from the index read the
 * same in the list.
 */
export const meetingTypeLabel = (type: MeetingType): string => {
  switch (type) {
    case 'ONE_ON_ONE':
      return '1:1';
    case 'PROJECT_REVIEW':
      return 'Project Review';
    case 'CLIENT_MEETING':
      return 'Client Meeting';
    case 'SCRUM':
      return 'Scrum';
    case 'PLANNING':
      return 'Planning';
    case 'INTERVIEW':
      return 'Interview';
    default:
      return 'General';
  }
};

export const SUMMARY_MODES: { value: SummaryMode; label: string; hint: string }[] = [
  { value: 'CONCISE', label: 'Concise', hint: 'Key points only' },
  { value: 'STANDARD', label: 'Standard', hint: 'Recommended' },
  { value: 'DETAILED', label: 'Detailed', hint: 'More context' },
];

/**
 * A short, honest description of where processing stands.
 *
 * Deliberately not a rendering of all seven internal stages: the user needs to
 * know whether the meeting is usable and what can be retried.
 */
export const processingHeadline = (
  processing?: MeetingProcessing | null,
): { label: string; tone: 'idle' | 'busy' | 'ok' | 'warn' | 'error' } => {
  const status: ProcessingStatus = processing?.status ?? 'NOT_STARTED';
  switch (status) {
    case 'RUNNING':
      return { label: 'Processing meeting…', tone: 'busy' };
    case 'READY':
      return { label: 'Ready', tone: 'ok' };
    case 'PARTIAL':
      return { label: 'Partly processed', tone: 'warn' };
    case 'FAILED':
      return { label: 'Processing failed', tone: 'error' };
    default:
      return { label: 'Not processed yet', tone: 'idle' };
  }
};

/**
 * Whether the pipeline reached a state where a summary can be produced at all.
 * A meeting with no transcribed speech cannot be summarized, and saying so is
 * better than offering a button that always fails.
 */
export const canSummarize = (processing?: MeetingProcessing | null): boolean =>
  (processing?.normalized?.segments.length ?? 0) > 0;
