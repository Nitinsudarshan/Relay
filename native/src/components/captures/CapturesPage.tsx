import React, { useCallback, useEffect, useMemo, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import {
  AlertTriangle,
  CheckCircle2,
  Globe,
  MessageSquare,
  PlusCircle,
  RefreshCw,
  Search,
  Settings,
  Sparkles,
  Trash2,
  Upload,
} from 'lucide-react';
import type {
  CaptureBridgeStatus,
  CaptureProgress,
  MainTabType,
  Scribble,
  VaultFile,
} from '../../types';
import { Badge } from '@/components/ui/badge';
import { ConfirmationModal } from '../common/ConfirmationModal';
import { EmptyState } from '../common/EmptyState';
import { CaptureDetailModal } from './CaptureDetailModal';
import { CaptureHubPage, type CaptureMethod } from './CaptureHubPage';
import { ImportConversationModal } from './ImportConversationModal';
import {
  captureTypeLabel,
  describeCompleteness,
  displayUrl,
  formatTimestamp,
  getLatestCaptureActivity,
  matchesQuery,
} from './captureFormatting';

type CapturesSubTab = 'capture' | 'pages';

interface CapturesPageProps {
  onNavigateTab?: (tab: MainTabType) => void;
  onOpenCaptureSettings?: () => void;
  /**
   * Opens the Capture tab on the given method. Set by a Home shortcut card, so
   * "Clipboard" on Home lands on the clipboard capture rather than on a tab the
   * user then has to find.
   */
  initialCaptureMethod?: CaptureMethod | null;
  /** Reveals a scribble captured here — or promoted from a capture — in Scribbles. */
  onOpenScribble?: (scribbleId: string) => void;
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
 * Everything Relay captures, in one surface.
 *
 * `Capture` is where a thought is written, pasted or handed to the surface that
 * owns its mode; `Captured Pages` is what the browser extension has sent here.
 * They are two tabs rather than two sidebar entries because they are the same
 * question asked in two tenses — what am I capturing, and what did I capture.
 *
 * Captured pages remain their own list rather than a filter on Files: an
 * imported document and a captured page answer different questions ("what did I
 * import?" versus "what was I looking at?"), and mixing them made both lists
 * worse.
 */
export const CapturesPage: React.FC<CapturesPageProps> = ({
  onNavigateTab,
  onOpenCaptureSettings,
  initialCaptureMethod = null,
  onOpenScribble,
}) => {
  const [activeSubTab, setActiveSubTab] = useState<CapturesSubTab>(
    initialCaptureMethod ? 'capture' : 'pages',
  );
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

  const visible = useMemo(() => {
    // 1. Identify older superseded capture IDs so only current heads appear in the list
    const supersededIds = new Set(
      captures
        .map((c) => c.capture?.previous_capture_id)
        .filter((id): id is string => Boolean(id)),
    );

    // 2. Filter out superseded captures and apply search query
    const active = captures
      .filter((capture) => !supersededIds.has(capture.id))
      .filter((capture) => matchesQuery(capture, query));

    // 3. Sort by latest activity (max of captured_at, updated_at, created_at)
    return [...active].sort((a, b) => {
      const aTime = getLatestCaptureActivity(a);
      const bTime = getLatestCaptureActivity(b);
      return bTime.localeCompare(aTime);
    });
  }, [captures, query]);

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
      {/* Sub-Navigation: Capture | Captured Pages */}
      <div className="flex items-center justify-between gap-2 border-b border-border px-5 py-3">
        <div className="flex items-center rounded-lg border border-border bg-muted/60 p-1 text-xs">
          <button
            type="button"
            onClick={() => setActiveSubTab('capture')}
            aria-pressed={activeSubTab === 'capture'}
            className={`flex items-center gap-1.5 rounded-lg px-3 py-1.5 font-medium transition-all ${
              activeSubTab === 'capture'
                ? 'bg-card font-bold text-foreground shadow-xs'
                : 'text-muted-foreground hover:text-foreground'
            }`}
          >
            <PlusCircle className="h-3.5 w-3.5 text-primary" />
            <span>Capture</span>
          </button>

          <button
            type="button"
            onClick={() => setActiveSubTab('pages')}
            aria-pressed={activeSubTab === 'pages'}
            className={`flex items-center gap-1.5 rounded-lg px-3 py-1.5 font-medium transition-all ${
              activeSubTab === 'pages'
                ? 'bg-card font-bold text-foreground shadow-xs'
                : 'text-muted-foreground hover:text-foreground'
            }`}
          >
            <Globe className="h-3.5 w-3.5" />
            <span>Captured Pages</span>
          </button>
        </div>

        {activeSubTab === 'pages' && (
          <Badge
            variant="outline"
            className="animate-in fade-in border-border bg-card/60 px-2.5 py-1 font-mono text-[11px] text-muted-foreground duration-150"
          >
            {visible.length} Page{visible.length === 1 ? '' : 's'}
          </Badge>
        )}
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

      {/* Surface 1: Capture — the modes this app captures with. */}
      {activeSubTab === 'capture' && (
        <div className="flex flex-1 flex-col overflow-hidden px-5 py-4">
          <CaptureHubPage
            initialMethod={initialCaptureMethod}
            bridgeRunning={status ? status.running : null}
            onOpenScribble={(id) => onOpenScribble?.(id)}
            onNavigate={(tab) => onNavigateTab?.(tab)}
            onOpenCapturedPages={() => setActiveSubTab('pages')}
          />
        </div>
      )}

      {/* Surface 2: Captured Pages — what the browser extension has sent here. */}
      {activeSubTab === 'pages' && (
        <>
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
                            {provenance.version > 1 && (
                              <>
                                <span aria-hidden>·</span>
                                <span className="font-semibold text-foreground/90">v{provenance.version}</span>
                              </>
                            )}
                            {provenance.recapture_count > 0 && provenance.version === 1 && (
                              <>
                                <span aria-hidden>·</span>
                                <span>v1 ({provenance.recapture_count + 1}×)</span>
                              </>
                            )}
                            <span aria-hidden>·</span>
                            <span>{formatTimestamp(provenance.captured_at)}</span>
                            <span aria-hidden>·</span>
                            <span
                              onClick={(e) => {
                                e.stopPropagation();
                                void invoke('open_external_url', { url: provenance.url });
                              }}
                              title={`Open ${provenance.url} in browser`}
                              className="truncate hover:text-foreground hover:underline cursor-pointer"
                            >
                              {displayUrl(provenance.url)}
                            </span>
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
        </>
      )}

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
