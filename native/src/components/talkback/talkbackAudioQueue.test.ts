import { describe, expect, it, vi } from 'vitest';
import {
  TalkbackAudioQueue,
  type AudioSink,
  type TalkbackAudioChunk,
} from './talkbackAudioQueue';

/**
 * A sink that records what it played and lets a test control when each
 * phrase "finishes", so ordering and interruption are observable rather
 * than timing-dependent.
 */
const makeSink = () => {
  const played: string[] = [];
  const stops: number[] = [];
  let resolveCurrent: (() => void) | null = null;

  const sink: AudioSink = {
    play(wavBase64: string) {
      played.push(wavBase64);
      return new Promise<void>((resolve) => {
        resolveCurrent = resolve;
      });
    },
    stop() {
      stops.push(played.length);
      resolveCurrent?.();
      resolveCurrent = null;
    },
  };

  return {
    sink,
    played,
    stops,
    /** Completes the phrase currently playing. */
    finishCurrent: async () => {
      resolveCurrent?.();
      resolveCurrent = null;
      // Let the queue's async drain loop advance.
      await Promise.resolve();
      await Promise.resolve();
    },
  };
};

const chunk = (
  seq: number,
  audio: string,
  overrides: Partial<TalkbackAudioChunk> = {},
): TalkbackAudioChunk => ({
  turnId: 'turn_1',
  seq,
  generation: 1,
  wavBase64: audio,
  ...overrides,
});

describe('TalkbackAudioQueue', () => {
  it('plays phrases in sequence', async () => {
    const { sink, played, finishCurrent } = makeSink();
    const queue = new TalkbackAudioQueue(sink);

    queue.enqueue(chunk(0, 'a'));
    queue.enqueue(chunk(1, 'b'));
    queue.enqueue(chunk(2, 'c'));

    expect(played).toEqual(['a']);
    await finishCurrent();
    expect(played).toEqual(['a', 'b']);
    await finishCurrent();
    expect(played).toEqual(['a', 'b', 'c']);
  });

  it('waits for a missing phrase rather than playing out of order', async () => {
    const { sink, played, finishCurrent } = makeSink();
    const queue = new TalkbackAudioQueue(sink);

    // Phrase 1 arrives before phrase 0 — the event bus does not order.
    queue.enqueue(chunk(1, 'second'));
    expect(played).toEqual([]);

    queue.enqueue(chunk(0, 'first'));
    expect(played).toEqual(['first']);
    await finishCurrent();
    expect(played).toEqual(['first', 'second']);
  });

  it('stops immediately on interruption and drops what is queued', async () => {
    const { sink, played, stops } = makeSink();
    const queue = new TalkbackAudioQueue(sink);

    queue.enqueue(chunk(0, 'a'));
    queue.enqueue(chunk(1, 'b'));
    queue.enqueue(chunk(2, 'c'));
    expect(played).toEqual(['a']);

    queue.interrupt();

    expect(stops).toHaveLength(1);
    expect(queue.pendingCount).toBe(0);
    expect(played).toEqual(['a']);
  });

  it('never plays audio from a turn the user has talked over', async () => {
    const { sink, played } = makeSink();
    const queue = new TalkbackAudioQueue(sink);

    queue.enqueue(chunk(0, 'a'));
    queue.interrupt();

    // Synthesis for the abandoned turn finishes after the barge-in and
    // arrives late — playing it would answer a superseded question.
    queue.enqueue(chunk(1, 'stale'));
    expect(played).toEqual(['a']);
    expect(queue.pendingCount).toBe(0);
  });

  it('accepts audio from the turn that follows an interruption', async () => {
    const { sink, played } = makeSink();
    const queue = new TalkbackAudioQueue(sink);

    queue.enqueue(chunk(0, 'old'));
    queue.interrupt();

    queue.enqueue(
      chunk(0, 'new', { generation: 2, turnId: 'turn_2' }),
    );
    expect(played).toEqual(['old', 'new']);
  });

  it('resets sequencing when a new turn starts', async () => {
    const { sink, played, finishCurrent } = makeSink();
    const queue = new TalkbackAudioQueue(sink);

    queue.enqueue(chunk(0, 'turn1-a'));
    await finishCurrent();

    queue.enqueue(chunk(0, 'turn2-a', { turnId: 'turn_2' }));
    expect(played).toEqual(['turn1-a', 'turn2-a']);
  });

  it('reports completion once the queue drains', async () => {
    const onFinished = vi.fn();
    const { sink, finishCurrent } = makeSink();
    const queue = new TalkbackAudioQueue(sink, onFinished);

    queue.enqueue(chunk(0, 'a'));
    expect(onFinished).not.toHaveBeenCalled();

    await finishCurrent();
    expect(onFinished).toHaveBeenCalledTimes(1);
    expect(queue.isPlaying).toBe(false);
  });

  it('survives a decode failure without wedging the queue', async () => {
    const failing: AudioSink = {
      play: vi.fn(async () => {
        throw new Error('decode failed');
      }),
      stop: vi.fn(),
    };
    const queue = new TalkbackAudioQueue(failing);

    queue.enqueue(chunk(0, 'broken'));
    await Promise.resolve();
    await Promise.resolve();

    // The answer text is already on screen; losing the voice must not
    // leave the queue permanently "playing".
    expect(queue.isPlaying).toBe(false);
  });

  it('clears everything on reset', () => {
    const { sink, stops } = makeSink();
    const queue = new TalkbackAudioQueue(sink);

    queue.enqueue(chunk(0, 'a'));
    queue.enqueue(chunk(1, 'b'));
    queue.reset();

    expect(queue.pendingCount).toBe(0);
    expect(queue.isPlaying).toBe(false);
    expect(stops.length).toBeGreaterThan(0);
  });
});
