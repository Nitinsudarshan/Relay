/**
 * The Relay extension's service worker.
 *
 * It holds the only two things the content script must not: the pairing
 * token, and permission to talk to Relay. The page is read in the tab; the
 * result is posted from here, to `127.0.0.1` and nowhere else.
 *
 * Nothing here runs until the user asks for a capture. The extension declares
 * no content scripts, no host permissions for websites, and no background
 * polling — a browser that never captures anything never gives Relay a single
 * byte of page content.
 */

import type { CapturePayload } from './types';
import type { ContentCaptureResult } from './content';

const CONTENT_BUNDLE = 'relay-extract.js';
const DEFAULT_PORT = 8765;
const REQUEST_TIMEOUT_MS = 20_000;
/**
 * Ceiling on the in-page reveal pass.
 *
 * The engine has its own wall-clock budget per source (10s by default), so
 * this is the outer guard for the case where the page never returns at all —
 * generous enough not to cut a legitimate long-conversation read short.
 */
const EXTRACT_TIMEOUT_MS = 30_000;

interface RelaySettings {
  port: number;
  token: string;
}

/** Pages no extension may script, and where a capture attempt only confuses. */
const BLOCKED_SCHEMES = /^(chrome|edge|about|moz-extension|chrome-extension|view-source|file|data):/i;

async function readSettings(): Promise<RelaySettings | null> {
  const stored = await chrome.storage.local.get(['relayPort', 'relayToken']);
  const token = typeof stored.relayToken === 'string' ? stored.relayToken.trim() : '';
  if (!token) return null;
  const port = typeof stored.relayPort === 'number' ? stored.relayPort : DEFAULT_PORT;
  return { port, token };
}

async function flash(tabId: number | undefined, text: string, color: string, title: string) {
  await chrome.action.setBadgeBackgroundColor({ color, tabId });
  await chrome.action.setBadgeText({ text, tabId });
  await chrome.action.setTitle({ title, tabId });
}

async function clearBadgeLater(tabId: number | undefined) {
  setTimeout(() => {
    void chrome.action.setBadgeText({ text: '', tabId });
    void chrome.action.setTitle({ title: 'Capture this page in Relay', tabId });
  }, 4000);
}

/**
 * Posts a payload to Relay.
 *
 * The token travels in a header rather than the URL so it never lands in a
 * log or a history entry, and the request is aborted rather than left hanging
 * if Relay is slow — a capture that cannot be delivered should say so, not
 * spin.
 */
async function postToRelay(
  payload: CapturePayload,
  settings: RelaySettings,
): Promise<{ ok: boolean; message: string }> {
  const controller = new AbortController();
  const timeout = setTimeout(() => controller.abort(), REQUEST_TIMEOUT_MS);

  try {
    const response = await fetch(`http://127.0.0.1:${settings.port}/v1/capture`, {
      method: 'POST',
      headers: {
        'content-type': 'application/json',
        'x-relay-token': settings.token,
      },
      body: JSON.stringify(payload),
      signal: controller.signal,
    });

    const body = (await response.json().catch(() => ({}))) as {
      message?: string;
      title?: string;
      code?: string;
    };

    if (!response.ok) {
      return {
        ok: false,
        message: body.message ?? `Relay refused the capture (${response.status}).`,
      };
    }
    return { ok: true, message: body.title ?? 'Saved to Relay' };
  } catch (error) {
    if (error instanceof Error && error.name === 'AbortError') {
      return { ok: false, message: 'Relay did not respond in time.' };
    }
    return {
      ok: false,
      message:
        'Relay is not reachable. Make sure Relay is running and that capture is switched on in its settings.',
    };
  } finally {
    clearTimeout(timeout);
  }
}

/**
 * The whole capture flow for one tab.
 *
 * Only ever called from a user gesture — the toolbar button or the keyboard
 * command — because those are the only things that grant `activeTab`, which
 * is the only permission this extension has over the page.
 */
export async function captureTab(tab: chrome.tabs.Tab | undefined): Promise<void> {
  const tabId = tab?.id;
  if (tabId === undefined) return;

  if (tab?.url && BLOCKED_SCHEMES.test(tab.url)) {
    await flash(tabId, '—', '#6b7280', 'Relay cannot capture browser pages.');
    await clearBadgeLater(tabId);
    return;
  }

  const settings = await readSettings();
  if (!settings) {
    await flash(tabId, '!', '#f59e0b', 'Pair this extension with Relay first (click to open options).');
    await clearBadgeLater(tabId);
    return;
  }

  await flash(tabId, '…', '#3b82f6', 'Capturing…');

  try {
    await chrome.scripting.executeScript({ target: { tabId }, files: [CONTENT_BUNDLE] });
    // A literal arrow, not a constructed function: Manifest V3's content
    // security policy forbids `eval`/`new Function` in a service worker, and
    // the function is serialized by `toString()` before it is evaluated in
    // the page, so it cannot close over anything from this module either.
    // Chrome awaits a promise returned from an injected function, which is how
    // an asynchronous reveal pass reports back without the worker polling.
    const injected = chrome.scripting.executeScript<Promise<ContentCaptureResult>>({
      target: { tabId },
      func: () =>
        (globalThis as unknown as Record<string, () => Promise<ContentCaptureResult>>)[
          '__relayCaptureRun'
        ]?.(),
    });

    const [injection] = await Promise.race([
      injected,
      new Promise<never>((_, reject) =>
        setTimeout(() => reject(new Error('Reading this page took too long.')), EXTRACT_TIMEOUT_MS),
      ),
    ]);

    const result = (await injection?.result) as ContentCaptureResult | undefined;
    if (!result?.ok || !result.payload) {
      await flash(tabId, '✕', '#ef4444', result?.error ?? 'Nothing readable on this page.');
      await clearBadgeLater(tabId);
      return;
    }

    await flash(tabId, '↑', '#3b82f6', 'Saving to Relay…');
    const outcome = await postToRelay(result.payload, settings);
    await flash(
      tabId,
      outcome.ok ? '✓' : '✕',
      outcome.ok ? '#10b981' : '#ef4444',
      outcome.message,
    );
    await clearBadgeLater(tabId);
  } catch (error) {
    // The usual cause is a page extensions may not script (the Chrome Web
    // Store, a PDF viewer). Say that, rather than failing silently.
    await flash(
      tabId,
      '✕',
      '#ef4444',
      error instanceof Error ? error.message : 'Relay could not read this page.',
    );
    await clearBadgeLater(tabId);
  }
}

chrome.action.onClicked.addListener((tab) => {
  void captureTab(tab);
});

chrome.commands.onCommand.addListener((command, tab) => {
  if (command !== 'capture-page') return;
  void (async () => {
    const target = tab ?? (await chrome.tabs.query({ active: true, currentWindow: true }))[0];
    await captureTab(target);
  })();
});
