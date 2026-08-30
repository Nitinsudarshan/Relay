import React, { useEffect, useRef, useState } from 'react';
import { Mic, MicOff, Send, Square, AlertTriangle } from 'lucide-react';
import { Button } from '@/components/ui/button';
import { Badge } from '@/components/ui/badge';
import { TalkbackAgent } from './TalkbackAgent';
import { useTalkback, sourceLabel } from './useTalkback';
import type { TalkbackContextItem, TalkbackTurn } from '../../types';

/** Source chips: where the answer actually came from, per turn. */
const SourceChips: React.FC<{ sources: TalkbackContextItem[] }> = ({ sources }) => {
  if (sources.length === 0) return null;
  return (
    <div className="flex flex-wrap gap-1.5 mt-2">
      {sources.map((source) => (
        <Badge
          key={`${source.source_type}-${source.source_id}`}
          variant="secondary"
          className="text-[10px] font-normal gap-1"
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

const TurnBubble: React.FC<{ turn: TalkbackTurn }> = ({ turn }) => {
  const isUser = turn.role === 'user';
  return (
    <div className={`flex ${isUser ? 'justify-end' : 'justify-start'}`}>
      <div
        className={`max-w-[80%] rounded-2xl px-4 py-2.5 text-sm ${
          isUser
            ? 'bg-primary text-primary-foreground rounded-br-sm'
            : 'bg-muted text-foreground rounded-bl-sm'
        }`}
      >
        <p className="whitespace-pre-wrap leading-relaxed">{turn.text}</p>
        {!isUser && <SourceChips sources={turn.sources ?? []} />}
      </div>
    </div>
  );
};

/**
 * Settings › nothing — this is a first-class surface, not a panel.
 *
 * Talkback is the conversational interface over everything Relay has
 * captured. Voice and text enter the *same* engine (the text box is not a
 * fallback chatbot, it is the same turn without a microphone), which is
 * what keeps one conversation rather than two.
 */
export const TalkbackPage: React.FC = () => {
  const {
    state,
    turns,
    streamingText,
    level,
    lastMetrics,
    error,
    busy,
    start,
    stop,
    interrupt,
    send,
  } = useTalkback();
  const [draft, setDraft] = useState('');
  const transcriptEnd = useRef<HTMLDivElement>(null);

  const isOn = state !== 'OFF';
  const isSpeaking = state === 'SPEAKING';

  useEffect(() => {
    transcriptEnd.current?.scrollIntoView({ behavior: 'smooth' });
  }, [turns.length, streamingText]);

  const handleSend = async (event: React.FormEvent) => {
    event.preventDefault();
    const text = draft.trim();
    if (!text) return;
    setDraft('');
    // Text turns work with Talkback off by starting a text-only session —
    // no microphone is opened for them.
    if (!isOn) await start(false);
    await send(text);
  };

  return (
    <div className="flex-1 flex flex-col gap-4 min-h-0">
      <div className="flex flex-col lg:flex-row gap-4 flex-1 min-h-0">
        {/* Agent + controls */}
        <aside className="lg:w-72 shrink-0 flex flex-col items-center gap-5 bg-card border border-border rounded-xl p-6">
          <TalkbackAgent state={state} level={level} />

          <div className="w-full flex flex-col gap-2">
            <Button
              onClick={() => (isOn ? stop() : start(true))}
              disabled={busy}
              variant={isOn ? 'secondary' : 'default'}
              className="w-full gap-2"
            >
              {isOn ? <MicOff className="w-4 h-4" /> : <Mic className="w-4 h-4" />}
              {isOn ? 'Turn Talkback off' : 'Turn Talkback on'}
            </Button>

            {isSpeaking && (
              <Button
                onClick={() => interrupt()}
                variant="outline"
                className="w-full gap-2"
              >
                <Square className="w-3.5 h-3.5" />
                Stop speaking
              </Button>
            )}
          </div>

          <p className="text-[11px] leading-relaxed text-muted-foreground text-center">
            {isOn
              ? 'Your microphone is active. Talk over Relay any time to interrupt it.'
              : 'Your microphone is off. Nothing is captured until you switch Talkback on.'}
          </p>

          {lastMetrics && (
            <dl className="w-full text-[10px] text-muted-foreground font-mono space-y-0.5 border-t border-border pt-3">
              <div className="flex justify-between">
                <dt>retrieval</dt>
                <dd>
                  {lastMetrics.retrieval_ms}ms · {lastMetrics.retrieved_count}/
                  {lastMetrics.candidate_count}
                </dd>
              </div>
              {lastMetrics.llm_first_token_ms != null && (
                <div className="flex justify-between">
                  <dt>first token</dt>
                  <dd>{lastMetrics.llm_first_token_ms}ms</dd>
                </div>
              )}
              {lastMetrics.tts_first_audio_ms != null && (
                <div className="flex justify-between">
                  <dt>first audio</dt>
                  <dd>{lastMetrics.tts_first_audio_ms}ms</dd>
                </div>
              )}
              <div className="flex justify-between">
                <dt>turn</dt>
                <dd>{lastMetrics.total_ms}ms</dd>
              </div>
            </dl>
          )}
        </aside>

        {/* Conversation */}
        <section className="flex-1 min-w-0 flex flex-col bg-card border border-border rounded-xl overflow-hidden">
          <div className="flex-1 overflow-y-auto p-4 space-y-3 min-h-0">
            {turns.length === 0 && !streamingText && (
              <div className="h-full flex flex-col items-center justify-center text-center gap-2 px-6">
                <p className="text-sm font-medium text-foreground">
                  Ask Relay what it remembers.
                </p>
                <p className="text-xs text-muted-foreground max-w-sm leading-relaxed">
                  “What did we decide about pricing?” · “Catch me up on the infra
                  work” · “Turn this into a Scribble”. Answers about your own
                  history come only from your Voice Notes, Scribbles and
                  Meetings — never from guesswork.
                </p>
              </div>
            )}

            {turns.map((turn) => (
              <TurnBubble key={turn.turn_id} turn={turn} />
            ))}

            {streamingText && (
              <div className="flex justify-start">
                <div className="max-w-[80%] rounded-2xl rounded-bl-sm px-4 py-2.5 text-sm bg-muted text-foreground">
                  <p className="whitespace-pre-wrap leading-relaxed">
                    {streamingText}
                  </p>
                </div>
              </div>
            )}

            <div ref={transcriptEnd} />
          </div>

          {error && (
            <div className="flex items-start gap-2 px-4 py-2 bg-destructive/10 border-t border-destructive/30 text-xs text-destructive">
              <AlertTriangle className="w-3.5 h-3.5 mt-0.5 shrink-0" />
              <span>{error}</span>
            </div>
          )}

          <form
            onSubmit={handleSend}
            className="flex items-center gap-2 border-t border-border p-3"
          >
            <input
              value={draft}
              onChange={(event) => setDraft(event.target.value)}
              placeholder="Type instead of talking…"
              aria-label="Talkback message"
              className="flex-1 bg-background border border-input rounded-lg px-3 py-2 text-sm outline-none focus-visible:ring-2 focus-visible:ring-ring"
            />
            <Button type="submit" size="icon" disabled={!draft.trim()} aria-label="Send">
              <Send className="w-4 h-4" />
            </Button>
          </form>
        </section>
      </div>
    </div>
  );
};
