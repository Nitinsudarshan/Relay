/**
 * The injected content script.
 *
 * Injected on demand under `activeTab` — never declared in the manifest, so
 * the extension has no standing access to any page. It runs in the isolated
 * world (the default), which means it can read the rendered DOM but cannot
 * see or be seen by the page's own JavaScript. Nothing captured is ever
 * evaluated, in either direction.
 *
 * It reads the document, reveals what the browser will legitimately reveal,
 * builds a payload and hands it back to the service worker. It never talks to
 * the network, and the only writes it makes to the page are the two the reveal
 * engine is allowed: moving the scroll position (restored afterwards) and
 * activating controls whose sole purpose is to disclose content.
 */

import { CaptureEmptyError, buildPayload } from './capture';
import type { CapturePayload } from './types';

export interface ContentCaptureResult {
  ok: boolean;
  payload?: CapturePayload;
  error?: string;
}

/** Runs one capture against the current document. */
export async function captureCurrentDocument(): Promise<ContentCaptureResult> {
  try {
    const payload = await buildPayload(document, window.location.href, {
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
 * under the same `activeTab` grant. Chrome awaits a promise returned from an
 * injected function, which is what lets the reveal pass be asynchronous
 * without the worker polling for a result.
 */
export const CONTENT_ENTRY_POINT = '__relayCaptureRun';

(globalThis as unknown as Record<string, unknown>)[CONTENT_ENTRY_POINT] =
  captureCurrentDocument;
