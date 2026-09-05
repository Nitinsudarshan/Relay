import React from 'react';
import {
  X,
  ExternalLink,
  Link as LinkIcon,
  GitMerge,
  Trash2,
  Network,
  Calendar,
  Layers,
  Sparkles,
} from 'lucide-react';
import { Button } from '@/components/ui/button';
import { Badge } from '@/components/ui/badge';
import { SimNode } from './graphTypes';

interface GraphNodeInspectorProps {
  selectedNode: SimNode | null;
  neighbors: SimNode[];
  onClose: () => void;
  onSelectNode: (id: string) => void;
  onOpenScribbleEditor?: (id: string) => void;
  onConnectScribble?: (scribbleId: string) => void;
  onMergeScribbles?: (scribbleId: string) => void;
  onDeleteScribble?: (scribbleId: string) => void;
  onExploreLocalGraph?: (nodeId: string) => void;
}

export const GraphNodeInspector: React.FC<GraphNodeInspectorProps> = ({
  selectedNode,
  neighbors,
  onClose,
  onSelectNode,
  onOpenScribbleEditor,
  onConnectScribble,
  onMergeScribbles,
  onDeleteScribble,
  onExploreLocalGraph,
}) => {
  if (!selectedNode) return null;

  const isScribble = selectedNode.node_type === 'scribble';
  const isTopicOrEntity = selectedNode.node_type === 'topic' || selectedNode.node_type === 'entity';
  const isVoice = selectedNode.node_type === 'voice_note' || selectedNode.node_type === 'source';

  const formattedDate = selectedNode.metadata?.created_at
    ? new Date(selectedNode.metadata.created_at).toLocaleDateString(undefined, {
        month: 'short',
        day: 'numeric',
        year: 'numeric',
      })
    : null;

  return (
    <div className="absolute bottom-4 left-4 w-84 max-h-[380px] bg-card/95 backdrop-blur-xl border border-border rounded-lg shadow-2xl p-4 overflow-y-auto space-y-3 animate-in fade-in duration-150 z-20 font-sans text-xs">
      {/* Header with Type Badge & Title */}
      <div className="flex items-start justify-between gap-2">
        <div className="flex-1 min-w-0">
          <Badge
            variant="outline"
            className="text-[9px] font-mono uppercase px-1.5 py-0 mb-1"
            style={{
              color: selectedNode.color,
              borderColor: `${selectedNode.color}60`,
              backgroundColor: `${selectedNode.color}15`,
            }}
          >
            {selectedNode.node_type}
          </Badge>
          <h4 className="text-xs font-bold text-foreground truncate">{selectedNode.label}</h4>
        </div>

        <button
          onClick={onClose}
          className="text-muted-foreground hover:text-foreground p-1 rounded-md hover:bg-muted/40 transition-colors shrink-0"
          title="Close Inspector"
        >
          <X className="w-3.5 h-3.5" />
        </button>
      </div>

      {/* Date metadata */}
      {formattedDate && (
        <div className="flex items-center gap-1.5 text-[10px] text-muted-foreground">
          <Calendar className="w-3 h-3" />
          <span>Created {formattedDate}</span>
        </div>
      )}

      {/* Summary / Snippet */}
      {selectedNode.summary && (
        <p className="text-[11px] text-muted-foreground leading-relaxed line-clamp-3 bg-muted/20 p-2 rounded-md border border-border/40">
          "{selectedNode.summary}"
        </p>
      )}

      {/* 1-Hop Neighbors List */}
      {neighbors.length > 0 && (
        <div className="space-y-1.5 pt-1 border-t border-border/50">
          <div className="flex items-center justify-between text-[10px] font-bold font-mono text-muted-foreground uppercase">
            <span>Connections ({neighbors.length})</span>
            {onExploreLocalGraph && (
              <button
                onClick={() => onExploreLocalGraph(selectedNode.id)}
                className="text-primary hover:underline flex items-center gap-1 normal-case font-normal"
              >
                <Network className="w-2.5 h-2.5" />
                <span>Local Graph</span>
              </button>
            )}
          </div>

          <div className="flex flex-wrap gap-1 max-h-24 overflow-y-auto p-0.5">
            {neighbors.map((nb) => (
              <button
                key={nb.id}
                onClick={() => onSelectNode(nb.id)}
                className="text-[10px] px-2 py-0.5 rounded-md bg-muted/40 hover:bg-muted/80 text-muted-foreground hover:text-foreground flex items-center gap-1.5 border border-border/40 transition-colors"
                title={nb.label}
              >
                <span className="w-1.5 h-1.5 rounded-full shrink-0" style={{ backgroundColor: nb.color }} />
                <span className="truncate max-w-[130px]">{nb.label}</span>
              </button>
            ))}
          </div>
        </div>
      )}

      {/* Contextual Action Buttons */}
      <div className="pt-2 border-t border-border/50 space-y-1.5">
        {isScribble && onOpenScribbleEditor && (
          <Button
            size="sm"
            variant="outline"
            onClick={() => onOpenScribbleEditor(selectedNode.id)}
            className="w-full h-7 text-xs gap-1.5 bg-background font-semibold hover:bg-muted/40"
          >
            <ExternalLink className="w-3 h-3 text-primary" />
            <span>Open in Editor</span>
          </Button>
        )}

        {isScribble && onConnectScribble && onMergeScribbles && (
          <div className="flex items-center gap-1.5">
            <Button
              size="sm"
              variant="outline"
              onClick={() => onConnectScribble(selectedNode.id)}
              className="flex-1 h-6 text-[11px] gap-1 bg-muted/20 hover:bg-muted/40"
            >
              <LinkIcon className="w-3 h-3 text-blue-400" />
              <span>Connect</span>
            </Button>

            <Button
              size="sm"
              variant="outline"
              onClick={() => onMergeScribbles(selectedNode.id)}
              className="flex-1 h-6 text-[11px] gap-1 bg-muted/20 hover:bg-muted/40"
            >
              <GitMerge className="w-3 h-3 text-amber-400" />
              <span>Merge</span>
            </Button>

            {onDeleteScribble && (
              <Button
                size="icon"
                variant="outline"
                onClick={() => onDeleteScribble(selectedNode.id)}
                className="h-6 w-6 bg-muted/20 text-muted-foreground hover:text-destructive hover:bg-destructive/10 shrink-0"
                title="Move to Trash"
              >
                <Trash2 className="w-3 h-3" />
              </Button>
            )}
          </div>
        )}

        {isTopicOrEntity && onExploreLocalGraph && (
          <Button
            size="sm"
            variant="outline"
            onClick={() => onExploreLocalGraph(selectedNode.id)}
            className="w-full h-7 text-xs gap-1.5 bg-muted/20 hover:bg-muted/40 font-semibold"
          >
            <Network className="w-3 h-3 text-amber-400" />
            <span>Explore Local Graph</span>
          </Button>
        )}
      </div>
    </div>
  );
};
