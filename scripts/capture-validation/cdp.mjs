/**
 * A minimal Chrome DevTools Protocol driver that serves fixtures by
 * intercepting requests.
 *
 * Zero dependencies, on purpose. Validating capture against a real browser is
 * worth doing; adding a browser-automation toolchain to a Tauri + Vitest repo
 * to do it is not, and `docs/capture.md` said as much before this existed. Node
 * 22 ships a global `WebSocket`, which is the only thing a CDP client actually
 * needs.
 *
 * ## Why interception rather than a local server
 *
 * The fixtures have to be served *as* `claude.ai` and `chatgpt.com`, because
 * the extractor registry selects on hostname: served from `127.0.0.1`, the
 * validation would only ever exercise the generic path and would prove nothing
 * about the site extractors.
 *
 * The obvious route — a local HTTP server plus `--host-resolver-rules` — does
 * not work, and the reason is worth writing down: both hosts are in Chrome's
 * *preloaded HSTS list*, so `http://claude.ai/…` is upgraded to HTTPS before
 * any resolver rule applies, and the request then fails TLS against a plain
 * HTTP server. Measured here: the same fixture loaded fine from
 * `http://127.0.0.1:<port>` and landed on `chrome-error://chromewebdata/` from
 * `http://claude.ai`.
 *
 * `Fetch.fulfillRequest` sidesteps all of it. The page is served over
 * `https://claude.ai/…` with the bytes coming straight off disk: no
 * certificate, no resolver rules, and no network involved.
 */

import { spawn } from 'node:child_process';
import { readFile } from 'node:fs/promises';
import { mkdtempSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { extname, join, resolve } from 'node:path';

const CHROME_CANDIDATES = [
  process.env.RELAY_CHROME,
  '/opt/pw-browsers/chromium-1194/chrome-linux/chrome',
  '/usr/bin/chromium',
  '/usr/bin/chromium-browser',
  '/usr/bin/google-chrome',
  '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome',
].filter(Boolean);

const MIME = {
  '.html': 'text/html; charset=utf-8',
  '.js': 'text/javascript; charset=utf-8',
  '.css': 'text/css; charset=utf-8',
  '.svg': 'image/svg+xml',
  '.png': 'image/png',
};

/** Launches headless Chromium and attaches to it over CDP. */
export async function launch({ fixtureRoot, width = 1280, height = 900 } = {}) {
  const chrome = CHROME_CANDIDATES.find(Boolean);
  const userDataDir = mkdtempSync(join(tmpdir(), 'relay-capture-validation-'));

  const proc = spawn(
    chrome,
    [
      '--headless=new',
      '--remote-debugging-port=0',
      '--no-sandbox',
      '--disable-gpu',
      '--disable-dev-shm-usage',
      '--hide-scrollbars',
      // Nothing here should ever reach the network: every request is either
      // fulfilled from a fixture or failed.
      '--no-proxy-server',
      `--window-size=${width},${height}`,
      `--user-data-dir=${userDataDir}`,
      'about:blank',
    ],
    { stdio: ['ignore', 'pipe', 'pipe'] },
  );

  const wsUrl = await new Promise((done, fail) => {
    let buffered = '';
    const timer = setTimeout(
      () => fail(new Error('Chromium never reported a debugging URL')),
      30_000,
    );
    proc.stderr.on('data', (chunk) => {
      buffered += chunk.toString();
      const match = /ws:\/\/[^\s]+/.exec(buffered);
      if (match) {
        clearTimeout(timer);
        done(match[0]);
      }
    });
    proc.on('exit', (code) => {
      clearTimeout(timer);
      fail(
        new Error(
          `Chromium exited with ${code}. Set RELAY_CHROME to a browser binary.\n${buffered}`,
        ),
      );
    });
  });

  const ws = new WebSocket(wsUrl);
  await new Promise((done, fail) => {
    ws.onopen = done;
    ws.onerror = () => fail(new Error('could not connect to Chromium'));
  });

  let nextId = 0;
  const pending = new Map();
  const listeners = new Set();

  ws.onmessage = (event) => {
    const message = JSON.parse(event.data);
    if (message.id) {
      const waiter = pending.get(message.id);
      if (!waiter) return;
      pending.delete(message.id);
      if (message.error) waiter.fail(new Error(JSON.stringify(message.error)));
      else waiter.done(message.result);
      return;
    }
    for (const listener of listeners) listener(message);
  };

  const send = (method, params = {}, sessionId) =>
    new Promise((done, fail) => {
      const id = ++nextId;
      pending.set(id, { done, fail });
      ws.send(JSON.stringify({ id, method, params, sessionId }));
    });

  /**
   * Serves one fixture directory for every request a page makes.
   *
   * The path is ignored in favour of an explicit `?page=` parameter, so a
   * fixture can claim whatever URL shape the real site uses (`/c/2b1f0e3a`,
   * `/chat/abc`) while still being addressed by filename. Sub-resources
   * resolve by their own path.
   */
  function serveFixtures(sessionId) {
    listeners.add(async (message) => {
      if (message.method !== 'Fetch.requestPaused' || message.sessionId !== sessionId) return;
      const { requestId, request } = message.params;

      let file;
      try {
        const url = new URL(request.url);
        const name = url.searchParams.get('page') ?? url.pathname.replace(/^\/+/, '');
        file = resolve(fixtureRoot, name || 'index.html');
        if (!file.startsWith(resolve(fixtureRoot))) throw new Error('outside the fixtures');
      } catch {
        await send('Fetch.failRequest', { requestId, errorReason: 'Aborted' }, sessionId).catch(
          () => {},
        );
        return;
      }

      try {
        const body = await readFile(file);
        await send(
          'Fetch.fulfillRequest',
          {
            requestId,
            responseCode: 200,
            responseHeaders: [
              { name: 'content-type', value: MIME[extname(file)] ?? 'application/octet-stream' },
            ],
            body: body.toString('base64'),
          },
          sessionId,
        );
      } catch {
        await send('Fetch.failRequest', { requestId, errorReason: 'Failed' }, sessionId).catch(
          () => {},
        );
      }
    });
  }

  return {
    async open(url) {
      const { targetId } = await send('Target.createTarget', { url: 'about:blank' });
      const { sessionId } = await send('Target.attachToTarget', { targetId, flatten: true });
      await send('Runtime.enable', {}, sessionId);
      await send('Page.enable', {}, sessionId);
      await send('Fetch.enable', { patterns: [{ urlPattern: '*' }] }, sessionId);
      serveFixtures(sessionId);

      await send('Page.navigate', { url }, sessionId);
      await new Promise((done, fail) => {
        const started = Date.now();
        const check = setInterval(async () => {
          const state = await send(
            'Runtime.evaluate',
            { expression: 'document.readyState', returnByValue: true },
            sessionId,
          ).catch(() => null);
          if (state?.result?.value === 'complete') {
            clearInterval(check);
            done();
          } else if (Date.now() - started > 20_000) {
            clearInterval(check);
            fail(new Error(`${url} never finished loading`));
          }
        }, 50);
      });

      return {
        async eval(expression) {
          const result = await send(
            'Runtime.evaluate',
            { expression, returnByValue: true, awaitPromise: true },
            sessionId,
          );
          if (result.exceptionDetails) {
            throw new Error(
              result.exceptionDetails.exception?.description ??
                JSON.stringify(result.exceptionDetails),
            );
          }
          return result.result.value;
        },
        close: () => send('Target.closeTarget', { targetId }),
      };
    },
    close() {
      try {
        ws.close();
      } catch {
        // The socket is going away either way.
      }
      proc.kill('SIGKILL');
      try {
        rmSync(userDataDir, { recursive: true, force: true });
      } catch {
        // A leftover profile directory in the temp dir is not worth failing on.
      }
    },
  };
}
