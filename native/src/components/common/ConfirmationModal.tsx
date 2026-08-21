import React, { useEffect } from 'react';
import { AlertTriangle, Info, Trash2, X } from 'lucide-react';
import { Button } from '@/components/ui/button';

interface ConfirmationModalProps {
  isOpen: boolean;
  title: string;
  description: string;
  confirmLabel?: string;
  cancelLabel?: string;
  variant?: 'destructive' | 'default' | 'primary';
  isBusy?: boolean;
  onConfirm: () => void;
  onCancel: () => void;
}

export const ConfirmationModal: React.FC<ConfirmationModalProps> = ({
  isOpen,
  title,
  description,
  confirmLabel = 'Confirm',
  cancelLabel = 'Cancel',
  variant = 'destructive',
  isBusy = false,
  onConfirm,
  onCancel,
}) => {
  // Listen for Escape key
  useEffect(() => {
    if (!isOpen) return;

    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.key === 'Escape' && !isBusy) {
        onCancel();
      }
    };

    window.addEventListener('keydown', handleKeyDown);
    return () => window.removeEventListener('keydown', handleKeyDown);
  }, [isOpen, isBusy, onCancel]);

  if (!isOpen) return null;

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center p-4">
      {/* Viewport Backdrop */}
      <div
        className="fixed inset-0 bg-black/60 backdrop-blur-xs transition-opacity animate-in fade-in duration-150"
        onClick={() => !isBusy && onCancel()}
      />

      {/* Centered Modal Card */}
      <div className="relative w-full max-w-md bg-card border border-border rounded-lg shadow-2xl overflow-hidden z-10 animate-in zoom-in-95 duration-150 p-6 space-y-4">
        <div className="flex items-start gap-3.5">
          <div
            className={`p-2.5 rounded-lg shrink-0 ${
              variant === 'destructive'
                ? 'bg-destructive/10 text-destructive'
                : 'bg-primary/10 text-primary'
            }`}
          >
            {variant === 'destructive' ? (
              <AlertTriangle className="w-5 h-5" />
            ) : (
              <Info className="w-5 h-5" />
            )}
          </div>

          <div className="flex-1 min-w-0">
            <h3 className="text-sm font-bold text-foreground">{title}</h3>
            <p className="text-xs text-muted-foreground mt-1.5 leading-relaxed">
              {description}
            </p>
          </div>

          <button
            onClick={() => !isBusy && onCancel()}
            className="text-muted-foreground hover:text-foreground p-1 rounded-lg hover:bg-muted shrink-0"
            disabled={isBusy}
          >
            <X className="w-4 h-4" />
          </button>
        </div>

        {/* Modal Actions */}
        <div className="flex items-center justify-end gap-2 pt-2 border-t border-border/50">
          <Button
            size="sm"
            variant="ghost"
            onClick={onCancel}
            disabled={isBusy}
            className="h-8 text-xs"
          >
            {cancelLabel}
          </Button>

          <Button
            size="sm"
            variant={variant === 'destructive' ? 'destructive' : 'default'}
            onClick={onConfirm}
            disabled={isBusy}
            className="h-8 text-xs font-semibold"
          >
            {confirmLabel}
          </Button>
        </div>
      </div>
    </div>
  );
};
