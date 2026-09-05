import React, { useEffect, useRef, useState, useMemo, useCallback } from 'react';
import { invoke } from '@tauri-apps/api/core';
import {
  KnowledgeGraphData,
  KnowledgeNode,
  KnowledgeEdge,
  Scribble,
  GraphFiltersSettings,
  GraphGroup,
  GraphDisplaySettings,
  GraphForcesSettings,
  LocalGraphSettings,
} from '@/types';
import {
  SimNode,
  CameraState,
  RELAY_COLOR_MAP,
  DEFAULT_LOCAL_GRAPH,
} from './graph/graphTypes';
import {
  loadGraphSettings,
  saveGraphSettings,
  loadNodePositions,
  saveNodePositions,
  clearNodePositions,
  StoredGraphSettings,
} from './graph/graphStorage';
import { stepSimulation } from './graph/graphPhysics';
import { renderGraph } from './graph/graphRenderer';
import { GraphSettingsPanel } from './graph/GraphSettingsPanel';
import { GraphNodeInspector } from './graph/GraphNodeInspector';
import { GraphToolbar } from './graph/GraphToolbar';
import { ConnectAndMergeModal } from '@/components/scribble/ConnectAndMergeModal';
import { ConfirmationModal } from '@/components/common/ConfirmationModal';

interface KnowledgeGraphViewProps {
  graphData: KnowledgeGraphData;
  allScribbles?: Scribble[];
  onSelectScribble?: (id: string) => void;
  onOpenScribbleEditor?: (id: string) => void;
  onScribbleUpdated?: (updated: Scribble) => void;
  onScribbleCreated?: (created: Scribble) => void;
  onScribbleDeleted?: (id: string) => void;
  isLoading?: boolean;
}

export const KnowledgeGraphView: React.FC<KnowledgeGraphViewProps> = ({
  graphData,
  allScribbles = [],
  onSelectScribble,
  onOpenScribbleEditor,
  onScribbleUpdated,
  onScribbleCreated,
  onScribbleDeleted,
  isLoading = false,
}) => {
  const containerRef = useRef<HTMLDivElement | null>(null);
  const canvasRef = useRef<HTMLCanvasElement | null>(null);

  // Settings state (Persisted in localStorage)
  const [storedSettings, setStoredSettings] = useState<StoredGraphSettings>(loadGraphSettings);
  const [saveFeedback, setSaveFeedback] = useState(false);

  const { filters, groups, display, forces } = storedSettings;

  const setFilters = (newFilters: GraphFiltersSettings) => {
    setStoredSettings((prev) => ({ ...prev, filters: newFilters }));
  };
  const setGroups = (newGroups: GraphGroup[]) => {
    setStoredSettings((prev) => ({ ...prev, groups: newGroups }));
  };
  const setDisplay = (newDisplay: GraphDisplaySettings) => {
    setStoredSettings((prev) => ({ ...prev, display: newDisplay }));
  };
  const setForces = (newForces: GraphForcesSettings) => {
    setStoredSettings((prev) => ({ ...prev, forces: newForces }));
  };

  // UI state
  const [showSettingsDrawer, setShowSettingsDrawer] = useState(false);
  const [hoveredNodeId, setHoveredNodeId] = useState<string | null>(null);
  const [selectedNodeId, setSelectedNodeId] = useState<string | null>(null);
  const [localGraph, setLocalGraph] = useState<LocalGraphSettings>(DEFAULT_LOCAL_GRAPH);

  // Camera Viewport Transformation { x, y, k } (Independent from node positions, NO auto-fit)
  const [camera, setCamera] = useState<CameraState>({ x: 0, y: 0, k: 1 });
  const cameraRef = useRef<CameraState>(camera);
  cameraRef.current = camera;

  // Modals state
  const [connectModalOpen, setConnectModalOpen] = useState(false);
  const [connectModalMode, setConnectModalMode] = useState<'connect' | 'merge'>('connect');
  const [activeModalScribbleId, setActiveModalScribbleId] = useState<string | null>(null);
  const [confirmTrashOpen, setConfirmTrashOpen] = useState(false);
  const [trashCandidateId, setTrashCandidateId] = useState<string | null>(null);
  const [confirmResetLayoutOpen, setConfirmResetLayoutOpen] = useState(false);

  // Time-lapse / Animation state
  const [isTimeLapsePlaying, setIsTimeLapsePlaying] = useState(false);
  const timeLapseTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const timeLapseStepRef = useRef<number>(0);

  // Simulation physics refs for 60fps canvas performance
  const simNodesRef = useRef<Map<string, SimNode>>(new Map());
  const energyRef = useRef<number>(1.0);
  const animationFrameRef = useRef<number | null>(null);
  const draggingNodeRef = useRef<{ node: SimNode; startX: number; startY: number } | null>(null);
  const isPanningRef = useRef<boolean>(false);
  const panStartRef = useRef<{ x: number; y: number }>({ x: 0, y: 0 });
  const positionsPersistTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  // Reheat physics simulation smoothly
  const reheatSimulation = useCallback((initialEnergy = 1.0) => {
    energyRef.current = Math.max(energyRef.current, initialEnergy);
  }, []);

  // Save Settings handler
  const handleSaveSettings = () => {
    const success = saveGraphSettings(storedSettings);
    if (success) {
      setSaveFeedback(true);
      setTimeout(() => setSaveFeedback(false), 2000);
    }
  };

  // Reset Settings handler
  const handleResetSettings = () => {
    const defaults = {
      filters: loadGraphSettings().filters,
      groups: [],
      display: loadGraphSettings().display,
      forces: loadGraphSettings().forces,
    };
    setStoredSettings(defaults);
    reheatSimulation(1.0);
  };

  // Reset Layout handler (Clears persisted node positions)
  const handleConfirmResetLayout = () => {
    clearNodePositions();
    const existing = simNodesRef.current;
    const width = containerRef.current?.clientWidth || 800;
    const height = containerRef.current?.clientHeight || 600;

    // Distribute randomly in a circular disc
    existing.forEach((node) => {
      const angle = Math.random() * Math.PI * 2;
      const dist = 30 + Math.random() * Math.min(width, height) * 0.35;
      node.x = Math.cos(angle) * dist;
      node.y = Math.sin(angle) * dist;
      node.vx = (Math.random() - 0.5) * 4;
      node.vy = (Math.random() - 0.5) * 4;
    });

    setConfirmResetLayoutOpen(false);
    reheatSimulation(1.0);
  };

  // Full graph adjacency map and edge list
  const { adjacencyMap, fullEdgeList } = useMemo(() => {
    const adj = new Map<string, Set<string>>();
    const edges: KnowledgeEdge[] = [];

    for (const edge of graphData.edges) {
      if (!adj.has(edge.source_id)) adj.set(edge.source_id, new Set());
      if (!adj.has(edge.target_id)) adj.set(edge.target_id, new Set());

      adj.get(edge.source_id)!.add(edge.target_id);
      adj.get(edge.target_id)!.add(edge.source_id);
      edges.push(edge);
    }

    return { adjacencyMap: adj, fullEdgeList: edges };
  }, [graphData]);

  // Filtered nodes based on filters, search query, and Local Graph traversal
  const visibleNodes = useMemo(() => {
    // 1. Base Filter matching
    const baseFiltered = graphData.nodes.filter((node) => {
      // Scribble toggle
      if (filters.showScribbles === false && node.node_type === 'scribble') return false;

      // Voice note toggle
      if (filters.showVoiceNotes === false && (node.node_type === 'voice_note' || node.source_type === 'voice')) return false;

      // Topics toggle (showTags)
      if (!filters.showTags && node.node_type === 'topic') return false;

      // Entities toggle
      if (filters.showEntities === false && (node.node_type === 'entity' || node.node_type === 'person' || node.node_type === 'organization' || node.node_type === 'place')) return false;

      // Attachments & source documents
      if (!filters.showAttachments && (node.node_type === 'source' || node.node_type === 'file')) return false;

      // Existing scribbles only
      if (filters.existingFilesOnly && node.node_type !== 'scribble') return false;

      // Orphans toggle
      if (!filters.showOrphans && (node.degree || 0) === 0) return false;

      // Search query
      if (filters.searchQuery && filters.searchQuery.trim()) {
        const q = filters.searchQuery.toLowerCase();
        const matchesLabel = node.label.toLowerCase().includes(q);
        const matchesSummary = (node.summary || '').toLowerCase().includes(q);
        const matchesTags = node.metadata?.tags && JSON.stringify(node.metadata.tags).toLowerCase().includes(q);
        const matchesTopics = node.metadata?.topics && JSON.stringify(node.metadata.topics).toLowerCase().includes(q);
        const matchesEntities = node.metadata?.entities && JSON.stringify(node.metadata.entities).toLowerCase().includes(q);
        return matchesLabel || matchesSummary || matchesTags || matchesTopics || matchesEntities;
      }

      return true;
    });

    // 2. Local Graph traversal (BFS up to localGraph.depth from rootNodeId)
    if (localGraph.enabled && localGraph.rootNodeId) {
      const rootId = localGraph.rootNodeId;
      const targetDepth = localGraph.depth || 1;
      const visited = new Set<string>([rootId]);
      let currentQueue = [rootId];

      for (let d = 0; d < targetDepth; d++) {
        const nextQueue: string[] = [];
        for (const curr of currentQueue) {
          const neighbors = adjacencyMap.get(curr);
          if (neighbors) {
            neighbors.forEach((n) => {
              if (!visited.has(n)) {
                visited.add(n);
                nextQueue.push(n);
              }
            });
          }
        }
        currentQueue = nextQueue;
      }

      return baseFiltered.filter((n) => visited.has(n.id));
    }

    return baseFiltered;
  }, [graphData.nodes, filters, localGraph, adjacencyMap]);

  const visibleNodeIds = useMemo(() => new Set(visibleNodes.map((n) => n.id)), [visibleNodes]);

  // Filtered edges where both source & target exist in visible nodes
  const visibleEdges = useMemo(() => {
    return fullEdgeList.filter((e) => visibleNodeIds.has(e.source_id) && visibleNodeIds.has(e.target_id));
  }, [fullEdgeList, visibleNodeIds]);

  // Synchronize Simulation Nodes with Persistent Positions and Groups
  useEffect(() => {
    const existing = simNodesRef.current;
    const savedPositions = loadNodePositions();
    const nextMap = new Map<string, SimNode>();
    const width = containerRef.current?.clientWidth || 800;
    const height = containerRef.current?.clientHeight || 600;

    let hasNewNodes = false;

    visibleNodes.forEach((node) => {
      const prev = existing.get(node.id);
      const degree = node.degree || 0;

      // Base radius calculation with degree scaling
      const baseRadius = node.node_type === 'topic' ? 6 : node.node_type === 'entity' ? 5 : 5.5;
      const radius = (baseRadius + Math.min(Math.sqrt(degree) * 2.8, 14)) * display.nodeSizeMultiplier;

      // Determine color (check custom search groups first, then fallback to RELAY_COLOR_MAP)
      let color = RELAY_COLOR_MAP[node.node_type] || RELAY_COLOR_MAP.default;
      for (const grp of groups) {
        if (grp.query && grp.query.trim()) {
          const q = grp.query.toLowerCase();
          if (
            node.label.toLowerCase().includes(q) ||
            (node.summary || '').toLowerCase().includes(q) ||
            (node.metadata?.topics && JSON.stringify(node.metadata.topics).toLowerCase().includes(q)) ||
            (node.metadata?.entities && JSON.stringify(node.metadata.entities).toLowerCase().includes(q))
          ) {
            color = grp.color;
            break;
          }
        }
      }

      // Restore position if existing in current session or saved in localStorage
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
          createdAtTimestamp: node.metadata?.created_at ? new Date(node.metadata.created_at).getTime() : undefined,
        });
      } else if (savedPositions[node.id]) {
        nextMap.set(node.id, {
          ...node,
          x: savedPositions[node.id].x,
          y: savedPositions[node.id].y,
          vx: 0,
          vy: 0,
          radius,
          color,
          createdAtTimestamp: node.metadata?.created_at ? new Date(node.metadata.created_at).getTime() : undefined,
        });
      } else {
        // New node: place close to connected neighbor if possible, otherwise small jitter around center
        hasNewNodes = true;
        let initX = 0;
        let initY = 0;

        const neighbors = adjacencyMap.get(node.id);
        let neighborPlaced = false;
        if (neighbors) {
          for (const nbId of neighbors) {
            const nbNode = existing.get(nbId) || (savedPositions[nbId] ? { x: savedPositions[nbId].x, y: savedPositions[nbId].y } : null);
            if (nbNode) {
              initX = nbNode.x + (Math.random() - 0.5) * 60;
              initY = nbNode.y + (Math.random() - 0.5) * 60;
              neighborPlaced = true;
              break;
            }
          }
        }

        if (!neighborPlaced) {
          const angle = Math.random() * Math.PI * 2;
          const dist = 30 + Math.random() * Math.min(width, height) * 0.30;
          initX = Math.cos(angle) * dist;
          initY = Math.sin(angle) * dist;
        }

        nextMap.set(node.id, {
          ...node,
          x: initX,
          y: initY,
          vx: (Math.random() - 0.5) * 2,
          vy: (Math.random() - 0.5) * 2,
          radius,
          color,
          createdAtTimestamp: node.metadata?.created_at ? new Date(node.metadata.created_at).getTime() : undefined,
        });
      }
    });

    simNodesRef.current = nextMap;

    // Local reheat if new nodes arrived or settings changed
    reheatSimulation(hasNewNodes ? 0.6 : 0.25);
  }, [visibleNodes, groups, display.nodeSizeMultiplier, adjacencyMap, reheatSimulation]);

  // Periodic persistence of node coordinates
  const debouncedSavePositions = useCallback(() => {
    if (positionsPersistTimerRef.current) {
      clearTimeout(positionsPersistTimerRef.current);
    }
    positionsPersistTimerRef.current = setTimeout(() => {
      const posMap: Record<string, { x: number; y: number }> = {};
      simNodesRef.current.forEach((node, id) => {
        posMap[id] = { x: Math.round(node.x), y: Math.round(node.y) };
      });
      saveNodePositions(posMap);
    }, 500);
  }, []);

  // 60FPS Force Simulation & Canvas Render Loop
  useEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas) return;
    const ctx = canvas.getContext('2d');
    if (!ctx) return;

    let isRunning = true;

    const renderLoop = () => {
      if (!isRunning) return;

      const nodes = Array.from(simNodesRef.current.values());
      const nodeMap = simNodesRef.current;

      // 1. Run physics step if energy > 0
      if (energyRef.current > 0.002) {
        const nextEnergy = stepSimulation(nodes, nodeMap, visibleEdges, forces, energyRef.current);
        energyRef.current = nextEnergy;

        if (nextEnergy === 0) {
          debouncedSavePositions();
        }
      }

      // 2. Render Canvas Frame
      renderGraph({
        canvas,
        ctx,
        nodes,
        nodeMap,
        edges: visibleEdges,
        adjacencyMap,
        camera: cameraRef.current,
        hoveredNodeId,
        selectedNodeId,
        display,
      });

      animationFrameRef.current = requestAnimationFrame(renderLoop);
    };

    animationFrameRef.current = requestAnimationFrame(renderLoop);

    return () => {
      isRunning = false;
      if (animationFrameRef.current) {
        cancelAnimationFrame(animationFrameRef.current);
      }
      if (positionsPersistTimerRef.current) {
        clearTimeout(positionsPersistTimerRef.current);
      }
    };
  }, [visibleEdges, adjacencyMap, hoveredNodeId, selectedNodeId, display, forces, debouncedSavePositions]);

  // Handle Resize with High-DPI support
  useEffect(() => {
    const handleResize = () => {
      if (containerRef.current && canvasRef.current) {
        const dpr = window.devicePixelRatio || 1;
        const rect = containerRef.current.getBoundingClientRect();
        canvasRef.current.width = Math.round(rect.width * dpr);
        canvasRef.current.height = Math.round(rect.height * dpr);
        canvasRef.current.style.width = `${rect.width}px`;
        canvasRef.current.style.height = `${rect.height}px`;
        reheatSimulation(0.2);
      }
    };

    handleResize();
    window.addEventListener('resize', handleResize);
    return () => window.removeEventListener('resize', handleResize);
  }, [reheatSimulation]);

  // Coordinate Conversion Helper: Screen (clientX, clientY) -> World Coordinates
  const screenToWorldCoords = useCallback((clientX: number, clientY: number): { x: number; y: number } | null => {
    const canvas = canvasRef.current;
    if (!canvas) return null;
    const rect = canvas.getBoundingClientRect();
    const canvasX = clientX - rect.left;
    const canvasY = clientY - rect.top;

    const curCam = cameraRef.current;
    const worldX = (canvasX - rect.width / 2 - curCam.x) / curCam.k;
    const worldY = (canvasY - rect.height / 2 - curCam.y) / curCam.k;
    return { x: worldX, y: worldY };
  }, []);

  // Hit test helper
  const getNodeAtCoords = useCallback(
    (clientX: number, clientY: number): SimNode | null => {
      const world = screenToWorldCoords(clientX, clientY);
      if (!world) return null;

      const nodes = Array.from(simNodesRef.current.values());
      for (let i = nodes.length - 1; i >= 0; i--) {
        const node = nodes[i];
        const dx = world.x - node.x;
        const dy = world.y - node.y;
        const hitRadius = Math.max(node.radius + 8, 16);
        if (dx * dx + dy * dy <= hitRadius * hitRadius) {
          return node;
        }
      }
      return null;
    },
    [screenToWorldCoords]
  );

  const pointerDownPosRef = useRef({ x: 0, y: 0 });

  // Mouse Handlers (Interactive Drag, Camera Pan, Zoom, Selection)
  const handleMouseDown = (e: React.MouseEvent<HTMLCanvasElement>) => {
    pointerDownPosRef.current = { x: e.clientX, y: e.clientY };
    const hitNode = getNodeAtCoords(e.clientX, e.clientY);
    if (hitNode) {
      draggingNodeRef.current = { node: hitNode, startX: hitNode.x, startY: hitNode.y };
      hitNode.isPinned = true;
      hitNode.vx = 0;
      hitNode.vy = 0;
      reheatSimulation(0.9);
    } else {
      isPanningRef.current = true;
      panStartRef.current = { x: e.clientX - camera.x, y: e.clientY - camera.y };
    }
  };

  const handleMouseMove = (e: React.MouseEvent<HTMLCanvasElement>) => {
    if (draggingNodeRef.current) {
      const world = screenToWorldCoords(e.clientX, e.clientY);
      if (world) {
        draggingNodeRef.current.node.x = world.x;
        draggingNodeRef.current.node.y = world.y;
        draggingNodeRef.current.node.vx = 0;
        draggingNodeRef.current.node.vy = 0;
        reheatSimulation(0.7);
      }
    } else if (isPanningRef.current) {
      setCamera((prev) => ({
        ...prev,
        x: e.clientX - panStartRef.current.x,
        y: e.clientY - panStartRef.current.y,
      }));
    } else {
      const hitNode = getNodeAtCoords(e.clientX, e.clientY);
      setHoveredNodeId(hitNode ? hitNode.id : null);
    }
  };

  const handleMouseUp = (e: React.MouseEvent<HTMLCanvasElement>) => {
    const movedDist = Math.hypot(e.clientX - pointerDownPosRef.current.x, e.clientY - pointerDownPosRef.current.y);
    const wasDraggingNode = draggingNodeRef.current;

    if (wasDraggingNode) {
      wasDraggingNode.node.isPinned = false;
      draggingNodeRef.current = null;
      debouncedSavePositions();
      reheatSimulation(0.4);
    }
    isPanningRef.current = false;

    // Distinguish click from drag: only select if mouse barely moved (< 5px)
    if (movedDist <= 5) {
      const hitNode = getNodeAtCoords(e.clientX, e.clientY);
      if (hitNode) {
        setSelectedNodeId(hitNode.id);
        if (onSelectScribble && hitNode.node_type === 'scribble') {
          onSelectScribble(hitNode.id);
        }
      } else {
        setSelectedNodeId(null);
      }
    }
  };

  const handleDoubleClick = (e: React.MouseEvent<HTMLCanvasElement>) => {
    const hitNode = getNodeAtCoords(e.clientX, e.clientY);
    if (hitNode && onOpenScribbleEditor && hitNode.node_type === 'scribble') {
      onOpenScribbleEditor(hitNode.id);
    }
  };

  // Cursor-Centered Mouse Wheel Zoom
  const handleWheel = (e: React.WheelEvent<HTMLCanvasElement>) => {
    e.preventDefault();
    const zoomFactor = e.deltaY < 0 ? 1.15 : 0.87;

    const canvas = canvasRef.current;
    if (!canvas) return;
    const rect = canvas.getBoundingClientRect();
    const mouseX = e.clientX - rect.left - rect.width / 2;
    const mouseY = e.clientY - rect.top - rect.height / 2;

    setCamera((prev) => {
      const newK = Math.max(0.15, Math.min(prev.k * zoomFactor, 4.5));
      const scaleChange = newK / prev.k;
      // Adjust camera pan so zoom centers on cursor
      const newX = mouseX - (mouseX - prev.x) * scaleChange;
      const newY = mouseY - (mouseY - prev.y) * scaleChange;
      return { x: newX, y: newY, k: newK };
    });
  };

  // Keyboard Navigation Controls
  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      // Don't trigger shortcuts if user is typing in an input
      if (['INPUT', 'TEXTAREA'].includes((e.target as HTMLElement)?.tagName)) {
        return;
      }

      const panDelta = e.shiftKey ? 45 : 15;

      switch (e.key) {
        case 'ArrowUp':
          e.preventDefault();
          setCamera((prev) => ({ ...prev, y: prev.y + panDelta }));
          break;
        case 'ArrowDown':
          e.preventDefault();
          setCamera((prev) => ({ ...prev, y: prev.y - panDelta }));
          break;
        case 'ArrowLeft':
          e.preventDefault();
          setCamera((prev) => ({ ...prev, x: prev.x + panDelta }));
          break;
        case 'ArrowRight':
          e.preventDefault();
          setCamera((prev) => ({ ...prev, x: prev.x - panDelta }));
          break;
        case '+':
        case '=':
          e.preventDefault();
          setCamera((prev) => ({ ...prev, k: Math.min(prev.k * 1.2, 4.5) }));
          break;
        case '-':
        case '_':
          e.preventDefault();
          setCamera((prev) => ({ ...prev, k: Math.max(prev.k * 0.83, 0.15) }));
          break;
        case '0':
          e.preventDefault();
          setCamera({ x: 0, y: 0, k: 1 });
          break;
        case ' ':
          e.preventDefault();
          reheatSimulation(1.0);
          break;
        case 'Escape':
          setSelectedNodeId(null);
          setShowSettingsDrawer(false);
          if (localGraph.enabled) {
            setLocalGraph(DEFAULT_LOCAL_GRAPH);
          }
          break;
      }
    };

    window.addEventListener('keydown', handleKeyDown);
    return () => window.removeEventListener('keydown', handleKeyDown);
  }, [localGraph.enabled, reheatSimulation]);

  // Time-Lapse / Chronological Reveal Animation
  const handleToggleTimeLapse = () => {
    if (isTimeLapsePlaying) {
      if (timeLapseTimerRef.current) clearInterval(timeLapseTimerRef.current);
      setIsTimeLapsePlaying(false);
    } else {
      setIsTimeLapsePlaying(true);
      const allNodes = Array.from(simNodesRef.current.values()).sort(
        (a, b) => (a.createdAtTimestamp || 0) - (b.createdAtTimestamp || 0)
      );

      // Hide all, then reveal 1 by 1
      timeLapseStepRef.current = 0;
      timeLapseTimerRef.current = setInterval(() => {
        timeLapseStepRef.current += 1;
        reheatSimulation(0.5);

        if (timeLapseStepRef.current >= allNodes.length) {
          if (timeLapseTimerRef.current) clearInterval(timeLapseTimerRef.current);
          setIsTimeLapsePlaying(false);
        }
      }, 300);
    }
  };

  // Local Graph Mode Handler
  const handleToggleLocalGraph = () => {
    if (localGraph.enabled) {
      setLocalGraph(DEFAULT_LOCAL_GRAPH);
    } else {
      const rootId = selectedNodeId || (visibleNodes[0]?.id ?? null);
      if (rootId) {
        setSelectedNodeId(rootId);
        setLocalGraph({ enabled: true, rootNodeId: rootId, depth: 1 });
      }
    }
    reheatSimulation(0.7);
  };

  const handleDepthChange = (depth: number) => {
    setLocalGraph((prev) => ({ ...prev, depth }));
    reheatSimulation(0.7);
  };

  // Contextual Node Actions
  const handleConnectScribble = (scribbleId: string) => {
    setActiveModalScribbleId(scribbleId);
    setConnectModalMode('connect');
    setConnectModalOpen(true);
  };

  const handleMergeScribbles = (scribbleId: string) => {
    setActiveModalScribbleId(scribbleId);
    setConnectModalMode('merge');
    setConnectModalOpen(true);
  };

  const handleDeleteScribble = (scribbleId: string) => {
    setTrashCandidateId(scribbleId);
    setConfirmTrashOpen(true);
  };

  const handleConfirmTrash = async () => {
    if (!trashCandidateId) return;
    try {
      await invoke('move_to_trash', { itemType: 'scribble', id: trashCandidateId });
      if (onScribbleDeleted) {
        onScribbleDeleted(trashCandidateId);
      }
      setSelectedNodeId(null);
      setConfirmTrashOpen(false);
      setTrashCandidateId(null);
    } catch (err) {
      console.error('Failed to move scribble to trash:', err);
    }
  };

  const handleExploreLocalGraph = (nodeId: string) => {
    setSelectedNodeId(nodeId);
    setLocalGraph({ enabled: true, rootNodeId: nodeId, depth: 1 });
    reheatSimulation(0.8);
  };

  // Selected Node & Neighbors for Inspector
  const selectedNode = selectedNodeId ? simNodesRef.current.get(selectedNodeId) || null : null;
  const connectedNeighbors = useMemo(() => {
    if (!selectedNodeId) return [];
    const neighbors = adjacencyMap.get(selectedNodeId);
    if (!neighbors) return [];
    return Array.from(neighbors)
      .map((id) => simNodesRef.current.get(id))
      .filter((n): n is SimNode => n !== undefined);
  }, [selectedNodeId, adjacencyMap]);

  const activeModalScribble = useMemo(() => {
    if (!activeModalScribbleId) return null;
    return allScribbles.find((s) => s.id === activeModalScribbleId) || null;
  }, [activeModalScribbleId, allScribbles]);

  return (
    <div
      ref={containerRef}
      className="relative w-full h-full min-h-[500px] overflow-hidden select-none bg-card rounded-lg border border-border"
    >
      {/* 2D High-DPI Force Simulation Canvas */}
      <canvas
        ref={canvasRef}
        onMouseDown={handleMouseDown}
        onMouseMove={handleMouseMove}
        onMouseUp={handleMouseUp}
        onMouseLeave={handleMouseUp}
        onDoubleClick={handleDoubleClick}
        onWheel={handleWheel}
        className={`w-full h-full block ${
          draggingNodeRef.current
            ? 'cursor-grabbing'
            : hoveredNodeId
            ? 'cursor-grab'
            : isPanningRef.current
            ? 'cursor-grabbing'
            : 'cursor-default'
        }`}
      />

      {/* Floating Graph Toolbar */}
      <GraphToolbar
        showSettingsDrawer={showSettingsDrawer}
        onToggleSettingsDrawer={() => setShowSettingsDrawer(!showSettingsDrawer)}
        localGraph={localGraph}
        onToggleLocalGraph={handleToggleLocalGraph}
        onDepthChange={handleDepthChange}
        onZoomIn={() => setCamera((prev) => ({ ...prev, k: Math.min(prev.k * 1.2, 4.5) }))}
        onZoomOut={() => setCamera((prev) => ({ ...prev, k: Math.max(prev.k * 0.83, 0.15) }))}
        onResetZoom={() => setCamera({ x: 0, y: 0, k: 1 })}
        isTimeLapsePlaying={isTimeLapsePlaying}
        onToggleTimeLapse={handleToggleTimeLapse}
        nodeCount={visibleNodes.length}
      />

      {/* Obsidian-Style Graph Settings Panel */}
      <GraphSettingsPanel
        isOpen={showSettingsDrawer}
        onClose={() => setShowSettingsDrawer(false)}
        filters={filters}
        onFiltersChange={setFilters}
        groups={groups}
        onGroupsChange={setGroups}
        display={display}
        onDisplayChange={setDisplay}
        forces={forces}
        onForcesChange={setForces}
        onSaveSettings={handleSaveSettings}
        onResetSettings={handleResetSettings}
        onResetLayout={() => setConfirmResetLayoutOpen(true)}
        onReheatSimulation={() => reheatSimulation(1.0)}
        saveFeedback={saveFeedback}
      />

      {/* Selected Node Inspector Drawer (Bottom Left) */}
      <GraphNodeInspector
        selectedNode={selectedNode}
        neighbors={connectedNeighbors}
        onClose={() => setSelectedNodeId(null)}
        onSelectNode={(id) => setSelectedNodeId(id)}
        onOpenScribbleEditor={onOpenScribbleEditor}
        onConnectScribble={handleConnectScribble}
        onMergeScribbles={handleMergeScribbles}
        onDeleteScribble={handleDeleteScribble}
        onExploreLocalGraph={handleExploreLocalGraph}
      />

      {/* Connect & Merge Scribbles Modal */}
      {connectModalOpen && activeModalScribble && (
        <ConnectAndMergeModal
          currentScribble={activeModalScribble}
          allScribbles={allScribbles}
          mode={connectModalMode}
          isOpen={connectModalOpen}
          onClose={() => {
            setConnectModalOpen(false);
            setActiveModalScribbleId(null);
          }}
          onScribbleUpdated={(updated) => {
            if (onScribbleUpdated) onScribbleUpdated(updated);
            setConnectModalOpen(false);
            setActiveModalScribbleId(null);
            reheatSimulation(0.8);
          }}
          onScribbleCreated={(created) => {
            if (onScribbleCreated) onScribbleCreated(created);
            setConnectModalOpen(false);
            setActiveModalScribbleId(null);
            setSelectedNodeId(created.id);
            reheatSimulation(1.0);
          }}
        />
      )}

      {/* Confirm Trash Modal */}
      <ConfirmationModal
        isOpen={confirmTrashOpen}
        title="Move Scribble to Trash?"
        description="This scribble will be moved to Trash and permanently removed after 30 days unless restored."
        confirmLabel="Move to Trash"
        variant="destructive"
        onConfirm={handleConfirmTrash}
        onCancel={() => {
          setConfirmTrashOpen(false);
          setTrashCandidateId(null);
        }}
      />

      {/* Confirm Reset Layout Modal */}
      <ConfirmationModal
        isOpen={confirmResetLayoutOpen}
        title="Reset Graph Layout?"
        description="This will clear all saved node coordinates and simulate a fresh layout. Your filters, groups, and display settings will be preserved."
        confirmLabel="Reset Layout"
        variant="destructive"
        onConfirm={handleConfirmResetLayout}
        onCancel={() => setConfirmResetLayoutOpen(false)}
      />
    </div>
  );
};
