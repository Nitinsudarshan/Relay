import React, { useState } from 'react';
import { Mic, MicOff, Sparkles, Kanban, Zap, Loader2 } from 'lucide-react';
import { invoke } from '@tauri-apps/api/core';
import { ProcessedPipelineResult } from '../../types';
import { Button } from '@/components/ui/button';
import { Card } from '@/components/ui/card';
import { Badge } from '@/components/ui/badge';

interface PTTWidgetProps {
  onProcessComplete: (result: ProcessedPipelineResult) => void;
}

export const PTTWidget: React.FC<PTTWidgetProps> = ({ onProcessComplete }) => {
  const [mode, setMode] = useState<'meeting' | 'scribble'>('meeting');
  const [isRecording, setIsRecording] = useState(false);
  const [isProcessing, setIsProcessing] = useState(false);
  const [statusText, setStatusText] = useState('Hold or Click to Record Voice');

  const handleStartRecording = async () => {
    try {
      setIsRecording(true);
      setStatusText(`Recording ${mode === 'meeting' ? 'Meeting Notes' : 'Audio Scribble'}...`);
      await invoke('start_capture', { mode });
    } catch (err) {
      console.error('Failed to start recording', err);
      setStatusText('Recording failed to start');
      setIsRecording(false);
    }
  };

  const handleStopRecording = async () => {
    if (!isRecording) return;
    try {
      setIsRecording(false);
      setIsProcessing(true);
      setStatusText('Processing audio & extracting structured state...');

      const result = await invoke<ProcessedPipelineResult>('stop_capture');
      setIsProcessing(false);
      setStatusText('Processing complete!');
      onProcessComplete(result);
    } catch (err) {
      console.error('Failed to stop recording', err);
      setIsProcessing(false);
      setStatusText('Failed to process audio');
    }
  };

  const toggleRecording = () => {
    if (isRecording) {
      handleStopRecording();
    } else {
      handleStartRecording();
    }
  };

  return (
    <Card className="p-6 mb-6 flex flex-col items-center border-slate-800/80">
      {/* Mode Selector */}
      <div className="flex items-center gap-2 bg-slate-950 p-1.5 rounded-xl border border-slate-800 mb-6">
        <Button
          type="button"
          size="sm"
          variant={mode === 'meeting' ? 'default' : 'ghost'}
          onClick={() => setMode('meeting')}
          className="gap-2"
        >
          <Kanban className="w-4 h-4" />
          Meeting → Kanban
        </Button>

        <Button
          type="button"
          size="sm"
          variant={mode === 'scribble' ? 'purple' : 'ghost'}
          onClick={() => setMode('scribble')}
          className="gap-2"
        >
          <Sparkles className="w-4 h-4" />
          Voice Scribble
        </Button>
      </div>

      {/* Mic Button & Waveform Container */}
      <div className="relative flex flex-col items-center my-4">
        <button
          onClick={toggleRecording}
          disabled={isProcessing}
          className={`w-28 h-28 rounded-full flex items-center justify-center transition-all duration-300 transform active:scale-95 ${
            isRecording
              ? 'bg-red-500 text-white recording-pulse scale-105'
              : isProcessing
              ? 'bg-amber-500/20 text-amber-400 border border-amber-500/40'
              : 'bg-gradient-to-tr from-blue-600 to-indigo-600 text-white hover:from-blue-500 hover:to-indigo-500 pulse-glow'
          }`}
        >
          {isProcessing ? (
            <Loader2 className="w-12 h-12 animate-spin" />
          ) : isRecording ? (
            <MicOff className="w-12 h-12" />
          ) : (
            <Mic className="w-12 h-12" />
          )}
        </button>

        <p className="mt-4 text-sm font-medium text-slate-300 animate-pulse">
          {statusText}
        </p>
      </div>

      <div className="flex items-center gap-2 text-xs text-slate-500 mt-2">
        <Zap className="w-3.5 h-3.5 text-amber-400" />
        <span>Push-to-talk hotkey active (Ctrl+Space)</span>
      </div>
    </Card>
  );
};
