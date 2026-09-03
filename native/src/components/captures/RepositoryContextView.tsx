import React from 'react';
import {
  AlertCircle,
  Boxes,
  CheckCircle2,
  Code2,
  Database,
  GitPullRequest,
  Globe,
  Layers,
  RefreshCw,
  Sparkles,
  Target,
  Terminal,
  TestTube2,
  Users,
  Wrench,
} from 'lucide-react';
import type { RepositoryContext, RepositoryIssue } from '../../types';

/**
 * The three states an issue list can be in, kept apart because collapsing them
 * is the failure this view exists to avoid.
 *
 * `unavailable` means Relay never saw issue data — a partial capture, or a page
 * that does not carry it. `empty` means Relay saw the evidence and there was
 * nothing in it. Reporting an empty tracker as "not captured" understates what
 * Relay knows; reporting an uncaptured one as "none" is a fabricated fact.
 */
type IssueEvidence = 'unavailable' | 'empty' | 'present';

function issueEvidence(available: boolean, issues: RepositoryIssue[]): IssueEvidence {
  if (!available) return 'unavailable';
  return issues.length > 0 ? 'present' : 'empty';
}

interface IssueSectionProps {
  title: string;
  icon: React.ReactNode;
  issues: RepositoryIssue[];
  available: boolean;
  defaultStatus: string;
  statusClassName: string;
  /** What is shown when Relay looked and found nothing. */
  emptyHeadline: string;
  /** What is shown when Relay never had the evidence to look at. */
  unavailableHeadline: string;
  unavailableDetail: string;
  /** Rendered instead of `description` for resolved items. */
  detailField: 'description' | 'resolution';
}

const IssueSection: React.FC<IssueSectionProps> = ({
  title,
  icon,
  issues,
  available,
  defaultStatus,
  statusClassName,
  emptyHeadline,
  unavailableHeadline,
  unavailableDetail,
  detailField,
}) => {
  const evidence = issueEvidence(available, issues);

  return (
    <section className="space-y-3">
      <h4 className="flex items-center gap-1.5 font-semibold text-foreground">
        {icon}
        <span>
          {title} {available ? `(${issues.length})` : ''}
        </span>
      </h4>

      {evidence === 'present' && (
        <div className="space-y-2">
          {issues.map((issue, idx) => {
            const detail = detailField === 'resolution' ? issue.resolution : issue.description;
            return (
              <div
                key={`${issue.title}-${idx}`}
                className="rounded-lg border border-border bg-card p-3 shadow-sm space-y-1"
              >
                <div className="flex items-center gap-2">
                  {issue.number && (
                    <span className="font-mono text-xs font-semibold text-primary">
                      #{issue.number}
                    </span>
                  )}
                  <span className="font-medium text-foreground">{issue.title}</span>
                  {issue.issue_type && (
                    <span className="rounded bg-muted px-1.5 py-0.2 text-[10px] text-muted-foreground">
                      {issue.issue_type}
                    </span>
                  )}
                  <span className={`rounded px-1.5 py-0.2 text-[10px] ${statusClassName}`}>
                    {issue.status ?? defaultStatus}
                  </span>
                </div>
                {detail && (
                  <p className="text-[11px] text-muted-foreground">
                    {detailField === 'resolution' ? `Resolution: ${detail}` : detail}
                  </p>
                )}
              </div>
            );
          })}
        </div>
      )}

      {evidence === 'empty' && (
        <div className="rounded-lg border border-border/80 bg-muted/20 p-3 text-muted-foreground">
          <p className="font-medium text-foreground/80">{emptyHeadline}</p>
          <p className="mt-0.5 text-[11px]">
            Relay read the issue evidence in this capture and found no matching entries.
          </p>
        </div>
      )}

      {evidence === 'unavailable' && (
        <div className="rounded-lg border border-dashed border-border/80 bg-muted/20 p-3 text-muted-foreground">
          <p className="font-medium text-foreground/80">{unavailableHeadline}</p>
          <p className="mt-0.5 text-[11px]">{unavailableDetail}</p>
        </div>
      )}
    </section>
  );
};

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
    stack,
    features,
    user_base,
    open_issues,
    past_issues,
    open_issues_available,
    past_issues_available,
    deterministic,
    model,
  } = context;

  const coreFeatures = features.filter((f) => f.is_core !== false);
  const supportingFeatures = features.filter((f) => f.is_core === false);

  const hasStackItems =
    stack.languages.length > 0 ||
    stack.frontend.length > 0 ||
    stack.backend.length > 0 ||
    stack.storage.length > 0 ||
    stack.testing.length > 0 ||
    stack.integrations.length > 0;

  return (
    <div className="space-y-6 text-xs leading-relaxed">
      {/* Objective */}
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

      {/* Stack */}
      <section className="space-y-3">
        <h4 className="flex items-center gap-1.5 font-semibold text-foreground">
          <Layers className="h-4 w-4 text-primary" />
          <span>Stack</span>
        </h4>

        {hasStackItems ? (
          <div className="grid gap-3 sm:grid-cols-2 lg:grid-cols-3">
            {stack.languages.length > 0 && (
              <div className="rounded-lg border border-border bg-card p-3 shadow-sm">
                <div className="flex items-center gap-1.5 text-muted-foreground font-medium">
                  <Code2 className="h-3.5 w-3.5 text-primary" />
                  <span>Languages</span>
                </div>
                <div className="mt-2 flex flex-wrap gap-1.5">
                  {stack.languages.map((item) => (
                    <span
                      key={item}
                      className="rounded bg-muted px-2 py-0.5 font-mono text-[11px] text-foreground"
                    >
                      {item}
                    </span>
                  ))}
                </div>
              </div>
            )}

            {stack.frontend.length > 0 && (
              <div className="rounded-lg border border-border bg-card p-3 shadow-sm">
                <div className="flex items-center gap-1.5 text-muted-foreground font-medium">
                  <Globe className="h-3.5 w-3.5 text-sky-500" />
                  <span>Frontend</span>
                </div>
                <div className="mt-2 flex flex-wrap gap-1.5">
                  {stack.frontend.map((item) => (
                    <span
                      key={item}
                      className="rounded bg-sky-500/10 px-2 py-0.5 text-[11px] text-sky-700 dark:text-sky-300 font-medium"
                    >
                      {item}
                    </span>
                  ))}
                </div>
              </div>
            )}

            {stack.backend.length > 0 && (
              <div className="rounded-lg border border-border bg-card p-3 shadow-sm">
                <div className="flex items-center gap-1.5 text-muted-foreground font-medium">
                  <Terminal className="h-3.5 w-3.5 text-indigo-500" />
                  <span>Backend & Native</span>
                </div>
                <div className="mt-2 flex flex-wrap gap-1.5">
                  {stack.backend.map((item) => (
                    <span
                      key={item}
                      className="rounded bg-indigo-500/10 px-2 py-0.5 text-[11px] text-indigo-700 dark:text-indigo-300 font-medium"
                    >
                      {item}
                    </span>
                  ))}
                </div>
              </div>
            )}

            {stack.storage.length > 0 && (
              <div className="rounded-lg border border-border bg-card p-3 shadow-sm">
                <div className="flex items-center gap-1.5 text-muted-foreground font-medium">
                  <Database className="h-3.5 w-3.5 text-amber-500" />
                  <span>Storage</span>
                </div>
                <div className="mt-2 flex flex-wrap gap-1.5">
                  {stack.storage.map((item) => (
                    <span
                      key={item}
                      className="rounded bg-amber-500/10 px-2 py-0.5 text-[11px] text-amber-700 dark:text-amber-300 font-medium"
                    >
                      {item}
                    </span>
                  ))}
                </div>
              </div>
            )}

            {stack.testing.length > 0 && (
              <div className="rounded-lg border border-border bg-card p-3 shadow-sm">
                <div className="flex items-center gap-1.5 text-muted-foreground font-medium">
                  <TestTube2 className="h-3.5 w-3.5 text-emerald-500" />
                  <span>Testing</span>
                </div>
                <div className="mt-2 flex flex-wrap gap-1.5">
                  {stack.testing.map((item) => (
                    <span
                      key={item}
                      className="rounded bg-emerald-500/10 px-2 py-0.5 text-[11px] text-emerald-700 dark:text-emerald-300 font-medium"
                    >
                      {item}
                    </span>
                  ))}
                </div>
              </div>
            )}

            {stack.integrations.length > 0 && (
              <div className="rounded-lg border border-border bg-card p-3 shadow-sm">
                <div className="flex items-center gap-1.5 text-muted-foreground font-medium">
                  <Wrench className="h-3.5 w-3.5 text-purple-500" />
                  <span>Integrations & Tooling</span>
                </div>
                <div className="mt-2 flex flex-wrap gap-1.5">
                  {stack.integrations.map((item) => (
                    <span
                      key={item}
                      className="rounded bg-purple-500/10 px-2 py-0.5 text-[11px] text-purple-700 dark:text-purple-300 font-medium"
                    >
                      {item}
                    </span>
                  ))}
                </div>
              </div>
            )}
          </div>
        ) : (
          <div className="rounded-lg border border-border/80 bg-muted/20 p-3 text-muted-foreground">
            No stack details could be confidently identified from the captured evidence.
          </div>
        )}
      </section>

      {/* Features */}
      <section className="space-y-3">
        <h4 className="flex items-center gap-1.5 font-semibold text-foreground">
          <Boxes className="h-4 w-4 text-primary" />
          <span>Features ({features.length})</span>
        </h4>

        {features.length > 0 ? (
          <div className="space-y-2">
            {coreFeatures.map((feat, idx) => (
              <div
                key={`${feat.name}-${idx}`}
                className="flex items-start gap-2.5 rounded-lg border border-border bg-card p-3 shadow-sm"
              >
                <CheckCircle2 className="mt-0.5 h-4 w-4 shrink-0 text-emerald-500" />
                <div className="space-y-0.5">
                  <div className="flex items-center gap-2">
                    <span className="font-semibold text-foreground">{feat.name}</span>
                    <span className="rounded bg-emerald-500/10 px-1.5 py-0.2 text-[10px] font-medium text-emerald-600 dark:text-emerald-400">
                      Core
                    </span>
                  </div>
                  {feat.description && (
                    <p className="text-muted-foreground text-[11px] leading-relaxed">
                      {feat.description}
                    </p>
                  )}
                </div>
              </div>
            ))}

            {supportingFeatures.map((feat, idx) => (
              <div
                key={`supp-${feat.name}-${idx}`}
                className="flex items-start gap-2.5 rounded-lg border border-border/70 bg-muted/20 p-3"
              >
                <Sparkles className="mt-0.5 h-3.5 w-3.5 shrink-0 text-muted-foreground" />
                <div className="space-y-0.5">
                  <div className="flex items-center gap-2">
                    <span className="font-medium text-foreground">{feat.name}</span>
                    <span className="rounded bg-muted px-1.5 py-0.2 text-[10px] text-muted-foreground">
                      Supporting
                    </span>
                  </div>
                  {feat.description && (
                    <p className="text-muted-foreground text-[11px] leading-relaxed">
                      {feat.description}
                    </p>
                  )}
                </div>
              </div>
            ))}
          </div>
        ) : (
          <div className="rounded-lg border border-border/80 bg-muted/20 p-3 text-muted-foreground">
            No distinct product features were detailed in the captured repository content.
          </div>
        )}
      </section>

      {/* User Base */}
      <section className="space-y-3">
        <h4 className="flex items-center gap-1.5 font-semibold text-foreground">
          <Users className="h-4 w-4 text-primary" />
          <span>User Base</span>
        </h4>

        <div className="rounded-lg border border-border bg-card p-4 shadow-sm space-y-3">
          {user_base.primary.length > 0 || user_base.secondary.length > 0 ? (
            <>
              {user_base.primary.length > 0 && (
                <div>
                  <span className="text-[11px] font-medium text-muted-foreground">Primary Users</span>
                  <div className="mt-1 flex flex-wrap gap-1.5">
                    {user_base.primary.map((user) => (
                      <span
                        key={user}
                        className="rounded-full bg-primary/10 px-2.5 py-0.5 text-[11px] font-medium text-primary"
                      >
                        {user}
                      </span>
                    ))}
                  </div>
                </div>
              )}

              {user_base.secondary.length > 0 && (
                <div>
                  <span className="text-[11px] font-medium text-muted-foreground">Secondary Users</span>
                  <div className="mt-1 flex flex-wrap gap-1.5">
                    {user_base.secondary.map((user) => (
                      <span
                        key={user}
                        className="rounded-full bg-muted px-2.5 py-0.5 text-[11px] text-muted-foreground"
                      >
                        {user}
                      </span>
                    ))}
                  </div>
                </div>
              )}

              {user_base.evidence && (
                <p className="border-t border-border/60 pt-2 text-[11px] text-muted-foreground italic">
                  Evidence: {user_base.evidence}
                </p>
              )}
            </>
          ) : (
            <p className="text-muted-foreground">
              {user_base.evidence ??
                'The repository does not provide sufficient evidence to confidently identify a primary user group.'}
            </p>
          )}
        </div>
      </section>

      <IssueSection
        title="Open Issues"
        icon={<AlertCircle className="h-4 w-4 text-amber-500" />}
        issues={open_issues}
        available={open_issues_available}
        defaultStatus="Open"
        statusClassName="bg-amber-500/10 text-amber-700 dark:text-amber-400"
        detailField="description"
        emptyHeadline="No open issues in the captured evidence."
        unavailableHeadline="Issue information was not available in the captured repository evidence."
        unavailableDetail="Relay captured repository metadata and documentation, which does not carry raw GitHub issue records. This is not a claim that the repository has no open issues."
      />

      <IssueSection
        title="Past Issues"
        icon={<GitPullRequest className="h-4 w-4 text-indigo-500" />}
        issues={past_issues}
        available={past_issues_available}
        defaultStatus="Resolved"
        statusClassName="bg-emerald-500/10 text-emerald-600 dark:text-emerald-400"
        detailField="resolution"
        emptyHeadline="No resolved issues in the captured evidence."
        unavailableHeadline="No historical issue information was available in the captured repository evidence."
        unavailableDetail="Changelogs, closed pull requests, and release notes were not included in this capture pass. This is not a claim that the repository has no history."
      />
    </div>
  );
};
