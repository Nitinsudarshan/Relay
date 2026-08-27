import React, { useState, useEffect } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { Wand2, Sparkles, Copy, Check, X, AlertCircle, Loader2, Play } from 'lucide-react';
import { Button } from '@/components/ui/button';
import { Badge } from '@/components/ui/badge';
import { AppSettings, PromptItem, Scribble } from '../../types';

interface PromptTransformModalProps {
  isOpen: boolean;
  onClose: () => void;
  inputText: string;
  sourceTitle?: string;
  sourceType: 'voice_note' | 'scribble';
  onScribbleCreated?: (scribble: Scribble) => void;
}

export const PromptTransformModal: React.FC<PromptTransformModalProps> = ({
  isOpen,
  onClose,
  inputText,
  sourceTitle = 'Untitled',
  sourceType,
  onScribbleCreated,
}) => {
  const [prompts, setPrompts] = useState<PromptItem[]>([]);
  const [selectedPromptId, setSelectedPromptId] = useState<string>('');
  const [isLoading, setIsLoading] = useState<boolean>(false);
  const [resultText, setResultText] = useState<string | null>(null);
  const [errorMessage, setErrorMessage] = useState<string | null>(null);
  const [copied, setCopied] = useState<boolean>(false);
  const [savedScribble, setSavedScribble] = useState<boolean>(false);

  useEffect(() => {
    if (!isOpen) {
      setResultText(null);
      setErrorMessage(null);
      setCopied(false);
      setSavedScribble(false);
      return;
    }

    const loadPrompts = async () => {
      try {
        const settings = await invoke<AppSettings>('get_settings');
        const available = (settings?.prompts || []).filter((p) => p.enabled);
        setPrompts(available);
        if (available.length > 0) {
          setSelectedPromptId(available[0].id);
        }
      } catch (err) {
        console.error('Failed to load prompts for transformation modal:', err);
      }
    };

    loadPrompts();
  }, [isOpen]);

  if (!isOpen) return null;

  const selectedPrompt = prompts.find((p) => p.id === selectedPromptId);

  const handleRunPrompt = async () => {
    if (!selectedPromptId || !inputText.trim()) return;

    setIsLoading(true);
    setErrorMessage(null);
    setResultText(null);
    setCopied(false);
    setSavedScribble(false);

    try {
      const output = await invoke<string>('execute_prompt', {
        promptId: selectedPromptId,
        inputText,
      });
      setResultText(output);
    } catch (err: any) {
      console.error('Failed to execute prompt:', err);
      setErrorMessage(err?.message || String(err) || 'Failed to transform with AI prompt');
    } finally {
      setIsLoading(false);
    }
  };

  const handleCopy = async () => {
    if (!resultText) return;
    try {
      await navigator.clipboard.writeText(resultText);
      setCopied(true);
      setTimeout(() => setCopied(false), 2000);
    } catch (err) {
      console.error('Failed to copy prompt result:', err);
    }
  };

  const handleSaveAsScribble = async () => {
    if (!resultText) return;
    try {
      const scribbleTitle = `${selectedPrompt?.name || 'Prompt'}: ${sourceTitle}`;
      const newScribble = await invoke<Scribble>('create_scribble', {
        title: scribbleTitle,
        content: resultText,
        sourceType: 'ai_transform',
        tags: ['prompt-result', sourceType],
      });
      setSavedScribble(true);
      if (onScribbleCreated) {
        onScribbleCreated(newScribble);
      }
    } catch (err: any) {
      console.error('Failed to create scribble from prompt result:', err);
      setErrorMessage(err?.message || 'Failed to save as Scribble');
    }
  };

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/60 backdrop-blur-xs p-4 animate-in fade-in duration-150">
      <div
        className="w-full max-w-2xl bg-card border border-border rounded-xl shadow-2xl overflow-hidden flex flex-col max-h-[85vh] animate-in zoom-in-95 duration-150"
        onClick={(e) => e.stopPropagation()}
      >
        {/* Header */}
        <div className="p-4 px-5 border-b border-border flex items-center justify-between bg-muted/30 shrink-0">
          <div className="flex items-center gap-2">
            <div className="p-1.5 rounded-lg bg-sky-500/10 text-sky-500">
              <Wand2 className="w-4 h-4" />
            </div>
            <div>
              <h3 className="text-sm font-bold text-foreground">Transform with Prompt</h3>
              <p className="text-[11px] text-muted-foreground">
                Apply an AI prompt template to {sourceType === 'voice_note' ? 'this Voice Note' : 'this Scribble'}
              </p>
            </div>
          </div>
          <Button
            size="icon"
            variant="ghost"
            onClick={onClose}
            className="h-8 w-8 text-muted-foreground hover:text-foreground"
            aria-label="Close"
          >
            <X className="w-4 h-4" />
          </Button>
        </div>

        {/* Content Body */}
        <div className="p-5 overflow-y-auto space-y-4 flex-1">
          {/* Source Text Excerpt */}
          <div className="space-y-1.5">
            <div className="flex items-center justify-between text-[11px] text-muted-foreground">
              <span className="font-semibold uppercase tracking-wider text-[10px]">Source Text</span>
              <span className="font-mono">{inputText.split(/\s+/).filter(Boolean).length} words</span>
            </div>
            <div className="p-3 rounded-lg bg-muted/40 border border-border text-xs text-foreground/90 max-h-28 overflow-y-auto leading-relaxed whitespace-pre-wrap font-sans">
              {inputText}
            </div>
          </div>

          {/* Prompt Template Selector */}
          <div className="space-y-2">
            <label className="text-[11px] font-semibold uppercase tracking-wider text-muted-foreground block">
              Select Prompt Template
            </label>
            {prompts.length === 0 ? (
              <p className="text-xs text-muted-foreground italic">No prompts available in library.</p>
            ) : (
              <div className="grid grid-cols-1 sm:grid-cols-2 gap-2">
                {prompts.map((p) => (
                  <button
                    key={p.id}
                    type="button"
                    onClick={() => setSelectedPromptId(p.id)}
                    className={`p-2.5 rounded-lg border text-left transition-all ${
                      selectedPromptId === p.id
                        ? 'border-sky-500/80 bg-sky-500/10 text-foreground shadow-2xs'
                        : 'border-border bg-card/60 hover:bg-muted/50 text-muted-foreground'
                    }`}
                  >
                    <div className="flex items-center justify-between gap-1 mb-0.5">
                      <span className="text-xs font-semibold text-foreground truncate">{p.name}</span>
                      {selectedPromptId === p.id && (
                        <Badge variant="outline" className="text-[9px] font-mono px-1 py-0 border-sky-500/30 text-sky-500">
                          Selected
                        </Badge>
                      )}
                    </div>
                    {p.description && (
                      <p className="text-[10px] text-muted-foreground line-clamp-1">{p.description}</p>
                    )}
                  </button>
                ))}
              </div>
            )}
          </div>

          {/* Run Button */}
          {!resultText && (
            <div className="pt-2 flex justify-end">
              <Button
                size="sm"
                onClick={handleRunPrompt}
                disabled={isLoading || !selectedPromptId}
                className="gap-1.5 bg-sky-600 hover:bg-sky-500 text-white shadow-xs text-xs"
              >
                {isLoading ? (
                  <>
                    <Loader2 className="w-3.5 h-3.5 animate-spin" />
                    <span>Transforming with AI…</span>
                  </>
                ) : (
                  <>
                    <Play className="w-3.5 h-3.5 fill-current" />
                    <span>Run {selectedPrompt?.name || 'Prompt'}</span>
                  </>
                )}
              </Button>
            </div>
          )}

          {/* Error Notice */}
          {errorMessage && (
            <div className="p-3 rounded-lg bg-destructive/10 border border-destructive/30 text-destructive text-xs flex items-center gap-2">
              <AlertCircle className="w-4 h-4 shrink-0" />
              <span>{errorMessage}</span>
            </div>
          )}

          {/* Generated Result Card */}
          {resultText && (
            <div className="space-y-2 pt-2 border-t border-border animate-in fade-in duration-200">
              <div className="flex items-center justify-between">
                <div className="flex items-center gap-1.5">
                  <Sparkles className="w-3.5 h-3.5 text-sky-500" />
                  <span className="text-xs font-bold text-foreground">
                    Transformation Result ({selectedPrompt?.name})
                  </span>
                </div>
                <div className="flex items-center gap-1">
                  <Button
                    size="sm"
                    variant="outline"
                    onClick={handleCopy}
                    className="h-7 text-xs gap-1 px-2.5"
                  >
                    {copied ? (
                      <>
                        <Check className="w-3 h-3 text-emerald-500" />
                        <span className="text-emerald-500">Copied</span>
                      </>
                    ) : (
                      <>
                        <Copy className="w-3 h-3" />
                        <span>Copy</span>
                      </>
                    )}
                  </Button>

                  <Button
                    size="sm"
                    variant="outline"
                    onClick={handleSaveAsScribble}
                    disabled={savedScribble}
                    className={`h-7 text-xs gap-1 px-2.5 ${
                      savedScribble
                        ? 'border-emerald-500/40 text-emerald-500 bg-emerald-500/10'
                        : 'text-amber-500 border-amber-500/30 hover:bg-amber-500/10'
                    }`}
                    title="Save result directly to Obsidian-compatible Scribble knowledge graph"
                  >
                    {savedScribble ? (
                      <>
                        <Check className="w-3 h-3 text-emerald-500" />
                        <span>Saved to Scribbles</span>
                      </>
                    ) : (
                      <>
                        <Sparkles className="w-3 h-3" />
                        <span>Save as Scribble</span>
                      </>
                    )}
                  </Button>
                </div>
              </div>

              <div className="p-3.5 rounded-lg bg-muted/30 border border-border text-xs text-foreground leading-relaxed whitespace-pre-wrap font-sans max-h-56 overflow-y-auto">
                {resultText}
              </div>
            </div>
          )}
        </div>

        {/* Footer */}
        <div className="p-3 px-5 border-t border-border bg-muted/20 flex items-center justify-between shrink-0">
          <span className="text-[10px] text-muted-foreground">
            Original {sourceType === 'voice_note' ? 'Voice Note' : 'Scribble'} remains intact
          </span>
          <Button size="sm" variant="ghost" onClick={onClose} className="h-8 text-xs">
            Done
          </Button>
        </div>
      </div>
    </div>
  );
};
