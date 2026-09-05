import { render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { invoke } from '@tauri-apps/api/core';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { MeetingReminderWindow } from './MeetingReminderWindow';
import type { MeetingReminderPayload } from '../../types';

function reminder(overrides: Partial<MeetingReminderPayload> = {}): MeetingReminderPayload {
  return {
    key: 'cal:evt_placement_review',
    kind: 'upcoming',
    title: 'Placement review',
    provider: 'google_meet',
    provider_name: 'Google Meet',
    time_label: 'Starts in 4 minutes',
    participants: ['Pranjali Sharma', 'Ayush Kumar'],
    can_join: true,
    ...overrides,
  };
}

/**
 * Mounts the card with one reminder already staged, which is how it appears in
 * practice: the backend stages the payload and reveals a window that is
 * already painted.
 */
async function renderWithReminder(payload: MeetingReminderPayload = reminder()) {
  vi.mocked(invoke).mockImplementation(async (command: string) =>
    command === 'get_pending_meeting_reminder' ? payload : undefined
  );
  render(<MeetingReminderWindow />);
  await screen.findByText(payload.title);
}

/**
 * The card is the only surface a reminder has, and each of its buttons is the
 * one chance to act on a meeting that is starting right now. These tests assert
 * on what a person sees and presses, and on the command each press sends —
 * because the failure this feature is known for was a primary action that
 * looked right and did nothing (`docs/decisions.md` Decision 45, Broken #1).
 */
describe('MeetingReminderWindow', () => {
  beforeEach(() => {
    vi.mocked(invoke).mockReset();
    vi.mocked(invoke).mockResolvedValue(undefined);
  });

  it('shows nothing at all until a reminder arrives', () => {
    render(<MeetingReminderWindow />);
    expect(screen.queryByRole('button')).not.toBeInTheDocument();
  });

  it('reports itself ready so the backend can reveal an already-painted window', async () => {
    render(<MeetingReminderWindow />);
    await waitFor(() =>
      expect(vi.mocked(invoke)).toHaveBeenCalledWith('meeting_reminder_ready')
    );
  });

  it('names the meeting, when it starts, and how many were invited', async () => {
    await renderWithReminder();

    expect(screen.getByText('Placement review')).toBeInTheDocument();
    expect(screen.getByText('Starts in 4 minutes')).toBeInTheDocument();
    expect(screen.getByText('Google Meet')).toBeInTheDocument();
    expect(screen.getByText('2 invited')).toBeInTheDocument();
  });

  it('starts recording the meeting the card is about', async () => {
    await renderWithReminder();

    await userEvent.click(screen.getByRole('button', { name: /record/i }));

    await waitFor(() =>
      expect(vi.mocked(invoke)).toHaveBeenCalledWith('start_meeting_from_reminder', {
        key: 'cal:evt_placement_review',
        kind: 'upcoming',
      })
    );
  });

  it('opens the call when the meeting has a link', async () => {
    await renderWithReminder();

    await userEvent.click(screen.getByRole('button', { name: /join/i }));

    await waitFor(() =>
      expect(vi.mocked(invoke)).toHaveBeenCalledWith('join_meeting_from_reminder', {
        key: 'cal:evt_placement_review',
        kind: 'upcoming',
      })
    );
  });

  it('offers no Join button for a meeting with no link, rather than one that cannot work', async () => {
    await renderWithReminder(reminder({ can_join: false }));

    expect(screen.queryByRole('button', { name: /join/i })).not.toBeInTheDocument();
    expect(screen.getByRole('button', { name: /record/i })).toBeInTheDocument();
  });

  it('snoozes for the duration the user picked', async () => {
    await renderWithReminder();

    await userEvent.click(screen.getByRole('button', { name: /snooze/i }));
    await userEvent.click(screen.getByRole('button', { name: '10 min' }));

    await waitFor(() =>
      expect(vi.mocked(invoke)).toHaveBeenCalledWith('snooze_meeting_reminder', {
        key: 'cal:evt_placement_review',
        kind: 'upcoming',
        minutes: 10,
      })
    );
  });

  it('lets the snooze durations be backed out of without answering the reminder', async () => {
    await renderWithReminder();

    await userEvent.click(screen.getByRole('button', { name: /snooze/i }));
    await userEvent.click(screen.getByRole('button', { name: /back/i }));

    expect(screen.getByRole('button', { name: /record/i })).toBeInTheDocument();
    expect(vi.mocked(invoke)).not.toHaveBeenCalledWith(
      'snooze_meeting_reminder',
      expect.anything()
    );
  });

  it('dismisses the reminder from the close button', async () => {
    await renderWithReminder();

    await userEvent.click(screen.getByRole('button', { name: /dismiss this reminder/i }));

    await waitFor(() =>
      expect(vi.mocked(invoke)).toHaveBeenCalledWith('dismiss_meeting_reminder', {
        key: 'cal:evt_placement_review',
        kind: 'upcoming',
      })
    );
  });

  it('tells the backend to pause its countdown while the pointer is on the card', async () => {
    await renderWithReminder();

    await userEvent.hover(screen.getByText('Placement review'));
    await waitFor(() =>
      expect(vi.mocked(invoke)).toHaveBeenCalledWith('meeting_reminder_hover_changed', {
        hovered: true,
      })
    );

    await userEvent.unhover(screen.getByText('Placement review'));
    await waitFor(() =>
      expect(vi.mocked(invoke)).toHaveBeenCalledWith('meeting_reminder_hover_changed', {
        hovered: false,
      })
    );
  });

  it('says why it is on screen, so two reminders are not read as the same one', async () => {
    await renderWithReminder(reminder({ kind: 'unrecorded', time_label: 'Started 6 minutes ago' }));
    expect(screen.getByText('Not recording')).toBeInTheDocument();

    vi.mocked(invoke).mockReset();
    vi.mocked(invoke).mockResolvedValue(undefined);
    await renderWithReminder(reminder({ kind: 'detected', can_join: false }));
    expect(screen.getByText('Detected')).toBeInTheDocument();
  });

  it('takes the card away once an action has been sent', async () => {
    await renderWithReminder();

    await userEvent.click(screen.getByRole('button', { name: /record/i }));

    await waitFor(() => expect(screen.queryByText('Placement review')).not.toBeInTheDocument());
  });
});
