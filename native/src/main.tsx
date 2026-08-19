import React from 'react';
import ReactDOM from 'react-dom/client';
import { App } from './App';
import { DictationIndicator } from './components/dictation/DictationIndicator';
import './index.css';

// The dictation "listening" indicator is a second Tauri window that loads
// this same bundle at `index.html#/dictation-indicator` (see
// hotkeys::ensure_indicator_window in the Rust backend) rather than a
// separate Vite entry point.
const isDictationIndicator = window.location.hash === '#/dictation-indicator';

ReactDOM.createRoot(document.getElementById('root') as HTMLElement).render(
  <React.StrictMode>
    {isDictationIndicator ? <DictationIndicator /> : <App />}
  </React.StrictMode>
);
