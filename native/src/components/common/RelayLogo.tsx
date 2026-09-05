import React from 'react';

interface RelayLogoProps extends React.SVGProps<SVGSVGElement> {
  className?: string;
}

/**
 * Relay brand logo — the molecule/share mark.
 *
 * Uses `currentColor` strokes so it inherits the parent's text colour.
 * In light mode the parent should be `text-foreground` (dark); in dark mode
 * the parent should be `text-foreground` (light). The logo automatically
 * adapts because it uses `stroke="currentColor"` and transparent fills.
 *
 * Circles use a double-ring outline style matching the brand asset.
 */
export const RelayLogo: React.FC<RelayLogoProps> = ({ className = 'w-6 h-6', ...props }) => {
  return (
    <svg
      viewBox="0 0 500 500"
      fill="none"
      xmlns="http://www.w3.org/2000/svg"
      className={className}
      aria-label="Relay logo"
      {...props}
    >
      {/* Connectors */}
      <line
        x1="245" y1="315" x2="145" y2="185"
        stroke="currentColor" strokeWidth="28" strokeLinecap="round"
      />
      <line
        x1="255" y1="315" x2="340" y2="185"
        stroke="currentColor" strokeWidth="28" strokeLinecap="round"
      />

      {/* Top-left node — small double ring */}
      <circle cx="128" cy="170" r="46" stroke="currentColor" strokeWidth="28" />
      <circle cx="128" cy="170" r="30" stroke="currentColor" strokeWidth="14" />

      {/* Top-right node — large double ring */}
      <circle cx="358" cy="170" r="76" stroke="currentColor" strokeWidth="28" />
      <circle cx="358" cy="170" r="58" stroke="currentColor" strokeWidth="14" />

      {/* Bottom-center node — medium double ring */}
      <circle cx="250" cy="336" r="58" stroke="currentColor" strokeWidth="28" />
      <circle cx="250" cy="336" r="40" stroke="currentColor" strokeWidth="14" />
    </svg>
  );
};
