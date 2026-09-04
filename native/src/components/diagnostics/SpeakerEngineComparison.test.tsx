import { render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { invoke } from '@tauri-apps/api/core';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import {
  SpeakerEngineComparison,
  __verdict,
} from './SpeakerEngineComparison';
import { makeDiarization, makeSession } from '../../test/factories';
import type { EngineComparison, EngineOutcome } from '../../types';

const mockedInvoke = vi.mocked(invoke);

const outcome = (overrides: Partial<EngineOutcome> = {}): EngineOutcome => ({
  engine: 'VOICEPRINT',
  id: 'voiceprint',
  label: 'Voice separation',
  summary: 'Separates individual voices once the recording ends.',
  diarization: makeDiarization(),
  speaker_sizes: [4, 3, 2],
  error: null,
  ...overrides,
});

const comparison = (overrides: Partial<EngineComparison> = {}): EngineComparison => ({
  meeting_id: 'mtg_1',
  active: 'VOICEPRINT',
  expected_speakers: null,
  outcomes: [
    outcome({
      engine: 'CHANNEL',
      id: 'channel',
      label: 'Channel only',
      speaker_sizes: [6, 3],
      diarization: makeDiarization({
        report: {
          ...makeDiarization().report,
          cluster_count: 2,
          well_separated: true,
          local_cluster: 0,
        },
      }),
    }),
    outcome({
      diarization: makeDiarization({
        report: {
          ...makeDiarization().report,
          cluster_count: 3,
          well_separated: true,
          silhouette: 0.89,
          local_cluster: 0,
        },
      }),
    }),
    outcome({
      engine: 'LIVE',
      id: 'live',
      label: 'Live (as recorded)',
      speaker_sizes: [4, 3, 2],
      diarization: makeDiarization({
        report: {
          ...makeDiarization().report,
          cluster_count: 3,
          well_separated: false,
          silhouette: 0,
          local_cluster: 0,
        },
      }),
    }),
  ],
  ...overrides,
});

beforeEach(() => {
  mockedInvoke.mockReset();
  mockedInvoke.mockImplementation(async (cmd: string) => {
    if (cmd === 'list_meetings_v2') return [makeSession({ id: 'mtg_1', title: 'Placement review' })];
    return undefined;
  });
});

/**
 * This panel exists so choosing a separation method does not require holding a
 * new meeting per method — the loop that let two wrong implementations ship.
 */
describe('SpeakerEngineComparison', () => {
  it('offers the recordings that already exist rather than asking for a new one', async () => {
    render(<SpeakerEngineComparison />);
    expect(await screen.findByText(/Placement review/)).toBeInTheDocument();
    expect(screen.getByText(/without holding a new meeting/)).toBeInTheDocument();
  });

  it('runs every method over the chosen recording and shows each answer', async () => {
    mockedInvoke.mockImplementation(async (cmd: string) => {
      if (cmd === 'list_meetings_v2') return [makeSession({ id: 'mtg_1' })];
      if (cmd === 'compare_meeting_v2_speaker_engines') return comparison();
      return undefined;
    });
    render(<SpeakerEngineComparison />);

    await userEvent.click(await screen.findByRole('button', { name: /compare/i }));

    expect(await screen.findByText('Channel only')).toBeInTheDocument();
    expect(screen.getByText('Voice separation')).toBeInTheDocument();
    expect(screen.getByText('Live (as recorded)')).toBeInTheDocument();
    expect(screen.getByText('2 voices, clearly apart')).toBeInTheDocument();
    expect(screen.getByText('3 voices, clearly apart')).toBeInTheDocument();
  });

  it('marks which method is actually in use', async () => {
    mockedInvoke.mockImplementation(async (cmd: string) => {
      if (cmd === 'list_meetings_v2') return [makeSession({ id: 'mtg_1' })];
      if (cmd === 'compare_meeting_v2_speaker_engines') return comparison();
      return undefined;
    });
    render(<SpeakerEngineComparison />);
    await userEvent.click(await screen.findByRole('button', { name: /compare/i }));
    expect(await screen.findByText('in use')).toBeInTheDocument();
  });

  it('passes the expected speaker count when the user gives one', async () => {
    mockedInvoke.mockImplementation(async (cmd: string) => {
      if (cmd === 'list_meetings_v2') return [makeSession({ id: 'mtg_1' })];
      if (cmd === 'compare_meeting_v2_speaker_engines') return comparison();
      return undefined;
    });
    render(<SpeakerEngineComparison />);

    await userEvent.type(await screen.findByLabelText('How many people spoke'), '3');
    await userEvent.click(screen.getByRole('button', { name: /compare/i }));

    await waitFor(() =>
      expect(mockedInvoke).toHaveBeenCalledWith('compare_meeting_v2_speaker_engines', {
        sessionId: 'mtg_1',
        expectedSpeakers: 3,
      }),
    );
  });

  it('reports a method that could not run instead of dropping it', async () => {
    mockedInvoke.mockImplementation(async (cmd: string) => {
      if (cmd === 'list_meetings_v2') return [makeSession({ id: 'mtg_1' })];
      if (cmd === 'compare_meeting_v2_speaker_engines') {
        return comparison({
          outcomes: [
            outcome({
              engine: 'LIVE',
              id: 'live',
              label: 'Live (as recorded)',
              error: "This meeting's audio has been discarded.",
            }),
          ],
        });
      }
      return undefined;
    });
    render(<SpeakerEngineComparison />);
    await userEvent.click(await screen.findByRole('button', { name: /compare/i }));

    expect(await screen.findByText('Could not run')).toBeInTheDocument();
    expect(screen.getByText(/audio has been discarded/)).toBeInTheDocument();
  });

  it('surfaces a failure to run at all', async () => {
    mockedInvoke.mockImplementation(async (cmd: string) => {
      if (cmd === 'list_meetings_v2') return [makeSession({ id: 'mtg_1' })];
      if (cmd === 'compare_meeting_v2_speaker_engines') {
        throw { message: 'A meeting id is required' };
      }
      return undefined;
    });
    render(<SpeakerEngineComparison />);
    await userEvent.click(await screen.findByRole('button', { name: /compare/i }));
    expect(await screen.findByText('A meeting id is required')).toBeInTheDocument();
  });

  it('distinguishes a confident roster from one worth checking', () => {
    // Three states, not two: "separated but not confidently" is exactly the
    // case where Relay split two similar voices and cannot be sure it was right.
    const report = makeDiarization().report;

    expect(
      __verdict(outcome({ diarization: { report: { ...report, cluster_count: 3, well_separated: true }, assignments: [] } })).tone,
    ).toBe('good');

    expect(
      __verdict(
        outcome({
          diarization: {
            report: { ...report, cluster_count: 3, well_separated: false, silhouette: 0.72 },
            assignments: [],
          },
        }),
      ).tone,
    ).toBe('caution');

    expect(
      __verdict(
        outcome({
          diarization: { report: { ...report, cluster_count: 1 }, assignments: [] },
        }),
      ).text,
    ).toBe('One voice');

    expect(__verdict(outcome({ error: 'nope' })).tone).toBe('bad');
  });

  it('says when a speaker was heard only once rather than burying it', () => {
    const report = makeDiarization().report;
    const result = __verdict(
      outcome({
        diarization: {
          report: {
            ...report,
            cluster_count: 3,
            well_separated: false,
            singleton_speaker_count: 1,
          },
          assignments: [],
        },
      }),
    );
    expect(result.text).toContain('heard only once');
  });
});
