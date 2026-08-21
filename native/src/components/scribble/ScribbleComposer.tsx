import React, { useState, useRef } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { Scribble } from '../../types';
import {
  Send,
  Upload,
  FileText,
  Sparkles,
  Paperclip,
  X,
  Plus,
  Loader2,
  Tag,
  Hash,
} from 'lucide-react';
import { Button } from '@/components/ui/button';
import { Badge } from '@/components/ui/badge';

interface ScribbleComposerProps {
  onScribbleCreated: (scribble: Scribble) => void;
}

export const ScribbleComposer: React.FC<ScribbleComposerProps> = ({
  onScribbleCreated,
}) => {
  const [content, setContent] = useState('');
  const [title, setTitle] = useState('');
  const [topicInput, setTopicInput] = useState('');
  const [topics, setTopics] = useState<string[]>([]);
  const [isExpanded, setIsExpanded] = useState(false);
  const [busy, setBusy] = useState(false);
  const fileInputRef = useRef<HTMLInputElement | null>(null);

  const handleAddTopic = () => {
    const trimmed = topicInput.trim();
    if (trimmed && !topics.includes(trimmed)) {
      setTopics([...topics, trimmed]);
      setTopicInput('');
    }
  };

  const handleRemoveTopic = (topic: string) => {
    setTopics(topics.filter((t) => t !== topic));
  };

  const handleCreate = async () => {
    if (!content.trim()) return;
    setBusy(true);
    try {
      const scribble = await invoke<Scribble>('create_scribble', {
        content: content.trim(),
        title: title.trim() || undefined,
        sourceType: 'text',
        topics: topics.length > 0 ? topics : undefined,
      });

      onScribbleCreated(scribble);
      setContent('');
      setTitle('');
      setTopics([]);
      setTopicInput('');
      setIsExpanded(false);
    } catch (err) {
      console.error('Failed to create Scribble:', err);
    } finally {
      setBusy(false);
    }
  };

  const handleFileUpload = async (e: React.ChangeEvent<HTMLInputElement>) => {
    const files = e.target.files;
    if (!files || files.length === 0) return;
    const file = files[0];

    try {
      const text = await file.text();
      const scribble = await invoke<Scribble>('create_file_scribble', {
        filename: file.name,
        content: text,
        mimeType: file.type || 'text/plain',
        sizeBytes: file.size,
      });

      onScribbleCreated(scribble);
    } catch (err) {
      console.error('Failed to upload file as scribble:', err);
    } finally {
      if (fileInputRef.current) fileInputRef.current.value = '';
    }
  };

  return (
    <div className="rounded-lg border border-border bg-card p-3 shadow-xs transition-all space-y-3">
      {/* Expanded Title Row */}
      {isExpanded && (
        <input
          type="text"
          value={title}
          onChange={(e) => setTitle(e.target.value)}
          placeholder="Scribble Title (Optional)"
          className="w-full text-sm font-semibold bg-transparent border-b border-border/60 pb-2 text-foreground focus:outline-none placeholder:text-muted-foreground/60"
        />
      )}

      {/* Main Content Input */}
      <textarea
        value={content}
        onChange={(e) => setContent(e.target.value)}
        onFocus={() => setIsExpanded(true)}
        onKeyDown={(e) => {
          if (e.key === 'Enter' && (e.ctrlKey || e.metaKey)) {
            handleCreate();
          }
        }}
        placeholder="Capture a thought, idea, question, or note… (Ctrl+Enter to save)"
        className="w-full min-h-[56px] max-h-48 text-xs bg-transparent text-foreground focus:outline-none placeholder:text-muted-foreground/70 resize-none font-sans leading-relaxed"
        rows={isExpanded ? 3 : 1}
      />

      {/* Expanded Metadata (Topics) */}
      {isExpanded && (
        <div className="space-y-2 pt-1 border-t border-border/40">
          <div className="flex flex-wrap items-center gap-1.5">
            {topics.map((topic) => (
              <Badge
                key={topic}
                variant="secondary"
                className="text-[10px] gap-1 px-2 py-0.5 bg-accent/60 text-accent-foreground font-mono"
              >
                <span>#{topic}</span>
                <button
                  type="button"
                  onClick={() => handleRemoveTopic(topic)}
                  className="hover:text-destructive"
                >
                  <X className="w-2.5 h-2.5" />
                </button>
              </Badge>
            ))}
            <div className="flex items-center gap-1">
              <input
                type="text"
                value={topicInput}
                onChange={(e) => setTopicInput(e.target.value)}
                onKeyDown={(e) => {
                  if (e.key === 'Enter') {
                    e.preventDefault();
                    handleAddTopic();
                  }
                }}
                placeholder="Add topic…"
                className="text-[11px] bg-muted/40 px-2 py-0.5 rounded border border-border/60 text-foreground w-24 focus:outline-none focus:w-36 transition-all"
              />
              <button
                type="button"
                onClick={handleAddTopic}
                className="text-muted-foreground hover:text-foreground text-xs"
              >
                <Plus className="w-3.5 h-3.5" />
              </button>
            </div>
          </div>
        </div>
      )}

      {/* Action Footer */}
      <div className="flex items-center justify-between pt-1 border-t border-border/40">
        <div className="flex items-center gap-1.5">
          <input
            type="file"
            ref={fileInputRef}
            onChange={handleFileUpload}
            className="hidden"
            accept=".txt,.md,.json,.csv"
          />
          <Button
            type="button"
            variant="ghost"
            size="sm"
            onClick={() => fileInputRef.current?.click()}
            className="h-7 px-2 text-[11px] gap-1 text-muted-foreground hover:text-foreground"
            title="Upload text or markdown file as Scribble"
          >
            <Upload className="w-3.5 h-3.5" />
            <span>Import File</span>
          </Button>

          <span className="text-[10px] text-muted-foreground hidden sm:inline flex items-center gap-1">
            <Sparkles className="w-3 h-3 text-primary/70" />
            <span>Async AI enrichment automatic</span>
          </span>
        </div>

        <div className="flex items-center gap-1.5">
          {isExpanded && (
            <Button
              type="button"
              variant="ghost"
              size="sm"
              onClick={() => {
                setIsExpanded(false);
                setContent('');
                setTitle('');
                setTopics([]);
              }}
              className="h-7 text-xs"
            >
              Cancel
            </Button>
          )}
          <Button
            type="button"
            size="sm"
            onClick={handleCreate}
            disabled={busy || !content.trim()}
            className="h-7 text-xs gap-1.5 font-semibold"
          >
            {busy ? <Loader2 className="w-3 h-3 animate-spin" /> : <Send className="w-3 h-3" />}
            <span>Capture Scribble</span>
          </Button>
        </div>
      </div>
    </div>
  );
};
