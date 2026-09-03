import React, { useState, useEffect, useCallback } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import {
  Scribble,
  KnowledgeGraphData,
} from '../../types';
import {
  Search,
  Plus,
  FileText,
  PlusCircle,
  LayoutGrid,
  Network,
} from 'lucide-react';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { Badge } from '@/components/ui/badge';
import { ScribbleDetailEditor } from './ScribbleDetailEditor';
import { KnowledgeGraphView } from './KnowledgeGraphView';
import { CaptureHubPage } from '../capture/CaptureHubPage';
import { EmptyState } from '../common/EmptyState';

type ScribbleSubTab = 'workspace' | 'capture' | 'graph';

export const ScribbleViewer: React.FC = () => {
  const [activeSubTab, setActiveSubTab] = useState<ScribbleSubTab>('workspace');
  const [scribbles, setScribbles] = useState<Scribble[]>([]);
  const [selectedScribbleId, setSelectedScribbleId] = useState<string | null>(null);
  const [searchQuery, setSearchQuery] = useState('');
  const [loading, setLoading] = useState(true);
  const [graphData, setGraphData] = useState<KnowledgeGraphData>({ nodes: [], edges: [] });

  // Fetch Scribbles & Graph
  const refreshData = useCallback(async () => {
    try {
      const [loadedScribbles, loadedGraph] = await Promise.all([
        invoke<Scribble[]>('get_scribbles'),
        invoke<KnowledgeGraphData>('get_knowledge_graph', { filter: null }),
      ]);
      setScribbles(loadedScribbles);
      setGraphData(loadedGraph);

      if (loadedScribbles.length > 0 && !selectedScribbleId) {
        setSelectedScribbleId(loadedScribbles[0].id);
      }
    } catch (err) {
      console.error('Failed to load scribbles or graph:', err);
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
    setActiveSubTab('workspace');
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
      {/* Top Scribbles Sub-Navigation Bar */}
      <div className="flex items-center justify-between pb-2.5 shrink-0 border-b border-border">
        {/* Sub-Tabs: Capture | Workspace | Knowledge Graph */}
        <div className="flex items-center bg-muted/60 p-1 rounded-lg border border-border text-xs">
          <button
            type="button"
            onClick={() => setActiveSubTab('capture')}
            className={`flex items-center gap-1.5 px-3 py-1.5 rounded-lg font-medium transition-all ${
              activeSubTab === 'capture'
                ? 'bg-card text-foreground font-bold shadow-xs'
                : 'text-muted-foreground hover:text-foreground'
            }`}
          >
            <PlusCircle className="w-3.5 h-3.5 text-primary" />
            <span>Capture</span>
          </button>

          <button
            type="button"
            onClick={() => setActiveSubTab('workspace')}
            className={`flex items-center gap-1.5 px-3 py-1.5 rounded-lg font-medium transition-all ${
              activeSubTab === 'workspace'
                ? 'bg-card text-foreground font-bold shadow-xs'
                : 'text-muted-foreground hover:text-foreground'
            }`}
          >
            <LayoutGrid className="w-3.5 h-3.5" />
            <span>Workspace</span>
          </button>

          <button
            type="button"
            onClick={() => {
              setActiveSubTab('graph');
              refreshData();
            }}
            className={`flex items-center gap-1.5 px-3 py-1.5 rounded-lg font-medium transition-all ${
              activeSubTab === 'graph'
                ? 'bg-card text-foreground font-bold shadow-xs'
                : 'text-muted-foreground hover:text-foreground'
            }`}
          >
            <Network className="w-3.5 h-3.5 text-blue-500" />
            <span>Knowledge Graph</span>
          </button>
        </div>

        {/* Scribble Count Badge (Shown strictly on Workspace page) */}
        {activeSubTab === 'workspace' && (
          <Badge variant="outline" className="text-[11px] font-mono text-muted-foreground border-border bg-card/60 px-2.5 py-1 animate-in fade-in duration-150">
            {scribbles.length} Scribble{scribbles.length === 1 ? '' : 's'}
          </Badge>
        )}
      </div>

      {/* Surface Tab 1: Dedicated Capture Tab */}
      {activeSubTab === 'capture' && (
        <CaptureHubPage
          onCaptureSuccess={(scribble) => {
            handleScribbleCreated(scribble);
            setSelectedScribbleId(scribble.id);
            setActiveSubTab('workspace');
          }}
          onNavigateToScribbles={() => setActiveSubTab('workspace')}
        />
      )}

      {/* Surface Tab 2: Living Knowledge Graph Tab */}
      {activeSubTab === 'graph' && (
        <div className="flex-1 flex min-h-0">
          <KnowledgeGraphView
            graphData={graphData}
            allScribbles={scribbles}
            onSelectScribble={(id) => setSelectedScribbleId(id)}
            onOpenScribbleEditor={(id) => {
              setSelectedScribbleId(id);
              setActiveSubTab('workspace');
            }}
            onScribbleUpdated={handleScribbleUpdated}
            onScribbleCreated={handleScribbleCreated}
            onScribbleDeleted={handleScribbleDeleted}
          />
        </div>
      )}

      {/* Surface Tab 3: Main Workspace (List + Editor) */}
      {activeSubTab === 'workspace' && (
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
                    title="No thoughts found"
                    description="Use the Capture tab or promote a Voice Note."
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
                  description="Click any thought from the list or create one from the Capture tab."
                  minHeight="min-h-[220px]"
                />
              </div>
            )}
          </div>
        </div>
      )}
    </div>
  );
};
