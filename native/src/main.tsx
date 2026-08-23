import React from 'react';
import ReactDOM from 'react-dom/client';
import { getCurrentWindow } from '@tauri-apps/api/window';
import { App } from './App';
import { FloatingPill } from './components/capture/FloatingPill';
import { MeetingReminderWindow } from './components/meetings/MeetingReminderWindow';
import './index.css';

let windowLabel = '';
try {
  windowLabel = getCurrentWindow().label;
} catch (e) {
  // browser fallback
}

const hash = window.location.hash || '';
const href = window.location.href || '';

const isReminderWindow =
  windowLabel === 'meeting-reminder' ||
  hash.includes('meeting-reminder') ||
  href.includes('meeting-reminder');

const isPillWindow =
  windowLabel === 'dictation-pill' ||
  hash.includes('dictation-pill') ||
  href.includes('dictation-pill');

const isOverlayWindow = isReminderWindow || isPillWindow;

if (isOverlayWindow) {
  document.documentElement.classList.add('overlay-window');
  document.body.classList.add('overlay-window');
}

const view = isPillWindow ? (
  <FloatingPill />
) : isReminderWindow ? (
  <MeetingReminderWindow />
) : (
  <App />
);

ReactDOM.createRoot(document.getElementById('root') as HTMLElement).render(
  <React.StrictMode>{view}</React.StrictMode>
);
