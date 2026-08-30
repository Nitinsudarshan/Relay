import { describe, expect, it } from 'vitest';
import { render, screen } from '@testing-library/react';
import { TalkbackAgent, agentCaption, visualStateFor } from './TalkbackAgent';
import type { TalkbackStateName } from '../../types';

const ALL_STATES: TalkbackStateName[] = [
  'OFF',
  'STARTING',
  'LISTENING',
  'USER_SPEAKING',
  'TRANSCRIBING',
  'THINKING',
  'SPEAKING',
  'INTERRUPTED',
  'ERROR',
];

describe('visualStateFor', () => {
  it('maps every backend state to a visual state', () => {
    for (const state of ALL_STATES) {
      expect(visualStateFor(state)).toBeTruthy();
    }
  });

  it('collapses the transient states into thinking rather than flickering', () => {
    // STARTING and TRANSCRIBING last a few hundred milliseconds; giving
    // each its own look would read as a glitch, not as information.
    expect(visualStateFor('STARTING')).toBe('thinking');
    expect(visualStateFor('TRANSCRIBING')).toBe('thinking');
    expect(visualStateFor('THINKING')).toBe('thinking');
  });

  it('shows listening for both halves of the user’s turn', () => {
    expect(visualStateFor('LISTENING')).toBe('listening');
    expect(visualStateFor('USER_SPEAKING')).toBe('listening');
  });

  it('keeps off, speaking, interrupted and error distinct', () => {
    expect(visualStateFor('OFF')).toBe('idle');
    expect(visualStateFor('SPEAKING')).toBe('speaking');
    expect(visualStateFor('INTERRUPTED')).toBe('interrupted');
    expect(visualStateFor('ERROR')).toBe('error');
  });
});

describe('agentCaption', () => {
  it('gives every state a caption', () => {
    for (const state of ALL_STATES) {
      expect(agentCaption(state).length).toBeGreaterThan(0);
    }
  });

  it('says the microphone is off when Talkback is off', () => {
    expect(agentCaption('OFF')).toMatch(/off/i);
  });

  it('tells the user they can interrupt while it speaks', () => {
    expect(agentCaption('SPEAKING')).toMatch(/talk over/i);
  });
});

describe('TalkbackAgent', () => {
  it('renders the state it is given rather than deriving one', () => {
    const { rerender } = render(<TalkbackAgent state="LISTENING" />);
    expect(screen.getByTestId('talkback-agent')).toHaveAttribute(
      'data-visual-state',
      'listening',
    );

    rerender(<TalkbackAgent state="SPEAKING" />);
    expect(screen.getByTestId('talkback-agent')).toHaveAttribute(
      'data-visual-state',
      'speaking',
    );
  });

  it('exposes its state to assistive technology', () => {
    render(<TalkbackAgent state="THINKING" />);
    expect(screen.getByRole('img', { name: /thinking/i })).toBeInTheDocument();
  });

  it('renders at any amplitude without throwing', () => {
    for (const level of [0, 0.5, 1, 12]) {
      const { unmount } = render(<TalkbackAgent state="LISTENING" level={level} />);
      expect(screen.getByTestId('talkback-agent')).toBeInTheDocument();
      unmount();
    }
  });
});
