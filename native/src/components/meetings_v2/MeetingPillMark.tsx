import React from 'react';

/**
 * The pill's visual identity, kept in one small file so it can be redrawn
 * without touching recording logic — `MeetingRecordingOverlay` stays a state
 * machine and this stays a drawing.
 */

interface MeetingPillWaveformProps {
  /** Microphone levels, newest last. Clamped to 0..1. */
  mic: number[];
  /** System-audio levels, newest last, on the same timeline as `mic`. */
  sys: number[];
  /** Flattens the bars and drops them to the idle colour. */
  muted?: boolean;
}

/** Half the waveform's height: the microphone above the centreline, the meeting below. */
const HALF_PX = 11;
/** Every bar keeps this much height, so silence is a line rather than a gap. */
const MIN_BAR_PX = 2;
/** Below this a bar is drawn idle, so room tone does not tint the whole meter. */
const AUDIBLE = 0.02;

const barPx = (level: number) =>
  Math.max(MIN_BAR_PX, Math.round(Math.max(0, Math.min(1, level)) * HALF_PX));

/**
 * The live level meter: one mirrored waveform, and the only moving part.
 *
 * Both channels are drawn on a single strip sharing one timeline — the
 * microphone above the centreline, the meeting's audio below it — so "who is
 * talking right now" is legible without a legend explaining which half is
 * which. Two separate meters side by side needed labels to be read at all, and
 * labels are what a pill has no room for.
 */
export const MeetingPillWaveform: React.FC<MeetingPillWaveformProps> = ({
  mic,
  sys,
  muted = false,
}) => (
  <div
    className="flex items-center gap-[2px]"
    style={{ height: HALF_PX * 2 + 2 }}
    aria-hidden="true"
  >
    {mic.map((micLevel, index) => {
      const sysLevel = sys[index] ?? 0;
      const micLive = !muted && micLevel > AUDIBLE;
      const sysLive = !muted && sysLevel > AUDIBLE;
      return (
        <div
          key={index}
          className="flex flex-col items-center justify-center gap-[2px] w-[3px]"
        >
          <span
            className={`w-full rounded-full transition-[height] duration-75 ease-out ${
              micLive ? 'bg-indigo-400' : 'bg-neutral-700'
            }`}
            style={{ height: muted ? MIN_BAR_PX : barPx(micLevel) }}
          />
          <span
            className={`w-full rounded-full transition-[height] duration-75 ease-out ${
              sysLive ? 'bg-sky-400' : 'bg-neutral-700'
            }`}
            style={{ height: muted ? MIN_BAR_PX : barPx(sysLevel) }}
          />
        </div>
      );
    })}
  </div>
);
