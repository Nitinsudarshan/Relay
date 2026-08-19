import React from 'react';
import { DictationPill } from './DictationPill';

/**
 * Rendered in its own always-on-top, transparent, non-focus-stealing Tauri
 * window (see overlay::ensure_pill_window in the Rust backend) so
 * "Click to dictate" lives on the desktop itself — reachable even while
 * Relay's main window is hidden — instead of being boxed inside the
 * dashboard's Capture tab.
 */
export const FloatingPill: React.FC = () => {
  return (
    <div className="flex h-screen w-screen items-center justify-center bg-transparent">
      <DictationPill />
    </div>
  );
};
