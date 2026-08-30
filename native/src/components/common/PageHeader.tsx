import React from 'react';
import { Badge } from '@/components/ui/badge';
import { LucideIcon } from 'lucide-react';
import { cn } from '@/lib/utils';

export interface PageHeaderBadge {
  label: string;
  icon?: LucideIcon;
  variant?: 'emerald' | 'default' | 'purple' | 'amber' | 'outline' | 'secondary' | 'destructive';
}

export interface PageHeaderProps {
  kicker?: string;
  badge?: PageHeaderBadge;
  title: string;
  highlightText?: string;
  description?: string | React.ReactNode;
  variant?: 'banner' | 'minimal';
  glowColor?: 'emerald' | 'primary' | 'purple' | 'amber' | 'none';
  children?: React.ReactNode;
  className?: string;
}

export const PageHeader: React.FC<PageHeaderProps> = ({
  kicker,
  badge,
  title,
  highlightText,
  description,
  variant = 'banner',
  glowColor = 'none',
  children,
  className,
}) => {
  if (variant === 'minimal') {
    return (
      <div className={cn("space-y-1 mb-5 shrink-0", className)}>
        {kicker && (
          <p className="font-mono text-[10px] font-semibold text-muted-foreground uppercase tracking-widest mb-1">
            {kicker}
          </p>
        )}
        {badge && (
          <div className="flex items-center gap-2 mb-1.5">
            <Badge
              variant={badge.variant || 'outline'}
              className="text-[10px] font-mono uppercase tracking-wider gap-1.5 py-0.5 px-2"
            >
              {badge.icon && <badge.icon className="w-3 h-3" />}
              <span>{badge.label}</span>
            </Badge>
          </div>
        )}
        <div className="flex flex-col sm:flex-row sm:items-center justify-between gap-3">
          <div>
            <h1 className="text-xl md:text-2xl font-extrabold tracking-tight text-foreground">
              {title}
              {highlightText && (
                <>
                  {' '}
                  <span className="italic text-primary">{highlightText}</span>
                </>
              )}
            </h1>
            {description && (
              <p className="text-xs text-muted-foreground max-w-2xl leading-relaxed mt-1">
                {description}
              </p>
            )}
          </div>
          {children && <div className="flex items-center gap-2 shrink-0">{children}</div>}
        </div>
      </div>
    );
  }

  const glowClass = {
    emerald: 'bg-emerald-500/10 to-emerald-500/5',
    primary: 'bg-primary/10 to-primary/5',
    purple: 'bg-purple-500/10 to-purple-500/5',
    amber: 'bg-amber-500/10 to-amber-500/5',
    none: 'bg-transparent',
  }[glowColor];

  return (
    <div
      className={cn(
        "relative rounded-lg border border-border bg-gradient-to-br from-card via-card/95 to-card/90 p-5 md:p-6 shadow-xs overflow-hidden mb-5 shrink-0",
        className
      )}
    >
      {glowColor !== 'none' && (
        <div
          className={cn(
            "absolute -right-10 -top-10 w-40 h-40 rounded-full blur-3xl pointer-events-none",
            glowClass
          )}
        />
      )}

      <div className="relative z-10 flex flex-col md:flex-row md:items-center justify-between gap-4">
        <div className="space-y-1.5">
          {kicker && (
            <p className="font-mono text-[10px] font-semibold text-muted-foreground uppercase tracking-widest">
              {kicker}
            </p>
          )}

          {badge && (
            <div className="flex items-center gap-2">
              <Badge
                variant={badge.variant || 'outline'}
                className="text-[10px] font-mono uppercase tracking-wider gap-1.5 py-0.5 px-2"
              >
                {badge.icon && <badge.icon className="w-3 h-3" />}
                <span>{badge.label}</span>
              </Badge>
            </div>
          )}

          <h1 className="text-xl md:text-2xl font-extrabold tracking-tight text-foreground">
            {title}
            {highlightText && (
              <>
                {' '}
                <span className="italic text-primary">{highlightText}</span>
              </>
            )}
          </h1>

          {description && (
            <div className="text-xs text-muted-foreground max-w-2xl leading-relaxed">
              {description}
            </div>
          )}
        </div>

        {children && <div className="flex items-center gap-2 shrink-0">{children}</div>}
      </div>
    </div>
  );
};
