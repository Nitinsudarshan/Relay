import React from 'react';

/**
 * The two marks the resting meeting pill is made of.
 *
 * Both live here rather than inside `MeetingRecordingOverlay` so the overlay
 * stays a state machine and the pill's visual identity is one small file that
 * can be redrawn without touching recording logic.
 */

interface MeetingPillSpiralProps {
  className?: string;
}

/**
 * The recording mark: a single continuous spiral.
 *
 * Drawn as one path rather than assembled from arcs so the stroke stays even all
 * the way in. It is deliberately monochrome and inherits `currentColor` — the
 * pill's only colour is the waveform, so the eye goes to the thing that moves.
 */
export const MeetingPillSpiral: React.FC<MeetingPillSpiralProps> = ({
  className = 'w-[22px] h-[22px]',
}) => (
  <svg viewBox="0 0 24 24" fill="none" className={className} aria-hidden="true">
    <path
      d="M12 3.2a8.8 8.8 0 1 1-8.8 8.8 6.9 6.9 0 0 1 6.9-6.9 5.2 5.2 0 0 1 5.2 5.2 3.6 3.6 0 0 1-3.6 3.6 2.1 2.1 0 0 1-2.1-2.1"
      stroke="currentColor"
      strokeWidth="1.9"
      strokeLinecap="round"
    />
  </svg>
);

interface MeetingPillWaveformProps {
  /** Newest level last. Values are clamped to 0..1. */
  levels: number[];
  /** Flattens the bars and drops them to the idle colour. */
  muted?: boolean;
  /** Bar height in pixels at full scale. */
  height?: number;
}

/** Minimum drawn bar height, so a silent waveform still reads as a waveform. */
const MIN_BAR_PX = 3;

/**
 * The live level meter: a few vertical bars, the only moving part of the pill.
 *
 * One combined meter rather than the separate mic and system waveforms the old
 * pill drew. At this size two opposing waveforms were unreadable, and the
 * question the pill answers is "is Relay still hearing the meeting" — which one
 * meter answers just as well.
 */
export const MeetingPillWaveform: React.FC<MeetingPillWaveformProps> = ({
  levels,
  muted = false,
  height = 20,
}) => (
  <div
    className="flex items-end justify-center gap-[3px]"
    style={{ height }}
    aria-hidden="true"
  >
    {levels.map((level, index) => {
      const clamped = Math.max(0, Math.min(1, level));
      const barPx = muted
        ? MIN_BAR_PX
        : Math.max(MIN_BAR_PX, Math.round(clamped * height));
      return (
        <span
          key={index}
          className={`w-[3px] rounded-full transition-[height] duration-100 ease-out ${
            muted ? 'bg-neutral-600' : 'bg-lime-400'
          }`}
          style={{ height: `${barPx}px` }}
        />
      );
    })}
  </div>
);
