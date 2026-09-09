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


describe('VoiceNotePage - Phrase Correction', () => {
  const correctableNote: VaultNote = {
    id: 'note_1',
    title: 'Note 1',
    note_type: 'voice_note',
    created_at: '2026-08-20T10:00:00Z',
    updated_at: '2026-08-20T10:00:00Z',
    tags: [],
    source_audio: null,
    content: 'I was testing super base yesterday and then opened super base again.',
  };

  /**
   * Selects a range of the rendered transcript the way a user would, and lets
   * the page's `selectionchange` listener see it. jsdom does not fire that
   * event from `addRange`, so it is dispatched explicitly.
   */
  const selectPhrase = async (phrase: string, occurrence: 1 | 2 = 1) => {
    const paragraph = screen.getByText(correctableNote.content);
    const textNode = paragraph.firstChild!;
    const text = correctableNote.content;
    const at = occurrence === 1 ? text.indexOf(phrase) : text.lastIndexOf(phrase);

    const range = document.createRange();
    range.setStart(textNode, at);
    range.setEnd(textNode, at + phrase.length);
    const selection = window.getSelection()!;
    selection.removeAllRanges();
    selection.addRange(range);
    document.dispatchEvent(new Event('selectionchange'));
    await waitFor(() => expect(screen.getByText('Selected')).toBeInTheDocument());
    return at;
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
      if (cmd === 'get_settings') return { provider: 'ollama' };
      if (cmd === 'get_voice_notes') return [correctableNote];
      return undefined;
    });
  });

  const renderPage = async () => {
    render(<VoiceNotePage />);
    await waitFor(() => {
      expect(screen.getByText(correctableNote.content)).toBeInTheDocument();
    });
  };

  it('offers Correct and Add to Dictionary when a phrase is selected, and nothing before', async () => {
    await renderPage();
    expect(screen.queryByRole('button', { name: 'Correct' })).not.toBeInTheDocument();

    await selectPhrase('super base');

    expect(screen.getByRole('button', { name: 'Correct' })).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Add to Dictionary' })).toBeInTheDocument();
    // Two steps: no input until the user says which action they want.
    expect(screen.queryByLabelText('Replacement text')).not.toBeInTheDocument();
  });

  it('does not offer to correct a whitespace-only selection', async () => {
    await renderPage();
    const paragraph = screen.getByText(correctableNote.content);
    const at = correctableNote.content.indexOf(' yesterday');
    const range = document.createRange();
    range.setStart(paragraph.firstChild!, at);
    range.setEnd(paragraph.firstChild!, at + 1);
    const selection = window.getSelection()!;
    selection.removeAllRanges();
    selection.addRange(range);
    document.dispatchEvent(new Event('selectionchange'));

    await waitFor(() => {
      expect(screen.queryByText('Selected')).not.toBeInTheDocument();
    });
  });

  it('sends the selected occurrence as a character range, without learning it', async () => {
    const user = userEvent.setup();
    await renderPage();
    const at = await selectPhrase('super base', 2);

    await user.click(screen.getByRole('button', { name: 'Correct' }));
    await user.type(screen.getByLabelText('Replacement text'), 'Supabase');
    await user.click(screen.getByRole('button', { name: 'Replace' }));

    await waitFor(() => {
      expect(mockedInvoke).toHaveBeenCalledWith('correct_voice_note_phrase', {
        id: 'note_1',
        start: at,
        end: at + 'super base'.length,
        original: 'super base',
        replacement: 'Supabase',
        // The unticked box is the point: an ordinary correction is not
        // vocabulary, so nothing reaches the dictionary by default.
        learn: false,
      });
    });
  });

  it('passes learn: true only when Teach Relay is ticked', async () => {
    const user = userEvent.setup();
    await renderPage();
    await selectPhrase('super base');

    await user.click(screen.getByRole('button', { name: 'Correct' }));
    await user.type(screen.getByLabelText('Replacement text'), 'Supabase');
    await user.click(screen.getByRole('checkbox', { name: /Teach Relay this correction/i }));
    await user.click(screen.getByRole('button', { name: 'Replace' }));

    await waitFor(() => {
      expect(mockedInvoke).toHaveBeenCalledWith(
        'correct_voice_note_phrase',
        expect.objectContaining({ learn: true }),
      );
    });
  });

  it('leaves the note untouched when the correction is cancelled', async () => {
    const user = userEvent.setup();
    await renderPage();
    await selectPhrase('super base');

    await user.click(screen.getByRole('button', { name: 'Correct' }));
    await user.type(screen.getByLabelText('Replacement text'), 'Supabase');
    await user.click(screen.getByRole('button', { name: 'Cancel' }));

    expect(screen.queryByLabelText('Replacement text')).not.toBeInTheDocument();
    expect(screen.getByText(correctableNote.content)).toBeInTheDocument();
    expect(mockedInvoke).not.toHaveBeenCalledWith(
      'correct_voice_note_phrase',
      expect.anything(),
    );
  });

  it('shows the corrected note and an Undo that reverses it', async () => {
    const user = userEvent.setup();
    const corrected = {
      ...correctableNote,
      content: 'I was testing Supabase yesterday and then opened super base again.',
      updated_at: '2026-08-20T11:00:00Z',
    };
    mockedInvoke.mockImplementation(async (cmd: string) => {
      if (cmd === 'get_vault_location') {
        return { path: 'v', default_path: 'v', configured: true, accessible: true };
      }
      if (cmd === 'get_settings') return { provider: 'ollama' };
      if (cmd === 'get_voice_notes') return [correctableNote];
      if (cmd === 'correct_voice_note_phrase') {
        return {
          note: corrected,
          record: {
            id: 'corr_1',
            note_id: 'note_1',
            original: 'super base',
            replacement: 'Supabase',
            start: 14,
            corrected_at: '2026-08-20T11:00:00Z',
            learned: false,
          },
          learned: false,
        };
      }
      if (cmd === 'undo_voice_note_correction') return correctableNote;
      return undefined;
    });

    await renderPage();
    await selectPhrase('super base');
    await user.click(screen.getByRole('button', { name: 'Correct' }));
    await user.type(screen.getByLabelText('Replacement text'), 'Supabase');
    await user.click(screen.getByRole('button', { name: 'Replace' }));

    // Only the selected occurrence changed; the second is still there.
    await waitFor(() => {
      expect(screen.getByText(corrected.content)).toBeInTheDocument();
    });
    expect(screen.getByText(/Corrected "super base" → "Supabase"/)).toBeInTheDocument();

    await user.click(screen.getByRole('button', { name: /Undo/i }));

    await waitFor(() => {
      expect(mockedInvoke).toHaveBeenCalledWith('undo_voice_note_correction', { id: 'note_1' });
    });
    await waitFor(() => {
      expect(screen.getByText(correctableNote.content)).toBeInTheDocument();
    });
  });

  it('reports a correction the backend refused instead of pretending it applied', async () => {
    const user = userEvent.setup();
    mockedInvoke.mockImplementation(async (cmd: string) => {
      if (cmd === 'get_vault_location') {
        return { path: 'v', default_path: 'v', configured: true, accessible: true };
      }
      if (cmd === 'get_settings') return { provider: 'ollama' };
      if (cmd === 'get_voice_notes') return [correctableNote];
      if (cmd === 'correct_voice_note_phrase') {
        throw { message: 'this note has changed since the text was selected' };
      }
      return undefined;
    });

    await renderPage();
    await selectPhrase('super base');
    await user.click(screen.getByRole('button', { name: 'Correct' }));
    await user.type(screen.getByLabelText('Replacement text'), 'Supabase');
    await user.click(screen.getByRole('button', { name: 'Replace' }));

    await waitFor(() => {
      expect(screen.getByRole('alert')).toHaveTextContent(/has changed since/i);
    });
    expect(screen.getByText(correctableNote.content)).toBeInTheDocument();
  });

  it('adds the selected spelling to the dictionary without touching the note', async () => {
    const user = userEvent.setup();
    await renderPage();
    await selectPhrase('yesterday');

    await user.click(screen.getByRole('button', { name: 'Add to Dictionary' }));

    await waitFor(() => {
      expect(mockedInvoke).toHaveBeenCalledWith('add_dictionary_word', { word: 'yesterday' });
    });
    expect(mockedInvoke).not.toHaveBeenCalledWith(
      'correct_voice_note_phrase',
      expect.anything(),
    );
    expect(screen.getByText(correctableNote.content)).toBeInTheDocument();
  });

  it('stops offering Undo once the note has moved on', async () => {
    const user = userEvent.setup();
    const corrected = { ...correctableNote, content: 'I was testing Supabase yesterday.' };
    mockedInvoke.mockImplementation(async (cmd: string) => {
      if (cmd === 'get_vault_location') {
        return { path: 'v', default_path: 'v', configured: true, accessible: true };
      }
      if (cmd === 'get_settings') return { provider: 'ollama' };
      if (cmd === 'get_voice_notes') return [correctableNote];
      if (cmd === 'correct_voice_note_phrase') {
        return {
          note: corrected,
          record: {
            id: 'corr_1',
            note_id: 'note_1',
            original: 'super base',
            replacement: 'Supabase',
            start: 14,
            corrected_at: '2026-08-20T11:00:00Z',
            learned: false,
          },
          learned: false,
        };
      }
      if (cmd === 'undo_voice_note_correction') {
        throw { message: 'this note has changed since the text was selected' };
      }
      return undefined;
    });

    await renderPage();
    await selectPhrase('super base');
    await user.click(screen.getByRole('button', { name: 'Correct' }));
    await user.type(screen.getByLabelText('Replacement text'), 'Supabase');
    await user.click(screen.getByRole('button', { name: 'Replace' }));

    await user.click(await screen.findByRole('button', { name: /Undo/i }));

    // Refusing is the point — the full editor's change is not thrown away —
    // so the button goes rather than inviting the same failure again.
    await waitFor(() => {
      expect(screen.getByText(/has changed since/i)).toBeInTheDocument();
    });
    expect(screen.queryByRole('button', { name: /Undo/i })).not.toBeInTheDocument();
    expect(screen.getByText(corrected.content)).toBeInTheDocument();
  });

  it('keeps the full-note editor available alongside the popover', async () => {
    const user = userEvent.setup();
    await renderPage();
    await selectPhrase('super base');

    // The pencil still opens the whole note — the two workflows coexist.
    await user.click(screen.getByLabelText('Edit transcript'));
    expect(screen.getByDisplayValue(correctableNote.content)).toBeInTheDocument();
  });
});
