import React, { useEffect, useRef } from 'react';
import type { TalkbackStateName } from '../../types';

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

/** Per-state palette, in design-system tokens rather than raw colours. */
const PALETTE: Record<AgentVisualState, { core: string; halo: string; ring: string }> = {
  idle: { core: 'text-muted-foreground', halo: 'bg-muted', ring: 'border-border' },
  listening: { core: 'text-emerald-500', halo: 'bg-emerald-500', ring: 'border-emerald-500/40' },
  thinking: { core: 'text-indigo-400', halo: 'bg-indigo-400', ring: 'border-indigo-400/40' },
  speaking: { core: 'text-primary', halo: 'bg-primary', ring: 'border-primary/40' },
  interrupted: { core: 'text-amber-500', halo: 'bg-amber-500', ring: 'border-amber-500/40' },
  error: { core: 'text-destructive', halo: 'bg-destructive', ring: 'border-destructive/40' },
};

interface TalkbackAgentProps {
  state: TalkbackStateName;
  /** Live amplitude, 0–1. Drives the halo while listening. */
  level?: number;
  size?: number;
}

/**
 * The conversational presence.
 *
 * Not a waveform: a waveform says "audio is being recorded", which is the
 * exact framing Talkback is trying to escape (`docs/talkback/RESEARCH.md`
 * §A). This reads as an entity whose state you can tell at a glance —
 * breathing when idle, responsive to your voice when listening, busy when
 * thinking, pulsing when it speaks.
 *
 * Amplitude is smoothed here rather than in the hook because it is purely
 * a rendering concern; a raw RMS stream at 25 Hz looks like jitter.
 */
export const TalkbackAgent: React.FC<TalkbackAgentProps> = ({
  state,
  level = 0,
  size = 148,
}) => {
  const visual = visualStateFor(state);
  const palette = PALETTE[visual];
  const haloRef = useRef<HTMLDivElement>(null);
  const smoothed = useRef(0);

  useEffect(() => {
    let frame = 0;
    const animate = () => {
      // Exponential smoothing towards the latest level. Only 'listening'
      // tracks the microphone; the other states have their own rhythm and
      // would otherwise twitch on room noise.
      const target = visual === 'listening' ? Math.min(1, level * 6) : 0;
      smoothed.current += (target - smoothed.current) * 0.18;
      if (haloRef.current) {
        const scale = 1 + smoothed.current * 0.45;
        haloRef.current.style.transform = `scale(${scale.toFixed(3)})`;
        haloRef.current.style.opacity = (0.18 + smoothed.current * 0.35).toFixed(3);
      }
      frame = requestAnimationFrame(animate);
    };
    frame = requestAnimationFrame(animate);
    return () => cancelAnimationFrame(frame);
  }, [level, visual]);

  return (
    <div
      className="flex flex-col items-center gap-4 select-none"
      data-testid="talkback-agent"
      data-visual-state={visual}
    >
      <div
        className="relative flex items-center justify-center"
        style={{ width: size, height: size }}
        role="img"
        aria-label={agentCaption(state)}
      >
        {/* Amplitude halo — driven by rAF, never by React state. */}
        <div
          ref={haloRef}
          aria-hidden
          className={`absolute inset-0 rounded-full blur-2xl ${palette.halo}`}
          style={{ opacity: 0.18 }}
        />

        {/* Orbit ring: spins while thinking, still otherwise. */}
        <div
          aria-hidden
          className={`absolute inset-3 rounded-full border-2 border-dashed ${palette.ring} ${
            visual === 'thinking' ? 'animate-spin' : ''
          }`}
          style={visual === 'thinking' ? { animationDuration: '3.2s' } : undefined}
        />

        {/* Core. Breathes when idle or listening, pulses while speaking. */}
        <div
          aria-hidden
          className={`relative rounded-full bg-card border ${palette.ring} shadow-lg flex items-center justify-center transition-transform duration-500 ${
            visual === 'speaking' ? 'animate-pulse' : ''
          }`}
          style={{ width: size * 0.52, height: size * 0.52 }}
        >
          <div
            className={`rounded-full ${palette.halo} transition-all duration-300`}
            style={{
              width: size * (visual === 'idle' ? 0.12 : 0.2),
              height: size * (visual === 'idle' ? 0.12 : 0.2),
              opacity: visual === 'idle' ? 0.5 : 0.9,
            }}
          />
        </div>
      </div>

      <p
        className={`text-xs font-medium tracking-wide ${palette.core}`}
        aria-live="polite"
      >
        {agentCaption(state)}
      </p>
    </div>
  );
};
