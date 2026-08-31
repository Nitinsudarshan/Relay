import React, { useCallback, useEffect, useRef, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { Mic, MicOff, Send, Square, AlertTriangle, VolumeX } from 'lucide-react';
import { Button } from '@/components/ui/button';
import { Badge } from '@/components/ui/badge';
import { TalkbackAgent } from './TalkbackAgent';
import { useTalkback, sourceLabel } from './useTalkback';
import type { TalkbackContextItem, TalkbackTurn, TtsStatus } from '../../types';

import { TalkbackImmersiveView } from './TalkbackImmersiveView';

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
    currentSpokenPhrase,
    spokenPhrases,
    lastUserTranscript,
    level,
    outputLevel,
    lastMetrics,
    error,
    busy,
    start,
    stop,
    interrupt,
    send,
  } = useTalkback();
  const [draft, setDraft] = useState('');
  const [voice, setVoice] = useState<TtsStatus | null>(null);
  const [viewPreference, setViewPreference] = useState<'immersive' | 'split'>('immersive');
  const transcriptEnd = useRef<HTMLDivElement>(null);

  const isOn = state !== 'OFF';
  const isSpeaking = state === 'SPEAKING';
  const isImmersive = isOn && viewPreference === 'immersive';

  const refreshVoice = useCallback(async () => {
    try {
      setVoice(await invoke<TtsStatus>('get_tts_status'));
    } catch {
      // A voice-status failure must never break the conversation; the
      // banner simply does not appear.
      setVoice(null);
    }
  }, []);

  useEffect(() => {
    void refreshVoice();
    // Settings changes come from the settings window, so the banner
    // clears itself the moment a voice is configured — no restart, no
    // navigating back and forth.
    const unlisten = listen('settings-changed', () => void refreshVoice());
    return () => {
      void unlisten.then((stop) => stop());
    };
  }, [refreshVoice]);

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

  const handleStartTalkback = async () => {
    setViewPreference('immersive');
    await start(true);
  };

  if (isImmersive) {
    return (
      <div className="flex-1 flex flex-col min-h-0 w-full animate-in fade-in duration-300">
        <TalkbackImmersiveView
          state={state}
          level={level}
          outputLevel={outputLevel}
          currentSpokenPhrase={currentSpokenPhrase}
          spokenPhrases={spokenPhrases}
          lastUserTranscript={lastUserTranscript}
          streamingText={streamingText}
          turns={turns}
          voice={voice}
          error={error}
          busy={busy}
          onToggleTalkback={stop}
          onInterrupt={interrupt}
          onSend={send}
          onSwitchToStandard={() => setViewPreference('split')}
        />
      </div>
    );
  }

  return (
    <div className="flex-1 flex flex-col gap-4 min-h-0 animate-in fade-in duration-300">
      <div className="flex flex-col lg:flex-row gap-4 flex-1 min-h-0">
        {/* Agent + controls */}
        <aside className="lg:w-72 shrink-0 flex flex-col items-center gap-5 bg-card border border-border rounded-xl p-6">
          <TalkbackAgent state={state} level={level} outputLevel={outputLevel} />

          <div className="w-full flex flex-col gap-2">
            <Button
              onClick={() => (isOn ? stop() : handleStartTalkback())}
              disabled={busy}
              variant={isOn ? 'secondary' : 'default'}
              className="w-full gap-2"
            >
              {isOn ? <MicOff className="w-4 h-4" /> : <Mic className="w-4 h-4" />}
              {isOn ? 'Turn Talkback off' : 'Turn Talkback on'}
            </Button>

            {isOn && (
              <Button
                onClick={() => setViewPreference('immersive')}
                variant="outline"
                className="w-full gap-2 text-xs"
              >
                Enter immersive mode
              </Button>
            )}

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

          {voice && !voice.ready && (
            <div
              className="w-full rounded-lg border border-amber-500/30 bg-amber-500/10 px-3 py-2.5"
              data-testid="voice-unavailable"
            >
              <p className="flex items-center gap-1.5 text-[11px] font-semibold text-amber-700 dark:text-amber-400">
                <VolumeX className="w-3.5 h-3.5 shrink-0" />
                Voice unavailable
              </p>
              <p className="mt-1 text-[11px] leading-relaxed text-muted-foreground">
                {voice.canInstall
                  ? 'Relay can set up a voice that runs on this computer.'
                  : (voice.installBlockedReason ??
                    'A local voice is not set up.')}{' '}
                Talkback still answers in text.
              </p>
              {voice.canInstall && (
                <Button
                  variant="outline"
                  size="sm"
                  className="mt-2 h-7 w-full text-[11px]"
                  onClick={() => {
                    window.dispatchEvent(
                      new CustomEvent('relay-navigate-tab', {
                        detail: { tab: 'settings', section: 'talkback' },
                      }),
                    );
                    void invoke('open_settings_window', { section: 'talkback' });
                  }}
                >
                  Set up local voice
                </Button>
              )}
            </div>
          )}

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
                  <dt title="From the start of the turn to audio being ready">
                    first audio
                  </dt>
                  <dd>{lastMetrics.tts_first_audio_ms}ms</dd>
                </div>
              )}
              {lastMetrics.tts_first_synthesis_ms != null && (
                <div className="flex justify-between">
                  <dt title="The first synthesis call on its own">synthesis</dt>
                  <dd>
                    {lastMetrics.tts_first_synthesis_ms}ms ·{' '}
                    {lastMetrics.tts_phrases ?? 0} phr
                  </dd>
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
