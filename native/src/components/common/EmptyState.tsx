import React from 'react';
import { LucideIcon } from 'lucide-react';
import { cn } from '@/lib/utils';

export interface EmptyStateProps {
  icon: LucideIcon;
  title: string;
  description?: string | React.ReactNode;
  action?: React.ReactNode;
  className?: string;
  minHeight?: string;
}

export const EmptyState: React.FC<EmptyStateProps> = ({
  icon: Icon,
  title,
  description,
  action,
  className,
  minHeight = 'min-h-[160px]',
}) => {
  return (
    <div
      className={cn(
        "flex flex-col items-center justify-center text-center p-6 md:p-8 rounded-lg border border-dashed border-border bg-muted/20 text-muted-foreground",
        minHeight,
        className
      )}
    >
      <div className="w-10 h-10 rounded-full bg-muted/60 border border-border/50 flex items-center justify-center mb-2.5 text-muted-foreground/60">
        <Icon className="w-5 h-5" />
      </div>
      <h3 className="text-xs md:text-sm font-semibold text-foreground">{title}</h3>
      {description && (
        <div className="text-[11px] md:text-xs text-muted-foreground max-w-sm mt-1 leading-relaxed">
          {description}
        </div>
      )}
      {action && <div className="mt-3.5">{action}</div>}
    </div>
  );
};
