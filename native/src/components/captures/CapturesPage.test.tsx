import React from 'react';
import { beforeEach, describe, expect, test, vi } from 'vitest';
import { render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { invoke } from '@tauri-apps/api/core';
import { CapturesPage } from './CapturesPage';
import type { CaptureBridgeStatus, VaultFile } from '../../types';

function capture(overrides: Partial<VaultFile> = {}): VaultFile {
  return {
    id: 'capture_1',
    original_filename: 'Designing Relay Capture',
    file_type: 'webcapture',
    mime_type: 'application/json',
    size_bytes: 4096,
    content_hash: 'abc',
    created_at: '2026-02-14T09:30:00Z',
    updated_at: '2026-02-14T09:30:00Z',
    last_known_source_path: 'https://chatgpt.com/c/abc',
    vault_path: 'captures/capture_1/original/Designing-Relay-Capture.json',
    extraction_status: 'extracted',
    processing_status: 'ready',
    content: '# Designing Relay Capture\n\n## USER\n\nHow should capture work?',
    summary: 'A conversation about structured acquisition.',
    tags: ['architecture'],
    topics: ['architecture'],
    entities: ['Relay'],
    relationships: [],
    ai_metadata: { enrichment_status: 'enriched', suggested_concepts: [], suggested_questions: [], suggested_relations: [] },
    capture: {
      source_type: 'web',
      capture_type: 'conversation',
      application: 'ChatGPT',
      domain: 'chatgpt.com',
      url: 'https://chatgpt.com/c/abc',
      page_title: 'Designing Relay Capture',
      captured_at: '2026-02-14T09:30:00Z',
      extractor_id: 'chatgpt',
      extractor_version: 1,
      fidelity: 'structured',
      coverage: 'rendered_dom',
      notes: ['Only the turns the page had rendered were captured.'],
      message_count: 2,
      block_count: 3,
      skipped_block_count: 0,
      truncated: false,
      version: 1,
      recapture_count: 0,
    },
    ...overrides,
  } as VaultFile;
}

const bridgeOn: CaptureBridgeStatus = {
  enabled: true,
  running: true,
  port: 8765,
  configured_port: 8765,
  pairing_token: 'token',
  protocol_version: 1,
  analyze_on_capture: true,
  capture_hotkey: 'Ctrl+Shift+C',
};

function mockBackend(captures: VaultFile[], status: CaptureBridgeStatus = bridgeOn) {
  vi.mocked(invoke).mockImplementation(((command: string) => {
    if (command === 'get_captures') return Promise.resolve(captures);
    if (command === 'get_capture_bridge_status') return Promise.resolve(status);
    if (command === 'delete_capture') return Promise.resolve(undefined);
    if (command === 'get_settings') {
      return Promise.resolve({ hotkeys: { dictation_hotkey: 'Ctrl+Shift+D' } });
    }
    if (command === 'create_scribble') {
      return Promise.resolve({ id: 'scr_9', title: 'A captured thought' });
    }
    return Promise.resolve(undefined);
  }) as typeof invoke);
}

describe('CapturesPage', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  test('lists captures with the application, kind and source they came from', async () => {
    mockBackend([capture()]);
    render(<CapturesPage />);

    expect(await screen.findByText('Designing Relay Capture')).toBeInTheDocument();
    expect(screen.getByText('ChatGPT')).toBeInTheDocument();
    expect(screen.getByText('Conversation')).toBeInTheDocument();
    expect(screen.getByText('chatgpt.com/c/abc')).toBeInTheDocument();
  });

  test('says plainly when only part of a page was captured', async () => {
    mockBackend([capture()]);
    render(<CapturesPage />);

    expect(
      await screen.findByText('Only what Relay could reach was captured'),
    ).toBeInTheDocument();
  });

  test('makes no completeness claim badge when the whole page was captured', async () => {
    const complete = capture();
    complete.capture!.coverage = 'full_document';
    mockBackend([complete]);
    render(<CapturesPage />);

    await screen.findByText('Designing Relay Capture');
    expect(screen.queryByText(/only what the page had loaded/i)).not.toBeInTheDocument();
  });

  test('filters by content, site and tag', async () => {
    const user = userEvent.setup();
    mockBackend([capture(), capture({ id: 'capture_2', original_filename: 'A GitHub issue' })]);
    render(<CapturesPage />);

    await screen.findByText('A GitHub issue');
    await user.type(screen.getByLabelText('Search captures'), 'github');

    await waitFor(() => {
      expect(screen.queryByText('Designing Relay Capture')).not.toBeInTheDocument();
    });
    expect(screen.getByText('A GitHub issue')).toBeInTheDocument();
  });

  test('warns and offers a way back when browser capture is switched off', async () => {
    const onOpenCaptureSettings = vi.fn();
    mockBackend([], { ...bridgeOn, enabled: false, running: false });
    render(<CapturesPage onOpenCaptureSettings={onOpenCaptureSettings} />);

    expect(
      await screen.findByText(/browser capture is off/i),
    ).toBeInTheDocument();

    await userEvent.setup().click(screen.getByRole('button', { name: 'Turn it on' }));
    expect(onOpenCaptureSettings).toHaveBeenCalled();
  });

  test('explains how to capture when nothing has been captured yet', async () => {
    mockBackend([]);
    render(<CapturesPage />);

    expect(await screen.findByText('Nothing captured yet')).toBeInTheDocument();
    expect(screen.getByText(/browser extension/i)).toBeInTheDocument();
  });

  test('moves a capture to Trash only after confirmation', async () => {
    const user = userEvent.setup();
    mockBackend([capture()]);
    render(<CapturesPage />);

    await screen.findByText('Designing Relay Capture');
    await user.click(
      screen.getByRole('button', { name: 'Delete capture Designing Relay Capture' }),
    );

    expect(await screen.findByText('Move this capture to Trash?')).toBeInTheDocument();
    expect(vi.mocked(invoke)).not.toHaveBeenCalledWith('delete_capture', expect.anything());

    await user.click(screen.getByRole('button', { name: 'Move to Trash' }));
    await waitFor(() => {
      expect(vi.mocked(invoke)).toHaveBeenCalledWith('delete_capture', { id: 'capture_1' });
    });
  });
  test('lands on the captured pages, and opens the capture modes on request', async () => {
    const user = userEvent.setup();
    mockBackend([capture()]);
    render(<CapturesPage />);

    // Arriving from the sidebar shows what was captured, not a compose box.
    await screen.findByText('Designing Relay Capture');
    expect(screen.queryByText('Type a thought directly')).not.toBeInTheDocument();

    await user.click(screen.getByRole('button', { name: 'Capture' }));
    expect(await screen.findByText('Type a thought directly')).toBeInTheDocument();
    expect(screen.queryByText('Designing Relay Capture')).not.toBeInTheDocument();
  });

  test('a capture mode asked for elsewhere opens on that mode', async () => {
    mockBackend([capture()]);
    render(<CapturesPage initialCaptureMethod="clipboard" />);

    expect(await screen.findByText('Paste from Clipboard')).toBeInTheDocument();
  });

  test('the modes another surface owns navigate there rather than duplicating it', async () => {
    const user = userEvent.setup();
    const onNavigateTab = vi.fn();
    mockBackend([]);
    render(<CapturesPage initialCaptureMethod="text" onNavigateTab={onNavigateTab} />);

    await user.click(await screen.findByText('Files & Docs'));
    expect(onNavigateTab).toHaveBeenCalledWith('files');

    await user.click(screen.getByText('Voice'));
    expect(onNavigateTab).toHaveBeenCalledWith('capture');

    await user.click(screen.getByText('Meeting'));
    expect(onNavigateTab).toHaveBeenCalledWith('meetings');
  });

  test('the web capture card switches to the pages this surface already holds', async () => {
    const user = userEvent.setup();
    mockBackend([capture()]);
    render(<CapturesPage initialCaptureMethod="text" />);

    await user.click(await screen.findByText('Web Capture'));
    expect(await screen.findByText('Designing Relay Capture')).toBeInTheDocument();
  });

  test('a thought captured here can be opened in Scribbles', async () => {
    const user = userEvent.setup();
    const onOpenScribble = vi.fn();
    mockBackend([]);
    render(<CapturesPage initialCaptureMethod="text" onOpenScribble={onOpenScribble} />);

    await user.type(
      await screen.findByPlaceholderText(/Capture an observation/),
      'Retrieval is the bottleneck',
    );
    await user.click(screen.getByRole('button', { name: /Save to Knowledge Layer/ }));

    await user.click(await screen.findByRole('button', { name: /Open in Scribbles/ }));
    expect(onOpenScribble).toHaveBeenCalledWith('scr_9');
  });
});
