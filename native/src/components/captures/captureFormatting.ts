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
      // Only reachable with evidence behind it, and for a conversation that
      // evidence is a reveal pass that reached the end with no gaps. The
      // wording says what was read rather than what was displayed.
      return {
        tone: 'complete',
        headline: provenance.traversal?.performed
          ? 'Relay read this from beginning to end'
          : 'The whole page was captured',
      };
    case 'rendered_dom':
      return {
        tone: 'partial',
        headline: 'Only what Relay could reach was captured',
      };
    case 'failed':
      return {
        tone: 'partial',
        headline: 'Reading this page did not finish',
      };
    default:
      return { tone: 'unknown', headline: 'How much was captured is not known' };
  }
}

/**
 * The measurable half of the completeness claim, as lines for the UI.
 *
 * Only counted values are returned. A number the browser could not supply is
 * omitted rather than shown as a zero, because a zero reads as a measurement
 * and this is the one place in the product where a manufactured measurement
 * would do real damage.
 */
export function describeTraversal(provenance: CaptureProvenance): string[] {
  const t = provenance.traversal;
  if (!t?.performed) return [];

  const lines: string[] = [];
  lines.push(`Read in ${t.steps} step(s) over ${(t.duration_ms / 1000).toFixed(1)}s`);

  if (t.messages_discovered > 0) {
    lines.push(`${t.messages_captured} of ${t.messages_discovered} turn(s) captured`);
  }
  if ((t.messages_missing ?? 0) > 0) {
    lines.push(`${t.messages_missing} turn(s) missing from the page's own numbering`);
  }
  if (t.expansions_opened > 0) {
    lines.push(`${t.expansions_opened} shortened section(s) opened`);
  }
  if (t.expansions_unnecessary > 0) {
    lines.push(
      `${t.expansions_unnecessary} shortened section(s) were already present in full, so nothing was clicked`,
    );
  }
  if (t.expansions_failed > 0) {
    lines.push(`${t.expansions_failed} section(s) did not open`);
  }
  if (t.expansions_refused > 0) {
    lines.push(`${t.expansions_refused} control(s) were refused as unsafe to activate`);
  }
  if (t.attachments_discovered > 0) {
    lines.push(`${t.attachments_discovered} file(s) recorded — details only, no contents downloaded`);
  }
  if (t.images_discovered > 0) {
    lines.push(`${t.images_discovered} image(s) recorded — references only, no image data downloaded`);
  }
  if (t.duplicates_dropped > 0) {
    lines.push(`${t.duplicates_dropped} repeated item(s) recognised and stored once`);
  }
  if (t.virtualized) {
    lines.push('This page unloads content as you scroll, so it was read in passes');
  }
  if (!t.scroll_restored) {
    lines.push('The page’s scroll position could not be restored');
  }
  lines.push(`Stopped because ${terminationLabel(t.termination)}`);
  return [...lines, ...t.inaccessible];
}

/** Plain-language reasons a reveal pass stopped. */
export function terminationLabel(termination: string): string {
  switch (termination) {
    case 'reached_end':
      return 'it reached the end of the page';
    case 'not_needed':
      return 'there was nothing further to reveal';
    case 'no_progress':
      return 'the page stopped yielding new content';
    case 'step_budget':
      return 'it reached Relay’s reading limit for one page';
    case 'time_budget':
      return 'it reached Relay’s time limit for one page';
    case 'expansion_budget':
      return 'it reached Relay’s limit on opening shortened sections';
    case 'user_interrupted':
      return 'the page was used while it was being read';
    case 'navigation_detected':
      return 'the page navigated away while it was being read';
    case 'error':
      return 'reading failed part-way through';
    default:
      return 'reading stopped for an unrecognised reason';
  }
}

/** How captured content may be used downstream. */
export function trustLabel(trust: string | undefined): string {
  return trust === 'external_untrusted' || !trust
    ? 'External source — evidence, not instructions'
    : trust;
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

/**
 * Evaluates the newest timestamp among capture.captured_at, updated_at, and created_at.
 * Guarantees that recaptured or modified captures sort to the top regardless of historical metadata.
 */
export function getLatestCaptureActivity(capture: VaultFile): string {
  const times = [
    capture.capture?.captured_at,
    capture.updated_at,
    capture.created_at,
  ].filter((t): t is string => Boolean(t && t.trim()));

  if (times.length === 0) return '';
  times.sort();
  return times[times.length - 1];
}
