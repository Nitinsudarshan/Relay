import React, { useState, useRef } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { Scribble } from '../../types';
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
} from 'lucide-react';
import { Button } from '@/components/ui/button';
import { Badge } from '@/components/ui/badge';

interface CaptureHubPageProps {
  onCaptureSuccess: (scribble: Scribble) => void;
  onNavigateToScribbles: () => void;
}

export const CaptureHubPage: React.FC<CaptureHubPageProps> = ({
  onCaptureSuccess,
  onNavigateToScribbles,
}) => {
  const [selectedMethod, setSelectedMethod] = useState<'text' | 'file' | 'clipboard'>('text');
  const [textContent, setTextContent] = useState('');
  const [textTitle, setTextTitle] = useState('');
  const [topicInput, setTopicInput] = useState('');
  const [topics, setTopics] = useState<string[]>([]);
  const [busy, setBusy] = useState(false);
  const [lastCaptured, setLastCaptured] = useState<Scribble | null>(null);

  const fileInputRef = useRef<HTMLInputElement | null>(null);

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
      onCaptureSuccess(scribble);
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
      onCaptureSuccess(scribble);
    } catch (err) {
      console.error('Failed to read from clipboard:', err);
    } finally {
      setBusy(false);
    }
  };

  const handleFileUpload = async (e: React.ChangeEvent<HTMLInputElement>) => {
    const files = e.target.files;
    if (!files || files.length === 0) return;
    const file = files[0];

    setBusy(true);
    try {
      const text = await file.text();
      const scribble = await invoke<Scribble>('create_file_scribble', {
        filename: file.name,
        content: text,
        mimeType: file.type || 'text/plain',
        sizeBytes: file.size,
      });

      setLastCaptured(scribble);
      onCaptureSuccess(scribble);
    } catch (err) {
      console.error('Failed to upload file as scribble:', err);
    } finally {
      setBusy(false);
      if (fileInputRef.current) fileInputRef.current.value = '';
    }
  };

  return (
    <div className="flex-1 flex flex-col gap-4 overflow-y-auto w-full pb-10">
      {/* Capture Method Selector Grid */}
      <div className="grid grid-cols-2 sm:grid-cols-3 md:grid-cols-6 gap-2.5">
        {/* 1. Voice (Active) */}
        <div className="p-3.5 rounded-lg border border-primary/40 bg-primary/5 flex flex-col justify-between space-y-2 transition-all">
          <div className="flex items-center justify-between">
            <Mic className="w-4 h-4 text-primary" />
            <Badge variant="outline" className="text-[8px] font-mono text-primary border-primary/30">
              Active
            </Badge>
          </div>
          <div>
            <span className="text-xs font-bold text-foreground block">Voice</span>
            <span className="text-[10px] text-muted-foreground block">Global hotkey PTT</span>
          </div>
          <div className="pt-1">
            <kbd className="font-mono text-[9px] bg-background/80 px-1.5 py-0.5 rounded-lg border border-border flex items-center gap-1 w-fit">
              <Command className="w-2.5 h-2.5" /> Ctrl + Space
            </kbd>
          </div>
        </div>

        {/* 2. Text (Active) */}
        <button
          type="button"
          onClick={() => setSelectedMethod('text')}
          className={`p-3.5 rounded-lg border text-left flex flex-col justify-between space-y-2 transition-all ${
            selectedMethod === 'text'
              ? 'border-primary bg-accent/60 shadow-xs'
              : 'border-border bg-card hover:bg-muted/40'
          }`}
        >
          <div className="flex items-center justify-between">
            <FileText className="w-4 h-4 text-amber-500" />
            <Badge variant="outline" className="text-[8px] font-mono text-emerald-500 border-emerald-500/30">
              Active
            </Badge>
          </div>
          <div>
            <span className="text-xs font-bold text-foreground block">Typed Text</span>
            <span className="text-[10px] text-muted-foreground block">Type raw thought</span>
          </div>
          <span className="text-[10px] text-primary font-medium">Quick Compose →</span>
        </button>

        {/* 3. Clipboard (Active - Requirement 9) */}
        <button
          type="button"
          onClick={() => setSelectedMethod('clipboard')}
          className={`p-3.5 rounded-lg border text-left flex flex-col justify-between space-y-2 transition-all ${
            selectedMethod === 'clipboard'
              ? 'border-primary bg-accent/60 shadow-xs'
              : 'border-border bg-card hover:bg-muted/40'
          }`}
        >
          <div className="flex items-center justify-between">
            <Clipboard className="w-4 h-4 text-emerald-500" />
            <Badge variant="outline" className="text-[8px] font-mono text-emerald-500 border-emerald-500/30">
              Active
            </Badge>
          </div>
          <div>
            <span className="text-xs font-bold text-foreground block">Clipboard</span>
            <span className="text-[10px] text-muted-foreground block">1-Click paste buffer</span>
          </div>
          <span className="text-[10px] text-primary font-medium">Paste Buffer →</span>
        </button>

        {/* 4. Files & Docs (Active) */}
        <button
          type="button"
          onClick={() => setSelectedMethod('file')}
          className={`p-3.5 rounded-lg border text-left flex flex-col justify-between space-y-2 transition-all ${
            selectedMethod === 'file'
              ? 'border-primary bg-accent/60 shadow-xs'
              : 'border-border bg-card hover:bg-muted/40'
          }`}
        >
          <div className="flex items-center justify-between">
            <Upload className="w-4 h-4 text-blue-500" />
            <Badge variant="outline" className="text-[8px] font-mono text-emerald-500 border-emerald-500/30">
              Active
            </Badge>
          </div>
          <div>
            <span className="text-xs font-bold text-foreground block">Files & Docs</span>
            <span className="text-[10px] text-muted-foreground block">TXT, MD, CSV, JSON</span>
          </div>
          <span className="text-[10px] text-primary font-medium">Import File →</span>
        </button>

        {/* 5. Browser (Future) */}
        <div className="p-3.5 rounded-lg border border-border/40 bg-card/40 opacity-70 flex flex-col justify-between space-y-2 select-none">
          <div className="flex items-center justify-between">
            <Globe className="w-4 h-4 text-muted-foreground" />
            <Badge variant="secondary" className="text-[8px] font-mono text-muted-foreground">
              Future
            </Badge>
          </div>
          <div>
            <span className="text-xs font-bold text-muted-foreground block">Browser Extension</span>
            <span className="text-[10px] text-muted-foreground block">Selection & Page clip</span>
          </div>
          <span className="text-[9px] text-muted-foreground italic">In development</span>
        </div>

        {/* 6. Meetings (Future Modality) */}
        <div className="p-3.5 rounded-lg border border-border/40 bg-card/40 opacity-70 flex flex-col justify-between space-y-2 select-none">
          <div className="flex items-center justify-between">
            <Users className="w-4 h-4 text-muted-foreground" />
            <Badge variant="secondary" className="text-[8px] font-mono text-muted-foreground">
              Future
            </Badge>
          </div>
          <div>
            <span className="text-xs font-bold text-muted-foreground block">Meeting Notes</span>
            <span className="text-[10px] text-muted-foreground block">Capture modality</span>
          </div>
          <span className="text-[9px] text-muted-foreground italic">Capture source</span>
        </div>
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

        {selectedMethod === 'file' && (
          <div className="space-y-4">
            <div className="flex items-center justify-between">
              <h3 className="text-sm font-bold text-foreground flex items-center gap-2">
                <Upload className="w-4 h-4 text-blue-500" />
                <span>Import File as Scribble</span>
              </h3>
            </div>

            {/* Supported Files Specification (Requirement 11) */}
            <div className="p-3.5 bg-muted/30 border border-border/60 rounded-lg space-y-1">
              <span className="text-[11px] font-semibold text-foreground block">
                Supported File Formats:
              </span>
              <p className="text-[11px] text-muted-foreground font-mono">
                TXT · MD · JSON · CSV · PDF · DOCX · PNG · JPG · JPEG
              </p>
            </div>

            {/* Dropzone */}
            <div
              onClick={() => fileInputRef.current?.click()}
              className="border-2 border-dashed border-border hover:border-primary/60 rounded-lg p-10 text-center cursor-pointer transition-all bg-muted/10 hover:bg-muted/30 space-y-3"
            >
              <input
                type="file"
                ref={fileInputRef}
                onChange={handleFileUpload}
                className="hidden"
                accept=".txt,.md,.json,.csv,.pdf,.docx,.png,.jpg,.jpeg"
              />
              <Upload className="w-10 h-10 mx-auto text-muted-foreground opacity-50" />
              <div>
                <p className="text-xs font-bold text-foreground">Click to upload or drag and drop</p>
                <p className="text-[11px] text-muted-foreground mt-0.5">
                  Content is converted to an addressable Scribble in your local vault.
                </p>
              </div>
            </div>
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
            onClick={onNavigateToScribbles}
            className="h-8 text-xs gap-1.5 shrink-0 bg-background"
          >
            <span>Open in Workspace</span>
            <ArrowRight className="w-3.5 h-3.5" />
          </Button>
        </div>
      )}
    </div>
  );
};
