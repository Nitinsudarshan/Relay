import { describe, expect, it, vi, beforeEach } from 'vitest';
import { render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { VoiceSettings } from './VoiceSettings';
import type { CatalogueVoice, InstallProgress, TtsStatus } from '../../types';

const mockedInvoke = vi.mocked(invoke);
const mockedListen = vi.mocked(listen);

const amy: CatalogueVoice = {
  id: 'en_US-amy-medium',
  displayName: 'English (US) — Amy',
  languageLabel: 'English (US)',
  description: 'Clear and neutral. Recommended.',
  recommended: true,
  installed: false,
  downloadBytes: 63_000_000,
};

const pratham: CatalogueVoice = {
  id: 'hi_IN-pratham-medium',
  displayName: 'हिन्दी — Pratham',
  languageLabel: 'Hindi',
  description: 'For Hindi and mixed English-Hindi speech.',
  recommended: false,
  installed: false,
  downloadBytes: 61_000_000,
};

const NOT_INSTALLED: TtsStatus = {
  engine: 'none',
  ready: false,
  binaryPath: null,
  binaryOrigin: null,
  voicePath: null,
  voiceLabel: null,
  voiceLanguage: null,
  availableVoices: [],
  problems: ['No Piper executable found.'],
  installDir: 'C:\\Users\\nitin\\AppData\\Roaming\\Relay\\tts\\piper',
  voicesDir: 'C:\\Users\\nitin\\AppData\\Roaming\\Relay\\tts\\voices',
  executableName: 'piper.exe',
  canInstall: true,
  installBlockedReason: null,
  recommendedVoice: amy,
  catalogue: [amy, pratham],
  downloadBytes: 84_000_000,
  engineVersion: '1.6.0',
};

const READY: TtsStatus = {
  ...NOT_INSTALLED,
  engine: 'piper',
  ready: true,
  binaryPath: 'C:\\Users\\nitin\\AppData\\Roaming\\Relay\\tts\\piper\\piper.exe',
  binaryOrigin: 'managed',
  voicePath: 'C:\\Users\\nitin\\AppData\\Roaming\\Relay\\tts\\voices\\en_US-amy-medium.onnx',
  voiceLabel: 'en_US-amy-medium',
  voiceLanguage: 'en_US',
  problems: [],
  catalogue: [{ ...amy, installed: true }, pratham],
  downloadBytes: 0,
};

const UNSUPPORTED: TtsStatus = {
  ...NOT_INSTALLED,
  canInstall: false,
  installBlockedReason: "Automatic voice setup isn't available for aarch64 processors yet",
  recommendedVoice: null,
  catalogue: [],
};

const withStatus = (status: TtsStatus, overrides: Record<string, unknown> = {}) => {
  mockedInvoke.mockImplementation(async (command: string) => {
    if (command in overrides) {
      const value = overrides[command];
      if (value instanceof Error) throw value;
      return value;
    }
    if (command === 'get_tts_status') return status;
    if (command === 'install_local_voice') return { ...status, ready: true };
    return undefined;
  });
};

beforeEach(() => {
  mockedInvoke.mockReset();
  mockedListen.mockReset();
  mockedListen.mockResolvedValue(() => {});
  vi.spyOn(window.HTMLMediaElement.prototype, 'play').mockResolvedValue();
  vi.spyOn(window.HTMLMediaElement.prototype, 'pause').mockImplementation(() => {});
});

describe('VoiceSettings — before setup', () => {
  it('offers one button and names no filesystem path', async () => {
    withStatus(NOT_INSTALLED);
    render(<VoiceSettings />);

    expect(await screen.findByText('Make Relay speak')).toBeInTheDocument();
    expect(
      screen.getByRole('button', { name: /download & set up/i }),
    ).toBeInTheDocument();

    // The whole point of this change: nothing about GitHub, ONNX, AppData
    // or file placement in the default experience.
    const body = document.body.textContent ?? '';
    for (const leak of ['piper.exe', 'AppData', '.onnx', 'GitHub', 'github.com']) {
      expect(body).not.toContain(leak);
    }
  });

  it('explains that the voice is local and private', async () => {
    withStatus(NOT_INSTALLED);
    render(<VoiceSettings />);

    expect(await screen.findByText(/entirely\s+on this computer/i)).toBeInTheDocument();
    expect(screen.getByText(/does not send the text it speaks/i)).toBeInTheDocument();
    expect(screen.getByText(/one-time setup/i)).toBeInTheDocument();
  });

  it('shows the recommended voice without asking the user to choose', async () => {
    withStatus(NOT_INSTALLED);
    render(<VoiceSettings />);

    expect(await screen.findByText('Recommended voice')).toBeInTheDocument();
    expect(screen.getByText('English (US) — Amy')).toBeInTheDocument();
    expect(screen.getByText(/80 MB to download/i)).toBeInTheDocument();
    // No picker before setup — choosing is a later, optional step.
    expect(screen.queryByLabelText(/^voice$/i)).not.toBeInTheDocument();
  });

  it('starts setup with no voice id, letting the backend pick', async () => {
    const user = userEvent.setup();
    withStatus(NOT_INSTALLED);
    render(<VoiceSettings />);

    await user.click(await screen.findByRole('button', { name: /download & set up/i }));

    await waitFor(() => {
      expect(mockedInvoke).toHaveBeenCalledWith('install_local_voice', { voiceId: null });
    });
  });

  it('says so plainly when Relay cannot install on this machine', async () => {
    withStatus(UNSUPPORTED);
    render(<VoiceSettings />);

    expect(await screen.findByText(/automatic setup unavailable/i)).toBeInTheDocument();
    expect(screen.getByText(/aarch64 processors/i)).toBeInTheDocument();
    expect(screen.getByText(/still answers in text/i)).toBeInTheDocument();
    expect(
      screen.queryByRole('button', { name: /download & set up/i }),
    ).not.toBeInTheDocument();
  });
});

describe('VoiceSettings — during setup', () => {
  /** Captures the progress listener so a test can drive it. */
  const captureProgress = () => {
    let emit: ((progress: InstallProgress) => void) | null = null;
    mockedListen.mockImplementation(async (event: string, handler: unknown) => {
      if (event === 'voice-install-progress') {
        emit = (progress) =>
          (handler as (e: { payload: InstallProgress }) => void)({ payload: progress });
      }
      return () => {};
    });
    return () => emit;
  };

  it('shows the current item and both progress bars', async () => {
    const user = userEvent.setup();
    const getEmit = captureProgress();
    // An install that never resolves, so the progress UI stays up.
    withStatus(NOT_INSTALLED, {
      install_local_voice: new Promise(() => {}) as unknown,
    });
    mockedInvoke.mockImplementation(async (command: string) => {
      if (command === 'get_tts_status') return NOT_INSTALLED;
      if (command === 'install_local_voice') return new Promise(() => {});
      return undefined;
    });

    render(<VoiceSettings />);
    await user.click(await screen.findByRole('button', { name: /download & set up/i }));

    expect(await screen.findByTestId('voice-installing')).toBeInTheDocument();

    getEmit()?.({
      stage: 'downloading_voice',
      label: 'Downloading voice',
      item: 'English (US) — Amy',
      receivedBytes: 41,
      totalBytes: 100,
      overall: 0.62,
    });

    expect(await screen.findByText('English (US) — Amy')).toBeInTheDocument();
    expect(await screen.findByText('41%')).toBeInTheDocument();
    expect(screen.getByText('62%')).toBeInTheDocument();
    expect(
      screen.getByRole('progressbar', { name: /overall setup progress/i }),
    ).toHaveAttribute('aria-valuenow', '62');
  });

  it('can be cancelled mid-download', async () => {
    const user = userEvent.setup();
    captureProgress();
    mockedInvoke.mockImplementation(async (command: string) => {
      if (command === 'get_tts_status') return NOT_INSTALLED;
      if (command === 'install_local_voice') return new Promise(() => {});
      return undefined;
    });

    render(<VoiceSettings />);
    await user.click(await screen.findByRole('button', { name: /download & set up/i }));
    await user.click(await screen.findByRole('button', { name: /^cancel$/i }));

    await waitFor(() => {
      expect(mockedInvoke).toHaveBeenCalledWith('cancel_voice_install');
    });
  });

  it('hides Advanced while installing', async () => {
    const user = userEvent.setup();
    captureProgress();
    mockedInvoke.mockImplementation(async (command: string) => {
      if (command === 'get_tts_status') return NOT_INSTALLED;
      if (command === 'install_local_voice') return new Promise(() => {});
      return undefined;
    });

    render(<VoiceSettings />);
    await user.click(await screen.findByRole('button', { name: /download & set up/i }));

    await screen.findByTestId('voice-installing');
    expect(screen.queryByText('Advanced')).not.toBeInTheDocument();
  });
});

describe('VoiceSettings — after setup', () => {
  it('confirms readiness and names the voice', async () => {
    withStatus(READY);
    render(<VoiceSettings />);

    const banner = await screen.findByText('Local voice ready');
    // The header names the active voice; the picker below lists all of
    // them, so scope the assertion to the header.
    expect(banner.parentElement).toHaveTextContent('English (US) — Amy — Recommended');
  });

  it('offers a voice picker of validated voices only', async () => {
    withStatus(READY);
    render(<VoiceSettings />);

    const picker = await screen.findByLabelText(/^voice$/i);
    expect(picker).toBeInTheDocument();
    // Exactly the catalogue — not every voice in the upstream repository.
    expect(screen.getAllByRole('option')).toHaveLength(READY.catalogue.length);
    expect(
      screen.getByRole('option', { name: /हिन्दी — Pratham/ }),
    ).toBeInTheDocument();
  });

  it('shows a download size for a voice that is not installed yet', async () => {
    withStatus(READY);
    render(<VoiceSettings />);

    const option = await screen.findByRole('option', { name: /Pratham/ });
    expect(option.textContent).toMatch(/58 MB download/);
  });

  it('installs the chosen voice through the same setup path', async () => {
    const user = userEvent.setup();
    withStatus(READY);
    render(<VoiceSettings />);

    await user.selectOptions(
      await screen.findByLabelText(/^voice$/i),
      'hi_IN-pratham-medium',
    );

    await waitFor(() => {
      expect(mockedInvoke).toHaveBeenCalledWith('install_local_voice', {
        voiceId: 'hi_IN-pratham-medium',
      });
    });
  });

  it('speaks a test sentence through the production path', async () => {
    const user = userEvent.setup();
    withStatus(READY, { test_tts_voice: 'UklGRgABBBB' });
    render(<VoiceSettings />);

    await user.click(await screen.findByRole('button', { name: /test voice/i }));

    await waitFor(() => {
      expect(mockedInvoke).toHaveBeenCalledWith('test_tts_voice');
    });
    expect(await screen.findByRole('button', { name: /stop/i })).toBeInTheDocument();
  });

  it('keeps the privacy wording accurate about the LLM', async () => {
    withStatus(READY);
    render(<VoiceSettings />);

    // Local *voice* — not a claim that the whole conversation is offline.
    expect(
      await screen.findByText(/generated on this computer/i),
    ).toBeInTheDocument();
    expect(screen.getByText(/whichever\s+AI provider you have configured/i)).toBeInTheDocument();
  });

  it('keeps paths and versions under Advanced', async () => {
    const user = userEvent.setup();
    withStatus(READY);
    render(<VoiceSettings />);

    await screen.findByText('Local voice ready');
    expect(screen.queryByText(/piper\.exe/)).not.toBeInTheDocument();

    await user.click(screen.getByText('Advanced'));
    expect(screen.getByText(/piper\.exe/)).toBeInTheDocument();
    expect(screen.getByText('1.6.0')).toBeInTheDocument();
  });
});

describe('VoiceSettings — failure', () => {
  it('shows a plain message and keeps the retry button', async () => {
    const user = userEvent.setup();
    withStatus(NOT_INSTALLED, {
      install_local_voice: Object.assign(new Error('x'), {
        code: 'CHECKSUM',
        message:
          "A downloaded file didn't match what Relay expected and was discarded. This usually means the download was interrupted — try again.",
      }),
    });
    render(<VoiceSettings />);

    await user.click(await screen.findByRole('button', { name: /download & set up/i }));

    expect(await screen.findByText(/didn't match what Relay expected/i)).toBeInTheDocument();
    // Recoverable: the primary action is still there to press again.
    expect(
      screen.getByRole('button', { name: /download & set up/i }),
    ).toBeInTheDocument();
  });

  it('never shows a Rust error, a path or a URL', async () => {
    const user = userEvent.setup();
    withStatus(NOT_INSTALLED, {
      install_local_voice: Object.assign(new Error('x'), {
        code: 'NETWORK',
        message: 'The download couldn’t be completed. Check your connection and try again.',
      }),
    });
    render(<VoiceSettings />);

    await user.click(await screen.findByRole('button', { name: /download & set up/i }));
    await screen.findByText(/check your connection/i);

    const body = document.body.textContent ?? '';
    expect(body).not.toContain('https://');
    expect(body).not.toContain('.rs:');
    expect(body).not.toContain('Err(');
  });

  it('survives a backend that cannot answer', async () => {
    mockedInvoke.mockRejectedValue(new Error('backend down'));
    render(<VoiceSettings />);

    expect(
      await screen.findByText(/could not read the voice configuration/i),
    ).toBeInTheDocument();
  });
});
