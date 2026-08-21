import { GraphDisplaySettings, KnowledgeEdge } from '../../../types';
import { CameraState, SimNode } from './graphTypes';

export interface RenderContext {
  canvas: HTMLCanvasElement;
  ctx: CanvasRenderingContext2D;
  nodes: SimNode[];
  nodeMap: Map<string, SimNode>;
  edges: KnowledgeEdge[];
  adjacencyMap: Map<string, Set<string>>;
  camera: CameraState;
  hoveredNodeId: string | null;
  selectedNodeId: string | null;
  display: GraphDisplaySettings;
}

/**
 * Truncates text with ellipsis to keep graph uncluttered
 */
function truncateLabel(text: string, maxLength: number = 22): string {
  if (!text) return '';
  if (text.length <= maxLength) return text;
  return text.substring(0, maxLength - 1) + '…';
}

/**
 * Draws directional arrow on edge
 */
function drawArrow(
  ctx: CanvasRenderingContext2D,
  fromX: number,
  fromY: number,
  toX: number,
  toY: number,
  targetRadius: number
) {
  const dx = toX - fromX;
  const dy = toY - fromY;
  const angle = Math.atan2(dy, dx);
  const arrowDist = targetRadius + 5;
  const arrowX = toX - Math.cos(angle) * arrowDist;
  const arrowY = toY - Math.sin(angle) * arrowDist;
  const arrowSize = 6;

  ctx.beginPath();
  ctx.moveTo(arrowX, arrowY);
  ctx.lineTo(
    arrowX - arrowSize * Math.cos(angle - Math.PI / 6),
    arrowY - arrowSize * Math.sin(angle - Math.PI / 6)
  );
  ctx.lineTo(
    arrowX - arrowSize * Math.cos(angle + Math.PI / 6),
    arrowY - arrowSize * Math.sin(angle + Math.PI / 6)
  );
  ctx.closePath();
  ctx.fill();
}

/**
 * Main 2D Canvas rendering pipeline
 */
export function renderGraph(rc: RenderContext) {
  const {
    canvas,
    ctx,
    nodes,
    nodeMap,
    edges,
    adjacencyMap,
    camera,
    hoveredNodeId,
    selectedNodeId,
    display,
  } = rc;

  const dpr = window.devicePixelRatio || 1;
  const cssWidth = canvas.width / dpr;
  const cssHeight = canvas.height / dpr;

  // 1. Clear background (Transparent to match Relay theme container)
  ctx.save();
  ctx.scale(dpr, dpr);
  ctx.clearRect(0, 0, cssWidth, cssHeight);

  // Center world coordinates + apply Camera
  ctx.translate(cssWidth / 2 + camera.x, cssHeight / 2 + camera.y);
  ctx.scale(camera.k, camera.k);

  // 2. Compute 1-hop highlight neighborhood
  const activeId = hoveredNodeId || selectedNodeId;
  const isQuietingActive = !!activeId;
  const highlightSet = new Set<string>();

  if (activeId) {
    highlightSet.add(activeId);
    const neighbors = adjacencyMap.get(activeId);
    if (neighbors) {
      neighbors.forEach((n) => highlightSet.add(n));
    }
  }

  // 3. Render Edges (Subordinate to nodes, thin, low-contrast)
  const baseLinkThickness = display.linkThickness || 1.0;

  for (let i = 0; i < edges.length; i++) {
    const edge = edges[i];
    const source = nodeMap.get(edge.source_id);
    const target = nodeMap.get(edge.target_id);
    if (!source || !target) continue;

    const isConnectedToActive =
      isQuietingActive &&
      highlightSet.has(edge.source_id) &&
      highlightSet.has(edge.target_id);

    const isDimmed = isQuietingActive && !isConnectedToActive;

    ctx.beginPath();
    ctx.moveTo(source.x, source.y);
    ctx.lineTo(target.x, target.y);

    if (isConnectedToActive) {
      ctx.strokeStyle = '#60a5fa'; // Bright highlight blue
      ctx.lineWidth = 1.8 * baseLinkThickness;
      ctx.globalAlpha = 0.95;
    } else if (isDimmed) {
      ctx.strokeStyle = '#334155';
      ctx.lineWidth = 0.5 * baseLinkThickness;
      ctx.globalAlpha = 0.08; // Quieted
    } else {
      const isExplicit = edge.relationship !== 'SAME_TOPIC' && edge.relationship !== 'MENTIONS';
      ctx.strokeStyle = isExplicit ? '#64748b' : '#475569';
      ctx.lineWidth = (isExplicit ? 0.9 : 0.7) * baseLinkThickness;
      ctx.globalAlpha = isExplicit ? 0.45 : 0.28;
    }

    ctx.stroke();

    // Optional directional arrows
    if (display.showArrows && !isDimmed) {
      ctx.fillStyle = ctx.strokeStyle;
      drawArrow(ctx, source.x, source.y, target.x, target.y, target.radius);
    }
  }

  ctx.globalAlpha = 1.0;

  // 4. Render Nodes (Circular, degree-scaled, color-coded)
  for (let i = 0; i < nodes.length; i++) {
    const node = nodes[i];
    const isSelected = node.id === selectedNodeId;
    const isHovered = node.id === hoveredNodeId;
    const isHighlighted = isQuietingActive && highlightSet.has(node.id);
    const isDimmed = isQuietingActive && !isHighlighted;

    ctx.save();
    ctx.beginPath();
    ctx.arc(node.x, node.y, node.radius, 0, Math.PI * 2);

    if (isSelected) {
      ctx.fillStyle = '#ffffff';
      ctx.shadowColor = node.color;
      ctx.shadowBlur = 18;
    } else if (isHovered) {
      ctx.fillStyle = node.color;
      ctx.shadowColor = node.color;
      ctx.shadowBlur = 14;
    } else if (isHighlighted) {
      ctx.fillStyle = node.color;
      ctx.shadowColor = node.color;
      ctx.shadowBlur = 8;
    } else if (isDimmed) {
      ctx.fillStyle = node.color;
      ctx.globalAlpha = 0.12; // Quieted
    } else {
      ctx.fillStyle = node.color;
      ctx.globalAlpha = 0.90;
    }

    ctx.fill();

    // Outline / Border
    if (!isDimmed) {
      ctx.lineWidth = isSelected ? 2.5 : isHovered ? 2.0 : 1.0;
      ctx.strokeStyle = isSelected
        ? node.color
        : isHovered
        ? '#ffffff'
        : 'rgba(255, 255, 255, 0.22)';
      ctx.stroke();
    }

    ctx.restore();
  }

  // 5. Render Labels with Zoom-Aware Dynamic Fade & Priority
  const textFadeThresh = display.textFadeThreshold ?? 0.6;

  // Sort labels so active/hovered/high-degree nodes render on top of others
  const labelNodes = [...nodes].sort((a, b) => {
    const aPriority = a.id === hoveredNodeId ? 3 : a.id === selectedNodeId ? 2 : (a.degree || 0);
    const bPriority = b.id === hoveredNodeId ? 3 : b.id === selectedNodeId ? 2 : (b.degree || 0);
    return aPriority - bPriority;
  });

  const fontSize = Math.max(9, Math.min(12, 11 / Math.sqrt(camera.k)));

  for (let i = 0; i < labelNodes.length; i++) {
    const node = labelNodes[i];
    const isSelected = node.id === selectedNodeId;
    const isHovered = node.id === hoveredNodeId;
    const isHighlighted = isQuietingActive && highlightSet.has(node.id);
    const isDimmed = isQuietingActive && !isHighlighted;

    if (isDimmed) continue;

    // Calculate dynamic text opacity
    let textOpacity = 1.0;
    const degree = node.degree || 0;

    if (isSelected || isHovered) {
      textOpacity = 1.0;
    } else if (isHighlighted) {
      textOpacity = 0.95;
    } else if (textFadeThresh <= 0.05) {
      textOpacity = 1.0;
    } else {
      // High degree hubs reveal at lower zoom levels
      const nodeThresh = textFadeThresh * Math.max(0.35, 1.0 - Math.min(degree * 0.08, 0.65));
      const factor = (camera.k - (nodeThresh * 0.5)) / (nodeThresh * 0.5 || 0.1);
      textOpacity = Math.max(0, Math.min(1, factor));
    }

    if (textOpacity > 0.05) {
      ctx.save();
      ctx.globalAlpha = textOpacity;
      ctx.font = `${isSelected || isHovered ? '600' : '400'} ${fontSize}px system-ui, -apple-system, BlinkMacSystemFont, sans-serif`;
      ctx.textAlign = 'center';
      ctx.textBaseline = 'top';

      const labelText = truncateLabel(node.label, isHovered || isSelected ? 30 : 20);
      const textX = node.x;
      const textY = node.y + node.radius + 4;

      // Draw subtle dark pill background for hovered/selected nodes for supreme readability
      if (isHovered || isSelected) {
        const textMetrics = ctx.measureText(labelText);
        const paddingX = 4;
        const paddingY = 2;
        const bgW = textMetrics.width + paddingX * 2;
        const bgH = fontSize + paddingY * 2;

        ctx.fillStyle = 'rgba(15, 17, 23, 0.85)';
        ctx.strokeStyle = isSelected ? node.color : 'rgba(255, 255, 255, 0.2)';
        ctx.lineWidth = 1;
        ctx.beginPath();
        ctx.roundRect(textX - bgW / 2, textY - paddingY, bgW, bgH, 3);
        ctx.fill();
        ctx.stroke();
      }

      ctx.fillStyle = isSelected ? '#ffffff' : isHovered ? '#f8fafc' : isHighlighted ? '#e2e8f0' : '#94a3b8';
      ctx.fillText(labelText, textX, textY);
      ctx.restore();
    }
  }

  ctx.restore();
}
