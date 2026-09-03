/**
 * The contract between the browser extension and the Rust backend.
 *
 * The payloads committed under `src-tauri/src/capture/web/fixtures/` are
 * generated here and consumed by Rust tests in `capture::web`. Both sides
 * assert against the same bytes, so a change to either model that the other
 * has not followed fails a test instead of failing a user's capture.
 *
 * Regenerate with `RELAY_UPDATE_CAPTURE_FIXTURES=1 npm test`.
 */

import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { describe, expect, it } from 'vitest';
import { buildPayload } from './capture';
import type { CapturePayload } from './types';

const here = path.dirname(fileURLToPath(import.meta.url));
const HTML_DIR = path.join(here, 'fixtures');
const JSON_DIR = path.resolve(here, '../../src-tauri/src/capture/web/fixtures');

/** Fixed stand-ins for the fields that change on every run. */
function stabilize(payload: CapturePayload): CapturePayload {
  return {
    ...payload,
    captured_at: '2026-02-14T09:30:00.000Z',
    browser: 'Relay contract fixture',
    diagnostics: { ...payload.diagnostics, elapsed_ms: 0 },
  };
}

const CASES = [
  { name: 'chatgpt-conversation', url: 'https://chatgpt.com/c/2b1f0e3a' },
  { name: 'article', url: 'https://example.com/posts/structured-capture' },
];

describe('extension → Relay payload contract', () => {
  for (const testCase of CASES) {
    it(`${testCase.name} matches the fixture the Rust backend is tested against`, () => {
      const html = fs.readFileSync(path.join(HTML_DIR, `${testCase.name}.html`), 'utf8');
      const doc = new DOMParser().parseFromString(html, 'text/html');
      const payload = stabilize(buildPayload(doc, testCase.url));
      const serialized = `${JSON.stringify(payload, null, 2)}\n`;
      const fixturePath = path.join(JSON_DIR, `${testCase.name}.json`);

      if (process.env.RELAY_UPDATE_CAPTURE_FIXTURES === '1') {
        fs.mkdirSync(JSON_DIR, { recursive: true });
        fs.writeFileSync(fixturePath, serialized);
      }

      expect(fs.existsSync(fixturePath)).toBe(true);
      const expected = fs.readFileSync(fixturePath, 'utf8').replace(/\r\n/g, '\n');
      expect(serialized).toBe(expected);
    });
  }
});
