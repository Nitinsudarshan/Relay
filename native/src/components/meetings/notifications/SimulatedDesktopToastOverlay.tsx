import React, { useState, useEffect } from 'react';
import { X, Pause, Play, Monitor } from 'lucide-react';
import { Button } from '@/components/ui/button';

interface ToastOverlayProps {
  variantId: string;
  variantName: string;
  onClose: () => void;
  children: React.ReactNode;
}

export const SimulatedDesktopToastOverlay: React.FC<ToastOverlayProps> = ({
  variantId,
  variantName,
  onClose,
  children,
}) => {
  const [hovered, setHovered] = useState(false);
  const [progress, setProgress] = useState(100);

  useEffect(() => {
    if (hovered) return;
    const startTime = Date.now();
    const duration = 5000; // 5 seconds OS auto-dismiss

    const interval = setInterval(() => {
      const elapsed = Date.now() - startTime;
      const remaining = Math.max(0, 100 - (elapsed / duration) * 100);
      setProgress(remaining);

      if (remaining <= 0) {
        clearInterval(interval);
        onClose();
      }
    }, 50);

    return () => clearInterval(interval);
  }, [hovered, onClose]);

  return (
    <div
      onMouseEnter={() => setHovered(true)}
      onMouseLeave={() => setHovered(false)}
      className="fixed top-6 right-6 z-50 max-w-[420px] w-full animate-in slide-in-from-top-6 fade-in duration-300 pointer-events-auto"
    >
      <div className="rounded-xl border border-border/80 bg-card/95 backdrop-blur-md p-3 shadow-2xl space-y-2 ring-1 ring-primary/20">
        {/* OS Floating Banner Header */}
        <div className="flex items-center justify-between px-1 text-[11px] font-mono text-muted-foreground border-b border-border/40 pb-1.5">
          <div className="flex items-center gap-1.5">
            <Monitor className="w-3.5 h-3.5 text-primary" />
            <span className="font-semibold text-foreground">
              OS Toast Simulation (Variant {variantId} — {variantName})
            </span>
          </div>
          <div className="flex items-center gap-1">
            {hovered && (
              <span className="text-[10px] text-amber-500 font-sans flex items-center gap-1">
                <Pause className="w-2.5 h-2.5" /> Paused
              </span>
            )}
            <button
              type="button"
              onClick={onClose}
              className="text-muted-foreground hover:text-foreground p-0.5 rounded transition-colors"
              title="Close simulation overlay"
            >
              <X className="w-3.5 h-3.5" />
            </button>
          </div>
        </div>

        {/* Selected Component Variant Live Preview */}
        <div className="p-1 flex justify-center">{children}</div>

        {/* 5-second OS Auto-Dismiss Progress Bar */}
        <div className="w-full bg-muted/60 h-1 rounded-full overflow-hidden">
          <div
            className={`h-full transition-all duration-75 ${
              hovered ? 'bg-amber-500' : 'bg-primary'
            }`}
            style={{ width: `${progress}%` }}
          />
        </div>
      </div>
    </div>
  );
};
