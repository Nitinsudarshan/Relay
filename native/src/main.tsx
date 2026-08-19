import React from 'react';
import ReactDOM from 'react-dom/client';
import { App } from './App';
import { DictationIndicator } from './components/dictation/DictationIndicator';
import { FloatingPill } from './components/capture/FloatingPill';
import './index.css';

// The dictation "listening" indicator and the floating dictation pill are
// each a second/third Tauri window that load this same bundle at a
// different hash route (see overlay::ensure_pill_window and
// hotkeys::ensure_indicator_window in the Rust backend) rather than
// separate Vite entry points.
const route = window.location.hash;
const isOverlayWindow = route === '#/dictation-indicator' || route === '#/dictation-pill';

// These windows are created with Tauri's `transparent: true`, but the
// page's own default `bg-background` (opaque, near-black in dark mode)
// otherwise paints right over that and shows up as a solid rectangle
// instead of floating on the desktop — see the `.overlay-window` rule in
// index.css this class activates.
if (isOverlayWindow) {
  document.documentElement.classList.add('overlay-window');
  document.body.classList.add('overlay-window');
}

const view =
  route === '#/dictation-indicator' ? (
    <DictationIndicator />
  ) : route === '#/dictation-pill' ? (
    <FloatingPill />
  ) : (
    <App />
  );

ReactDOM.createRoot(document.getElementById('root') as HTMLElement).render(
  <React.StrictMode>{view}</React.StrictMode>
);
