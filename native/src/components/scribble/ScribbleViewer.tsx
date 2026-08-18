import React, { useState } from 'react';
import { Copy, Check, FileText, Sparkles } from 'lucide-react';
import { Card, CardHeader, CardTitle, CardContent } from '@/components/ui/card';
import { Button } from '@/components/ui/button';

interface ScribbleViewerProps {
  content: string;
  transcript: string;
}

export const ScribbleViewer: React.FC<ScribbleViewerProps> = ({ content, transcript }) => {
  const [copied, setCopied] = useState(false);

  const handleCopy = () => {
    navigator.clipboard.writeText(content);
    setCopied(true);
    setTimeout(() => setCopied(false), 2000);
  };

  return (
    <Card className="flex flex-col h-full border-slate-800">
      <CardHeader className="flex-row items-center justify-between pb-3 space-y-0">
        <div className="flex items-center gap-2">
          <Sparkles className="w-5 h-5 text-purple-400" />
          <CardTitle>Structured Voice Scribble</CardTitle>
        </div>
        <Button size="sm" variant="outline" onClick={handleCopy} className="gap-1.5">
          {copied ? <Check className="w-3.5 h-3.5 text-emerald-400" /> : <Copy className="w-3.5 h-3.5" />}
          <span>{copied ? 'Copied!' : 'Copy Markdown'}</span>
        </Button>
      </CardHeader>

      <CardContent className="flex-1 overflow-y-auto space-y-4 pr-3">
        <div className="bg-slate-950 rounded-lg p-4 font-mono text-xs text-slate-200 border border-slate-800/80 whitespace-pre-wrap leading-relaxed">
          {content || 'No structured scribble generated yet.'}
        </div>

        {transcript && (
          <div className="bg-slate-950/60 rounded-lg p-3 border border-slate-800/80">
            <div className="flex items-center gap-1.5 text-xs font-semibold text-slate-400 mb-1">
              <FileText className="w-3.5 h-3.5" />
              <span>Original Audio Transcript</span>
            </div>
            <p className="text-xs text-slate-300 italic">{transcript}</p>
          </div>
        )}
      </CardContent>
    </Card>
  );
};
