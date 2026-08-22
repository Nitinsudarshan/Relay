import React, { useState, useEffect } from 'react';
import { invoke } from '@tauri-apps/api/core';
import {
  RelayAccount,
  InstallationInfo,
  UpdateInfo,
  AppSettings,
} from '../../types';
import {
  User,
  ShieldCheck,
  HardDrive,
  Cloud,
  CheckCircle2,
  RefreshCw,
  Copy,
  Check,
  LogOut,
  Sparkles,
  ArrowRight,
  AlertCircle,
  ExternalLink,
  Laptop,
} from 'lucide-react';
import { Button } from '@/components/ui/button';
import { Badge } from '@/components/ui/badge';
import { Switch } from '@/components/ui/switch';

interface AccountSettingsProps {
  settings: AppSettings;
  onUpdateSettings: (updater: (prev: AppSettings) => AppSettings) => void;
  onOpenExplanation?: () => void;
}

export const AccountSettings: React.FC<AccountSettingsProps> = ({
  settings,
  onUpdateSettings,
  onOpenExplanation,
}) => {
  const [account, setAccount] = useState<RelayAccount | null>(null);
  const [installation, setInstallation] = useState<InstallationInfo | null>(null);
  const [updateInfo, setUpdateInfo] = useState<UpdateInfo | null>(null);
  const [loading, setLoading] = useState(true);
  const [signingIn, setSigningIn] = useState(false);
  const [checkingUpdate, setCheckingUpdate] = useState(false);
  const [showSignOutConfirm, setShowSignOutConfirm] = useState(false);
  const [copiedId, setCopiedId] = useState(false);
  const [showHybridModal, setShowHybridModal] = useState(false);
  const [errorMsg, setErrorMsg] = useState<string | null>(null);

  const loadData = async () => {
    try {
      setLoading(true);
      const [acc, inst] = await Promise.all([
        invoke<RelayAccount>('get_account_state'),
        invoke<InstallationInfo>('get_installation_info'),
      ]);
      setAccount(acc);
      setInstallation(inst);
    } catch (err) {
      console.error('Failed to load account/installation state:', err);
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => {
    loadData();
  }, []);

  const handleSignIn = async () => {
    try {
      setSigningIn(true);
      setErrorMsg(null);
      const acc = await invoke<RelayAccount>('start_google_sign_in');
      setAccount(acc);
      if (onOpenExplanation) {
        onOpenExplanation();
      }
    } catch (err: unknown) {
      console.error('Google Sign-In failed:', err);
      const msg = typeof err === 'string' ? err : (err as { message?: string })?.message || 'Sign-in failed. Please try again.';
      setErrorMsg(msg);
    } finally {
      setSigningIn(false);
    }
  };

  const handleSignOut = async () => {
    try {
      setErrorMsg(null);
      const acc = await invoke<RelayAccount>('sign_out_account');
      setAccount(acc);
      setShowSignOutConfirm(false);
    } catch (err: unknown) {
      console.error('Sign-out failed:', err);
      const msg = typeof err === 'string' ? err : (err as { message?: string })?.message || 'Sign-out failed.';
      setErrorMsg(msg);
    }
  };

  const handleCheckUpdates = async () => {
    try {
      setCheckingUpdate(true);
      const info = await invoke<UpdateInfo>('check_for_app_updates');
      setUpdateInfo(info);
    } catch (err) {
      console.error('Failed to check for updates:', err);
    } finally {
      setCheckingUpdate(false);
    }
  };

  const handleToggleDiagnostics = async (checked: boolean) => {
    try {
      const updated = await invoke<AppSettings>('set_diagnostics_consent', { enabled: checked });
      onUpdateSettings(() => updated);
    } catch (err) {
      console.error('Failed to update diagnostics consent:', err);
    }
  };

  const copyInstallationId = () => {
    if (installation?.installation_id) {
      navigator.clipboard.writeText(installation.installation_id);
      setCopiedId(true);
      setTimeout(() => setCopiedId(false), 2000);
    }
  };

  const maskedId = installation?.installation_id
    ? installation.installation_id.length > 8
      ? `••••••••-••••-${installation.installation_id.slice(-4)}`
      : installation.installation_id
    : '••••••••••••';

  const isDiagnosticsAllowed = settings.diagnostics?.allow_anonymous_diagnostics ?? true;

  return (
    <div className="space-y-8 animate-in fade-in-50 duration-200">
      {/* Header & Invariant Statement */}
      <div className="border-b border-border/40 pb-5">
        <div className="flex items-center gap-3 mb-1.5">
          <h2 className="text-xl font-bold tracking-tight text-foreground">Relay Account & Identity</h2>
          <Badge variant="outline" className="text-[10px] font-mono border-primary/30 text-primary bg-primary/5 uppercase">
            {account?.authenticated ? 'Google Authenticated' : 'Local Mode'}
          </Badge>
        </div>
        <p className="text-xs text-muted-foreground leading-relaxed max-w-2xl">
          Your Relay account manages your installation identity, updates, and cloud service access.
          <strong className="text-foreground ml-1">Your Scribbles, Voice Notes, and Meetings remain 100% local on this device.</strong>
        </p>
      </div>

      {errorMsg && (
        <div className="p-3.5 rounded-lg border border-destructive/30 bg-destructive/10 text-destructive text-xs flex items-start gap-2.5">
          <AlertCircle className="w-4 h-4 shrink-0 mt-0.5" />
          <div className="space-y-1">
            <span className="font-semibold">Authentication Message:</span>
            <p>{errorMsg}</p>
          </div>
        </div>
      )}

      {/* 1. Account Profile Card */}
      <div className="p-5 rounded-xl border border-border/80 bg-card/60 backdrop-blur-xs space-y-5">
        <div className="flex flex-col sm:flex-row sm:items-center justify-between gap-4">
          <div className="flex items-center gap-4">
            {account?.authenticated && account.profile_image ? (
              <img
                src={account.profile_image}
                alt="Profile"
                className="w-14 h-14 rounded-full border-2 border-primary/30 object-cover shadow-xs"
              />
            ) : (
              <div className="w-14 h-14 rounded-full bg-muted border border-border flex items-center justify-center text-muted-foreground shadow-xs">
                <User className="w-7 h-7" />
              </div>
            )}

            <div className="space-y-1">
              <div className="flex items-center gap-2">
                <h3 className="text-base font-semibold text-foreground">
                  {account?.authenticated ? account.display_name || account.email : 'Local User'}
                </h3>
                {account?.authenticated && (
                  <Badge variant="secondary" className="text-[10px] gap-1 py-0 px-2 bg-emerald-500/10 text-emerald-500 border border-emerald-500/20">
                    <CheckCircle2 className="w-3 h-3" />
                    <span>Google Connected</span>
                  </Badge>
                )}
              </div>
              <p className="text-xs text-muted-foreground font-mono">
                {account?.authenticated ? account.email : 'No Relay account connected. Operating locally.'}
              </p>
            </div>
          </div>

          <div>
            {account?.authenticated ? (
              <Button
                variant="outline"
                size="sm"
                className="text-xs text-destructive hover:bg-destructive/10 hover:text-destructive border-destructive/30 gap-1.5"
                onClick={() => setShowSignOutConfirm(true)}
              >
                <LogOut className="w-3.5 h-3.5" />
                <span>Sign Out</span>
              </Button>
            ) : (
              <Button
                size="sm"
                className="text-xs font-semibold gap-2 bg-primary hover:bg-primary/90 text-primary-foreground shadow-xs"
                onClick={handleSignIn}
                disabled={signingIn}
              >
                {signingIn ? <RefreshCw className="w-3.5 h-3.5 animate-spin" /> : (
                  <svg className="w-3.5 h-3.5" viewBox="0 0 24 24">
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
                <span>{signingIn ? 'Connecting...' : 'Sign in with Google'}</span>
              </Button>
            )}
          </div>
        </div>

        {/* Sign Out Confirmation Modal */}
        {showSignOutConfirm && (
          <div className="p-4 rounded-lg border border-destructive/40 bg-destructive/5 space-y-3 animate-in fade-in-50">
            <div className="space-y-1">
              <h4 className="text-xs font-bold text-destructive flex items-center gap-1.5">
                <AlertCircle className="w-4 h-4" />
                <span>Sign out of Relay?</span>
              </h4>
              <p className="text-xs text-muted-foreground leading-relaxed">
                Your local Scribbles, Voice Notes, and Meetings will remain untouched on this device.
                You will only be disconnected from your Relay identity and Google Calendar sync.
              </p>
            </div>
            <div className="flex items-center gap-2">
              <Button size="sm" variant="destructive" className="text-xs h-8" onClick={handleSignOut}>
                Confirm Sign Out
              </Button>
              <Button size="sm" variant="ghost" className="text-xs h-8" onClick={() => setShowSignOutConfirm(false)}>
                Cancel
              </Button>
            </div>
          </div>
        )}
      </div>

      {/* 2. Installation & Version Metadata */}
      <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
        {/* App Version & Update Card */}
        <div className="p-5 rounded-xl border border-border/80 bg-card/60 space-y-4">
          <div className="flex items-center justify-between">
            <div className="flex items-center gap-2">
              <Laptop className="w-4 h-4 text-primary" />
              <span className="text-xs font-semibold text-foreground">Relay Application</span>
            </div>
            <Badge variant="outline" className="text-[10px] font-mono">
              v{installation?.app_version || '0.8.2'}
            </Badge>
          </div>

          <p className="text-xs text-muted-foreground">
            Platform: <span className="font-mono text-foreground capitalize">{installation?.platform || 'Windows'}</span> ({installation?.os_version || 'x86_64'})
          </p>

          <div className="pt-2 border-t border-border/40 flex items-center justify-between">
            <Button
              variant="outline"
              size="sm"
              className="text-xs gap-1.5 h-8"
              onClick={handleCheckUpdates}
              disabled={checkingUpdate}
            >
              <RefreshCw className={`w-3.5 h-3.5 ${checkingUpdate ? 'animate-spin' : ''}`} />
              <span>{checkingUpdate ? 'Checking...' : 'Check for Updates'}</span>
            </Button>

            {updateInfo && (
              <span className="text-xs text-muted-foreground">
                {updateInfo.is_offline ? (
                  <span className="text-amber-500">Offline mode</span>
                ) : updateInfo.update_available ? (
                  <span className="text-emerald-500 font-semibold">v{updateInfo.latest_version} available</span>
                ) : (
                  <span className="text-emerald-500 flex items-center gap-1">
                    <Check className="w-3 h-3" /> Up to date
                  </span>
                )}
              </span>
            )}
          </div>
        </div>

        {/* Installation Identity Card */}
        <div className="p-5 rounded-xl border border-border/80 bg-card/60 space-y-4">
          <div className="flex items-center justify-between">
            <div className="flex items-center gap-2">
              <ShieldCheck className="w-4 h-4 text-emerald-500" />
              <span className="text-xs font-semibold text-foreground">Installation Identity</span>
            </div>
            <Badge variant="outline" className="text-[10px] font-mono text-muted-foreground">
              Anonymous
            </Badge>
          </div>

          <div className="space-y-1.5">
            <div className="flex items-center justify-between bg-muted/40 p-2 rounded-lg border border-border/40">
              <span className="text-xs font-mono text-muted-foreground">{maskedId}</span>
              <Button
                variant="ghost"
                size="sm"
                className="h-6 w-6 p-0 text-muted-foreground hover:text-foreground"
                onClick={copyInstallationId}
                title="Copy Full Installation ID"
              >
                {copiedId ? <Check className="w-3 h-3 text-emerald-500" /> : <Copy className="w-3 h-3" />}
              </Button>
            </div>
            <p className="text-[11px] text-muted-foreground">
              Survives restarts and updates. Used solely for diagnostic routing and update compatibility.
            </p>
          </div>
        </div>
      </div>

      {/* 3. Account Mode & Hybrid Architecture */}
      <div className="p-5 rounded-xl border border-border/80 bg-gradient-to-br from-card/80 to-primary/5 space-y-4">
        <div className="flex items-center justify-between">
          <div className="space-y-1">
            <div className="flex items-center gap-2">
              <HardDrive className="w-4 h-4 text-primary" />
              <h3 className="text-sm font-semibold text-foreground">Current Operating Mode: Local</h3>
            </div>
            <p className="text-xs text-muted-foreground">
              Your notes, vector embeddings, meeting audio, and knowledge graph live exclusively on this computer.
            </p>
          </div>
          <Button
            size="sm"
            variant="outline"
            className="text-xs gap-1.5 border-primary/40 text-primary hover:bg-primary/10 shadow-xs"
            onClick={() => setShowHybridModal(true)}
          >
            <Sparkles className="w-3.5 h-3.5" />
            <span>Explore Hybrid</span>
          </Button>
        </div>
      </div>


      {/* 5. Privacy & Diagnostics Consent */}
      <div className="p-5 rounded-xl border border-border/80 bg-card/60 space-y-4">
        <div className="flex items-start justify-between gap-4">
          <div className="space-y-1">
            <div className="flex items-center gap-2">
              <ShieldCheck className="w-4 h-4 text-emerald-500" />
              <h3 className="text-sm font-semibold text-foreground">Help Improve Relay</h3>
            </div>
            <p className="text-xs text-muted-foreground leading-relaxed max-w-xl">
              Share anonymous diagnostic telemetry (Relay version, app crashes, performance metadata) to help fix bugs.
              <strong className="text-foreground block mt-1">
                Your notes, scribbles, audio recordings, and transcripts are NEVER included.
              </strong>
            </p>
          </div>
          <Switch
            checked={isDiagnosticsAllowed}
            onCheckedChange={handleToggleDiagnostics}
          />
        </div>
      </div>

      {/* Hybrid Mode Preview Modal */}
      {showHybridModal && (
        <div className="fixed inset-0 z-50 flex items-center justify-center bg-background/80 backdrop-blur-xs p-4 animate-in fade-in-50">
          <div className="w-full max-w-md bg-card border border-border rounded-2xl p-6 shadow-2xl space-y-6">
            <div className="text-center space-y-2">
              <div className="w-12 h-12 rounded-2xl bg-primary/10 border border-primary/20 text-primary flex items-center justify-center mx-auto mb-2">
                <Sparkles className="w-6 h-6" />
              </div>
              <h3 className="text-lg font-bold text-foreground">Relay Hybrid</h3>
              <p className="text-xs text-muted-foreground max-w-xs mx-auto">
                Local-first speed with selective, user-controlled cloud synchronization.
              </p>
            </div>

            <div className="space-y-2.5 text-xs text-muted-foreground">
              <div className="flex items-center gap-2.5 p-2 rounded-lg bg-muted/40">
                <CheckCircle2 className="w-4 h-4 text-emerald-500 shrink-0" />
                <span>Full Google Calendar real-time synchronization</span>
              </div>
              <div className="flex items-center gap-2.5 p-2 rounded-lg bg-muted/40">
                <CheckCircle2 className="w-4 h-4 text-emerald-500 shrink-0" />
                <span>Cross-device access across Windows and macOS</span>
              </div>
              <div className="flex items-center gap-2.5 p-2 rounded-lg bg-muted/40">
                <CheckCircle2 className="w-4 h-4 text-emerald-500 shrink-0" />
                <span>Encrypted cloud backup with granular sync controls</span>
              </div>
              <div className="flex items-center gap-2.5 p-2 rounded-lg bg-muted/40">
                <CheckCircle2 className="w-4 h-4 text-emerald-500 shrink-0" />
                <span>Zero manual migrations: seamless transition from Local</span>
              </div>
            </div>

            <div className="pt-2 border-t border-border/40 flex items-center justify-between">
              <span className="text-xs text-muted-foreground">Coming in Hybrid Release</span>
              <Button size="sm" onClick={() => setShowHybridModal(false)}>
                Got it
              </Button>
            </div>
          </div>
        </div>
      )}
    </div>
  );
};
