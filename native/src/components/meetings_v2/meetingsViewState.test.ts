import { describe, expect, it } from 'vitest';
import {
  ACTIVE_STATES,
  activeWordCount,
  countLiveWords,
  countWords,
  formatDuration,
  isActiveState,
  resolveSelectedSession,
} from './meetingsViewState';
import { makeSession } from '../../test/factories';
import type { LiveTranscriptUpdate, TranscriptSegment } from '../../types';

const segment = (overrides: Partial<TranscriptSegment> = {}): TranscriptSegment => ({
  chunk_index: 0,
  start_time_s: 0,
  end_time_s: 5,
  text: 'one two three',
  created_at: '2026-08-26T14:03:00Z',
  status: 'SUCCESS',
  ...overrides,
});

const update = (overrides: Partial<LiveTranscriptUpdate> = {}): LiveTranscriptUpdate => ({
  segment_id: 'seg_live_1',
  session_id: 'mtg_1',
  utterance_index: 0,
  start_time_s: 0,
  end_time_s: 3,
  text: 'one two',
  is_final: false,
  latency_ms: 400,
  ...overrides,
});

describe('isActiveState', () => {
  it('is true for every state in which a session still owns the recorder', () => {
    for (const state of ACTIVE_STATES) {
      expect(isActiveState(state)).toBe(true);
    }
  });

  it('is false for finished, interrupted, and errored sessions', () => {
    expect(isActiveState('COMPLETED')).toBe(false);
    expect(isActiveState('IDLE')).toBe(false);
    expect(isActiveState('INTERRUPTED')).toBe(false);
    expect(isActiveState('RECOVERED')).toBe(false);
    expect(isActiveState('ERROR')).toBe(false);
  });

  it('is false for a missing state rather than throwing', () => {
    expect(isActiveState(null)).toBe(false);
    expect(isActiveState(undefined)).toBe(false);
  });

  // PAUSED is the one that matters: a paused meeting is still recording's
  // problem, not history's, and dropping it here would end the session.
  it('keeps a paused session active', () => {
    expect(isActiveState('PAUSED')).toBe(true);
  });
});

describe('countWords', () => {
  it('counts words across successful segments', () => {
    expect(countWords([segment(), segment({ text: 'four five' })])).toBe(5);
  });

  it('ignores segments that produced no usable transcript', () => {
    const segments = [
      segment({ text: 'one two three' }),
      segment({ status: 'EMPTY', text: '' }),
      // A FAILED chunk can still carry partial text; it must not be counted,
      // or the UI reports progress the user does not actually have.
      segment({ status: 'FAILED', text: 'partial garbage text here' }),
    ];
    expect(countWords(segments)).toBe(3);
  });

  it('does not count whitespace as words', () => {
    expect(countWords([segment({ text: '   ' })])).toBe(0);
    expect(countWords([segment({ text: 'one\n\ntwo   three\t' })])).toBe(3);
  });

  it('is zero for no segments', () => {
    expect(countWords([])).toBe(0);
  });
});

describe('countLiveWords', () => {
  it('counts every update, final or not', () => {
    expect(countLiveWords([update(), update({ text: 'three', is_final: true })])).toBe(3);
  });

  it('skips empty updates', () => {
    expect(countLiveWords([update({ text: '' }), update({ text: '  ' })])).toBe(0);
    expect(countLiveWords([])).toBe(0);
  });
});

describe('activeWordCount', () => {
  // The three sources describe the same speech at different stages of
  // settling, so the count is a max, never a sum.
  it('takes the largest source rather than adding them', () => {
    const segments = [segment({ text: 'one two three' })];
    const live = [update({ text: 'one two' })];
    expect(activeWordCount(segments, live, 2)).toBe(3);
  });

  it('lets the live stream lead before anything is durable', () => {
    expect(activeWordCount([], [update({ text: 'one two three four' })], 0)).toBe(4);
  });

  it("lets the backend's own count lead when it is ahead", () => {
    expect(activeWordCount([segment({ text: 'one' })], [], 900)).toBe(900);
  });

  it('treats a missing backend count as zero', () => {
    expect(activeWordCount([segment({ text: 'one two' })], [], null)).toBe(2);
    expect(activeWordCount([segment({ text: 'one two' })], [], undefined)).toBe(2);
  });

  it('is zero when nothing has been transcribed', () => {
    expect(activeWordCount([], [], 0)).toBe(0);
  });
});

describe('resolveSelectedSession', () => {
  const finished = makeSession({ id: 'mtg_1', state: 'COMPLETED' });
  const other = makeSession({ id: 'mtg_2', state: 'COMPLETED' });
  const recording = makeSession({ id: 'mtg_3', state: 'RECORDING', word_count: 120 });

  it('returns the selected session from the list', () => {
    expect(resolveSelectedSession([finished, other], 'mtg_2', null)).toBe(other);
  });

  // The active copy carries live state the list copy does not, so it must win
  // for the same id — otherwise the detail pane renders a stale snapshot.
  it('prefers the active copy over the list copy for the same id', () => {
    const staleListCopy = makeSession({ id: 'mtg_3', state: 'STARTING', word_count: 0 });
    const resolved = resolveSelectedSession([staleListCopy], 'mtg_3', recording);
    expect(resolved).toBe(recording);
    expect(resolved?.word_count).toBe(120);
  });

  it('falls back to the recording rather than showing an empty pane', () => {
    expect(resolveSelectedSession([], 'mtg_missing', recording)).toBe(recording);
  });

  it('is undefined when there is nothing to show at all', () => {
    expect(resolveSelectedSession([], null, null)).toBeUndefined();
    expect(resolveSelectedSession([finished], 'mtg_missing', null)).toBeUndefined();
  });
});

describe('formatDuration', () => {
  it('renders minutes and seconds', () => {
    expect(formatDuration(0)).toBe('0m 0s');
    expect(formatDuration(59)).toBe('0m 59s');
    expect(formatDuration(60)).toBe('1m 0s');
    expect(formatDuration(3661)).toBe('61m 1s');
  });

  it('truncates fractional seconds', () => {
    expect(formatDuration(90.9)).toBe('1m 30s');
  });

  it('clamps negatives instead of rendering "-1m -1s"', () => {
    expect(formatDuration(-5)).toBe('0m 0s');
  });
});
