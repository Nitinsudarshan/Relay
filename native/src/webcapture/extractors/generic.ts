/**
 * The generic extractor: what Relay does on a page no site-specific extractor
 * claims — which is most of the web, and always will be.
 *
 * A miniature readability pass: find the region that holds the article, drop
 * the furniture around it, and walk what is left. Deliberately not a port of
 * Mozilla's Readability — that is a large dependency to ship into every page
 * a user captures, and its extra accuracy buys little once the fallback below
 * it is "the visible text of the page, honestly labelled".
 */

import {
  assessCoverage,
  blocksTextLength,
  collectLinks,
  extractBlocks,
  normalizeWhitespace,
  stripNoise,
  textOf,
} from '../dom';
import type { CapturedLink, ContentBlock, ExtractionResult } from '../types';

export const GENERIC_EXTRACTOR_ID = 'generic';
export const GENERIC_EXTRACTOR_VERSION = 1;

/** Semantic containers, best first. A page that uses these is telling us where its content is. */
const MAIN_SELECTORS = [
  'article',
  'main',
  '[role="main"]',
  '#main-content',
  '#content',
  '.post-content',
  '.entry-content',
  '.article-body',
  '.markdown-body',
];

/** Containers that are structurally large but semantically empty. */
const CANDIDATE_SELECTOR = 'article, main, section, div';

function candidateScore(el: Element): number {
  const paragraphs = el.querySelectorAll('p').length;
  const text = (el.textContent ?? '').trim().length;
  const links = el.querySelectorAll('a').length;
  // Link-dense blocks are navigation even when they are long, which is the
  // single most useful signal for telling a sidebar from an article.
  const linkPenalty = links > 0 ? Math.min(links * 40, text * 0.5) : 0;
  return text + paragraphs * 120 - linkPenalty;
}

/**
 * Picks the element most likely to contain the page's main content.
 *
 * Exported because it is the part worth testing directly: a page with a
 * navigation sidebar longer than its article is the case that separates a
 * useful capture from a useless one.
 */
export function findMainRegion(doc: Document): Element {
  for (const selector of MAIN_SELECTORS) {
    const el = doc.querySelector(selector);
    if (el && (el.textContent ?? '').trim().length > 200) return el;
  }

  let best: Element | null = null;
  let bestScore = 0;
  for (const el of Array.from(doc.querySelectorAll(CANDIDATE_SELECTOR))) {
    const score = candidateScore(el);
    if (score > bestScore) {
      best = el;
      bestScore = score;
    }
  }

  return best ?? doc.body ?? doc.documentElement;
}

function documentTitle(doc: Document, region: Element): string | undefined {
  const heading = region.querySelector('h1') ?? doc.querySelector('h1');
  const fromHeading = heading ? textOf(heading) : '';
  if (fromHeading) return fromHeading;
  const title = normalizeWhitespace(doc.title ?? '');
  return title || undefined;
}

/**
 * Extracts a page as a document.
 *
 * Never returns `null`: this is the floor of the ladder for anything with a
 * body, and a capture with an honest coverage label beats no capture at all.
 */
/**
 * One pass of document-shaped content, without any coverage judgement.
 *
 * Split out because the reveal engine calls it once per sample: a page that
 * loads content as you scroll gives up a little more of itself each time, and
 * the merge step is what turns those overlapping reads back into a document.
 * A page that needs one sample simply gets one, which is the same code path.
 */
export function harvestGeneric(
  doc: Document,
  baseUrl?: string,
): { blocks: ContentBlock[]; truncated: boolean; links: CapturedLink[]; region: Element } {
  const region = findMainRegion(doc);
  const cleaned = stripNoise(region);
  const { blocks, truncated } = extractBlocks(cleaned);
  // Links come from the cleaned content region, so a capture's link list is
  // the article's references rather than the site's navigation menu.
  const links = baseUrl ? collectLinks(cleaned, baseUrl) : [];
  return { blocks, truncated, links, region };
}

export function extractGeneric(doc: Document, baseUrl?: string): ExtractionResult {
  const { blocks, truncated, links, region } = harvestGeneric(doc, baseUrl);
  const { coverage, notes } = assessCoverage(doc, blocksTextLength(blocks), truncated);
  return {
    kind: 'article',
    strategy: 'article',
    extractorId: GENERIC_EXTRACTOR_ID,
    extractorVersion: GENERIC_EXTRACTOR_VERSION,
    blocks,
    messages: [],
    coverage,
    notes,
    truncated,
    title: documentTitle(doc, region),
    links,
  };
}

/**
 * The last resort: the page's visible text as paragraphs.
 *
 * Reached when structured extraction produced nothing at all — a canvas app,
 * a page that renders entirely into shadow roots, a viewer Relay does not
 * understand. Labelled `text` so the artifact says plainly that this is what
 * happened, rather than presenting a thin capture as a good one.
 */
export function extractVisibleText(doc: Document): ExtractionResult {
  const body = doc.body as HTMLElement | null;
  const raw = body
    ? typeof body.innerText === 'string'
      ? body.innerText
      : body.textContent ?? ''
    : '';

  const paragraphs = normalizeWhitespace(raw)
    .split(/\n{1,}/)
    .map((line) => line.trim())
    .filter((line) => line.length > 1)
    .slice(0, 2000);

  return {
    kind: 'generic',
    strategy: 'text',
    extractorId: 'visible-text',
    extractorVersion: GENERIC_EXTRACTOR_VERSION,
    blocks: paragraphs.map((text) => ({ type: 'paragraph' as const, text })),
    messages: [],
    coverage: 'rendered_dom',
    notes: [
      'Relay could not recognise this page’s structure, so it captured the text that was visible on screen.',
    ],
    truncated: false,
    title: normalizeWhitespace(doc.title ?? '') || undefined,
  };
}
