/**
 * Presentation helpers for captures.
 *
 * Pure functions, kept out of the components so the parts that carry meaning
 * — what a capture is allowed to claim about its own completeness, and how
 * that is worded to a user — can be tested directly.
 */

import type { CaptureProvenance, VaultFile } from '../../types';

/** Human label for Relay's capture types. */
export function captureTypeLabel(captureType: string): string {
  switch (captureType) {
    case 'conversation':
      return 'Conversation';
    case 'article':
      return 'Article';
    case 'repository':
      return 'Repository';
    case 'issue':
      return 'Issue';
    case 'pull_request':
      return 'Pull request';
    case 'discussion':
      return 'Discussion';
    case 'code':
      return 'Code';
    case 'page':
      return 'Page';
    default:
      return captureType.replace(/_/g, ' ');
  }
}

export type CompletenessTone = 'complete' | 'partial' | 'unknown';

export interface Completeness {
  tone: CompletenessTone;
  /** One line the user can act on, never a technical term. */
  headline: string;
}

/**
 * Turns coverage and fidelity into a claim the artifact can stand behind.
 *
 * The rule this encodes is the product's central promise: a capture only says
 * "the whole page" when the extractor had evidence of it. Everything else
 * reads as a limitation, because it is one.
 */
export function describeCompleteness(provenance: CaptureProvenance): Completeness {
  if (provenance.truncated || provenance.coverage === 'partial') {
    return {
      tone: 'partial',
      headline: 'Part of this page was left out',
    };
  }
  switch (provenance.coverage) {
    case 'full_document':
      return { tone: 'complete', headline: 'The whole page was captured' };
    case 'rendered_dom':
      return {
        tone: 'partial',
        headline: 'Only what the page had loaded was captured',
      };
    default:
      return { tone: 'unknown', headline: 'How much was captured is not known' };
  }
}

/** Human label for how the content was obtained. */
export function fidelityLabel(fidelity: string): string {
  switch (fidelity) {
    case 'structured':
      return 'Structured extraction';
    case 'generic':
      return 'Article extraction';
    case 'text_only':
      return 'Visible text only';
    default:
      return fidelity;
  }
}

/** `https://chatgpt.com/c/abc?x=1` → `chatgpt.com/c/abc`, for compact display. */
export function displayUrl(url: string): string {
  try {
    const parsed = new URL(url);
    const path = parsed.pathname === '/' ? '' : parsed.pathname;
    return `${parsed.host}${path}`;
  } catch {
    return url;
  }
}

export function formatTimestamp(iso: string): string {
  const date = new Date(iso);
  if (Number.isNaN(date.getTime())) return iso;
  return date.toLocaleString(undefined, {
    dateStyle: 'medium',
    timeStyle: 'short',
  });
}

/** Matches a capture against a free-text query over its content and provenance. */
export function matchesQuery(capture: VaultFile, query: string): boolean {
  const trimmed = query.trim().toLowerCase();
  if (!trimmed) return true;
  const haystack = [
    capture.original_filename,
    capture.summary ?? '',
    capture.content,
    capture.tags.join(' '),
    capture.capture?.application ?? '',
    capture.capture?.domain ?? '',
    capture.capture?.url ?? '',
    capture.capture ? captureTypeLabel(capture.capture.capture_type) : '',
  ]
    .join(' ')
    .toLowerCase();
  return trimmed.split(/\s+/).every((term) => haystack.includes(term));
}
