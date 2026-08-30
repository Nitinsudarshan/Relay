import { describe, expect, it, vi, beforeEach } from 'vitest';
import { render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { invoke } from '@tauri-apps/api/core';
import { TalkbackPage } from './TalkbackPage';

const mockedInvoke = vi.mocked(invoke);

beforeEach(() => {
  mockedInvoke.mockReset();
  mockedInvoke.mockImplementation(async (command: string) => {
    if (command === 'start_talkback') return 'LISTENING';
    if (command === 'stop_talkback') return 'OFF';
    if (command === 'get_talkback_session') return { turns: [] };
    return undefined;
  });
});

describe('TalkbackPage', () => {
  it('starts with the microphone off and says so', () => {
    render(<TalkbackPage />);
    expect(screen.getByText(/your microphone is off/i)).toBeInTheDocument();
    expect(
      screen.getByRole('button', { name: /turn talkback on/i }),
    ).toBeInTheDocument();
  });

  it('opens the microphone only when Talkback is switched on', async () => {
    const user = userEvent.setup();
    render(<TalkbackPage />);

    expect(mockedInvoke).not.toHaveBeenCalledWith(
      'start_talkback',
      expect.anything(),
    );

    await user.click(screen.getByRole('button', { name: /turn talkback on/i }));

    await waitFor(() => {
      expect(mockedInvoke).toHaveBeenCalledWith('start_talkback', { voice: true });
    });
  });

  it('sends a typed turn through the same engine, without a microphone', async () => {
    const user = userEvent.setup();
    render(<TalkbackPage />);

    await user.type(
      screen.getByLabelText(/talkback message/i),
      'what did we decide about pricing',
    );
    await user.click(screen.getByRole('button', { name: /send/i }));

    await waitFor(() => {
      // A text turn starts a *text-only* session — voice: false — so
      // typing never silently opens the microphone.
      expect(mockedInvoke).toHaveBeenCalledWith('start_talkback', { voice: false });
      expect(mockedInvoke).toHaveBeenCalledWith('submit_talkback_turn', {
        text: 'what did we decide about pricing',
        typed: true,
        sttMs: null,
      });
    });
  });

  it('does not submit an empty turn', async () => {
    const user = userEvent.setup();
    render(<TalkbackPage />);

    const send = screen.getByRole('button', { name: /send/i });
    expect(send).toBeDisabled();

    await user.type(screen.getByLabelText(/talkback message/i), '   ');
    expect(send).toBeDisabled();
  });

  it('explains what Talkback is for before the first turn', () => {
    render(<TalkbackPage />);
    expect(screen.getByText(/ask relay what it remembers/i)).toBeInTheDocument();
    expect(
      screen.getByText(/never from guesswork/i),
    ).toBeInTheDocument();
  });

  it('surfaces a backend failure instead of failing silently', async () => {
    const user = userEvent.setup();
    mockedInvoke.mockImplementation(async (command: string) => {
      if (command === 'start_talkback') {
        throw { code: 'CAPTURE_ACTIVE', message: 'Relay is already recording.' };
      }
      return undefined;
    });

    render(<TalkbackPage />);
    await user.click(screen.getByRole('button', { name: /turn talkback on/i }));

    expect(
      await screen.findByText(/relay is already recording/i),
    ).toBeInTheDocument();
  });
});
