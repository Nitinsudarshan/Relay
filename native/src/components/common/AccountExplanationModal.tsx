import React, { useState } from 'react';
import { Button } from '@/components/ui/button';
import { Badge } from '@/components/ui/badge';
import { ShieldCheck, HardDrive, Sparkles, CheckCircle2, ChevronRight, Lock } from 'lucide-react';

interface AccountExplanationModalProps {
  isOpen: boolean;
  onClose: () => void;
}

export const AccountExplanationModal: React.FC<AccountExplanationModalProps> = ({
  isOpen,
  onClose,
}) => {
  const [showDetails, setShowDetails] = useState(false);

  if (!isOpen) return null;

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-background/85 backdrop-blur-md p-4 animate-in fade-in-50">
      <div className="w-full max-w-md bg-card/95 border border-border rounded-2xl p-6 md:p-7 shadow-2xl space-y-6 relative overflow-hidden">
        {/* Glow */}
        <div className="absolute -right-12 -top-12 w-36 h-36 bg-emerald-500/15 rounded-full blur-3xl pointer-events-none" />

        <div className="text-center space-y-2.5">
          <div className="w-14 h-14 rounded-full bg-emerald-500/10 border border-emerald-500/20 text-emerald-500 flex items-center justify-center mx-auto shadow-sm">
            <CheckCircle2 className="w-7 h-7" />
          </div>
          <h2 className="text-xl font-bold tracking-tight text-foreground">
            You're signed into Relay
          </h2>
          <p className="text-xs text-muted-foreground leading-relaxed">
            Your Relay account helps us provide updates, diagnose issues, and unlock future cloud services like Google Calendar.
          </p>
        </div>

        <div className="p-4 rounded-xl border border-border/70 bg-muted/40 space-y-3 text-left">
          <div className="flex items-center gap-2 text-xs font-semibold text-foreground">
            <HardDrive className="w-4 h-4 text-primary" />
            <span>Your local knowledge remains on this device</span>
          </div>
          <p className="text-[11px] text-muted-foreground leading-relaxed">
            Relay does not upload your Scribbles, Voice Notes, Meetings, transcripts, or audio recordings simply because you signed in.
          </p>
        </div>

        {showDetails && (
          <div className="p-3.5 rounded-xl border border-primary/20 bg-primary/5 space-y-2 text-[11px] text-muted-foreground animate-in fade-in-50">
            <div className="font-semibold text-foreground flex items-center gap-1.5">
              <Lock className="w-3.5 h-3.5 text-primary" />
              <span>Data Boundaries & Security</span>
            </div>
            <ul className="space-y-1 pl-3 list-disc">
              <li><strong className="text-foreground">Account identity:</strong> Email & name are used only for account & update tracking.</li>
              <li><strong className="text-foreground">Local Vault:</strong> Notes, audio, and embeddings stay strictly in your local vault.</li>
              <li><strong className="text-foreground">Calendar:</strong> Only accessed when you explicitly sync Google Calendar.</li>
            </ul>
          </div>
        )}

        <div className="space-y-2.5 pt-1">
          <Button
            className="w-full h-10 text-xs font-semibold bg-primary hover:bg-primary/90 text-primary-foreground shadow-sm"
            onClick={onClose}
          >
            Continue to Relay
          </Button>

          <Button
            variant="ghost"
            className="w-full h-9 text-xs text-muted-foreground hover:text-foreground"
            onClick={() => setShowDetails(!showDetails)}
          >
            {showDetails ? 'Hide Privacy Details' : 'View Privacy Details'}
          </Button>
        </div>
      </div>
    </div>
  );
};
