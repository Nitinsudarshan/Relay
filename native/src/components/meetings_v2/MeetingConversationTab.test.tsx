import { render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { describe, expect, it, vi } from 'vitest';
import { MeetingConversationTab } from './MeetingConversationTab';
import { makeDiarization, makeSpeaker } from '../../test/factories';
import type { Conversation } from '../../types';

const conversation = (overrides: Partial<Conversation> = {}): Conversation => ({
  turns: [
    {
      id: 'turn_00000',
      speaker_id: 'spk_me',
      start_time_s: 0,
      end_time_s: 20,
      text: 'Shall we start with the placement numbers?',
      segment_ids: ['seg_00000_000'],
    },
    {
      id: 'turn_00001',
      speaker_id: 'spk_1',
      start_time_s: 20,
      end_time_s: 45,
      text: 'We closed forty-one this month.',
      segment_ids: ['seg_00000_001'],
    },
  ],
  unattributed_turn_count: 0,
  ...overrides,
});

const props = (
  overrides: Partial<React.ComponentProps<typeof MeetingConversationTab>> = {},
) => ({
  conversation: conversation(),
  speakers: [
    makeSpeaker({ id: 'spk_me', fallback_label: 'Me', is_local_user: true }),
    makeSpeaker({ id: 'spk_1', fallback_label: 'Speaker 1', origin: 'DIARIZATION' as const }),
  ],
  diarization: makeDiarization(),
  onRenameSpeaker: vi.fn(),
  onIdentifySpeakers: vi.fn(),
  isRenaming: false,
  isIdentifying: false,
  isDisabled: false,
  ...overrides,
});

/**
 * The conversation tab is where the "one Speaker 1 for a room of twenty"
 * failure was visible, so these tests are about what the roster claims and how
 * honestly it claims it.
 */
describe('MeetingConversationTab', () => {
  it('renders the conversation as speaker-labelled turns', () => {
    render(<MeetingConversationTab {...props()} />);
    expect(screen.getByText('We closed forty-one this month.')).toBeInTheDocument();
    expect(screen.getAllByText('Speaker 1').length).toBeGreaterThan(0);
  });

  it('says the roster is channel-only, and offers to separate the voices', () => {
    // This is the state the app shipped in: everyone remote shares one label.
    render(<MeetingConversationTab {...props({ diarization: null })} />);

    expect(screen.getByText(/told apart by capture channel only/)).toBeInTheDocument();
    expect(
      screen.getByRole('button', { name: /identify speakers/i }),
    ).toBeInTheDocument();
  });

  it('runs separation with the expected speaker count the user typed', async () => {
    const onIdentifySpeakers = vi.fn();
    render(<MeetingConversationTab {...props({ onIdentifySpeakers })} />);

    await userEvent.type(
      screen.getByLabelText('Expected number of speakers'),
      '4',
    );
    await userEvent.click(screen.getByRole('button', { name: /re-identify/i }));

    await waitFor(() => expect(onIdentifySpeakers).toHaveBeenCalledWith(4));
  });

  it('passes no hint when the count is left on Auto', async () => {
    const onIdentifySpeakers = vi.fn();
    render(<MeetingConversationTab {...props({ onIdentifySpeakers })} />);
    await userEvent.click(screen.getByRole('button', { name: /re-identify/i }));
    await waitFor(() => expect(onIdentifySpeakers).toHaveBeenCalledWith(null));
  });

  it('warns when the voices were not cleanly separated', () => {
    // A roster the clusterer is unsure about must not be presented as fact.
    render(
      <MeetingConversationTab
        {...props({
          diarization: makeDiarization({
            report: {
              cluster_count: 3,
              placed_count: 12,
              unplaced_count: 0,
              skipped_count: 0,
              well_separated: false,
              mean_within_distance: 2.1,
              min_between_distance: 2.4,
              expected_speakers: null,
              duration_ms: 90,
            },
          }),
        })}
      />,
    );

    expect(screen.getByText(/not cleanly separated/)).toBeInTheDocument();
    expect(screen.getByText(/provisional/)).toBeInTheDocument();
  });

  it('says nothing about separation quality when it was clean', () => {
    render(<MeetingConversationTab {...props()} />);
    expect(screen.queryByText(/not cleanly separated/)).not.toBeInTheDocument();
    expect(
      screen.queryByText(/told apart by capture channel only/),
    ).not.toBeInTheDocument();
  });

  it('reports unattributed stretches rather than guessing a speaker', () => {
    render(
      <MeetingConversationTab
        {...props({
          conversation: conversation({
            turns: [
              {
                id: 'turn_00000',
                speaker_id: null,
                start_time_s: 0,
                end_time_s: 20,
                text: 'Two people at once here.',
                segment_ids: ['seg_00000_000'],
              },
            ],
            unattributed_turn_count: 1,
          }),
        })}
      />,
    );

    expect(screen.getByText(/could not be attributed to anyone/)).toBeInTheDocument();
    expect(screen.getByText('Unknown speaker')).toBeInTheDocument();
  });

  it('renames a speaker without touching the turn text', async () => {
    const onRenameSpeaker = vi.fn();
    render(<MeetingConversationTab {...props({ onRenameSpeaker })} />);

    await userEvent.click(screen.getByTitle(/Rename Speaker 1/));
    await userEvent.type(screen.getByPlaceholderText('Speaker 1'), 'Pranjali');
    await userEvent.click(screen.getByRole('button', { name: /save name for speaker 1/i }));

    await waitFor(() => expect(onRenameSpeaker).toHaveBeenCalledWith('spk_1', 'Pranjali'));
    expect(screen.getByText('We closed forty-one this month.')).toBeInTheDocument();
  });

  it('marks how each speaker was established', () => {
    render(
      <MeetingConversationTab
        {...props({
          speakers: [
            makeSpeaker({
              id: 'spk_1',
              fallback_label: 'Speaker 1',
              display_name: 'Pranjali',
              origin: 'MANUAL' as const,
            }),
            makeSpeaker({
              id: 'spk_2',
              fallback_label: 'Speaker 2',
              origin: 'DIARIZATION' as const,
            }),
          ],
        })}
      />,
    );

    expect(screen.getByTitle(/You named this speaker/)).toBeInTheDocument();
    expect(
      screen.getByTitle(/distinct voice was isolated but nobody has named it/),
    ).toBeInTheDocument();
  });

  it('explains itself rather than rendering blank when switched off', () => {
    render(<MeetingConversationTab {...props({ isDisabled: true })} />);
    expect(screen.getByText(/conversation transcript is off/i)).toBeInTheDocument();
  });
});
