import { describe, expect, test } from 'vitest';

import {
  WEEK_MS,
  buildHomeStats,
  buildHomeVitals,
  buildRecentActivity,
  countCreatedSince,
  countWords,
  emptySnapshot,
  formatCount,
  formatDurationShort,
  formatRelativeTime,
  type HomeSnapshot,
} from './homeStats';

import type {
  KnowledgeTelemetrySnapshot,
  MeetingSession,
  Scribble,
  VaultFile,
  VaultNote,
} from '@/types';

const NOW = new Date('2026-09-04T12:00:00Z').getTime();
const hoursAgo = (h: number) => new Date(NOW - h * 3_600_000).toISOString();
const daysAgo = (d: number) => new Date(NOW - d * 86_400_000).toISOString();

const makeVoiceNote = (overrides: Partial<VaultNote> = {}): VaultNote => ({
  id: 'note_1',
  title: 'Standup thoughts',
  note_type: 'voice_note',
  created_at: hoursAgo(2),
  updated_at: hoursAgo(2),
  tags: [],
  content: 'three words here',
  ...overrides,
});

const makeScribble = (overrides: Partial<Scribble> = {}): Scribble => ({
  id: 'scr_1',
  title: 'Retrieval is the bottleneck',
  content: 'A thought about retrieval.',
  source_type: 'text',
  source_metadata: {},
  created_at: hoursAgo(3),
  updated_at: hoursAgo(3),
  tags: [],
  topics: ['Retrieval'],
  entities: [],
  relationships: [],
  attachments: [],
  status: 'active',
  ai_metadata: {
    enrichment_status: 'enriched',
    suggested_concepts: [],
    suggested_questions: [],
    suggested_relations: [],
  },
  ...overrides,
});

const makeMeeting = (overrides: Partial<MeetingSession> = {}): MeetingSession => ({
  id: 'mtg_1',
  title: 'Weekly sync',
  state: 'COMPLETED',
  created_at: daysAgo(1),
  updated_at: daysAgo(1),
  duration_seconds: 3660,
  chunk_count: 10,
  mic_active: false,
  sys_audio_active: false,
  mic_heard: true,
  sys_audio_heard: true,
  paused_seconds: 0,
  total_audio_bytes: 1024,
  transcript_segment_count: 40,
  word_count: 900,
  pending_transcription_chunks: 0,
  ...overrides,
});

const makeVaultFile = (overrides: Partial<VaultFile> = {}): VaultFile => ({
  id: 'file_1',
  original_filename: 'Proposal.pdf',
  file_type: 'pdf',
  mime_type: 'application/pdf',
  size_bytes: 2048,
  content_hash: 'hash',
  created_at: daysAgo(2),
  updated_at: daysAgo(2),
  last_known_source_path: 'C:\\Docs\\Proposal.pdf',
  vault_path: 'files/file_1/original/Proposal.pdf',
  extraction_status: 'extracted',
  processing_status: 'ready',
  content: 'Proposal text.',
  tags: [],
  topics: [],
  entities: [],
  relationships: [],
  ai_metadata: {
    enrichment_status: 'enriched',
    suggested_concepts: [],
    suggested_questions: [],
    suggested_relations: [],
  },
  ...overrides,
});

const makeTelemetry = (
  overrides: Partial<KnowledgeTelemetrySnapshot> = {},
): KnowledgeTelemetrySnapshot => ({
  total_memories: 4,
  active_memories: 3,
  total_entities: 12,
  total_relationships: 7,
  total_scribbles: 2,
  total_notes: 5,
  total_files: 1,
  total_captures: 1,
  ...overrides,
});

const populated = (): HomeSnapshot => ({
  voiceNotes: [
    makeVoiceNote({ id: 'note_1', created_at: hoursAgo(2) }),
    makeVoiceNote({ id: 'note_2', created_at: daysAgo(20), content: 'one two' }),
  ],
  scribbles: [makeScribble()],
  meetings: [makeMeeting()],
  files: [makeVaultFile()],
  captures: [
    makeVaultFile({
      id: 'cap_1',
      original_filename: 'capture.json',
      file_type: 'capture',
      created_at: hoursAgo(1),
      capture: {
        source_type: 'browser_page',
        capture_type: 'article',
        application: 'Chrome',
        domain: 'example.com',
        url: 'https://example.com/a',
        page_title: 'A captured article',
        captured_at: hoursAgo(1),
        extractor_id: 'article',
        extractor_version: 1,
        trust: 'external_untrusted',
        fidelity: 'dom_text',
        coverage: 'rendered_dom',
        notes: [],
        block_count: 10,
        skipped_block_count: 0,
        truncated: false,
        version: 1,
        recapture_count: 0,
      },
    }),
  ],
  telemetry: makeTelemetry(),
});

describe('countWords', () => {
  test('counts whitespace-separated words and tolerates absent content', () => {
    expect(countWords('three words here')).toBe(3);
    expect(countWords('  padded   out  ')).toBe(2);
    expect(countWords('')).toBe(0);
    expect(countWords(null)).toBe(0);
    expect(countWords(undefined)).toBe(0);
  });
});

describe('countCreatedSince', () => {
  test('counts only records at or after the cutoff', () => {
    const items = [{ created_at: hoursAgo(1) }, { created_at: daysAgo(30) }];
    expect(countCreatedSince(items, NOW - WEEK_MS)).toBe(1);
  });

  test('does not count an unparseable or missing timestamp', () => {
    const items = [{ created_at: 'not a date' }, { created_at: null }, {}];
    expect(countCreatedSince(items, NOW - WEEK_MS)).toBe(0);
  });
});

describe('buildHomeStats', () => {
  test('reports one counter per surface with a seven-day delta', () => {
    const stats = buildHomeStats(populated(), NOW);
    const byId = Object.fromEntries(stats.map((s) => [s.id, s]));

    expect(byId.voice_notes.value).toBe(2);
    expect(byId.voice_notes.thisWeek).toBe(1);
    expect(byId.scribbles.value).toBe(1);
    expect(byId.meetings.value).toBe(1);
    expect(byId.files.value).toBe(1);
    expect(byId.captures.value).toBe(1);
    expect(byId.entities.value).toBe(12);
    expect(byId.relationships.value).toBe(7);
    expect(byId.memories.value).toBe(3);
  });

  test('every counter names the surface it opens', () => {
    const stats = buildHomeStats(populated(), NOW);
    expect(stats.map((s) => s.surface)).toEqual([
      'capture',
      'scribble',
      'meetings',
      'files',
      'captures',
      'graph',
      'graph',
      'graph',
    ]);
  });

  test('an empty vault reads as zeros, not as missing data', () => {
    const stats = buildHomeStats(emptySnapshot(), NOW);
    expect(stats).toHaveLength(8);
    expect(stats.every((s) => s.value === 0 && s.thisWeek === 0)).toBe(true);
  });

  test('absent telemetry falls back to zero rather than throwing', () => {
    const stats = buildHomeStats({ ...populated(), telemetry: null }, NOW);
    const byId = Object.fromEntries(stats.map((s) => [s.id, s]));
    expect(byId.entities.value).toBe(0);
    expect(byId.relationships.value).toBe(0);
    expect(byId.memories.value).toBe(0);
  });
});

describe('buildHomeVitals', () => {
  test('sums transcribed words across voice notes and meetings', () => {
    // 3 words + 2 words + a 900-word meeting transcript.
    expect(buildHomeVitals(populated()).spokenWords).toBe(905);
  });

  test('sums recorded meeting time', () => {
    expect(buildHomeVitals(populated()).recordedSeconds).toBe(3660);
  });

  test('counts scribbles still awaiting enrichment', () => {
    const snapshot = populated();
    snapshot.scribbles = [
      makeScribble({ id: 'a' }),
      makeScribble({
        id: 'b',
        ai_metadata: {
          enrichment_status: 'pending',
          suggested_concepts: [],
          suggested_questions: [],
          suggested_relations: [],
        },
      }),
    ];
    expect(buildHomeVitals(snapshot).awaitingEnrichment).toBe(1);
  });

  test('counts captures and documents that have no Scribble yet', () => {
    const snapshot = populated();
    expect(buildHomeVitals(snapshot).awaitingPromotion).toBe(2);

    snapshot.files = [makeVaultFile({ linked_scribble_id: 'scr_1' })];
    expect(buildHomeVitals(snapshot).awaitingPromotion).toBe(1);
  });

  test('counts connected scribbles and distinct topics case-insensitively', () => {
    const snapshot = populated();
    snapshot.scribbles = [
      makeScribble({
        id: 'a',
        topics: ['Retrieval', 'Vault'],
        relationships: [
          {
            id: 'rel_1',
            target_id: 'b',
            relationship_type: 'RELATED_TO',
            confidence: 0.9,
            source: 'user',
          },
        ],
      }),
      makeScribble({ id: 'b', topics: ['retrieval'] }),
    ];
    const vitals = buildHomeVitals(snapshot);
    expect(vitals.connectedScribbles).toBe(1);
    expect(vitals.distinctTopics).toBe(2);
  });

  test('an empty vault produces zeros', () => {
    expect(buildHomeVitals(emptySnapshot())).toEqual({
      spokenWords: 0,
      recordedSeconds: 0,
      awaitingEnrichment: 0,
      awaitingPromotion: 0,
      connectedScribbles: 0,
      distinctTopics: 0,
    });
  });
});

describe('buildRecentActivity', () => {
  test('merges every surface newest-first and respects the limit', () => {
    const activity = buildRecentActivity(populated(), 3);
    expect(activity.map((a) => a.kind)).toEqual(['capture', 'voice_note', 'scribble']);
    expect(activity[0].title).toBe('A captured article');
    expect(activity[0].detail).toBe('example.com');
  });

  test('a meeting carries its duration, a document its type', () => {
    const activity = buildRecentActivity(populated(), 10);
    expect(activity.find((a) => a.kind === 'meeting')?.detail).toBe('1h 1m');
    expect(activity.find((a) => a.kind === 'file')?.detail).toBe('PDF');
  });

  test('an unreadable timestamp sorts last but is not dropped', () => {
    const snapshot = emptySnapshot();
    snapshot.voiceNotes = [
      makeVoiceNote({ id: 'broken', created_at: 'nonsense' }),
      makeVoiceNote({ id: 'good', created_at: hoursAgo(1) }),
    ];
    const activity = buildRecentActivity(snapshot, 10);
    expect(activity.map((a) => a.id)).toEqual(['good', 'broken']);
  });

  test('untitled records get a readable placeholder', () => {
    const snapshot = emptySnapshot();
    snapshot.voiceNotes = [makeVoiceNote({ title: '' })];
    snapshot.scribbles = [makeScribble({ title: '' })];
    const activity = buildRecentActivity(snapshot, 10);
    expect(activity.map((a) => a.title)).toContain('Untitled voice note');
    expect(activity.map((a) => a.title)).toContain('Untitled scribble');
  });

  test('an empty vault yields no rows', () => {
    expect(buildRecentActivity(emptySnapshot())).toEqual([]);
  });
});

describe('formatDurationShort', () => {
  test('renders minutes, hours and nonsense inputs', () => {
    expect(formatDurationShort(0)).toBe('0m');
    expect(formatDurationShort(59)).toBe('0m');
    expect(formatDurationShort(90)).toBe('1m');
    expect(formatDurationShort(3660)).toBe('1h 1m');
    expect(formatDurationShort(-10)).toBe('0m');
    expect(formatDurationShort(Number.NaN)).toBe('0m');
  });
});

describe('formatRelativeTime', () => {
  test('scales from minutes to a calendar date', () => {
    expect(formatRelativeTime(new Date(NOW - 30_000).toISOString(), NOW)).toBe('just now');
    expect(formatRelativeTime(new Date(NOW - 5 * 60_000).toISOString(), NOW)).toBe('5m ago');
    expect(formatRelativeTime(hoursAgo(4), NOW)).toBe('4h ago');
    expect(formatRelativeTime(daysAgo(3), NOW)).toBe('3d ago');
    expect(formatRelativeTime(daysAgo(40), NOW)).not.toMatch(/ago/);
  });

  test('says so rather than inventing a date it cannot read', () => {
    expect(formatRelativeTime('nonsense', NOW)).toBe('Unknown date');
    expect(formatRelativeTime(null, NOW)).toBe('Unknown date');
  });
});

describe('formatCount', () => {
  test('separates thousands and survives a non-finite value', () => {
    expect(formatCount(9412)).toBe((9412).toLocaleString());
    expect(formatCount(0)).toBe('0');
    expect(formatCount(Number.NaN)).toBe('0');
  });
});
