import { describe, expect, it } from 'vitest';
import {
  canSummarize,
  formatTimestamp,
  meetingTitle,
  meetingTypeLabel,
  ownerLabel,
  processingHeadline,
  speakerLabel,
  SUMMARY_MODES,
} from './meetingProcessing';
import {
  makeActionItem,
  makeFacts,
  makeNormalized,
  makeProcessing,
  makeSession,
  makeSpeaker,
} from '../../test/factories';

describe('speakerLabel', () => {
  const speakers = [
    makeSpeaker({ id: 'spk_1', fallback_label: 'Speaker 1' }),
    makeSpeaker({ id: 'spk_2', display_name: 'Priya', fallback_label: 'Speaker 2' }),
  ];

  it('prefers a display name once one has been set', () => {
    expect(speakerLabel(speakers, 'spk_2')).toBe('Priya');
  });

  it('falls back to the generated label while a speaker is unnamed', () => {
    expect(speakerLabel(speakers, 'spk_1')).toBe('Speaker 1');
  });

  // The rule the module's own doc comment states: names are never invented.
  it('never invents a name for an unknown or missing id', () => {
    expect(speakerLabel(speakers, 'spk_99')).toBe('Unknown speaker');
    expect(speakerLabel(speakers, null)).toBe('Unknown speaker');
    expect(speakerLabel(speakers, undefined)).toBe('Unknown speaker');
    expect(speakerLabel([], 'spk_1')).toBe('Unknown speaker');
  });

  it('treats a whitespace-only display name as unnamed', () => {
    const blank = [makeSpeaker({ id: 'spk_3', display_name: '   ', fallback_label: 'Speaker 3' })];
    expect(speakerLabel(blank, 'spk_3')).toBe('Speaker 3');
  });

  it('resolves through the id, so a rename needs no regeneration', () => {
    const before = speakerLabel(speakers, 'spk_1');
    const renamed = speakers.map((s) =>
      s.id === 'spk_1' ? { ...s, display_name: 'Dev' } : s,
    );
    expect(before).toBe('Speaker 1');
    expect(speakerLabel(renamed, 'spk_1')).toBe('Dev');
  });
});

describe('ownerLabel', () => {
  const speakers = [makeSpeaker({ id: 'spk_1', display_name: 'Priya' })];

  it('resolves ME and SPEAKER owners through the speaker list', () => {
    expect(ownerLabel(makeActionItem({ owner_type: 'SPEAKER' }), speakers)).toBe('Priya');
    expect(ownerLabel(makeActionItem({ owner_type: 'ME' }), speakers)).toBe('Priya');
  });

  it('uses the free-text label for owners who were never captured speakers', () => {
    const item = makeActionItem({
      owner_type: 'EXTERNAL',
      owner_speaker_id: null,
      owner_label: 'Finance team',
    });
    expect(ownerLabel(item, speakers)).toBe('Finance team');
  });

  it('names the group rather than guessing an individual', () => {
    expect(ownerLabel(makeActionItem({ owner_type: 'GROUP' }), speakers)).toBe('The group');
  });

  it('reports Unassigned rather than a wrong owner when the data is incomplete', () => {
    expect(
      ownerLabel(makeActionItem({ owner_type: 'SPEAKER', owner_speaker_id: null }), speakers),
    ).toBe('Unassigned');
    expect(
      ownerLabel(
        makeActionItem({ owner_type: 'EXTERNAL', owner_speaker_id: null, owner_label: '  ' }),
        speakers,
      ),
    ).toBe('Unassigned');
    expect(ownerLabel(makeActionItem({ owner_type: 'UNASSIGNED' }), speakers)).toBe('Unassigned');
  });
});

describe('meetingTitle', () => {
  it('prefers the extracted title over the recorder placeholder', () => {
    const session = makeSession({ title: 'Meeting - Aug 26, 2026 02:03PM' });
    const processing = makeProcessing({ facts: makeFacts({ title: 'Q3 pricing' }) });
    expect(meetingTitle(session, processing)).toBe('Q3 pricing');
  });

  it("falls back to the session's own title when nothing was extracted", () => {
    const session = makeSession({ title: 'Standup' });
    expect(meetingTitle(session, null)).toBe('Standup');
    expect(meetingTitle(session, undefined)).toBe('Standup');
    expect(meetingTitle(session, makeProcessing({ facts: null }))).toBe('Standup');
  });

  it('does not let a blank extracted title blank out the display', () => {
    const session = makeSession({ title: 'Standup' });
    const processing = makeProcessing({ facts: makeFacts({ title: '   ' }) });
    expect(meetingTitle(session, processing)).toBe('Standup');
  });
});

describe('formatTimestamp', () => {
  it('pads to mm:ss under an hour', () => {
    expect(formatTimestamp(0)).toBe('0:00');
    expect(formatTimestamp(9)).toBe('0:09');
    expect(formatTimestamp(75)).toBe('1:15');
    expect(formatTimestamp(599)).toBe('9:59');
  });

  it('grows to h:mm:ss past an hour', () => {
    expect(formatTimestamp(3600)).toBe('1:00:00');
    expect(formatTimestamp(3725)).toBe('1:02:05');
    expect(formatTimestamp(36000)).toBe('10:00:00');
  });

  it('rounds fractional seconds rather than truncating them', () => {
    expect(formatTimestamp(59.6)).toBe('1:00');
    expect(formatTimestamp(0.4)).toBe('0:00');
  });

  // A negative offset can reach this from a seek before the clip start; it
  // should read as the beginning, never as "-1:-1".
  it('clamps negatives to zero', () => {
    expect(formatTimestamp(-5)).toBe('0:00');
  });
});

describe('meetingTypeLabel', () => {
  it('maps every known meeting type to its display form', () => {
    expect(meetingTypeLabel('ONE_ON_ONE')).toBe('1:1');
    expect(meetingTypeLabel('PROJECT_REVIEW')).toBe('Project Review');
    expect(meetingTypeLabel('CLIENT_MEETING')).toBe('Client Meeting');
    expect(meetingTypeLabel('SCRUM')).toBe('Scrum');
    expect(meetingTypeLabel('PLANNING')).toBe('Planning');
    expect(meetingTypeLabel('INTERVIEW')).toBe('Interview');
    expect(meetingTypeLabel('GENERAL')).toBe('General');
  });
});

describe('processingHeadline', () => {
  it('reports a tone that matches whether the meeting is usable', () => {
    expect(processingHeadline(makeProcessing({ status: 'RUNNING' }))).toEqual({
      label: 'Processing meeting…',
      tone: 'busy',
    });
    expect(processingHeadline(makeProcessing({ status: 'READY' })).tone).toBe('ok');
    expect(processingHeadline(makeProcessing({ status: 'PARTIAL' })).tone).toBe('warn');
    expect(processingHeadline(makeProcessing({ status: 'FAILED' })).tone).toBe('error');
  });

  it('treats absent processing as not started, not as an error', () => {
    expect(processingHeadline(null)).toEqual({ label: 'Not processed yet', tone: 'idle' });
    expect(processingHeadline(undefined).tone).toBe('idle');
    expect(processingHeadline(makeProcessing({ status: 'NOT_STARTED' })).tone).toBe('idle');
  });
});

describe('canSummarize', () => {
  it('is true only once there is transcribed speech to summarize', () => {
    expect(canSummarize(makeProcessing())).toBe(true);
  });

  it('is false for a meeting that captured no speech', () => {
    const silent = makeProcessing({ normalized: makeNormalized({ segments: [] }) });
    expect(canSummarize(silent)).toBe(false);
  });

  it('is false before normalization has produced anything', () => {
    expect(canSummarize(makeProcessing({ normalized: null }))).toBe(false);
    expect(canSummarize(null)).toBe(false);
    expect(canSummarize(undefined)).toBe(false);
  });
});

describe('SUMMARY_MODES', () => {
  it('offers exactly the three backend summary modes, standard in the middle', () => {
    expect(SUMMARY_MODES.map((m) => m.value)).toEqual(['CONCISE', 'STANDARD', 'DETAILED']);
    expect(SUMMARY_MODES.every((m) => m.label && m.hint)).toBe(true);
  });
});
