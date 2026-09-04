import React from 'react';
import { beforeEach, describe, expect, test, vi } from 'vitest';
import { render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { invoke } from '@tauri-apps/api/core';

import { KnowledgeGraphPage } from './KnowledgeGraphPage';

import type { KnowledgeGraphData } from '@/types';

const graph: KnowledgeGraphData = {
  nodes: [
    { id: 'scr_1', node_type: 'scribble', label: 'Chunking strategy', metadata: {}, degree: 2 },
    { id: 'topic_1', node_type: 'topic', label: 'Retrieval', metadata: {}, degree: 1 },
    { id: 'ent_1', node_type: 'entity', label: 'Relay', metadata: {}, degree: 1 },
    { id: 'scr_2', node_type: 'scribble', label: 'Orphan thought', metadata: {}, degree: 0 },
  ],
  edges: [
    {
      id: 'edge_1',
      source_id: 'scr_1',
      target_id: 'topic_1',
      relationship: 'HAS_TOPIC',
      confidence: 1,
      source: 'system',
    },
    {
      id: 'edge_2',
      source_id: 'scr_1',
      target_id: 'ent_1',
      relationship: 'MENTIONS',
      confidence: 0.9,
      source: 'ai',
    },
  ],
};

const telemetry = {
  total_memories: 2,
  active_memories: 2,
  total_entities: 1,
  total_relationships: 2,
  total_scribbles: 2,
  total_notes: 0,
  total_files: 0,
  total_captures: 0,
};

describe('KnowledgeGraphPage', () => {
  beforeEach(() => {
    vi.mocked(invoke).mockImplementation(async (cmd: string) => {
      switch (cmd) {
        case 'get_knowledge_graph':
          return graph;
        case 'get_scribbles':
          return [];
        case 'get_knowledge_telemetry':
          return telemetry;
        default:
          return null;
      }
    });
  });

  test('reads the graph itself rather than depending on the Scribbles surface', async () => {
    render(<KnowledgeGraphPage />);

    await waitFor(() => {
      expect(vi.mocked(invoke)).toHaveBeenCalledWith('get_knowledge_graph', { filter: null });
    });
    expect(vi.mocked(invoke)).toHaveBeenCalledWith('get_scribbles');
  });

  test('summarises what the canvas toolbar does not: links, orphans, resolved knowledge', async () => {
    render(<KnowledgeGraphPage />);

    expect(await screen.findByText('2 links')).toBeInTheDocument();
    expect(screen.getByText('1 unconnected')).toBeInTheDocument();
    expect(screen.getByText(/1 entity · 2 relationships · 2 memories/)).toBeInTheDocument();
  });

  test('rebuilding re-reads the graph', async () => {
    render(<KnowledgeGraphPage />);
    const user = userEvent.setup();

    await screen.findByText('2 links');
    const graphReads = () =>
      vi.mocked(invoke).mock.calls.filter(([cmd]) => cmd === 'get_knowledge_graph').length;
    const before = graphReads();

    await user.click(screen.getByRole('button', { name: /Rebuild graph/i }));

    await waitFor(() => expect(graphReads()).toBeGreaterThan(before));
  });

  test('says there is nothing to connect instead of drawing an empty canvas', async () => {
    vi.mocked(invoke).mockImplementation(async (cmd: string) =>
      cmd === 'get_knowledge_graph' ? { nodes: [], edges: [] } : null,
    );

    render(<KnowledgeGraphPage />);

    expect(await screen.findByText('Nothing to connect yet')).toBeInTheDocument();
    expect(screen.getByText('0 links')).toBeInTheDocument();
  });

  test('a failed read leaves an empty graph rather than a thrown page', async () => {
    vi.mocked(invoke).mockImplementation(async () => {
      throw new Error('vault unreadable');
    });

    render(<KnowledgeGraphPage />);

    expect(await screen.findByText('Nothing to connect yet')).toBeInTheDocument();
  });
});
