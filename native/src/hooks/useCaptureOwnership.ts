import { useState, useEffect } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';

export interface CaptureOwnership {
  active: boolean;
  mode: string | null;
  ownedByMeeting: boolean;
  ownedByOther: boolean;
}

export function useCaptureOwnership() {
  const [ownership, setOwnership] = useState<CaptureOwnership>({
    active: false,
    mode: null,
    ownedByMeeting: false,
    ownedByOther: false,
  });

  useEffect(() => {
    const updateOwnership = (active: boolean, mode: string | null) => {
      setOwnership({
        active,
        mode,
        ownedByMeeting: active && mode === 'meeting',
        ownedByOther: active && mode !== 'meeting',
      });
    };

    // Initial fetch
    invoke<{ active: boolean; mode: string | null }>('get_capture_status')
      .then((status) => {
        updateOwnership(status.active, status.mode);
      })
      .catch(console.error);

    // Subscribe to changes
    const unlisten = listen<{ active: boolean; mode: string | null }>('capture-state-changed', (event) => {
      updateOwnership(event.payload.active, event.payload.mode);
    });

    return () => {
      unlisten.then((fn) => fn());
    };
  }, []);

  return ownership;
}
