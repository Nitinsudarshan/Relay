import React from 'react';
import { Clock, Link2, Loader2, Tags, Type } from 'lucide-react';

import { formatCount, formatDurationShort, type HomeStat, type HomeSurface, type HomeVitals } from './homeStats';

export interface HomeLibraryStatsProps {
  stats: HomeStat[];
  vitals: HomeVitals;
  loading: boolean;
  onNavigate: (surface: HomeSurface) => void;
}

interface VitalRow {
  label: string;
  value: string;
  icon: typeof Clock;
}

/**
 * What is in the vault, as numbers.
 *
 * Presentational only — every figure arrives already derived from
 * `homeStats.ts`, so there is nothing here to disagree with the surfaces the
 * tiles link to. Each tile is a real button: the count and the way to the
 * records behind it are the same control.
 */
export const HomeLibraryStats: React.FC<HomeLibraryStatsProps> = ({
  stats,
  vitals,
  loading,
  onNavigate,
}) => {
  const vitalRows: VitalRow[] = [
    { label: 'Words transcribed', value: formatCount(vitals.spokenWords), icon: Type },
    { label: 'Recorded', value: formatDurationShort(vitals.recordedSeconds), icon: Clock },
    { label: 'Connected thoughts', value: formatCount(vitals.connectedScribbles), icon: Link2 },
    { label: 'Distinct topics', value: formatCount(vitals.distinctTopics), icon: Tags },
  ];

  return (
    <section className="space-y-2.5">
      <div className="flex items-center gap-2">
        <h2 className="text-[10px] font-mono uppercase tracking-widest text-muted-foreground">
          Your library
        </h2>
        {loading && <Loader2 className="w-3 h-3 animate-spin text-muted-foreground" />}
      </div>

      {/* Counters — one per surface, each one the way in. */}
      <div className="grid grid-cols-2 sm:grid-cols-4 gap-2.5">
        {stats.map((stat) => (
          <button
            key={stat.id}
            type="button"
            onClick={() => onNavigate(stat.surface)}
            title={stat.hint}
            className="rounded-lg border border-border bg-card p-3.5 text-left hover:bg-muted/40 hover:border-primary/50 transition-all focus:outline-none focus-visible:ring-2 focus-visible:ring-ring"
          >
            <p className="text-[10px] font-mono uppercase tracking-widest text-muted-foreground mb-1.5 truncate">
              {stat.label}
            </p>
            <p className="text-2xl font-extrabold text-foreground leading-none">
              {formatCount(stat.value)}
            </p>
            <p className="text-[10px] text-muted-foreground mt-1.5 truncate">
              {stat.thisWeek > 0 ? (
                <span className="text-emerald-600 dark:text-emerald-400 font-medium">
                  +{formatCount(stat.thisWeek)} this week
                </span>
              ) : (
                stat.hint
              )}
            </p>
          </button>
        ))}
      </div>

      {/* Second-order figures: what was said, and what Relay still owes you. */}
      <div className="rounded-lg border border-border bg-card divide-y divide-border sm:divide-y-0 sm:flex sm:divide-x">
        {vitalRows.map((row) => {
          const Icon = row.icon;
          return (
            <div key={row.label} className="flex items-center gap-2.5 p-3 sm:flex-1 min-w-0">
              <Icon className="w-3.5 h-3.5 text-muted-foreground shrink-0" />
              <div className="min-w-0">
                <p className="text-sm font-bold text-foreground leading-none">{row.value}</p>
                <p className="text-[10px] text-muted-foreground truncate mt-0.5">{row.label}</p>
              </div>
            </div>
          );
        })}
      </div>

      {/* Backlog prompts, shown only when there is actually a backlog. */}
      {(vitals.awaitingPromotion > 0 || vitals.awaitingEnrichment > 0) && (
        <div className="flex flex-wrap gap-2">
          {vitals.awaitingPromotion > 0 && (
            <button
              type="button"
              onClick={() => onNavigate('captures')}
              className="text-[11px] px-2.5 py-1.5 rounded-lg border border-amber-500/30 bg-amber-500/10 text-amber-600 dark:text-amber-400 font-medium hover:bg-amber-500/20 transition-colors"
            >
              {formatCount(vitals.awaitingPromotion)}{' '}
              {vitals.awaitingPromotion === 1
                ? 'capture or document has'
                : 'captures and documents have'}{' '}
              no Scribble yet
            </button>
          )}
          {vitals.awaitingEnrichment > 0 && (
            <button
              type="button"
              onClick={() => onNavigate('scribble')}
              className="text-[11px] px-2.5 py-1.5 rounded-lg border border-border bg-muted/40 text-muted-foreground font-medium hover:bg-muted/70 transition-colors"
            >
              {formatCount(vitals.awaitingEnrichment)} scribble
              {vitals.awaitingEnrichment === 1 ? '' : 's'} awaiting AI enrichment
            </button>
          )}
        </div>
      )}
    </section>
  );
};
