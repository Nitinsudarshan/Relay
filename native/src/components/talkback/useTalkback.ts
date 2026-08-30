import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import type {
  TalkbackContextItem,
  TalkbackMetrics,
  TalkbackSession,
  TalkbackStateName,
  TalkbackTurn,
} from '../../types';
import {
  TalkbackAudioQueue,
  createElementSink,
  type TalkbackAudioChunk,
} from './talkbackAudioQueue';

/** Backend event names, mirroring `talkback::engine`. */
const EVENTS = {
  state: 'talkback-state',
  turn: 'talkback-turn',
  delta: 'talkback-delta',
  audio: 'talkback-audio',
  level: 'talkback-level',
  metrics: 'talkback-metrics',
  error: 'talkback-error',
  utterance: 'talkback-utterance',
} as const;

export interface UseTalkback {
  state: TalkbackStateName;
  turns: TalkbackTurn[];
  /** The answer currently being generated, before it becomes a turn. */
  streamingText: string;
  /** Microphone amplitude, 0–1, for the agent animation. */
  level: number;
  lastMetrics: TalkbackMetrics | null;
  error: string | null;
  busy: boolean;
  start: (voice: boolean) => Promise<void>;
  stop: () => Promise<void>;
  interrupt: () => Promise<void>;
  send: (text: string) => Promise<void>;
}

/**
 * Subscribes to the Talkback engine and exposes its state.
 *
 * The hook holds no conversation logic of its own: state comes from the
 * backend's `talkback-state` events and turns from `talkback-turn`. What
 * it *does* own is playback — the browser is where Talkback's audio is
 * played (see `talkbackAudioQueue`).
 */
export const useTalkback = (): UseTalkback => {
  const [state, setState] = useState<TalkbackStateName>('OFF');
  const [turns, setTurns] = useState<TalkbackTurn[]>([]);
  const [streamingText, setStreamingText] = useState('');
  const [level, setLevel] = useState(0);
  const [lastMetrics, setLastMetrics] = useState<TalkbackMetrics | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  const queue = useMemo(() => new TalkbackAudioQueue(createElementSink()), []);
  // Read inside event callbacks, which capture their first render's
  // closure; a ref keeps them looking at the live value.
  const stateRef = useRef<TalkbackStateName>('OFF');
  stateRef.current = state;

  useEffect(() => {
    const unlisteners = [
      listen<{ state: TalkbackStateName }>(EVENTS.state, (event) => {
        const next = event.payload?.state;
        if (!next) return;
        setState(next);
        // An interruption invalidates queued audio for the abandoned
        // turn; clearing here is what makes barge-in feel immediate
        // rather than "after it finishes this sentence".
        if (next === 'INTERRUPTED' || next === 'OFF') {
          queue.interrupt();
          setStreamingText('');
        }
      }),

      listen<TalkbackTurn>(EVENTS.turn, (event) => {
        const turn = event.payload;
        if (!turn) return;
        setTurns((previous) =>
          previous.some((t) => t.turn_id === turn.turn_id)
            ? previous
            : [...previous, turn],
        );
        if (turn.role === 'agent') setStreamingText('');
      }),

      listen<{ text: string }>(EVENTS.delta, (event) => {
        const delta = event.payload?.text;
        if (delta) setStreamingText((previous) => previous + delta);
      }),

      listen<TalkbackAudioChunk>(EVENTS.audio, (event) => {
        if (event.payload) queue.enqueue(event.payload);
      }),

      listen<{ level: number }>(EVENTS.level, (event) => {
        setLevel(event.payload?.level ?? 0);
      }),

      listen<TalkbackMetrics>(EVENTS.metrics, (event) => {
        if (event.payload) setLastMetrics(event.payload);
      }),

      listen<{ code: string; message: string }>(EVENTS.error, (event) => {
        setError(event.payload?.message ?? 'Talkback failed');
      }),

      // A spoken utterance the backend transcribed. It arrives as an
      // event rather than a command result because the voice worker runs
      // on its own thread and has no invoke to return to.
      listen<{ text: string; sttMs: number }>(EVENTS.utterance, (event) => {
        const payload = event.payload;
        if (!payload?.text?.trim()) return;
        void invoke('submit_talkback_turn', {
          text: payload.text,
          typed: false,
          sttMs: payload.sttMs,
        }).catch((err) => setError(String(err?.message ?? err)));
      }),
    ];

    return () => {
      unlisteners.forEach((pending) => {
        void pending.then((unlisten) => unlisten());
      });
      queue.reset();
    };
  }, [queue]);

  const start = useCallback(
    async (voice: boolean) => {
      setError(null);
      setBusy(true);
      try {
        const next = await invoke<TalkbackStateName>('start_talkback', { voice });
        setState(next);
        const session = await invoke<TalkbackSession>('get_talkback_session');
        setTurns(session?.turns ?? []);
      } catch (err) {
        setError(errorMessage(err));
      } finally {
        setBusy(false);
      }
    },
    [],
  );

  const stop = useCallback(async () => {
    setBusy(true);
    try {
      queue.reset();
      const next = await invoke<TalkbackStateName>('stop_talkback');
      setState(next);
      setStreamingText('');
      setLevel(0);
    } catch (err) {
      setError(errorMessage(err));
    } finally {
      setBusy(false);
    }
  }, [queue]);

  const interrupt = useCallback(async () => {
    queue.interrupt();
    try {
      const next = await invoke<TalkbackStateName>('interrupt_talkback');
      setState(next);
    } catch (err) {
      setError(errorMessage(err));
    }
  }, [queue]);

  const send = useCallback(async (text: string) => {
    if (!text.trim()) return;
    setError(null);
    try {
      await invoke('submit_talkback_turn', { text, typed: true, sttMs: null });
    } catch (err) {
      setError(errorMessage(err));
    }
  }, []);

  return {
    state,
    turns,
    streamingText,
    level,
    lastMetrics,
    error,
    busy,
    start,
    stop,
    interrupt,
    send,
  };
};

/** Tauri `CommandError` is `{ code, message }`; everything else is best effort. */
const errorMessage = (err: unknown): string => {
  if (err && typeof err === 'object' && 'message' in err) {
    return String((err as { message: unknown }).message);
  }
  return String(err);
};

/** Source chips show where an answer came from, in the user's language. */
export const sourceLabel = (item: TalkbackContextItem): string => {
  switch (item.source_type) {
    case 'VOICE_NOTE':
      return 'Voice Note';
    case 'SCRIBBLE':
      return 'Scribble';
    case 'MEETING':
      return 'Meeting';
    case 'MEETING_FACTS':
      return 'Meeting Intelligence';
    default:
      return 'Source';
  }
};
