import { render, screen } from '@testing-library/react';
import { describe, expect, it } from 'vitest';
import { MeetingMetadataHeader } from './MeetingMetadataHeader';
import {
  makeMetadata,
  makeParticipant,
  makeSession,
  makeTranscriptHealth,
} from '../../test/factories';

/**
 * The header is the answer to "what am I looking at". These tests assert on
 * what a reader sees, not on markup: the reported complaint was that the header
 * showed chunk and word counts and said nothing about who was in the room.
 */
describe('MeetingMetadataHeader', () => {
  it('names the participants rather than only counting chunks and words', () => {
    render(
      <MeetingMetadataHeader
        metadata={makeMetadata({
          participants: [
            makeParticipant({
              speaker_id: 'spk_me',
              label: 'Nitin',
              is_named: true,
              is_confirmed: true,
              origin: 'LOCAL_USER',
              is_local_user: true,
              share_of_talk: 0.6,
            }),
            makeParticipant({
              speaker_id: 'spk_1',
              label: 'Pranjali',
              is_named: true,
              is_confirmed: true,
              origin: 'DIARIZATION',
              share_of_talk: 0.4,
            }),
          ],
        })}
        session={makeSession()}
        wordCount={4200}
      />,
    );

    expect(screen.getByText('Nitin')).toBeInTheDocument();
    expect(screen.getByText('Pranjali')).toBeInTheDocument();
    expect(screen.getByText(/2 spoke/)).toBeInTheDocument();
    expect(screen.getByText(/4,200 words/)).toBeInTheDocument();
    expect(screen.getByText(/45m 0s/)).toBeInTheDocument();
  });

  it('marks a name nobody confirmed differently from one somebody did', () => {
    render(
      <MeetingMetadataHeader
        metadata={makeMetadata({
          speaking_participant_count: 2,
          participants: [
            makeParticipant({
              speaker_id: 'spk_1',
              label: 'Pranjali',
              is_named: true,
              is_confirmed: true,
            }),
            makeParticipant({
              speaker_id: 'spk_2',
              label: 'Ayush',
              is_named: true,
              is_confirmed: false,
              origin: 'SELF_INTRODUCED',
            }),
          ],
        })}
        session={makeSession()}
        wordCount={100}
      />,
    );

    expect(screen.getByLabelText('confirmed')).toBeInTheDocument();
    expect(screen.getByLabelText('unconfirmed')).toBeInTheDocument();
  });

  it('says somebody was mentioned rather than presenting them as a speaker', () => {
    render(
      <MeetingMetadataHeader
        metadata={makeMetadata({
          speaking_participant_count: 1,
          participants: [
            makeParticipant({ speaker_id: 'spk_1', label: 'Pranjali', is_named: true }),
            makeParticipant({
              speaker_id: null,
              label: 'Rahul',
              is_named: true,
              origin: 'MENTIONED',
              speaking_seconds: 0,
              turn_count: 0,
              share_of_talk: 0,
            }),
          ],
        })}
        session={makeSession()}
        wordCount={100}
      />,
    );

    expect(screen.getByText('mentioned')).toBeInTheDocument();
    expect(screen.getByText(/1 spoke of 2/)).toBeInTheDocument();
  });

  it('says how much of the recording was discarded, and that the audio is intact', () => {
    // The number that explains a thin summary. Hiding it would leave the user
    // wondering why a 45-minute meeting produced three bullets.
    render(
      <MeetingMetadataHeader
        metadata={makeMetadata({
          health: makeTranscriptHealth({
            rejected_chunk_count: 9,
            rejected_seconds: 270,
            decoded_chunk_count: 81,
            chunk_count: 90,
          }),
        })}
        session={makeSession()}
        wordCount={4200}
      />,
    );

    expect(screen.getByText(/4m 30s/)).toBeInTheDocument();
    expect(screen.getByText(/no usable speech/)).toBeInTheDocument();
    expect(screen.getByText(/audio and raw transcript are unchanged/)).toBeInTheDocument();
  });

  it('reports spans withheld from an old recording', () => {
    render(
      <MeetingMetadataHeader
        metadata={makeMetadata({
          health: makeTranscriptHealth({
            withheld_on_read: { repetition_loop: 9 },
            withheld_word_count: 1314,
          }),
        })}
        session={makeSession()}
        wordCount={4200}
      />,
    );

    expect(screen.getByText(/9 spans recorded before/)).toBeInTheDocument();
    expect(screen.getByText(/1,314 words/)).toBeInTheDocument();
  });

  it('says nothing about transcript health when nothing was lost', () => {
    render(
      <MeetingMetadataHeader
        metadata={makeMetadata()}
        session={makeSession()}
        wordCount={4200}
      />,
    );
    expect(screen.queryByText(/no usable speech/)).not.toBeInTheDocument();
    expect(screen.queryByText(/withheld/)).not.toBeInTheDocument();
  });

  it('discloses a channel-only roster instead of implying voices were separated', () => {
    render(
      <MeetingMetadataHeader
        metadata={makeMetadata({ speaker_method: 'CHANNEL' })}
        session={makeSession()}
        wordCount={4200}
      />,
    );
    expect(screen.getByText(/told apart by capture channel only/)).toBeInTheDocument();
  });

  it('falls back to the session record before the meeting has been prepared', () => {
    render(
      <MeetingMetadataHeader
        metadata={null}
        session={makeSession({ duration_seconds: 600, chunk_count: 20 })}
        wordCount={0}
      />,
    );
    expect(screen.getByText(/no speakers yet/)).toBeInTheDocument();
    expect(screen.getByText(/10m 0s/)).toBeInTheDocument();
    expect(screen.getByText(/20 chunks/)).toBeInTheDocument();
  });

  it('shows live elapsed time while a meeting is still recording', () => {
    render(
      <MeetingMetadataHeader
        metadata={makeMetadata({ duration_seconds: 0 })}
        session={makeSession({ duration_seconds: 0, state: 'RECORDING' })}
        liveElapsedSeconds={95}
        wordCount={12}
      />,
    );
    expect(screen.getByText(/1m 35s/)).toBeInTheDocument();
  });
});
