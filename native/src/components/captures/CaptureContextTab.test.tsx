import { fireEvent, render, screen } from '@testing-library/react';
import React from 'react';
import { describe, expect, it, vi } from 'vitest';
import type { CaptureProvenance, ConversationContext, RepositoryContext, SourceContext } from '../../types';
import { CaptureContextTab } from './CaptureContextTab';

describe('CaptureContextTab', () => {
  const mockOnAnalyze = vi.fn().mockResolvedValue(undefined);

  const sampleRepoContext: RepositoryContext = {
    capture_id: 'cap_orca',
    repository_name: 'stablyai/orca',
    objective:
      'Orca is an AI orchestration/development environment for running multiple coding agents in parallel, using isolated Git worktrees and tools for managing and monitoring agent work.',
    stack: [
      'Git / Git worktrees',
      'WebGL terminal rendering',
      'Chromium/browser integration',
      'SSH / remote development',
      'CLI-based coding-agent ecosystem',
    ],
    features: [
      'Parallel coding-agent orchestration',
      'Isolated Git worktrees',
      'Multi-agent development workflows',
      'Integrated terminal splits',
      'GitHub and Linear integration',
      'Supports a broad range of CLI coding agents',
    ],
    user_base: [
      'Software developers',
      'AI-assisted developers',
      'Developers using coding agents',
    ],
    licensing: 'MIT License',
    generated_at: '2026-09-04T00:00:00Z',
    model: 'Anthropic Claude',
    deterministic: false,
  };

  const sampleConversationContext: ConversationContext = {
    capture_id: 'cap_chat',
    title: 'Architecture Discussion',
    objective: 'Design local-first context handoff',
    background: ['Relay is desktop assistant'],
    current_state: 'Architecture finalized',
    decisions: [
      {
        id: 'dec_1',
        decision: 'Use SQLite',
        rationale: 'Reliable storage',
        status: 'CURRENT',
        source_turn_ordinals: [1],
      },
    ],
    requirements: [{ id: 'req_1', statement: 'Must work offline', source_turn_ordinals: [1] }],
    constraints: [{ id: 'con_1', statement: 'Zero telemetry', reason: 'Privacy', source_turn_ordinals: [1] }],
    preferences: [],
    rejected_approaches: [{ approach: 'Cloud sync', reason_rejected: 'Privacy risk', source_turn_ordinals: [1] }],
    open_questions: [],
    action_items: [{ id: 'act_1', description: 'Write unit tests', status: 'OPEN', source_turn_ordinals: [1] }],
    important_facts: [],
    key_artifacts: [],
    generated_at: '2026-09-04T00:00:00Z',
    model: 'OpenAI GPT-4',
    deterministic: false,
  };

  it('renders GitHub-specific empty state with "Extract Repository Context" button', () => {
    const gitHubProvenance: CaptureProvenance = {
      source_type: 'web',
      capture_type: 'repository',
      application: 'github',
      domain: 'github.com',
      url: 'https://github.com/stablyai/orca',
      page_title: 'stablyai/orca',
      captured_at: '2026-09-03T10:00:00Z',
      coverage: 'full',
      fidelity: 'structural',
      trust: 'external_untrusted',
      extractor_id: 'github',
      extractor_version: 1,
      notes: [],
      block_count: 5,
      skipped_block_count: 0,
      truncated: false,
      version: 1,
      recapture_count: 0,
    };

    render(
      <CaptureContextTab
        context={null}
        provenance={gitHubProvenance}
        loading={false}
        analyzing={false}
        onAnalyze={mockOnAnalyze}
      />,
    );

    expect(screen.getByText('Structured Context Unavailable')).toBeDefined();
    expect(
      screen.getByText('Relay has captured this repository, but has not yet extracted structured repository context.'),
    ).toBeDefined();
    const btn = screen.getByRole('button', { name: /extract repository context/i });
    expect(btn).toBeDefined();

    fireEvent.click(btn);
    expect(mockOnAnalyze).toHaveBeenCalledTimes(1);
  });

  it('renders RepositoryContext with 5 core dimensions and no issue or conversation headings', () => {
    const sourceContext: SourceContext = {
      capture_id: 'cap_orca',
      generated_at: '2026-09-04T00:00:00Z',
      deterministic: false,
      model: 'Anthropic Claude',
      kind: 'repository',
      data: sampleRepoContext,
    };

    render(
      <CaptureContextTab
        context={sourceContext}
        loading={false}
        analyzing={false}
        onAnalyze={mockOnAnalyze}
      />,
    );

    // 1. Objective & Repository Name
    expect(screen.getByText('Objective')).toBeDefined();
    expect(screen.getByText('stablyai/orca')).toBeDefined();
    expect(screen.getByText(sampleRepoContext.objective)).toBeDefined();

    // 2. Stack (evidenced technical signals)
    expect(screen.getByText('Stack')).toBeDefined();
    expect(screen.getByText('Git / Git worktrees')).toBeDefined();
    expect(screen.getByText('WebGL terminal rendering')).toBeDefined();
    expect(screen.getByText('Chromium/browser integration')).toBeDefined();

    // 3. Features / Ecosystem (product & ecosystem capabilities)
    expect(screen.getByText('Features / Ecosystem')).toBeDefined();
    expect(screen.getByText('Parallel coding-agent orchestration')).toBeDefined();
    expect(screen.getByText('Supports a broad range of CLI coding agents')).toBeDefined();

    // 4. User Base
    expect(screen.getByText('User Base')).toBeDefined();
    expect(screen.getByText('Software developers')).toBeDefined();
    expect(screen.getByText('AI-assisted developers')).toBeDefined();

    // 5. Licensing
    expect(screen.getByText('Licensing')).toBeDefined();
    expect(screen.getByText('MIT License')).toBeDefined();

    // INVARIANT: Open Issues, Past Issues, and issue placeholders must NOT exist
    expect(screen.queryByText(/Open Issues/i)).toBeNull();
    expect(screen.queryByText(/Past Issues/i)).toBeNull();
    expect(screen.queryByText(/No issue/i)).toBeNull();
    expect(screen.queryByText(/No historical/i)).toBeNull();

    // INVARIANT: Conversation-only concepts must NOT appear
    expect(screen.queryByText(/Key Decisions Made/i)).toBeNull();
    expect(screen.queryByText(/Constraints & Boundaries/i)).toBeNull();
    expect(screen.queryByText(/Rejected Approaches/i)).toBeNull();
    expect(screen.queryByText(/Next Actions/i)).toBeNull();
  });

  it('renders ConversationContext with conversation dimensions and no repository Stack heading', () => {
    const sourceContext: SourceContext = {
      capture_id: 'cap_chat',
      generated_at: '2026-09-04T00:00:00Z',
      deterministic: false,
      model: 'OpenAI GPT-4',
      kind: 'conversation',
      data: sampleConversationContext,
    };

    render(
      <CaptureContextTab
        context={sourceContext}
        loading={false}
        analyzing={false}
        onAnalyze={mockOnAnalyze}
      />,
    );

    expect(screen.getByText('Objective')).toBeDefined();
    expect(screen.getByText(/Key Decisions Made \(1\)/)).toBeDefined();
    expect(screen.getByText('Use SQLite')).toBeDefined();
    expect(screen.getByText(/Constraints & Boundaries/)).toBeDefined();
    expect(screen.getByText('Zero telemetry')).toBeDefined();
    expect(screen.getByText(/Next Actions/)).toBeDefined();
    expect(screen.getByText('Write unit tests')).toBeDefined();

    // INVARIANT: Repository Stack heading must NOT appear
    expect(screen.queryByText('Stack')).toBeNull();
    expect(screen.queryByText('Features / Ecosystem')).toBeNull();
    expect(screen.queryByText('Licensing')).toBeNull();
  });
});
