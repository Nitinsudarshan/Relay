/**
 * The capture orchestrator: document in, payload out.
 *
 * Two things live here, and only here.
 *
 * **The acquisition order.** Inspect, reveal only what needs revealing,
 * extract, deduplicate, reconstruct, then judge completeness. The least
 * invasive mechanism that can obtain the content is the one used: content
 * already in the DOM is read, content that is merely clipped is read, and only
 * content that is genuinely absent earns an interaction.
 *
 * **The fallback ladder**, which is the whole reliability story of the feature:
 *
 * 1. a site-specific extractor recognised the page → structured
 * 2. the generic article extractor found a main region → generic
 * 3. the page's visible text → text-only
 * 4. nothing readable at all → refuse, and say so
 *
 * A step never silently substitutes for the one above it: whichever rung was
 * reached is recorded in the payload, so the artifact can tell the user how it
 * was made — and, since v2, how completely.
 */

import {
  MAX_LINKS,
  assessCoverage,
  blocksTextLength,
  countRichBlocks,
  domTextLength,
  extractableTextLength,
  normalizeWhitespace,
  readMetadata,
} from './dom';
import {
  extractGeneric,
  extractVisibleText,
  harvestGeneric,
  selectExtractor,
} from './extractors';
import { BlockMerger, SampleMerger } from './merge';
import { traverse } from './traversal/engine';
import { GENERIC_TRAVERSAL } from './traversal/plans';
import { resolveSurface } from './traversal/surface';
import type { ScrollSurface, TraversalPlan } from './traversal/types';
import { PROTOCOL_VERSION, emptyTraversalDiagnostics } from './types';
import type {
  CaptureCoverage,
  CaptureMessage,
  CapturePayload,
  ContentBlock,
  ExtractionResult,
  SiteExtractor,
  TraversalDiagnostics,
} from './types';

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

function describe(error: unknown): string {
  return error instanceof Error ? error.message : 'unknown error';
}

/**
 * Runs the ladder against a document, in one pass, without moving the page.
 *
 * Exported separately so the choice of strategy can be asserted directly in
 * tests, which is the property that actually matters: not "did we get text"
 * but "did we get it the way we said we did". Also the fallback the revealed
 * path uses when a site extractor recognises nothing.
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

export interface RevealOptions {
  /** Skips the reveal pass entirely. Used by the single-pass tests. */
  reveal?: boolean;
  plan?: TraversalPlan;
  deps?: Parameters<typeof traverse>[1]['deps'];
}

export interface RevealedExtraction {
  result: ExtractionResult;
  traversal: TraversalDiagnostics;
}

/**
 * Acquires a page: reveal, sample, merge, then interpret.
 *
 * The engine exposes content and this function interprets it; they meet at one
 * callback, and neither knows the other's job. For a conversation the sampler
 * harvests turns while they are mounted — mandatory on a virtualizing page,
 * where content exists only while it is on screen. For a document it harvests
 * blocks, which is the same mechanism with a different key.
 */
export async function runReveal(
  doc: Document,
  url: URL,
  options: RevealOptions = {},
): Promise<RevealedExtraction> {
  const site = selectExtractor(url);
  const plan = options.plan ?? site?.traversal ?? GENERIC_TRAVERSAL;

  if (options.reveal === false) {
    return { result: runExtraction(doc, url), traversal: emptyTraversalDiagnostics(plan.id) };
  }

  const conversational = typeof site?.harvest === 'function';
  const merger = new SampleMerger();
  const blocks = new BlockMerger();
  let harvestFailure: string | null = null;
  let genericTruncated = false;
  let links: CapturePayload['links'] = [];

  const sample = (document: Document, surface: ScrollSurface): number => {
    try {
      if (conversational) {
        return merger.add((site as SiteExtractor).harvest!(document, url, surface));
      }
      const harvested = harvestGeneric(document, url.toString());
      genericTruncated = genericTruncated || harvested.truncated;
      if (harvested.links.length > links.length) links = harvested.links;
      return blocks.add(harvested.blocks);
    } catch (error) {
      // A sampler that throws costs this sample, not the capture: the run
      // continues and the single-pass ladder still has the last word.
      harvestFailure = harvestFailure ?? describe(error);
      return 0;
    }
  };

  const traversal = await traverse(doc, { plan, sample, deps: options.deps });

  const merged = conversational ? merger.result() : null;
  const notes: string[] = [];
  if (harvestFailure) {
    notes.push(`Relay could not read part of this page while reading it (${harvestFailure}).`);
  }

  let result: ExtractionResult;

  if (merged && merged.messages.length > 0) {
    const messages: CaptureMessage[] = merged.messages;
    traversal.messages_discovered = merged.discovered;
    traversal.messages_captured = messages.length;
    traversal.duplicates_dropped = merged.duplicatesDropped;
    if (typeof merged.missing === 'number') traversal.messages_missing = merged.missing;

    result = {
      kind: 'conversation',
      strategy: 'site',
      extractorId: site!.id,
      extractorVersion: site!.version,
      blocks: [],
      messages,
      coverage: 'rendered_dom',
      notes,
      truncated: false,
    };
  } else if (!conversational && blocks.result().blocks.length > 0) {
    const collected = blocks.result();
    traversal.duplicates_dropped = collected.duplicatesDropped;
    const single = runExtraction(doc, url);

    // The revealed blocks replace the single pass's, but its identity — which
    // rung of the ladder was reached, what it called the page, its title —
    // stands: the reveal pass changed how much was read, not what read it.
    result = {
      ...single,
      blocks: collected.blocks.length >= single.blocks.length ? collected.blocks : single.blocks,
      truncated: single.truncated || genericTruncated,
      links: links.length ? links : single.links,
      notes: [...notes, ...single.notes],
    };
  } else {
    // Either the site extractor recognised nothing after all, or the page is
    // not document-shaped. Down the ladder, with the reveal pass's work still
    // counted — the page was moved and settled, so a fresh single pass reads a
    // more complete DOM than it would have before.
    result = runExtraction(doc, url);
    result.notes = [...notes, ...result.notes];
  }

  const rich = countRichBlocks([
    ...result.blocks,
    ...result.messages.flatMap((message) => message.blocks),
  ]);
  traversal.attachments_discovered = rich.attachments;
  traversal.attachments_captured = rich.attachments;
  traversal.images_discovered = rich.images;
  traversal.images_captured = rich.images;

  return { result, traversal };
}

/**
 * The completeness verdict, and the plain-language account of it.
 *
 * Split out from `assessCoverage` because a conversation's evidence is
 * different in kind from a document's: there is no meaningful ratio of
 * recognised to total text in a chat UI, so what decides the verdict is
 * whether the traversal finished, whether the page numbered its turns, and
 * whether any of those numbers are missing.
 */
export function judgeCompleteness(
  doc: Document,
  result: ExtractionResult,
  traversal: TraversalDiagnostics,
  siteExtractorExists: boolean,
): { coverage: CaptureCoverage; notes: string[] } {
  if (traversal.termination === 'error') {
    return {
      coverage: 'failed',
      notes: [
        'Relay hit an error while reading this page, so what was captured is a fragment of unknown size.',
      ],
    };
  }

  if (result.messages.length === 0) {
    // Document-shaped: the text ratio decides, with the reveal pass's own
    // findings as additional evidence against a full claim.
    return assessCoverage(doc, blocksTextLength(result.blocks), result.truncated, {
      siteExtractorExists: siteExtractorExists && result.strategy !== 'site',
      traversalPerformed: traversal.performed,
      traversalReachedEnd: traversal.termination === 'reached_end' || !traversal.performed,
      knownGaps: traversal.inaccessible.length,
      availability: traversal.availability,
    });
  }

  const notes: string[] = [];

  if (result.truncated) {
    notes.push('This conversation was longer than the capture limits, so it was cut short.');
    return { coverage: 'partial', notes };
  }

  if ((traversal.messages_missing ?? 0) > 0) {
    notes.push(
      `The page numbers its turns, and ${traversal.messages_missing} of them are missing from this capture.`,
    );
    return { coverage: 'partial', notes };
  }

  if (!traversal.performed) {
    notes.push(
      'Only the turns the page had rendered were captured. Long conversations load earlier turns as you scroll up.',
    );
    return { coverage: 'rendered_dom', notes };
  }

  switch (traversal.termination) {
    case 'time_budget':
      notes.push(
        `Relay read for ${Math.round(traversal.duration_ms / 1000)}s and stopped at its time limit, so earlier or later turns may be missing.`,
      );
      return { coverage: 'partial', notes };
    case 'step_budget':
    case 'expansion_budget':
      notes.push('Relay reached its reading limit for one page, so this capture is incomplete.');
      return { coverage: 'partial', notes };
    case 'user_interrupted':
      notes.push('Reading stopped because the page was used while it was being captured.');
      return { coverage: 'partial', notes };
    case 'navigation_detected':
      notes.push('The page navigated while it was being read, so reading stopped there.');
      return { coverage: 'partial', notes };
    case 'no_progress':
      notes.push('The page stopped yielding new content before its end was reached.');
      return { coverage: 'rendered_dom', notes };
    default:
      break;
  }

  if (traversal.expansions_failed > 0) {
    notes.push(
      `${traversal.expansions_failed} section(s) did not open when Relay tried, so their content is not in this capture.`,
    );
    return { coverage: 'rendered_dom', notes };
  }

  if (traversal.inaccessible.length > 0) {
    return { coverage: 'rendered_dom', notes: [...notes, ...traversal.inaccessible] };
  }

  // Reached the end, nothing missing, nothing left closed. For a conversation
  // this is the first time Relay can say this and mean it.
  notes.push(
    `Relay read this conversation from its beginning to its end and captured ${traversal.messages_captured} turn(s).`,
  );
  if (traversal.expansions_unnecessary > 0) {
    notes.push(
      `${traversal.expansions_unnecessary} shortened section(s) were already fully present in the page, so Relay read them without opening anything.`,
    );
  }
  return { coverage: 'full_document', notes };
}

/**
 * Builds the payload posted to Relay.
 *
 * Throws [`CaptureEmptyError`] rather than sending an empty capture: the user
 * is told nothing was found, and no artifact is created. A vault full of
 * empty captures would be worse than no capture feature.
 */
export async function buildPayload(
  doc: Document,
  href: string,
  options: { browser?: string; startedAt?: number } & RevealOptions = {},
): Promise<CapturePayload> {
  const startedAt = options.startedAt ?? Date.now();
  const url = new URL(href);

  const { result, traversal } = await runReveal(doc, url, options);
  if (isEmpty(result)) throw new CaptureEmptyError();

  const site = selectExtractor(url);
  const { coverage, notes } = judgeCompleteness(doc, result, traversal, Boolean(site));

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
      coverage,
      notes: [...result.notes, ...notes],
      dom_text_length: domTextLength(doc),
      truncated: result.truncated,
      elapsed_ms: Date.now() - startedAt,
      traversal,
    },
  };
}

/** Re-exported so the harness and tests can measure the same denominator. */
export { extractableTextLength, resolveSurface };

/** Exposed for tests that need a block list without a document. */
export type { ContentBlock };
