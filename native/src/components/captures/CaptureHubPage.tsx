import React, { useState, useEffect } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { AppSettings, MainTabType, Scribble } from '../../types';
import {
  Mic,
  FileText,
  Upload,
  Clipboard,
  Globe,
  Users,
  Sparkles,
  Check,
  Plus,
  X,
  Loader2,
  ArrowRight,
  Command,
  type LucideIcon,
} from 'lucide-react';
import { Button } from '@/components/ui/button';
import { Badge } from '@/components/ui/badge';

/**
 * The capture methods this hub performs in place.
 *
 * Deliberately only two. Voice is a global hotkey on its own surface, a
 * document belongs in the Files Vault (which extracts PDF and Word text rather
 * than reading the bytes as if they were plain text), a page comes from the
 * browser extension, and a meeting is a recording — each of those is owned by
 * a surface that already implements it, so the card opens that surface instead
 * of reimplementing it here.
 */
export type CaptureMethod = 'text' | 'clipboard';

interface CaptureHubPageProps {
  /**
   * Reveals a just-captured thought in Scribbles.
   *
   * Nothing else is handed back: `create_scribble` emits `scribble-saved`, which
   * Scribbles already listens for, so a second notification path would only be a
   * second thing to keep true.
   */
  onOpenScribble: (scribbleId: string) => void;
  /** Hands the user to the surface that owns a mode this hub does not perform. */
  onNavigate: (tab: MainTabType) => void;
  /** Switches to the captured-pages list, which is this surface's other tab. */
  onOpenCapturedPages: () => void;
  /** Method to open on. Set by a Home shortcut card; defaults to typed text. */
  initialMethod?: CaptureMethod | null;
  /** Whether the capture bridge is actually listening, or null when unknown. */
  bridgeRunning?: boolean | null;
}

/** A mode this hub hands off rather than performs. */
interface HandoffCard {
  id: string;
  title: string;
  subtitle: string;
  icon: LucideIcon;
  accent: string;
  action: string;
  onSelect: () => void;
}

export const CaptureHubPage: React.FC<CaptureHubPageProps> = ({
  onOpenScribble,
  onNavigate,
  onOpenCapturedPages,
  initialMethod = null,
  bridgeRunning = null,
}) => {
  const [selectedMethod, setSelectedMethod] = useState<CaptureMethod>(initialMethod ?? 'text');
  const [textContent, setTextContent] = useState('');
  const [textTitle, setTextTitle] = useState('');
  const [topicInput, setTopicInput] = useState('');
  const [topics, setTopics] = useState<string[]>([]);
  const [busy, setBusy] = useState(false);
  const [lastCaptured, setLastCaptured] = useState<Scribble | null>(null);

  // The accelerator is read rather than assumed: it is user-configurable, and a
  // card that names the wrong key is worse than one that names none.
  const [dictationHotkey, setDictationHotkey] = useState<string | null>(null);

  useEffect(() => {
    invoke<AppSettings>('get_settings')
      .then((s) => setDictationHotkey(s?.hotkeys?.dictation_hotkey ?? null))
      .catch(() => setDictationHotkey(null));
  }, []);

  const handleAddTopic = () => {
    const trimmed = topicInput.trim();
    if (trimmed && !topics.includes(trimmed)) {
      setTopics([...topics, trimmed]);
      setTopicInput('');
    }
  };

  const handleRemoveTopic = (t: string) => {
    setTopics(topics.filter((x) => x !== t));
  };

  const handleCreateTextScribble = async () => {
    if (!textContent.trim()) return;
    setBusy(true);
    try {
      const scribble = await invoke<Scribble>('create_scribble', {
        content: textContent.trim(),
        title: textTitle.trim() || undefined,
        sourceType: 'text',
        topics: topics.length > 0 ? topics : undefined,
      });

      setLastCaptured(scribble);
      setTextContent('');
      setTextTitle('');
      setTopics([]);
      setTopicInput('');
    } catch (err) {
      console.error('Failed to create Scribble:', err);
    } finally {
      setBusy(false);
    }
  };

  const handlePasteClipboard = async () => {
    try {
      const text = await navigator.clipboard.readText();
      if (!text.trim()) return;
      setBusy(true);
      const scribble = await invoke<Scribble>('create_scribble', {
        content: text.trim(),
        sourceType: 'clipboard',
      });
      setLastCaptured(scribble);
    } catch (err) {
      console.error('Failed to read from clipboard:', err);
    } finally {
      setBusy(false);
    }
  };

  /**
   * The three modes another surface owns.
   *
   * The web-capture card names whether the bridge is actually listening rather
   * than implying it is, per `rules/ui-components.md`'s no-fake-controls rule.
   */
  const handoffs: HandoffCard[] = [
    {
      id: 'file',
      title: 'Files & Docs',
      subtitle: 'PDF, Word, Markdown, Text',
      icon: Upload,
      accent: 'text-blue-500',
      action: 'Open Files Vault →',
      onSelect: () => onNavigate('files'),
    },
    {
      id: 'web',
      title: 'Web Capture',
      subtitle:
        bridgeRunning === null
          ? 'Pages & AI chats'
          : bridgeRunning
            ? 'Pages & AI chats · bridge live'
            : 'Pages & AI chats · bridge off',
      icon: Globe,
      accent: 'text-sky-500',
      action: 'Captured Pages →',
      onSelect: onOpenCapturedPages,
    },
    {
      id: 'meeting',
      title: 'Meeting',
      subtitle: 'Mic + system audio',
      icon: Users,
      accent: 'text-indigo-400',
      action: 'Open Meetings →',
      onSelect: () => onNavigate('meetings'),
    },
  ];

  return (
    <div className="flex-1 flex flex-col gap-4 overflow-y-auto w-full pb-10">
      {/* Capture Method Selector Grid */}
      <div className="grid grid-cols-2 sm:grid-cols-3 md:grid-cols-6 gap-2.5">
        {/* 1. Voice — a global hotkey and its own surface, not a panel here. */}
        <button
          type="button"
          onClick={() => onNavigate('capture')}
          className="p-3.5 rounded-lg border border-border bg-card hover:bg-muted/40 hover:border-primary/50 text-left flex flex-col justify-between space-y-2 transition-all focus:outline-none focus-visible:ring-2 focus-visible:ring-ring"
        >
          <div className="flex items-center justify-between">
            <Mic className="w-4 h-4 text-emerald-500" />
          </div>
          <div>
            <span className="text-xs font-bold text-foreground block">Voice</span>
            <span className="text-[10px] text-muted-foreground block">Global hotkey PTT</span>
          </div>
          <kbd className="font-mono text-[9px] bg-background/80 px-1.5 py-0.5 rounded-lg border border-border flex items-center gap-1 w-fit max-w-full">
            <Command className="w-2.5 h-2.5 shrink-0" />
            <span className="truncate">{dictationHotkey ?? 'Set in Settings'}</span>
          </kbd>
        </button>

        {/* 2. Text (performed here) */}
        <button
          type="button"
          onClick={() => setSelectedMethod('text')}
          aria-pressed={selectedMethod === 'text'}
          className={`p-3.5 rounded-lg border text-left flex flex-col justify-between space-y-2 transition-all focus:outline-none focus-visible:ring-2 focus-visible:ring-ring ${
            selectedMethod === 'text'
              ? 'border-primary bg-accent/60 shadow-xs'
              : 'border-border bg-card hover:bg-muted/40'
          }`}
        >
          <div className="flex items-center justify-between">
            <FileText className="w-4 h-4 text-amber-500" />
          </div>
          <div>
            <span className="text-xs font-bold text-foreground block">Typed Text</span>
            <span className="text-[10px] text-muted-foreground block">Type raw thought</span>
          </div>
          <span className="text-[10px] text-primary font-medium">Quick Compose →</span>
        </button>

        {/* 3. Clipboard (performed here) */}
        <button
          type="button"
          onClick={() => setSelectedMethod('clipboard')}
          aria-pressed={selectedMethod === 'clipboard'}
          className={`p-3.5 rounded-lg border text-left flex flex-col justify-between space-y-2 transition-all focus:outline-none focus-visible:ring-2 focus-visible:ring-ring ${
            selectedMethod === 'clipboard'
              ? 'border-primary bg-accent/60 shadow-xs'
              : 'border-border bg-card hover:bg-muted/40'
          }`}
        >
          <div className="flex items-center justify-between">
            <Clipboard className="w-4 h-4 text-emerald-500" />
          </div>
          <div>
            <span className="text-xs font-bold text-foreground block">Clipboard</span>
            <span className="text-[10px] text-muted-foreground block">1-Click paste buffer</span>
          </div>
          <span className="text-[10px] text-primary font-medium">Paste Buffer →</span>
        </button>

        {/* 4-6. The modes another surface owns. */}
        {handoffs.map((card) => {
          const Icon = card.icon;
          return (
            <button
              key={card.id}
              type="button"
              onClick={card.onSelect}
              className="p-3.5 rounded-lg border border-border bg-card hover:bg-muted/40 hover:border-primary/50 text-left flex flex-col justify-between space-y-2 transition-all focus:outline-none focus-visible:ring-2 focus-visible:ring-ring"
            >
              <div className="flex items-center justify-between">
                <Icon className={`w-4 h-4 ${card.accent}`} />
              </div>
              <div>
                <span className="text-xs font-bold text-foreground block">{card.title}</span>
                <span className="text-[10px] text-muted-foreground block">{card.subtitle}</span>
              </div>
              <span className="text-[10px] text-primary font-medium">{card.action}</span>
            </button>
          );
        })}
      </div>

      {/* Main Active Surface Area */}
      <div className="rounded-lg border border-border bg-card p-6 shadow-xs space-y-5">
        {selectedMethod === 'text' && (
          <div className="space-y-4">
            <div className="flex items-center justify-between">
              <h3 className="text-sm font-bold text-foreground flex items-center gap-2">
                <FileText className="w-4 h-4 text-amber-500" />
                <span>Type a thought directly</span>
              </h3>
              <span className="text-[11px] text-muted-foreground">
                <kbd className="font-mono text-[9px] bg-muted px-1.5 py-0.5 rounded">Ctrl + Enter</kbd> to save
              </span>
            </div>

            <input
              type="text"
              value={textTitle}
              onChange={(e) => setTextTitle(e.target.value)}
              placeholder="Title (Optional — AI will distill a descriptive concept title if left blank)"
              className="w-full text-xs font-semibold bg-muted/20 border border-border/70 rounded-lg p-3 text-foreground focus:outline-none focus:ring-1 focus:ring-ring placeholder:text-muted-foreground/60"
            />

            <textarea
              value={textContent}
              onChange={(e) => setTextContent(e.target.value)}
              onKeyDown={(e) => {
                if (e.key === 'Enter' && (e.ctrlKey || e.metaKey)) {
                  handleCreateTextScribble();
                }
              }}
              placeholder="Capture an observation, idea, question, or note in raw markdown… (AI enrichment will extract topics and connections automatically)"
              className="w-full min-h-[140px] text-xs font-sans bg-muted/20 border border-border/70 rounded-lg p-3 text-foreground focus:outline-none focus:ring-1 focus:ring-ring leading-relaxed resize-y placeholder:text-muted-foreground/60"
            />

            {/* Topic Chips (Semantic Topics, no hashtag tags) */}
            <div className="flex flex-wrap items-center gap-1.5 pt-1">
              <span className="text-[11px] text-muted-foreground mr-1">Topics:</span>
              {topics.map((t) => (
                <Badge key={t} variant="secondary" className="text-[10px] gap-1 px-2 py-0.5 bg-amber-500/10 text-amber-600 dark:text-amber-400 font-sans">
                  <span>{t}</span>
                  <button onClick={() => handleRemoveTopic(t)} className="hover:text-destructive">
                    <X className="w-2.5 h-2.5" />
                  </button>
                </Badge>
              ))}
              <div className="flex items-center gap-1">
                <input
                  type="text"
                  value={topicInput}
                  onChange={(e) => setTopicInput(e.target.value)}
                  onKeyDown={(e) => e.key === 'Enter' && handleAddTopic()}
                  placeholder="Add topic…"
                  className="text-[10px] bg-muted/30 px-2 py-1 rounded border border-border text-foreground w-24 focus:outline-none focus:w-36 transition-all"
                />
                <button onClick={handleAddTopic} className="text-muted-foreground hover:text-foreground">
                  <Plus className="w-3.5 h-3.5" />
                </button>
              </div>
            </div>

            <div className="flex items-center justify-between pt-3 border-t border-border/50">
              <span className="text-[11px] text-muted-foreground flex items-center gap-1">
                <Sparkles className="w-3.5 h-3.5 text-primary" />
                <span>Async AI enrichment extracts summary, topics, and concept connections</span>
              </span>

              <Button
                size="sm"
                onClick={handleCreateTextScribble}
                disabled={busy || !textContent.trim()}
                className="h-8 text-xs gap-1.5 font-semibold"
              >
                {busy ? <Loader2 className="w-3.5 h-3.5 animate-spin" /> : <Plus className="w-3.5 h-3.5" />}
                <span>Save to Knowledge Layer</span>
              </Button>
            </div>
          </div>
        )}

        {selectedMethod === 'clipboard' && (
          <div className="space-y-4 text-center py-6">
            <Clipboard className="w-12 h-12 mx-auto text-emerald-500 opacity-60 mb-2" />
            <h3 className="text-sm font-bold text-foreground">Paste from Clipboard</h3>
            <p className="text-xs text-muted-foreground max-w-md mx-auto leading-relaxed">
              Grab copied text, code snippet, or link currently in your system clipboard and instantly turn it into an enriched Scribble with provenance preserved.
            </p>
            <Button
              size="sm"
              onClick={handlePasteClipboard}
              disabled={busy}
              className="h-8 text-xs gap-1.5 font-semibold"
            >
              {busy ? <Loader2 className="w-3.5 h-3.5 animate-spin" /> : <Clipboard className="w-3.5 h-3.5" />}
              <span>Paste and Save as Scribble</span>
            </Button>
          </div>
        )}
      </div>

      {/* Success Feedback Card */}
      {lastCaptured && (
        <div className="p-4 rounded-lg bg-emerald-500/10 border border-emerald-500/30 flex items-center justify-between gap-4 animate-in fade-in duration-200">
          <div className="flex items-center gap-3 min-w-0">
            <div className="w-8 h-8 rounded-full bg-emerald-500/20 text-emerald-500 flex items-center justify-center shrink-0">
              <Check className="w-4 h-4" />
            </div>
            <div className="min-w-0">
              <span className="text-[10px] font-mono text-emerald-600 dark:text-emerald-400 font-bold uppercase block">
                Saved as Scribble
              </span>
              <h4 className="text-xs font-bold text-foreground truncate">{lastCaptured.title}</h4>
            </div>
          </div>

          <Button
            size="sm"
            variant="outline"
            onClick={() => onOpenScribble(lastCaptured.id)}
            className="h-8 text-xs gap-1.5 shrink-0 bg-background"
          >
            <span>Open in Scribbles</span>
            <ArrowRight className="w-3.5 h-3.5" />
          </Button>
        </div>
      )}
    </div>
  );
};
