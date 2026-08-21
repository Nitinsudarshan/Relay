import React, { useEffect, useRef, useState, useMemo, useCallback } from 'react';
import {
  KnowledgeGraphData,
  KnowledgeNode,
  KnowledgeEdge,
  GraphFiltersSettings,
  GraphGroup,
  GraphDisplaySettings,
  GraphForcesSettings,
} from '../../types';
import {
  Search,
  SlidersHorizontal,
  X,
  Play,
  RotateCcw,
  ChevronDown,
  ChevronRight,
  Plus,
  ExternalLink,
  Save,
  Check,
} from 'lucide-react';
import { Button } from '@/components/ui/button';
import { Badge } from '@/components/ui/badge';

interface KnowledgeGraphViewProps {
  graphData: KnowledgeGraphData;
  onSelectScribble?: (id: string) => void;
  onOpenScribbleEditor?: (id: string) => void;
  isLoading?: boolean;
}

interface SimNode extends KnowledgeNode {
  x: number;
  y: number;
  vx: number;
  vy: number;
  radius: number;
  color: string;
  isPinned?: boolean;
}

const COLOR_MAP: Record<string, string> = {
  scribble: '#3b82f6', // Electric blue
  topic: '#f59e0b',    // Warm amber
  entity: '#10b981',   // Emerald
  source: '#8b5cf6',   // Violet
  voice_note: '#ec4899', // Pink
  project: '#06b6d4',  // Cyan
  default: '#94a3b8',  // Slate
};

const PRESET_GROUP_COLORS = [
  '#ef4444', // Red
  '#f97316', // Orange
  '#eab308', // Yellow
  '#22c55e', // Green
  '#06b6d4', // Cyan
  '#3b82f6', // Blue
  '#a855f7', // Purple
  '#ec4899', // Pink
];

const DEFAULT_FILTERS: GraphFiltersSettings = {
  searchQuery: '',
  showTags: true, // Used as showTopics in UI
  showAttachments: true,
  existingFilesOnly: false,
  showOrphans: true,
};

const DEFAULT_DISPLAY: GraphDisplaySettings = {
  showArrows: false,
  textFadeThreshold: 0.6,
  nodeSizeMultiplier: 1.0,
  linkThickness: 1.0,
};

const DEFAULT_FORCES: GraphForcesSettings = {
  centerForce: 0.52,
  repelForce: 10.0,
  linkForce: 1.0,
  linkDistance: 120,
};

const STORAGE_KEY = 'relay_knowledge_graph_settings_v1';

export const KnowledgeGraphView: React.FC<KnowledgeGraphViewProps> = ({
  graphData,
  onSelectScribble,
  onOpenScribbleEditor,
  isLoading = false,
}) => {
  const canvasRef = useRef<HTMLCanvasElement | null>(null);
  const containerRef = useRef<HTMLDivElement | null>(null);

  // Full-screen state (Requirement 18)
  const [isFullScreen, setIsFullScreen] = useState(false);

  // Viewport transformation
  const [transform, setTransform] = useState({ x: 0, y: 0, k: 1 });
  const transformRef = useRef(transform);
  transformRef.current = transform;

  // Interaction states
  const [hoveredNodeId, setHoveredNodeId] = useState<string | null>(null);
  const [selectedNodeId, setSelectedNodeId] = useState<string | null>(null);

  // Obsidian Graph Settings (Drawer minimized/closed by default)
  const [showSettingsDrawer, setShowSettingsDrawer] = useState(false);
  const [collapsedSections, setCollapsedSections] = useState<Record<string, boolean>>({
    filters: true,
    groups: true,
    display: true,
    forces: true,
  });

  // Load persistent settings (Requirement 17)
  const [filters, setFilters] = useState<GraphFiltersSettings>(() => {
    try {
      const saved = localStorage.getItem(STORAGE_KEY);
      if (saved) {
        const parsed = JSON.parse(saved);
        if (parsed.filters) return parsed.filters;
      }
    } catch {}
    return DEFAULT_FILTERS;
  });

  const [groups, setGroups] = useState<GraphGroup[]>(() => {
    try {
      const saved = localStorage.getItem(STORAGE_KEY);
      if (saved) {
        const parsed = JSON.parse(saved);
        if (parsed.groups) return parsed.groups;
      }
    } catch {}
    return [];
  });

  const [display, setDisplay] = useState<GraphDisplaySettings>(() => {
    try {
      const saved = localStorage.getItem(STORAGE_KEY);
      if (saved) {
        const parsed = JSON.parse(saved);
        if (parsed.display) return parsed.display;
      }
    } catch {}
    return DEFAULT_DISPLAY;
  });

  const [forces, setForces] = useState<GraphForcesSettings>(() => {
    try {
      const saved = localStorage.getItem(STORAGE_KEY);
      if (saved) {
        const parsed = JSON.parse(saved);
        if (parsed.forces) return parsed.forces;
      }
    } catch {}
    return DEFAULT_FORCES;
  });

  const [saveFeedback, setSaveFeedback] = useState(false);

  // Simulation nodes state stored in ref for 60fps animation
  const simNodesRef = useRef<Map<string, SimNode>>(new Map());
  const animationFrameRef = useRef<number | null>(null);
  const draggingNodeRef = useRef<{ node: SimNode; startX: number; startY: number } | null>(null);
  const isPanningRef = useRef(false);
  const panStartRef = useRef({ x: 0, y: 0 });
  const energyRef = useRef(1.0);

  const toggleSection = (section: string) => {
    setCollapsedSections((prev) => ({ ...prev, [section]: !prev[section] }));
  };

  const reheatSimulation = useCallback(() => {
    energyRef.current = 1.0;
  }, []);

  // Save Settings explicitly (Requirement 17)
  const handleSaveSettings = () => {
    try {
      localStorage.setItem(
        STORAGE_KEY,
        JSON.stringify({ filters, groups, display, forces })
      );
      setSaveFeedback(true);
      setTimeout(() => setSaveFeedback(false), 2000);
    } catch (e) {
      console.error('Failed to save graph settings:', e);
    }
  };

  // Calculate connected edges & neighbors map
  const { adjacencyMap, edgeList } = useMemo(() => {
    const adj = new Map<string, Set<string>>();
    const edges: KnowledgeEdge[] = [];

    for (const edge of graphData.edges) {
      if (!adj.has(edge.source_id)) adj.set(edge.source_id, new Set());
      if (!adj.has(edge.target_id)) adj.set(edge.target_id, new Set());

      adj.get(edge.source_id)!.add(edge.target_id);
      adj.get(edge.target_id)!.add(edge.source_id);
      edges.push(edge);
    }

    return { adjacencyMap: adj, edgeList: edges };
  }, [graphData]);

  // Filtered nodes according to Obsidian Filter Toggles (Topics instead of Tags per Requirement 20)
  const filteredNodes = useMemo(() => {
    return graphData.nodes.filter((node) => {
      // Topics toggle (replaces Tags)
      if (!filters.showTags && node.node_type === 'topic') return false;

      // Attachments toggle
      if (!filters.showAttachments && node.node_type === 'source') return false;

      // Existing files only (exclude pure entity/external nodes)
      if (filters.existingFilesOnly && node.node_type !== 'scribble') return false;

      // Orphans toggle (if false, hide unconnected nodes)
      if (!filters.showOrphans && (node.degree || 0) === 0) return false;

      // Search query
      if (filters.searchQuery.trim()) {
        const q = filters.searchQuery.toLowerCase();
        const matchesLabel = node.label.toLowerCase().includes(q);
        const matchesSummary = (node.summary || '').toLowerCase().includes(q);
        return matchesLabel || matchesSummary;
      }

      return true;
    });
  }, [graphData.nodes, filters]);

  // Synchronize Simulation Nodes with Filtered Nodes & Group Colors
  useEffect(() => {
    const existing = simNodesRef.current;
    const nextMap = new Map<string, SimNode>();
    const width = containerRef.current?.clientWidth || 800;
    const height = containerRef.current?.clientHeight || 600;

    filteredNodes.forEach((node) => {
      const prev = existing.get(node.id);
      const degree = node.degree || 0;
      const baseRadius = node.node_type === 'topic' ? 7 : node.node_type === 'entity' ? 5 : 6;
      const radius = (baseRadius + Math.min(degree * 1.5, 14)) * display.nodeSizeMultiplier;

      // Determine color (check groups first, fallback to node_type color)
      let color = COLOR_MAP[node.node_type] || COLOR_MAP.default;
      for (const grp of groups) {
        if (grp.query.trim()) {
          const q = grp.query.toLowerCase();
          if (
            node.label.toLowerCase().includes(q) ||
            (node.summary || '').toLowerCase().includes(q) ||
            (node.metadata?.topics && JSON.stringify(node.metadata.topics).toLowerCase().includes(q))
          ) {
            color = grp.color;
            break;
          }
        }
      }

      if (prev) {
        nextMap.set(node.id, {
          ...node,
          x: prev.x,
          y: prev.y,
          vx: prev.vx,
          vy: prev.vy,
          radius,
          color,
          isPinned: prev.isPinned,
        });
      } else {
        const angle = Math.random() * Math.PI * 2;
        const dist = 40 + Math.random() * Math.min(width, height) * 0.35;
        nextMap.set(node.id, {
          ...node,
          x: Math.cos(angle) * dist,
          y: Math.sin(angle) * dist,
          vx: (Math.random() - 0.5) * 2,
          vy: (Math.random() - 0.5) * 2,
          radius,
          color,
        });
      }
    });

    simNodesRef.current = nextMap;
    reheatSimulation();
  }, [filteredNodes, groups, display.nodeSizeMultiplier, reheatSimulation]);

  // 60FPS Force Simulation & Canvas Render Loop with Dynamic Text Fade (Requirement 15)
  useEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas) return;
    const ctx = canvas.getContext('2d');
    if (!ctx) return;

    let isRunning = true;

    const render = () => {
      if (!isRunning) return;

      const width = canvas.width;
      const height = canvas.height;
      const nodes = Array.from(simNodesRef.current.values());
      const nodeMap = simNodesRef.current;

      // 1. Force Simulation Physics
      if (energyRef.current > 0.005) {
        const alpha = energyRef.current;
        const repelForceEffective = forces.repelForce * 35;
        const centerForceEffective = forces.centerForce * 0.01;
        const linkForceEffective = forces.linkForce * 0.06;

        // A. Repulsion (Coulomb)
        for (let i = 0; i < nodes.length; i++) {
          for (let j = i + 1; j < nodes.length; j++) {
            const a = nodes[i];
            const b = nodes[j];
            let dx = b.x - a.x;
            let dy = b.y - a.y;
            let dist = Math.sqrt(dx * dx + dy * dy) || 1;

            if (dist < 600) {
              const force = (repelForceEffective / (dist * dist)) * alpha;
              const fx = (dx / dist) * force;
              const fy = (dy / dist) * force;

              if (!a.isPinned) { a.vx -= fx; a.vy -= fy; }
              if (!b.isPinned) { b.vx += fx; b.vy += fy; }
            }
          }
        }

        // B. Spring Links (Hooke)
        for (const edge of edgeList) {
          const source = nodeMap.get(edge.source_id);
          const target = nodeMap.get(edge.target_id);
          if (source && target) {
            let dx = target.x - source.x;
            let dy = target.y - source.y;
            let dist = Math.sqrt(dx * dx + dy * dy) || 1;
            let diff = dist - forces.linkDistance;
            let force = diff * linkForceEffective * alpha;

            let fx = (dx / dist) * force;
            let fy = (dy / dist) * force;

            if (!source.isPinned) { source.vx += fx; source.vy += fy; }
            if (!target.isPinned) { target.vx -= fx; target.vy -= fy; }
          }
        }

        // C. Center Gravity & Velocity Damping
        for (const node of nodes) {
          if (!node.isPinned) {
            node.vx += -node.x * centerForceEffective * alpha;
            node.vy += -node.y * centerForceEffective * alpha;

            node.vx *= 0.88;
            node.vy *= 0.88;

            node.x += node.vx;
            node.y += node.vy;
          }
        }

        energyRef.current *= 0.985;
      }

      // 2. Clear & Draw Canvas (Obsidian dark minimal background #0f1117)
      ctx.clearRect(0, 0, width, height);
      ctx.fillStyle = '#0f1117';
      ctx.fillRect(0, 0, width, height);

      ctx.save();
      const currentTransform = transformRef.current;
      ctx.translate(width / 2 + currentTransform.x, height / 2 + currentTransform.y);
      ctx.scale(currentTransform.k, currentTransform.k);

      // Determine 1-hop highlight neighborhood
      const activeId = hoveredNodeId || selectedNodeId;
      const highlightSet = new Set<string>();
      if (activeId) {
        highlightSet.add(activeId);
        const neighbors = adjacencyMap.get(activeId);
        if (neighbors) {
          neighbors.forEach((n) => highlightSet.add(n));
        }
      }

      // 3. Draw Edges (Subtle low-contrast lines with optional arrows)
      for (const edge of edgeList) {
        const source = nodeMap.get(edge.source_id);
        const target = nodeMap.get(edge.target_id);
        if (!source || !target) continue;

        const isHighlighted =
          highlightSet.size > 0 &&
          highlightSet.has(edge.source_id) &&
          highlightSet.has(edge.target_id);
        const isDimmed = highlightSet.size > 0 && !isHighlighted;

        ctx.beginPath();
        ctx.moveTo(source.x, source.y);
        ctx.lineTo(target.x, target.y);

        if (isHighlighted) {
          ctx.strokeStyle = '#60a5fa';
          ctx.lineWidth = 1.8 * display.linkThickness;
          ctx.globalAlpha = 0.9;
        } else if (isDimmed) {
          ctx.strokeStyle = '#334155';
          ctx.lineWidth = 0.6 * display.linkThickness;
          ctx.globalAlpha = 0.12;
        } else {
          ctx.strokeStyle = '#475569';
          ctx.lineWidth = 0.8 * display.linkThickness;
          ctx.globalAlpha = 0.35;
        }

        ctx.stroke();

        // Directional Arrows (Display setting)
        if (display.showArrows && !isDimmed) {
          const dx = target.x - source.x;
          const dy = target.y - source.y;
          const angle = Math.atan2(dy, dx);
          const arrowDist = target.radius + 6;
          const arrowX = target.x - Math.cos(angle) * arrowDist;
          const arrowY = target.y - Math.sin(angle) * arrowDist;

          ctx.beginPath();
          ctx.moveTo(arrowX, arrowY);
          ctx.lineTo(
            arrowX - 6 * Math.cos(angle - Math.PI / 6),
            arrowY - 6 * Math.sin(angle - Math.PI / 6)
          );
          ctx.lineTo(
            arrowX - 6 * Math.cos(angle + Math.PI / 6),
            arrowY - 6 * Math.sin(angle + Math.PI / 6)
          );
          ctx.fillStyle = ctx.strokeStyle;
          ctx.fill();
        }
      }

      ctx.globalAlpha = 1.0;

      // 4. Draw Nodes (Circular, degree-scaled)
      for (const node of nodes) {
        const isSelected = node.id === selectedNodeId;
        const isHovered = node.id === hoveredNodeId;
        const isHighlighted = highlightSet.size > 0 && highlightSet.has(node.id);
        const isDimmed = highlightSet.size > 0 && !isHighlighted;

        ctx.save();
        ctx.beginPath();
        ctx.arc(node.x, node.y, node.radius, 0, Math.PI * 2);

        if (isSelected) {
          ctx.fillStyle = '#ffffff';
          ctx.shadowColor = node.color;
          ctx.shadowBlur = 16;
        } else if (isHighlighted) {
          ctx.fillStyle = node.color;
          ctx.shadowColor = node.color;
          ctx.shadowBlur = 10;
        } else if (isDimmed) {
          ctx.fillStyle = node.color;
          ctx.globalAlpha = 0.18;
        } else {
          ctx.fillStyle = node.color;
          ctx.globalAlpha = 0.88;
        }

        ctx.fill();

        // Node outline border
        ctx.lineWidth = isSelected ? 2.5 : isHovered ? 2 : 1;
        ctx.strokeStyle = isSelected ? node.color : 'rgba(255,255,255,0.18)';
        ctx.stroke();

        ctx.restore();

        // 5. Dynamic Text Fade Threshold Calculation (Requirement 15)
        let textOpacity = 1.0;
        if (isSelected || isHovered || isHighlighted) {
          textOpacity = 1.0;
        } else if (display.textFadeThreshold <= 0.05) {
          textOpacity = 1.0;
        } else {
          // If zoom level (k) is below threshold, smoothly fade out
          const factor = (currentTransform.k - (display.textFadeThreshold * 0.4)) / (display.textFadeThreshold * 0.6 || 0.1);
          textOpacity = Math.max(0, Math.min(1, factor));
        }

        if (textOpacity > 0.05 && !isDimmed) {
          ctx.save();
          ctx.globalAlpha = textOpacity;
          ctx.font = `${Math.max(10, 11 / Math.sqrt(currentTransform.k))}px system-ui, sans-serif`;
          ctx.textAlign = 'center';
          ctx.textBaseline = 'top';

          ctx.fillStyle = isSelected ? '#ffffff' : isHighlighted ? '#e2e8f0' : '#94a3b8';
          ctx.fillText(node.label, node.x, node.y + node.radius + 4);
          ctx.restore();
        }
      }

      ctx.restore();
      animationFrameRef.current = requestAnimationFrame(render);
    };

    animationFrameRef.current = requestAnimationFrame(render);

    return () => {
      isRunning = false;
      if (animationFrameRef.current) {
        cancelAnimationFrame(animationFrameRef.current);
      }
    };
  }, [edgeList, adjacencyMap, hoveredNodeId, selectedNodeId, display, forces]);

  // Handle Resize
  useEffect(() => {
    const handleResize = () => {
      if (containerRef.current && canvasRef.current) {
        canvasRef.current.width = containerRef.current.clientWidth;
        canvasRef.current.height = containerRef.current.clientHeight;
        reheatSimulation();
      }
    };

    handleResize();
    window.addEventListener('resize', handleResize);
    return () => window.removeEventListener('resize', handleResize);
  }, [reheatSimulation]);

  // Hit test helper
  const getNodeAtCoords = useCallback(
    (clientX: number, clientY: number): SimNode | null => {
      const canvas = canvasRef.current;
      if (!canvas) return null;
      const rect = canvas.getBoundingClientRect();
      const canvasX = clientX - rect.left;
      const canvasY = clientY - rect.top;

      const currentTransform = transformRef.current;
      const worldX = (canvasX - canvas.width / 2 - currentTransform.x) / currentTransform.k;
      const worldY = (canvasY - canvas.height / 2 - currentTransform.y) / currentTransform.k;

      const nodes = Array.from(simNodesRef.current.values());
      for (let i = nodes.length - 1; i >= 0; i--) {
        const node = nodes[i];
        const dx = worldX - node.x;
        const dy = worldY - node.y;
        const hitRadius = Math.max(node.radius + 4, 12);
        if (dx * dx + dy * dy <= hitRadius * hitRadius) {
          return node;
        }
      }
      return null;
    },
    []
  );

  // Mouse Handlers (Pan, Zoom, Drag, Select)
  const handleMouseDown = (e: React.MouseEvent<HTMLCanvasElement>) => {
    const hitNode = getNodeAtCoords(e.clientX, e.clientY);
    if (hitNode) {
      draggingNodeRef.current = { node: hitNode, startX: hitNode.x, startY: hitNode.y };
      hitNode.isPinned = true;
      reheatSimulation();
    } else {
      isPanningRef.current = true;
      panStartRef.current = { x: e.clientX - transform.x, y: e.clientY - transform.y };
    }
  };

  const handleMouseMove = (e: React.MouseEvent<HTMLCanvasElement>) => {
    if (draggingNodeRef.current) {
      const canvas = canvasRef.current;
      if (!canvas) return;
      const rect = canvas.getBoundingClientRect();
      const canvasX = e.clientX - rect.left;
      const canvasY = e.clientY - rect.top;

      const currentTransform = transformRef.current;
      const worldX = (canvasX - canvas.width / 2 - currentTransform.x) / currentTransform.k;
      const worldY = (canvasY - canvas.height / 2 - currentTransform.y) / currentTransform.k;

      draggingNodeRef.current.node.x = worldX;
      draggingNodeRef.current.node.y = worldY;
      draggingNodeRef.current.node.vx = 0;
      draggingNodeRef.current.node.vy = 0;
      reheatSimulation();
    } else if (isPanningRef.current) {
      setTransform((prev) => ({
        ...prev,
        x: e.clientX - panStartRef.current.x,
        y: e.clientY - panStartRef.current.y,
      }));
    } else {
      const hitNode = getNodeAtCoords(e.clientX, e.clientY);
      setHoveredNodeId(hitNode ? hitNode.id : null);
    }
  };

  const handleMouseUp = () => {
    if (draggingNodeRef.current) {
      draggingNodeRef.current.node.isPinned = false;
      draggingNodeRef.current = null;
    }
    isPanningRef.current = false;
  };

  const handleClick = (e: React.MouseEvent<HTMLCanvasElement>) => {
    const hitNode = getNodeAtCoords(e.clientX, e.clientY);
    if (hitNode) {
      setSelectedNodeId(hitNode.id);
      if (onSelectScribble && hitNode.node_type === 'scribble') {
        onSelectScribble(hitNode.id);
      }
    } else {
      setSelectedNodeId(null);
    }
  };

  const handleDoubleClick = (e: React.MouseEvent<HTMLCanvasElement>) => {
    const hitNode = getNodeAtCoords(e.clientX, e.clientY);
    if (hitNode && onOpenScribbleEditor && hitNode.node_type === 'scribble') {
      onOpenScribbleEditor(hitNode.id);
    }
  };

  const handleWheel = (e: React.WheelEvent<HTMLCanvasElement>) => {
    e.preventDefault();
    const zoomFactor = e.deltaY < 0 ? 1.15 : 0.87;
    setTransform((prev) => {
      const newK = Math.max(0.15, Math.min(prev.k * zoomFactor, 5.0));
      return { ...prev, k: newK };
    });
  };

  const handleResetZoom = () => {
    setTransform({ x: 0, y: 0, k: 1 });
    reheatSimulation();
  };

  const handleAddGroup = () => {
    const nextColor = PRESET_GROUP_COLORS[groups.length % PRESET_GROUP_COLORS.length];
    const newGroup: GraphGroup = {
      id: `grp_${Date.now()}`,
      query: '',
      color: nextColor,
    };
    setGroups([...groups, newGroup]);
  };

  const handleUpdateGroup = (id: string, updates: Partial<GraphGroup>) => {
    setGroups(groups.map((g) => (g.id === id ? { ...g, ...updates } : g)));
  };

  const handleRemoveGroup = (id: string) => {
    setGroups(groups.filter((g) => g.id !== id));
  };

  const selectedNode = selectedNodeId ? simNodesRef.current.get(selectedNodeId) : null;
  const connectedNeighbors = useMemo(() => {
    if (!selectedNodeId) return [];
    const neighbors = adjacencyMap.get(selectedNodeId);
    if (!neighbors) return [];
    return Array.from(neighbors)
      .map((id) => simNodesRef.current.get(id))
      .filter((n): n is SimNode => n !== undefined);
  }, [selectedNodeId, adjacencyMap]);

  return (
    <div
      ref={containerRef}
      className="relative w-full h-full min-h-[500px] overflow-hidden select-none bg-[#0f1117] rounded-lg border border-border"
    >
      {/* 2D Force Simulation Canvas */}
      <canvas
        ref={canvasRef}
        onMouseDown={handleMouseDown}
        onMouseMove={handleMouseMove}
        onMouseUp={handleMouseUp}
        onMouseLeave={handleMouseUp}
        onClick={handleClick}
        onDoubleClick={handleDoubleClick}
        onWheel={handleWheel}
        className="w-full h-full cursor-grab active:cursor-grabbing block"
      />

      {/* Top Floating Controls Toggle & Actions */}
      <div className="absolute top-4 right-4 flex items-center gap-1.5 z-20">
        <Button
          size="sm"
          variant="outline"
          onClick={() => setShowSettingsDrawer(!showSettingsDrawer)}
          className={`h-8 text-xs gap-1.5 bg-[#1a1d27]/90 backdrop-blur-md border-border/60 shadow-lg ${
            showSettingsDrawer ? 'border-primary text-primary font-bold' : 'text-foreground'
          }`}
        >
          <SlidersHorizontal className="w-3.5 h-3.5" />
          <span>Graph Settings</span>
        </Button>

        <Button
          size="icon"
          variant="outline"
          onClick={handleResetZoom}
          className="h-8 w-8 bg-[#1a1d27]/90 backdrop-blur-md border-border/60 shadow-lg text-muted-foreground hover:text-foreground"
          title="Reset Zoom & Pan"
        >
          <RotateCcw className="w-3.5 h-3.5" />
        </Button>
      </div>

      {/* Obsidian-Style Collapsible Graph Settings Drawer (Top Right) */}
      {showSettingsDrawer && (
        <div className="absolute top-16 right-4 w-72 max-h-[calc(100%-80px)] overflow-y-auto bg-[#1a1d27]/95 backdrop-blur-xl border border-border/80 rounded-lg shadow-2xl p-3.5 space-y-3 z-30 animate-in fade-in duration-150 font-sans text-xs">
          {/* Header */}
          <div className="flex items-center justify-between pb-1.5 border-b border-border/50">
            <span className="font-bold text-foreground">Graph Settings</span>
            <button
              onClick={() => setShowSettingsDrawer(false)}
              className="text-muted-foreground hover:text-foreground p-1 rounded"
            >
              <X className="w-3.5 h-3.5" />
            </button>
          </div>

          {/* 1. FILTERS SECTION (Obsidian Structure - Topics instead of Tags per Req 20) */}
          <div className="border border-border/60 rounded-lg overflow-hidden bg-card/40">
            <button
              type="button"
              onClick={() => toggleSection('filters')}
              className="w-full p-2.5 flex items-center justify-between font-semibold text-foreground hover:bg-muted/30 transition-all text-left"
            >
              <span>Filters</span>
              {collapsedSections.filters ? <ChevronRight className="w-3.5 h-3.5" /> : <ChevronDown className="w-3.5 h-3.5" />}
            </button>

            {!collapsedSections.filters && (
              <div className="p-3 border-t border-border/50 space-y-3">
                {/* Search files... */}
                <div className="relative">
                  <Search className="w-3.5 h-3.5 absolute left-2.5 top-2 text-muted-foreground" />
                  <input
                    type="text"
                    value={filters.searchQuery}
                    onChange={(e) => setFilters({ ...filters, searchQuery: e.target.value })}
                    placeholder="Search files…"
                    className="w-full pl-8 pr-2 py-1 text-xs bg-muted/30 border border-border/60 rounded-lg text-foreground placeholder:text-muted-foreground/60 focus:outline-none"
                  />
                </div>

                {/* Toggles */}
                <div className="space-y-2 text-[11px]">
                  <label className="flex items-center justify-between text-muted-foreground hover:text-foreground cursor-pointer">
                    <span>Topics</span>
                    <input
                      type="checkbox"
                      checked={filters.showTags}
                      onChange={(e) => setFilters({ ...filters, showTags: e.target.checked })}
                      className="rounded border-border text-primary focus:ring-0 cursor-pointer"
                    />
                  </label>

                  <label className="flex items-center justify-between text-muted-foreground hover:text-foreground cursor-pointer">
                    <span>Attachments</span>
                    <input
                      type="checkbox"
                      checked={filters.showAttachments}
                      onChange={(e) => setFilters({ ...filters, showAttachments: e.target.checked })}
                      className="rounded border-border text-primary focus:ring-0 cursor-pointer"
                    />
                  </label>

                  <label className="flex items-center justify-between text-muted-foreground hover:text-foreground cursor-pointer">
                    <span>Existing Scribbles only</span>
                    <input
                      type="checkbox"
                      checked={filters.existingFilesOnly}
                      onChange={(e) => setFilters({ ...filters, existingFilesOnly: e.target.checked })}
                      className="rounded border-border text-primary focus:ring-0 cursor-pointer"
                    />
                  </label>

                  <label className="flex items-center justify-between text-muted-foreground hover:text-foreground cursor-pointer">
                    <span>Orphans</span>
                    <input
                      type="checkbox"
                      checked={filters.showOrphans}
                      onChange={(e) => setFilters({ ...filters, showOrphans: e.target.checked })}
                      className="rounded border-border text-primary focus:ring-0 cursor-pointer"
                    />
                  </label>
                </div>
              </div>
            )}
          </div>

          {/* 2. GROUPS SECTION (Obsidian Structure) */}
          <div className="border border-border/60 rounded-lg overflow-hidden bg-card/40">
            <button
              type="button"
              onClick={() => toggleSection('groups')}
              className="w-full p-2.5 flex items-center justify-between font-semibold text-foreground hover:bg-muted/30 transition-all text-left"
            >
              <span>Groups ({groups.length})</span>
              {collapsedSections.groups ? <ChevronRight className="w-3.5 h-3.5" /> : <ChevronDown className="w-3.5 h-3.5" />}
            </button>

            {!collapsedSections.groups && (
              <div className="p-3 border-t border-border/50 space-y-2.5">
                {groups.map((grp) => (
                  <div key={grp.id} className="flex items-center gap-1.5">
                    <input
                      type="text"
                      value={grp.query}
                      onChange={(e) => handleUpdateGroup(grp.id, { query: e.target.value })}
                      placeholder="Enter query…"
                      className="flex-1 px-2 py-1 text-xs bg-muted/30 border border-border/60 rounded-lg text-foreground placeholder:text-muted-foreground/60 focus:outline-none"
                    />
                    <input
                      type="color"
                      value={grp.color}
                      onChange={(e) => handleUpdateGroup(grp.id, { color: e.target.value })}
                      className="w-6 h-6 rounded-lg cursor-pointer border-0 bg-transparent shrink-0"
                      title="Group color"
                    />
                    <button
                      type="button"
                      onClick={() => handleRemoveGroup(grp.id)}
                      className="text-muted-foreground hover:text-destructive p-1 rounded-lg"
                    >
                      <X className="w-3.5 h-3.5" />
                    </button>
                  </div>
                ))}

                <Button
                  size="sm"
                  variant="outline"
                  onClick={handleAddGroup}
                  className="w-full h-7 text-[11px] gap-1 bg-muted/20"
                >
                  <Plus className="w-3.5 h-3.5" />
                  <span>New group</span>
                </Button>
              </div>
            )}
          </div>

          {/* 3. DISPLAY SECTION (Obsidian Structure) */}
          <div className="border border-border/60 rounded-lg overflow-hidden bg-card/40">
            <button
              type="button"
              onClick={() => toggleSection('display')}
              className="w-full p-2.5 flex items-center justify-between font-semibold text-foreground hover:bg-muted/30 transition-all text-left"
            >
              <span>Display</span>
              {collapsedSections.display ? <ChevronRight className="w-3.5 h-3.5" /> : <ChevronDown className="w-3.5 h-3.5" />}
            </button>

            {!collapsedSections.display && (
              <div className="p-3 border-t border-border/50 space-y-3 text-[11px]">
                {/* Arrows Toggle */}
                <label className="flex items-center justify-between text-muted-foreground hover:text-foreground cursor-pointer">
                  <span>Arrows</span>
                  <input
                    type="checkbox"
                    checked={display.showArrows}
                    onChange={(e) => setDisplay({ ...display, showArrows: e.target.checked })}
                    className="rounded-lg border-border text-primary focus:ring-0 cursor-pointer"
                  />
                </label>

                {/* Text fade threshold */}
                <div className="space-y-1">
                  <div className="flex items-center justify-between text-muted-foreground">
                    <span>Text fade threshold</span>
                    <span className="font-mono">{display.textFadeThreshold.toFixed(2)}</span>
                  </div>
                  <input
                    type="range"
                    min="0.00"
                    max="2.00"
                    step="0.05"
                    value={display.textFadeThreshold}
                    onChange={(e) => setDisplay({ ...display, textFadeThreshold: parseFloat(e.target.value) })}
                    className="w-full h-1 bg-muted rounded-lg appearance-none cursor-pointer"
                  />
                </div>

                {/* Node size */}
                <div className="space-y-1">
                  <div className="flex items-center justify-between text-muted-foreground">
                    <span>Node size</span>
                    <span className="font-mono">{display.nodeSizeMultiplier.toFixed(2)}</span>
                  </div>
                  <input
                    type="range"
                    min="0.50"
                    max="3.00"
                    step="0.10"
                    value={display.nodeSizeMultiplier}
                    onChange={(e) => setDisplay({ ...display, nodeSizeMultiplier: parseFloat(e.target.value) })}
                    className="w-full h-1 bg-muted rounded-lg appearance-none cursor-pointer"
                  />
                </div>

                {/* Link thickness */}
                <div className="space-y-1">
                  <div className="flex items-center justify-between text-muted-foreground">
                    <span>Link thickness</span>
                    <span className="font-mono">{display.linkThickness.toFixed(2)}</span>
                  </div>
                  <input
                    type="range"
                    min="0.50"
                    max="3.00"
                    step="0.10"
                    value={display.linkThickness}
                    onChange={(e) => setDisplay({ ...display, linkThickness: parseFloat(e.target.value) })}
                    className="w-full h-1 bg-muted rounded-lg appearance-none cursor-pointer"
                  />
                </div>

                {/* Animate Button */}
                <Button
                  size="sm"
                  variant="outline"
                  onClick={reheatSimulation}
                  className="w-full h-7 text-[11px] gap-1.5 bg-muted/20 mt-1"
                >
                  <Play className="w-3.5 h-3.5" />
                  <span>Animate</span>
                </Button>
              </div>
            )}
          </div>

          {/* 4. FORCES SECTION (Obsidian Structure) */}
          <div className="border border-border/60 rounded-lg overflow-hidden bg-card/40">
            <button
              type="button"
              onClick={() => toggleSection('forces')}
              className="w-full p-2.5 flex items-center justify-between font-semibold text-foreground hover:bg-muted/30 transition-all text-left"
            >
              <span>Forces</span>
              {collapsedSections.forces ? <ChevronRight className="w-3.5 h-3.5" /> : <ChevronDown className="w-3.5 h-3.5" />}
            </button>

            {!collapsedSections.forces && (
              <div className="p-3 border-t border-border/50 space-y-3 text-[11px]">
                {/* Center force */}
                <div className="space-y-1">
                  <div className="flex items-center justify-between text-muted-foreground">
                    <span>Center force</span>
                    <span className="font-mono">{forces.centerForce.toFixed(2)}</span>
                  </div>
                  <input
                    type="range"
                    min="0.00"
                    max="1.00"
                    step="0.02"
                    value={forces.centerForce}
                    onChange={(e) => {
                      setForces({ ...forces, centerForce: parseFloat(e.target.value) });
                      reheatSimulation();
                    }}
                    className="w-full h-1 bg-muted rounded-lg appearance-none cursor-pointer"
                  />
                </div>

                {/* Repel force */}
                <div className="space-y-1">
                  <div className="flex items-center justify-between text-muted-foreground">
                    <span>Repel force</span>
                    <span className="font-mono">{forces.repelForce.toFixed(2)}</span>
                  </div>
                  <input
                    type="range"
                    min="0.00"
                    max="20.00"
                    step="0.50"
                    value={forces.repelForce}
                    onChange={(e) => {
                      setForces({ ...forces, repelForce: parseFloat(e.target.value) });
                      reheatSimulation();
                    }}
                    className="w-full h-1 bg-muted rounded-lg appearance-none cursor-pointer"
                  />
                </div>

                {/* Link force */}
                <div className="space-y-1">
                  <div className="flex items-center justify-between text-muted-foreground">
                    <span>Link force</span>
                    <span className="font-mono">{forces.linkForce.toFixed(2)}</span>
                  </div>
                  <input
                    type="range"
                    min="0.00"
                    max="1.00"
                    step="0.02"
                    value={forces.linkForce}
                    onChange={(e) => {
                      setForces({ ...forces, linkForce: parseFloat(e.target.value) });
                      reheatSimulation();
                    }}
                    className="w-full h-1 bg-muted rounded-lg appearance-none cursor-pointer"
                  />
                </div>

                {/* Link distance */}
                <div className="space-y-1">
                  <div className="flex items-center justify-between text-muted-foreground">
                    <span>Link distance</span>
                    <span className="font-mono">{forces.linkDistance}</span>
                  </div>
                  <input
                    type="range"
                    min="30"
                    max="500"
                    step="10"
                    value={forces.linkDistance}
                    onChange={(e) => {
                      setForces({ ...forces, linkDistance: parseInt(e.target.value, 10) });
                      reheatSimulation();
                    }}
                    className="w-full h-1 bg-muted rounded-lg appearance-none cursor-pointer"
                  />
                </div>
              </div>
            )}
          </div>

          {/* Save Settings Action (Requirement 17) */}
          <div className="pt-2 border-t border-border/50 flex items-center justify-between">
            <Button
              size="sm"
              variant="outline"
              onClick={handleSaveSettings}
              className="w-full h-7 text-xs gap-1.5 bg-primary text-primary-foreground font-semibold"
            >
              {saveFeedback ? (
                <>
                  <Check className="w-3.5 h-3.5 text-white" />
                  <span>Settings Saved</span>
                </>
              ) : (
                <>
                  <Save className="w-3.5 h-3.5" />
                  <span>Save Graph Settings</span>
                </>
              )}
            </Button>
          </div>
        </div>
      )}

      {/* Selected Node Inspector (Floating Bottom Left) */}
      {selectedNode && (
        <div className="absolute bottom-4 left-4 w-80 max-h-72 bg-[#1a1d27]/95 backdrop-blur-xl border border-border/80 rounded-lg shadow-2xl p-4 overflow-y-auto space-y-3 animate-in fade-in duration-150 z-20">
          <div className="flex items-start justify-between gap-2">
            <div className="flex-1 min-w-0">
              <Badge
                variant="outline"
                className="text-[8px] font-mono uppercase px-1.5 py-0 mb-1"
                style={{ color: selectedNode.color, borderColor: `${selectedNode.color}50` }}
              >
                {selectedNode.node_type}
              </Badge>
              <h4 className="text-xs font-bold text-foreground truncate">{selectedNode.label}</h4>
            </div>

            <button
              onClick={() => setSelectedNodeId(null)}
              className="text-muted-foreground hover:text-foreground p-1 rounded-lg"
            >
              <X className="w-3.5 h-3.5" />
            </button>
          </div>

          {selectedNode.summary && (
            <p className="text-[11px] text-muted-foreground line-clamp-3 leading-relaxed">
              "{selectedNode.summary}"
            </p>
          )}

          {/* 1-Hop Neighbors List */}
          {connectedNeighbors.length > 0 && (
            <div className="space-y-1.5 pt-1 border-t border-border/50">
              <span className="text-[9px] font-bold font-mono text-muted-foreground uppercase">
                Connections ({connectedNeighbors.length})
              </span>
              <div className="flex flex-wrap gap-1 max-h-20 overflow-y-auto">
                {connectedNeighbors.map((nb) => (
                  <button
                    key={nb.id}
                    onClick={() => setSelectedNodeId(nb.id)}
                    className="text-[10px] px-2 py-0.5 rounded-lg bg-muted/60 text-muted-foreground hover:text-foreground flex items-center gap-1 border border-border/40 font-sans"
                  >
                    <span className="w-1.5 h-1.5 rounded-full" style={{ backgroundColor: nb.color }} />
                    <span className="truncate max-w-[120px]">{nb.label}</span>
                  </button>
                ))}
              </div>
            </div>
          )}

          {/* Open in Editor button */}
          {selectedNode.node_type === 'scribble' && onOpenScribbleEditor && (
            <Button
              size="sm"
              variant="outline"
              onClick={() => {
                onOpenScribbleEditor(selectedNode.id);
              }}
              className="w-full h-7 text-xs gap-1.5 bg-background font-semibold"
            >
              <span>Open in Editor</span>
              <ExternalLink className="w-3 h-3" />
            </Button>
          )}
        </div>
      )}
    </div>
  );
};
