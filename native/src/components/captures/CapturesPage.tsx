import React, { useCallback, useEffect, useMemo, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import {
  AlertTriangle,
  CheckCircle2,
  Globe,
  MessageSquare,
  RefreshCw,
  Search,
  Settings,
  Sparkles,
  Trash2,
  Upload,
} from 'lucide-react';
import type { CaptureBridgeStatus, CaptureProgress, Scribble, VaultFile } from '../../types';
import { ConfirmationModal } from '../common/ConfirmationModal';
import { EmptyState } from '../common/EmptyState';
import { CaptureDetailModal } from './CaptureDetailModal';
import { ImportConversationModal } from './ImportConversationModal';
import {
  captureTypeLabel,
  describeCompleteness,
  displayUrl,
  formatTimestamp,
  matchesQuery,
} from './captureFormatting';

interface CapturesPageProps {
  onNavigateTab?: (tab: string) => void;
  onOpenCaptureSettings?: () => void;
}

/** The live "Capturing… / Saved" banner, driven by backend events. */
const STAGE_COPY: Record<string, string> = {
  SAVING: 'Capturing…',
  SAVED: 'Saved',
  ANALYSING: 'Analysing…',
  ANALYSED: 'Analysed',
  FAILED: 'Capture failed',
};

/**
 * Everything captured from the browser.
 *
 * Deliberately its own surface rather than a filter on Files: an imported
 * document and a captured page answer different questions ("what did I
 * import?" versus "what was I looking at?"), and mixing them made both lists
 * worse.
 */
export const CapturesPage: React.FC<CapturesPageProps> = ({
  onNavigateTab,
  onOpenCaptureSettings,
}) => {
  const [captures, setCaptures] = useState<VaultFile[]>([]);
  const [status, setStatus] = useState<CaptureBridgeStatus | null>(null);
  const [loading, setLoading] = useState(true);
  const [query, setQuery] = useState('');
  const [selected, setSelected] = useState<VaultFile | null>(null);
  const [pendingDelete, setPendingDelete] = useState<VaultFile | null>(null);
  const [progress, setProgress] = useState<CaptureProgress | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [showImport, setShowImport] = useState(false);

  const load = useCallback(async () => {
    try {
      const [list, bridge] = await Promise.all([
        invoke<VaultFile[]>('get_captures'),
        invoke<CaptureBridgeStatus>('get_capture_bridge_status'),
      ]);
      setCaptures(list);
      setStatus(bridge);
      setError(null);
    } catch (err) {
      console.error('Failed to load captures', err);
      setError('Relay could not read your captures.');
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    void load();

    const unlisten = listen<CaptureProgress>('capture-progress', (event) => {
      setProgress(event.payload);
      // A capture is durable at SAVED; the later ANALYSED event only adds a
      // summary, so the list is refreshed on both.
      if (event.payload.stage === 'SAVED' || event.payload.stage === 'ANALYSED') {
        void load();
      }
    });

    return () => {
      void unlisten.then((fn) => fn());
    };
  }, [load]);

  useEffect(() => {
    if (!progress || progress.stage === 'SAVING' || progress.stage === 'ANALYSING') return;
    const timer = setTimeout(() => setProgress(null), 6000);
    return () => clearTimeout(timer);
  }, [progress]);

  const visible = useMemo(
    () => captures.filter((capture) => matchesQuery(capture, query)),
    [captures, query],
  );

  const analyse = async (id: string) => {
    const updated = await invoke<VaultFile>('analyze_vault_file', { id });
    setCaptures((current) => current.map((c) => (c.id === id ? updated : c)));
    setSelected((current) => (current?.id === id ? updated : current));
  };

  const createScribble = async (id: string): Promise<Scribble | undefined> => {
    try {
      const scribble = await invoke<Scribble>('create_scribble_from_vault_file', { id });
      await load();
      return scribble;
    } catch (err) {
      console.error('Failed to promote capture to a Scribble', err);
      setError('Relay could not add this capture to Scribbles.');
      return undefined;
    }
  };

  const remove = async (capture: VaultFile) => {
    try {
      await invoke('delete_capture', { id: capture.id });
      setPendingDelete(null);
      setSelected(null);
      await load();
    } catch (err) {
      console.error('Failed to delete capture', err);
      setError('Relay could not move that capture to Trash.');
    }
  };

  const bridgeOffline = status !== null && !status.running;

  return (
    <div className="flex h-full flex-col">
      <div className="flex flex-wrap items-center gap-2 border-b border-border px-5 py-3">
        <div className="relative min-w-[14rem] flex-1">
          <Search className="pointer-events-none absolute left-2.5 top-1/2 h-3.5 w-3.5 -translate-y-1/2 text-muted-foreground" />
          <input
            type="search"
            value={query}
            onChange={(event) => setQuery(event.target.value)}
            placeholder="Search captures by content, site or tag"
            aria-label="Search captures"
            className="w-full rounded-md border border-border bg-background py-1.5 pl-8 pr-3 text-xs text-foreground placeholder:text-muted-foreground focus:outline-none focus:ring-2 focus:ring-ring"
          />
        </div>
        <button
          type="button"
          onClick={() => void load()}
          className="inline-flex items-center gap-1.5 rounded-md border border-border px-2.5 py-1.5 text-xs font-medium text-foreground transition-colors hover:bg-muted"
        >
          <RefreshCw className="h-3.5 w-3.5" /> Refresh
        </button>
        <button
          type="button"
          onClick={() => setShowImport(true)}
          className="inline-flex items-center gap-1.5 rounded-md bg-primary px-3 py-1.5 text-xs font-medium text-primary-foreground shadow-sm transition-opacity hover:opacity-90"
        >
          <Upload className="h-3.5 w-3.5" /> Import AI conversation
        </button>
        <button
          type="button"
          onClick={onOpenCaptureSettings}
          className="inline-flex items-center gap-1.5 rounded-md border border-border px-2.5 py-1.5 text-xs font-medium text-foreground transition-colors hover:bg-muted"
        >
          <Settings className="h-3.5 w-3.5" /> Capture settings
        </button>
      </div>

      {progress && (
        <div
          role="status"
          aria-live="polite"
          className={`flex items-center gap-2 border-b border-border px-5 py-2 text-xs ${
            progress.stage === 'FAILED'
              ? 'bg-red-500/10 text-red-600 dark:text-red-400'
              : 'bg-primary/10 text-foreground'
          }`}
        >
          {progress.stage === 'FAILED' ? (
            <AlertTriangle className="h-3.5 w-3.5" />
          ) : progress.stage === 'SAVED' || progress.stage === 'ANALYSED' ? (
            <CheckCircle2 className="h-3.5 w-3.5 text-emerald-500" />
          ) : (
            <RefreshCw className="h-3.5 w-3.5 animate-spin" />
          )}
          <span className="font-medium">{STAGE_COPY[progress.stage] ?? progress.stage}</span>
          {progress.title && <span className="truncate text-muted-foreground">{progress.title}</span>}
          {progress.message && <span className="truncate">{progress.message}</span>}
        </div>
      )}

      {error && (
        <div className="border-b border-border bg-red-500/10 px-5 py-2 text-xs text-red-600 dark:text-red-400">
          {error}
        </div>
      )}

      {bridgeOffline && (
        <div className="flex flex-wrap items-center gap-2 border-b border-border bg-amber-500/10 px-5 py-2 text-xs text-amber-700 dark:text-amber-400">
          <AlertTriangle className="h-3.5 w-3.5" />
          <span>
            Browser capture is off, so nothing can reach Relay from your browser right now.
          </span>
          <button
            type="button"
            onClick={onOpenCaptureSettings}
            className="font-semibold underline underline-offset-2"
          >
            Turn it on
          </button>
        </div>
      )}

      <div className="flex-1 overflow-y-auto px-5 py-4">
        {loading ? (
          <p className="text-xs text-muted-foreground">Loading captures…</p>
        ) : visible.length === 0 ? (
          <EmptyState
            icon={Globe}
            title={captures.length === 0 ? 'Nothing captured yet' : 'No captures match that search'}
            description={
              captures.length === 0
                ? status?.capture_hotkey
                  ? `Install the Relay browser extension, then press its shortcut on any page. ${status.capture_hotkey} brings this list back up from anywhere.`
                  : 'Install the Relay browser extension, then press its shortcut on any page.'
                : 'Try fewer words, or search for the site the page came from.'
            }
          />
        ) : (
          <ul className="space-y-2">
            {visible.map((capture) => {
              const provenance = capture.capture;
              const completeness = provenance ? describeCompleteness(provenance) : null;
              const isConversation = provenance?.capture_type === 'conversation';

              return (
                <li key={capture.id}>
                  <div className="group flex items-start gap-3 rounded-lg border border-border bg-card p-3 transition-colors hover:border-primary/40">
                    <button
                      type="button"
                      onClick={() => setSelected(capture)}
                      className="min-w-0 flex-1 text-left"
                    >
                      <div className="flex items-center gap-2">
                        {isConversation ? (
                          <MessageSquare className="h-3.5 w-3.5 shrink-0 text-primary" />
                        ) : (
                          <Globe className="h-3.5 w-3.5 shrink-0 text-primary" />
                        )}
                        <span className="truncate text-sm font-medium text-foreground">
                          {capture.original_filename}
                        </span>
                        {capture.summary && (
                          <Sparkles
                            className="h-3 w-3 shrink-0 text-primary/70"
                            aria-label="Analysed"
                          />
                        )}
                      </div>

                      {provenance && (
                        <p className="mt-1 flex flex-wrap items-center gap-x-2 gap-y-0.5 text-[11px] text-muted-foreground">
                          <span className="font-medium text-foreground/80">
                            {provenance.application}
                          </span>
                          <span aria-hidden>·</span>
                          <span>{captureTypeLabel(provenance.capture_type)}</span>
                          <span aria-hidden>·</span>
                          <span>{formatTimestamp(provenance.captured_at)}</span>
                          <span aria-hidden>·</span>
                          <span className="truncate">{displayUrl(provenance.url)}</span>
                        </p>
                      )}

                      {capture.summary && (
                        <p className="mt-1.5 line-clamp-2 text-[11px] text-muted-foreground">
                          {capture.summary.replace(/[*#`]/g, '')}
                        </p>
                      )}

                      {completeness && completeness.tone !== 'complete' && (
                        <p className="mt-1.5 inline-flex items-center gap-1 rounded border border-amber-500/40 bg-amber-500/10 px-1.5 py-0.5 text-[10px] font-medium text-amber-700 dark:text-amber-400">
                          <AlertTriangle className="h-2.5 w-2.5" />
                          {completeness.headline}
                        </p>
                      )}
                    </button>

                    <button
                      type="button"
                      onClick={() => setPendingDelete(capture)}
                      aria-label={`Delete capture ${capture.original_filename}`}
                      className="rounded-md p-1.5 text-muted-foreground opacity-0 transition-opacity hover:bg-muted hover:text-red-500 focus:opacity-100 group-hover:opacity-100"
                    >
                      <Trash2 className="h-3.5 w-3.5" />
                    </button>
                  </div>
                </li>
              );
            })}
          </ul>
        )}
      </div>

      {selected && (
        <CaptureDetailModal
          capture={selected}
          onClose={() => setSelected(null)}
          onAnalyse={analyse}
          onCreateScribble={createScribble}
          onNavigateTab={onNavigateTab}
        />
      )}

      {pendingDelete && (
        <ConfirmationModal
          isOpen
          title="Move this capture to Trash?"
          description={`“${pendingDelete.original_filename}” goes to Trash, where it can be restored for 30 days.`}
          confirmLabel="Move to Trash"
          variant="destructive"
          onConfirm={() => void remove(pendingDelete)}
          onCancel={() => setPendingDelete(null)}
        />
      )}

      {showImport && (
        <ImportConversationModal
          onClose={() => setShowImport(false)}
          onSuccess={(imported) => {
            void load();
            setSelected(imported);
          }}
        />
      )}
    </div>
  );
};
