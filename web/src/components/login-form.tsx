"use client";

import React, { useState } from "react";
import { toast } from "sonner";
import { Toaster } from "@/components/ui/sonner";
import { useRouter } from "next/navigation";
import Link from "next/link";
import Input from "./Input";
import { MiniLoader } from "./mini-loader";
import { LoadingSpinner } from "./loading-view";

type AuthMode = 'login' | 'forgot' | 'reset';

export function LoginForm() {
  const [mode, setMode] = useState<AuthMode>('login');
  const [email, setEmail] = useState('');
  const [password, setPassword] = useState('');
  const [isLoading, setIsLoading] = useState(false);
  const [isSocialLoading, setIsSocialLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const router = useRouter();

  const handleGoogleLogin = async () => {
    setIsSocialLoading(true);
    setError(null);
    try {
      // Mock login delay
      await new Promise(resolve => setTimeout(resolve, 1000));
      toast.success("Login successful (Mocked)");
      router.push("/");
    } catch (err: any) {
      setError(err.message || "Failed to connect.");
      toast.error("Login failed");
    } finally {
      setIsSocialLoading(false);
    }
  };

  const handleAuth = async (e: React.FormEvent) => {
    e.preventDefault();
    setIsLoading(true);
    setError(null);
    try {
      await new Promise(resolve => setTimeout(resolve, 1000));
      if (mode === 'forgot') {
        toast.success("Password reset link sent to your email!");
        setMode('login');
      } else if (mode === 'reset') {
        toast.success("Password updated successfully!");
        setMode('login');
      }
    } catch (err: any) {
      setError(err.message || "An error occurred");
      toast.error(err.message || "Authentication failed");
    } finally {
      setIsLoading(false);
    }
  };

  return (
    <div className="flex items-center justify-center bg-transparent py-4 transition-all duration-500">
      <div className="w-full max-w-[360px] bg-white dark:bg-slate-900 rounded-lg shadow-2xl overflow-hidden border border-slate-200 dark:border-slate-800 animate-in zoom-in-95 duration-500">
        <Toaster />
        <div className="p-7 md:p-8">
          <header className="text-center mb-7">
            <div className="flex justify-center mb-4">
              <MiniLoader />
            </div>
            <h1 className="text-2xl font-black text-slate-900 dark:text-white tracking-tight">
              {mode === 'login' && 'Welcome Back'}
              {mode === 'forgot' && 'Reset Password'}
              {mode === 'reset' && 'Update Password'}
            </h1>
            <p className="text-slate-500 dark:text-slate-400 mt-1.5 text-xs font-medium px-4">
              {mode === 'login' && 'Access the Boilerplate with your account'}
              {mode === 'forgot' && 'Enter your email to receive a reset link'}
              {mode === 'reset' && 'Enter your new password below'}
            </p>
          </header>

          {error && (
            <div className="mb-5 p-3.5 bg-rose-50 dark:bg-rose-950/30 border border-rose-100 dark:border-rose-900/50 rounded-lg flex items-start gap-2.5 animate-in fade-in slide-in-from-top-2 duration-300">
              <i className="fa-solid fa-circle-exclamation text-rose-500 mt-0.5"></i>
              <p className="text-[11px] font-bold text-rose-700 dark:text-rose-400 leading-relaxed">{error}</p>
            </div>
          )}

          {/* Sign-In */}
          {mode === 'login' && (
            <div className="space-y-6">
              <button
                onClick={handleGoogleLogin}
                disabled={isSocialLoading}
                className="w-full flex items-center justify-center gap-2.5 bg-white dark:bg-slate-800 border-2 border-slate-200 dark:border-slate-700 py-3 rounded-lg font-bold text-slate-700 dark:text-slate-200 hover:border-indigo-500 dark:hover:border-indigo-500 hover:bg-slate-50 dark:hover:bg-slate-700/50 transition-all active:scale-[0.98] disabled:opacity-50 shadow-md hover:shadow-lg shadow-slate-200/50 dark:shadow-none"
              >
                {isSocialLoading ? (
                  <LoadingSpinner size="sm" />
                ) : (
                  <div className="flex items-center gap-2.5">
                    <span className="text-sm">Mock Sign In</span>
                  </div>
                )}
              </button>

              <div className="text-center pt-3.5">
                <p className="text-xs font-bold text-slate-500 dark:text-slate-400 capitalize tracking-wide">
                  New here?{" "}
                  <button
                    type="button"
                    className="text-indigo-600 dark:text-indigo-400 hover:underline ml-1"
                  >
                    Register Now!
                  </button>
                </p>
              </div>
            </div>
          )}

          {/* Password recovery forms */}
          {(mode === 'forgot' || mode === 'reset') && (
            <form onSubmit={handleAuth} className="space-y-4">
              {mode === 'forgot' && (
                <Input
                  label="Email Address"
                  type="email"
                  placeholder="name@example.com"
                  required
                  value={email}
                  onChange={(e) => setEmail(e.target.value)}
                />
              )}
              {mode === 'reset' && (
                <Input
                  label="New Password"
                  type="password"
                  placeholder="••••••••"
                  required
                  value={password}
                  onChange={(e) => setPassword(e.target.value)}
                />
              )}
              <button
                type="submit"
                disabled={isLoading}
                className="w-full bg-slate-900 dark:bg-slate-800 hover:bg-slate-800 dark:hover:bg-slate-700 text-white dark:text-slate-200 py-3.5 rounded-lg font-bold transition-all active:scale-[0.98] disabled:opacity-50 mt-3.5 flex items-center justify-center shadow-xl shadow-slate-900/20 dark:shadow-indigo-500/10"
              >
                {isLoading ? (
                  <div className="flex items-center gap-2">
                    <LoadingSpinner size="sm" />
                    <span>Processing...</span>
                  </div>
                ) : (
                  <i className={`fa-solid ${mode === 'forgot' ? 'fa-paper-plane' : 'fa-check'} mr-2`}></i>
                )}
                {mode === 'forgot' ? 'Send Reset Link' : 'Update Password'}
              </button>
              <div className="text-center mt-3.5">
                <button
                  type="button"
                  onClick={() => { setMode('login'); setError(null); }}
                  className="text-xs font-bold text-slate-500 hover:text-indigo-600 dark:text-slate-400 dark:hover:text-indigo-400 transition-all flex items-center justify-center gap-2 mx-auto"
                >
                  <i className="fa-solid fa-arrow-left"></i>
                  <span>Back to Login</span>
                </button>
              </div>
            </form>
          )}
        </div>
      </div>
    </div>
  );
}

