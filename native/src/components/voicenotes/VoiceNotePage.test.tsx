import { describe, expect, it, vi, beforeEach } from 'vitest';
import { render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { invoke } from '@tauri-apps/api/core';
import { VoiceNotePage } from './VoiceNotePage';
import { VaultNote } from '../../types';

const mockedInvoke = vi.mocked(invoke);

const sampleNormalNote: VaultNote = {
  id: 'note_1',
  title: 'Note 1',
  note_type: 'voice_note',
  created_at: '2026-08-20T10:00:00Z',
  updated_at: '2026-08-20T10:00:00Z',
  tags: [],
  source_audio: null,
  content: 'First transcript content.',
};

const sampleNormalNote2: VaultNote = {
  id: 'note_2',
  title: 'Note 2',
  note_type: 'voice_note',
  created_at: '2026-08-20T10:05:00Z',
  updated_at: '2026-08-20T10:05:00Z',
  tags: [],
  source_audio: null,
  content: 'Second transcript content.',
};

const sampleMergedNote: VaultNote = {
  id: 'note_1',
  title: 'Combined Note Title',
  note_type: 'voice_note',
  created_at: '2026-08-20T10:00:00Z',
  updated_at: '2026-08-20T10:10:00Z',
  tags: [],
  source_audio: null,
  content: 'First transcript content.\n\nSecond transcript content.',
  merged_from: ['note_1', 'note_2'],
};

beforeEach(() => {
  mockedInvoke.mockReset();
  mockedInvoke.mockImplementation(async (cmd: string) => {
    if (cmd === 'get_vault_location') {
      return {
        path: 'C:\\Users\\Test\\RelayVault',
        default_path: 'C:\\Users\\Test\\RelayVault',
        configured: true,
        accessible: true,
      };
    }
    if (cmd === 'get_settings') {
      return { provider: 'ollama' };
    }
    if (cmd === 'get_voice_notes') {
      return [sampleNormalNote2, sampleNormalNote];
    }
    return undefined;
  });
});

describe('VoiceNotePage - Reversible Merging', () => {
  it('renders normal voice notes without merged badge or unmerge button', async () => {
    render(<VoiceNotePage />);

    await waitFor(() => {
      expect(screen.getByText('First transcript content.')).toBeInTheDocument();
    });

    expect(screen.queryByText(/Merged ·/i)).not.toBeInTheDocument();
    expect(screen.queryByTitle('Unmerge this Voice Note')).not.toBeInTheDocument();
  });

  it('renders merged badge and unmerge button for merged notes', async () => {
    mockedInvoke.mockImplementation(async (cmd: string) => {
      if (cmd === 'get_vault_location') {
        return {
          path: 'C:\\Vault',
          default_path: 'C:\\Vault',
          configured: true,
          accessible: true,
        };
      }
      if (cmd === 'get_voice_notes') {
        return [sampleMergedNote];
      }
      return undefined;
    });

    render(<VoiceNotePage />);

    await waitFor(() => {
      expect(screen.getByText(/Merged · 2 Voice Notes/i)).toBeInTheDocument();
    });

    expect(screen.getByTitle('Unmerge this Voice Note')).toBeInTheDocument();
  });

  it('shows confirmation banner when Unmerge button is clicked and cancels correctly', async () => {
    const user = userEvent.setup();
    mockedInvoke.mockImplementation(async (cmd: string) => {
      if (cmd === 'get_vault_location') {
        return {
          path: 'C:\\Vault',
          default_path: 'C:\\Vault',
          configured: true,
          accessible: true,
        };
      }
      if (cmd === 'get_voice_notes') {
        return [sampleMergedNote];
      }
      return undefined;
    });

    render(<VoiceNotePage />);

    await waitFor(() => {
      expect(screen.getByTitle('Unmerge this Voice Note')).toBeInTheDocument();
    });

    await user.click(screen.getByTitle('Unmerge this Voice Note'));

    expect(screen.getByText('Unmerge this Voice Note?')).toBeInTheDocument();
    expect(
      screen.getByText('This will restore the original Voice Notes and remove the merged version.')
    ).toBeInTheDocument();

    // Click Cancel
    await user.click(screen.getByRole('button', { name: 'Cancel' }));

    expect(screen.queryByText('Unmerge this Voice Note?')).not.toBeInTheDocument();
  });

  it('handles successful unmerge state transition', async () => {
    const user = userEvent.setup();
    mockedInvoke.mockImplementation(async (cmd: string, args?: any) => {
      if (cmd === 'get_vault_location') {
        return {
          path: 'C:\\Vault',
          default_path: 'C:\\Vault',
          configured: true,
          accessible: true,
        };
      }
      if (cmd === 'get_voice_notes') {
        return [sampleMergedNote];
      }
      if (cmd === 'unmerge_voice_note') {
        expect(args).toEqual({ id: 'note_1' });
        return {
          primary: sampleNormalNote,
          secondary: sampleNormalNote2,
        };
      }
      return undefined;
    });

    render(<VoiceNotePage />);

    await waitFor(() => {
      expect(screen.getByTitle('Unmerge this Voice Note')).toBeInTheDocument();
    });

    await user.click(screen.getByTitle('Unmerge this Voice Note'));
    await user.click(screen.getByRole('button', { name: 'Unmerge' }));

    await waitFor(() => {
      expect(screen.getByText('First transcript content.')).toBeInTheDocument();
      expect(screen.getByText('Second transcript content.')).toBeInTheDocument();
    });

    expect(screen.queryByText(/Merged ·/i)).not.toBeInTheDocument();
  });

  it('displays user-visible error banner on unmerge failure', async () => {
    const user = userEvent.setup();
    mockedInvoke.mockImplementation(async (cmd: string) => {
      if (cmd === 'get_vault_location') {
        return {
          path: 'C:\\Vault',
          default_path: 'C:\\Vault',
          configured: true,
          accessible: true,
        };
      }
      if (cmd === 'get_voice_notes') {
        return [sampleMergedNote];
      }
      if (cmd === 'unmerge_voice_note') {
        throw new Error('Merge stack file missing or corrupt');
      }
      return undefined;
    });

    render(<VoiceNotePage />);

    await waitFor(() => {
      expect(screen.getByTitle('Unmerge this Voice Note')).toBeInTheDocument();
    });

    await user.click(screen.getByTitle('Unmerge this Voice Note'));
    await user.click(screen.getByRole('button', { name: 'Unmerge' }));

    await waitFor(() => {
      expect(screen.getByText('Merge stack file missing or corrupt')).toBeInTheDocument();
    });

    // Merged note remains present
    expect(screen.getByText(/Merged · 2 Voice Notes/i)).toBeInTheDocument();
  });
});
