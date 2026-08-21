import React from 'react';
import {
  SlidersHorizontal,
  RotateCcw,
  ZoomIn,
  ZoomOut,
  Network,
  Globe,
  Play,
  Pause,
  X,
} from 'lucide-react';
import { Button } from '@/components/ui/button';
import { Badge } from '@/components/ui/badge';
import { LocalGraphSettings } from '../../../types';

interface GraphToolbarProps {
  showSettingsDrawer: boolean;
  onToggleSettingsDrawer: () => void;
  localGraph: LocalGraphSettings;
  onToggleLocalGraph: () => void;
  onDepthChange: (depth: number) => void;
  onZoomIn: () => void;
  onZoomOut: () => void;
  onResetZoom: () => void;
  isTimeLapsePlaying: boolean;
  onToggleTimeLapse: () => void;
  nodeCount: number;
}

export const GraphToolbar: React.FC<GraphToolbarProps> = ({
  showSettingsDrawer,
  onToggleSettingsDrawer,
  localGraph,
  onToggleLocalGraph,
  onDepthChange,
  onZoomIn,
  onZoomOut,
  onResetZoom,
  isTimeLapsePlaying,
  onToggleTimeLapse,
  nodeCount,
}) => {
  return (
    <div className="absolute top-4 right-4 flex items-center gap-1.5 z-20 font-sans text-xs">
      {/* Node Count indicator */}
      <div className="hidden sm:flex items-center px-2 py-1 bg-card/90 backdrop-blur-md border border-border rounded-md text-[11px] font-mono text-muted-foreground shadow-lg">
        <span>{nodeCount} nodes</span>
      </div>

      {/* Local Graph Mode Pill */}
      {localGraph.enabled ? (
        <div className="flex items-center gap-1 bg-card/95 backdrop-blur-md border border-primary/50 rounded-md p-0.5 shadow-lg">
          <Badge variant="default" className="text-[10px] gap-1 h-6 bg-primary text-primary-foreground font-semibold px-2">
            <Network className="w-3 h-3" />
            <span>Local Graph</span>
          </Badge>

          {/* Depth Selector */}
          <div className="flex items-center gap-0.5 px-1">
            <span className="text-[10px] text-muted-foreground mr-1">Depth:</span>
            {[1, 2, 3].map((d) => (
              <button
                key={d}
                onClick={() => onDepthChange(d)}
                className={`w-5 h-5 rounded text-[10px] font-mono font-bold transition-colors ${
                  localGraph.depth === d
                    ? 'bg-primary text-primary-foreground'
                    : 'text-muted-foreground hover:text-foreground hover:bg-muted/40'
                }`}
              >
                {d}
              </button>
            ))}
          </div>

          <Button
            size="icon"
            variant="ghost"
            onClick={onToggleLocalGraph}
            className="h-6 w-6 text-muted-foreground hover:text-foreground"
            title="Exit Local Graph Mode"
          >
            <X className="w-3 h-3" />
          </Button>
        </div>
      ) : (
        <Button
          size="sm"
          variant="outline"
          onClick={onToggleLocalGraph}
          className="h-8 text-xs gap-1.5 bg-card/90 backdrop-blur-md border-border shadow-lg text-foreground hover:text-primary"
          title="Toggle Local Graph Mode"
        >
          <Network className="w-3.5 h-3.5" />
          <span className="hidden sm:inline">Local Graph</span>
        </Button>
      )}

      {/* Time-Lapse / Animate Button */}
      <Button
        size="sm"
        variant="outline"
        onClick={onToggleTimeLapse}
        className={`h-8 text-xs gap-1.5 bg-card/90 backdrop-blur-md border-border shadow-lg ${
          isTimeLapsePlaying ? 'text-primary border-primary animate-pulse' : 'text-foreground'
        }`}
        title={isTimeLapsePlaying ? 'Pause Time-lapse' : 'Play Time-lapse (Chronological Creation)'}
      >
        {isTimeLapsePlaying ? <Pause className="w-3.5 h-3.5" /> : <Play className="w-3.5 h-3.5" />}
        <span className="hidden sm:inline">Time-lapse</span>
      </Button>

      {/* Zoom In & Out */}
      <div className="flex items-center bg-card/90 backdrop-blur-md border border-border rounded-md shadow-lg overflow-hidden">
        <Button
          size="icon"
          variant="ghost"
          onClick={onZoomIn}
          className="h-8 w-7 rounded-none text-muted-foreground hover:text-foreground border-r border-border"
          title="Zoom In (+)"
        >
          <ZoomIn className="w-3.5 h-3.5" />
        </Button>

        <Button
          size="icon"
          variant="ghost"
          onClick={onZoomOut}
          className="h-8 w-7 rounded-none text-muted-foreground hover:text-foreground border-r border-border"
          title="Zoom Out (-)"
        >
          <ZoomOut className="w-3.5 h-3.5" />
        </Button>

        <Button
          size="icon"
          variant="ghost"
          onClick={onResetZoom}
          className="h-8 w-7 rounded-none text-muted-foreground hover:text-foreground"
          title="Reset Camera View (0)"
        >
          <RotateCcw className="w-3.5 h-3.5" />
        </Button>
      </div>

      {/* Graph Settings Toggle */}
      <Button
        size="sm"
        variant="outline"
        onClick={onToggleSettingsDrawer}
        className={`h-8 text-xs gap-1.5 bg-card/90 backdrop-blur-md border-border shadow-lg ${
          showSettingsDrawer ? 'border-primary text-primary font-bold' : 'text-foreground'
        }`}
      >
        <SlidersHorizontal className="w-3.5 h-3.5" />
        <span className="hidden md:inline">Settings</span>
      </Button>
    </div>
  );
};
