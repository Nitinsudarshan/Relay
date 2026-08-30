import { describe, expect, it } from 'vitest';
import { stepSimulation } from './graphPhysics';
import { DEFAULT_FORCES } from './graphTypes';
import type { SimNode } from './graphTypes';
import type { KnowledgeEdge } from '../../../types';

/**
 * The force-directed layout behind the knowledge graph.
 *
 * `stepSimulation` mutates nodes in place for performance, so these assert on
 * the invariants a layout must hold — pinned nodes never drift, energy always
 * decays, coincident nodes separate — rather than on exact coordinates, which
 * are an implementation detail of the force constants.
 */

const node = (overrides: Partial<SimNode> = {}): SimNode =>
  ({
    id: 'n1',
    label: 'Node',
    node_type: 'scribble',
    x: 0,
    y: 0,
    vx: 0,
    vy: 0,
    radius: 6,
    color: '#fff',
    ...overrides,
  }) as SimNode;

const mapOf = (nodes: SimNode[]) => new Map(nodes.map((n) => [n.id, n]));

const edge = (source_id: string, target_id: string): KnowledgeEdge =>
  ({ source_id, target_id }) as KnowledgeEdge;

describe('stepSimulation', () => {
  it('reports settled and does no work once alpha falls below the floor', () => {
    const nodes = [node({ x: 100, y: 100 })];
    const before = { ...nodes[0] };

    expect(stepSimulation(nodes, mapOf(nodes), [], DEFAULT_FORCES, 0.001)).toBe(0);
    expect(nodes[0].x).toBe(before.x);
    expect(nodes[0].y).toBe(before.y);
  });

  it('decays alpha every tick so the layout always comes to rest', () => {
    const nodes = [node({ x: 50, y: 50 })];
    let alpha = 1;
    const first = stepSimulation(nodes, mapOf(nodes), [], DEFAULT_FORCES, alpha);
    expect(first).toBeLessThan(alpha);

    // And it reaches zero rather than approaching it forever.
    alpha = first;
    for (let i = 0; i < 5000 && alpha > 0; i++) {
      alpha = stepSimulation(nodes, mapOf(nodes), [], DEFAULT_FORCES, alpha);
    }
    expect(alpha).toBe(0);
  });

  it('never moves a pinned node', () => {
    const pinned = node({ id: 'pinned', x: 200, y: -50, isPinned: true });
    const free = node({ id: 'free', x: 210, y: -40 });
    const nodes = [pinned, free];

    for (let i = 0; i < 20; i++) {
      stepSimulation(nodes, mapOf(nodes), [edge('pinned', 'free')], DEFAULT_FORCES, 1);
    }

    expect(pinned.x).toBe(200);
    expect(pinned.y).toBe(-50);
    expect(pinned.vx).toBe(0);
    expect(pinned.vy).toBe(0);
    // The unpinned neighbour did move — otherwise this proves nothing.
    expect(free.x).not.toBe(210);
  });

  it('pushes two unconnected nodes apart', () => {
    const a = node({ id: 'a', x: -5, y: 0 });
    const b = node({ id: 'b', x: 5, y: 0 });
    const nodes = [a, b];
    const gapBefore = Math.abs(b.x - a.x);

    for (let i = 0; i < 30; i++) {
      stepSimulation(nodes, mapOf(nodes), [], DEFAULT_FORCES, 1);
    }

    expect(Math.abs(b.x - a.x)).toBeGreaterThan(gapBefore);
  });

  // Coincident nodes are a real case — two entities extracted at the same
  // seed position. Without the random nudge this divides by zero and every
  // coordinate becomes NaN, which renders as a blank canvas.
  it('separates exactly coincident nodes instead of producing NaN', () => {
    const a = node({ id: 'a', x: 0, y: 0 });
    const b = node({ id: 'b', x: 0, y: 0 });
    const nodes = [a, b];

    for (let i = 0; i < 20; i++) {
      stepSimulation(nodes, mapOf(nodes), [edge('a', 'b')], DEFAULT_FORCES, 1);
    }

    for (const n of nodes) {
      expect(Number.isFinite(n.x)).toBe(true);
      expect(Number.isFinite(n.y)).toBe(true);
    }
    expect(a.x === b.x && a.y === b.y).toBe(false);
  });

  it('pulls a linked pair toward the configured link distance', () => {
    const a = node({ id: 'a', x: -2000, y: 0 });
    const b = node({ id: 'b', x: 2000, y: 0 });
    const nodes = [a, b];
    const edges = [edge('a', 'b')];
    const startGap = Math.abs(b.x - a.x);

    let alpha = 1;
    for (let i = 0; i < 400 && alpha > 0; i++) {
      alpha = stepSimulation(nodes, mapOf(nodes), edges, DEFAULT_FORCES, alpha);
    }

    // Far apart, the spring dominates: the pair must end up closer than it
    // started, heading for `linkDistance`.
    expect(Math.abs(b.x - a.x)).toBeLessThan(startGap);
  });

  it('ignores edges pointing at nodes that are not in the graph', () => {
    const a = node({ id: 'a', x: 10, y: 10 });
    const nodes = [a];

    expect(() =>
      stepSimulation(nodes, mapOf(nodes), [edge('a', 'ghost')], DEFAULT_FORCES, 1),
    ).not.toThrow();
    expect(Number.isFinite(a.x)).toBe(true);
  });

  it('handles an empty graph', () => {
    expect(stepSimulation([], new Map(), [], DEFAULT_FORCES, 1)).toBeGreaterThan(0);
  });
});
