import React from 'react';
import type { TalkbackStateName } from '../../types';
import { TalkbackOrbCanvas } from './TalkbackOrbCanvas';

/**
 * The visual states the agent can be in.
 *
 * Deliberately fewer than the backend's state machine: `STARTING` and
 * `TRANSCRIBING` are transient and would show as a flicker, so they read
 * as thinking. The agent renders state; it does not own it.
 */
export type AgentVisualState =
  | 'idle'
  | 'listening'
  | 'thinking'
  | 'speaking'
  | 'interrupted'
  | 'error';

/** Maps the backend's authoritative state onto what the agent shows. */
export const visualStateFor = (state: TalkbackStateName): AgentVisualState => {
  switch (state) {
    case 'OFF':
      return 'idle';
    case 'STARTING':
    case 'TRANSCRIBING':
    case 'THINKING':
      return 'thinking';
    case 'LISTENING':
    case 'USER_SPEAKING':
      return 'listening';
    case 'SPEAKING':
      return 'speaking';
    case 'INTERRUPTED':
      return 'interrupted';
    case 'ERROR':
      return 'error';
    default:
      return 'idle';
  }
};

/** What the agent is doing, in one line, for the caption and for a11y. */
export const agentCaption = (state: TalkbackStateName): string => {
  switch (state) {
    case 'OFF':
      return 'Talkback is off';
    case 'STARTING':
      return 'Waking up…';
    case 'LISTENING':
      return 'Listening';
    case 'USER_SPEAKING':
      return 'Listening — go on';
    case 'TRANSCRIBING':
      return 'Getting that down…';
    case 'THINKING':
      return 'Thinking…';
    case 'SPEAKING':
      return 'Speaking — talk over me any time';
    case 'INTERRUPTED':
      return 'Go ahead';
    case 'ERROR':
      return 'Something went wrong';
    default:
      return '';
  }
};

/** Concise single-word/phrase label for active immersive UI. */
export const agentStateLabel = (state: TalkbackStateName): string => {
  switch (state) {
    case 'OFF':
      return 'Off';
    case 'STARTING':
      return 'Waking up…';
    case 'LISTENING':
    case 'USER_SPEAKING':
      return 'Listening';
    case 'TRANSCRIBING':
      return 'Processing…';
    case 'THINKING':
      return 'Thinking…';
    case 'SPEAKING':
      return 'Speaking';
    case 'INTERRUPTED':
      return 'Go ahead';
    case 'ERROR':
      return 'Something went wrong';
    default:
      return '';
  }
};

/** Per-state text palette in design-system tokens. */
export const STATE_PALETTE: Record<AgentVisualState, { core: string; badge: string; glow: string }> = {
  idle: {
    core: 'text-muted-foreground',
    badge: 'bg-muted/50 text-muted-foreground border-border',
    glow: 'text-muted-foreground/40',
  },
  listening: {
    core: 'text-emerald-500 dark:text-emerald-400',
    badge: 'bg-emerald-500/10 text-emerald-500 border-emerald-500/30',
    glow: 'text-emerald-400/40',
  },
  thinking: {
    core: 'text-indigo-400 dark:text-indigo-300',
    badge: 'bg-indigo-500/10 text-indigo-400 border-indigo-500/30',
    glow: 'text-indigo-400/40',
  },
  speaking: {
    core: 'text-teal-500 dark:text-teal-300',
    badge: 'bg-teal-500/10 text-teal-400 border-teal-500/30',
    glow: 'text-teal-400/40',
  },
  interrupted: {
    core: 'text-amber-500 dark:text-amber-400',
    badge: 'bg-amber-500/10 text-amber-500 border-amber-500/30',
    glow: 'text-amber-400/40',
  },
  error: {
    core: 'text-destructive',
    badge: 'bg-destructive/10 text-destructive border-destructive/30',
    glow: 'text-destructive/40',
  },
};

interface TalkbackAgentProps {
  state: TalkbackStateName;
  /** Live microphone amplitude, 0–1. Drives reactive waves while listening. */
  level?: number;
  /** Live output speaker amplitude, 0–1. Drives acoustic waves while speaking. */
  outputLevel?: number;
  size?: number;
  showCaption?: boolean;
  className?: string;
}

/**
 * The conversational presence.
 *
 * An expressive, living AI entity that breathes quietly when idle, reacts in real-time
 * to microphone amplitude when listening, spins with computational energy
 * when thinking, and vibrates / pulses in sync with spoken voice audio.
 */
export const TalkbackAgent: React.FC<TalkbackAgentProps> = ({
  state,
  level = 0,
  outputLevel = 0,
  size = 180,
  showCaption = true,
  className = '',
}) => {
  const visual = visualStateFor(state);
  const palette = STATE_PALETTE[visual];

  return (
    <div
      className={`flex flex-col items-center gap-3.5 select-none ${className}`}
      data-testid="talkback-agent"
      data-visual-state={visual}
    >
      <div
        className="relative flex items-center justify-center"
        style={{ width: size, height: size }}
        role="img"
        aria-label={agentCaption(state)}
      >
        <TalkbackOrbCanvas
          visualState={visual}
          micLevel={level}
          outputLevel={outputLevel}
          size={size}
        />
      </div>

      {showCaption && (
        <div className="flex flex-col items-center gap-1 text-center">
          <p
            className={`text-xs font-semibold tracking-wide transition-colors duration-300 ${palette.core}`}
            aria-live="polite"
          >
            {agentCaption(state)}
          </p>
        </div>
      )}
    </div>
  );
};
