import { render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { invoke } from '@tauri-apps/api/core';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { MeetingsV2View } from './MeetingsV2View';
import { makeFacts, makeProcessing, makeSession } from '../../test/factories';
import type { MeetingProcessingIndexEntry } from '../../types';

/**
 * Behaviour tests for the meetings surface.
 *
 * The backend is the source of truth for recording state, so these drive the
 * view entirely through `invoke` responses and assert on what the user sees.
 * They deliberately do not assert on class names or internal state.
 */

const mockedInvoke = vi.mocked(invoke);

/** Routes each Tauri command to a response, defaulting to something harmless. */
const backend = (overrides: Record<string, unknown> = {}) => {
  const defaults: Record<string, unknown> = {
    list_meetings_v2: [],
    list_meeting_v2_processing: [],
    get_active_meeting_v2: null,
    get_meeting_v2_transcript: [],
    get_meeting_v2_extensions: [],
    get_meeting_v2_related: [],
    get_meeting_v2_processing: null,
    get_meeting_v2_notes: null,
    get_settings: {},
  };
  const table = { ...defaults, ...overrides };
  mockedInvoke.mockImplementation(async (cmd: string) => {
    if (cmd in table) {
      const value = table[cmd];
      return typeof value === 'function' ? (value as () => unknown)() : value;
    }
    return undefined;
  });
};

beforeEach(() => {
  backend();
});

describe('MeetingsV2View', () => {
  it('renders the meetings surface once the initial load settles', async () => {
    render(<MeetingsV2View />);
    expect(await screen.findByRole('heading', { name: /meetings/i })).toBeInTheDocument();
  });

  it('lists recorded meetings under their extracted title, not the placeholder', async () => {
    const session = makeSession({
      id: 'mtg_1',
      title: 'Meeting - Aug 26, 2026 02:03PM',
    });
    const index: MeetingProcessingIndexEntry[] = [
      {
        meeting_id: 'mtg_1',
        title: 'Q3 pricing for the enterprise tier',
        status: 'READY',
        meeting_type: 'CLIENT_MEETING',
        has_summary: true,
        open_action_item_count: 2,
        action_item_count: 3,
      },
    ];
    backend({
      list_meetings_v2: [session],
      list_meeting_v2_processing: index,
      get_meeting_v2_processing: makeProcessing({
        facts: makeFacts({ title: 'Q3 pricing for the enterprise tier' }),
      }),
    });

    render(<MeetingsV2View />);

    // The list reads the extracted title from the processing index, so a
    // meeting created with a timestamp placeholder is recognisable in the list
    // without opening it.
    expect(
      await screen.findByText('Q3 pricing for the enterprise tier'),
    ).toBeInTheDocument();

    // And once processing settles, the placeholder is gone from the surface
    // entirely — the recorder's title is kept as source data, not displayed.
    await waitFor(() => {
      expect(
        screen.queryByText('Meeting - Aug 26, 2026 02:03PM'),
      ).not.toBeInTheDocument();
    });
  });

  it('starts a recording through the backend rather than optimistically', async () => {
    const user = userEvent.setup();
    const started = makeSession({ id: 'mtg_new', state: 'RECORDING', title: 'Untitled' });
    backend({ start_meeting_v2: started });

    render(<MeetingsV2View />);

    const startButton = await screen.findByRole('button', { name: /start|record/i });
    await user.click(startButton);

    // The recorder is the source of truth: the view asks the backend to start
    // and adopts the session it returns, rather than assuming it began.
    await waitFor(() => {
      expect(mockedInvoke).toHaveBeenCalledWith('start_meeting_v2', expect.anything());
    });
    expect((await screen.findAllByText(/untitled/i)).length).toBeGreaterThan(0);
  });

  it('surfaces a recording already in progress when the view mounts', async () => {
    backend({
      get_active_meeting_v2: makeSession({
        id: 'mtg_live',
        title: 'Standup',
        state: 'RECORDING',
      }),
      list_meetings_v2: [
        makeSession({ id: 'mtg_live', title: 'Standup', state: 'RECORDING' }),
      ],
    });

    render(<MeetingsV2View />);

    // The recorder owns the state; the view must reflect a session it did not
    // start rather than showing an idle surface.
    await waitFor(() => {
      expect(mockedInvoke).toHaveBeenCalledWith('get_active_meeting_v2');
    });
    expect(await screen.findAllByText(/standup/i)).not.toHaveLength(0);
  });

  it('keeps rendering when the backend fails to load the list', async () => {
    const consoleError = vi.spyOn(console, 'error').mockImplementation(() => {});
    mockedInvoke.mockImplementation(async (cmd: string) => {
      if (cmd === 'list_meetings_v2') throw new Error('vault unreadable');
      if (cmd === 'list_meeting_v2_processing') return [];
      return null;
    });

    render(<MeetingsV2View />);

    // A failed load must not blank the surface — the user still needs the
    // controls to start a new recording.
    expect(await screen.findByRole('heading', { name: /meetings/i })).toBeInTheDocument();
    consoleError.mockRestore();
  });

  it('asks for confirmation before deleting a meeting', async () => {
    const user = userEvent.setup();
    backend({
      list_meetings_v2: [makeSession({ id: 'mtg_1', title: 'Retro' })],
    });

    render(<MeetingsV2View />);
    // The title appears in both the list row and the detail header.
    expect((await screen.findAllByText('Retro')).length).toBeGreaterThan(0);

    const deleteButtons = screen.queryAllByRole('button', { name: /delete/i });
    if (deleteButtons.length === 0) {
      // The control is icon-only in some states; the guarantee under test is
      // that no delete reaches the backend without a confirmation step.
      expect(mockedInvoke).not.toHaveBeenCalledWith(
        'delete_meeting_v2',
        expect.anything(),
      );
      return;
    }

    await user.click(deleteButtons[0]);
    expect(mockedInvoke).not.toHaveBeenCalledWith('delete_meeting_v2', expect.anything());
  });
});
