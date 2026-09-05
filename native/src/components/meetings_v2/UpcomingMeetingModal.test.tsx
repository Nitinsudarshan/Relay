import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { describe, expect, it, vi } from 'vitest';
import {
  UpcomingMeetingModal,
  formatRelativeTimeToMeeting,
  getMeetingJoinUrl,
} from './UpcomingMeetingModal';
import { CalendarEvent } from '../../types';

describe('formatRelativeTimeToMeeting', () => {
  it('formats upcoming meetings under an hour', () => {
    const start = new Date(Date.now() + 25 * 60 * 1000).toISOString();
    expect(formatRelativeTimeToMeeting(start)).toMatch(/^in 2[45]min$/);
  });

  it('formats upcoming meetings in hours and minutes (e.g. in 2hr 20min)', () => {
    const start = new Date(Date.now() + (2 * 60 + 20) * 60 * 1000).toISOString();
    expect(formatRelativeTimeToMeeting(start)).toMatch(/^in 2hr 20min$/);
  });

  it('formats upcoming meeting starting right now', () => {
    const start = new Date(Date.now() + 10 * 1000).toISOString();
    expect(formatRelativeTimeToMeeting(start)).toBe('now');
  });

  it('formats meeting in progress', () => {
    const start = new Date(Date.now() - 10 * 60 * 1000).toISOString();
    const end = new Date(Date.now() + 20 * 60 * 1000).toISOString();
    expect(formatRelativeTimeToMeeting(start, end)).toBe('In progress');
  });
});

describe('getMeetingJoinUrl', () => {
  it('extracts URL from conference_url field', () => {
    const evt: CalendarEvent = {
      id: '1',
      title: 'Sync',
      starts_at: new Date().toISOString(),
      ends_at: new Date().toISOString(),
      attendees: [],
      conference_url: 'https://meet.google.com/xyz-abcd-efg',
    };
    expect(getMeetingJoinUrl(evt)).toBe('https://meet.google.com/xyz-abcd-efg');
  });

  it('extracts URL from location field if conference_url is missing', () => {
    const evt: CalendarEvent = {
      id: '2',
      title: 'Sync',
      starts_at: new Date().toISOString(),
      ends_at: new Date().toISOString(),
      attendees: [],
      location: 'https://zoom.us/j/1234567890',
    };
    expect(getMeetingJoinUrl(evt)).toBe('https://zoom.us/j/1234567890');
  });

  it('extracts URL from description field as fallback', () => {
    const evt: CalendarEvent = {
      id: '3',
      title: 'Sync',
      starts_at: new Date().toISOString(),
      ends_at: new Date().toISOString(),
      attendees: [],
      description: 'Join the Teams meeting at https://teams.microsoft.com/l/meetup-join/123 please',
    };
    expect(getMeetingJoinUrl(evt)).toBe('https://teams.microsoft.com/l/meetup-join/123');
  });
});

describe('UpcomingMeetingModal', () => {
  const sampleEvent: CalendarEvent = {
    id: 'evt_1',
    title: 'Design Review & Architecture',
    starts_at: new Date(Date.now() + 140 * 60 * 1000).toISOString(),
    ends_at: new Date(Date.now() + 200 * 60 * 1000).toISOString(),
    conference_url: 'https://meet.google.com/abc-defg-hij',
    organizer: 'Sarah Connor',
    description: 'Review the upcoming sprint designs and technical architecture.',
    attendees: [
      { name: 'Sarah Connor', response: 'ACCEPTED', is_organizer: true, is_self: false },
      { name: 'John Doe', response: 'TENTATIVE', is_organizer: false, is_self: false },
      { name: 'Jane Smith', response: 'NO_RESPONSE', is_organizer: false, is_self: true },
    ],
  };

  it('renders modal with meeting details, time to meeting badge, and participants', () => {
    render(
      <UpcomingMeetingModal
        event={sampleEvent}
        isOpen={true}
        onClose={vi.fn()}
        onStartRecording={vi.fn()}
        onJoinAndRecord={vi.fn()}
      />,
    );

    // Meeting title and time to meeting
    expect(screen.getByRole('heading', { name: 'Design Review & Architecture' })).toBeInTheDocument();
    expect(screen.getByText(/in 2hr (?:19|20)min/)).toBeInTheDocument();

    // Participants count and names
    expect(screen.getByText(/Participants \(3\)/i)).toBeInTheDocument();
    expect(screen.getAllByText('Sarah Connor').length).toBeGreaterThanOrEqual(1);
    expect(screen.getByText('John Doe')).toBeInTheDocument();
    expect(screen.getByText('Jane Smith')).toBeInTheDocument();

    // Organizer & description
    expect(screen.getByText('Organizer')).toBeInTheDocument();
    expect(screen.getByText('Review the upcoming sprint designs and technical architecture.')).toBeInTheDocument();

    // Actions
    expect(screen.getByRole('button', { name: /start recording/i })).toBeInTheDocument();
    expect(screen.getByRole('button', { name: /join and record/i })).toBeInTheDocument();
    expect(screen.getAllByRole('button', { name: /close/i }).length).toBeGreaterThanOrEqual(1);
  });

  it('calls onStartRecording when clicking Start Recording button', async () => {
    const user = userEvent.setup();
    const handleStartRecording = vi.fn();

    render(
      <UpcomingMeetingModal
        event={sampleEvent}
        isOpen={true}
        onClose={vi.fn()}
        onStartRecording={handleStartRecording}
        onJoinAndRecord={vi.fn()}
      />,
    );

    await user.click(screen.getByRole('button', { name: /start recording/i }));
    expect(handleStartRecording).toHaveBeenCalledWith(sampleEvent);
  });

  it('calls onJoinAndRecord when clicking Join and Record button', async () => {
    const user = userEvent.setup();
    const handleJoinAndRecord = vi.fn();

    render(
      <UpcomingMeetingModal
        event={sampleEvent}
        isOpen={true}
        onClose={vi.fn()}
        onStartRecording={vi.fn()}
        onJoinAndRecord={handleJoinAndRecord}
      />,
    );

    await user.click(screen.getByRole('button', { name: /join and record/i }));
    expect(handleJoinAndRecord).toHaveBeenCalledWith(sampleEvent);
  });

  it('calls onClose when clicking Close button or X', async () => {
    const user = userEvent.setup();
    const handleClose = vi.fn();

    render(
      <UpcomingMeetingModal
        event={sampleEvent}
        isOpen={true}
        onClose={handleClose}
        onStartRecording={vi.fn()}
        onJoinAndRecord={vi.fn()}
      />,
    );

    const closeButtons = screen.getAllByRole('button', { name: /close/i });
    await user.click(closeButtons[0]);
    expect(handleClose).toHaveBeenCalled();
  });

  it('renders clean conferencing link button and copies link to clipboard', async () => {
    const user = userEvent.setup();
    const writeTextMock = vi.fn().mockResolvedValue(undefined);
    vi.spyOn(navigator.clipboard, 'writeText').mockImplementation(writeTextMock);

    render(
      <UpcomingMeetingModal
        event={sampleEvent}
        isOpen={true}
        onClose={vi.fn()}
        onStartRecording={vi.fn()}
        onJoinAndRecord={vi.fn()}
      />,
    );

    // Clean "Join Video Call" link and "Copy" button
    expect(screen.getByText('Join Video Call')).toBeInTheDocument();
    const copyButton = screen.getByRole('button', { name: /copy/i });
    expect(copyButton).toBeInTheDocument();

    await user.click(copyButton);
    expect(writeTextMock).toHaveBeenCalledWith('https://meet.google.com/abc-defg-hij');
    expect(await screen.findByText('Copied')).toBeInTheDocument();
  });

  it('sanitizes and wraps HTML descriptions without showing literal HTML tags', () => {
    const htmlEvent: CalendarEvent = {
      ...sampleEvent,
      description: "We have scheduled a class.<br><a href='https://example.com/session?token=12345'>Click here to join</a><br><b>Please NOTE:</b> attendance is required.",
    };

    render(
      <UpcomingMeetingModal
        event={htmlEvent}
        isOpen={true}
        onClose={vi.fn()}
        onStartRecording={vi.fn()}
        onJoinAndRecord={vi.fn()}
      />,
    );

    expect(screen.getByText(/We have scheduled a class/)).toBeInTheDocument();
    expect(screen.getByText('Click here to join')).toBeInTheDocument();
    expect(screen.getByText(/Please NOTE:/)).toBeInTheDocument();
    // Raw HTML tags should NOT be displayed verbatim
    expect(screen.queryByText(/<a href=/)).not.toBeInTheDocument();
    expect(screen.queryByText(/<b>/)).not.toBeInTheDocument();
  });
});
