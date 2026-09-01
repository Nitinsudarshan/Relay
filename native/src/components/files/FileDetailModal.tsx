import React, { useState } from 'react';
import {
  X,
  FileText,
  Sparkles,
  Wand2,
  ExternalLink,
  RefreshCw,
  Tag as TagIcon,
  Info,
  HardDrive,
  FileCode,
  AlertTriangle,
  Layers,
  Plus,
  Copy,
  Check
} from 'lucide-react';
import { VaultFile, Scribble } from '../../types';
import { MarkdownView } from '../common/MarkdownView';

interface FileDetailModalProps {
  file: VaultFile;
  onClose: () => void;
  onSummarize: (id: string) => Promise<void>;
  onEnrich: (id: string) => Promise<void>;
  onCreateScribble: (id: string) => Promise<Scribble | undefined>;
  onReprocess: (id: string) => Promise<void>;
  onUpdateTags: (id: string, tags: string[], topics: string[], entities: string[]) => Promise<void>;
  onOpenLocation: (id: string) => Promise<void>;
  onNavigateTab?: (tab: string) => void;
}

export const FileDetailModal: React.FC<FileDetailModalProps> = ({
  file,
  onClose,
  onSummarize,
  onEnrich,
  onCreateScribble,
  onReprocess,
  onUpdateTags,
  onOpenLocation,
  onNavigateTab
}) => {
  const [activeTab, setActiveTab] = useState<'content' | 'intelligence' | 'metadata'>('content');
  const [busyAction, setBusyAction] = useState<string | null>(null);
  const [copiedSummary, setCopiedSummary] = useState(false);
  const [newTag, setNewTag] = useState('');
  const [newTopic, setNewTopic] = useState('');
  const [newEntity, setNewEntity] = useState('');

  const isScribbleCreated = Boolean(file.linked_scribble_id);

  const formatBytes = (bytes: number) => {
    if (bytes === 0) return '0 Bytes';
    const k = 1024;
    const sizes = ['Bytes', 'KB', 'MB', 'GB'];
    const i = Math.floor(Math.log(bytes) / Math.log(k));
    return parseFloat((bytes / Math.pow(k, i)).toFixed(2)) + ' ' + sizes[i];
  };

  const formatDate = (iso: string) => {
    try {
      return new Date(iso).toLocaleString(undefined, {
        dateStyle: 'medium',
        timeStyle: 'short'
      });
    } catch {
      return iso;
    }
  };

  const handleAction = async (name: string, fn: () => Promise<void>) => {
    setBusyAction(name);
    try {
      await fn();
    } finally {
      setBusyAction(null);
    }
  };

  const handleCopySummary = () => {
    if (!file.summary) return;
    navigator.clipboard.writeText(file.summary);
    setCopiedSummary(true);
    setTimeout(() => setCopiedSummary(false), 2000);
  };

  const handleAddTag = async () => {
    if (!newTag.trim()) return;
    const updated = Array.from(new Set([...file.tags, newTag.trim()]));
    await onUpdateTags(file.id, updated, file.topics, file.entities);
    setNewTag('');
  };

  const handleRemoveTag = async (tagToRemove: string) => {
    const updated = file.tags.filter((t) => t !== tagToRemove);
    await onUpdateTags(file.id, updated, file.topics, file.entities);
  };

  const handleAddTopic = async () => {
    if (!newTopic.trim()) return;
    const updated = Array.from(new Set([...file.topics, newTopic.trim()]));
    await onUpdateTags(file.id, file.tags, updated, file.entities);
    setNewTopic('');
  };

  const handleRemoveTopic = async (topicToRemove: string) => {
    const updated = file.topics.filter((t) => t !== topicToRemove);
    await onUpdateTags(file.id, file.tags, updated, file.entities);
  };

  const handleAddEntity = async () => {
    if (!newEntity.trim()) return;
    const updated = Array.from(new Set([...file.entities, newEntity.trim()]));
    await onUpdateTags(file.id, file.tags, file.topics, updated);
    setNewEntity('');
  };

  const handleRemoveEntity = async (entityToRemove: string) => {
    const updated = file.entities.filter((e) => e !== entityToRemove);
    await onUpdateTags(file.id, file.tags, file.topics, updated);
  };

  return (
    <div className="fixed inset-0 z-50 bg-black/60 backdrop-blur-sm flex items-center justify-center p-4 overflow-y-auto">
      <div className="bg-card text-card-foreground rounded-xl border border-border shadow-2xl w-[80vw] max-w-[80vw] max-h-[88vh] flex flex-col overflow-hidden animate-in fade-in zoom-in-95 duration-200">
        {/* Header */}
        <div className="p-6 border-b border-border flex items-start justify-between bg-muted/20">
          <div className="flex items-start gap-4">
            <div className="p-3 bg-primary/10 text-primary rounded-xl">
              <FileText className="w-8 h-8" />
            </div>
            <div>
              <div className="flex items-center gap-2">
                <h2 className="text-xl font-bold text-foreground">{file.original_filename}</h2>
                <span className="px-2 py-0.5 text-xs font-semibold rounded-full bg-accent text-accent-foreground uppercase">
                  {file.file_type}
                </span>
                {file.ai_metadata?.last_enriched_at && (
                  <span className="px-2 py-0.5 text-xs font-medium rounded-full bg-emerald-500/10 text-emerald-500 border border-emerald-500/20 flex items-center gap-1">
                    <Check className="w-3 h-3 text-emerald-500" />
                    <span>
                      Analysed ·{' '}
                      {(() => {
                        try {
                          const date = new Date(file.ai_metadata.last_enriched_at);
                          const diffMins = Math.floor((Date.now() - date.getTime()) / (1000 * 60));
                          if (diffMins < 1) return 'just now';
                          if (diffMins < 60) return `${diffMins} min ago`;
                          const diffHours = Math.floor(diffMins / 60);
                          if (diffHours < 24) return `${diffHours}h ago`;
                          return date.toLocaleDateString(undefined, { month: 'short', day: 'numeric' });
                        } catch {
                          return '';
                        }
                      })()}
                    </span>
                  </span>
                )}
                {file.extraction_status === 'unsupported' && (
                  <span className="px-2 py-0.5 text-xs font-medium rounded-full bg-amber-500/10 text-amber-500 border border-amber-500/20">
                    Text Extraction Unsupported
                  </span>
                )}
              </div>
              <p className="text-xs text-muted-foreground mt-1 flex items-center gap-3">
                <span>{formatBytes(file.size_bytes)}</span>
                <span>•</span>
                <span>Imported {formatDate(file.created_at)}</span>
              </p>
            </div>
          </div>

          <button
            onClick={onClose}
            className="p-2 text-muted-foreground hover:text-foreground hover:bg-muted rounded-lg transition-colors"
          >
            <X className="w-5 h-5" />
          </button>
        </div>

        {/* Action Toolbar */}
        <div className="px-6 py-3 bg-muted/40 border-b border-border flex items-center justify-between gap-2 overflow-x-auto">
          <div className="flex items-center gap-2">
            <button
              disabled={busyAction !== null}
              onClick={() => handleAction('analyze', () => onEnrich(file.id))}
              className="inline-flex items-center gap-1.5 px-3.5 py-1.5 text-xs font-semibold bg-primary hover:bg-primary/90 text-primary-foreground rounded-lg transition-colors shadow-sm disabled:opacity-50"
              title={
                file.ai_metadata?.last_enriched_at
                  ? 'Re-run the analysis and refresh its derived knowledge.'
                  : 'Analyse this file to generate structured knowledge, topics, entities, concepts and connections.'
              }
            >
              {busyAction === 'analyze' ? (
                <RefreshCw className="w-3.5 h-3.5 animate-spin" />
              ) : (
                <Wand2 className="w-3.5 h-3.5" />
              )}
              {busyAction === 'analyze'
                ? 'Analysing…'
                : file.ai_metadata?.last_enriched_at
                ? 'Re-analyse'
                : 'Analyse'}
            </button>

            <button
              disabled={busyAction !== null}
              onClick={() => handleAction('summarize', () => onSummarize(file.id))}
              className="inline-flex items-center gap-1.5 px-3.5 py-1.5 text-xs font-medium border border-border hover:bg-muted text-foreground rounded-lg transition-colors disabled:opacity-50"
              title="Summarise this file in a concise structured summary"
            >
              {busyAction === 'summarize' ? (
                <RefreshCw className="w-3.5 h-3.5 animate-spin" />
              ) : (
                <Sparkles className="w-3.5 h-3.5" />
              )}
              {busyAction === 'summarize' ? 'Summarising…' : 'Summarise'}
            </button>

            {isScribbleCreated ? (
              <button
                onClick={() => {
                  onClose();
                  if (onNavigateTab) onNavigateTab('scribble');
                }}
                className="inline-flex items-center gap-1.5 px-3 py-1.5 text-xs font-medium bg-amber-500/15 border border-amber-500/30 text-amber-500 rounded-lg transition-colors hover:bg-amber-500/25"
                title="View linked Scribble in Knowledge Layer"
              >
                <Check className="w-3.5 h-3.5 text-emerald-500" />
                Scribble Created
              </button>
            ) : (
              <button
                disabled={busyAction !== null}
                onClick={() => handleAction('scribble', async () => { await onCreateScribble(file.id); })}
                className="inline-flex items-center gap-1.5 px-3 py-1.5 text-xs font-medium bg-amber-500/10 hover:bg-amber-500/20 text-amber-500 rounded-lg transition-colors disabled:opacity-50"
              >
                {busyAction === 'scribble' ? (
                  <RefreshCw className="w-3.5 h-3.5 animate-spin" />
                ) : (
                  <Sparkles className="w-3.5 h-3.5 text-amber-500" />
                )}
                Create Scribble
              </button>
            )}
          </div>

          <button
            onClick={() => onOpenLocation(file.id)}
            title="Open file in Windows Explorer"
            className="inline-flex items-center gap-1.5 px-3 py-1.5 text-xs font-medium border border-border hover:bg-muted text-foreground rounded-lg transition-colors"
          >
            <ExternalLink className="w-3.5 h-3.5" />
            Open File Location
          </button>
        </div>

        {/* Tab Navigation */}
        <div className="flex border-b border-border bg-card px-6">
          <button
            onClick={() => setActiveTab('content')}
            className={`py-3 px-4 text-sm font-medium border-b-2 transition-colors ${
              activeTab === 'content'
                ? 'border-primary text-primary'
                : 'border-transparent text-muted-foreground hover:text-foreground'
            }`}
          >
            Extracted Content
          </button>
          <button
            onClick={() => setActiveTab('intelligence')}
            className={`py-3 px-4 text-sm font-medium border-b-2 transition-colors ${
              activeTab === 'intelligence'
                ? 'border-primary text-primary'
                : 'border-transparent text-muted-foreground hover:text-foreground'
            }`}
          >
            Summary & Intelligence
          </button>
          <button
            onClick={() => setActiveTab('metadata')}
            className={`py-3 px-4 text-sm font-medium border-b-2 transition-colors ${
              activeTab === 'metadata'
                ? 'border-primary text-primary'
                : 'border-transparent text-muted-foreground hover:text-foreground'
            }`}
          >
            File Info & Metadata
          </button>
        </div>

        {/* Tab Content Body */}
        <div className="p-6 overflow-y-auto flex-1 space-y-6">
          {activeTab === 'content' && (
            <div className="space-y-4">
              {file.extraction_status === 'unsupported' ? (
                <div className="p-4 rounded-xl bg-amber-500/10 border border-amber-500/20 text-amber-700 dark:text-amber-300 text-sm flex items-start gap-3">
                  <AlertTriangle className="w-5 h-5 shrink-0 mt-0.5" />
                  <div>
                    <p className="font-semibold">Text extraction not supported for legacy .doc format.</p>
                    <p className="text-xs mt-1">
                      Relay copied your file safely into the Vault. To extract text and generate summaries, please convert the file to .docx or .pdf.
                    </p>
                  </div>
                </div>
              ) : file.content.trim().length === 0 ? (
                <div className="p-8 text-center text-muted-foreground">
                  <FileCode className="w-12 h-12 mx-auto text-muted-foreground/40 mb-3" />
                  <p className="text-sm font-medium">No extracted text content available.</p>
                  <p className="text-xs text-muted-foreground mt-1">
                    Click Re-analyze to attempt text extraction again.
                  </p>
                </div>
              ) : (
                <div className="bg-muted/30 p-6 rounded-xl border border-border/60 font-mono text-sm text-foreground whitespace-pre-wrap leading-relaxed max-h-[55vh] overflow-y-auto">
                  {file.content}
                </div>
              )}
            </div>
          )}

          {activeTab === 'intelligence' && (
            <div className="space-y-6">
              {/* AI Summary Structured like Scribble Summary */}
              <div className="p-4 rounded-xl bg-muted/30 border border-border space-y-3 text-xs">
                <div className="flex items-center justify-between pb-1 border-b border-border/40">
                  <div className="flex items-center gap-1.5 font-semibold text-foreground">
                    <Sparkles className="w-3.5 h-3.5 text-primary" />
                    <span className="font-mono text-[10px] font-bold text-muted-foreground uppercase tracking-widest">
                      AI Summary
                    </span>
                  </div>

                  {file.summary && (
                    <button
                      onClick={handleCopySummary}
                      className="inline-flex items-center gap-1 px-2 py-1 text-[10px] font-medium text-muted-foreground hover:text-foreground rounded transition-colors"
                      title="Copy AI summary"
                    >
                      {copiedSummary ? (
                        <>
                          <Check className="w-3 h-3 text-emerald-500" />
                          <span className="text-emerald-500 font-semibold">Copied</span>
                        </>
                      ) : (
                        <>
                          <Copy className="w-3 h-3 opacity-60 hover:opacity-100" />
                          <span>Copy</span>
                        </>
                      )}
                    </button>
                  )}
                </div>

                {file.summary ? (
                  <div className="text-xs text-foreground leading-relaxed">
                    <MarkdownView content={file.summary} />
                  </div>
                ) : (
                  <p className="text-xs text-muted-foreground italic py-2">
                    No summary generated yet. Click Summarise above to generate an AI summary.
                  </p>
                )}
              </div>

              {/* Topics */}
              <div className="p-4 rounded-xl bg-muted/30 border border-border space-y-3 text-xs">
                <div className="flex items-center gap-1.5 font-semibold text-foreground pb-1 border-b border-border/40">
                  <Layers className="w-3.5 h-3.5 text-secondary" />
                  <span className="font-mono text-[10px] font-bold text-muted-foreground uppercase tracking-widest">
                    Topics & Concepts
                  </span>
                </div>
                <div className="flex flex-wrap items-center gap-1.5">
                  {file.topics.map((topic, i) => (
                    <span key={i} className="inline-flex items-center gap-1 px-2.5 py-1 text-xs rounded-lg bg-secondary/80 text-secondary-foreground font-medium">
                      {topic}
                      <button onClick={() => handleRemoveTopic(topic)} className="hover:text-destructive">
                        <X className="w-3 h-3" />
                      </button>
                    </span>
                  ))}
                  <div className="inline-flex items-center gap-1">
                    <input
                      type="text"
                      placeholder="Add topic..."
                      value={newTopic}
                      onChange={(e) => setNewTopic(e.target.value)}
                      onKeyDown={(e) => e.key === 'Enter' && handleAddTopic()}
                      className="px-2.5 py-1 text-xs rounded-lg bg-muted border border-border focus:outline-none focus:ring-1 focus:ring-primary w-28"
                    />
                    <button onClick={handleAddTopic} className="p-1 hover:bg-muted rounded">
                      <Plus className="w-3.5 h-3.5 text-muted-foreground" />
                    </button>
                  </div>
                </div>
              </div>

              {/* Named Entities */}
              <div className="p-4 rounded-xl bg-muted/30 border border-border space-y-3 text-xs">
                <div className="flex items-center gap-1.5 font-semibold text-foreground pb-1 border-b border-border/40">
                  <TagIcon className="w-3.5 h-3.5 text-accent-foreground" />
                  <span className="font-mono text-[10px] font-bold text-muted-foreground uppercase tracking-widest">
                    Named Entities
                  </span>
                </div>
                <div className="flex flex-wrap items-center gap-1.5">
                  {file.entities.map((entity, i) => (
                    <span key={i} className="inline-flex items-center gap-1 px-2.5 py-1 text-xs rounded-lg bg-accent text-accent-foreground font-medium">
                      {entity}
                      <button onClick={() => handleRemoveEntity(entity)} className="hover:text-destructive">
                        <X className="w-3 h-3" />
                      </button>
                    </span>
                  ))}
                  <div className="inline-flex items-center gap-1">
                    <input
                      type="text"
                      placeholder="Add entity..."
                      value={newEntity}
                      onChange={(e) => setNewEntity(e.target.value)}
                      onKeyDown={(e) => e.key === 'Enter' && handleAddEntity()}
                      className="px-2.5 py-1 text-xs rounded-lg bg-muted border border-border focus:outline-none focus:ring-1 focus:ring-primary w-28"
                    />
                    <button onClick={handleAddEntity} className="p-1 hover:bg-muted rounded">
                      <Plus className="w-3.5 h-3.5 text-muted-foreground" />
                    </button>
                  </div>
                </div>
              </div>

              {/* Tags */}
              <div className="p-4 rounded-xl bg-muted/30 border border-border space-y-3 text-xs">
                <div className="flex items-center gap-1.5 font-semibold text-foreground pb-1 border-b border-border/40">
                  <TagIcon className="w-3.5 h-3.5" />
                  <span className="font-mono text-[10px] font-bold text-muted-foreground uppercase tracking-widest">
                    Custom Tags
                  </span>
                </div>
                <div className="flex flex-wrap items-center gap-1.5">
                  {file.tags.map((tag, i) => (
                    <span key={i} className="inline-flex items-center gap-1 px-2.5 py-1 text-xs rounded-lg bg-muted text-foreground font-medium">
                      #{tag}
                      <button onClick={() => handleRemoveTag(tag)} className="hover:text-destructive">
                        <X className="w-3 h-3" />
                      </button>
                    </span>
                  ))}
                  <div className="inline-flex items-center gap-1">
                    <input
                      type="text"
                      placeholder="Add tag..."
                      value={newTag}
                      onChange={(e) => setNewTag(e.target.value)}
                      onKeyDown={(e) => e.key === 'Enter' && handleAddTag()}
                      className="px-2.5 py-1 text-xs rounded-lg bg-muted border border-border focus:outline-none focus:ring-1 focus:ring-primary w-28"
                    />
                    <button onClick={handleAddTag} className="p-1 hover:bg-muted rounded">
                      <Plus className="w-3.5 h-3.5 text-muted-foreground" />
                    </button>
                  </div>
                </div>
              </div>
            </div>
          )}

          {activeTab === 'metadata' && (
            <div className="space-y-4 text-xs">
              <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
                <div className="p-4 rounded-xl bg-muted/30 border border-border space-y-2">
                  <div className="flex items-center gap-2 text-muted-foreground font-medium">
                    <Info className="w-4 h-4 text-primary" />
                    File Identity
                  </div>
                  <div>
                    <span className="text-muted-foreground">Relay Stable ID:</span>
                    <p className="font-mono text-foreground mt-0.5">{file.id}</p>
                  </div>
                  <div>
                    <span className="text-muted-foreground">Original Filename:</span>
                    <p className="font-medium text-foreground mt-0.5">{file.original_filename}</p>
                  </div>
                  <div>
                    <span className="text-muted-foreground">MIME Type:</span>
                    <p className="font-mono text-foreground mt-0.5">{file.mime_type}</p>
                  </div>
                  <div>
                    <span className="text-muted-foreground">Content Hash (SHA-256):</span>
                    <p className="font-mono text-foreground mt-0.5 truncate">{file.content_hash}</p>
                  </div>
                </div>

                <div className="p-4 rounded-xl bg-muted/30 border border-border space-y-2">
                  <div className="flex items-center gap-2 text-muted-foreground font-medium">
                    <HardDrive className="w-4 h-4 text-primary" />
                    Storage & Provenance
                  </div>
                  <div>
                    <span className="text-muted-foreground">Last Known Original Source Path:</span>
                    <p className="font-mono text-foreground mt-0.5 truncate">{file.last_known_source_path}</p>
                    <p className="text-[10px] text-muted-foreground mt-0.5">
                      ✓ Original file outside Relay remains 100% untouched.
                    </p>
                  </div>
                  <div>
                    <span className="text-muted-foreground">Relay Vault Relative Path:</span>
                    <p className="font-mono text-foreground mt-0.5 truncate">{file.vault_path}</p>
                  </div>
                  <div>
                    <span className="text-muted-foreground">Extraction / Processing Status:</span>
                    <p className="font-medium text-foreground mt-0.5 capitalize">
                      {file.extraction_status} / {file.processing_status}
                    </p>
                  </div>
                </div>
              </div>
            </div>
          )}
        </div>
      </div>
    </div>
  );
};
