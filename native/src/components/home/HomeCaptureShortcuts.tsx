import React from 'react';
import {
  Calendar,
  Clipboard,
  Command,
  FileText,
  Globe,
  MessageCircle,
  Mic,
  Network,
  Upload,
  type LucideIcon,
} from 'lucide-react';

import { Badge } from '@/components/ui/badge';

import type { CaptureMethod } from '@/components/capture/CaptureHubPage';
import type { HomeSurface } from './homeStats';

interface ShortcutCard {
  id: string;
  title: string;
  subtitle: string;
  icon: LucideIcon;
  /** Icon tint — the same per-surface colours the sidebar uses. */
  accent: string;
  action: string;
  /**
   * A fact read from the running app rather than assumed: an accelerator renders
   * as a `<kbd>`, a service state as plain text, because they are not the same
   * kind of thing to a screen reader.
   */
  meta?: { kind: 'hotkey' | 'status'; text: string };
  onSelect: () => void;
}

export interface HomeCaptureShortcutsProps {
  /** The configured dictation accelerator, or null when settings could not be read. */
  dictationHotkey: string | null;
  /** Whether the capture bridge is actually listening, or null when unknown. */
  bridgeRunning: boolean | null;
  bridgePort: number | null;
  onNavigate: (surface: HomeSurface) => void;
  onStartScribbleCapture: (method: CaptureMethod) => void;
}

const ShortcutButton: React.FC<{ card: ShortcutCard }> = ({ card }) => {
  const Icon = card.icon;

  return (
    <button
      type="button"
      onClick={card.onSelect}
      className="p-3.5 rounded-lg border border-border bg-card hover:bg-muted/40 hover:border-primary/50 text-left flex flex-col justify-between gap-2 transition-all focus:outline-none focus-visible:ring-2 focus-visible:ring-ring"
    >
      <Icon className={`w-4 h-4 ${card.accent}`} />

      <div>
        <span className="text-xs font-bold text-foreground block">{card.title}</span>
        <span className="text-[10px] text-muted-foreground block">{card.subtitle}</span>
      </div>

      {card.meta?.kind === 'hotkey' && (
        <kbd className="font-mono text-[9px] bg-background/80 px-1.5 py-0.5 rounded-lg border border-border flex items-center gap-1 w-fit max-w-full">
          <Command className="w-2.5 h-2.5 shrink-0" />
          <span className="truncate">{card.meta.text}</span>
        </kbd>
      )}

      {card.meta?.kind === 'status' && (
        <span className="font-mono text-[9px] text-muted-foreground truncate" title={card.meta.text}>
          {card.meta.text}
        </span>
      )}

      {!card.meta && <span className="text-[10px] text-primary font-medium">{card.action}</span>}
    </button>
  );
};

/**
 * The capture modes, on Home.
 *
 * Deliberately the same six modes as `Scribbles › Capture`, in the same order,
 * because they are the same six modes — these cards navigate to the surface that
 * performs the capture rather than reimplementing any of it. Nothing here claims
 * a capability it does not have: the voice card names the accelerator read from
 * settings, and the web-capture card names whether the bridge is actually up.
 */
export const HomeCaptureShortcuts: React.FC<HomeCaptureShortcutsProps> = ({
  dictationHotkey,
  bridgeRunning,
  bridgePort,
  onNavigate,
  onStartScribbleCapture,
}) => {
  const bridgeLabel =
    bridgeRunning === null
      ? 'Bridge state unknown'
      : bridgeRunning
        ? `Bridge live${bridgePort ? ` · :${bridgePort}` : ''}`
        : 'Bridge off — enable in Settings';

  const cards: ShortcutCard[] = [
    {
      id: 'voice',
      title: 'Voice',
      subtitle: 'Push-to-talk dictation',
      icon: Mic,
      accent: 'text-emerald-500',
      action: 'Open Voice Notes →',
      meta: { kind: 'hotkey', text: dictationHotkey ?? 'Set a hotkey in Settings' },
      onSelect: () => onNavigate('capture'),
    },
    {
      id: 'text',
      title: 'Typed Text',
      subtitle: 'Write a raw thought',
      icon: FileText,
      accent: 'text-amber-500',
      action: 'Quick Compose →',
      onSelect: () => onStartScribbleCapture('text'),
    },
    {
      id: 'clipboard',
      title: 'Clipboard',
      subtitle: '1-click paste buffer',
      icon: Clipboard,
      accent: 'text-emerald-500',
      action: 'Paste Buffer →',
      onSelect: () => onStartScribbleCapture('clipboard'),
    },
    {
      id: 'file',
      title: 'Files & Docs',
      subtitle: 'TXT, MD, CSV, JSON',
      icon: Upload,
      accent: 'text-blue-500',
      action: 'Import File →',
      onSelect: () => onStartScribbleCapture('file'),
    },
    {
      id: 'meeting',
      title: 'Meeting',
      subtitle: 'Mic + system audio',
      icon: Calendar,
      accent: 'text-indigo-400',
      action: 'Open Meetings →',
      onSelect: () => onNavigate('meetings'),
    },
    {
      id: 'web',
      title: 'Web Capture',
      subtitle: 'Pages & AI chats',
      icon: Globe,
      accent: 'text-sky-500',
      action: 'Open Captures →',
      meta: { kind: 'status', text: bridgeLabel },
      onSelect: () => onNavigate('captures'),
    },
  ];

  return (
    <section className="space-y-2.5">
      <div className="flex items-center justify-between">
        <h2 className="text-[10px] font-mono uppercase tracking-widest text-muted-foreground">
          Start a capture
        </h2>
        <Badge variant="outline" className="text-[9px] font-mono text-muted-foreground px-1.5 py-0">
          Everything stays on this machine
        </Badge>
      </div>

      <div className="grid grid-cols-2 sm:grid-cols-3 lg:grid-cols-6 gap-2.5">
        {cards.map((card) => (
          <ShortcutButton key={card.id} card={card} />
        ))}
      </div>

      {/* Reading back out of what was captured. */}
      <div className="grid grid-cols-1 sm:grid-cols-2 gap-2.5 pt-0.5">
        <button
          type="button"
          onClick={() => onNavigate('talkback')}
          className="p-3.5 rounded-lg border border-border bg-card hover:bg-muted/40 hover:border-primary/50 text-left flex items-center gap-3 transition-all focus:outline-none focus-visible:ring-2 focus-visible:ring-ring"
        >
          <div className="w-8 h-8 rounded-lg bg-emerald-500/10 text-emerald-500 flex items-center justify-center shrink-0">
            <MessageCircle className="w-4 h-4" />
          </div>
          <div className="min-w-0">
            <span className="text-xs font-bold text-foreground block">Ask Relay</span>
            <span className="text-[10px] text-muted-foreground block truncate">
              Talkback answers from your own capture, with sources
            </span>
          </div>
        </button>

        <button
          type="button"
          onClick={() => onNavigate('graph')}
          className="p-3.5 rounded-lg border border-border bg-card hover:bg-muted/40 hover:border-primary/50 text-left flex items-center gap-3 transition-all focus:outline-none focus-visible:ring-2 focus-visible:ring-ring"
        >
          <div className="w-8 h-8 rounded-lg bg-blue-500/10 text-blue-500 flex items-center justify-center shrink-0">
            <Network className="w-4 h-4" />
          </div>
          <div className="min-w-0">
            <span className="text-xs font-bold text-foreground block">Knowledge Graph</span>
            <span className="text-[10px] text-muted-foreground block truncate">
              See how thoughts, topics and entities connect
            </span>
          </div>
        </button>
      </div>
    </section>
  );
};
