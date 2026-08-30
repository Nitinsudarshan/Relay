"use client";

import React, { useState } from "react";
import { toast } from "sonner";
import { useRouter } from "next/navigation";
import { Card, CardHeader, CardTitle, CardDescription, CardContent } from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Badge } from "@/components/ui/badge";
import { RelayLogo } from "@/components/relay-logo";
import { ArrowLeft, Loader2, Cloud, ShieldCheck } from "lucide-react";

type AuthMode = 'login' | 'forgot' | 'reset';

export function LoginForm() {
  const [mode, setMode] = useState<AuthMode>('login');
  const [email, setEmail] = useState('');
  const [password, setPassword] = useState('');
  const [isLoading, setIsLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const router = useRouter();

  const handleLogin = async (e: React.FormEvent) => {
    e.preventDefault();
    setIsLoading(true);
    setError(null);
    try {
      await new Promise(resolve => setTimeout(resolve, 800));
      toast.success("Signed in to Relay Cloud");
      router.push("/");
    } catch (err: any) {
      setError(err.message || "Authentication failed");
      toast.error("Login failed");
    } finally {
      setIsLoading(false);
    }
  };

  return (
    <Card className="w-full max-w-sm border-border bg-card shadow-2xl">
      <CardHeader className="text-center pb-4 space-y-2">
        <div className="flex justify-center mb-1">
          <div className="w-12 h-12 rounded-lg bg-card border border-border flex items-center justify-center shadow-md">
            <RelayLogo className="w-7 h-7" />
          </div>
        </div>
        <div className="flex justify-center gap-1.5 mb-1">
          <Badge variant="default" className="text-[10px] gap-1 px-2 py-0">
            <Cloud className="w-3 h-3" /> Hybrid Sync
          </Badge>
          <Badge variant="emerald" className="text-[10px] gap-1 px-2 py-0">
            <ShieldCheck className="w-3 h-3" /> Supabase Auth
          </Badge>
        </div>
        <CardTitle className="text-xl font-bold text-foreground">
          {mode === 'login' && 'Relay Cloud Access'}
          {mode === 'forgot' && 'Reset Password'}
          {mode === 'reset' && 'Update Password'}
        </CardTitle>
        <CardDescription className="text-xs">
          {mode === 'login' && 'Sign in to access your synced vault notes & Kanban state'}
          {mode === 'forgot' && 'Enter your email to receive a password reset link'}
          {mode === 'reset' && 'Enter your new password below'}
        </CardDescription>
      </CardHeader>

      <CardContent className="space-y-4">
        {error && (
          <div className="p-3 rounded-lg bg-destructive/10 border border-destructive/30 text-destructive text-xs font-medium">
            {error}
          </div>
        )}

        {mode === 'login' && (
          <form onSubmit={handleLogin} className="space-y-3.5">
            <div>
              <label htmlFor="login-email" className="block text-xs font-medium text-muted-foreground mb-1">Email Address</label>
              <Input
                id="login-email"
                type="email"
                placeholder="name@example.com"
                value={email}
                onChange={(e) => setEmail(e.target.value)}
                required
              />
            </div>

            <div>
              <div className="flex items-center justify-between mb-1">
                <label htmlFor="login-password" className="text-xs font-medium text-muted-foreground">Password</label>
                <button
                  type="button"
                  onClick={() => setMode('forgot')}
                  className="text-xs text-primary hover:underline"
                >
                  Forgot?
                </button>
              </div>
              <Input
                id="login-password"
                type="password"
                placeholder="••••••••"
                value={password}
                onChange={(e) => setPassword(e.target.value)}
                required
              />
            </div>

            <Button type="submit" className="w-full font-medium text-xs mt-2" disabled={isLoading}>
              {isLoading ? (
                <div className="flex items-center gap-2">
                  <Loader2 className="w-4 h-4 animate-spin" />
                  <span>Signing in...</span>
                </div>
              ) : (
                'Sign In to Relay'
              )}
            </Button>
          </form>
        )}

        {(mode === 'forgot' || mode === 'reset') && (
          <form onSubmit={handleLogin} className="space-y-3.5">
            {mode === 'forgot' && (
              <div>
                <label htmlFor="reset-email" className="block text-xs font-medium text-muted-foreground mb-1">Email Address</label>
                <Input
                  id="reset-email"
                  type="email"
                  placeholder="name@example.com"
                  value={email}
                  onChange={(e) => setEmail(e.target.value)}
                  required
                />
              </div>
            )}

            {mode === 'reset' && (
              <div>
                <label htmlFor="new-password" className="block text-xs font-medium text-muted-foreground mb-1">New Password</label>
                <Input
                  id="new-password"
                  type="password"
                  placeholder="••••••••"
                  value={password}
                  onChange={(e) => setPassword(e.target.value)}
                  required
                />
              </div>
            )}

            <Button type="submit" className="w-full font-medium text-xs" disabled={isLoading}>
              {isLoading ? <Loader2 className="w-4 h-4 animate-spin" /> : mode === 'forgot' ? 'Send Reset Link' : 'Update Password'}
            </Button>

            <Button
              type="button"
              variant="ghost"
              size="sm"
              onClick={() => { setMode('login'); setError(null); }}
              className="w-full text-xs gap-1.5 text-muted-foreground hover:text-foreground"
            >
              <ArrowLeft className="w-3.5 h-3.5" />
              <span>Back to Sign In</span>
            </Button>
          </form>
        )}
      </CardContent>
    </Card>
  );
}
