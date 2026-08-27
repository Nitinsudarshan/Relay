import React, { useCallback, useEffect, useState } from 'react';

const MODIFIER_KEYS = new Set(['Control', 'Shift', 'Alt', 'Meta']);

const NAMED_KEYS: Record<string, string> = {
  ArrowUp: 'Up',
  ArrowDown: 'Down',
  ArrowLeft: 'Left',
  ArrowRight: 'Right',
  Tab: 'Tab',
  Enter: 'Return',
  NumpadEnter: 'Return',
  Backspace: 'Backspace',
  Delete: 'Delete',
  Insert: 'Insert',
  Home: 'Home',
  End: 'End',
  PageUp: 'PageUp',
  PageDown: 'PageDown',
  Numpad0: 'Num0',
  Numpad1: 'Num1',
  Numpad2: 'Num2',
  Numpad3: 'Num3',
  Numpad4: 'Num4',
  Numpad5: 'Num5',
  Numpad6: 'Num6',
  Numpad7: 'Num7',
  Numpad8: 'Num8',
  Numpad9: 'Num9',
  NumpadDecimal: 'NumDecimal',
  NumpadAdd: 'NumAdd',
  NumpadSubtract: 'NumSubtract',
  NumpadMultiply: 'NumMultiply',
  NumpadDivide: 'NumDivide',
  '.': 'Period',
  ',': 'Comma',
  ';': 'Semicolon',
  '/': 'Slash',
  '\\': 'Backslash',
  '-': 'Minus',
  '=': 'Equal',
  '`': 'Backquote',
};

function normalizeKey(e: KeyboardEvent): string | null {
  if (e.code === 'Space') return 'Space';
  if (NAMED_KEYS[e.code]) return NAMED_KEYS[e.code];
  if (NAMED_KEYS[e.key]) return NAMED_KEYS[e.key];
  if (/^F([1-9]|1[0-9]|2[0-4])$/.test(e.key)) return e.key;
  if (/^[a-zA-Z]$/.test(e.key)) return e.key.toUpperCase();
  if (/^[0-9]$/.test(e.key)) return e.key;
  return null;
}

/** Builds a `tauri-plugin-global-shortcut` accelerator string, e.g. "Ctrl+Shift+Space". */
function eventToAccelerator(e: KeyboardEvent): string | null {
  if (MODIFIER_KEYS.has(e.key)) return null;

  const key = normalizeKey(e);
  if (!key) return null;

  const parts: string[] = [];
  if (e.ctrlKey) parts.push('Ctrl');
  if (e.altKey) parts.push('Alt');
  if (e.shiftKey) parts.push('Shift');
  if (e.metaKey) parts.push('Super');
  // Require at least one modifier: an un-modified global hotkey would
  // hijack every ordinary keystroke typed anywhere on the OS.
  if (parts.length === 0) return null;

  parts.push(key);
  return parts.join('+');
}

interface HotkeyRecorderProps {
  id?: string;
  value: string;
  onCapture: (accelerator: string) => void;
}

/**
 * A click-to-record hotkey input: click it, then press the actual key
 * combination you want — no typing a shortcut string by hand. Esc cancels.
 */
export const HotkeyRecorder: React.FC<HotkeyRecorderProps> = ({ id, value, onCapture }) => {
  const [recording, setRecording] = useState(false);

  const stopRecording = useCallback(() => setRecording(false), []);

  useEffect(() => {
    if (!recording) return;

    const handleKeyDown = (e: KeyboardEvent) => {
      e.preventDefault();
      e.stopPropagation();

      if (e.key === 'Escape') {
        stopRecording();
        return;
      }

      const accelerator = eventToAccelerator(e);
      if (accelerator) {
        onCapture(accelerator);
        stopRecording();
      }
    };

    // Capture phase so this wins even if focus is on some other control.
    window.addEventListener('keydown', handleKeyDown, true);
    return () => window.removeEventListener('keydown', handleKeyDown, true);
  }, [recording, onCapture, stopRecording]);

  return (
    <button
      type="button"
      id={id}
      onClick={() => setRecording(true)}
      onBlur={stopRecording}
      className={`w-full h-9 rounded-lg border px-3 text-left text-xs font-mono transition-colors cursor-pointer ${
        recording
          ? 'border-primary ring-1 ring-primary/40 bg-primary/10 text-primary animate-pulse'
          : 'border-border bg-background text-foreground hover:border-primary/50'
      }`}
    >
      {recording ? 'Press a key combination… (Esc to cancel)' : value || 'Click to set hotkey'}
    </button>
  );
};
