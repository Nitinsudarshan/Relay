import { describe, expect, it } from 'vitest';
import {
  captureTypeLabel,
  describeCompleteness,
  displayUrl,
  fidelityLabel,
  matchesQuery,
} from './captureFormatting';
import type { CaptureProvenance, VaultFile } from '../../types';

function provenance(overrides: Partial<CaptureProvenance> = {}): CaptureProvenance {
  return {
    source_type: 'web',
    capture_type: 'conversation',
    application: 'ChatGPT',
    domain: 'chatgpt.com',
    url: 'https://chatgpt.com/c/abc',
    page_title: 'Designing capture',
    captured_at: '2026-02-14T09:30:00Z',
    extractor_id: 'chatgpt',
    extractor_version: 1,
    fidelity: 'structured',
    coverage: 'rendered_dom',
    notes: [],
    block_count: 4,
    skipped_block_count: 0,
    truncated: false,
    version: 1,
    recapture_count: 0,
    ...overrides,
  };
}

describe('describeCompleteness', () => {
  it('claims a whole page only when coverage says so', () => {
    expect(describeCompleteness(provenance({ coverage: 'full_document' }))).toEqual({
      tone: 'complete',
      headline: 'The whole page was captured',
    });
  });

  it('says plainly when only the rendered part was read', () => {
    const result = describeCompleteness(provenance({ coverage: 'rendered_dom' }));
    expect(result.tone).toBe('partial');
    expect(result.headline).toMatch(/only what the page had loaded/i);
  });

  it('treats truncation as partial even if coverage claims otherwise', () => {
    const result = describeCompleteness(
      provenance({ coverage: 'full_document', truncated: true }),
    );
    expect(result.tone).toBe('partial');
    expect(result.headline).toMatch(/left out/i);
  });

  it('admits when completeness is unknown', () => {
    expect(describeCompleteness(provenance({ coverage: 'unknown' })).tone).toBe('unknown');
  });
});

describe('labels', () => {
  it('names every capture type Relay produces', () => {
    expect(captureTypeLabel('pull_request')).toBe('Pull request');
    expect(captureTypeLabel('conversation')).toBe('Conversation');
    expect(captureTypeLabel('something_new')).toBe('something new');
  });

  it('explains fidelity without jargon', () => {
    expect(fidelityLabel('structured')).toBe('Structured extraction');
    expect(fidelityLabel('text_only')).toBe('Visible text only');
  });
});

describe('displayUrl', () => {
  it('drops the scheme and query for display', () => {
    expect(displayUrl('https://chatgpt.com/c/abc?utm=1')).toBe('chatgpt.com/c/abc');
    expect(displayUrl('https://example.com/')).toBe('example.com');
  });

  it('leaves an unparseable url alone', () => {
    expect(displayUrl('not a url')).toBe('not a url');
  });
});

describe('matchesQuery', () => {
  const capture = {
    original_filename: 'Designing capture',
    summary: 'A conversation about structured acquisition.',
    content: '## USER\nHow should capture work?',
    tags: ['architecture'],
    capture: provenance(),
  } as unknown as VaultFile;

  it('matches on content, provenance and tags', () => {
    expect(matchesQuery(capture, 'chatgpt')).toBe(true);
    expect(matchesQuery(capture, 'architecture')).toBe(true);
    expect(matchesQuery(capture, 'acquisition')).toBe(true);
    expect(matchesQuery(capture, 'conversation')).toBe(true);
  });

  it('requires every term to match', () => {
    expect(matchesQuery(capture, 'chatgpt architecture')).toBe(true);
    expect(matchesQuery(capture, 'chatgpt kubernetes')).toBe(false);
  });

  it('treats an empty query as matching everything', () => {
    expect(matchesQuery(capture, '   ')).toBe(true);
  });
});
