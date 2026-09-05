import { render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { invoke } from '@tauri-apps/api/core';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { CalendarViewModal } from './CalendarViewModal';
import { CalendarAccount, CalendarEvent } from '../../types';

const mockedInvoke = vi.mocked(invoke);

describe('CalendarViewModal', () => {
  const sampleAccounts: CalendarAccount[] = [
    {
      id: 'cal_work',
      name: 'Work',
      color: '#3b82f6',
      account_email: 'user@work.com',
      enabled: true,
      last_synced_at: new Date().toISOString(),
    },
    {
      id: 'cal_personal',
      name: 'Personal',
      color: '#10b981',
      account_email: 'user@gmail.com',
      enabled: true,
      last_synced_at: new Date().toISOString(),
    },
    {
      id: 'cal_school',
      name: 'School',
      color: '#8b5cf6',
      account_email: 'student@university.edu',
      enabled: false,
      last_synced_at: new Date().toISOString(),
    },
  ];

  const sampleEvents: CalendarEvent[] = [
    {
      id: 'evt_work_1',
      title: 'Sprint Planning',
      starts_at: new Date(Date.now() + 60 * 60 * 1000).toISOString(),
      ends_at: new Date(Date.now() + 120 * 60 * 1000).toISOString(),
      attendees: [{ name: 'Colleague', response: 'ACCEPTED', is_organizer: false, is_self: false }],
      conference_url: 'https://meet.google.com/xyz-work-meet',
      calendar_id: 'cal_work',
      calendar_name: 'Work',
      calendar_color: '#3b82f6',
    },
    {
      id: 'evt_pers_1',
      title: 'Doctor Appointment',
      starts_at: new Date(Date.now() + 180 * 60 * 1000).toISOString(),
      ends_at: new Date(Date.now() + 210 * 60 * 1000).toISOString(),
      attendees: [],
      location: 'Medical Center',
      calendar_id: 'cal_personal',
      calendar_name: 'Personal',
      calendar_color: '#10b981',
    },
  ];

  beforeEach(() => {
    mockedInvoke.mockImplementation(async (cmd: string) => {
      if (cmd === 'list_calendar_accounts') return sampleAccounts;
      if (cmd === 'get_upcoming_calendar_events') return sampleEvents;
      if (cmd === 'sync_calendar_accounts') return sampleAccounts;
      if (cmd === 'update_calendar_account') return sampleAccounts[0];
      if (cmd === 'disconnect_calendar_account') return [];
      if (cmd === 'open_external_url') return null;
      return undefined;
    });
  });

  it('renders calendar accounts and events with distinctive colors', async () => {
    render(
      <CalendarViewModal
        isOpen={true}
        onClose={vi.fn()}
        onSelectEvent={vi.fn()}
        onStartRecording={vi.fn()}
        onJoinAndRecord={vi.fn()}
      />,
    );

    // Modal title and accounts in sidebar
    expect(await screen.findByRole('heading', { name: 'Calendar View' })).toBeInTheDocument();
    expect((await screen.findAllByText('Work')).length).toBeGreaterThanOrEqual(1);
    expect((await screen.findAllByText('Personal')).length).toBeGreaterThanOrEqual(1);
    expect((await screen.findAllByText('School')).length).toBeGreaterThanOrEqual(1);

    // Events are listed
    expect((await screen.findAllByText('Sprint Planning')).length).toBeGreaterThanOrEqual(1);
    expect(screen.getByText('Doctor Appointment')).toBeInTheDocument();
  });

  it('triggers start recording when clicking Record button on event card', async () => {
    const user = userEvent.setup();
    const handleStartRecording = vi.fn();

    render(
      <CalendarViewModal
        isOpen={true}
        onClose={vi.fn()}
        onSelectEvent={vi.fn()}
        onStartRecording={handleStartRecording}
        onJoinAndRecord={vi.fn()}
      />,
    );

    const recordButtons = await screen.findAllByRole('button', { name: 'Record' });
    expect(recordButtons.length).toBeGreaterThan(0);
    await user.click(recordButtons[0]);

    expect(handleStartRecording).toHaveBeenCalled();
  });

  it('triggers join and record when clicking Join & Record button', async () => {
    const user = userEvent.setup();
    const handleJoinAndRecord = vi.fn();

    render(
      <CalendarViewModal
        isOpen={true}
        onClose={vi.fn()}
        onSelectEvent={vi.fn()}
        onStartRecording={vi.fn()}
        onJoinAndRecord={handleJoinAndRecord}
      />,
    );

    const joinButtons = await screen.findAllByRole('button', { name: /join & record/i });
    expect(joinButtons.length).toBeGreaterThan(0);
    await user.click(joinButtons[0]);

    expect(handleJoinAndRecord).toHaveBeenCalled();
  });

  it('triggers onSelectEvent when clicking Details', async () => {
    const user = userEvent.setup();
    const handleSelectEvent = vi.fn();

    render(
      <CalendarViewModal
        isOpen={true}
        onClose={vi.fn()}
        onSelectEvent={handleSelectEvent}
        onStartRecording={vi.fn()}
        onJoinAndRecord={vi.fn()}
      />,
    );

    const detailButtons = await screen.findAllByRole('button', { name: 'Details' });
    await user.click(detailButtons[0]);

    expect(handleSelectEvent).toHaveBeenCalled();
  });
});
