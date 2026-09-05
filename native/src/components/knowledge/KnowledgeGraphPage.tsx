import React, { useCallback, useEffect, useMemo, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { Network, RefreshCw } from 'lucide-react';

import { Badge } from '@/components/ui/badge';
import { Button } from '@/components/ui/button';
import { EmptyState } from '@/components/common/EmptyState';

import { KnowledgeGraphView } from './KnowledgeGraphView';

import type { KnowledgeGraphData, KnowledgeTelemetrySnapshot, Scribble } from '@/types';

interface KnowledgeGraphPageProps {
  /**
   * Reveals a scribble in the Scribbles workspace. The graph itself owns
   * connecting, merging and trashing; editing a thought's prose belongs to the
   * surface built for it, so opening the editor is a navigation, not a mode.
   */
  onOpenScribble?: (id: string) => void;
}

/**
 * The Knowledge Graph as its own surface.
 *
 * It reads the same two commands the Scribbles workspace does — `get_scribbles`
 * for the thoughts a node can act on, `get_knowledge_graph` for the topology —
 * because the graph spans more than scribbles (topics, entities, sources,
 * meetings, documents) and is no longer a sub-tab of one of its inputs.
 */
export const KnowledgeGraphPage: React.FC<KnowledgeGraphPageProps> = ({ onOpenScribble }) => {
  const [graphData, setGraphData] = useState<KnowledgeGraphData>({ nodes: [], edges: [] });
  const [scribbles, setScribbles] = useState<Scribble[]>([]);
  const [telemetry, setTelemetry] = useState<KnowledgeTelemetrySnapshot | null>(null);
  const [loading, setLoading] = useState(true);
  const [refreshing, setRefreshing] = useState(false);

  const refreshData = useCallback(async () => {
    try {
      const [loadedGraph, loadedScribbles, loadedTelemetry] = await Promise.all([
        invoke<KnowledgeGraphData>('get_knowledge_graph', { filter: null }),
        invoke<Scribble[]>('get_scribbles'),
        invoke<KnowledgeTelemetrySnapshot>('get_knowledge_telemetry').catch(() => null),
      ]);
      setGraphData(loadedGraph ?? { nodes: [], edges: [] });
      setScribbles(loadedScribbles ?? []);
      setTelemetry(loadedTelemetry);
    } catch (err) {
      console.error('Failed to load the knowledge graph:', err);
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    refreshData();
  }, [refreshData]);

  // Anything that changes a scribble changes the graph it sits in.
  useEffect(() => {
    const unlistenSaved = listen<Scribble>('scribble-saved', () => refreshData());
    const unlistenEnriched = listen<Scribble>('scribble-enriched', () => refreshData());

    return () => {
      unlistenSaved.then((u) => u());
      unlistenEnriched.then((u) => u());
    };
  }, [refreshData]);

  const handleManualRefresh = async () => {
    setRefreshing(true);
    await refreshData();
    setRefreshing(false);
  };

  const handleScribbleUpdated = (updated: Scribble) => {
    setScribbles((prev) => prev.map((s) => (s.id === updated.id ? updated : s)));
    refreshData();
  };

  const handleScribbleCreated = (created: Scribble) => {
    setScribbles((prev) => [created, ...prev.filter((s) => s.id !== created.id)]);
    refreshData();
  };

  const handleScribbleDeleted = async (id: string) => {
    try {
      await invoke('delete_scribble', { id });
      setScribbles((prev) => prev.filter((s) => s.id !== id));
      refreshData();
    } catch (err) {
      console.error('Failed to move scribble to trash:', err);
    }
  };

  const counts = useMemo(
    () => ({
      nodes: graphData.nodes.length,
      edges: graphData.edges.length,
      orphans: graphData.nodes.filter((n) => n.degree === 0).length,
    }),
    [graphData],
  );

  return (
    <div className="flex-1 flex flex-col gap-3 min-h-0 min-w-0 overflow-hidden">
      {/* Topology summary + manual reload */}
      <div className="flex items-center justify-between gap-3 pb-2.5 shrink-0 border-b border-border">
        {/*
          The toolbar inside the canvas already counts the nodes currently
          visible, so this row deliberately reports only what it does not: how
          much of the graph is linked, and the resolved knowledge behind it.
        */}
        <div className="flex items-center gap-1.5 flex-wrap">
          <Badge variant="outline" className="text-[11px] font-mono text-muted-foreground bg-card/60 px-2.5 py-1">
            {counts.edges} link{counts.edges === 1 ? '' : 's'}
          </Badge>
          <Badge variant="outline" className="text-[11px] font-mono text-muted-foreground bg-card/60 px-2.5 py-1">
            {counts.orphans} unconnected
          </Badge>
          {telemetry && (
            <Badge variant="outline" className="text-[11px] font-mono text-muted-foreground bg-card/60 px-2.5 py-1">
              {telemetry.total_entities} entit{telemetry.total_entities === 1 ? 'y' : 'ies'} ·{' '}
              {telemetry.total_relationships} relationship
              {telemetry.total_relationships === 1 ? '' : 's'} · {telemetry.active_memories} memor
              {telemetry.active_memories === 1 ? 'y' : 'ies'}
            </Badge>
          )}
        </div>

        <Button
          size="sm"
          variant="outline"
          onClick={handleManualRefresh}
          disabled={refreshing}
          className="h-8 text-xs gap-1.5 shrink-0"
        >
          <RefreshCw className={`w-3.5 h-3.5 ${refreshing ? 'animate-spin' : ''}`} />
          <span>Rebuild graph</span>
        </Button>
      </div>

      {!loading && counts.nodes === 0 ? (
        <EmptyState
          icon={Network}
          title="Nothing to connect yet"
          description="The graph is built from Scribbles, their topics and the entities Relay resolves out of them. Capture a thought, promote a Voice Note or import a document and it will appear here."
          minHeight="min-h-[260px]"
        />
      ) : (
        <div className="flex-1 flex min-h-0">
          <KnowledgeGraphView
            graphData={graphData}
            allScribbles={scribbles}
            isLoading={loading}
            onOpenScribbleEditor={onOpenScribble}
            onScribbleUpdated={handleScribbleUpdated}
            onScribbleCreated={handleScribbleCreated}
            onScribbleDeleted={handleScribbleDeleted}
          />
        </div>
      )}
    </div>
  );
};
