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

interface RouteEntry {
  component: React.ReactElement;
  isOverlay: boolean;
}

const ROUTE_MAP: Record<string, RouteEntry> = {
  'dictation-pill': {
    component: <FloatingPill />,
    isOverlay: true,
  },
  'meeting-reminder': {
    component: <MeetingReminderWindow />,
    isOverlay: true,
  },
};

function resolveActiveRoute(): RouteEntry {
  for (const [key, entry] of Object.entries(ROUTE_MAP)) {
    if (windowLabel === key || hash.includes(key) || href.includes(key)) {
      return entry;
    }
  }
  return {
    component: <App />,
    isOverlay: false,
  };
}

const activeRoute = resolveActiveRoute();

if (activeRoute.isOverlay) {
  document.documentElement.classList.add('overlay-window');
  document.body.classList.add('overlay-window');
}

ReactDOM.createRoot(document.getElementById('root') as HTMLElement).render(
  <React.StrictMode>{activeRoute.component}</React.StrictMode>
);
