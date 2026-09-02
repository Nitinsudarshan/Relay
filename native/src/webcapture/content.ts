/**
 * The injected content script.
 *
 * Injected on demand under `activeTab` — never declared in the manifest, so
 * the extension has no standing access to any page. It runs in the isolated
 * world (the default), which means it can read the rendered DOM but cannot
 * see or be seen by the page's own JavaScript.
 *
 * It does one thing and returns: read the document, build a payload, hand it
 * back to the service worker. It never talks to the network, and it never
 * writes to the page.
 */

import { CaptureEmptyError, buildPayload } from './capture';
import type { CapturePayload } from './types';

export interface ContentCaptureResult {
  ok: boolean;
  payload?: CapturePayload;
  error?: string;
}

/** Runs one capture against the current document. */
export function captureCurrentDocument(): ContentCaptureResult {
  try {
    const payload = buildPayload(document, window.location.href, {
      browser: navigator.userAgent,
      startedAt: Date.now(),
    });
    return { ok: true, payload };
  } catch (error) {
    if (error instanceof CaptureEmptyError) {
      return { ok: false, error: error.message };
    }
    return {
      ok: false,
      error: error instanceof Error ? error.message : 'Relay could not read this page.',
    };
  }
}

/**
 * The name the service worker calls after injecting this bundle.
 *
 * Injecting a file and reading its "result" is unreliable — a bundled script
 * has no completion value to speak of — so the bundle publishes one function
 * and the worker invokes it with a second, argument-free `executeScript`
 * under the same `activeTab` grant.
 */
export const CONTENT_ENTRY_POINT = '__relayCaptureRun';

(globalThis as unknown as Record<string, unknown>)[CONTENT_ENTRY_POINT] =
  captureCurrentDocument;
