import React, { useCallback, useEffect, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import {
  AlertTriangle,
  Check,
  CheckCircle2,
  Copy,
  Globe,
  Keyboard,
  RefreshCw,
  ShieldCheck,
  Sparkles,
} from 'lucide-react';
import type { CaptureBridgeStatus } from '../../types';
import { Switch } from '@/components/ui/switch';

/**
 * Capture settings: the one place the user turns browser capture on and pairs
 * a browser with it.
 *
 * The pairing token is shown here in full, because pairing is a copy-paste
 * step the user performs deliberately. Nothing on this screen explains DOM
 * extraction, extractors, or IPC — what a user needs to know is that capture
 * is on, which browser is paired, and that nothing leaves the machine.
 */
export const CaptureSettingsView: React.FC = () => {
  const [status, setStatus] = useState<CaptureBridgeStatus | null>(null);
  const [busy, setBusy] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [copied, setCopied] = useState<'token' | 'port' | null>(null);
  const [portDraft, setPortDraft] = useState('');

  const refresh = useCallback(async () => {
    try {
      const next = await invoke<CaptureBridgeStatus>('get_capture_bridge_status');
      setStatus(next);
      setPortDraft(String(next.configured_port));
    } catch (err) {
      console.error('Failed to read capture bridge status', err);
      setError('Relay could not read your capture settings.');
    }
  }, []);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  const run = async <T,>(name: string, action: () => Promise<T>) => {
    setBusy(name);
    setError(null);
    try {
      const next = (await action()) as CaptureBridgeStatus;
      setStatus(next);
      setPortDraft(String(next.configured_port));
    } catch (err) {
      const message =
        typeof err === 'object' && err !== null && 'message' in err
          ? String((err as { message: unknown }).message)
          : 'That did not work. Please try again.';
      setError(message);
      await refresh();
    } finally {
      setBusy(null);
    }
  };

  const copy = async (value: string, which: 'token' | 'port') => {
    try {
      await navigator.clipboard.writeText(value);
      setCopied(which);
      setTimeout(() => setCopied(null), 2000);
    } catch (err) {
      console.error('Could not copy to the clipboard', err);
    }
  };

  if (!status) {
    return <p className="text-xs text-muted-foreground">Loading capture settings…</p>;
  }

  return (
    <div className="space-y-6">
      <header>
        <h2 className="flex items-center gap-2 text-sm font-semibold text-foreground">
          <Globe className="h-4 w-4 text-primary" /> Capture
        </h2>
        <p className="mt-1 text-xs text-muted-foreground">
          Save the page or conversation you are looking at into your Vault, with its source, its
          structure, and a record of how much of it Relay could read.
        </p>
      </header>

      {error && (
        <p className="flex items-start gap-2 rounded-lg border border-red-500/40 bg-red-500/10 p-3 text-xs text-red-600 dark:text-red-400">
          <AlertTriangle className="mt-px h-3.5 w-3.5 shrink-0" />
          {error}
        </p>
      )}

      <section className="rounded-lg border border-border p-4">
        <div className="flex items-start justify-between gap-4">
          <div>
            <h3 className="text-xs font-semibold text-foreground">Browser capture</h3>
            <p className="mt-1 text-xs text-muted-foreground">
              Lets the Relay browser extension send captures to this computer. Relay listens only
              on <code className="rounded bg-muted px-1">127.0.0.1</code> — never on your network —
              and only accepts captures signed with the pairing token below.
            </p>
          </div>
          <Switch
            checked={status.enabled}
            disabled={busy !== null}
            aria-label="Browser capture"
            onCheckedChange={(checked) =>
              void run('toggle', () =>
                invoke<CaptureBridgeStatus>('set_capture_bridge_enabled', { enabled: checked }),
              )
            }
          />
        </div>

        <p className="mt-3 flex items-center gap-1.5 text-xs">
          {status.running ? (
            <>
              <CheckCircle2 className="h-3.5 w-3.5 text-emerald-500" />
              <span className="text-emerald-600 dark:text-emerald-400">
                Listening on 127.0.0.1:{status.port}
              </span>
            </>
          ) : (
            <>
              <AlertTriangle className="h-3.5 w-3.5 text-muted-foreground" />
              <span className="text-muted-foreground">Not listening</span>
            </>
          )}
        </p>

        {status.running && status.port !== status.configured_port && (
          <p className="mt-2 text-xs text-amber-600 dark:text-amber-400">
            Port {status.configured_port} was already in use, so Relay is on {status.port}. Use{' '}
            {status.port} when pairing.
          </p>
        )}
      </section>

      {status.enabled && (
        <section className="rounded-lg border border-border p-4">
          <h3 className="flex items-center gap-1.5 text-xs font-semibold text-foreground">
            <ShieldCheck className="h-3.5 w-3.5 text-primary" /> Pair a browser
          </h3>
          <ol className="mt-2 list-decimal space-y-1 pl-4 text-xs text-muted-foreground">
            <li>
              Install the Relay extension — <strong>Load unpacked</strong> from{' '}
              <code className="rounded bg-muted px-1">native/browser-extension</code>.
            </li>
            <li>Open the extension&apos;s Options.</li>
            <li>Paste the port and token below, then choose <strong>Save and test</strong>.</li>
          </ol>

          <div className="mt-3 space-y-2">
            <label className="block text-[11px] font-medium text-muted-foreground" htmlFor="capture-port">
              Port
            </label>
            <div className="flex gap-2">
              <input
                id="capture-port"
                type="number"
                min={1024}
                max={65535}
                value={portDraft}
                onChange={(event) => setPortDraft(event.target.value)}
                className="w-28 rounded-md border border-border bg-background px-2 py-1.5 font-mono text-xs text-foreground focus:outline-none focus:ring-2 focus:ring-ring"
              />
              <button
                type="button"
                disabled={busy !== null || Number(portDraft) === status.configured_port}
                onClick={() =>
                  void run('port', () =>
                    invoke<CaptureBridgeStatus>('set_capture_bridge_port', {
                      port: Number(portDraft),
                    }),
                  )
                }
                className="rounded-md border border-border px-2.5 py-1.5 text-xs font-medium text-foreground transition-colors hover:bg-muted disabled:opacity-50"
              >
                Use this port
              </button>
              <button
                type="button"
                onClick={() => void copy(String(status.port), 'port')}
                className="rounded-md p-1.5 text-muted-foreground transition-colors hover:bg-muted hover:text-foreground"
                aria-label="Copy the port"
              >
                {copied === 'port' ? (
                  <Check className="h-3.5 w-3.5 text-emerald-500" />
                ) : (
                  <Copy className="h-3.5 w-3.5" />
                )}
              </button>
            </div>

            <label className="block pt-2 text-[11px] font-medium text-muted-foreground" htmlFor="capture-token">
              Pairing token
            </label>
            <div className="flex gap-2">
              <input
                id="capture-token"
                readOnly
                value={status.pairing_token ?? ''}
                className="flex-1 rounded-md border border-border bg-muted/40 px-2 py-1.5 font-mono text-xs text-foreground"
              />
              <button
                type="button"
                onClick={() => void copy(status.pairing_token ?? '', 'token')}
                className="rounded-md p-1.5 text-muted-foreground transition-colors hover:bg-muted hover:text-foreground"
                aria-label="Copy the pairing token"
              >
                {copied === 'token' ? (
                  <Check className="h-3.5 w-3.5 text-emerald-500" />
                ) : (
                  <Copy className="h-3.5 w-3.5" />
                )}
              </button>
              <button
                type="button"
                disabled={busy !== null}
                onClick={() =>
                  void run('token', () =>
                    invoke<CaptureBridgeStatus>('regenerate_capture_pairing_token'),
                  )
                }
                className="inline-flex items-center gap-1.5 rounded-md border border-border px-2.5 py-1.5 text-xs font-medium text-foreground transition-colors hover:bg-muted disabled:opacity-50"
              >
                {busy === 'token' ? (
                  <RefreshCw className="h-3.5 w-3.5 animate-spin" />
                ) : (
                  <RefreshCw className="h-3.5 w-3.5" />
                )}
                New token
              </button>
            </div>
            <p className="text-[11px] text-muted-foreground">
              A new token unpairs every browser immediately — you will need to paste the new one in
              again.
            </p>
          </div>
        </section>
      )}

      <section className="rounded-lg border border-border p-4">
        <div className="flex items-start justify-between gap-4">
          <div>
            <h3 className="flex items-center gap-1.5 text-xs font-semibold text-foreground">
              <Sparkles className="h-3.5 w-3.5 text-primary" /> Analyse each capture
            </h3>
            <p className="mt-1 text-xs text-muted-foreground">
              Summarise and tag a capture as soon as it arrives. Captures are saved either way —
              turning this off costs you summaries, never the captured content.
            </p>
          </div>
          <Switch
            checked={status.analyze_on_capture}
            disabled={busy !== null}
            aria-label="Analyse each capture"
            onCheckedChange={(checked) =>
              void run('analyse', () =>
                invoke<CaptureBridgeStatus>('set_capture_analyze_on_capture', { enabled: checked }),
              )
            }
          />
        </div>
      </section>

      <section className="rounded-lg border border-border p-4">
        <h3 className="flex items-center gap-1.5 text-xs font-semibold text-foreground">
          <Keyboard className="h-3.5 w-3.5 text-primary" /> Shortcuts
        </h3>
        <dl className="mt-2 grid grid-cols-[10rem_1fr] gap-x-4 gap-y-1.5 text-xs">
          <dt className="text-muted-foreground">In your browser</dt>
          <dd className="text-foreground">
            <kbd className="rounded border border-border bg-muted px-1.5 py-0.5 font-mono text-[11px]">
              Ctrl+Shift+Y
            </kbd>{' '}
            captures the page you are on. Change it at{' '}
            <code className="rounded bg-muted px-1">chrome://extensions/shortcuts</code>.
          </dd>
          <dt className="text-muted-foreground">In Relay</dt>
          <dd className="text-foreground">
            <kbd className="rounded border border-border bg-muted px-1.5 py-0.5 font-mono text-[11px]">
              {status.capture_hotkey}
            </kbd>{' '}
            opens your captures.
          </dd>
        </dl>
        <p className="mt-2 text-[11px] text-muted-foreground">
          Reading a page has to be started from inside the browser: browsers grant an extension
          access to a tab only in response to a gesture made there, which is what keeps Relay from
          needing permission to every site you visit.
        </p>
      </section>
    </div>
  );
};
