import React, { useEffect, useState } from 'react';
import {
  AlertTriangle,
  ArrowUpRight,
  BookOpen,
  Check,
  CheckCircle2,
  Copy,
  ExternalLink,
  FileJson,
  Globe,
  HelpCircle,
  Info,
  RefreshCw,
  Sparkles,
  X,
} from 'lucide-react';
import { invoke } from '@tauri-apps/api/core';
import type { ConversationContext, Scribble, VaultFile } from '../../types';
import { MarkdownView } from '../common/MarkdownView';
import { CaptureContextTab } from './CaptureContextTab';
import {
  captureTypeLabel,
  describeCompleteness,
  describeTraversal,
  displayUrl,
  fidelityLabel,
  trustLabel,
  formatTimestamp,
} from './captureFormatting';

interface CaptureDetailModalProps {
  capture: VaultFile;
  onClose: () => void;
  onAnalyse: (id: string) => Promise<void>;
  onCreateScribble: (id: string) => Promise<Scribble | undefined>;
  onNavigateTab?: (tab: string) => void;
}

type DetailTab = 'content' | 'context' | 'provenance' | 'source';

/**
 * One capture, in full: what was saved, where it came from, and — the part
 * that makes it trustworthy — what Relay could not get.
 */
export const CaptureDetailModal: React.FC<CaptureDetailModalProps> = ({
  capture,
  onClose,
  onAnalyse,
  onCreateScribble,
  onNavigateTab,
}) => {
  const [activeTab, setActiveTab] = useState<DetailTab>('content');
  const [rawPayload, setRawPayload] = useState<string | null>(null);
  const [payloadError, setPayloadError] = useState<string | null>(null);
  const [busy, setBusy] = useState<string | null>(null);
  const [copied, setCopied] = useState(false);

  const [context, setContext] = useState<ConversationContext | null>(null);
  const [loadingContext, setLoadingContext] = useState(false);
  const [analyzingContext, setAnalyzingContext] = useState(false);

  const provenance = capture.capture;
  const isConversation =
    provenance?.capture_type === 'conversation' ||
    capture.content.includes('**User**:') ||
    capture.content.includes('**Assistant**:');

  useEffect(() => {
    if (!isConversation) return;
    let cancelled = false;
    setLoadingContext(true);
    invoke<ConversationContext | null>('get_capture_context', { id: capture.id })
      .then((ctx) => {
        if (!cancelled) setContext(ctx);
      })
      .catch((err: unknown) => {
        console.error('Failed to get capture context', err);
      })
      .finally(() => {
        if (!cancelled) setLoadingContext(false);
      });
    return () => {
      cancelled = true;
    };
  }, [capture.id, isConversation]);

  const handleAnalyzeContext = async () => {
    setAnalyzingContext(true);
    try {
      const ctx = await invoke<ConversationContext>('analyze_capture_context', { id: capture.id });
      setContext(ctx);
    } catch (err) {
      console.error('Failed to analyze capture context', err);
    } finally {
      setAnalyzingContext(false);
    }
  };

  const completeness = provenance ? describeCompleteness(provenance) : null;
  const traversal = provenance ? describeTraversal(provenance) : [];

  useEffect(() => {
    if (activeTab !== 'source' || rawPayload !== null) return;
    let cancelled = false;
    invoke<unknown>('get_capture_payload', { id: capture.id })
      .then((payload) => {
        if (!cancelled) setRawPayload(JSON.stringify(payload, null, 2));
      })
      .catch((err: unknown) => {
        if (!cancelled) {
          setPayloadError(
            'Relay could not read the stored source for this capture. The readable version above is unaffected.',
          );
          console.error('Failed to read capture payload', err);
        }
      });
    return () => {
      cancelled = true;
    };
  }, [activeTab, capture.id, rawPayload]);

  const run = async (name: string, fn: () => Promise<unknown>) => {
    setBusy(name);
    try {
      await fn();
    } finally {
      setBusy(null);
    }
  };

  const copyUrl = async () => {
    if (!provenance) return;
    try {
      await navigator.clipboard.writeText(provenance.url);
      setCopied(true);
      setTimeout(() => setCopied(false), 2000);
    } catch (err) {
      console.error('Could not copy the capture URL', err);
    }
  };

  const toneClasses: Record<string, string> = {
    complete: 'border-emerald-500/40 bg-emerald-500/10 text-emerald-600 dark:text-emerald-400',
    partial: 'border-amber-500/40 bg-amber-500/10 text-amber-600 dark:text-amber-400',
    unknown: 'border-muted bg-muted/40 text-muted-foreground',
  };

  return (
    <div
      className="fixed inset-0 z-50 flex items-center justify-center bg-black/60 p-6"
      role="dialog"
      aria-modal="true"
      aria-label={`Capture: ${capture.original_filename}`}
    >
      <div className="flex h-full max-h-[88vh] w-full max-w-4xl flex-col overflow-hidden rounded-xl border border-border bg-card shadow-2xl">
        <header className="flex items-start justify-between gap-4 border-b border-border px-5 py-4">
          <div className="min-w-0">
            <h2 className="truncate text-base font-semibold text-foreground">
              {capture.original_filename}
            </h2>
            {provenance && (
              <p className="mt-1 flex flex-wrap items-center gap-x-2 gap-y-1 text-xs text-muted-foreground">
                <Globe className="h-3.5 w-3.5 text-primary" />
                <span className="font-medium text-foreground">{provenance.application}</span>
                <span aria-hidden>·</span>
                <span>{captureTypeLabel(provenance.capture_type)}</span>
                <span aria-hidden>·</span>
                <span>{formatTimestamp(provenance.captured_at)}</span>
              </p>
            )}
          </div>
          <button
            type="button"
            onClick={onClose}
            className="rounded-md p-1.5 text-muted-foreground transition-colors hover:bg-muted hover:text-foreground"
            aria-label="Close capture"
          >
            <X className="h-4 w-4" />
          </button>
        </header>

        {completeness && (
          <div className={`flex items-start gap-2 border-b border-border px-5 py-2.5 text-xs ${toneClasses[completeness.tone]}`}>
            {completeness.tone === 'complete' ? (
              <CheckCircle2 className="mt-px h-3.5 w-3.5 shrink-0" />
            ) : completeness.tone === 'partial' ? (
              <AlertTriangle className="mt-px h-3.5 w-3.5 shrink-0" />
            ) : (
              <HelpCircle className="mt-px h-3.5 w-3.5 shrink-0" />
            )}
            <span className="font-medium">{completeness.headline}</span>
          </div>
        )}

        <nav className="flex gap-1 border-b border-border px-4 pt-2" aria-label="Capture views">
          {(
            [
              ['content', 'Content'],
              ...(isConversation ? [['context', 'Context'] as [DetailTab, string]] : []),
              ['provenance', 'Where it came from'],
              ['source', 'Stored source'],
            ] as [DetailTab, string][]
          ).map(([id, label]) => (
            <button
              key={id}
              type="button"
              onClick={() => setActiveTab(id)}
              aria-current={activeTab === id ? 'page' : undefined}
              className={`rounded-t-md px-3 py-2 text-xs font-medium transition-colors ${
                activeTab === id
                  ? 'border-b-2 border-primary text-foreground'
                  : 'text-muted-foreground hover:text-foreground'
              }`}
            >
              {label}
            </button>
          ))}
        </nav>

        <div className="flex-1 overflow-y-auto px-5 py-4">
          {activeTab === 'content' && (
            <>
              {capture.summary && (
                <section className="mb-4 rounded-lg border border-border bg-muted/30 p-3">
                  <h3 className="mb-1.5 flex items-center gap-1.5 text-xs font-semibold text-foreground">
                    <Sparkles className="h-3.5 w-3.5 text-primary" /> Summary
                  </h3>
                  <MarkdownView content={capture.summary} className="text-xs" />
                </section>
              )}
              <MarkdownView content={capture.content} />
            </>
          )}

          {activeTab === 'context' && (
            <CaptureContextTab
              context={context}
              provenance={provenance}
              loading={loadingContext}
              analyzing={analyzingContext}
              onAnalyze={handleAnalyzeContext}
            />
          )}

          {activeTab === 'provenance' && provenance && (
            <div className="space-y-4 text-xs">
              <dl className="grid grid-cols-[9rem_1fr] gap-x-4 gap-y-2">
                <dt className="text-muted-foreground">Application</dt>
                <dd className="text-foreground">{provenance.application}</dd>
                <dt className="text-muted-foreground">Domain</dt>
                <dd className="text-foreground">{provenance.domain}</dd>
                <dt className="text-muted-foreground">URL</dt>
                <dd className="flex items-center gap-2 break-all text-foreground">
                  {provenance.url}
                  <button
                    type="button"
                    onClick={copyUrl}
                    className="shrink-0 rounded p-1 text-muted-foreground hover:bg-muted hover:text-foreground"
                    aria-label="Copy the capture URL"
                  >
                    {copied ? <Check className="h-3 w-3 text-emerald-500" /> : <Copy className="h-3 w-3" />}
                  </button>
                </dd>
                <dt className="text-muted-foreground">Page title</dt>
                <dd className="text-foreground">{provenance.page_title}</dd>
                <dt className="text-muted-foreground">Captured</dt>
                <dd className="text-foreground">{formatTimestamp(provenance.captured_at)}</dd>
                <dt className="text-muted-foreground">Kind</dt>
                <dd className="text-foreground">{captureTypeLabel(provenance.capture_type)}</dd>
                <dt className="text-muted-foreground">How it was read</dt>
                <dd className="text-foreground">
                  {fidelityLabel(provenance.fidelity)} · {provenance.extractor_id} v
                  {provenance.extractor_version}
                </dd>
                <dt className="text-muted-foreground">Trust</dt>
                <dd className="text-foreground">{trustLabel(provenance.trust)}</dd>
                {provenance.author && (
                  <>
                    <dt className="text-muted-foreground">Author</dt>
                    <dd className="text-foreground">{provenance.author}</dd>
                  </>
                )}
                {provenance.published_at && (
                  <>
                    <dt className="text-muted-foreground">Published</dt>
                    <dd className="text-foreground">{provenance.published_at}</dd>
                  </>
                )}
                {provenance.message_count != null && (
                  <>
                    <dt className="text-muted-foreground">Turns captured</dt>
                    <dd className="text-foreground">{provenance.message_count}</dd>
                  </>
                )}
                <dt className="text-muted-foreground">Version</dt>
                <dd className="text-foreground">
                  Version {provenance.version}
                  {provenance.recapture_count > 0 &&
                    ` · re-captured unchanged ${provenance.recapture_count}× (latest: ${formatTimestamp(provenance.captured_at)})`}
                </dd>
              </dl>

              {traversal.length > 0 && (
                <section className="rounded-lg border border-border bg-muted/30 p-3">
                  <h3 className="mb-1.5 flex items-center gap-1.5 text-xs font-semibold text-foreground">
                    <Info className="h-3.5 w-3.5 text-primary" /> What reading this page measured
                  </h3>
                  <ul className="list-disc space-y-1 pl-4 text-muted-foreground">
                    {traversal.map((line) => (
                      <li key={line}>{line}</li>
                    ))}
                  </ul>
                </section>
              )}

              {provenance.notes.length > 0 && (
                <section className="rounded-lg border border-border bg-muted/30 p-3">
                  <h3 className="mb-1.5 flex items-center gap-1.5 text-xs font-semibold text-foreground">
                    <Info className="h-3.5 w-3.5 text-primary" /> What Relay could and could not get
                  </h3>
                  <ul className="list-disc space-y-1 pl-4 text-muted-foreground">
                    {provenance.notes.map((note) => (
                      <li key={note}>{note}</li>
                    ))}
                  </ul>
                </section>
              )}
            </div>
          )}

          {activeTab === 'source' && (
            <>
              <p className="mb-3 flex items-start gap-2 text-xs text-muted-foreground">
                <FileJson className="mt-px h-3.5 w-3.5 shrink-0 text-primary" />
                The structured payload exactly as it was captured. It is written once and never
                rewritten, so it stays a faithful record even after summaries and tags change.
              </p>
              {payloadError && (
                <p className="rounded-lg border border-amber-500/40 bg-amber-500/10 p-3 text-xs text-amber-600 dark:text-amber-400">
                  {payloadError}
                </p>
              )}
              {rawPayload && (
                <pre className="overflow-x-auto rounded-lg border border-border bg-muted/30 p-3 font-mono text-[11px] leading-relaxed text-foreground">
                  {rawPayload}
                </pre>
              )}
              {!rawPayload && !payloadError && (
                <p className="text-xs text-muted-foreground">Loading the stored source…</p>
              )}
            </>
          )}
        </div>

        <footer className="flex flex-wrap items-center gap-2 border-t border-border px-5 py-3">
          <button
            type="button"
            disabled={busy !== null}
            onClick={() => run('analyse', () => onAnalyse(capture.id))}
            className="inline-flex items-center gap-1.5 rounded-md border border-border px-3 py-1.5 text-xs font-medium text-foreground transition-colors hover:bg-muted disabled:opacity-50"
          >
            {busy === 'analyse' ? (
              <RefreshCw className="h-3.5 w-3.5 animate-spin" />
            ) : (
              <Sparkles className="h-3.5 w-3.5 text-primary" />
            )}
            {capture.summary ? 'Re-analyse' : 'Analyse'}
          </button>

          <button
            type="button"
            disabled={busy !== null || Boolean(capture.linked_scribble_id)}
            onClick={() =>
              run('scribble', async () => {
                await onCreateScribble(capture.id);
                onNavigateTab?.('scribble');
              })
            }
            className="inline-flex items-center gap-1.5 rounded-md border border-border px-3 py-1.5 text-xs font-medium text-foreground transition-colors hover:bg-muted disabled:opacity-50"
          >
            <BookOpen className="h-3.5 w-3.5 text-primary" />
            {capture.linked_scribble_id ? 'Already a Scribble' : 'Add to Scribbles'}
          </button>

          {provenance && (
            <a
              href={provenance.url}
              target="_blank"
              rel="noreferrer noopener"
              className="ml-auto inline-flex items-center gap-1.5 rounded-md px-3 py-1.5 text-xs font-medium text-muted-foreground transition-colors hover:bg-muted hover:text-foreground"
            >
              <ExternalLink className="h-3.5 w-3.5" />
              {displayUrl(provenance.url)}
              <ArrowUpRight className="h-3 w-3" />
            </a>
          )}
        </footer>
      </div>
    </div>
  );
};
