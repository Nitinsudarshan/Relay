import React from 'react';
import { Card, CardContent } from '@/components/ui/card';
import { Badge } from '@/components/ui/badge';
import { Mic, Zap, ShieldCheck, Sparkles, Kanban, Command } from 'lucide-react';

export const PTTWidget: React.FC = () => {
  return (
    <div className="w-full flex flex-col items-center justify-center py-6">
      <Badge variant="outline" className="gap-2 px-4 py-2 text-xs font-mono bg-muted/50 border-border">
        <Command className="w-3.5 h-3.5 text-primary" />
        Dictation pill is floating on your desktop
      </Badge>

      {/* Hero Showcase Card */}
      <Card className="w-full max-w-2xl mt-6 border-border bg-card shadow-lg rounded-2xl overflow-hidden">
        <CardContent className="p-6 flex flex-col items-center text-center space-y-4">
          <div className="flex items-center gap-2">
            <Badge variant="outline" className="text-xs font-mono gap-1.5 px-3 py-1 bg-muted/50 border-border">
              <Zap className="w-3.5 h-3.5 text-primary" /> Global Shortcut Active
            </Badge>
            <Badge variant="secondary" className="text-xs font-mono gap-1.5 px-3 py-1">
              <ShieldCheck className="w-3.5 h-3.5 text-emerald-500" /> Local Vault ($0)
            </Badge>
          </div>

          <div className="space-y-1 max-w-md">
            <h3 className="text-lg font-bold tracking-tight text-foreground">
              Always-on-top Voice Dictation
            </h3>
            <p className="text-xs text-muted-foreground leading-relaxed">
              Hover or press <kbd className="px-1.5 py-0.5 bg-muted rounded border border-border font-mono text-[10px] text-foreground">Ctrl+Space</kbd> anywhere to start recording speech directly into structured Kanban task cards or polished Scribble Markdown notes.
            </p>
          </div>

          <div className="grid grid-cols-2 gap-3 w-full pt-2">
            <div className="p-3 rounded-xl bg-muted/40 border border-border flex items-center gap-3 text-left">
              <div className="p-2 rounded-lg bg-primary/10 text-primary">
                <Kanban className="w-4 h-4" />
              </div>
              <div>
                <p className="text-xs font-semibold text-foreground">Meeting → Kanban</p>
                <p className="text-[11px] text-muted-foreground">Extracts action points to board</p>
              </div>
            </div>

            <div className="p-3 rounded-xl bg-muted/40 border border-border flex items-center gap-3 text-left">
              <div className="p-2 rounded-lg bg-primary/10 text-primary">
                <Sparkles className="w-4 h-4" />
              </div>
              <div>
                <p className="text-xs font-semibold text-foreground">Voice Scribble</p>
                <p className="text-[11px] text-muted-foreground">Transforms thoughts to markdown</p>
              </div>
            </div>
          </div>
        </CardContent>
      </Card>
    </div>
  );
};
