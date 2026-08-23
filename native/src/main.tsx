import React from 'react';
import ReactDOM from 'react-dom/client';
import { App } from './App';
import { FloatingPill } from './components/capture/FloatingPill';
import { MeetingReminderWindow } from './components/meetings/MeetingReminderWindow';
import './index.css';

// The floating dictation pill is a second Tauri window that loads this
// same bundle at `index.html#/dictation-pill` (see overlay::ensure_pill_window
// in the Rust backend) rather than a separate Vite entry point. It's the
// only PTT visual surface — there's no separate "listening" indicator.
const route = window.location.hash;
const isOverlayWindow = route === '#/dictation-pill' || route === '#/meeting-reminder';

// This window is created with Tauri's `transparent: true`, but the page's
// own default `bg-background` (opaque, near-black in dark mode) otherwise
// paints right over that and shows up as a solid rectangle instead of
// floating on the desktop — see the `.overlay-window` rule in index.css
// this class activates.
if (isOverlayWindow) {
  document.documentElement.classList.add('overlay-window');
  document.body.classList.add('overlay-window');
}

const view = route === '#/dictation-pill' ? <FloatingPill /> : route === '#/meeting-reminder' ? <MeetingReminderWindow /> : <App />;

ReactDOM.createRoot(document.getElementById('root') as HTMLElement).render(
  <React.StrictMode>{view}</React.StrictMode>
);
