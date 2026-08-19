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
