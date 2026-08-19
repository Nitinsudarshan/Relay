"use client";

import React from "react";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { X, Activity } from "lucide-react";

interface ChangelogDialogProps {
  open: boolean;
  onClose: () => void;
  currentVersion: string;
}

export const CHANGELOG_DATA = [
  {
    version: "0.2.1",
    date: "2026-08-19",
    type: "patch",
    title: "Fix Windows Build (clang.dll / whisper-rs-sys missing dependency)",
    items: [
      "Gated whisper-rs under optional whisper-local feature to prevent LLVM/clang.dll missing panics on Windows.",
      "Added fallback heuristic STT engine when local Ollama or Whisper model is not configured.",
    ],
  },
  {
    version: "0.2.0",
    date: "2026-08-19",
    type: "minor",
    title: 'Relay Visual Identity Pass ("Monochrome & Electric Blue")',
    items: [
      "Updated CSS variables to Monochrome & Electric Blue palette (#2563EB light / #60A5FA dark) with 3-way semantic colors.",
      "Designed two-tone Relay logo mark and integrated across native sidebar, web dashboard, login page, and favicon.",
      "Rebuilt Push-to-Talk floating Dictation Pill overlay with state machine, live waveform, and rotating captions.",
      "Restructured Provider Settings with General, Providers, Triggers, Vault, Account, and Data & Privacy domains.",
      "Unified native and web Kanban boards with 3-way semantic priority badges.",
      "Restructured Scribble Notes with master list, detail pane, pill actions toolbar, and raw audio reassurance line.",
    ],
  },
  {
    version: "0.1.2",
    date: "2026-08-19",
    type: "patch",
    title: "Complete Theme System & Token Refactoring",
    items: [
      "Replaced ad-hoc Tailwind colors with theme token classes (bg-primary, bg-card, bg-muted, border-border).",
      "Implemented live audio level meter animation in native capture widget.",
    ],
  },
  {
    version: "0.1.0",
    date: "2026-08-19",
    type: "major",
    title: "Initial Release — Multi-Surface Architecture",
    items: [
      "Windows native Tauri app with WASAPI audio capture & Rust backend pipeline.",
      "Next.js web dashboard with Supabase Cloud hybrid sync.",
    ],
  },
];

export function ChangelogDialog({
  open,
  onClose,
  currentVersion,
}: ChangelogDialogProps) {
  if (!open) return null;

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/60 backdrop-blur-xs p-4 animate-in fade-in duration-200">
      <div className="bg-popover border border-border shadow-2xl rounded-2xl w-full max-w-xl max-h-[85vh] flex flex-col overflow-hidden text-popover-foreground">
        {/* Modal Header */}
        <div className="p-4 md:p-5 border-b border-border flex items-center justify-between shrink-0">
          <div className="flex items-center gap-2.5">
            <div className="p-2 rounded-xl bg-primary/10 text-primary">
              <Activity className="w-5 h-5" />
            </div>
            <div>
              <div className="flex items-center gap-2">
                <h3 className="text-base font-extrabold text-foreground">Relay Release Notes</h3>
                <Badge variant="outline" className="text-xs font-mono border-primary/30 text-primary">
                  v{currentVersion}
                </Badge>
              </div>
              <p className="text-xs text-muted-foreground">Version history & recent change log</p>
            </div>
          </div>

          <Button
            size="icon"
            variant="ghost"
            onClick={onClose}
            className="h-8 w-8 rounded-full text-muted-foreground hover:text-foreground"
          >
            <X className="w-4 h-4" />
          </Button>
        </div>

        {/* Modal Content Scrollable Area */}
        <div className="flex-1 overflow-y-auto p-4 md:p-5 space-y-6">
          {CHANGELOG_DATA.map((entry) => (
            <div key={entry.version} className="space-y-2 border-b border-border/60 pb-5 last:border-none last:pb-0">
              <div className="flex items-center justify-between">
                <div className="flex items-center gap-2">
                  <span className="font-mono font-bold text-xs text-primary bg-primary/10 px-2 py-0.5 rounded-md">
                    v{entry.version}
                  </span>
                  <span className="text-xs font-bold text-foreground">{entry.title}</span>
                </div>
                <span className="font-mono text-[10px] text-muted-foreground">{entry.date}</span>
              </div>

              <ul className="space-y-1.5 pt-1 pl-2">
                {entry.items.map((item, idx) => (
                  <li key={idx} className="text-xs text-muted-foreground flex items-start gap-2 leading-relaxed">
                    <span className="w-1.5 h-1.5 rounded-full bg-primary shrink-0 mt-1.5" />
                    <span>{item}</span>
                  </li>
                ))}
              </ul>
            </div>
          ))}
        </div>

        {/* Modal Footer */}
        <div className="p-3 bg-muted/30 border-t border-border flex items-center justify-between shrink-0 text-xs text-muted-foreground">
          <span className="font-mono text-[10px]">Root Registry: VERSION</span>
          <Button size="sm" variant="default" onClick={onClose} className="text-xs h-8">
            Close Changelog
          </Button>
        </div>
      </div>
    </div>
  );
}
