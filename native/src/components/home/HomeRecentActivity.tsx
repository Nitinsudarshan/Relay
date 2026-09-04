import React from 'react';
import { Calendar, FileText, Globe, History, Mic, Sparkles, type LucideIcon } from 'lucide-react';

import { EmptyState } from '@/components/common/EmptyState';

import { formatRelativeTime, type HomeActivityItem, type HomeActivityKind, type HomeSurface } from './homeStats';

export interface HomeRecentActivityProps {
  items: HomeActivityItem[];
  loading: boolean;
  /** Evaluated once by the caller so every row agrees on what "now" is. */
  nowMs: number;
  onNavigate: (surface: HomeSurface) => void;
}

const KIND_ICON: Record<HomeActivityKind, LucideIcon> = {
  voice_note: Mic,
  scribble: Sparkles,
  meeting: Calendar,
  file: FileText,
  capture: Globe,
};

const KIND_ACCENT: Record<HomeActivityKind, string> = {
  voice_note: 'text-emerald-500',
  scribble: 'text-amber-500',
  meeting: 'text-indigo-400',
  file: 'text-blue-500',
  capture: 'text-sky-500',
};

/**
 * The newest records across every surface.
 *
 * Titles come from the vault verbatim — including a captured page's own title,
 * which is external untrusted text (`docs/capture.md`) and is rendered as text,
 * never interpreted. A row opens the surface that owns the record.
 */
export const HomeRecentActivity: React.FC<HomeRecentActivityProps> = ({
  items,
  loading,
  nowMs,
  onNavigate,
}) => (
  <section className="space-y-2.5 min-w-0">
    <h2 className="text-[10px] font-mono uppercase tracking-widest text-muted-foreground">
      Latest activity
    </h2>

    <div className="rounded-lg border border-border bg-card overflow-hidden">
      {items.length === 0 ? (
        <EmptyState
          icon={History}
          title={loading ? 'Reading the vault…' : 'Nothing captured yet'}
          description={
            loading
              ? 'Counting what is already there.'
              : 'Dictate a thought, record a meeting or import a document and it will show up here.'
          }
          minHeight="min-h-[160px]"
          className="border-none bg-transparent"
        />
      ) : (
        <ul className="divide-y divide-border">
          {items.map((item) => {
            const Icon = KIND_ICON[item.kind];
            return (
              <li key={`${item.kind}_${item.id}`}>
                <button
                  type="button"
                  onClick={() => onNavigate(item.surface)}
                  className="w-full flex items-center gap-3 p-3 text-left hover:bg-muted/40 transition-colors focus:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-inset"
                >
                  <Icon className={`w-3.5 h-3.5 shrink-0 ${KIND_ACCENT[item.kind]}`} />

                  <div className="min-w-0 flex-1">
                    <p className="text-xs font-semibold text-foreground truncate">{item.title}</p>
                    <p className="text-[10px] text-muted-foreground truncate">{item.detail}</p>
                  </div>

                  <span className="text-[10px] font-mono text-muted-foreground shrink-0">
                    {formatRelativeTime(item.createdAt, nowMs)}
                  </span>
                </button>
              </li>
            );
          })}
        </ul>
      )}
    </div>
  </section>
);
