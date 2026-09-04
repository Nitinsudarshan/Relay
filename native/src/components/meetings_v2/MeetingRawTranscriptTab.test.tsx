import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { describe, expect, it } from 'vitest';
import { MeetingRawTranscriptTab } from './MeetingRawTranscriptTab';
import { makeTranscriptSegment } from '../../test/factories';

const props = (
  overrides: Partial<React.ComponentProps<typeof MeetingRawTranscriptTab>> = {},
) => ({
  segments: [makeTranscriptSegment()],
  liveUpdates: [],
  isRecording: false,
  isPaused: false,
  backlog: 0,
  ...overrides,
});

/**
 * This tab is the diagnostic source for the whole pipeline, so the property
 * that matters most is that a rejected chunk is *visible* as rejected. A chunk
 * that silently vanishes is indistinguishable from one that never existed.
 */
describe('MeetingRawTranscriptTab', () => {
  it('shows decoded text for a chunk that produced speech', () => {
    render(<MeetingRawTranscriptTab {...props()} />);
    expect(
      screen.getByText(/placement numbers came in at forty-one/),
    ).toBeInTheDocument();
    expect(screen.getByText('SUCCESS')).toBeInTheDocument();
  });

  it('reports the voiced time the gate measured', () => {
    // The number that decides whether a chunk is decoded at all, and therefore
    // the first thing worth looking at when a transcript is thin.
    render(<MeetingRawTranscriptTab {...props()} />);
    expect(screen.getByText('22.0s voiced')).toBeInTheDocument();
  });

  it('shows a rejected chunk as rejected, with the reason', () => {
    render(
      <MeetingRawTranscriptTab
        {...props({
          segments: [
            makeTranscriptSegment({
              chunk_index: 11,
              text: '',
              status: 'REJECTED',
              speech: {
                voiced_seconds: 0,
                total_seconds: 30,
                peak_amplitude: 0.03,
                rms: 0.006,
                noise_floor_rms: 0.0055,
              },
              rejection: {
                reason: {
                  kind: 'REPETITION_LOOP',
                  phrase: 'thank you',
                  repeats: 73,
                },
                discarded_text: 'Thank you. Thank you. Thank you.',
                truncated: true,
                discarded_word_count: 146,
              },
            }),
          ],
        })}
      />,
    );

    expect(screen.getByText('REJECTED')).toBeInTheDocument();
    expect(screen.getByText(/decoder looped on "thank you" 73 times/)).toBeInTheDocument();
    expect(screen.getByText(/audio for this chunk is unchanged/)).toBeInTheDocument();
  });

  it('keeps the discarded text available as the evidence for the rejection', async () => {
    render(
      <MeetingRawTranscriptTab
        {...props({
          segments: [
            makeTranscriptSegment({
              text: '',
              status: 'REJECTED',
              rejection: {
                reason: { kind: 'NO_SPEECH', probability: 0.94 },
                discarded_text: 'Thanks for watching!',
                truncated: false,
                discarded_word_count: 3,
              },
            }),
          ],
        })}
      />,
    );

    expect(screen.queryByText('Thanks for watching!')).not.toBeInTheDocument();
    await userEvent.click(screen.getByRole('button', { name: /show what was discarded/i }));
    expect(screen.getByText('Thanks for watching!')).toBeInTheDocument();
  });

  it('describes each kind of rejection in words a person can act on', () => {
    render(
      <MeetingRawTranscriptTab
        {...props({
          segments: [
            makeTranscriptSegment({
              chunk_index: 0,
              text: '',
              status: 'REJECTED',
              rejection: {
                reason: {
                  kind: 'FILLER_OVER_SILENCE',
                  phrase: 'thank you',
                  voiced_seconds: 0.1,
                  voiced_ratio: 0.003,
                },
                discarded_text: 'Thank you.',
                truncated: false,
                discarded_word_count: 2,
              },
            }),
            makeTranscriptSegment({
              chunk_index: 1,
              text: '',
              status: 'REJECTED',
              rejection: {
                reason: {
                  kind: 'IMPLAUSIBLE_RATE',
                  words: 120,
                  voiced_seconds: 2,
                  words_per_second: 60,
                },
                discarded_text: 'word word word',
                truncated: true,
                discarded_word_count: 120,
              },
            }),
          ],
        })}
      />,
    );

    expect(screen.getByText(/subtitle filler, not speech/)).toBeInTheDocument();
    expect(screen.getByText(/120 words over 2.0s of voice/)).toBeInTheDocument();
  });

  it('distinguishes a silent chunk from a discarded one', () => {
    render(
      <MeetingRawTranscriptTab
        {...props({
          segments: [
            makeTranscriptSegment({ chunk_index: 0, text: '', status: 'EMPTY' }),
          ],
        })}
      />,
    );
    expect(screen.getByText('EMPTY')).toBeInTheDocument();
    expect(screen.getByText(/Silence \/ No Speech/)).toBeInTheDocument();
    expect(screen.queryByText(/Discarded/)).not.toBeInTheDocument();
  });

  it('says nothing was recognised rather than rendering an empty list', () => {
    render(<MeetingRawTranscriptTab {...props({ segments: [] })} />);
    expect(screen.getByText(/no speech was recognized/i)).toBeInTheDocument();
  });
});
