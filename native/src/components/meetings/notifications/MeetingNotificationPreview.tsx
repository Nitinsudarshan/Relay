import React from 'react';
import { Button } from '@/components/ui/button';
import { Badge } from '@/components/ui/badge';
import { Check } from 'lucide-react';

interface PreviewProps {
  id: string;
  name: string;
  description: string;
  isSelected: boolean;
  onSelect: () => void;
  children: React.ReactNode;
}

export const MeetingNotificationPreview: React.FC<PreviewProps> = ({
  id,
  name,
  description,
  isSelected,
  onSelect,
  children,
}) => {
  return (
    <div
      className={`rounded-lg border transition-all duration-200 flex flex-col bg-card overflow-hidden ${
        isSelected
          ? 'border-primary shadow-md ring-1 ring-primary/40'
          : 'border-border hover:border-border/80'
      }`}
    >
      {/* Header */}
      <div className="px-4 py-3 border-b border-border bg-muted/20 flex items-center justify-between">
        <div className="flex items-center gap-2">
          <span className="flex size-5 items-center justify-center rounded bg-primary/10 text-primary font-mono font-bold text-[11px]">
            {id}
          </span>
          <h3 className="text-xs font-bold text-foreground">{name}</h3>
        </div>
        <Button
          type="button"
          variant={isSelected ? 'default' : 'outline'}
          size="sm"
          onClick={onSelect}
          className="h-6 text-[11px] px-2 font-medium rounded-md gap-1"
        >
          {isSelected ? (
            <>
              <Check className="w-3 h-3" /> Selected
            </>
          ) : (
            'Select Direction'
          )}
        </Button>
      </div>

      {/* Live Preview Area */}
      <div className="p-6 flex-1 flex items-center justify-center bg-muted/10 relative overflow-hidden min-h-[160px]">
        {children}
      </div>

      {/* Footer Description */}
      <div className="px-4 py-2.5 border-t border-border bg-card">
        <p className="text-[11px] text-muted-foreground leading-snug">{description}</p>
      </div>
    </div>
  );
};
