import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

/**
 * Dictation audio feedback.
 *
 * These are the only code path behind the `sound.dictation_sounds` setting, so
 * a regression here turns a user-facing toggle into a control that changes
 * nothing — the failure mode `rules/ui-components.md` calls a fake control.
 * The Web Audio API is stubbed and the assertions are about the contract:
 * oscillators are created, routed to the destination, started, and stopped.
 *
 * The module caches one AudioContext across calls, so each test imports it
 * fresh after `resetModules` rather than sharing a context between cases.
 */

interface OscillatorStub {
  connect: ReturnType<typeof vi.fn>;
  start: ReturnType<typeof vi.fn>;
  stop: ReturnType<typeof vi.fn>;
  frequency: { setValueAtTime: ReturnType<typeof vi.fn> };
}

interface GainStub {
  connect: ReturnType<typeof vi.fn>;
}

const graph = {
  oscillators: [] as OscillatorStub[],
  gains: [] as GainStub[],
  destination: {} as object,
  resume: vi.fn(),
};

const stubAudioContext = (state = 'running') => {
  graph.oscillators = [];
  graph.gains = [];
  graph.destination = { id: 'destination' };
  graph.resume = vi.fn().mockResolvedValue(undefined);

  vi.stubGlobal(
    'AudioContext',
    vi.fn(() => ({
      state,
      currentTime: 0,
      destination: graph.destination,
      resume: graph.resume,
      createOscillator: () => {
        const osc: OscillatorStub = {
          connect: vi.fn(),
          start: vi.fn(),
          stop: vi.fn(),
          frequency: { setValueAtTime: vi.fn() },
        };
        graph.oscillators.push(osc);
        return osc;
      },
      createGain: () => {
        const gain: GainStub & { gain: object } = {
          connect: vi.fn(),
          gain: {
            setValueAtTime: vi.fn(),
            exponentialRampToValueAtTime: vi.fn(),
          },
        };
        graph.gains.push(gain);
        return gain;
      },
    })),
  );
};

type SoundName = 'playDictationStartSound' | 'playDictationStopSound';

const load = async (name: SoundName): Promise<() => void> => {
  const mod = await import('./soundEffects');
  return mod[name];
};

beforeEach(() => {
  vi.resetModules();
});

afterEach(() => {
  vi.unstubAllGlobals();
});

describe.each<[SoundName]>([['playDictationStartSound'], ['playDictationStopSound']])(
  '%s',
  (name) => {
    it('emits a two-tone chime', async () => {
      stubAudioContext();
      (await load(name))();
      expect(graph.oscillators).toHaveLength(2);
      expect(graph.gains).toHaveLength(2);
    });

    it('routes every oscillator through a gain node to the destination', async () => {
      stubAudioContext();
      (await load(name))();
      for (const osc of graph.oscillators) {
        expect(osc.connect).toHaveBeenCalledTimes(1);
      }
      for (const gain of graph.gains) {
        expect(gain.connect).toHaveBeenCalledWith(graph.destination);
      }
    });

    // An oscillator that is started and never stopped runs for the lifetime of
    // the audio context — a tone that never ends.
    it('schedules a stop for every oscillator it starts', async () => {
      stubAudioContext();
      (await load(name))();
      for (const osc of graph.oscillators) {
        expect(osc.start).toHaveBeenCalledTimes(1);
        expect(osc.stop).toHaveBeenCalledTimes(1);
      }
    });

    it('resumes a context the browser suspended', async () => {
      stubAudioContext('suspended');
      (await load(name))();
      expect(graph.resume).toHaveBeenCalled();
    });

    // Audio is a nicety; it must never take down dictation itself.
    it('stays silent rather than throwing when Web Audio is unavailable', async () => {
      vi.stubGlobal('AudioContext', undefined);
      const debug = vi.spyOn(console, 'debug').mockImplementation(() => {});
      const play = await load(name);
      expect(() => play()).not.toThrow();
      debug.mockRestore();
    });

    it('swallows a failure inside the audio graph', async () => {
      vi.stubGlobal(
        'AudioContext',
        vi.fn(() => ({
          state: 'running',
          currentTime: 0,
          destination: {},
          createOscillator: () => {
            throw new Error('audio device disappeared');
          },
          createGain: vi.fn(),
        })),
      );
      const debug = vi.spyOn(console, 'debug').mockImplementation(() => {});
      const play = await load(name);
      expect(() => play()).not.toThrow();
      debug.mockRestore();
    });
  },
);
