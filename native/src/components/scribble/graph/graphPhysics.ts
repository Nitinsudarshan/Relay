import { GraphForcesSettings, KnowledgeEdge } from '../../../types';
import { SimNode } from './graphTypes';

export interface PhysicsStepResult {
  hasMovement: boolean;
  alpha: number;
}

/**
 * Executes a single physics simulation tick over simNodes.
 * Modifies x, y, vx, vy in-place for maximum performance.
 */
export function stepSimulation(
  nodes: SimNode[],
  nodeMap: Map<string, SimNode>,
  edges: KnowledgeEdge[],
  forces: GraphForcesSettings,
  alpha: number
): number {
  if (alpha <= 0.002) {
    return 0; // Settled
  }

  const repelEffective = forces.repelForce * 35;
  const centerEffective = forces.centerForce * 0.008;
  const linkEffective = forces.linkForce * 0.05;
  const linkDistance = forces.linkDistance;

  const nodeCount = nodes.length;

  // 1. Coulomb Repulsion (All-pairs with distance threshold)
  for (let i = 0; i < nodeCount; i++) {
    const a = nodes[i];
    for (let j = i + 1; j < nodeCount; j++) {
      const b = nodes[j];
      let dx = b.x - a.x;
      let dy = b.y - a.y;
      let distSq = dx * dx + dy * dy;

      if (distSq === 0) {
        dx = (Math.random() - 0.5) * 2;
        dy = (Math.random() - 0.5) * 2;
        distSq = dx * dx + dy * dy;
      }

      if (distSq < 900000) { // ~950px max interaction distance
        const dist = Math.sqrt(distSq);
        // Softened Coulomb repulsion
        const force = (repelEffective / Math.max(distSq, 400)) * alpha;
        const fx = (dx / dist) * force;
        const fy = (dy / dist) * force;

        if (!a.isPinned) {
          a.vx -= fx;
          a.vy -= fy;
        }
        if (!b.isPinned) {
          b.vx += fx;
          b.vy += fy;
        }
      }
    }
  }

  // 2. Hooke Spring Links (Connected edges)
  for (let i = 0; i < edges.length; i++) {
    const edge = edges[i];
    const source = nodeMap.get(edge.source_id);
    const target = nodeMap.get(edge.target_id);

    if (source && target) {
      let dx = target.x - source.x;
      let dy = target.y - source.y;
      let dist = Math.sqrt(dx * dx + dy * dy);

      if (dist === 0) {
        dx = (Math.random() - 0.5) * 2;
        dy = (Math.random() - 0.5) * 2;
        dist = Math.sqrt(dx * dx + dy * dy);
      }

      const diff = dist - linkDistance;
      const force = diff * linkEffective * alpha;
      const fx = (dx / dist) * force;
      const fy = (dy / dist) * force;

      if (!source.isPinned) {
        source.vx += fx;
        source.vy += fy;
      }
      if (!target.isPinned) {
        target.vx -= fx;
        target.vy -= fy;
      }
    }
  }

  // 3. Center Gravity & Velocity Damping
  let maxVelocitySq = 0;

  for (let i = 0; i < nodeCount; i++) {
    const node = nodes[i];
    if (!node.isPinned) {
      // Center gravity
      node.vx -= node.x * centerEffective * alpha;
      node.vy -= node.y * centerEffective * alpha;

      // Damping / Friction
      node.vx *= 0.88;
      node.vy *= 0.88;

      // Position update
      node.x += node.vx;
      node.y += node.vy;

      const vSq = node.vx * node.vx + node.vy * node.vy;
      if (vSq > maxVelocitySq) {
        maxVelocitySq = vSq;
      }
    }
  }

  // Alpha energy decay
  const nextAlpha = alpha * 0.985;
  return nextAlpha < 0.002 ? 0 : nextAlpha;
}
