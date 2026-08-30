import { describe, expect, it, vi, beforeEach } from 'vitest';
import { render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { invoke } from '@tauri-apps/api/core';
import { VoiceSettings } from './VoiceSettings';
import type { TtsStatus } from '../../types';

const mockedInvoke = vi.mocked(invoke);

const NOT_CONFIGURED: TtsStatus = {
  engine: 'none',
  ready: false,
  binaryPath: null,
  binaryOrigin: null,
  voicePath: null,
  voiceLabel: null,
  voiceLanguage: null,
  availableVoices: [],
  problems: [
    "No Piper executable found. Add one to Relay's voice folder or browse for it.",
    'No voice model selected. Add a Piper .onnx voice to Relay voice folder.',
  ],
  installDir: 'C:\\Users\\nitin\\.relay\\config\\tts\\piper',
  voicesDir: 'C:\\Users\\nitin\\.relay\\config\\tts\\voices',
  executableName: 'piper.exe',
};

const READY: TtsStatus = {
  engine: 'piper',
  ready: true,
  binaryPath: 'C:\\Users\\nitin\\.relay\\config\\tts\\piper\\piper.exe',
  binaryOrigin: 'managed',
  voicePath: 'C:\\voices\\en_US-amy-medium.onnx',
  voiceLabel: 'en_US-amy-medium',
  voiceLanguage: 'en_US',
  availableVoices: [
    {
      path: 'C:\\voices\\en_US-amy-medium.onnx',
      label: 'en_US-amy-medium',
      language: 'en_US',
      has_config: true,
    },
    {
      path: 'C:\\voices\\hi_IN-pratham-medium.onnx',
      label: 'hi_IN-pratham-medium',
      language: 'hi_IN',
      has_config: true,
    },
  ],
  problems: [],
  installDir: 'C:\\Users\\nitin\\.relay\\config\\tts\\piper',
  voicesDir: 'C:\\Users\\nitin\\.relay\\config\\tts\\voices',
  executableName: 'piper.exe',
};

const withStatus = (status: TtsStatus, overrides: Record<string, unknown> = {}) => {
  mockedInvoke.mockImplementation(async (command: string) => {
    if (command in overrides) {
      const value = overrides[command];
      if (value instanceof Error) throw value;
      return value;
    }
    if (command === 'get_tts_status') return status;
    if (command === 'set_tts_configuration') return status;
    if (command === 'prepare_tts_folders') return status.voicesDir;
    return undefined;
  });
};

beforeEach(() => {
  mockedInvoke.mockReset();
  // jsdom cannot decode or play audio.
  vi.spyOn(window.HTMLMediaElement.prototype, 'play').mockResolvedValue();
  vi.spyOn(window.HTMLMediaElement.prototype, 'pause').mockImplementation(() => {});
});

describe('VoiceSettings', () => {
  it('says plainly when local voice is not set up', async () => {
    withStatus(NOT_CONFIGURED);
    render(<VoiceSettings />);

    expect(await screen.findByText('Not configured')).toBeInTheDocument();
    expect(
      screen.getByText(/talkback will answer in text only/i),
    ).toBeInTheDocument();
  });

  it('surfaces every problem in the backend’s own words', async () => {
    withStatus(NOT_CONFIGURED);
    render(<VoiceSettings />);

    for (const problem of NOT_CONFIGURED.problems) {
      expect(await screen.findByText(problem)).toBeInTheDocument();
    }
  });

  it('tells the user exactly where to put Piper and its voices', async () => {
    withStatus(NOT_CONFIGURED);
    render(<VoiceSettings />);

    // The whole point of the setup flow: no unexplained filesystem paths.
    expect(await screen.findByText(NOT_CONFIGURED.installDir)).toBeInTheDocument();
    expect(screen.getByText(NOT_CONFIGURED.voicesDir)).toBeInTheDocument();
    expect(screen.getAllByText('piper.exe').length).toBeGreaterThan(0);
    expect(screen.getByText(/\.onnx\.json/)).toBeInTheDocument();
  });

  it('cannot test a voice that is not configured', async () => {
    withStatus(NOT_CONFIGURED);
    render(<VoiceSettings />);

    expect(await screen.findByRole('button', { name: /test voice/i })).toBeDisabled();
  });

  it('shows readiness, the engine and where the program came from', async () => {
    withStatus(READY);
    render(<VoiceSettings />);

    expect(await screen.findByText('Ready')).toBeInTheDocument();
    expect(screen.getByText('Local Piper')).toBeInTheDocument();
    expect(screen.getByText(/found in relay's voice folder/i)).toBeInTheDocument();
  });

  it('hides the setup instructions once it works', async () => {
    withStatus(READY);
    render(<VoiceSettings />);

    await screen.findByText('Ready');
    expect(screen.queryByText(/setting up a local voice/i)).not.toBeInTheDocument();
  });

  it('offers the discovered voices, including a Hindi one', async () => {
    withStatus(READY);
    render(<VoiceSettings />);

    const picker = await screen.findByLabelText(/voice model/i);
    expect(picker).toHaveValue(READY.voicePath);
    expect(
      screen.getByRole('option', { name: /hi_IN-pratham-medium/ }),
    ).toBeInTheDocument();
  });

  it('persists a voice change through the backend', async () => {
    const user = userEvent.setup();
    withStatus(READY);
    render(<VoiceSettings />);

    const picker = await screen.findByLabelText(/voice model/i);
    await user.selectOptions(picker, 'C:\\voices\\hi_IN-pratham-medium.onnx');

    await waitFor(() => {
      expect(mockedInvoke).toHaveBeenCalledWith('set_tts_configuration', {
        binaryPath: null,
        voicePath: 'C:\\voices\\hi_IN-pratham-medium.onnx',
      });
    });
  });

  it('flags a voice missing its sidecar rather than silently offering it', async () => {
    withStatus({
      ...READY,
      availableVoices: [
        {
          path: 'C:\\voices\\broken.onnx',
          label: 'broken',
          language: null,
          has_config: false,
        },
      ],
    });
    render(<VoiceSettings />);

    expect(
      await screen.findByRole('option', { name: /missing \.onnx\.json/ }),
    ).toBeInTheDocument();
  });

  it('browses for a program and saves what was picked', async () => {
    const user = userEvent.setup();
    withStatus(READY, { browse_for_piper_binary: 'D:\\tools\\piper\\piper.exe' });
    render(<VoiceSettings />);

    await screen.findByText('Ready');
    const [programBrowse] = await screen.findAllByRole('button', { name: /browse/i });
    await user.click(programBrowse);

    await waitFor(() => {
      expect(mockedInvoke).toHaveBeenCalledWith('set_tts_configuration', {
        binaryPath: 'D:\\tools\\piper\\piper.exe',
        voicePath: null,
      });
    });
  });

  it('saves nothing when the file picker is cancelled', async () => {
    const user = userEvent.setup();
    withStatus(READY, { browse_for_piper_binary: null });
    render(<VoiceSettings />);

    await screen.findByText('Ready');
    const [programBrowse] = await screen.findAllByRole('button', { name: /browse/i });
    await user.click(programBrowse);

    await waitFor(() => {
      expect(mockedInvoke).toHaveBeenCalledWith('browse_for_piper_binary');
    });
    expect(mockedInvoke).not.toHaveBeenCalledWith(
      'set_tts_configuration',
      expect.anything(),
    );
  });

  it('speaks a test sentence through the real provider', async () => {
    const user = userEvent.setup();
    withStatus(READY, { test_tts_voice: 'UklGRgABBBB' });
    render(<VoiceSettings />);

    await user.click(await screen.findByRole('button', { name: /test voice/i }));

    await waitFor(() => {
      expect(mockedInvoke).toHaveBeenCalledWith('test_tts_voice');
    });
    // Playback started, so a stop control replaces the test button.
    expect(await screen.findByRole('button', { name: /stop/i })).toBeInTheDocument();
  });

  it('shows an actionable message when the test fails', async () => {
    const user = userEvent.setup();
    withStatus(READY, {
      test_tts_voice: Object.assign(new Error('x'), {
        message: 'Piper reported success but wrote no audio.',
      }),
    });
    render(<VoiceSettings />);

    await user.click(await screen.findByRole('button', { name: /test voice/i }));

    expect(
      await screen.findByText(/wrote no audio/i),
    ).toBeInTheDocument();
  });

  it('creates the managed folders on request', async () => {
    const user = userEvent.setup();
    withStatus(NOT_CONFIGURED);
    render(<VoiceSettings />);

    await user.click(
      await screen.findByRole('button', { name: /create voice folder/i }),
    );

    await waitFor(() => {
      expect(mockedInvoke).toHaveBeenCalledWith('prepare_tts_folders');
    });
  });

  it('re-checks on demand without a restart', async () => {
    const user = userEvent.setup();
    withStatus(NOT_CONFIGURED);
    render(<VoiceSettings />);

    await screen.findByText('Not configured');
    withStatus(READY);
    await user.click(screen.getByRole('button', { name: /re-check voice setup/i }));

    expect(await screen.findByText('Ready')).toBeInTheDocument();
  });

  it('survives a backend that cannot answer', async () => {
    mockedInvoke.mockRejectedValue(new Error('backend down'));
    render(<VoiceSettings />);

    expect(
      await screen.findByText(/could not read the voice configuration/i),
    ).toBeInTheDocument();
  });
});
