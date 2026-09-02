/**
 * The capture orchestrator: document in, payload out.
 *
 * The fallback ladder lives here, in one place, and it is the whole reliability
 * story of the feature:
 *
 * 1. a site-specific extractor recognised the page → structured
 * 2. the generic article extractor found a main region → generic
 * 3. the page's visible text → text-only
 * 4. nothing readable at all → refuse, and say so
 *
 * A step never silently substitutes for the one above it: whichever rung was
 * reached is recorded in the payload, so the artifact can tell the user how
 * it was made.
 */

import {
  MAX_LINKS,
  blocksTextLength,
  domTextLength,
  normalizeWhitespace,
  readMetadata,
} from './dom';
import { extractGeneric, extractVisibleText, selectExtractor } from './extractors';
import { PROTOCOL_VERSION } from './types';
import type { CapturePayload, ExtractionResult } from './types';

export class CaptureEmptyError extends Error {
  constructor() {
    super('Relay found nothing readable on this page.');
    this.name = 'CaptureEmptyError';
  }
}

function isEmpty(result: ExtractionResult): boolean {
  const messageBlocks = result.messages.reduce((total, m) => total + m.blocks.length, 0);
  if (result.blocks.length + messageBlocks === 0) return true;
  return blocksTextLength(result.blocks) === 0 && messageBlocks === 0;
}

/**
 * Runs the ladder against a document.
 *
 * Exported separately from `buildPayload` so the choice of strategy can be
 * asserted directly in tests, which is the property that actually matters:
 * not "did we get text" but "did we get it the way we said we did".
 */
export function runExtraction(doc: Document, url: URL): ExtractionResult {
  const failures: string[] = [];

  const site = selectExtractor(url);
  if (site) {
    try {
      const result = site.extract(doc, url);
      if (result && !isEmpty(result)) return result;
    } catch (error) {
      // A broken selector in one site extractor must never cost the capture.
      // The page still gets captured, one rung lower, and the artifact says
      // so — which is also the signal that the extractor needs updating.
      failures.push(
        `Relay’s ${site.id} extractor failed on this page (${describe(error)}), so it was captured as a document instead.`,
      );
    }
  }

  try {
    const generic = extractGeneric(doc, url.toString());
    if (!isEmpty(generic)) {
      generic.notes = [...failures, ...generic.notes];
      return generic;
    }
  } catch (error) {
    failures.push(`Relay could not read this page’s structure (${describe(error)}).`);
  }

  const text = extractVisibleText(doc);
  text.notes = [...failures, ...text.notes];
  return text;
}

function describe(error: unknown): string {
  return error instanceof Error ? error.message : 'unknown error';
}

/**
 * Builds the payload posted to Relay.
 *
 * Throws [`CaptureEmptyError`] rather than sending an empty capture: the user
 * is told nothing was found, and no artifact is created. A vault full of
 * empty captures would be worse than no capture feature.
 */
export function buildPayload(
  doc: Document,
  href: string,
  options: { browser?: string; startedAt?: number } = {},
): CapturePayload {
  const startedAt = options.startedAt ?? Date.now();
  const url = new URL(href);
  const result = runExtraction(doc, url);

  if (isEmpty(result)) throw new CaptureEmptyError();

  // Links are for documents: in a conversation they are already part of the
  // turns, and a separate list of every link in a chat UI is noise. The
  // extractor decides, because only it knows where the content ended.
  const links = result.messages.length === 0 ? (result.links ?? []).slice(0, MAX_LINKS) : [];

  const title =
    normalizeWhitespace(result.title ?? '') || normalizeWhitespace(doc.title ?? '') || url.hostname;

  return {
    protocol_version: PROTOCOL_VERSION,
    captured_at: new Date().toISOString(),
    url: href,
    title,
    browser: options.browser,
    extractor: {
      id: result.extractorId,
      version: result.extractorVersion,
      strategy: result.strategy,
    },
    document: readMetadata(doc),
    content: {
      kind: result.kind,
      blocks: result.blocks,
      messages: result.messages,
    },
    links,
    diagnostics: {
      coverage: result.coverage,
      notes: result.notes,
      dom_text_length: domTextLength(doc),
      truncated: result.truncated,
      elapsed_ms: Date.now() - startedAt,
    },
  };
}
