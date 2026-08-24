import React from 'react';
import ReactDOM from 'react-dom/client';
import { getCurrentWindow } from '@tauri-apps/api/window';
import { App } from './App';
import { FloatingPill } from './components/capture/FloatingPill';
import './index.css';

let windowLabel = '';
try {
  windowLabel = getCurrentWindow().label;
} catch (e) {
  // browser fallback
}

const hash = window.location.hash || '';
const href = window.location.href || '';

const isPillWindow =
  windowLabel === 'dictation-pill' ||
  hash.includes('dictation-pill') ||
  href.includes('dictation-pill');

if (isPillWindow) {
  document.documentElement.classList.add('overlay-window');
  document.body.classList.add('overlay-window');
}

const view = isPillWindow ? <FloatingPill /> : <App />;

ReactDOM.createRoot(document.getElementById('root') as HTMLElement).render(
  <React.StrictMode>{view}</React.StrictMode>
);
