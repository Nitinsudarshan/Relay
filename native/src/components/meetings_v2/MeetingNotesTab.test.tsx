import { render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { describe, expect, it, vi } from 'vitest';
import { MeetingNotesTab } from './MeetingNotesTab';
import { makeDirective, makeNotes, makeSpeaker } from '../../test/factories';

/**
 * The notes tab exists to be the *structured* input surface. A paragraph box
 * asks the user to write prose and hope a model acts on it; these tests assert
 * that the specific correction a user actually has — "Speaker 1 is Pranjali" —
 * can be entered as that, and reaches the backend as that.
 */
const props = (overrides: Partial<React.ComponentProps<typeof MeetingNotesTab>> = {}) => ({
  notes: makeNotes(),
  speakers: [makeSpeaker({ id: 'spk_1', fallback_label: 'Speaker 1' })],
  unresolved: [],
  isLoaded: true,
  onSave: vi.fn().mockResolvedValue(undefined),
  onAddDirective: vi.fn().mockResolvedValue(undefined),
  onRemoveDirective: vi.fn().mockResolvedValue(undefined),
  ...overrides,
});

describe('MeetingNotesTab', () => {
  it('offers the structured kinds before the paragraph box', () => {
    render(<MeetingNotesTab {...props()} />);

    expect(screen.getByRole('button', { name: 'Name a speaker' })).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Fix a misheard word' })).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Add a participant' })).toBeInTheDocument();
    // The paragraph box is available but collapsed: it is the fallback, not the
    // primary surface.
    expect(
      screen.getByRole('button', { name: /write it as a paragraph instead/i }),
    ).toBeInTheDocument();
    expect(
      screen.queryByPlaceholderText(/what mattered, what was decided/i),
    ).not.toBeInTheDocument();
  });

  it('sends a name correction as a typed directive against a real speaker', async () => {
    const onAddDirective = vi.fn().mockResolvedValue(undefined);
    render(<MeetingNotesTab {...props({ onAddDirective })} />);

    await userEvent.selectOptions(
      screen.getByRole('combobox'),
      'spk_1',
    );
    await userEvent.type(screen.getByPlaceholderText(/Speaker 2 → Pranjali/), 'Pranjali');
    await userEvent.click(screen.getByRole('button', { name: /^Add$/ }));

    await waitFor(() =>
      expect(onAddDirective).toHaveBeenCalledWith('SPEAKER_NAME', 'spk_1', 'Pranjali'),
    );
  });

  it('will not submit a name correction that does not say which speaker', async () => {
    const onAddDirective = vi.fn();
    render(<MeetingNotesTab {...props({ onAddDirective })} />);

    await userEvent.type(screen.getByPlaceholderText(/Speaker 2 → Pranjali/), 'Pranjali');
    expect(screen.getByRole('button', { name: /^Add$/ })).toBeDisabled();
    expect(onAddDirective).not.toHaveBeenCalled();
  });

  it('sends a kind that needs no subject without one', async () => {
    const onAddDirective = vi.fn().mockResolvedValue(undefined);
    render(<MeetingNotesTab {...props({ onAddDirective })} />);

    await userEvent.click(screen.getByRole('button', { name: 'Add a participant' }));
    await userEvent.type(
      screen.getByPlaceholderText(/somebody who was there but did not speak/i),
      'Rahul',
    );
    await userEvent.click(screen.getByRole('button', { name: /^Add$/ }));

    await waitFor(() =>
      expect(onAddDirective).toHaveBeenCalledWith('PARTICIPANT', null, 'Rahul'),
    );
  });

  it('lists stored directives and can remove one', async () => {
    const onRemoveDirective = vi.fn().mockResolvedValue(undefined);
    render(
      <MeetingNotesTab
        {...props({
          notes: makeNotes({
            directives: [
              makeDirective({ id: 'dir_1', subject: 'Speaker 1', value: 'Pranjali' }),
              makeDirective({
                id: 'dir_2',
                kind: 'TERM',
                subject: 'Lance TV',
                value: 'LanceDB',
              }),
            ],
          }),
          onRemoveDirective,
        })}
      />,
    );

    expect(screen.getByText('Pranjali')).toBeInTheDocument();
    expect(screen.getByText('LanceDB')).toBeInTheDocument();

    await userEvent.click(screen.getByRole('button', { name: 'Remove: LanceDB' }));
    await waitFor(() => expect(onRemoveDirective).toHaveBeenCalledWith('dir_2'));
  });

  it('says on the row when a correction could not be applied', () => {
    // Silently ignoring it would leave the user assuming it worked.
    render(
      <MeetingNotesTab
        {...props({
          notes: makeNotes({
            directives: [makeDirective({ id: 'dir_1', subject: 'Speaker 4', value: 'Ayush' })],
          }),
          unresolved: [
            {
              directive_id: 'dir_1',
              kind: 'SPEAKER_NAME',
              summary: 'Speaker 4 is Ayush',
              reason: 'there is no "Speaker 4" in this meeting',
            },
          ],
        })}
      />,
    );

    expect(screen.getByText(/Not applied/)).toBeInTheDocument();
    expect(screen.getByText(/no "Speaker 4" in this meeting/)).toBeInTheDocument();
  });

  it('surfaces a rejected directive instead of failing silently', async () => {
    const onAddDirective = vi
      .fn()
      .mockRejectedValue({ message: 'This note is empty' });
    render(<MeetingNotesTab {...props({ onAddDirective })} />);

    await userEvent.click(screen.getByRole('button', { name: 'Remember this' }));
    await userEvent.type(screen.getByPlaceholderText(/vault rewrite/i), 'x');
    await userEvent.click(screen.getByRole('button', { name: /^Add$/ }));

    expect(await screen.findByText('This note is empty')).toBeInTheDocument();
  });

  it('still offers the paragraph box, and opens it when there is prose already', () => {
    render(
      <MeetingNotesTab
        {...props({ notes: makeNotes({ during: 'ask about the funding gap' }) })}
      />,
    );
    expect(screen.getByDisplayValue('ask about the funding gap')).toBeInTheDocument();
  });

  it('shows a loading state rather than an empty form while notes load', () => {
    render(<MeetingNotesTab {...props({ notes: null, isLoaded: false })} />);
    expect(screen.queryByRole('button', { name: 'Name a speaker' })).not.toBeInTheDocument();
  });
});
