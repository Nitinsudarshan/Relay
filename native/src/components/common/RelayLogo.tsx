import React from 'react';

interface RelayLogoProps extends React.SVGProps<SVGSVGElement> {
  className?: string;
}

/**
 * Relay brand logo — bold "R" inside a circle ring.
 *
 * Uses `currentColor` so it inherits the parent text colour, making it
 * automatically correct for both light and dark mode without any JS logic.
 * The path is hand-traced to match the brand asset: thick outer ring,
 * bold R with a rounded bowl and diagonal leg.
 */
export const RelayLogo: React.FC<RelayLogoProps> = ({ className = 'w-6 h-6', ...props }) => {
  return (
    <svg
      viewBox="0 0 500 500"
      fill="currentColor"
      xmlns="http://www.w3.org/2000/svg"
      className={className}
      aria-label="Relay logo"
      {...props}
    >
      {/* Outer circle ring */}
      <path
        fillRule="evenodd"
        clipRule="evenodd"
        d="
          M 250 18
          C 122.1 18 18 122.1 18 250
          C 18 377.9 122.1 482 250 482
          C 377.9 482 482 377.9 482 250
          C 482 122.1 377.9 18 250 18 Z
          M 250 60
          C 145.3 60 60 145.3 60 250
          C 60 354.7 145.3 440 250 440
          C 354.7 440 440 354.7 440 250
          C 440 145.3 354.7 60 250 60 Z
        "
      />
      {/* Bold R letter — stem + bowl (with inner cutout) + diagonal leg */}
      <path
        fillRule="evenodd"
        clipRule="evenodd"
        d="
          M 152 134
          L 220 134
          C 308 134 362 168 362 218
          C 362 258 330 284 280 292
          L 362 374
          L 286 374
          L 210 294
          L 220 294
          L 220 374
          L 152 374
          Z

          M 220 178
          L 220 256
          C 270 256 316 240 316 218
          C 316 196 278 178 220 178
          Z
        "
      />
    </svg>
  );
};
