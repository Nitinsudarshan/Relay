import React, { useState, useMemo } from 'react';
import { invoke } from '@tauri-apps/api/core';
import {
  Scribble,
  ScribbleRelationship,
  ScribbleRelationshipType,
} from '../../types';
import {
  Link as LinkIcon,
  GitMerge,
  Search,
  Check,
  X,
  Sparkles,
  Hash,
  Box,
  Layers,
  ArrowRight,
  AlertTriangle,
} from 'lucide-react';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { Badge } from '@/components/ui/badge';
import { ConfirmationModal } from '../common/ConfirmationModal';

interface ConnectAndMergeModalProps {
  currentScribble: Scribble;
  allScribbles: Scribble[];
  mode: 'connect' | 'merge';
  isOpen: boolean;
  onClose: () => void;
  onScribbleUpdated: (updated: Scribble) => void;
  onScribbleCreated?: (created: Scribble) => void;
}

const RELATIONSHIP_OPTIONS: { type: ScribbleRelationshipType; label: string }[] = [
  { type: 'RELATED_TO', label: 'Related to (Default)' },
  { type: 'SAME_TOPIC', label: 'Same topic' },
  { type: 'SAME_PROJECT', label: 'Same project' },
  { type: 'EXTENDS', label: 'Extends / Continues' },
  { type: 'MENTIONS', label: 'Mentions' },
  { type: 'CONTRADICTS', label: 'Contradicts' },
  { type: 'DERIVED_FROM', label: 'Derived from' },
];

export const ConnectAndMergeModal: React.FC<ConnectAndMergeModalProps> = ({
  currentScribble,
  allScribbles,
  mode,
  isOpen,
  onClose,
  onScribbleUpdated,
  onScribbleCreated,
}) => {
  const [searchQuery, setSearchQuery] = useState('');
  const [selectedIds, setSelectedIds] = useState<string[]>([]);
  const [relType, setRelType] = useState<ScribbleRelationshipType>('RELATED_TO');
  const [busy, setBusy] = useState(false);
  const [confirmMergeOpen, setConfirmMergeOpen] = useState(false);

  // Filter candidate Scribbles (exclude current scribble)
  const candidateScribbles = useMemo(() => {
    const others = allScribbles.filter((s) => s.id !== currentScribble.id);

    // Calculate score based on shared topics and entities
    const scored = others.map((s) => {
      let score = 0;
      const sharedTopics = s.topics.filter((t) => currentScribble.topics.includes(t));
      const sharedEntities = s.entities.filter((e) => currentScribble.entities.includes(e));
      score += sharedTopics.length * 3 + sharedEntities.length * 2;

      // Existing relationship
      const alreadyLinked = currentScribble.relationships?.some((r) => r.target_id === s.id);

      return {
        scribble: s,
        score,
        sharedTopics,
        sharedEntities,
        alreadyLinked,
      };
    });

    // Apply search filter if query is present
    let filtered = scored;
    if (searchQuery.trim()) {
      const q = searchQuery.toLowerCase();
      filtered = scored.filter(
        (item) =>
          item.scribble.title.toLowerCase().includes(q) ||
          item.scribble.content.toLowerCase().includes(q) ||
          item.scribble.topics.some((t) => t.toLowerCase().includes(q)) ||
          item.scribble.entities.some((e) => e.toLowerCase().includes(q))
      );
    }

    // Sort by relevance score descending
    filtered.sort((a, b) => b.score - a.score);
    return filtered;
  }, [allScribbles, currentScribble, searchQuery]);

  if (!isOpen) return null;

  const toggleSelect = (id: string) => {
    if (selectedIds.includes(id)) {
      setSelectedIds(selectedIds.filter((x) => x !== id));
    } else {
      setSelectedIds([...selectedIds, id]);
    }
  };

  const handleExecuteConnect = async () => {
    if (selectedIds.length === 0) return;
    setBusy(true);
    try {
      let updated = currentScribble;
      for (const targetId of selectedIds) {
        const rel: ScribbleRelationship = {
          id: `rel_${Date.now()}_${Math.random().toString(36).substring(2, 7)}`,
          target_id: targetId,
          relationship_type: relType,
          confidence: 1.0,
          source: 'user',
        };
        updated = await invoke<Scribble>('add_scribble_relationship', {
          sourceId: currentScribble.id,
          relationship: rel,
        });
      }
      onScribbleUpdated(updated);
      onClose();
    } catch (err) {
      console.error('Failed to connect scribbles:', err);
    } finally {
      setBusy(false);
    }
  };

  const handleExecuteMerge = async () => {
    if (selectedIds.length === 0) return;
    setBusy(true);
    try {
      const sourceIds = [currentScribble.id, ...selectedIds];
      const mergedScribble = await invoke<Scribble>('merge_scribbles', {
        sourceIds,
      });

      if (onScribbleCreated) {
        onScribbleCreated(mergedScribble);
      }
      setConfirmMergeOpen(false);
      onClose();
    } catch (err) {
      console.error('Failed to merge scribbles:', err);
    } finally {
      setBusy(false);
    }
  };

  return (
    <>
      <div className="fixed inset-0 z-40 flex items-center justify-center bg-black/60 backdrop-blur-xs p-4 animate-in fade-in duration-150">
        <div className="w-full max-w-2xl bg-card border border-border rounded-lg shadow-xl overflow-hidden flex flex-col max-h-[85vh]">
          {/* Modal Header */}
          <div className="p-5 border-b border-border flex items-center justify-between shrink-0">
            <div className="flex items-center gap-2.5">
              <div className="p-2 rounded-lg bg-primary/10 text-primary">
                {mode === 'connect' ? <LinkIcon className="w-5 h-5" /> : <GitMerge className="w-5 h-5" />}
              </div>
              <div>
                <h3 className="text-sm font-bold text-foreground">
                  {mode === 'connect' ? 'Connect Scribble' : 'Merge Scribbles into Consolidated Thought'}
                </h3>
                <p className="text-xs text-muted-foreground">
                  Current: <span className="font-semibold text-foreground">{currentScribble.title}</span>
                </p>
              </div>
            </div>

            <button
              onClick={onClose}
              className="text-muted-foreground hover:text-foreground p-1 rounded-lg hover:bg-muted"
            >
              <X className="w-4 h-4" />
            </button>
          </div>

          {/* Modal Body */}
          <div className="flex-1 overflow-y-auto p-5 space-y-4">
            {/* Merge Explainer Banner */}
            {mode === 'merge' && (
              <div className="p-3.5 bg-primary/5 border border-primary/20 rounded-lg space-y-1 text-xs">
                <div className="flex items-center gap-1.5 font-semibold text-foreground">
                  <Sparkles className="w-3.5 h-3.5 text-primary" />
                  <span>Consolidated Synthesis</span>
                </div>
                <p className="text-[11px] text-muted-foreground leading-relaxed">
                  Merging synthesizes the selected thoughts into <strong>one fresh consolidated Scribble</strong>. The original source Scribbles are retired to Trash (recoverable for 30 days) and preserved in provenance metadata.
                </p>
              </div>
            )}

            {/* Search Input */}
            <div className="relative">
              <Search className="w-4 h-4 absolute left-3 top-2.5 text-muted-foreground" />
              <Input
                value={searchQuery}
                onChange={(e) => setSearchQuery(e.target.value)}
                placeholder="Search candidate thoughts by title, text, or topic…"
                className="pl-9 text-xs h-9 bg-muted/20"
              />
            </div>

            {/* Candidate Scribbles List */}
            <div className="space-y-2 max-h-72 overflow-y-auto pr-1">
              <span className="text-[10px] font-bold font-mono text-muted-foreground uppercase tracking-wider block">
                Suggested Scribbles ({candidateScribbles.length})
              </span>

              {candidateScribbles.length === 0 ? (
                <div className="text-center py-8 text-xs text-muted-foreground">
                  No other Scribbles found.
                </div>
              ) : (
                candidateScribbles.map(({ scribble, score, sharedTopics, sharedEntities, alreadyLinked }) => {
                  const isSelected = selectedIds.includes(scribble.id);

                  return (
                    <div
                      key={scribble.id}
                      onClick={() => toggleSelect(scribble.id)}
                      className={`p-3.5 rounded-lg border text-left cursor-pointer transition-all flex items-start justify-between gap-3 ${
                        isSelected
                          ? 'border-primary bg-primary/10 shadow-xs'
                          : 'border-border bg-card hover:bg-muted/40'
                      }`}
                    >
                      <div className="flex-1 min-w-0 space-y-1">
                        <div className="flex items-center gap-2">
                          <h4 className="text-xs font-bold text-foreground truncate">{scribble.title}</h4>
                          {alreadyLinked && (
                            <Badge variant="outline" className="text-[8px] font-mono text-primary px-1 py-0">
                              Already Connected
                            </Badge>
                          )}
                        </div>

                        <p className="text-[11px] text-muted-foreground line-clamp-2 leading-relaxed">
                          {scribble.summary || scribble.content}
                        </p>

                        {/* Shared Topic / Entity Badges (Topics and Named Entities, no hashtag tags) */}
                        {(sharedTopics.length > 0 || sharedEntities.length > 0) && (
                          <div className="flex flex-wrap items-center gap-1 pt-1">
                            {sharedTopics.map((t) => (
                              <Badge key={t} variant="secondary" className="text-[9px] px-1.5 py-0 bg-amber-500/10 text-amber-600 dark:text-amber-400 font-sans">
                                {t}
                              </Badge>
                            ))}
                            {sharedEntities.map((e) => (
                              <Badge key={e} variant="secondary" className="text-[9px] px-1.5 py-0 bg-emerald-500/10 text-emerald-600 dark:text-emerald-400 font-sans">
                                {e}
                              </Badge>
                            ))}
                          </div>
                        )}
                      </div>

                      <div
                        className={`w-5 h-5 rounded-lg border flex items-center justify-center shrink-0 transition-all ${
                          isSelected
                            ? 'bg-primary border-primary text-primary-foreground'
                            : 'border-border bg-background'
                        }`}
                      >
                        {isSelected && <Check className="w-3.5 h-3.5" />}
                      </div>
                    </div>
                  );
                })
              )}
            </div>

            {/* Secondary Relationship Type selector (for Connect mode) */}
            {mode === 'connect' && selectedIds.length > 0 && (
              <div className="p-3.5 rounded-lg bg-muted/20 border border-border space-y-2 animate-in fade-in duration-150">
                <label className="text-[11px] font-semibold text-foreground block">
                  How are they connected? (Optional)
                </label>
                <div className="grid grid-cols-2 sm:grid-cols-3 gap-1.5">
                  {RELATIONSHIP_OPTIONS.map((opt) => (
                    <button
                      key={opt.type}
                      type="button"
                      onClick={() => setRelType(opt.type)}
                      className={`px-2.5 py-1.5 text-[11px] rounded-lg border text-left transition-all ${
                        relType === opt.type
                          ? 'border-primary bg-primary/10 text-primary font-semibold'
                          : 'border-border bg-card text-muted-foreground hover:text-foreground'
                      }`}
                    >
                      {opt.label}
                    </button>
                  ))}
                </div>
              </div>
            )}
          </div>

          {/* Modal Footer */}
          <div className="p-4 border-t border-border flex items-center justify-between bg-muted/10 shrink-0">
            <span className="text-xs text-muted-foreground">
              {selectedIds.length} candidate{selectedIds.length === 1 ? '' : 's'} selected
            </span>

            <div className="flex items-center gap-2">
              <Button size="sm" variant="ghost" onClick={onClose} disabled={busy} className="h-8 text-xs">
                Cancel
              </Button>

              {mode === 'connect' ? (
                <Button
                  size="sm"
                  onClick={handleExecuteConnect}
                  disabled={busy || selectedIds.length === 0}
                  className="h-8 text-xs gap-1.5 font-semibold"
                >
                  <LinkIcon className="w-3.5 h-3.5" />
                  <span>Connect ({selectedIds.length})</span>
                </Button>
              ) : (
                <Button
                  size="sm"
                  variant="default"
                  onClick={() => setConfirmMergeOpen(true)}
                  disabled={busy || selectedIds.length === 0}
                  className="h-8 text-xs gap-1.5 font-semibold"
                >
                  <GitMerge className="w-3.5 h-3.5" />
                  <span>Merge ({selectedIds.length + 1} Scribbles)</span>
                </Button>
              )}
            </div>
          </div>
        </div>
      </div>

      {/* Viewport-Level Confirmation Modal for Merge */}
      <ConfirmationModal
        isOpen={confirmMergeOpen}
        title={`Merge ${selectedIds.length + 1} Scribbles?`}
        description={`This will consolidate "${currentScribble.title}" and ${selectedIds.length} other thought(s) into a new synthesized Scribble. The original ${selectedIds.length + 1} Scribbles will be safely retired to Trash (recoverable for 30 days) with full provenance preserved.`}
        confirmLabel="Confirm Merge"
        cancelLabel="Cancel"
        variant="primary"
        isBusy={busy}
        onConfirm={handleExecuteMerge}
        onCancel={() => setConfirmMergeOpen(false)}
      />
    </>
  );
};
