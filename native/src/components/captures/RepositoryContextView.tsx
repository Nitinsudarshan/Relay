import React from 'react';
import {
  Boxes,
  CheckCircle2,
  Layers,
  RefreshCw,
  Scale,
  Sparkles,
  Target,
  Users,
} from 'lucide-react';
import type { RepositoryContext } from '../../types';

interface RepositoryContextViewProps {
  context: RepositoryContext;
  analyzing: boolean;
  onAnalyze: () => Promise<void>;
}

export const RepositoryContextView: React.FC<RepositoryContextViewProps> = ({
  context,
  analyzing,
  onAnalyze,
}) => {
  const {
    repository_name,
    objective,
    stack = [],
    features = [],
    user_base = [],
    licensing,
    deterministic,
    model,
  } = context;

  return (
    <div className="space-y-6 text-xs leading-relaxed">
      {/* 1. Objective */}
      <section className="rounded-lg border border-border bg-muted/30 p-4">
        <div className="flex flex-wrap items-center justify-between gap-2 border-b border-border/60 pb-3">
          <div className="flex items-center gap-2">
            <Target className="h-4 w-4 text-primary" />
            <h3 className="font-semibold text-foreground">Objective</h3>
            {repository_name && (
              <span className="rounded bg-muted px-2 py-0.5 font-mono text-[11px] text-muted-foreground">
                {repository_name}
              </span>
            )}
          </div>
          <div className="flex items-center gap-2">
            <span className="rounded bg-primary/10 px-2 py-0.5 text-[10px] font-medium text-primary">
              {deterministic ? 'Deterministic Analysis' : `AI: ${model ?? 'Enriched'}`}
            </span>
            <button
              type="button"
              disabled={analyzing}
              onClick={onAnalyze}
              className="inline-flex items-center gap-1 rounded border border-border bg-card px-2 py-1 text-[11px] font-medium text-muted-foreground transition-colors hover:bg-muted hover:text-foreground disabled:opacity-50"
              title="Re-analyze repository context with current LLM"
            >
              <RefreshCw className={`h-3 w-3 ${analyzing ? 'animate-spin' : ''}`} />
              <span>{analyzing ? 'Analyzing…' : 'Re-analyze'}</span>
            </button>
          </div>
        </div>
        <p className="mt-2.5 text-foreground font-medium leading-relaxed">{objective}</p>
      </section>

      {/* 2. Stack (Only where supported by captured evidence) */}
      {stack.length > 0 && (
        <section className="space-y-2.5">
          <h4 className="flex items-center gap-1.5 font-semibold text-foreground">
            <Layers className="h-4 w-4 text-primary" />
            <span>Stack</span>
          </h4>
          <div className="rounded-lg border border-border bg-card p-3 shadow-sm">
            <ul className="grid gap-2 sm:grid-cols-2">
              {stack.map((item, idx) => (
                <li key={idx} className="flex items-center gap-2 text-foreground">
                  <span className="h-1.5 w-1.5 shrink-0 rounded-full bg-primary" />
                  <span className="font-medium text-[11px]">{item}</span>
                </li>
              ))}
            </ul>
          </div>
        </section>
      )}

      {/* 3. Features / Ecosystem */}
      {features.length > 0 && (
        <section className="space-y-2.5">
          <h4 className="flex items-center gap-1.5 font-semibold text-foreground">
            <Boxes className="h-4 w-4 text-primary" />
            <span>Features / Ecosystem</span>
          </h4>
          <div className="rounded-lg border border-border bg-card p-3 shadow-sm">
            <ul className="grid gap-2 sm:grid-cols-2">
              {features.map((feat, idx) => {
                const isEcosystem = feat.toLowerCase().includes('cli') || feat.toLowerCase().includes('agent');
                return (
                  <li
                    key={idx}
                    className={`flex items-start gap-2 rounded-md p-2 transition-colors ${
                      isEcosystem
                        ? 'border border-primary/20 bg-primary/5'
                        : 'border border-border/50 bg-muted/20'
                    }`}
                  >
                    <CheckCircle2
                      className={`mt-0.5 h-3.5 w-3.5 shrink-0 ${
                        isEcosystem ? 'text-primary' : 'text-muted-foreground'
                      }`}
                    />
                    <span className="font-medium text-foreground text-[11px] leading-snug">
                      {feat}
                    </span>
                  </li>
                );
              })}
            </ul>
          </div>
        </section>
      )}

      {/* 4. User Base */}
      {user_base.length > 0 && (
        <section className="space-y-2.5">
          <h4 className="flex items-center gap-1.5 font-semibold text-foreground">
            <Users className="h-4 w-4 text-primary" />
            <span>User Base</span>
          </h4>
          <div className="rounded-lg border border-border bg-card p-3 shadow-sm">
            <ul className="flex flex-wrap gap-2">
              {user_base.map((user, idx) => (
                <li
                  key={idx}
                  className="inline-flex items-center gap-1.5 rounded-md border border-border bg-muted/30 px-2.5 py-1 text-[11px] font-medium text-foreground"
                >
                  <span className="h-1.5 w-1.5 rounded-full bg-emerald-500" />
                  <span>{user}</span>
                </li>
              ))}
            </ul>
          </div>
        </section>
      )}

      {/* 5. Licensing */}
      {licensing && (
        <section className="space-y-2.5">
          <h4 className="flex items-center gap-1.5 font-semibold text-foreground">
            <Scale className="h-4 w-4 text-primary" />
            <span>Licensing</span>
          </h4>
          <div className="rounded-lg border border-border bg-card p-3 shadow-sm">
            <div className="inline-flex items-center gap-2 rounded-md border border-border bg-muted/30 px-3 py-1.5 text-[11px] font-medium text-foreground">
              <Scale className="h-3.5 w-3.5 text-primary" />
              <span>{licensing}</span>
            </div>
          </div>
        </section>
      )}
    </div>
  );
};
