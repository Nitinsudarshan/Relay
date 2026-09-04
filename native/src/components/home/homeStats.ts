/**
 * Derivations behind the Home page.
 *
 * Every number Home shows is computed here from records the vault already
 * returns — there is no `get_home_stats` command, and there should not be one:
 * a second count written in Rust is a second count to keep true. These are pure
 * functions over the lists the surfaces themselves read, so what Home claims and
 * what Scribbles, Meetings or Files claim cannot drift.
 */
import type {
  KnowledgeTelemetrySnapshot,
  MeetingSession,
  Scribble,
  VaultFile,
  VaultNote,
} from '@/types';

/** The tab a Home card hands the user to. Mirrors `MainTabType` in `App.tsx`. */
export type HomeSurface =
  | 'capture'
  | 'meetings'
  | 'scribble'
  | 'files'
  | 'captures'
  | 'graph'
  | 'talkback';

export const WEEK_MS = 7 * 24 * 60 * 60 * 1000;

/** Everything Home reads, in one shape, so the derivations stay testable. */
export interface HomeSnapshot {
  voiceNotes: VaultNote[];
  scribbles: Scribble[];
  meetings: MeetingSession[];
  /** Imported documents only — web captures are counted separately. */
  files: VaultFile[];
  captures: VaultFile[];
  telemetry: KnowledgeTelemetrySnapshot | null;
}

export interface HomeStat {
  id: string;
  label: string;
  value: number;
  /** How many of `value` were created in the last seven days. */
  thisWeek: number;
  /** What the number is, in the vocabulary of the surface it opens. */
  hint: string;
  surface: HomeSurface;
}

export interface HomeVitals {
  /** Words Relay transcribed: voice notes plus meeting transcripts. */
  spokenWords: number;
  /** Total meeting recording time, in seconds. */
  recordedSeconds: number;
  /** Scribbles whose AI enrichment has not finished. */
  awaitingEnrichment: number;
  /** Captures and documents with no Scribble yet — the knowledge layer's backlog. */
  awaitingPromotion: number;
  /** Scribbles carrying at least one relationship. */
  connectedScribbles: number;
  /** Distinct topics across every scribble. */
  distinctTopics: number;
}

export type HomeActivityKind = 'voice_note' | 'scribble' | 'meeting' | 'file' | 'capture';

export interface HomeActivityItem {
  id: string;
  kind: HomeActivityKind;
  title: string;
  /** ISO timestamp, exactly as the vault recorded it. */
  createdAt: string;
  /** One short factual line — never a summary Relay had to invent. */
  detail: string;
  surface: HomeSurface;
}

export const emptySnapshot = (): HomeSnapshot => ({
  voiceNotes: [],
  scribbles: [],
  meetings: [],
  files: [],
  captures: [],
  telemetry: null,
});

/** Words in a body of text. Whitespace-separated, empty string counts as zero. */
export const countWords = (text: string | null | undefined): number =>
  (text ?? '').trim().split(/\s+/).filter(Boolean).length;

/** Milliseconds since the epoch, or `null` when the vault wrote something unparseable. */
const parseTime = (iso: string | null | undefined): number | null => {
  if (!iso) return null;
  const ms = new Date(iso).getTime();
  return Number.isNaN(ms) ? null : ms;
};

/**
 * How many records were created at or after `sinceMs`.
 *
 * An unparseable timestamp is not counted rather than treated as "now" — a
 * broken date should not inflate a weekly figure.
 */
export const countCreatedSince = (
  items: ReadonlyArray<{ created_at?: string | null }>,
  sinceMs: number,
): number =>
  items.filter((item) => {
    const ms = parseTime(item.created_at);
    return ms !== null && ms >= sinceMs;
  }).length;

/** The clickable counters, in the order Home shows them. */
export const buildHomeStats = (snapshot: HomeSnapshot, nowMs: number): HomeStat[] => {
  const weekAgo = nowMs - WEEK_MS;
  const telemetry = snapshot.telemetry;

  return [
    {
      id: 'voice_notes',
      label: 'Voice Notes',
      value: snapshot.voiceNotes.length,
      thisWeek: countCreatedSince(snapshot.voiceNotes, weekAgo),
      hint: 'Dictated and transcribed',
      surface: 'capture',
    },
    {
      id: 'scribbles',
      label: 'Scribbles',
      value: snapshot.scribbles.length,
      thisWeek: countCreatedSince(snapshot.scribbles, weekAgo),
      hint: 'Atomic thoughts',
      surface: 'scribble',
    },
    {
      id: 'meetings',
      label: 'Meetings',
      value: snapshot.meetings.length,
      thisWeek: countCreatedSince(snapshot.meetings, weekAgo),
      hint: 'Recorded and derived',
      surface: 'meetings',
    },
    {
      id: 'files',
      label: 'Documents',
      value: snapshot.files.length,
      thisWeek: countCreatedSince(snapshot.files, weekAgo),
      hint: 'Imported, originals untouched',
      surface: 'files',
    },
    {
      id: 'captures',
      label: 'Web Captures',
      value: snapshot.captures.length,
      thisWeek: countCreatedSince(snapshot.captures, weekAgo),
      hint: 'Pages and conversations',
      surface: 'captures',
    },
    {
      id: 'entities',
      label: 'Entities',
      value: telemetry?.total_entities ?? 0,
      thisWeek: 0,
      hint: 'People, orgs and places resolved',
      surface: 'graph',
    },
    {
      id: 'relationships',
      label: 'Connections',
      value: telemetry?.total_relationships ?? 0,
      thisWeek: 0,
      hint: 'Links across the graph',
      surface: 'graph',
    },
    {
      id: 'memories',
      label: 'Memories',
      value: telemetry?.active_memories ?? 0,
      thisWeek: 0,
      hint: 'Facts Relay holds as current',
      surface: 'graph',
    },
  ];
};

/** The second-order numbers: what was said, what is still waiting on Relay. */
export const buildHomeVitals = (snapshot: HomeSnapshot): HomeVitals => {
  const voiceWords = snapshot.voiceNotes.reduce((sum, n) => sum + countWords(n.content), 0);
  const meetingWords = snapshot.meetings.reduce((sum, m) => sum + (m.word_count ?? 0), 0);

  const topics = new Set<string>();
  snapshot.scribbles.forEach((s) => s.topics?.forEach((t) => topics.add(t.toLowerCase())));

  const unpromoted = [...snapshot.captures, ...snapshot.files].filter((f) => !f.linked_scribble_id);

  return {
    spokenWords: voiceWords + meetingWords,
    recordedSeconds: snapshot.meetings.reduce((sum, m) => sum + (m.duration_seconds ?? 0), 0),
    awaitingEnrichment: snapshot.scribbles.filter(
      (s) => s.ai_metadata?.enrichment_status === 'pending',
    ).length,
    awaitingPromotion: unpromoted.length,
    connectedScribbles: snapshot.scribbles.filter((s) => (s.relationships?.length ?? 0) > 0).length,
    distinctTopics: topics.size,
  };
};

/** `0m`, `7m`, `1h 3m`. Never a bare second count — nobody reads a meeting in seconds. */
export const formatDurationShort = (seconds: number): string => {
  const safe = Number.isFinite(seconds) && seconds > 0 ? Math.floor(seconds) : 0;
  const hours = Math.floor(safe / 3600);
  const minutes = Math.floor((safe % 3600) / 60);
  if (hours > 0) return `${hours}h ${minutes}m`;
  return `${minutes}m`;
};

/**
 * The newest records across every surface, merged.
 *
 * Anything with an unreadable timestamp sorts last rather than being dropped —
 * it is still a real artifact, and hiding it would make the vault look emptier
 * than it is.
 */
export const buildRecentActivity = (snapshot: HomeSnapshot, limit = 6): HomeActivityItem[] => {
  const items: HomeActivityItem[] = [
    ...snapshot.voiceNotes.map((n) => ({
      id: n.id,
      kind: 'voice_note' as const,
      title: n.title || 'Untitled voice note',
      createdAt: n.created_at,
      detail: `${countWords(n.content).toLocaleString()} words`,
      surface: 'capture' as const,
    })),
    ...snapshot.scribbles.map((s) => ({
      id: s.id,
      kind: 'scribble' as const,
      title: s.title || 'Untitled scribble',
      createdAt: s.created_at,
      detail: s.topics?.length ? s.topics.slice(0, 2).join(' · ') : String(s.source_type),
      surface: 'scribble' as const,
    })),
    ...snapshot.meetings.map((m) => ({
      id: m.id,
      kind: 'meeting' as const,
      title: m.title || 'Untitled meeting',
      createdAt: m.created_at,
      detail: formatDurationShort(m.duration_seconds ?? 0),
      surface: 'meetings' as const,
    })),
    ...snapshot.files.map((f) => ({
      id: f.id,
      kind: 'file' as const,
      title: f.original_filename,
      createdAt: f.created_at,
      detail: f.file_type.toUpperCase(),
      surface: 'files' as const,
    })),
    ...snapshot.captures.map((c) => ({
      id: c.id,
      kind: 'capture' as const,
      title: c.capture?.page_title || c.original_filename,
      createdAt: c.created_at,
      detail: c.capture?.domain || c.file_type.toUpperCase(),
      surface: 'captures' as const,
    })),
  ];

  return items
    .sort((a, b) => (parseTime(b.createdAt) ?? -Infinity) - (parseTime(a.createdAt) ?? -Infinity))
    .slice(0, limit);
};

/** `just now`, `4h ago`, `3d ago`, or a date once it is older than a week. */
export const formatRelativeTime = (iso: string | null | undefined, nowMs: number): string => {
  const ms = parseTime(iso);
  if (ms === null) return 'Unknown date';

  const diff = nowMs - ms;
  if (diff < 60_000) return 'just now';
  if (diff < 3_600_000) return `${Math.floor(diff / 60_000)}m ago`;
  if (diff < 86_400_000) return `${Math.floor(diff / 3_600_000)}h ago`;
  if (diff < WEEK_MS) return `${Math.floor(diff / 86_400_000)}d ago`;

  return new Date(ms).toLocaleDateString([], { month: 'short', day: 'numeric' });
};

/** `9,412` — thousands separated, so a five-figure word count stays readable. */
export const formatCount = (value: number): string =>
  Number.isFinite(value) ? Math.round(value).toLocaleString() : '0';
