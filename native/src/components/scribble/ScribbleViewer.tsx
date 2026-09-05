import React, { useState, useEffect, useCallback } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { Scribble } from '@/types';
import {
  Search,
  FileText,
  PlusCircle,
} from 'lucide-react';
import { Input } from '@/components/ui/input';
import { Badge } from '@/components/ui/badge';
import { ScribbleDetailEditor } from './ScribbleDetailEditor';
import { EmptyState } from '../common/EmptyState';

export interface ScribbleViewerProps {
  /** Scribble to select on arrival — how the Knowledge Graph reveals a thought here. */
  focusScribbleId?: string | null;
  /**
   * Opens `Captures › Capture`, which owns every capture mode.
   *
   * Scribbles is the workspace for thoughts that already exist; it deliberately
   * carries no second implementation of capturing one.
   */
  onStartCapture: () => void;
}

export const ScribbleViewer: React.FC<ScribbleViewerProps> = ({
  focusScribbleId = null,
  onStartCapture,
}) => {
  const [scribbles, setScribbles] = useState<Scribble[]>([]);
  const [selectedScribbleId, setSelectedScribbleId] = useState<string | null>(focusScribbleId);
  const [searchQuery, setSearchQuery] = useState('');
  const [loading, setLoading] = useState(true);

  // Fetch Scribbles
  const refreshData = useCallback(async () => {
    try {
      const loadedScribbles = await invoke<Scribble[]>('get_scribbles');
      setScribbles(loadedScribbles);

      if (loadedScribbles.length > 0 && !selectedScribbleId) {
        setSelectedScribbleId(loadedScribbles[0].id);
      }
    } catch (err) {
      console.error('Failed to load scribbles:', err);
    } finally {
      setLoading(false);
    }
  }, [selectedScribbleId]);

  useEffect(() => {
    refreshData();
  }, [refreshData]);

  // Live updates from backend
  useEffect(() => {
    const unlistenSaved = listen<Scribble>('scribble-saved', ({ payload }) => {
      setScribbles((prev) => {
        const filtered = prev.filter((s) => s.id !== payload.id);
        return [payload, ...filtered];
      });
      setSelectedScribbleId((prev) => prev || payload.id);
      refreshData();
    });

    const unlistenEnriched = listen<Scribble>('scribble-enriched', ({ payload }) => {
      setScribbles((prev) => prev.map((s) => (s.id === payload.id ? payload : s)));
      refreshData();
    });

    return () => {
      unlistenSaved.then((u) => u());
      unlistenEnriched.then((u) => u());
    };
  }, [refreshData]);

  const handleScribbleCreated = (newScribble: Scribble) => {
    setScribbles((prev) => [newScribble, ...prev.filter((s) => s.id !== newScribble.id)]);
    setSelectedScribbleId(newScribble.id);
    refreshData();
  };

  const handleScribbleUpdated = (updated: Scribble) => {
    setScribbles((prev) => prev.map((s) => (s.id === updated.id ? updated : s)));
    refreshData();
  };

  const handleScribbleDeleted = async (id: string) => {
    try {
      await invoke('delete_scribble', { id });
      setScribbles((prev) => prev.filter((s) => s.id !== id));
      if (selectedScribbleId === id) {
        const remaining = scribbles.filter((s) => s.id !== id);
        setSelectedScribbleId(remaining.length > 0 ? remaining[0].id : null);
      }
      refreshData();
    } catch (err) {
      console.error('Failed to move scribble to trash:', err);
    }
  };

  // Filtered Scribbles
  const filteredScribbles = scribbles.filter((s) => {
    if (searchQuery.trim()) {
      const q = searchQuery.toLowerCase();
      const haystack = `${s.title} ${s.content} ${s.topics.join(' ')} ${s.entities.join(' ')}`.toLowerCase();
      return haystack.includes(q);
    }
    return true;
  });

  const selectedScribble = scribbles.find((s) => s.id === selectedScribbleId) || null;

  return (
    <div className="flex-1 flex flex-col gap-3 min-h-0 min-w-0 overflow-hidden">
      {/* Workspace header — the count, and the one way to add a thought. */}
      <div className="flex items-center justify-between pb-2.5 shrink-0 border-b border-border">
        <Badge variant="outline" className="text-[11px] font-mono text-muted-foreground border-border bg-card/60 px-2.5 py-1">
          {scribbles.length} Scribble{scribbles.length === 1 ? '' : 's'}
        </Badge>

        {/*
          Capture lives on the Captures surface. This is a navigation, not a
          second capture implementation.
        */}
        <button
          type="button"
          onClick={onStartCapture}
          className="flex items-center gap-1.5 px-3 py-1.5 rounded-lg border border-border bg-card text-xs font-medium text-foreground transition-colors hover:bg-muted focus:outline-none focus-visible:ring-2 focus-visible:ring-ring"
        >
          <PlusCircle className="w-3.5 h-3.5 text-primary" />
          <span>New thought</span>
        </button>
      </div>

      {/* Main Workspace (List + Editor) */}
      <div className="flex-1 flex min-h-0 min-w-0 overflow-hidden">
        <div className="flex-1 flex gap-4 min-h-0 min-w-0 overflow-hidden">
          {/* Left: Master Scribbles List */}
          <aside className="w-full md:w-80 lg:w-96 flex flex-col shrink-0 bg-card rounded-lg border border-border overflow-hidden shadow-xs transition-all duration-150">
            <div className="p-3 border-b border-border">
              <div className="relative">
                <Search className="w-3.5 h-3.5 absolute left-3 top-2.5 text-muted-foreground" />
                <Input
                  value={searchQuery}
                  onChange={(e) => setSearchQuery(e.target.value)}
                  placeholder="Search thoughts, topics…"
                  className="pl-8 text-xs h-8 bg-muted/30"
                />
              </div>
            </div>

            <div className="flex-1 overflow-y-auto p-2 space-y-1.5">
              {filteredScribbles.length === 0 ? (
                <EmptyState
                  icon={FileText}
                  title={loading ? 'Loading thoughts…' : 'No thoughts found'}
                  description={
                    loading ? 'Reading the vault.' : 'Capture one from Captures, or promote a Voice Note.'
                  }
                  minHeight="min-h-[140px]"
                  className="my-2 border-none bg-transparent"
                />
              ) : (
                filteredScribbles.map((note) => (
                  <div
                    key={note.id}
                    onClick={() => setSelectedScribbleId(note.id)}
                    className={`p-3 rounded-lg border text-left cursor-pointer transition-all ${
                      selectedScribbleId === note.id
                        ? 'bg-accent/60 border-primary/50 shadow-xs'
                        : 'bg-card border-transparent hover:bg-muted/40'
                    }`}
                  >
                    <div className="flex items-start justify-between gap-2 mb-1">
                      <h4 className="text-xs font-bold text-foreground line-clamp-1 flex-1">
                        {note.title}
                      </h4>
                      <Badge variant="outline" className="text-[8px] font-mono uppercase px-1 py-0 bg-muted">
                        {note.source_type}
                      </Badge>
                    </div>

                    <p className="text-[11px] text-muted-foreground line-clamp-2 leading-relaxed mb-2">
                      {note.summary || note.content}
                    </p>

                    <div className="flex items-center justify-between text-[10px] text-muted-foreground font-mono">
                      <span>
                        {new Date(note.created_at).toLocaleDateString([], {
                          month: 'short',
                          day: 'numeric',
                        })}
                      </span>

                      {note.topics.length > 0 && (
                        <span className="text-amber-500 truncate max-w-[120px] font-sans">
                          {note.topics[0]}
                        </span>
                      )}
                    </div>
                  </div>
                ))
              )}
            </div>
          </aside>

          {/* Right: Detail Editor Pane */}
          {selectedScribble ? (
            <div className="flex-1 flex min-h-0 min-w-0 overflow-hidden transition-all duration-150">
              <ScribbleDetailEditor
                scribble={selectedScribble}
                allScribbles={scribbles}
                onUpdate={handleScribbleUpdated}
                onDelete={handleScribbleDeleted}
                onSelectScribble={(id) => setSelectedScribbleId(id)}
                onScribbleCreated={handleScribbleCreated}
              />
            </div>
          ) : (
            <div className="flex-1 flex min-w-0 items-center justify-center p-8 bg-card rounded-lg border border-border">
              <EmptyState
                icon={FileText}
                title="Select a scribble to inspect"
                description="Click any thought from the list, or capture a new one from Captures."
                minHeight="min-h-[220px]"
              />
            </div>
          )}
        </div>
      </div>
    </div>
  );
};
