import React, { useState, useEffect } from 'react';
import { RelayLogo } from './RelayLogo';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { Badge } from '@/components/ui/badge';
import { ShieldCheck, HardDrive, Sparkles, ArrowRight, ArrowLeft, RefreshCw, User } from 'lucide-react';

interface WelcomeModalProps {
  isOpen: boolean;
  initialDisplayName?: string;
  onContinueGoogle: (displayName: string) => Promise<void>;
  onContinueLocally: (displayName: string) => Promise<void> | void;
}

export const WelcomeModal: React.FC<WelcomeModalProps> = ({
  isOpen,
  initialDisplayName = '',
  onContinueGoogle,
  onContinueLocally,
}) => {
  const [step, setStep] = useState<1 | 2>(1);
  const [displayName, setDisplayName] = useState(initialDisplayName);
  const [connecting, setConnecting] = useState(false);
  const [errorMsg, setErrorMsg] = useState<string | null>(null);

  useEffect(() => {
    if (initialDisplayName && !displayName) {
      setDisplayName(initialDisplayName);
    }
  }, [initialDisplayName]);

  // Reset step whenever modal opens
  useEffect(() => {
    if (isOpen) {
      setStep(1);
      setErrorMsg(null);
    }
  }, [isOpen]);

  if (!isOpen) return null;

  const handleStep1Submit = (e?: React.FormEvent) => {
    if (e) e.preventDefault();
    const trimmed = displayName.trim();
    if (!trimmed) return;
    setStep(2);
  };

  const handleGoogleClick = async () => {
    try {
      setConnecting(true);
      setErrorMsg(null);
      await onContinueGoogle(displayName.trim() || 'Local User');
    } catch (err: unknown) {
      console.error('Google Sign-In failed:', err);
      const msg = typeof err === 'string' ? err : (err as { message?: string })?.message || 'Sign-in failed. Please try again.';
      setErrorMsg(msg);
    } finally {
      setConnecting(false);
    }
  };

  const handleLocallyClick = async () => {
    try {
      setConnecting(true);
      setErrorMsg(null);
      await onContinueLocally(displayName.trim() || 'Local User');
    } catch (err: unknown) {
      console.error('Local onboarding failed:', err);
      const msg = typeof err === 'string' ? err : (err as { message?: string })?.message || 'Setup failed. Please try again.';
      setErrorMsg(msg);
    } finally {
      setConnecting(false);
    }
  };

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-background/85 backdrop-blur-md p-4 animate-in fade-in-50">
      <div className="w-full max-w-lg bg-card/95 border border-border/80 rounded-lg p-7 md:p-8 shadow-2xl space-y-6 relative overflow-hidden">
        {/* Background ambient glow */}
        <div className="absolute -right-16 -top-16 w-48 h-48 bg-primary/10 rounded-full blur-3xl pointer-events-none" />
        <div className="absolute -left-16 -bottom-16 w-48 h-48 bg-emerald-500/10 rounded-full blur-3xl pointer-events-none" />

        {/* Brand Header */}
        <div className="text-center space-y-2.5 relative z-10">
          <div className="flex justify-center mb-1">
            <RelayLogo className="w-10 h-10" />
          </div>
          <div className="space-y-1">
            <h2 className="text-2xl font-extrabold tracking-tight text-foreground">
              {step === 1 ? 'Welcome to Relay' : 'How would you like to use Relay?'}
            </h2>
            <p className="text-xs text-muted-foreground max-w-sm mx-auto leading-relaxed">
              {step === 1
                ? 'Your thoughts stay yours. Relay is local-first by design.'
                : 'Choose your operating mode. Your local notes always stay on this device.'}
            </p>
          </div>
        </div>

        {errorMsg && (
          <div className="p-3 rounded-lg border border-destructive/30 bg-destructive/10 text-destructive text-xs">
            {errorMsg}
          </div>
        )}

        {/* STEP 1: PERSONALIZATION */}
        {step === 1 && (
          <form onSubmit={handleStep1Submit} className="space-y-6 relative z-10 animate-in fade-in-50">
            <div className="space-y-2">
              <label htmlFor="display_name_input" className="text-xs font-semibold text-foreground flex items-center gap-1.5">
                <User className="w-3.5 h-3.5 text-primary" />
                <span>What should we call you?</span>
              </label>
              <Input
                id="display_name_input"
                value={displayName}
                onChange={(e) => setDisplayName(e.target.value)}
                placeholder="e.g. Nitin"
                autoFocus
                className="h-11 text-sm bg-muted/40 border-border/80 focus:border-primary px-3.5"
              />
              <p className="text-[11px] text-muted-foreground">
                This is purely for personalization. It is not an account username or cloud identity.
              </p>
            </div>

            <Button
              type="submit"
              className="w-full h-11 text-xs font-semibold gap-2 bg-primary hover:bg-primary/90 text-primary-foreground shadow-sm"
              disabled={!displayName.trim()}
            >
              <span>Continue</span>
              <ArrowRight className="w-4 h-4" />
            </Button>
          </form>
        )}

        {/* STEP 2: ACCOUNT MODE SELECTION */}
        {step === 2 && (
          <div className="space-y-5 relative z-10 animate-in fade-in-50">
            {/* Core Pillars */}
            <div className="grid grid-cols-2 gap-3">
              <div className="p-3 rounded-lg border border-border/60 bg-muted/30 space-y-1 text-left">
                <div className="flex items-center gap-1.5 text-xs font-semibold text-foreground">
                  <HardDrive className="w-3.5 h-3.5 text-primary" />
                  <span>100% Local Storage</span>
                </div>
                <p className="text-[11px] text-muted-foreground leading-relaxed">
                  Notes, recordings, and graph stay on your local disk.
                </p>
              </div>

              <div className="p-3 rounded-lg border border-border/60 bg-muted/30 space-y-1 text-left">
                <div className="flex items-center gap-1.5 text-xs font-semibold text-foreground">
                  <ShieldCheck className="w-3.5 h-3.5 text-emerald-500" />
                  <span>Privacy Guard</span>
                </div>
                <p className="text-[11px] text-muted-foreground leading-relaxed">
                  Never uploads your notes merely because you connect.
                </p>
              </div>
            </div>

            {/* Actions */}
            <div className="space-y-3 pt-1">
              {/* Primary: Continue with Google */}
              <div className="space-y-1">
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
                <p className="text-[10px] text-center text-muted-foreground">
                  Sync your Relay identity and enable account-based features.
                </p>
              </div>

              <div className="relative flex items-center justify-center my-2">
                <div className="absolute inset-0 flex items-center">
                  <div className="w-full border-t border-border/40" />
                </div>
                <span className="relative bg-card px-3 text-[11px] uppercase tracking-wider text-muted-foreground">
                  or
                </span>
              </div>

              {/* Secondary: Continue Locally */}
              <div className="space-y-1">
                <Button
                  variant="outline"
                  className="w-full h-11 text-xs font-semibold gap-2 border-border/80 hover:bg-accent text-foreground"
                  onClick={handleLocallyClick}
                  disabled={connecting}
                >
                  <span>Continue Locally</span>
                  <ArrowRight className="w-3.5 h-3.5" />
                </Button>
                <p className="text-[10px] text-center text-muted-foreground">
                  Your data stays on this device. You can connect an account later.
                </p>
              </div>
            </div>

            <div className="flex items-center justify-between pt-2 border-t border-border/60">
              <Button
                variant="ghost"
                size="sm"
                className="text-xs text-muted-foreground hover:text-foreground h-8 gap-1.5"
                onClick={() => setStep(1)}
                disabled={connecting}
              >
                <ArrowLeft className="w-3.5 h-3.5" />
                <span>Change Name</span>
              </Button>
              <span className="text-[11px] text-muted-foreground">
                Hello, <strong className="text-foreground">{displayName.trim() || 'friend'}</strong>
              </span>
            </div>
          </div>
        )}
      </div>
    </div>
  );
};
