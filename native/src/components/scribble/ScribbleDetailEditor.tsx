import React, { useState, useEffect, useMemo } from 'react';
import { invoke } from '@tauri-apps/api/core';
import {
  Scribble,
} from '../../types';
import {
  Save,
  Trash2,
  Sparkles,
  RefreshCw,
  Edit3,
  Copy,
  Check,
  X,
  Plus,
  FileText,
  Mic,
  Hash,
  Box,
  Link as LinkIcon,
  HelpCircle,
  ArrowUpRight,
  ShieldCheck,
  GitMerge,
  ChevronDown,
  ChevronUp,
  ChevronRight,
  Upload,
  Clipboard,
  Globe,
  Users,
} from 'lucide-react';
import { Button } from '@/components/ui/button';
import { Badge } from '@/components/ui/badge';
import { ConnectAndMergeModal } from './ConnectAndMergeModal';
import { ConfirmationModal } from '../common/ConfirmationModal';
import { MarkdownView } from '../common/MarkdownView';

interface ScribbleDetailEditorProps {
  scribble: Scribble;
  allScribbles: Scribble[];
  onUpdate: (updated: Scribble) => void;
  onDelete: (id: string) => void;
  onSelectScribble: (id: string) => void;
  onScribbleCreated?: (created: Scribble) => void;
}

export const ScribbleDetailEditor: React.FC<ScribbleDetailEditorProps> = ({
  scribble,
  allScribbles,
  onUpdate,
  onDelete,
  onSelectScribble,
  onScribbleCreated,
}) => {
  const [isEditing, setIsEditing] = useState(false);
  const [title, setTitle] = useState(scribble.title);
  const [content, setContent] = useState(scribble.content);
  const [summary, setSummary] = useState(scribble.summary || '');
  const [topics, setTopics] = useState<string[]>(scribble.topics || []);
  const [entities, setEntities] = useState<string[]>(scribble.entities || []);
  const [questions, setQuestions] = useState<string[]>(scribble.ai_metadata?.suggested_questions || []);
  const [topicInput, setTopicInput] = useState('');
  const [entityInput, setEntityInput] = useState('');

  // Modals and confirmation states
  const [modalMode, setModalMode] = useState<'connect' | 'merge' | null>(null);
  const [showTechnicalProvenance, setShowTechnicalProvenance] = useState(false);
  const [confirmDelete, setConfirmDelete] = useState(false);
  const [isContentExpanded, setIsContentExpanded] = useState(false);

  // Copy feedback states
  const [copiedContent, setCopiedContent] = useState(false);
  const [copiedSummary, setCopiedSummary] = useState(false);
  const [copiedQuestionIndex, setCopiedQuestionIndex] = useState<number | null>(null);
  const [isEnriching, setIsEnriching] = useState(false);
  const [isSummarizing, setIsSummarizing] = useState(false);

  // Sync state when scribble changes or updates
  useEffect(() => {
    if (!isEditing) {
      setTitle(scribble.title);
      setContent(scribble.content);
      setSummary(scribble.summary || '');
      setTopics(scribble.topics || []);
      setEntities(scribble.entities || []);
      setQuestions(scribble.ai_metadata?.suggested_questions || []);
    }
  }, [scribble, isEditing]);

  useEffect(() => {
    setIsEditing(false);
    setConfirmDelete(false);
    setIsContentExpanded(false);
  }, [scribble.id]);

  // Word count & thresholds (100+ for summary, >200 for read more toggle)
  const wordCount = useMemo(() => {
    return scribble.content.trim().split(/\s+/).filter(Boolean).length;
  }, [scribble.content]);

  const isLongScribble = wordCount >= 100;
  const isVeryLongScribble = wordCount > 200;

  // Guaranteed effective exploration questions with dynamic fallback
  const effectiveQuestions = useMemo(() => {
    if (questions && questions.length > 0) return questions;
    if (scribble.ai_metadata?.suggested_questions && scribble.ai_metadata.suggested_questions.length > 0) {
      return scribble.ai_metadata.suggested_questions;
    }
    const mainTopic = (scribble.topics && scribble.topics.length > 0) ? scribble.topics[0] : (scribble.title || 'Knowledge Organization');
    return [
      `How does '${mainTopic}' connect with your broader project architecture and roadmap?`,
      `What are the critical implementation risks, performance trade-offs, or UX edge cases for '${scribble.title}'?`,
      `What actionable next step or prototype would best advance this thinking forward?`,
    ];
  }, [questions, scribble.ai_metadata?.suggested_questions, scribble.topics, scribble.title]);

  // Only display valid relationships pointing to currently active, non-trashed scribbles
  const activeRelationships = useMemo(() => {
    return (scribble.relationships || []).filter((rel) =>
      allScribbles.some((s) => s.id === rel.target_id)
    );
  }, [scribble.relationships, allScribbles]);

  const handleCopyContent = () => {
    navigator.clipboard.writeText(scribble.content);
    setCopiedContent(true);
    setTimeout(() => setCopiedContent(false), 2000);
  };

  const handleCopySummary = () => {
    if (scribble.summary) {
      navigator.clipboard.writeText(scribble.summary);
      setCopiedSummary(true);
      setTimeout(() => setCopiedSummary(false), 2000);
    }
  };

  const handleCopyIndividualQuestion = (questionText: string, index: number) => {
    navigator.clipboard.writeText(questionText);
    setCopiedQuestionIndex(index);
    setTimeout(() => setCopiedQuestionIndex(null), 2000);
  };

  const handleSummarize = async () => {
    setIsSummarizing(true);
    try {
      const res = await invoke<Scribble>('summarize_scribble', { id: scribble.id });
      onUpdate(res);
      setSummary(res.summary || '');
    } catch (err) {
      console.error('Failed to summarize scribble:', err);
    } finally {
      setIsSummarizing(false);
    }
  };

  const handleSaveEdit = async () => {
    const updated: Scribble = {
      ...scribble,
      title: title.trim() || 'Untitled Thought',
      content: content.trim(),
      summary: summary.trim() || undefined,
      topics,
      entities,
      updated_at: new Date().toISOString(),
    };

    try {
      const res = await invoke<Scribble>('update_scribble', { scribble: updated });
      onUpdate(res);
      setIsEditing(false);
    } catch (err) {
      console.error('Failed to update scribble:', err);
    }
  };

  const handleAddTopic = () => {
    const trimmed = topicInput.trim();
    if (trimmed && !topics.includes(trimmed)) {
      const nextTopics = [...topics, trimmed];
      setTopics(nextTopics);
      setTopicInput('');
      if (!isEditing) {
        invoke<Scribble>('update_scribble', {
          scribble: { ...scribble, topics: nextTopics },
        }).then(onUpdate);
      }
    }
  };

  const handleRemoveTopic = (topic: string) => {
    const nextTopics = topics.filter((t) => t !== topic);
    setTopics(nextTopics);
    if (!isEditing) {
      invoke<Scribble>('update_scribble', {
        scribble: { ...scribble, topics: nextTopics },
      }).then(onUpdate);
    }
  };

  const handleAddEntity = () => {
    const trimmed = entityInput.trim();
    if (trimmed && !entities.includes(trimmed)) {
      const nextEntities = [...entities, trimmed];
      setEntities(nextEntities);
      setEntityInput('');
      if (!isEditing) {
        invoke<Scribble>('update_scribble', {
          scribble: { ...scribble, entities: nextEntities },
        }).then(onUpdate);
      }
    }
  };

  const handleRemoveEntity = (entity: string) => {
    const nextEntities = entities.filter((e) => e !== entity);
    setEntities(nextEntities);
    if (!isEditing) {
      invoke<Scribble>('update_scribble', {
        scribble: { ...scribble, entities: nextEntities },
      }).then(onUpdate);
    }
  };

  const handleRemoveRelationship = async (relId: string) => {
    try {
      const res = await invoke<Scribble>('remove_scribble_relationship', {
        sourceId: scribble.id,
        relationshipId: relId,
      });
      onUpdate(res);
    } catch (err) {
      console.error('Failed to remove relationship:', err);
    }
  };

  const handleReEnrich = async () => {
    setIsEnriching(true);
    try {
      const enriched = await invoke<Scribble>('trigger_enrich_scribble', { id: scribble.id });
      if (enriched) {
        onUpdate(enriched);
        setTitle(enriched.title);
        setContent(enriched.content);
        setSummary(enriched.summary || '');
        setTopics(enriched.topics || []);
        setEntities(enriched.entities || []);
        setQuestions(enriched.ai_metadata?.suggested_questions || []);
      }
    } catch (err) {
      console.error('Failed to trigger enrichment:', err);
    } finally {
      setIsEnriching(false);
    }
  };

  const handleDeleteConfirm = () => {
    onDelete(scribble.id);
    setConfirmDelete(false);
  };

  // Clean minimal source indicator (Requirement 2)
  const getSourceDisplay = (sourceType: string) => {
    switch (sourceType.toLowerCase()) {
      case 'voice':
        return { label: 'VOICE', icon: Mic, color: 'text-primary' };
      case 'text':
        return { label: 'TEXT', icon: FileText, color: 'text-amber-500' };
      case 'file':
        return { label: 'FILE', icon: Upload, color: 'text-blue-500' };
      case 'clipboard':
        return { label: 'CLIPBOARD', icon: Clipboard, color: 'text-emerald-500' };
      case 'browser_selection':
      case 'browser_page':
      case 'browser_conversation':
        return { label: 'BROWSER', icon: Globe, color: 'text-purple-500' };
      case 'meeting':
        return { label: 'MEETING', icon: Users, color: 'text-indigo-500' };
      default:
        return { label: sourceType.toUpperCase(), icon: FileText, color: 'text-muted-foreground' };
    }
  };

  const sourceMeta = getSourceDisplay(scribble.source_type);
  const SourceIcon = sourceMeta.icon;

  return (
    <div className="flex-1 flex flex-col bg-card rounded-lg border border-border overflow-hidden min-h-0 shadow-xs">
      {/* 1. Header Toolbar (Title, Minimal Source Badge, Date, Summarise & Edit Action) */}
      <div className="p-5 border-b border-border flex flex-wrap items-center justify-between gap-3 shrink-0 bg-card">
        <div className="flex-1 min-w-0">
          <div className="flex items-center gap-2 mb-1.5 flex-wrap">
            {/* Minimal Source Badge */}
            <Badge variant="outline" className="text-[9px] font-mono px-2 py-0.5 gap-1 bg-muted">
              <SourceIcon className={`w-3 h-3 ${sourceMeta.color}`} />
              <span>{sourceMeta.label}</span>
            </Badge>

            <span className="text-[10px] text-muted-foreground font-mono">
              {new Date(scribble.created_at).toLocaleString([], {
                month: 'short',
                day: 'numeric',
                hour: '2-digit',
                minute: '2-digit',
              })}
            </span>

            {scribble.ai_metadata?.enrichment_status === 'enriched' && (
              <Badge variant="outline" className="text-[9px] font-mono text-emerald-500 border-emerald-500/30 gap-1">
                <Sparkles className="w-2.5 h-2.5" />
                <span>AI Enriched</span>
              </Badge>
            )}
          </div>

          {isEditing ? (
            <input
              type="text"
              value={title}
              onChange={(e) => setTitle(e.target.value)}
              className="text-lg font-bold text-foreground bg-transparent border-b border-input focus:outline-none w-full"
            />
          ) : (
            <h2 className="text-lg font-extrabold text-foreground tracking-tight truncate">
              {scribble.title}
            </h2>
          )}
        </div>

        {/* Action Toolbar: Summarise & Edit */}
        <div className="flex items-center gap-1.5">
          {!isEditing && isLongScribble && (
            <Button
              size="sm"
              variant="outline"
              onClick={handleSummarize}
              disabled={isSummarizing}
              className="h-8 text-xs gap-1.5 text-primary border-primary/30 hover:bg-primary/10"
              title="Summarise this thought in 2-3 lines with AI"
            >
              <Sparkles className={`w-3.5 h-3.5 ${isSummarizing ? 'animate-spin' : ''}`} />
              <span>{isSummarizing ? 'Summarising…' : 'Summarise'}</span>
            </Button>
          )}

          {isEditing ? (
            <>
              <Button size="sm" variant="ghost" onClick={() => setIsEditing(false)} className="h-8 text-xs">
                Cancel
              </Button>
              <Button size="sm" variant="default" onClick={handleSaveEdit} className="h-8 text-xs gap-1">
                <Save className="w-3.5 h-3.5" />
                <span>Save</span>
              </Button>
            </>
          ) : (
            <Button
              size="sm"
              variant="outline"
              onClick={() => setIsEditing(true)}
              className="h-8 text-xs gap-1.5"
            >
              <Edit3 className="w-3.5 h-3.5" />
              <span>Edit</span>
            </Button>
          )}
        </div>
      </div>

      {/* Main Scrollable Body */}
      <div className="flex-1 overflow-y-auto p-6 space-y-6">
        {/* 2. AI Summary (Displayed BEFORE Scribble text when 100+ words and present) */}
        {scribble.summary && isLongScribble && !isEditing && (
          <div className="p-4 rounded-lg bg-accent/20 border border-accent/40 space-y-2 text-xs">
            <div className="flex items-center justify-between">
              <div className="flex items-center gap-1.5 font-semibold text-foreground">
                <Sparkles className="w-3.5 h-3.5 text-primary" />
                <span className="font-mono text-[10px] font-bold text-muted-foreground uppercase tracking-widest">
                  AI Summary
                </span>
              </div>

              <Button
                size="sm"
                variant="ghost"
                onClick={handleCopySummary}
                className="h-6 px-1.5 text-[10px] gap-1 shrink-0 text-muted-foreground hover:text-foreground"
                title="Copy AI summary"
              >
                {copiedSummary ? (
                  <>
                    <Check className="w-3 h-3 text-emerald-500" />
                    <span className="text-emerald-500">Copied</span>
                  </>
                ) : (
                  <>
                    <Copy className="w-3 h-3 opacity-60 hover:opacity-100" />
                    <span>Copy</span>
                  </>
                )}
              </Button>
            </div>

            <div className="p-3 rounded-lg bg-card/80 border border-border/60 text-xs text-foreground leading-relaxed">
              <MarkdownView content={scribble.summary} />
            </div>
          </div>
        )}

        {/* 3. Thought Content Section (with internal [Copy] button) */}
        <div className="space-y-2">
          <div className="flex items-center justify-between">
            <span className="font-mono text-[10px] font-bold text-muted-foreground uppercase tracking-widest">
              Thought Content
            </span>

            {!isEditing && (
              <Button
                size="sm"
                variant="ghost"
                onClick={handleCopyContent}
                className="h-6 px-2 text-[11px] gap-1 text-muted-foreground hover:text-foreground"
                title="Copy thought content"
              >
                {copiedContent ? (
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
            )}
          </div>

          {isEditing ? (
            <div className="space-y-3">
              <div className="space-y-1">
                <label className="text-[10px] font-mono font-bold text-muted-foreground uppercase">Summary</label>
                <input
                  type="text"
                  value={summary}
                  onChange={(e) => setSummary(e.target.value)}
                  placeholder="Concise 1-2 sentence distillation…"
                  className="w-full text-xs p-2.5 rounded-lg bg-muted/20 border border-border text-foreground focus:outline-none focus:ring-1 focus:ring-ring"
                />
              </div>

              <div className="space-y-1">
                <label className="text-[10px] font-mono font-bold text-muted-foreground uppercase">Content</label>
                <textarea
                  value={content}
                  onChange={(e) => setContent(e.target.value)}
                  className="w-full min-h-[160px] p-4 text-xs font-sans bg-muted/20 border border-border rounded-lg text-foreground focus:outline-none focus:ring-1 focus:ring-ring leading-relaxed resize-y"
                />
              </div>
            </div>
          ) : isVeryLongScribble ? (
            <div className="space-y-2">
              <div className="relative">
                <div
                  className={`p-4 rounded-lg bg-muted/20 border border-border font-sans text-xs text-foreground leading-relaxed transition-all duration-300 ${
                    !isContentExpanded ? 'max-h-52 overflow-hidden' : ''
                  }`}
                >
                  <MarkdownView content={scribble.content} />
                </div>

                {!isContentExpanded && (
                  <div className="absolute inset-x-0 bottom-0 h-24 bg-gradient-to-t from-card via-card/85 to-transparent flex items-end justify-center pb-2.5 rounded-b-lg">
                    <Button
                      size="sm"
                      variant="secondary"
                      onClick={() => setIsContentExpanded(true)}
                      className="h-7 text-xs px-3.5 gap-1.5 font-semibold shadow-xs bg-card border border-border text-foreground hover:bg-muted"
                    >
                      <span>Read More ({wordCount} words)</span>
                      <ChevronDown className="w-3.5 h-3.5 text-primary" />
                    </Button>
                  </div>
                )}
              </div>

              {isContentExpanded && (
                <div className="flex justify-center pt-0.5">
                  <Button
                    size="sm"
                    variant="ghost"
                    onClick={() => setIsContentExpanded(false)}
                    className="h-6 text-[11px] px-3 gap-1 text-muted-foreground hover:text-foreground"
                  >
                    <span>Show Less</span>
                    <ChevronUp className="w-3 h-3" />
                  </Button>
                </div>
              )}
            </div>
          ) : (
            <div className="p-4 rounded-lg bg-muted/20 border border-border font-sans text-xs text-foreground leading-relaxed">
              <MarkdownView content={scribble.content} />
            </div>
          )}
        </div>

        {/* 4. Topics & Entities */}
        <div className="grid grid-cols-1 sm:grid-cols-2 gap-4">
          {/* Topics */}
          <div className="p-3.5 rounded-lg border border-border bg-card space-y-2">
            <span className="font-mono text-[10px] font-bold text-muted-foreground uppercase tracking-widest flex items-center gap-1">
              <Hash className="w-3 h-3 text-amber-500" /> Topics
            </span>
            <div className="flex flex-wrap items-center gap-1.5">
              {topics.map((t) => (
                <Badge
                  key={t}
                  variant="secondary"
                  className="text-[10px] gap-1 px-2 py-0.5 bg-amber-500/10 text-amber-600 dark:text-amber-400 border border-amber-500/20"
                >
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
                  className="text-[10px] bg-muted/30 px-2 py-0.5 rounded border border-border text-foreground w-20 focus:outline-none focus:w-28 transition-all"
                />
                <button onClick={handleAddTopic} className="text-muted-foreground hover:text-foreground">
                  <Plus className="w-3 h-3" />
                </button>
              </div>
            </div>
          </div>

          {/* Named Entities */}
          <div className="p-3.5 rounded-lg border border-border bg-card space-y-2">
            <span className="font-mono text-[10px] font-bold text-muted-foreground uppercase tracking-widest flex items-center gap-1">
              <Box className="w-3 h-3 text-emerald-500" /> Named Entities
            </span>
            <div className="flex flex-wrap items-center gap-1.5">
              {entities.map((e) => (
                <Badge
                  key={e}
                  variant="secondary"
                  className="text-[10px] gap-1 px-2 py-0.5 bg-emerald-500/10 text-emerald-600 dark:text-emerald-400 border border-emerald-500/20"
                >
                  <span>{e}</span>
                  <button onClick={() => handleRemoveEntity(e)} className="hover:text-destructive">
                    <X className="w-2.5 h-2.5" />
                  </button>
                </Badge>
              ))}
              <div className="flex items-center gap-1">
                <input
                  type="text"
                  value={entityInput}
                  onChange={(e) => setEntityInput(e.target.value)}
                  onKeyDown={(e) => e.key === 'Enter' && handleAddEntity()}
                  placeholder="Add entity…"
                  className="text-[10px] bg-muted/30 px-2 py-0.5 rounded border border-border text-foreground w-20 focus:outline-none focus:w-28 transition-all"
                />
                <button onClick={handleAddEntity} className="text-muted-foreground hover:text-foreground">
                  <Plus className="w-3 h-3" />
                </button>
              </div>
            </div>
          </div>
        </div>

        {/* 5. Provenance (Human-readable with progressive disclosure for technical metadata) */}
        <div className="p-3.5 rounded-lg border border-border bg-muted/10 space-y-2 text-xs">
          <div className="flex items-center justify-between">
            <span className="font-mono text-[10px] font-bold text-muted-foreground uppercase tracking-widest flex items-center gap-1.5">
              <ShieldCheck className="w-3.5 h-3.5 text-emerald-500" /> Provenance
            </span>

            <button
              type="button"
              onClick={() => setShowTechnicalProvenance(!showTechnicalProvenance)}
              className="text-[10px] text-muted-foreground hover:text-foreground flex items-center gap-1 font-mono"
            >
              <span>{showTechnicalProvenance ? 'Hide Technical Details' : 'View Technical Details'}</span>
              {showTechnicalProvenance ? <ChevronDown className="w-3 h-3" /> : <ChevronRight className="w-3 h-3" />}
            </button>
          </div>

          <div className="grid grid-cols-1 sm:grid-cols-2 gap-2 text-[11px] text-muted-foreground">
            <div>
              <span className="text-foreground font-medium">Source Modality:</span>{' '}
              <span className="font-semibold">{sourceMeta.label}</span>
            </div>
            <div>
              <span className="text-foreground font-medium">Captured:</span>{' '}
              <span>
                {new Date(scribble.created_at).toLocaleString([], {
                  month: 'short',
                  day: 'numeric',
                  year: 'numeric',
                  hour: '2-digit',
                  minute: '2-digit',
                })}
              </span>
            </div>
          </div>

          {/* Expandable Technical Details */}
          {showTechnicalProvenance && (
            <div className="pt-2 border-t border-border/40 text-[10px] font-mono text-muted-foreground space-y-1.5 bg-muted/20 p-2.5 rounded-lg animate-in fade-in duration-150">
              <div>
                <span className="text-foreground font-medium">Scribble ID:</span> {scribble.id}
              </div>
              {scribble.source_metadata?.source_modality && (
                <div>
                  <span className="text-foreground font-medium">Source Modality:</span>{' '}
                  <span className="text-primary font-bold">{scribble.source_metadata.source_modality}</span>
                </div>
              )}
              {scribble.source_metadata?.source_voice_note_id && (
                <div>
                  <span className="text-foreground font-medium">Primary Voice Note ID:</span>{' '}
                  {scribble.source_metadata.source_voice_note_id}
                </div>
              )}
              {scribble.source_metadata?.source_voice_note_ids && Array.isArray(scribble.source_metadata.source_voice_note_ids) && (
                <div>
                  <span className="text-foreground font-medium">Contributing Voice Note IDs ({scribble.source_metadata.source_voice_note_ids.length}):</span>{' '}
                  {scribble.source_metadata.source_voice_note_ids.join(', ')}
                  {scribble.source_metadata?.is_merged && (
                    <span className="ml-1.5 px-1 py-0.2 bg-primary/20 text-primary text-[9px] rounded font-sans font-bold">MERGED</span>
                  )}
                </div>
              )}
              {scribble.source_metadata?.filename && (
                <div>
                  <span className="text-foreground font-medium">Original Filename:</span>{' '}
                  {scribble.source_metadata.filename}
                </div>
              )}
              {scribble.source_metadata?.source_scribble_ids && (
                <div>
                  <span className="text-foreground font-medium">Merged Source Scribbles:</span>{' '}
                  {JSON.stringify(scribble.source_metadata.source_scribble_ids)}
                </div>
              )}
              {scribble.ai_metadata?.last_enriched_at && (
                <div>
                  <span className="text-foreground font-medium">Last Enriched:</span>{' '}
                  {new Date(scribble.ai_metadata.last_enriched_at).toLocaleString()}
                </div>
              )}
            </div>
          )}
        </div>

        {/* 6. Connections & Merge Section */}
        <div className="space-y-3 pt-2">
          <div className="flex items-center justify-between">
            <span className="font-mono text-[10px] font-bold text-muted-foreground uppercase tracking-widest flex items-center gap-1.5">
              <LinkIcon className="w-3.5 h-3.5 text-primary" /> Knowledge Connections ({activeRelationships.length})
            </span>

            <div className="flex items-center gap-2">
              <Button
                size="sm"
                variant="outline"
                onClick={() => setModalMode('merge')}
                className="h-6 text-[10px] gap-1 px-2.5"
                title="Merge selected thoughts into a consolidated Scribble"
              >
                <GitMerge className="w-3 h-3" />
                <span>Merge Scribbles</span>
              </Button>

              <Button
                size="sm"
                variant="default"
                onClick={() => setModalMode('connect')}
                className="h-6 text-[10px] gap-1 px-2.5 font-semibold"
              >
                <Plus className="w-3 h-3" />
                <span>Connect Scribble</span>
              </Button>
            </div>
          </div>

          {/* List of Relationships */}
          <div className="space-y-1.5">
            {activeRelationships.length === 0 ? (
              <div className="p-4 rounded-lg border border-dashed border-border text-center text-xs text-muted-foreground">
                No connected thoughts yet. Click <strong>Connect Scribble</strong> to link related ideas.
              </div>
            ) : (
              activeRelationships.map((rel) => {
                const target = allScribbles.find((s) => s.id === rel.target_id);
                return (
                  <div
                    key={rel.id}
                    className="p-2.5 rounded-lg bg-card border border-border flex items-center justify-between gap-3 text-xs"
                  >
                    <div className="flex items-center gap-2 min-w-0">
                      <Badge variant="outline" className="text-[9px] font-mono px-1.5 py-0 uppercase">
                        {rel.relationship_type.replace(/_/g, ' ')}
                      </Badge>
                      <button
                        onClick={() => onSelectScribble(rel.target_id)}
                        className="font-medium text-foreground hover:text-primary truncate flex items-center gap-1 group text-left"
                      >
                        <span>{target?.title || rel.target_id}</span>
                        <ArrowUpRight className="w-3 h-3 opacity-50 group-hover:opacity-100" />
                      </button>
                      {rel.source === 'ai' && (
                        <Badge variant="secondary" className="text-[8px] font-mono px-1 py-0 text-primary">
                          AI Suggested
                        </Badge>
                      )}
                    </div>

                    <button
                      onClick={() => handleRemoveRelationship(rel.id)}
                      className="text-muted-foreground hover:text-destructive p-1 rounded hover:bg-muted"
                      title="Remove connection"
                    >
                      <X className="w-3.5 h-3.5" />
                    </button>
                  </div>
                );
              })
            )}
          </div>
        </div>

        {/* 7. AI Exploration Questions (with individual [Copy] button for each question) */}
        {effectiveQuestions.length > 0 && (
          <div className="p-4 rounded-lg bg-accent/20 border border-accent/40 space-y-2.5 text-xs">
            <div className="flex items-center gap-1.5 font-semibold text-foreground">
              <HelpCircle className="w-3.5 h-3.5 text-primary" />
              <span className="font-mono text-[10px] font-bold text-muted-foreground uppercase tracking-widest">
                AI Exploration Questions
              </span>
            </div>

            <div className="space-y-2">
              {effectiveQuestions.map((q, i) => {
                const isCopied = copiedQuestionIndex === i;
                return (
                  <div
                    key={i}
                    className="p-2.5 rounded-lg bg-card/80 border border-border/60 flex items-start justify-between gap-2.5 group"
                  >
                    <span className="text-xs text-foreground leading-relaxed flex-1">
                      {q}
                    </span>

                    <Button
                      size="sm"
                      variant="ghost"
                      onClick={() => handleCopyIndividualQuestion(q, i)}
                      className="h-6 px-1.5 text-[10px] gap-1 shrink-0 text-muted-foreground hover:text-foreground"
                      title="Copy question text"
                    >
                      {isCopied ? (
                        <>
                          <Check className="w-3 h-3 text-emerald-500" />
                          <span className="text-emerald-500">Copied</span>
                        </>
                      ) : (
                        <>
                          <Copy className="w-3 h-3 opacity-60 group-hover:opacity-100" />
                          <span>Copy</span>
                        </>
                      )}
                    </Button>
                  </div>
                );
              })}
            </div>
          </div>
        )}

        {/* 8. Secondary & Destructive Actions Footer */}
        <div className="pt-4 border-t border-border flex flex-wrap items-center justify-between gap-3">
          <Button
            size="sm"
            variant="outline"
            onClick={handleReEnrich}
            disabled={isEnriching}
            className="h-8 text-xs gap-1.5 text-primary"
            title="Re-run AI title, topic, and entity extraction"
          >
            <RefreshCw className={`w-3.5 h-3.5 ${isEnriching ? 'animate-spin' : ''}`} />
            <span>Re-Enrich with AI</span>
          </Button>

          <Button
            size="sm"
            variant="ghost"
            onClick={() => setConfirmDelete(true)}
            className="h-8 text-xs gap-1.5 text-muted-foreground hover:text-destructive hover:bg-destructive/10"
            title="Move scribble to 30-day Trash"
          >
            <Trash2 className="w-3.5 h-3.5" />
            <span>Move to Trash</span>
          </Button>
        </div>
      </div>

      {/* Viewport-Level Confirmation Modal for Delete (Requirement 5) */}
      <ConfirmationModal
        isOpen={confirmDelete}
        title="Move Scribble to Trash?"
        description={`"${scribble.title}" will be moved to Trash. It will remain recoverable for 30 days before permanent deletion.`}
        confirmLabel="Move to Trash"
        cancelLabel="Cancel"
        variant="destructive"
        onConfirm={handleDeleteConfirm}
        onCancel={() => setConfirmDelete(false)}
      />

      {/* Connect / Merge Visual Modal */}
      {modalMode && (
        <ConnectAndMergeModal
          currentScribble={scribble}
          allScribbles={allScribbles}
          mode={modalMode}
          isOpen={true}
          onClose={() => setModalMode(null)}
          onScribbleUpdated={onUpdate}
          onScribbleCreated={onScribbleCreated}
        />
      )}
    </div>
  );
};
