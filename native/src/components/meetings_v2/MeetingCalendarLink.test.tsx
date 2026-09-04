import { render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { describe, expect, it, vi } from 'vitest';
import { MeetingCalendarLinkPanel } from './MeetingCalendarLink';
import type {
  CalendarAttendee,
  CalendarEvent,
  EventMatch,
  MeetingCalendarLink,
} from '../../types';

function makeAttendee(overrides: Partial<CalendarAttendee> = {}): CalendarAttendee {
  return {
    name: 'Pranjali',
    email: 'pranjali@example.org',
    response: 'ACCEPTED',
    is_organizer: false,
    is_self: false,
    ...overrides,
  };
}

function makeEvent(overrides: Partial<CalendarEvent> = {}): CalendarEvent {
  return {
    id: 'evt_placement_review',
    title: 'Placement Review',
    starts_at: '2026-09-04T10:00:00Z',
    ends_at: '2026-09-04T10:30:00Z',
    description: 'Decide the launch date.',
    location: null,
    attendees: [makeAttendee()],
    conference_url: null,
    organizer: 'nitin@example.org',
    ...overrides,
  };
}

function makeMatch(overrides: Partial<EventMatch> = {}): EventMatch {
  return { event: makeEvent(), overlap: 0.94, ...overrides };
}

function matchedLink(overrides: Partial<MeetingCalendarLink> = {}): MeetingCalendarLink {
  return {
    outcome: { kind: 'MATCHED', ...makeMatch() },
    linked_at: '2026-09-04T10:31:00Z',
    chosen_by_user: false,
    ...overrides,
  };
}

/**
 * The panel answers "which meeting was this?" — and, when Relay could not
 * answer it, hands the question to the person who already knows. These tests
 * assert on what that person sees and can click, because the reported failure
 * was a calendar integration that existed in the backend and never surfaced.
 */
describe('MeetingCalendarLinkPanel', () => {
  it('points at Settings when no calendar is connected, rather than offering a control that cannot work', () => {
    render(
      <MeetingCalendarLinkPanel
        link={null}
        isConnected={false}
        onMatch={vi.fn()}
        onChoose={vi.fn()}
      />,
    );

    expect(screen.getByText(/Connect Google Calendar in Settings/i)).toBeInTheDocument();
    expect(screen.queryByRole('button')).not.toBeInTheDocument();
  });

  it('offers to search the calendar when connected and nothing is linked yet', async () => {
    const onMatch = vi.fn().mockResolvedValue(undefined);
    render(
      <MeetingCalendarLinkPanel
        link={null}
        isConnected
        onMatch={onMatch}
        onChoose={vi.fn()}
      />,
    );

    await userEvent.click(screen.getByRole('button', { name: /find this in my calendar/i }));
    await waitFor(() => expect(onMatch).toHaveBeenCalledTimes(1));
  });

  it('names the matched event and counts who was invited', () => {
    render(
      <MeetingCalendarLinkPanel
        link={matchedLink({
          outcome: {
            kind: 'MATCHED',
            ...makeMatch({
              event: makeEvent({
                attendees: [
                  makeAttendee({ name: 'Pranjali' }),
                  makeAttendee({ name: 'Ayush', response: 'TENTATIVE' }),
                  makeAttendee({ name: 'Rahul', response: 'DECLINED' }),
                ],
              }),
            }),
          },
        })}
        isConnected
        onMatch={vi.fn()}
        onChoose={vi.fn()}
      />,
    );

    expect(screen.getByText('Placement Review')).toBeInTheDocument();
    // Someone who declined was not in the meeting, so they are not counted.
    expect(screen.getByText(/2 invited/)).toBeInTheDocument();
  });

  it('lets a wrong match be cleared', async () => {
    const onChoose = vi.fn().mockResolvedValue(undefined);
    render(
      <MeetingCalendarLinkPanel
        link={matchedLink()}
        isConnected
        onMatch={vi.fn()}
        onChoose={onChoose}
      />,
    );

    await userEvent.click(screen.getByRole('button', { name: /not this meeting/i }));
    await waitFor(() => expect(onChoose).toHaveBeenCalledWith(null));
  });

  it('shows the candidates when two events fit equally well, instead of only saying no', async () => {
    const onChoose = vi.fn().mockResolvedValue(undefined);
    render(
      <MeetingCalendarLinkPanel
        link={{
          outcome: {
            kind: 'NONE',
            reason: 'AMBIGUOUS',
            candidates: [
              makeMatch({ event: makeEvent({ id: 'evt_a', title: 'Placement Review' }) }),
              makeMatch({
                event: makeEvent({ id: 'evt_b', title: 'Design Sync' }),
                overlap: 0.9,
              }),
            ],
          },
          linked_at: '2026-09-04T10:31:00Z',
          chosen_by_user: false,
        }}
        isConnected
        onMatch={vi.fn()}
        onChoose={onChoose}
      />,
    );

    expect(screen.getByText(/More than one event fits/i)).toBeInTheDocument();
    await userEvent.click(screen.getByRole('button', { name: /Design Sync/ }));
    await waitFor(() => expect(onChoose).toHaveBeenCalledWith('evt_b'));
  });

  it('explains an empty calendar without implying a fault', () => {
    render(
      <MeetingCalendarLinkPanel
        link={{
          outcome: { kind: 'NONE', reason: 'NOTHING_SCHEDULED', candidates: [] },
          linked_at: '2026-09-04T10:31:00Z',
          chosen_by_user: false,
        }}
        isConnected
        onMatch={vi.fn()}
        onChoose={vi.fn()}
      />,
    );

    expect(screen.getByText(/Nothing in your calendar overlapped/i)).toBeInTheDocument();
    expect(screen.getByRole('button', { name: /look again/i })).toBeInTheDocument();
  });

  it('marks an event the user chose, so a human decision is not mistaken for a guess', () => {
    render(
      <MeetingCalendarLinkPanel
        link={matchedLink({ chosen_by_user: true })}
        isConnected
        onMatch={vi.fn()}
        onChoose={vi.fn()}
      />,
    );

    expect(screen.getByTitle('You picked this event')).toBeInTheDocument();
  });
});
