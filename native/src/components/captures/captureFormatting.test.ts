import { describe, expect, it } from 'vitest';
import {
  captureTypeLabel,
  describeCompleteness,
  describeTraversal,
  displayUrl,
  fidelityLabel,
  matchesQuery,
  terminationLabel,
  trustLabel,
} from './captureFormatting';
import type { CaptureProvenance, CaptureTraversal, VaultFile } from '../../types';

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
    extractor_version: 2,
    trust: 'external_untrusted',
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
    expect(result.headline).toMatch(/only what Relay could reach/i);
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

/**
 * Capture v2: the wording that turns measurements into a claim.
 *
 * The product's central promise lives in these strings. v0.26.0 told a user
 * "the whole page was captured" about a Claude conversation with a visibly
 * shortened message in it, so these tests assert both directions: what the UI
 * may say, and what it may not.
 */
function traversal(overrides: Partial<CaptureTraversal> = {}): CaptureTraversal {
  return {
    performed: true,
    plan: 'chatgpt',
    termination: 'reached_end',
    steps: 74,
    samples: 76,
    scroll_span_px: 65_000,
    duration_ms: 6_000,
    scroll_restored: true,
    virtualized: true,
    settle_timeouts: 0,
    expansions_found: 0,
    expansions_opened: 0,
    expansions_refused: 0,
    expansions_failed: 0,
    expansions_unnecessary: 0,
    messages_discovered: 300,
    messages_captured: 300,
    messages_missing: 0,
    duplicates_dropped: 228,
    attachments_discovered: 0,
    attachments_captured: 0,
    images_discovered: 0,
    images_captured: 0,
    availability: {
      outside_viewport: 0,
      visually_truncated: 0,
      collapsed: 0,
      not_loaded: 0,
      virtualized: 0,
      inaccessible: 0,
    },
    inaccessible: [],
    ...overrides,
  };
}

describe('describeCompleteness with a reveal pass', () => {
  it('says what was read rather than what was displayed', () => {
    const result = describeCompleteness(
      provenance({ coverage: 'full_document', traversal: traversal() }),
    );
    expect(result.tone).toBe('complete');
    expect(result.headline).toMatch(/read this from beginning to end/i);
  });

  it('does not claim the whole page when reading did not finish', () => {
    const result = describeCompleteness(provenance({ coverage: 'failed' }));
    expect(result.tone).toBe('partial');
    expect(result.headline).toMatch(/did not finish/i);
  });

  it('keeps the old wording for a capture made before there was a reveal pass', () => {
    const result = describeCompleteness(provenance({ coverage: 'full_document' }));
    expect(result.headline).toBe('The whole page was captured');
  });
});

describe('describeTraversal', () => {
  it('reports the discovered-versus-captured pair', () => {
    const lines = describeTraversal(provenance({ traversal: traversal() }));
    expect(lines.join(' | ')).toMatch(/300 of 300 turn\(s\) captured/);
    expect(lines.join(' | ')).toMatch(/reached the end of the page/);
  });

  it('says when shortened content was read without being clicked', () => {
    // The Claude case, in the user's language.
    const lines = describeTraversal(
      provenance({
        traversal: traversal({ expansions_found: 2, expansions_unnecessary: 2 }),
      }),
    );
    expect(lines.join(' | ')).toMatch(/already present in full, so nothing was clicked/i);
  });

  it('reports files and images as recorded, never as downloaded', () => {
    const lines = describeTraversal(
      provenance({ traversal: traversal({ attachments_discovered: 2, images_discovered: 3 }) }),
    ).join(' | ');
    expect(lines).toMatch(/2 file\(s\) recorded — details only, no contents downloaded/);
    expect(lines).toMatch(/3 image\(s\) recorded — references only, no image data downloaded/);
  });

  it('omits a number the browser could not supply rather than showing a zero', () => {
    const lines = describeTraversal(provenance({ traversal: traversal() })).join(' | ');
    expect(lines).not.toMatch(/0 file\(s\)/);
    expect(lines).not.toMatch(/0 shortened section/);
    expect(lines).not.toMatch(/0 turn\(s\) missing/);
  });

  it('returns nothing at all when no reveal pass ran', () => {
    expect(describeTraversal(provenance({}))).toEqual([]);
    expect(describeTraversal(provenance({ traversal: traversal({ performed: false }) }))).toEqual([]);
  });

  it('names the reason reading stopped, in plain language', () => {
    for (const [termination, pattern] of [
      ['time_budget', /time limit/i],
      ['user_interrupted', /page was used/i],
      ['navigation_detected', /navigated away/i],
      ['no_progress', /stopped yielding/i],
      ['error', /failed part-way/i],
    ] as const) {
      expect(terminationLabel(termination)).toMatch(pattern);
    }
  });
});

describe('trustLabel', () => {
  it('says a capture is evidence rather than instructions', () => {
    expect(trustLabel('external_untrusted')).toMatch(/evidence, not instructions/i);
  });

  it('treats a capture from before the field existed as external too', () => {
    expect(trustLabel(undefined)).toMatch(/external source/i);
  });
});
