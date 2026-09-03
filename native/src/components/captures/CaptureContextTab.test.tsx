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
    objective: 'Orca is an agentic workspace built with Rust, Tauri, and React.',
    stack: {
      languages: ['Rust', 'TypeScript'],
      frontend: ['React', 'Vite'],
      backend: ['Tauri'],
      storage: ['SQLite'],
      testing: ['Vitest'],
      integrations: ['GitHub API'],
    },
    features: [
      { name: 'Universal voice dictation', description: 'Real-time whisper', is_core: true },
      { name: 'Local memory vault', description: 'SQLite-backed knowledge', is_core: true },
      { name: 'Debug logging', description: 'Internal diagnostics', is_core: false },
    ],
    user_base: {
      primary: ['Developers', 'AI Researchers'],
      secondary: ['Open-Source Contributors'],
      evidence: 'Grounded in developer workflows section of README',
    },
    open_issues: [],
    past_issues: [],
    open_issues_available: false,
    past_issues_available: false,
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
    rejected_approaches: [
      { approach: 'Cloud sync', reason_rejected: 'Privacy risk', source_turn_ordinals: [2] },
    ],
    open_questions: [],
    action_items: [
      { id: 'act_1', description: 'Write unit tests', status: 'OPEN', source_turn_ordinals: [3] },
    ],
    important_facts: [],
    key_artifacts: [],
    generated_at: '2026-09-04T00:00:00Z',
    model: 'OpenAI GPT-4',
    deterministic: false,
  };

  // `application` is `"GitHub"` because that is what `capture/web/source.rs`
  // actually writes. The empty state must key off `capture_type`, which is
  // the classification Relay already derived from the URL, not off a
  // case-sensitive name match the detector never produces.
  const gitHubProvenanceFixture: CaptureProvenance = {
    source_type: 'web',
    capture_type: 'repository',
    application: 'GitHub',
    domain: 'github.com',
    url: 'https://github.com/stablyai/orca',
    page_title: 'stablyai/orca',
    captured_at: '2026-09-03T10:00:00Z',
    extractor_id: 'github',
    extractor_version: 1,
    coverage: 'full',
    fidelity: 'structured',
    trust: 'external_untrusted',
    notes: [],
    block_count: 12,
    skipped_block_count: 0,
    truncated: false,
    version: 1,
    recapture_count: 0,
  };

  it('renders GitHub-specific empty state with "Extract Repository Context" button', () => {
    const gitHubProvenance = gitHubProvenanceFixture;

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

  it('renders RepositoryContext with repository dimensions and no conversation headings', () => {
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

    // Objective & Repository Name
    expect(screen.getByText('Objective')).toBeDefined();
    expect(screen.getByText('stablyai/orca')).toBeDefined();
    expect(screen.getByText(sampleRepoContext.objective)).toBeDefined();

    // Stack sections
    expect(screen.getByText('Stack')).toBeDefined();
    expect(screen.getByText('Rust')).toBeDefined();
    expect(screen.getByText('TypeScript')).toBeDefined();
    expect(screen.getByText('React')).toBeDefined();
    expect(screen.getByText('Tauri')).toBeDefined();

    // Features
    expect(screen.getByText(/Features \(3\)/)).toBeDefined();
    expect(screen.getByText('Universal voice dictation')).toBeDefined();
    expect(screen.getAllByText('Core').length).toBe(2);
    expect(screen.getByText('Supporting')).toBeDefined();

    // User Base
    expect(screen.getByText('User Base')).toBeDefined();
    expect(screen.getByText('Developers')).toBeDefined();

    // Honest missing issues notices. The fixture has `*_available: false`, so
    // both sections must report absent evidence, never an empty result.
    expect(
      screen.getByText('Issue information was not available in the captured repository evidence.'),
    ).toBeDefined();
    expect(
      screen.getByText('No historical issue information was available in the captured repository evidence.'),
    ).toBeDefined();
    expect(screen.queryByText(/No open issues in the captured evidence/i)).toBeNull();

    // INVARIANT: Conversation-only concepts must NOT appear
    expect(screen.queryByText(/Key Decisions Made/i)).toBeNull();
    expect(screen.queryByText(/Constraints & Boundaries/i)).toBeNull();
    expect(screen.queryByText(/Rejected Approaches/i)).toBeNull();
    expect(screen.queryByText(/Next Actions/i)).toBeNull();
  });

  it('keys the repository empty state off capture_type, not the URL or application name', () => {
    // A page that merely mentions github.com in a query string. The old check
    // matched this substring and offered to extract "repository context" from
    // an arbitrary web page.
    const lookalike: CaptureProvenance = {
      ...gitHubProvenanceFixture,
      capture_type: 'page',
      application: 'evil.example',
      domain: 'evil.example',
      url: 'https://evil.example/?ref=github.com',
      page_title: 'Not a repository',
    };

    render(
      <CaptureContextTab
        context={null}
        provenance={lookalike}
        loading={false}
        analyzing={false}
        onAnalyze={mockOnAnalyze}
      />,
    );

    expect(screen.queryByRole('button', { name: /extract repository context/i })).toBeNull();
    expect(screen.getByRole('button', { name: /extract structured context/i })).toBeDefined();
  });

  it('distinguishes a repository with no open issues from one whose issues were never captured', () => {
    const knownEmpty: SourceContext = {
      capture_id: 'cap_orca',
      generated_at: '2026-09-04T00:00:00Z',
      deterministic: false,
      model: 'Anthropic Claude',
      kind: 'repository',
      data: {
        ...sampleRepoContext,
        // Relay saw the issue evidence. There was nothing in it. That is a
        // fact about the repository, not a gap in the capture.
        open_issues: [],
        open_issues_available: true,
        past_issues: [],
        past_issues_available: false,
      },
    };

    render(
      <CaptureContextTab
        context={knownEmpty}
        loading={false}
        analyzing={false}
        onAnalyze={mockOnAnalyze}
      />,
    );

    expect(screen.getByText('No open issues in the captured evidence.')).toBeDefined();
    expect(
      screen.queryByText('Issue information was not available in the captured repository evidence.'),
    ).toBeNull();

    // The other section is genuinely unavailable and must still say so.
    expect(
      screen.getByText('No historical issue information was available in the captured repository evidence.'),
    ).toBeDefined();
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
  });
});
