/**
 * The playback queue for Talkback's spoken answers.
 *
 * Playback lives here rather than in Rust deliberately: adding `rodio`
 * would drag a second `cpal` into the backend (see
 * `docs/talkback/RESEARCH.md` §E.4), and the browser audio stack already
 * gives the two things barge-in needs most — ordered queueing and instant
 * cancellation.
 *
 * The backend synthesizes one phrase at a time and emits each as it is
 * ready, so this must play them strictly in order while more are still
 * arriving, and drop everything the moment a turn is superseded.
 */

/** One synthesized phrase from the backend. */
export interface TalkbackAudioChunk {
  turnId: string;
  /** Order within the turn. Chunks can arrive out of order. */
  seq: number;
  /** The engine's cancellation counter at synthesis time. */
  generation: number;
  wavBase64: string;
}

/** Injectable so the queue is testable without a real audio element. */
export interface AudioSink {
  play(wavBase64: string): Promise<void>;
  stop(): void;
}

/**
 * Plays phrases in sequence and abandons them on interruption.
 *
 * Not a React hook: this is ordering and lifecycle logic, and keeping it
 * a plain class is what lets the awkward parts — out-of-order arrival,
 * cancellation mid-phrase, a chunk from a superseded turn — be unit
 * tested rather than eyeballed.
 */
export class TalkbackAudioQueue {
  private pending: TalkbackAudioChunk[] = [];
  private playing = false;
  private nextSeq = 0;
  private currentTurnId: string | null = null;
  private generation = 0;

  constructor(
    private readonly sink: AudioSink,
    private readonly onFinished?: () => void,
  ) {}

  /** Chunks queued but not yet played. */
  get pendingCount(): number {
    return this.pending.length;
  }

  get isPlaying(): boolean {
    return this.playing;
  }

  /**
   * Queues a chunk and starts playback if nothing is running.
   *
   * A chunk from an older generation is dropped: it was synthesized for a
   * turn the user has already talked over, and playing it would have the
   * agent answer a question that no longer stands.
   */
  enqueue(chunk: TalkbackAudioChunk): void {
    if (chunk.generation < this.generation) return;

    // A newer generation means a new turn — everything queued belonged to
    // the old one.
    if (chunk.generation > this.generation) {
      this.generation = chunk.generation;
      this.pending = [];
      this.nextSeq = 0;
      this.currentTurnId = chunk.turnId;
    }

    if (this.currentTurnId !== chunk.turnId) {
      this.currentTurnId = chunk.turnId;
      this.pending = [];
      this.nextSeq = 0;
    }

    this.pending.push(chunk);
    // Phrases are synthesized sequentially but the events are not ordered
    // by the bus, so sort rather than assume.
    this.pending.sort((a, b) => a.seq - b.seq);
    void this.drain();
  }

  /**
   * Barge-in. Stops the current phrase, drops the queue, and moves the
   * generation past anything still in flight.
   */
  interrupt(): void {
    this.generation += 1;
    this.pending = [];
    this.nextSeq = 0;
    this.playing = false;
    this.sink.stop();
  }

  /** Full reset, for switching Talkback off. */
  reset(): void {
    this.interrupt();
    this.currentTurnId = null;
  }

  private async drain(): Promise<void> {
    if (this.playing) return;
    this.playing = true;
    const generationAtStart = this.generation;

    try {
      // Strict ordering: a later phrase waits rather than jumping ahead
      // of a gap, or the answer is spoken out of sequence.
      while (this.pending.length > 0 && this.pending[0].seq === this.nextSeq) {
        const chunk = this.pending.shift()!;
        await this.sink.play(chunk.wavBase64);
        if (this.generation !== generationAtStart) return;
        this.nextSeq += 1;
      }
    } catch (err) {
      // A decode failure loses the voice, never the turn: the answer text
      // is already on screen.
      console.warn('[talkback] playback failed, continuing text-only', err);
    } finally {
      if (this.generation === generationAtStart) {
        this.playing = false;
        if (this.pending.length === 0) this.onFinished?.();
      }
    }
  }
}

/** An {@link AudioSink} backed by an HTML audio element. */
export const createElementSink = (): AudioSink => {
  let current: HTMLAudioElement | null = null;

  return {
    play(wavBase64: string) {
      return new Promise<void>((resolve, reject) => {
        const audio = new Audio(`data:audio/wav;base64,${wavBase64}`);
        current = audio;
        audio.onended = () => resolve();
        audio.onerror = () => reject(new Error('audio decode failed'));
        void audio.play().catch(reject);
      });
    },
    stop() {
      if (current) {
        current.pause();
        current.src = '';
        current = null;
      }
    },
  };
};
