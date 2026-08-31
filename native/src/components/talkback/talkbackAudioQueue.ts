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
  /** The transcribed text segment being spoken. */
  text?: string;
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
    private readonly onChunkStart?: (chunk: TalkbackAudioChunk) => void,
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
        this.onChunkStart?.(chunk);
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

/** An {@link AudioSink} backed by an HTML audio element with real-time level metering. */
export const createElementSink = (onLevel?: (level: number) => void): AudioSink => {
  let current: HTMLAudioElement | null = null;
  let animId: number | null = null;
  let audioCtx: AudioContext | null = null;
  let analyser: AnalyserNode | null = null;
  let dataArray: Uint8Array | null = null;

  const cleanupMeter = () => {
    if (animId !== null) {
      cancelAnimationFrame(animId);
      animId = null;
    }
    onLevel?.(0);
  };

  const startMeter = (audio: HTMLAudioElement) => {
    if (!onLevel) return;
    try {
      if (!audioCtx && typeof window !== 'undefined') {
        const AudioCtxClass =
          window.AudioContext ||
          (window as unknown as { webkitAudioContext?: typeof AudioContext }).webkitAudioContext;
        if (AudioCtxClass) {
          audioCtx = new AudioCtxClass();
        }
      }
      if (audioCtx && !analyser) {
        analyser = audioCtx.createAnalyser();
        analyser.fftSize = 128;
        analyser.smoothingTimeConstant = 0.5;
        dataArray = new Uint8Array(analyser.frequencyBinCount);
      }
      if (audioCtx && analyser) {
        if (audioCtx.state === 'suspended') {
          void audioCtx.resume().catch(() => {});
        }
        try {
          const sourceNode = audioCtx.createMediaElementSource(audio);
          sourceNode.connect(analyser);
          analyser.connect(audioCtx.destination);
        } catch {
          // If source node is already connected or not allowed, continue with fallback
        }
      }
    } catch {
      // AudioContext unavailable or restricted
    }

    const tick = () => {
      if (!current || current.paused || current.ended) {
        cleanupMeter();
        return;
      }
      if (analyser && dataArray) {
        analyser.getByteFrequencyData(dataArray);
        let sum = 0;
        for (let i = 0; i < dataArray.length; i++) {
          sum += dataArray[i];
        }
        const avg = sum / dataArray.length;
        const normalized = Math.min(1, avg / 120);
        onLevel(normalized);
      } else {
        // Subtle rhythmic modulation if Web Audio analyser node is unavailable
        const t = performance.now() / 120;
        const fallbackLevel = 0.25 + 0.2 * Math.sin(t) + 0.1 * Math.sin(t * 2.7);
        onLevel(fallbackLevel);
      }
      animId = requestAnimationFrame(tick);
    };
    animId = requestAnimationFrame(tick);
  };

  return {
    play(wavBase64: string) {
      return new Promise<void>((resolve, reject) => {
        cleanupMeter();
        const audio = new Audio(`data:audio/wav;base64,${wavBase64}`);
        current = audio;
        audio.onplay = () => {
          startMeter(audio);
        };
        audio.onended = () => {
          cleanupMeter();
          resolve();
        };
        audio.onerror = () => {
          cleanupMeter();
          reject(new Error('audio decode failed'));
        };
        void audio.play().catch((err) => {
          cleanupMeter();
          reject(err);
        });
      });
    },
    stop() {
      cleanupMeter();
      if (current) {
        current.pause();
        current.src = '';
        current = null;
      }
    },
  };
};
