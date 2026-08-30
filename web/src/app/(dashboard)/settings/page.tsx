"use client";

import React, { useState } from "react";
import {
  Sliders,
  Cpu,
  User,
  ShieldCheck,
  CheckCircle,
  Download,
  AlertTriangle,
  Cloud,
} from "lucide-react";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Switch } from "@/components/ui/switch";
import { PageHeader } from "@/components/page-header";

type SettingsSection = "general" | "providers" | "account" | "privacy";

export default function WebSettingsPage() {
  const [activeSection, setActiveSection] = useState<SettingsSection>("general");
  const [saved, setSaved] = useState(false);

  const [hybridSyncEnabled, setHybridSyncEnabled] = useState(true);
  const [cloudApiKey, setCloudApiKey] = useState("");

  const handleSave = (e: React.FormEvent) => {
    e.preventDefault();
    setSaved(true);
    setTimeout(() => setSaved(false), 2000);
  };

  return (
    <div className="flex flex-1 flex-col gap-6 p-4 md:p-6 lg:p-8 max-w-7xl mx-auto w-full">
      {/* Centralized Page Header */}
      <PageHeader
        kicker="RELAY · SETTINGS"
        title="How Relay"
        highlightText="behaves."
        description="Hybrid cloud dashboard settings, Supabase sync configuration, and privacy controls."
      />

      <div className="flex-1 flex flex-col md:flex-row gap-6 min-h-0">
        {/* Sub-nav Sidebar */}
        <aside className="w-full md:w-56 flex flex-col shrink-0 gap-1 bg-card p-3 rounded-lg border border-border h-fit">
          <div className="px-3 py-2 mb-1">
            <span className="font-mono text-[10px] font-bold tracking-widest text-muted-foreground uppercase">
              SETTINGS SECTIONS
            </span>
          </div>

          <button
            type="button"
            onClick={() => setActiveSection("general")}
            className={`flex items-center gap-2.5 px-3 py-2 rounded-lg text-xs font-medium transition-all text-left ${
              activeSection === "general"
                ? "bg-accent text-accent-foreground font-semibold shadow-xs"
                : "text-muted-foreground hover:bg-muted hover:text-foreground"
            }`}
          >
            <Sliders className="w-4 h-4 text-primary" />
            <span>General</span>
          </button>

          <button
            type="button"
            onClick={() => setActiveSection("providers")}
            className={`flex items-center gap-2.5 px-3 py-2 rounded-lg text-xs font-medium transition-all text-left ${
              activeSection === "providers"
                ? "bg-accent text-accent-foreground font-semibold shadow-xs"
                : "text-muted-foreground hover:bg-muted hover:text-foreground"
            }`}
          >
            <Cpu className="w-4 h-4 text-primary" />
            <span>Cloud Providers</span>
          </button>

          <button
            type="button"
            onClick={() => setActiveSection("account")}
            className={`flex items-center gap-2.5 px-3 py-2 rounded-lg text-xs font-medium transition-all text-left ${
              activeSection === "account"
                ? "bg-accent text-accent-foreground font-semibold shadow-xs"
                : "text-muted-foreground hover:bg-muted hover:text-foreground"
            }`}
          >
            <User className="w-4 h-4 text-primary" />
            <span>Account</span>
          </button>

          <button
            type="button"
            onClick={() => setActiveSection("privacy")}
            className={`flex items-center gap-2.5 px-3 py-2 rounded-lg text-xs font-medium transition-all text-left ${
              activeSection === "privacy"
                ? "bg-accent text-accent-foreground font-semibold shadow-xs"
                : "text-muted-foreground hover:bg-muted hover:text-foreground"
            }`}
          >
            <ShieldCheck className="w-4 h-4 text-primary" />
            <span>Data & Privacy</span>
          </button>
        </aside>

        {/* Main Settings Content */}
        <main className="flex-1 bg-card rounded-lg border border-border p-6">
          {saved && (
            <div className="mb-4 p-3 rounded-lg bg-success/20 border border-success/40 text-success-foreground text-xs flex items-center justify-between">
              <span className="flex items-center gap-2">
                <CheckCircle className="w-4 h-4 text-emerald-500" />
                Settings saved successfully
              </span>
            </div>
          )}

          {activeSection === "general" && (
            <div className="space-y-6">
              <div>
                <p className="font-mono text-[10px] font-bold text-muted-foreground uppercase tracking-widest mb-1">
                  HYBRID DASHBOARD DEFAULTS
                </p>
                <h2 className="text-lg font-bold text-foreground">Cloud Sync & Preferences</h2>
              </div>

              <div className="space-y-4">
                <div className="py-3 border-b border-border flex items-center justify-between">
                  <div>
                    <p className="text-xs font-semibold text-foreground">Supabase Hybrid Vault Sync</p>
                    <p className="text-[11px] text-muted-foreground">Sync notes & tasks live between desktop & web</p>
                  </div>
                  <Switch checked={hybridSyncEnabled} onCheckedChange={setHybridSyncEnabled} />
                </div>
              </div>
            </div>
          )}

          {activeSection === "providers" && (
            <form onSubmit={handleSave} className="space-y-6">
              <div>
                <p className="font-mono text-[10px] font-bold text-muted-foreground uppercase tracking-widest mb-1">
                  CLOUD API CONFIGURATION
                </p>
                <h2 className="text-lg font-bold text-foreground">LLM & Speech Credentials</h2>
              </div>

              <div className="space-y-4">
                <div>
                  <label htmlFor="cloud-api-key" className="block text-xs font-medium text-foreground mb-1">
                    OpenAI / Gemini API Key
                  </label>
                  <Input
                    id="cloud-api-key"
                    type="password"
                    value={cloudApiKey}
                    onChange={(e) => setCloudApiKey(e.target.value)}
                    placeholder="sk-..."
                  />
                </div>

                <Button type="submit" size="sm" variant="default">
                  Save Credentials
                </Button>
              </div>
            </form>
          )}

          {activeSection === "account" && (
            <div className="space-y-6">
              <div>
                <p className="font-mono text-[10px] font-bold text-muted-foreground uppercase tracking-widest mb-1">
                  SUPABASE CLOUD AUTHENTICATION
                </p>
                <h2 className="text-lg font-bold text-foreground">Account Profile</h2>
              </div>

              <div className="p-4 rounded-lg bg-card border border-border flex items-center gap-4">
                <div className="w-12 h-12 rounded-full bg-primary text-primary-foreground font-extrabold text-lg flex items-center justify-center">
                  N
                </div>
                <div className="space-y-0.5">
                  <p className="text-sm font-bold text-foreground">Nitin Sudarshan</p>
                  <p className="text-xs text-muted-foreground">nitin@example.com</p>
                </div>
              </div>
            </div>
          )}

          {activeSection === "privacy" && (
            <div className="space-y-6">
              <div>
                <p className="font-mono text-[10px] font-bold text-muted-foreground uppercase tracking-widest mb-1">
                  DATA CONTROL & PRIVACY BOUNDARIES
                </p>
                <h2 className="text-lg font-bold text-foreground">Data & Privacy Control</h2>
              </div>

              <div className="space-y-4">
                <div className="p-4 rounded-lg bg-card border border-border flex items-center justify-between">
                  <div>
                    <p className="text-xs font-bold text-foreground">Export All Synced Data</p>
                    <p className="text-[11px] text-muted-foreground">Download full backup of notes and tasks</p>
                  </div>
                  <Button variant="default" size="sm" className="gap-2">
                    <Download className="w-4 h-4" />
                    <span>Export Everything</span>
                  </Button>
                </div>

                <div className="p-4 rounded-lg border border-destructive/40 bg-destructive/5 space-y-3">
                  <div className="flex items-center gap-2 text-destructive font-bold text-xs">
                    <AlertTriangle className="w-4 h-4 shrink-0" />
                    <span>Irreversible Actions</span>
                  </div>

                  <div className="py-2 border-t border-destructive/20 flex items-center justify-between">
                    <div>
                      <p className="text-xs font-semibold text-foreground">Disconnect Hybrid Sync</p>
                      <p className="text-[11px] text-muted-foreground">Stop cloud syncing from desktop machine</p>
                    </div>
                    <Button variant="outline" size="sm" className="border-destructive/50 text-destructive hover:bg-destructive/10 gap-1.5 text-xs">
                      <Cloud className="w-3.5 h-3.5" />
                      <span>Disconnect Sync</span>
                    </Button>
                  </div>
                </div>
              </div>
            </div>
          )}
        </main>
      </div>
    </div>
  );
}

