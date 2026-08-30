import '@testing-library/jest-dom/vitest';
import { cleanup } from '@testing-library/react';
import { afterEach, vi } from 'vitest';

/**
 * Global test setup for the native frontend.
 *
 * Everything Tauri-shaped is stubbed here rather than per-file: these modules
 * only resolve inside a Tauri webview, so importing any component that talks
 * to the backend fails at module load in jsdom. Individual tests override the
 * mock's behaviour with `vi.mocked(invoke).mockResolvedValue(...)`.
 */

vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(async () => undefined),
  convertFileSrc: vi.fn((p: string) => p),
}));

vi.mock('@tauri-apps/api/event', () => ({
  // Listeners resolve to a no-op unlisten so component cleanup works.
  listen: vi.fn(async () => () => {}),
  emit: vi.fn(async () => undefined),
}));

vi.mock('@tauri-apps/api/window', () => ({
  getCurrentWindow: vi.fn(() => ({
    setSize: vi.fn(async () => undefined),
    setPosition: vi.fn(async () => undefined),
    show: vi.fn(async () => undefined),
    hide: vi.fn(async () => undefined),
  })),
}));

// jsdom implements neither of these, and components that scroll or observe
// resize would otherwise throw rather than fail on the thing being tested.
Element.prototype.scrollIntoView = vi.fn();

globalThis.ResizeObserver = class {
  observe() {}
  unobserve() {}
  disconnect() {}
} as unknown as typeof ResizeObserver;

afterEach(() => {
  cleanup();
  vi.clearAllMocks();
});
