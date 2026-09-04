import React from 'react';
import {
  AlertCircle,
  AlertTriangle,
  Ban,
  CheckCircle2,
  Compass,
  FileCode,
  HelpCircle,
  ListChecks,
  RefreshCw,
  Sparkles,
  Target,
} from 'lucide-react';
import type { CaptureProvenance, ConversationContext, RepositoryContext, SourceContext } from '../../types';
import { RepositoryContextView } from './RepositoryContextView';

interface CaptureContextTabProps {
  context: SourceContext | null;
  provenance?: CaptureProvenance | null;
  loading: boolean;
  analyzing: boolean;
  onAnalyze: () => Promise<void>;
}

export const CaptureContextTab: React.FC<CaptureContextTabProps> = ({
  context,
  provenance,
  loading,
  analyzing,
  onAnalyze,
}) => {
  // The classification Relay derived from the URL at capture time, not a name
  // or substring match here. `application` is `"GitHub"`, so the old
  // `=== 'github'` test never matched, and the URL fallback treated any address
  // containing "github.com" — including `https://evil.example/?ref=github.com`
  // — as a repository.
  const isRepository = provenance?.capture_type === 'repository';

  if (loading) {
    return (
      <div className="flex h-64 flex-col items-center justify-center gap-2 text-muted-foreground">
        <RefreshCw className="h-5 w-5 animate-spin text-primary" />
        <p className="text-xs">Loading structured context…</p>
      </div>
    );
  }

  if (!context) {
    return (
      <div className="flex h-72 flex-col items-center justify-center rounded-lg border border-dashed border-border p-6 text-center">
        <div className="mb-3 rounded-full bg-primary/10 p-3 text-primary">
          <Sparkles className="h-6 w-6" />
        </div>
        <h3 className="text-sm font-semibold text-foreground">Structured Context Unavailable</h3>
        <p className="mt-1.5 max-w-sm text-xs text-muted-foreground leading-relaxed">
          {isRepository
            ? 'Relay has captured this repository, but has not yet extracted structured repository context.'
            : 'Extract objectives, settled decisions, requirements, constraints, open questions, and next actions to preserve this work in Relay.'}
        </p>
        <button
          type="button"
          disabled={analyzing}
          onClick={onAnalyze}
          className="mt-4 inline-flex items-center gap-2 rounded-md bg-primary px-3.5 py-1.5 text-xs font-medium text-primary-foreground shadow-sm transition-opacity hover:opacity-90 disabled:opacity-50"
        >
          {analyzing ? (
            <>
              <RefreshCw className="h-3.5 w-3.5 animate-spin" />
              <span>{isRepository ? 'Analyzing Repository…' : 'Analyzing Source…'}</span>
            </>
          ) : (
            <>
              <Sparkles className="h-3.5 w-3.5" />
              <span>{isRepository ? 'Extract Repository Context' : 'Extract Structured Context'}</span>
            </>
          )}
        </button>
      </div>
    );
  }

  const isPartial = provenance?.coverage === 'partial' || provenance?.coverage === 'rendered_dom';
  const isRepo = Boolean('kind' in context && context.kind === 'repository');

  if (isRepo) {
    const repoContext = (context as { kind: 'repository'; data: RepositoryContext }).data;
    return (
      <div className="space-y-6 text-xs leading-relaxed">
        {isPartial && (
          <div className="flex items-start gap-2.5 rounded-lg border border-amber-500/40 bg-amber-500/10 p-3 text-xs text-amber-700 dark:text-amber-400">
            <AlertTriangle className="mt-0.5 h-4 w-4 shrink-0" />
            <div className="space-y-0.5">
              <p className="font-semibold">Context based on a partial capture</p>
              <p className="text-[11px] text-muted-foreground leading-relaxed">
                Relay stopped reading before reaching the full repository contents or documentation.
                This analytical model was derived only from the content Relay could reach; additional parts may be absent from this record.
              </p>
            </div>
          </div>
        )}
        <RepositoryContextView
          context={repoContext}
          analyzing={analyzing}
          onAnalyze={onAnalyze}
        />
      </div>
    );
  }

  const conv: ConversationContext =
    'kind' in context && context.kind === 'conversation'
      ? context.data
      : (context as unknown as ConversationContext);

  return (
    <div className="space-y-6 text-xs leading-relaxed">
      {/* Honesty Banner: Incomplete / Partial Source Material */}
      {isPartial && (
        <div className="flex items-start gap-2.5 rounded-lg border border-amber-500/40 bg-amber-500/10 p-3 text-xs text-amber-700 dark:text-amber-400">
          <AlertTriangle className="mt-0.5 h-4 w-4 shrink-0" />
          <div className="space-y-0.5">
            <p className="font-semibold">Context based on a partial capture</p>
            <p className="text-[11px] text-muted-foreground leading-relaxed">
              Relay stopped reading before reaching the full document or conversation thread.
              This analytical model was derived only from the content Relay could reach; earlier or later parts may be absent from this record.
            </p>
          </div>
        </div>
      )}

      {/* Overview & Objective Banner */}
      <section className="rounded-lg border border-border bg-muted/30 p-4">
        <div className="flex flex-wrap items-center justify-between gap-2 border-b border-border/60 pb-3">
          <div className="flex items-center gap-2">
            <Target className="h-4 w-4 text-primary" />
            <h3 className="font-semibold text-foreground">Objective</h3>
          </div>
          <div className="flex items-center gap-2">
            <span className="rounded bg-primary/10 px-2 py-0.5 text-[10px] font-medium text-primary">
              {conv.deterministic ? 'Deterministic Analysis' : `AI: ${conv.model ?? 'Enriched'}`}
            </span>
            <button
              type="button"
              disabled={analyzing}
              onClick={onAnalyze}
              className="inline-flex items-center gap-1 rounded border border-border bg-card px-2 py-1 text-[11px] font-medium text-muted-foreground transition-colors hover:bg-muted hover:text-foreground disabled:opacity-50"
              title="Re-analyze context with current LLM"
            >
              <RefreshCw className={`h-3 w-3 ${analyzing ? 'animate-spin' : ''}`} />
              <span>{analyzing ? 'Analyzing…' : 'Re-analyze'}</span>
            </button>
          </div>
        </div>
        <p className="mt-2.5 text-foreground font-medium">{conv.objective}</p>

        {conv.current_state && (
          <div className="mt-3 rounded border border-border/80 bg-card p-2.5">
            <div className="flex items-center gap-1.5 font-medium text-muted-foreground text-[11px]">
              <Compass className="h-3.5 w-3.5 text-primary" />
              <span>Current State of Work</span>
            </div>
            <p className="mt-1 text-foreground">{conv.current_state}</p>
          </div>
        )}
      </section>

      {/* Decisions Made */}
      {conv.decisions.length > 0 && (
        <section className="space-y-2">
          <h4 className="flex items-center gap-1.5 font-semibold text-foreground">
            <CheckCircle2 className="h-3.5 w-3.5 text-emerald-500" />
            <span>Key Decisions Made ({conv.decisions.length})</span>
          </h4>
          <div className="grid gap-2 sm:grid-cols-2">
            {conv.decisions.map((dec) => (
              <div
                key={dec.id}
                className="flex flex-col justify-between rounded-lg border border-border bg-card p-3 shadow-sm"
              >
                <div>
                  <div className="flex items-start justify-between gap-2">
                    <p className="font-medium text-foreground">{dec.decision}</p>
                    <span className="shrink-0 rounded bg-emerald-500/10 px-1.5 py-0.5 text-[9px] font-medium text-emerald-600 dark:text-emerald-400">
                      {dec.status}
                    </span>
                  </div>
                  {dec.rationale && (
                    <p className="mt-1 text-muted-foreground text-[11px] leading-normal">
                      <span className="font-semibold text-foreground/80">Rationale:</span>{' '}
                      {dec.rationale}
                    </p>
                  )}
                </div>
                {dec.source_turn_ordinals.length > 0 && (
                  <div className="mt-2 flex items-center gap-1 text-[10px] text-muted-foreground">
                    <span>Source turns:</span>
                    {dec.source_turn_ordinals.map((ord) => (
                      <span
                        key={ord}
                        className="rounded bg-muted px-1.5 py-0.2 font-mono text-[9px]"
                      >
                        #{ord}
                      </span>
                    ))}
                  </div>
                )}
              </div>
            ))}
          </div>
        </section>
      )}

      {/* Requirements & Constraints */}
      {(conv.requirements.length > 0 || conv.constraints.length > 0) && (
        <div className="grid gap-4 sm:grid-cols-2">
          {conv.requirements.length > 0 && (
            <section className="space-y-2 rounded-lg border border-border bg-muted/20 p-3">
              <h4 className="flex items-center gap-1.5 font-semibold text-foreground">
                <ListChecks className="h-3.5 w-3.5 text-primary" />
                <span>Requirements ({conv.requirements.length})</span>
              </h4>
              <ul className="space-y-1.5 pl-2">
                {conv.requirements.map((req) => (
                  <li key={req.id} className="flex items-start gap-1.5 text-muted-foreground">
                    <span className="text-primary">•</span>
                    <span className="text-foreground">{req.statement}</span>
                  </li>
                ))}
              </ul>
            </section>
          )}

          {conv.constraints.length > 0 && (
            <section className="space-y-2 rounded-lg border border-border bg-muted/20 p-3">
              <h4 className="flex items-center gap-1.5 font-semibold text-foreground">
                <AlertCircle className="h-3.5 w-3.5 text-amber-500" />
                <span>Constraints & Boundaries ({conv.constraints.length})</span>
              </h4>
              <ul className="space-y-1.5 pl-2">
                {conv.constraints.map((con) => (
                  <li key={con.id} className="flex flex-col gap-0.5 text-muted-foreground">
                    <div className="flex items-start gap-1.5">
                      <span className="text-amber-500">•</span>
                      <span className="text-foreground font-medium">{con.statement}</span>
                    </div>
                    {con.reason && (
                      <span className="pl-3 text-[11px] text-muted-foreground">
                        Why: {con.reason}
                      </span>
                    )}
                  </li>
                ))}
              </ul>
            </section>
          )}
        </div>
      )}

      {/* Rejected Approaches */}
      {conv.rejected_approaches.length > 0 && (
        <section className="space-y-2 rounded-lg border border-rose-500/20 bg-rose-500/5 p-3">
          <h4 className="flex items-center gap-1.5 font-semibold text-rose-600 dark:text-rose-400">
            <Ban className="h-3.5 w-3.5" />
            <span>Rejected Approaches — Do Not Repeat</span>
          </h4>
          <ul className="space-y-1.5 pl-2">
            {conv.rejected_approaches.map((rej, idx) => (
              <li key={idx} className="flex flex-col gap-0.5">
                <span className="font-medium text-foreground">{rej.approach}</span>
                <span className="text-[11px] text-muted-foreground">
                  Reason: {rej.reason_rejected}
                </span>
              </li>
            ))}
          </ul>
        </section>
      )}

      {/* Open Questions & Action Items */}
      <div className="grid gap-4 sm:grid-cols-2">
        {conv.open_questions.length > 0 && (
          <section className="space-y-2 rounded-lg border border-border bg-muted/20 p-3">
            <h4 className="flex items-center gap-1.5 font-semibold text-foreground">
              <HelpCircle className="h-3.5 w-3.5 text-primary" />
              <span>Open Questions ({conv.open_questions.length})</span>
            </h4>
            <ul className="space-y-1.5 pl-1">
              {conv.open_questions.map((q) => (
                <li key={q.id} className="flex items-start gap-2">
                  <span className="mt-1 h-1.5 w-1.5 shrink-0 rounded-full bg-primary" />
                  <div>
                    <p className="font-medium text-foreground">{q.question}</p>
                    {q.context_note && (
                      <p className="text-[11px] text-muted-foreground">{q.context_note}</p>
                    )}
                  </div>
                </li>
              ))}
            </ul>
          </section>
        )}

        {conv.action_items.length > 0 && (
          <section className="space-y-2 rounded-lg border border-border bg-muted/20 p-3">
            <h4 className="flex items-center gap-1.5 font-semibold text-foreground">
              <ListChecks className="h-3.5 w-3.5 text-emerald-500" />
              <span>Next Actions ({conv.action_items.length})</span>
            </h4>
            <ul className="space-y-1.5 pl-1">
              {conv.action_items.map((act) => (
                <li key={act.id} className="flex items-start gap-2">
                  <span className="mt-1 h-1.5 w-1.5 shrink-0 rounded-full bg-emerald-500" />
                  <div className="flex-1">
                    <p className="text-foreground">{act.description}</p>
                    {act.owner && (
                      <span className="text-[10px] text-muted-foreground">
                        Owner: {act.owner}
                      </span>
                    )}
                  </div>
                </li>
              ))}
            </ul>
          </section>
        )}
      </div>

      {/* Key Artifacts */}
      {conv.key_artifacts.length > 0 && (
        <section className="space-y-2">
          <h4 className="flex items-center gap-1.5 font-semibold text-foreground">
            <FileCode className="h-3.5 w-3.5 text-primary" />
            <span>Artifacts & Code Modules ({conv.key_artifacts.length})</span>
          </h4>
          <div className="flex flex-wrap gap-2">
            {conv.key_artifacts.map((art, idx) => (
              <div
                key={idx}
                className="flex items-center gap-2 rounded-md border border-border bg-card px-2.5 py-1.5 shadow-sm"
              >
                <code className="font-mono text-[11px] text-primary">{art.name}</code>
                <span className="rounded bg-muted px-1.5 py-0.5 text-[9px] uppercase tracking-wider text-muted-foreground">
                  {art.kind}
                </span>
                {art.description && (
                  <span className="text-[11px] text-muted-foreground truncate max-w-xs">
                    {art.description}
                  </span>
                )}
              </div>
            ))}
          </div>
        </section>
      )}
    </div>
  );
};
