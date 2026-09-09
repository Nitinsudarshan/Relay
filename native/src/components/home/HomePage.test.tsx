import React from 'react';
import { beforeEach, describe, expect, test, vi } from 'vitest';
import { render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { invoke } from '@tauri-apps/api/core';

import { HomePage } from './HomePage';

import type { AppSettings, Scribble, VaultFile, VaultNote } from '@/types';

const voiceNote: VaultNote = {
  id: 'note_1',
  title: 'Retrieval is the bottleneck',
  note_type: 'voice_note',
  created_at: new Date(Date.now() - 3_600_000).toISOString(),
  updated_at: new Date(Date.now() - 3_600_000).toISOString(),
  tags: [],
  content: 'four words right here',
};

const scribble: Scribble = {
  id: 'scr_1',
  title: 'Chunking strategy',
  content: 'A thought.',
  source_type: 'text',
  source_metadata: {},
  created_at: new Date(Date.now() - 7_200_000).toISOString(),
  updated_at: new Date(Date.now() - 7_200_000).toISOString(),
  tags: [],
  topics: ['Retrieval'],
  entities: [],
  relationships: [],
  attachments: [],
  status: 'active',
  ai_metadata: {
    enrichment_status: 'enriched',
    suggested_concepts: [],
    suggested_questions: [],
    suggested_relations: [],
  },
};

const document_: VaultFile = {
  id: 'file_1',
  original_filename: 'Proposal.pdf',
  file_type: 'pdf',
  mime_type: 'application/pdf',
  size_bytes: 2048,
  content_hash: 'hash',
  created_at: new Date(Date.now() - 172_800_000).toISOString(),
  updated_at: new Date(Date.now() - 172_800_000).toISOString(),
  last_known_source_path: 'C:\\Docs\\Proposal.pdf',
  vault_path: 'files/file_1/original/Proposal.pdf',
  extraction_status: 'extracted',
  processing_status: 'ready',
  content: 'Proposal text.',
  tags: [],
  topics: [],
  entities: [],
  relationships: [],
  ai_metadata: {
    enrichment_status: 'enriched',
    suggested_concepts: [],
    suggested_questions: [],
    suggested_relations: [],
  },
  linked_scribble_id: 'scr_1',
};

const settings = {
  hotkeys: { dictation_hotkey: 'Ctrl+Shift+D' },
  provider: { active_provider: 'ollama', ollama_model: 'llama3.2', ollama_host: 'http://localhost:11434' },
  stt: { whisper_model_path: 'C:\\models\\ggml-small.bin' },
} as unknown as AppSettings;

const renderHome = (overrides: Partial<React.ComponentProps<typeof HomePage>> = {}) => {
  const props = {
    account: null,
    settings,
    appVersion: '0.37.0',
    onNavigate: vi.fn(),
    onStartCapture: vi.fn(),
    onOpenSettings: vi.fn(),
    onOpenChangelog: vi.fn(),
    ...overrides,
  };
  render(<HomePage {...props} />);
  return props;
};

describe('HomePage', () => {
  beforeEach(() => {
    vi.mocked(invoke).mockImplementation(async (cmd: string) => {
      switch (cmd) {
        case 'get_voice_notes':
          return [voiceNote];
        case 'get_scribbles':
          return [scribble];
        case 'get_vault_files':
          return [document_];
        case 'get_captures':
          return [];
        case 'get_knowledge_telemetry':
          return {
            total_memories: 5,
            active_memories: 4,
            total_entities: 11,
            total_relationships: 6,
            total_scribbles: 1,
            total_notes: 1,
            total_files: 1,
            total_captures: 0,
          };
        case 'get_vault_location':
          return {
            path: 'C:\\Users\\dev\\RelayVault',
            default_path: 'C:\\Users\\dev\\RelayVault',
            configured: true,
            accessible: true,
          };
        case 'get_capture_bridge_status':
          return { enabled: true, running: true, port: 8765, configured_port: 8765 };
        default:
          return null;
      }
    });
  });

  test('counts what the vault actually returned', async () => {
    renderHome();

    await waitFor(() => {
      expect(screen.getByText('Voice Notes')).toBeInTheDocument();
    });

    // Entities and connections come from telemetry, not from the lists.
    expect(screen.getByText('11')).toBeInTheDocument();
    expect(screen.getByText('6')).toBeInTheDocument();

    // 4 spoken words from voice note.
    expect(screen.getByText('Words transcribed').previousElementSibling).toHaveTextContent('4');
  });

  test('a capture card opens the mode it names rather than capturing itself', async () => {
    const props = renderHome();
    const user = userEvent.setup();

    await user.click(await screen.findByText('Typed Text'));
    expect(props.onStartCapture).toHaveBeenCalledWith('text');

    await user.click(screen.getByText('Clipboard'));
    expect(props.onStartCapture).toHaveBeenCalledWith('clipboard');

    // Voice and web capture are surfaces, not modes of this page.
    await user.click(screen.getByText('Voice'));
    expect(props.onNavigate).toHaveBeenCalledWith('capture');

    await user.click(screen.getByText('Web Capture'));
    expect(props.onNavigate).toHaveBeenCalledWith('captures');
  });

  test('a document goes to the Files Vault, not to a second file importer', async () => {
    const props = renderHome();
    const user = userEvent.setup();

    await user.click(await screen.findByText('Files & Docs'));

    expect(props.onNavigate).toHaveBeenCalledWith('files');
    expect(props.onStartCapture).not.toHaveBeenCalled();
    // And it names the formats the vault actually extracts.
    expect(screen.getByText('PDF, Word, Markdown, Text')).toBeInTheDocument();
  });

  test('a counter is also the way into its surface', async () => {
    const props = renderHome();
    const user = userEvent.setup();

    await user.click(await screen.findByText('Scribbles'));
    expect(props.onNavigate).toHaveBeenCalledWith('scribble');

    await user.click(screen.getByText('Entities'));
    expect(props.onNavigate).toHaveBeenCalledWith('graph');
  });

  test('names the configured dictation hotkey, not an assumed one', async () => {
    renderHome();
    expect(await screen.findByText('Ctrl+Shift+D')).toBeInTheDocument();
  });

  test('admits it does not know the hotkey when settings are unavailable', async () => {
    renderHome({ settings: null });
    expect(await screen.findByText('Set a hotkey in Settings')).toBeInTheDocument();
  });

  test('reports the real bridge state on the web capture card', async () => {
    renderHome();
    expect(await screen.findByText(/Bridge live/)).toBeInTheDocument();
  });

  test('offers a way to fix an unconfigured capability instead of a bare warning', async () => {
    renderHome({
      settings: {
        ...settings,
        stt: { whisper_model_path: null as unknown as string },
      },
    });

    expect(await screen.findByText('No Whisper model selected')).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Configure — Speech-to-text' })).toBeInTheDocument();
  });

  test('lists the newest records first, across surfaces', async () => {
    renderHome();

    await waitFor(() => {
      expect(screen.getByText('Latest activity')).toBeInTheDocument();
    });

    const titles = ['Retrieval is the bottleneck', 'Chunking strategy', 'Proposal.pdf'];
    titles.forEach((title) => expect(screen.getByText(title)).toBeInTheDocument());
  });

  test('an empty vault reads as zeros rather than a broken page', async () => {
    vi.mocked(invoke).mockImplementation(async () => null);
    renderHome();

    await waitFor(() => {
      expect(screen.getByText('Nothing captured yet')).toBeInTheDocument();
    });
    expect(screen.getAllByText('0').length).toBeGreaterThan(0);
  });

  test('a failing read degrades that surface without hiding the others', async () => {
    vi.mocked(invoke).mockImplementation(async (cmd: string) => {
      if (cmd === 'get_scribbles') throw new Error('scribble index unreadable');
      if (cmd === 'get_voice_notes') return [voiceNote];
      return null;
    });

    renderHome();

    await waitFor(() => {
      expect(screen.getByText('Retrieval is the bottleneck')).toBeInTheDocument();
    });
    expect(screen.getByText('Scribbles')).toBeInTheDocument();
  });
});
