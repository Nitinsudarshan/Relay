/**
 * The extension's pairing screen.
 *
 * Two fields — the port Relay is listening on and the pairing token it shows
 * in Settings → Capture. They are stored in the extension's own local
 * storage, never synced to a browser account, and the token is only ever sent
 * to `127.0.0.1`.
 */

const DEFAULT_PORT = 8765;

function el<T extends HTMLElement>(id: string): T {
  const found = document.getElementById(id);
  if (!found) throw new Error(`Missing element: ${id}`);
  return found as T;
}

/** Reports whether Relay answers on this port with this token. */
export async function testConnection(
  port: number,
  token: string,
): Promise<{ ok: boolean; message: string }> {
  try {
    const response = await fetch(`http://127.0.0.1:${port}/v1/health`, {
      headers: { 'x-relay-token': token },
    });
    if (response.status === 401) {
      return { ok: false, message: 'Relay is running, but the pairing token is wrong.' };
    }
    if (!response.ok) {
      return { ok: false, message: `Relay answered with ${response.status}.` };
    }
    const body = (await response.json()) as { protocol_version?: number; version?: string };
    return {
      ok: true,
      message: `Connected to Relay ${body.version ?? ''} (capture protocol v${
        body.protocol_version ?? '?'
      }).`,
    };
  } catch {
    return {
      ok: false,
      message: 'No answer on that port. Is Relay running with capture switched on?',
    };
  }
}

async function main(): Promise<void> {
  const portInput = el<HTMLInputElement>('port');
  const tokenInput = el<HTMLInputElement>('token');
  const status = el<HTMLParagraphElement>('status');

  const stored = await chrome.storage.local.get(['relayPort', 'relayToken']);
  portInput.value = String(stored.relayPort ?? DEFAULT_PORT);
  tokenInput.value = typeof stored.relayToken === 'string' ? stored.relayToken : '';

  el<HTMLFormElement>('pairing').addEventListener('submit', (event) => {
    event.preventDefault();
    void (async () => {
      const port = Number(portInput.value) || DEFAULT_PORT;
      const token = tokenInput.value.trim();
      await chrome.storage.local.set({ relayPort: port, relayToken: token });
      status.textContent = 'Checking…';
      const result = await testConnection(port, token);
      status.textContent = result.message;
      status.dataset.state = result.ok ? 'ok' : 'error';
    })();
  });
}

void main();
