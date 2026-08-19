import React from "react";

export function RelayLogo({
  className = "w-6 h-6",
  ...props
}: React.SVGProps<SVGSVGElement>) {
  return (
    <svg
      viewBox="0 0 24 24"
      fill="none"
      xmlns="http://www.w3.org/2000/svg"
      className={className}
      {...props}
    >
      {/* Vertical bar - inherits text/foreground color */}
      <rect x="3" y="3" width="3.2" height="18" rx="1.6" className="fill-foreground" />
      {/* Top horizontal bar - electric blue accent */}
      <rect x="8.5" y="3" width="8.5" height="3.2" rx="1.6" className="fill-primary" />
      {/* Middle horizontal bar (asymmetric, longer) - electric blue accent */}
      <rect x="8.5" y="10.4" width="12.5" height="3.2" rx="1.6" className="fill-primary" />
      {/* Bottom horizontal bar - electric blue accent */}
      <rect x="8.5" y="17.8" width="8.5" height="3.2" rx="1.6" className="fill-primary" />
    </svg>
  );
}
