import React from 'react';
import { Mic } from 'lucide-react';

/**
 * Rendered in a separate, always-on-top, non-focus-stealing Tauri window
 * (see hotkeys::ensure_indicator_window in the Rust backend) while the
 * push-to-talk dictation hotkey is held down. Its only job is to make it
 * visually obvious that Relay is listening, wherever the user's cursor
 * actually is.
 */
export const DictationIndicator: React.FC = () => {
  return (
    <div className="flex h-screen w-screen items-center justify-center gap-2 rounded-2xl bg-red-600 px-4 text-white shadow-2xl">
      <Mic className="h-5 w-5 animate-pulse" />
      <span className="text-sm font-semibold tracking-wide">Listening…</span>
    </div>
  );
};
