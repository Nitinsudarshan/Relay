import React, { useEffect, useState } from 'react';
import {
  MicOff,
  Square,
  History,
  LayoutGrid,
  Send,
  MessageSquare,
  X,
} from 'lucide-react';
import { Button } from '@/components/ui/button';
import { Badge } from '@/components/ui/badge';
import {
  TalkbackAgent,
  agentStateLabel,
  visualStateFor,
  STATE_PALETTE,
} from './TalkbackAgent';
import { sourceLabel } from './useTalkback';
import type {
  TalkbackContextItem,
  TalkbackStateName,
  TalkbackTurn,
  TtsStatus,
} from '../../types';

interface TalkbackImmersiveViewProps {
  state: TalkbackStateName;
  level: number;
  outputLevel: number;
  currentSpokenPhrase: string;
  spokenPhrases: string[];
  lastUserTranscript: string;
  streamingText: string;
  turns: TalkbackTurn[];
  voice: TtsStatus | null;
  error: string | null;
  busy: boolean;
  onToggleTalkback: () => void;
  onInterrupt: () => void;
  onSend: (text: string) => Promise<void>;
  onSwitchToStandard: () => void;
}

/** Source chips for turn references in History */
const SourceChips: React.FC<{ sources: TalkbackContextItem[] }> = ({ sources }) => {
  if (sources.length === 0) return null;
  return (
    <div className="flex flex-wrap items-center justify-center gap-1.5 mt-2">
      {sources.map((source) => (
        <Badge
          key={`${source.source_type}-${source.source_id}`}
          variant="secondary"
          className="text-[10px] font-normal gap-1 bg-muted/60 hover:bg-muted"
          title={source.excerpt}
        >
          <span className="font-medium">{sourceLabel(source)}</span>
          <span className="text-muted-foreground">{source.title}</span>
          {source.expanded && (
            <span className="text-muted-foreground/70 italic">related</span>
          )}
        </Badge>
      ))}
    </div>
  );
};

export const TalkbackImmersiveView: React.FC<TalkbackImmersiveViewProps> = ({
  state,
  level,
  outputLevel,
  spokenPhrases,
  lastUserTranscript,
  streamingText,
  turns,
  voice,
  error,
  onToggleTalkback,
  onInterrupt,
  onSend,
  onSwitchToStandard,
}) => {
  const [showHistory, setShowHistory] = useState(false);
  const [typedInput, setTypedInput] = useState('');
  const [showTextInput, setShowTextInput] = useState(false);

  const visualState = visualStateFor(state);
  const palette = STATE_PALETTE[visualState];
  const stateLabel = agentStateLabel(state);
  const isSpeaking = state === 'SPEAKING';
  const isVoiceReady = voice === null || voice.ready;

  // Global keyboard shortcuts: Escape (to toggle off or close drawers/input)
  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.key === 'Escape') {
        if (showTextInput) {
          setShowTextInput(false);
        } else if (showHistory) {
          setShowHistory(false);
        } else {
          onToggleTalkback();
        }
      }
    };
    window.addEventListener('keydown', handleKeyDown);
    return () => window.removeEventListener('keydown', handleKeyDown);
  }, [showHistory, showTextInput, onToggleTalkback]);

  const dialogScrollRef = React.useRef<HTMLDivElement>(null);

  // Automatically scroll to the latest spoken text without growing the container
  useEffect(() => {
    if (dialogScrollRef.current) {
      dialogScrollRef.current.scrollTop = dialogScrollRef.current.scrollHeight;
    }
  }, [spokenPhrases.length, streamingText, lastUserTranscript]);

  const handleSendSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!typedInput.trim()) return;
    const text = typedInput.trim();
    setTypedInput('');
    setShowTextInput(false);
    await onSend(text);
  };

  return (
    <div
      className="fixed inset-0 z-40 flex flex-col items-center justify-between min-h-0 w-full overflow-hidden select-none bg-background px-6 py-5 animate-in fade-in duration-300"
      data-testid="talkback-immersive-view"
    >
      {/* Top Header Bar — Spread fully to the ends */}
      <div className="w-full flex items-center justify-between z-20 shrink-0">
        {/* Far Left: Turn off / Escape hint */}
        <Button
          variant="ghost"
          size="sm"
          onClick={onToggleTalkback}
          className="text-xs text-muted-foreground hover:text-foreground gap-2 h-8 px-2.5 rounded-lg border border-border/40 hover:border-border hover:bg-muted/40 transition-colors"
          title="Turn Talkback off (Press Escape)"
        >
          <MicOff className="w-3.5 h-3.5 text-muted-foreground" />
          <span>Turn off</span>
          <kbd className="text-[10px] font-mono bg-muted px-1.5 py-0.5 rounded border border-border/60 text-muted-foreground font-semibold">
            Esc
          </kbd>
        </Button>

        {/* Far Right: Stop speaking (if speaking), Type message, History, Split view */}
        <div className="flex items-center gap-2">
          {isSpeaking && (
            <Button
              variant="outline"
              size="sm"
              onClick={onInterrupt}
              className="text-xs text-destructive border-destructive/40 hover:bg-destructive/10 gap-1.5 h-8 px-2.5 rounded-lg animate-in fade-in"
              title="Stop speaking"
            >
              <Square className="w-3.5 h-3.5 fill-destructive" />
              <span>Stop speaking</span>
            </Button>
          )}

          <Button
            variant={showTextInput ? 'secondary' : 'ghost'}
            size="sm"
            onClick={() => setShowTextInput((prev) => !prev)}
            className="text-xs text-muted-foreground hover:text-foreground gap-1.5 h-8 px-2.5 rounded-lg border border-transparent hover:border-border/40"
            title="Type a question"
          >
            <MessageSquare className="w-3.5 h-3.5" />
            <span>Type message</span>
          </Button>

          {turns.length > 0 && (
            <Button
              variant={showHistory ? 'secondary' : 'ghost'}
              size="sm"
              onClick={() => setShowHistory((prev) => !prev)}
              className="text-xs text-muted-foreground hover:text-foreground gap-1.5 h-8 px-2.5 rounded-lg border border-transparent hover:border-border/40"
              title="Toggle turn history"
            >
              <History className="w-3.5 h-3.5" />
              <span>History ({turns.length})</span>
            </Button>
          )}

          <Button
            variant="ghost"
            size="sm"
            onClick={onSwitchToStandard}
            className="text-xs text-muted-foreground hover:text-foreground gap-1.5 h-8 px-2.5 rounded-lg border border-transparent hover:border-border/40"
            title="Switch to split panel view"
          >
            <LayoutGrid className="w-3.5 h-3.5" />
            <span>Split view</span>
          </Button>
        </div>
      </div>

      {/* Floating Type Input Bar (when toggled open at top) */}
      {showTextInput && (
        <div className="w-full max-w-2xl z-30 mt-2 animate-in slide-in-from-top-2 duration-200">
          <form
            onSubmit={handleSendSubmit}
            className="w-full flex items-center gap-2 bg-card/95 backdrop-blur border border-border rounded-xl p-2 shadow-xl"
          >
            <input
              type="text"
              value={typedInput}
              onChange={(e) => setTypedInput(e.target.value)}
              placeholder="Ask Relay a question without speaking…"
              className="flex-1 bg-transparent border-0 px-3 py-1.5 text-sm outline-none placeholder:text-muted-foreground"
              autoFocus
            />
            <Button
              type="submit"
              size="sm"
              disabled={!typedInput.trim()}
              className="gap-1 text-xs h-8"
            >
              <Send className="w-3.5 h-3.5" />
              Send
            </Button>
            <Button
              type="button"
              variant="ghost"
              size="icon"
              onClick={() => setShowTextInput(false)}
              className="h-8 w-8 text-muted-foreground"
              title="Close input"
            >
              <X className="w-3.5 h-3.5" />
            </Button>
          </form>
        </div>
      )}

      {/* Central Living Presence Canvas (Layer 1 + Layer 2 — positioned higher up) */}
      <div className="flex-1 flex flex-col items-center justify-center -mt-8 sm:-mt-12 w-full text-center gap-3.5 z-10 min-h-0">
        {/* Layer 1: Large Living Entity Canvas */}
        <div className="relative flex items-center justify-center transition-transform duration-700 ease-out">
          <TalkbackAgent
            state={state}
            level={level}
            outputLevel={outputLevel}
            size={450}
            showCaption={false}
          />
        </div>

        {/* Layer 2: Clean, Single State Indicator */}
        <div className="flex flex-col items-center">
          <p
            className={`text-sm font-semibold tracking-wide transition-colors duration-300 ${palette.core}`}
            aria-live="polite"
          >
            {stateLabel}
          </p>
        </div>
      </div>

      {/* Bottom Area: Conversational Dialog Box (Anchored in bottom ~25% of screen with ZERO layout shift) */}
      <div className="w-full flex flex-col items-center justify-end z-10 shrink-0 pb-4 gap-2.5 px-2 sm:px-6">
        {/* Layer 3: Spacious Dialog Box (80% of viewport width) */}
        <div
          ref={dialogScrollRef}
          className="w-[80vw] h-[235px] max-h-[235px] shrink-0 overflow-y-auto px-10 py-6 rounded-2xl bg-card/40 border border-border/50 backdrop-blur-sm shadow-sm flex flex-col items-center justify-center text-center gap-3 scrollbar-thin transition-all duration-300"
        >
          {/* User's recent question in CC style (Emerald) */}
          {lastUserTranscript && (
            <p className="text-base sm:text-lg font-medium text-emerald-600 dark:text-emerald-400 leading-relaxed w-full max-w-[75vw] transition-opacity duration-300">
              "{lastUserTranscript}"
            </p>
          )}

          {/* Relay's Response in CC subtitle style (Foreground / Slate) — only active spoken phrases or text-only streaming */}
          {spokenPhrases.length > 0 ? (
            <div className="text-base sm:text-lg font-normal leading-relaxed text-foreground w-full max-w-[75vw] transition-all duration-300">
              {spokenPhrases.map((phrase, idx) => (
                <span
                  key={idx}
                  className="transition-opacity duration-300"
                >
                  {phrase}{' '}
                </span>
              ))}
            </div>
          ) : !isVoiceReady && streamingText ? (
            <div className="text-base sm:text-lg font-normal leading-relaxed text-foreground w-full max-w-[75vw] transition-all duration-300">
              <span>{streamingText}</span>
            </div>
          ) : null}
        </div>

        {/* Error message toast */}
        {error && (
          <div className="w-full max-w-xl bg-destructive/10 border border-destructive/30 text-destructive text-xs px-3 py-2 rounded-lg text-center animate-in fade-in">
            {error}
          </div>
        )}
      </div>

      {/* History Slide-over Drawer */}
      {showHistory && (
        <div className="absolute inset-y-0 right-0 w-full sm:w-96 bg-card/95 backdrop-blur-md border-l border-border z-50 flex flex-col shadow-2xl animate-in slide-in-from-right duration-300">
          <div className="flex items-center justify-between p-4 border-b border-border">
            <h3 className="text-sm font-semibold text-foreground flex items-center gap-2">
              <History className="w-4 h-4 text-muted-foreground" />
              Conversation History
            </h3>
            <Button
              variant="ghost"
              size="sm"
              onClick={() => setShowHistory(false)}
              className="h-8 w-8 p-0"
            >
              ✕
            </Button>
          </div>

          <div className="flex-1 overflow-y-auto p-4 space-y-3">
            {turns.length === 0 ? (
              <p className="text-xs text-muted-foreground text-center py-8">
                No turns yet in this session.
              </p>
            ) : (
              turns.map((t) => (
                <div
                  key={t.turn_id}
                  className={`flex flex-col gap-1 text-xs p-3 rounded-xl ${
                    t.role === 'user'
                      ? 'bg-primary/10 text-foreground ml-4 border border-primary/20'
                      : 'bg-muted text-foreground mr-4 border border-border'
                  }`}
                >
                  <span className="font-semibold text-[10px] uppercase text-muted-foreground">
                    {t.role === 'user' ? 'You' : 'Relay'}
                  </span>
                  <p className="leading-relaxed whitespace-pre-wrap">{t.text}</p>
                  {t.role === 'agent' && t.sources && (
                    <SourceChips sources={t.sources} />
                  )}
                </div>
              ))
            )}
          </div>
        </div>
      )}
    </div>
  );
};
