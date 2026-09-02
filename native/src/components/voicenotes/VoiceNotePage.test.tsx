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

describe('VoiceNotePage - Multi-Select and Bulk Delete', () => {
  it('hides checkboxes until Select button is clicked and does not feature Select All', async () => {
    const user = userEvent.setup();
    render(<VoiceNotePage />);

    await waitFor(() => {
      expect(screen.getByText('First transcript content.')).toBeInTheDocument();
    });

    // Checkboxes should not be present initially
    expect(screen.queryByRole('checkbox')).not.toBeInTheDocument();
    expect(screen.queryByRole('button', { name: /Select All/i })).not.toBeInTheDocument();

    const selectBtn = screen.getByRole('button', { name: /^Select$/i });
    expect(selectBtn).toBeInTheDocument();

    // Click Select button to enter select mode
    await user.click(selectBtn);

    const checkboxes = screen.getAllByRole('checkbox');
    expect(checkboxes).toHaveLength(2);
    expect(screen.getByRole('button', { name: 'Done' })).toBeInTheDocument();
    expect(screen.queryByRole('button', { name: /Select All/i })).not.toBeInTheDocument();

    // Select both notes individually
    await user.click(checkboxes[0]);
    await user.click(checkboxes[1]);

    expect(screen.getByText('2 selected')).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Delete Selected (2)' })).toBeInTheDocument();

    // Clear selection
    await user.click(screen.getByRole('button', { name: 'Clear' }));
    expect(screen.queryByText('2 selected')).not.toBeInTheDocument();
  });

  it('handles bulk delete confirmation and deletion call', async () => {
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
        return [sampleNormalNote2, sampleNormalNote];
      }
      if (cmd === 'delete_voice_notes') {
        expect(args).toEqual({ ids: ['note_2', 'note_1'] });
        return 2;
      }
      return undefined;
    });

    render(<VoiceNotePage />);

    await waitFor(() => {
      expect(screen.getByText('First transcript content.')).toBeInTheDocument();
    });

    // Enter Select mode
    await user.click(screen.getByRole('button', { name: /^Select$/i }));

    const checkboxes = screen.getAllByRole('checkbox');
    await user.click(checkboxes[0]);
    await user.click(checkboxes[1]);

    // Click Delete Selected (2)
    await user.click(screen.getByRole('button', { name: 'Delete Selected (2)' }));

    // Confirmation banner should be displayed
    expect(
      screen.getByText(/Move 2 selected Voice Notes to Trash\?/i)
    ).toBeInTheDocument();

    // Confirm Move 2 to Trash
    await user.click(screen.getByRole('button', { name: /Move 2 to Trash/i }));

    await waitFor(() => {
      expect(screen.queryByText('First transcript content.')).not.toBeInTheDocument();
      expect(screen.queryByText('Second transcript content.')).not.toBeInTheDocument();
      expect(screen.getByText('No Voice Notes yet')).toBeInTheDocument();
    });
  });
});

