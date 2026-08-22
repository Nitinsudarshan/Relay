import React, { useState } from 'react';
import { RelayLogo } from './RelayLogo';
import { Button } from '@/components/ui/button';
import { Badge } from '@/components/ui/badge';
import { ShieldCheck, HardDrive, Sparkles, ArrowRight, RefreshCw, Lock } from 'lucide-react';

interface WelcomeModalProps {
  isOpen: boolean;
  onContinueGoogle: () => Promise<void>;
  onContinueLocally: () => void;
}

export const WelcomeModal: React.FC<WelcomeModalProps> = ({
  isOpen,
  onContinueGoogle,
  onContinueLocally,
}) => {
  const [connecting, setConnecting] = useState(false);
  const [errorMsg, setErrorMsg] = useState<string | null>(null);

  if (!isOpen) return null;

  const handleGoogleClick = async () => {
    try {
      setConnecting(true);
      setErrorMsg(null);
      await onContinueGoogle();
    } catch (err: unknown) {
      console.error('Google Sign-In failed:', err);
      const msg = typeof err === 'string' ? err : (err as { message?: string })?.message || 'Sign-in failed. Please try again.';
      setErrorMsg(msg);
    } finally {
      setConnecting(false);
    }
  };

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-background/85 backdrop-blur-md p-4 animate-in fade-in-50">
      <div className="w-full max-w-lg bg-card/95 border border-border/80 rounded-2xl p-7 md:p-8 shadow-2xl space-y-7 relative overflow-hidden">
        {/* Background ambient glow */}
        <div className="absolute -right-16 -top-16 w-48 h-48 bg-primary/10 rounded-full blur-3xl pointer-events-none" />
        <div className="absolute -left-16 -bottom-16 w-48 h-48 bg-emerald-500/10 rounded-full blur-3xl pointer-events-none" />

        {/* Brand Header */}
        <div className="text-center space-y-3 relative z-10">
          <div className="flex justify-center mb-1">
            <RelayLogo className="w-11 h-11" />
          </div>
          <div className="space-y-1">
            <h2 className="text-2xl font-extrabold tracking-tight text-foreground">
              Your thoughts stay <span className="text-primary italic">yours</span>.
            </h2>
            <p className="text-xs text-muted-foreground max-w-sm mx-auto leading-relaxed">
              Relay is local-first by design. Your notes, voice recordings, and knowledge graph remain on this device.
            </p>
          </div>
        </div>

        {/* Core Pillars */}
        <div className="grid grid-cols-2 gap-3 relative z-10">
          <div className="p-3.5 rounded-xl border border-border/60 bg-muted/30 space-y-1.5 text-left">
            <div className="flex items-center gap-1.5 text-xs font-semibold text-foreground">
              <HardDrive className="w-3.5 h-3.5 text-primary" />
              <span>100% Local Storage</span>
            </div>
            <p className="text-[11px] text-muted-foreground leading-relaxed">
              Voice notes, scribbles, audio, and vector databases stay strictly on your local disk.
            </p>
          </div>

          <div className="p-3.5 rounded-xl border border-border/60 bg-muted/30 space-y-1.5 text-left">
            <div className="flex items-center gap-1.5 text-xs font-semibold text-foreground">
              <ShieldCheck className="w-3.5 h-3.5 text-emerald-500" />
              <span>Privacy Guard</span>
            </div>
            <p className="text-[11px] text-muted-foreground leading-relaxed">
              Relay never uploads your knowledge simply because you sign in or check for updates.
            </p>
          </div>
        </div>

        {errorMsg && (
          <div className="p-3 rounded-lg border border-destructive/30 bg-destructive/10 text-destructive text-xs">
            {errorMsg}
          </div>
        )}

        {/* Action Buttons */}
        <div className="space-y-3 pt-1 relative z-10">
          <Button
            className="w-full h-11 text-xs font-semibold gap-2.5 bg-primary hover:bg-primary/90 text-primary-foreground shadow-sm"
            onClick={handleGoogleClick}
            disabled={connecting}
          >
            {connecting ? (
              <RefreshCw className="w-4 h-4 animate-spin" />
            ) : (
              <svg className="w-4 h-4" viewBox="0 0 24 24">
                <path
                  fill="currentColor"
                  d="M22.56 12.25c0-.78-.07-1.53-.2-2.25H12v4.26h5.92c-.26 1.37-1.04 2.53-2.21 3.31v2.77h3.57c2.08-1.92 3.28-4.74 3.28-8.09z"
                />
                <path
                  fill="currentColor"
                  d="M12 23c2.97 0 5.46-.98 7.28-2.66l-3.57-2.77c-.98.66-2.23 1.06-3.71 1.06-2.86 0-5.29-1.93-6.16-4.53H2.18v2.84C3.99 20.53 7.7 23 12 23z"
                />
                <path
                  fill="currentColor"
                  d="M5.84 14.09c-.22-.66-.35-1.36-.35-2.09s.13-1.43.35-2.09V7.06H2.18C1.43 8.55 1 10.22 1 12s.43 3.45 1.18 4.94l2.85-2.22.81-.63z"
                />
                <path
                  fill="currentColor"
                  d="M12 5.38c1.62 0 3.06.56 4.21 1.64l3.15-3.15C17.45 2.09 14.97 1 12 1 7.7 1 3.99 3.47 2.18 7.06l3.66 2.84c.87-2.6 3.3-4.52 6.16-4.52z"
                />
              </svg>
            )}
            <span>{connecting ? 'Authorizing in Browser...' : 'Continue with Google'}</span>
          </Button>

          <div className="relative flex items-center justify-center">
            <div className="absolute inset-0 flex items-center">
              <div className="w-full border-t border-border/40" />
            </div>
            <span className="relative bg-card px-3 text-[11px] uppercase tracking-wider text-muted-foreground">
              or
            </span>
          </div>

          <Button
            variant="outline"
            className="w-full h-11 text-xs font-semibold gap-2 border-border/80 hover:bg-accent text-foreground"
            onClick={onContinueLocally}
            disabled={connecting}
          >
            <span>Continue Locally (No Account Required)</span>
            <ArrowRight className="w-3.5 h-3.5" />
          </Button>
        </div>

        <p className="text-[10px] text-center text-muted-foreground/80 leading-relaxed">
          Google Sign-In enables automatic updates, diagnostics, and Calendar sync. You can sign in or sign out at any time in Settings.
        </p>
      </div>
    </div>
  );
};
