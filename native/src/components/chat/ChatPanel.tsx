import React, { useRef, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { Mic, MicOff, Loader2, Volume2, BookOpen, Bot, User } from 'lucide-react';
import { ProcessedPipelineResult } from '../../types';
import { Card, CardHeader, CardTitle, CardDescription, CardContent } from '@/components/ui/card';
import { Badge } from '@/components/ui/badge';

interface ChatTurn {
  question: string;
  answer: string;
  sources: string[];
  audioBase64?: string | null;
}

/**
 * "Voice input inside the app": record a question, transcribe it, answer it
 * grounded in the user's own vault notes, and speak the answer back if a
 * local TTS engine is configured. Distinct from the meeting/scribble PTT
 * capture modes — this is a conversational Q&A surface, not a note-taker.
 */
export const ChatPanel: React.FC = () => {
  const [isRecording, setIsRecording] = useState(false);
  const [isProcessing, setIsProcessing] = useState(false);
  const [statusText, setStatusText] = useState('Click the mic and ask a question about your notes');
  const [turns, setTurns] = useState<ChatTurn[]>([]);
  const audioRef = useRef<HTMLAudioElement | null>(null);

  const handleStart = async () => {
    try {
      setIsRecording(true);
      setStatusText('Listening…');
      await invoke('start_capture', { mode: 'chat' });
    } catch (err) {
      console.error('Failed to start voice chat recording', err);
      setStatusText('Recording failed to start');
      setIsRecording(false);
    }
  };

  const handleStop = async () => {
    if (!isRecording) return;
    try {
      setIsRecording(false);
      setIsProcessing(true);
      setStatusText('Transcribing & searching your notes…');

      const result = await invoke<ProcessedPipelineResult | null>('stop_capture');
      if (result) {
        setTurns((prev) => [
          ...prev,
          {
            question: result.transcript,
            answer: result.output_markdown,
            sources: result.sources,
            audioBase64: result.spoken_audio_base64,
          },
        ]);

        if (result.spoken_audio_base64 && audioRef.current) {
          audioRef.current.src = `data:audio/wav;base64,${result.spoken_audio_base64}`;
          void audioRef.current.play();
        }
      }
      // No audio was captured (silence/no input) — nothing to answer, just
      // fall through to resetting the status text below.
      setStatusText('Click the mic and ask a question about your notes');
    } catch (err) {
      console.error('Voice chat failed', err);
      setStatusText('Something went wrong — check your Whisper model & LLM provider settings');
    } finally {
      setIsProcessing(false);
    }
  };

  const toggleRecording = () => {
    if (isRecording) {
      void handleStop();
    } else {
      void handleStart();
    }
  };

  return (
    <Card className="h-full flex flex-col border-slate-800">
      <CardHeader className="flex-row items-center justify-between pb-3 space-y-0">
        <div className="flex items-center gap-2">
          <Bot className="w-5 h-5 text-blue-400" />
          <div>
            <CardTitle>Voice Chat</CardTitle>
            <CardDescription>Ask questions out loud, grounded in your own vault notes</CardDescription>
          </div>
        </div>
        <button
          onClick={toggleRecording}
          disabled={isProcessing}
          className={`flex items-center gap-2 rounded-lg px-4 py-2 text-xs font-semibold transition-all ${
            isRecording
              ? 'bg-red-500 text-white recording-pulse'
              : isProcessing
              ? 'bg-amber-500/20 text-amber-400 border border-amber-500/40'
              : 'bg-gradient-to-tr from-blue-600 to-indigo-600 text-white hover:from-blue-500 hover:to-indigo-500'
          }`}
        >
          {isProcessing ? (
            <Loader2 className="w-4 h-4 animate-spin" />
          ) : isRecording ? (
            <MicOff className="w-4 h-4" />
          ) : (
            <Mic className="w-4 h-4" />
          )}
          {isRecording ? 'Stop' : 'Ask'}
        </button>
      </CardHeader>

      <CardContent className="flex-1 overflow-y-auto space-y-4 pr-1">
        <p className="text-xs text-slate-500 italic">{statusText}</p>

        {turns.length === 0 && (
          <div className="text-center py-10 text-slate-500 text-xs italic">
            No questions asked yet this session.
          </div>
        )}

        {turns.map((turn, i) => (
          <div key={i} className="space-y-2">
            <div className="flex items-start gap-2 justify-end">
              <div className="bg-blue-600/20 border border-blue-500/30 rounded-lg rounded-tr-sm px-3.5 py-2.5 max-w-[85%] text-xs text-slate-100">
                {turn.question}
              </div>
              <User className="w-5 h-5 text-blue-400 shrink-0 mt-1" />
            </div>

            <div className="flex items-start gap-2">
              <Bot className="w-5 h-5 text-purple-400 shrink-0 mt-1" />
              <div className="bg-slate-950/80 border border-slate-800 rounded-lg rounded-tl-sm px-3.5 py-2.5 max-w-[85%] space-y-2">
                <p className="text-xs text-slate-200 whitespace-pre-wrap">{turn.answer}</p>

                {turn.sources.length > 0 && (
                  <div className="flex flex-wrap items-center gap-1.5 pt-1 border-t border-slate-800/80">
                    <BookOpen className="w-3 h-3 text-slate-500" />
                    {turn.sources.map((s, j) => (
                      <Badge key={j} variant="secondary" className="text-[10px]">
                        {s}
                      </Badge>
                    ))}
                  </div>
                )}

                {turn.audioBase64 && (
                  <div className="flex items-center gap-1.5 text-[10px] text-slate-500 pt-1">
                    <Volume2 className="w-3 h-3" />
                    Spoken aloud
                  </div>
                )}
              </div>
            </div>
          </div>
        ))}
      </CardContent>

      {/* Hidden player used to speak answers back; browser <audio> works fine
          inside the Tauri webview without a dedicated plugin. */}
      <audio ref={audioRef} className="hidden" />
    </Card>
  );
};
