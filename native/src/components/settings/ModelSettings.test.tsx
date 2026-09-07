import { describe, expect, it, vi, beforeEach } from 'vitest';
import { render, screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { invoke } from '@tauri-apps/api/core';
import { ProviderSettings } from './ProviderSettings';
import { DiagnosticsPage } from '../diagnostics/DiagnosticsPage';
import type { AppSettings, OllamaModelDetails, SttModelsOverview } from '../../types';

vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(),
}));

vi.mock('@tauri-apps/api/event', () => ({
  listen: vi.fn().mockResolvedValue(() => {}),
}));

const DEFAULT_TEST_SETTINGS: AppSettings = {
  provider: {
    active_provider: 'ollama',
    ollama_host: 'http://localhost:11434',
    ollama_model: 'llama3.2:latest',
    cloud_model: 'gpt-4o-mini',
  },
  stt: { whisper_model_path: null, dictation_quality: 'accurate' },
  tts: { piper_binary_path: '', piper_voice_path: '' },
  hotkeys: {
    show_hide_hotkey: 'Ctrl+Shift+Space',
    dictation_hotkey: 'Ctrl+Space',
    toggle_to_talk: false,
    capture_hotkey: 'Ctrl+Shift+C',
  },
  ui: { pill_position: 'bottom_center' },
  vault: { directory: null },
  language: {
    primary_dictation_language: 'en',
    spoken_languages: ['en'],
    notes_language: 'en',
    output_script: 'latin',
  },
  diagnostics: {
    allow_anonymous_diagnostics: true,
    first_run_completed: true,
  },
  sound: { dictation_sounds: true },
  clipboard: { auto_paste: true, copy_to_clipboard: true },
  startup: { launch_at_login: false, start_minimized: false },
  audio_input: { prefer_builtin_mic: true, selected_device: null, keep_microphone_warm: 'off', auto_learn_words: true },
  talkback: { activation_mode: 'toggle', speak_responses: true, allow_barge_in: true, sources: [], end_of_turn_silence_ms: 1000 },
  dictionary: ['Relay', 'Whisper'],
  snippets: [],
};

const MOCK_OLLAMA_MODELS: OllamaModelDetails[] = [
  {
    name: 'llama3.2:latest',
    model: 'llama3.2:latest',
    size: 2019393189,
    parameter_size: '3.2B',
    quantization_level: 'Q4_K_M',
    family: 'llama',
    format: 'gguf',
  },
  {
    name: 'qwen2.5:7b',
    model: 'qwen2.5:7b',
    size: 4682000000,
    parameter_size: '7.6B',
    quantization_level: 'Q4_K_M',
    family: 'qwen2',
    format: 'gguf',
  },
];

const MOCK_STT_OVERVIEW: SttModelsOverview = {
  active_model_name: 'Whisper Small (Default)',
  active_model_path: 'C:\\Relay\\models\\ggml-small.bin',
  active_profile: 'accurate',
  models_dir: 'C:\\Relay\\models',
  models: [
    {
      name: 'Whisper Base',
      filename: 'ggml-base.bin',
      path: 'C:\\Relay\\models\\ggml-base.bin',
      size_bytes: 148000000,
      exists: true,
      is_managed: true,
      profile: 'fast',
      status: 'ready',
    },
    {
      name: 'Whisper Small (Default)',
      filename: 'ggml-small.bin',
      path: 'C:\\Relay\\models\\ggml-small.bin',
      size_bytes: 488000000,
      exists: true,
      is_managed: true,
      profile: 'accurate',
      status: 'ready',
    },
    // The accuracy ceiling, absent from disk. `get_stt_models_overview` always
    // returns all three tiers and lists this one whether or not it has been
    // downloaded, so the mock has to as well — a two-model mock could not see
    // the download offer at all.
    {
      name: 'Whisper Large v3 Turbo',
      filename: 'ggml-large-v3-turbo.bin',
      path: 'C:\\Relay\\models\\ggml-large-v3-turbo.bin',
      size_bytes: 0,
      exists: false,
      is_managed: true,
      profile: null,
      status: 'missing',
    },
  ],
};

describe('AI Models & STT Settings — Refactored Model Selection', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.mocked(invoke).mockImplementation(async (cmd: string) => {
      switch (cmd) {
        case 'get_settings':
          return DEFAULT_TEST_SETTINGS;
        case 'ensure_local_llm_ready':
          return { state: 'running' };
        case 'get_available_llm_models':
          return MOCK_OLLAMA_MODELS;
        case 'ensure_stt_model_ready':
          return { state: 'ready', path: 'C:\\Relay\\models\\ggml-small.bin' };
        case 'get_available_stt_models':
          return MOCK_STT_OVERVIEW;
        case 'get_audio_devices':
          return [{ name: 'Default Mic', is_default: true }];
        case 'get_app_version':
          return '0.29.0';
        case 'get_vault_location':
          return { path: 'C:\\Vault' };
        case 'save_settings':
          return undefined;
        case 'download_stt_model':
          return { state: 'ready', path: 'C:\\Relay\\models\\ggml-large-v3-turbo.bin' };
        default:
          return null;
      }
    });
  });

  it('renders available Ollama models queried from backend instead of only raw text input', async () => {
    render(<ProviderSettings initialSection="advanced" />);

    // Wait for settings and models to load
    await waitFor(() => {
      expect(screen.getByText('qwen2.5:7b')).toBeDefined();
    });

    // Both installed models should be rendered in the available models picker
    expect(screen.getAllByText('llama3.2:latest').length).toBeGreaterThanOrEqual(2);
    expect(screen.getByText('Available Models from Ollama (2)')).toBeDefined();

    // Verify parameter size badges
    expect(screen.getByText('3.2B')).toBeDefined();
    expect(screen.getByText('7.6B')).toBeDefined();
  });

  it('allows user to select an available Ollama model', async () => {
    const user = userEvent.setup();
    render(<ProviderSettings initialSection="advanced" />);

    await waitFor(() => {
      expect(screen.getByText('qwen2.5:7b')).toBeDefined();
    });

    // Click qwen2.5:7b model card
    const qwenButton = screen.getByText('qwen2.5:7b').closest('button');
    expect(qwenButton).not.toBeNull();
    if (qwenButton) {
      await user.click(qwenButton);
    }

    // Verify current selection updates
    await waitFor(() => {
      const currentCards = screen.getAllByText('qwen2.5:7b');
      expect(currentCards.length).toBeGreaterThanOrEqual(1);
    });
  });

  it('renders active STT model and data-driven available models list', async () => {
    render(<ProviderSettings initialSection="advanced" />);

    await waitFor(() => {
      expect(screen.getByText('Active STT Model')).toBeDefined();
    });

    // STT Model should be ready
    expect(screen.getByText('✓ Model ready · Whisper')).toBeDefined();

    // Available models list should show Whisper Base and Whisper Small
    expect(screen.getByText('Available STT Models on Disk (3)')).toBeDefined();
    expect(screen.getByText('Fast (Base Model)')).toBeDefined();
    expect(screen.getByText('Accurate (Small Model)')).toBeDefined();
  });

  // Two settings that shipped without a way to reach them: the accuracy tier
  // was listed as "missing" with no download, and the decode preset had
  // per-surface defaults with no control to override them.

  it('offers a download for a managed model that is listed but not on disk', async () => {
    const user = userEvent.setup();
    render(<ProviderSettings initialSection="advanced" />);

    await waitFor(() => {
      expect(screen.getByText('Whisper Large v3 Turbo')).toBeDefined();
    });

    const card = screen.getByText('Whisper Large v3 Turbo').closest('div.p-2\\.5');
    expect(card).not.toBeNull();
    const download = within(card as HTMLElement).getByRole('button', { name: /download/i });

    await user.click(download);

    await waitFor(() => {
      expect(vi.mocked(invoke)).toHaveBeenCalledWith('download_stt_model', {
        filename: 'ggml-large-v3-turbo.bin',
      });
    });
  });

  it('does not offer a download for a model already on disk', async () => {
    render(<ProviderSettings initialSection="advanced" />);

    await waitFor(() => {
      expect(screen.getByText('Whisper Base')).toBeDefined();
    });

    const card = screen.getByText('Whisper Base').closest('div.p-2\\.5');
    expect(card).not.toBeNull();
    expect(within(card as HTMLElement).queryByRole('button', { name: /download/i })).toBeNull();
  });

  it('scopes a model choice to meetings so dictation is not slowed with it', async () => {
    const user = userEvent.setup();
    render(<ProviderSettings initialSection="advanced" />);

    await waitFor(() => {
      expect(screen.getByText('Whisper Base')).toBeDefined();
    });

    expect(screen.getByText(/Meetings currently follow the global model/)).toBeDefined();

    const card = screen.getByText('Whisper Base').closest('div.p-2\\.5');
    const use = within(card as HTMLElement).getByRole('button', { name: /use for meetings/i });
    await user.click(use);

    // The choice is meetings-only: the card now reports it, and the global
    // dictation profile below is untouched.
    await waitFor(() => {
      expect(screen.getByText(/Meetings use the model marked above/)).toBeDefined();
    });
    expect(
      within(card as HTMLElement).getByRole('button', { name: /meetings use this/i }),
    ).toBeDefined();
  });

  it('exposes the decode preset and defaults to letting each surface choose', async () => {
    const user = userEvent.setup();
    render(<ProviderSettings initialSection="advanced" />);

    await waitFor(() => {
      expect(screen.getByText('Decode Preset')).toBeDefined();
    });

    // Automatic is the shipped default: the empty preset is what lets dictation
    // run Fast and meetings run Quality.
    const automatic = screen.getByText('Automatic').closest('button');
    expect(automatic?.className).toContain('border-primary');

    const quality = screen.getByText('Quality').closest('button');
    expect(quality).not.toBeNull();
    await user.click(quality as HTMLElement);

    await waitFor(() => {
      expect(screen.getByText('Quality').closest('button')?.className).toContain('border-primary');
    });
  });

  it('includes a link to dedicated Diagnostics page and removes telemetry from configuration view', async () => {
    const onNavigateTab = vi.fn();
    render(<ProviderSettings initialSection="advanced" onNavigateTab={onNavigateTab} />);

    await waitFor(() => {
      expect(screen.getByText('Need Technical Testing or Observability?')).toBeDefined();
    });

    const openDiagBtn = screen.getByText('Open Diagnostics');
    expect(openDiagBtn).toBeDefined();

    // Verify telemetry is NOT present on the settings page
    expect(screen.queryByText('Last Transcription Snapshot')).toBeNull();
    expect(screen.queryByText('Audio Telemetry')).toBeNull();
  });
});

describe('DiagnosticsPage — Technical Observability Hub', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.mocked(invoke).mockImplementation(async (cmd: string) => {
      switch (cmd) {
        case 'get_settings':
          return DEFAULT_TEST_SETTINGS;
        case 'ensure_local_llm_ready':
          return { state: 'running' };
        case 'get_available_llm_models':
          return MOCK_OLLAMA_MODELS;
        case 'get_available_stt_models':
          return MOCK_STT_OVERVIEW;
        case 'get_last_stt_diagnostics':
          return null;
        case 'get_stt_corpus':
          return [];
        case 'get_audio_devices':
          return [{ name: 'Default Microphone', is_default: true }];
        case 'get_app_version':
          return '0.29.0';
        case 'get_vault_location':
          return { path: 'C:\\Relay\\vault' };
        default:
          return null;
      }
    });
  });

  it('renders system status overview cards for LLM, STT, and TTS', async () => {
    render(<DiagnosticsPage />);

    await waitFor(() => {
      expect(screen.getByText('Diagnostics & System Health')).toBeDefined();
    });

    // Overview cards
    expect(screen.getByText('LLM Backend')).toBeDefined();
    expect(screen.getByText('Active LLM Model')).toBeDefined();
    expect(screen.getByText('STT Engine')).toBeDefined();
    expect(screen.getByText('Active STT Model')).toBeDefined();
    expect(screen.getByText('TTS Engine')).toBeDefined();
  });

  it('switches tabs to LLM diagnostics and displays installed models table', async () => {
    const user = userEvent.setup();
    render(<DiagnosticsPage />);

    await waitFor(() => {
      expect(screen.getByText('LLM Diagnostics & Latency')).toBeDefined();
    });

    // Click LLM Diagnostics tab
    await user.click(screen.getByText('LLM Diagnostics & Latency'));

    // Should display prompt test benchmark and installed models
    await waitFor(() => {
      expect(screen.getByText('Live LLM Prompt & Latency Benchmark')).toBeDefined();
      expect(screen.getByText('Installed Local Ollama Models (2)')).toBeDefined();
    });
  });
});
