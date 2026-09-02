/**
 * DOM → structured blocks. The shared machinery every extractor builds on.
 *
 * Kept free of any site knowledge and of any browser-extension API so it can
 * be unit-tested against fixture documents in jsdom, which is where all of
 * the interesting cases live: malformed markup, hidden nodes, virtualized
 * lists, tables with ragged rows.
 */

import type { CaptureCoverage, ContentBlock } from './types';

/** Caps mirroring the backend's, so a page is trimmed before it is sent. */
export const MAX_BLOCKS = 5000;
export const MAX_TEXT_CHARS = 100_000;
export const MAX_LIST_ITEMS = 1000;
export const MAX_TABLE_ROWS = 500;
export const MAX_LINKS = 200;
/** How deep to walk before treating a subtree as a leaf paragraph. */
const MAX_DEPTH = 40;

/** Structural chrome that is never page content. */
const SKIP_TAGS = new Set([
  'script',
  'style',
  'noscript',
  'template',
  'svg',
  'canvas',
  'iframe',
  'object',
  'embed',
  'audio',
  'video',
  'form',
  'button',
  'select',
  'textarea',
  'input',
  'link',
  'meta',
]);

/** Regions that surround content rather than being it. */
const NOISE_SELECTOR = [
  'nav',
  'aside',
  'header',
  'footer',
  '[role="navigation"]',
  '[role="banner"]',
  '[role="complementary"]',
  '[role="search"]',
  '[aria-hidden="true"]',
  '.sidebar',
  '.site-header',
  '.site-footer',
  '.advertisement',
  '.cookie-banner',
].join(',');

export function normalizeWhitespace(text: string): string {
  return text.replace(/\r\n?/g, '\n').replace(/[ \t ]+/g, ' ').replace(/\n{3,}/g, '\n\n').trim();
}

function clampText(text: string): string {
  return text.length > MAX_TEXT_CHARS ? text.slice(0, MAX_TEXT_CHARS) : text;
}

export function textOf(node: Node): string {
  return clampText(normalizeWhitespace(node.textContent ?? ''));
}

/**
 * Whether an element is invisible to the reader.
 *
 * A page can hide content in several ways and a capture should honor all of
 * them — hidden text is a common place for keyword stuffing and for stale
 * drafts, neither of which belongs in a knowledge vault. `getComputedStyle`
 * is guarded because it is unavailable or partial outside a real browser.
 */
export function isHidden(el: Element): boolean {
  if (el.hasAttribute('hidden') || el.getAttribute('aria-hidden') === 'true') return true;

  const inline = el.getAttribute('style') ?? '';
  if (/display\s*:\s*none|visibility\s*:\s*hidden/i.test(inline)) return true;

  try {
    const view = el.ownerDocument?.defaultView;
    if (view?.getComputedStyle) {
      const style = view.getComputedStyle(el);
      if (style.display === 'none' || style.visibility === 'hidden') return true;
    }
  } catch {
    // Some documents refuse computed styles; the attribute checks above stand.
  }
  return false;
}

function isSkippable(el: Element): boolean {
  return SKIP_TAGS.has(el.tagName.toLowerCase()) || isHidden(el);
}

function pushText(blocks: ContentBlock[], text: string, seen: Set<string>): void {
  const cleaned = normalizeWhitespace(text);
  if (!cleaned) return;
  // Consecutive duplicates are a rendering artifact (sticky headers, screen
  // reader copies of the same string), not repetition the author wrote.
  const key = cleaned.slice(0, 200);
  if (seen.has(key)) return;
  seen.add(key);
  blocks.push({ type: 'paragraph', text: clampText(cleaned) });
}

function listBlock(el: Element): ContentBlock | null {
  const items: string[] = [];
  for (const li of Array.from(el.children)) {
    if (li.tagName.toLowerCase() !== 'li' || isHidden(li)) continue;
    const text = textOf(li);
    if (text) items.push(text);
    if (items.length >= MAX_LIST_ITEMS) break;
  }
  if (!items.length) return null;
  return { type: 'list', ordered: el.tagName.toLowerCase() === 'ol', items };
}

function codeBlock(el: Element): ContentBlock | null {
  const code = el.querySelector('code') ?? el;
  const text = clampText((code.textContent ?? '').replace(/\r\n?/g, '\n').replace(/\s+$/, ''));
  if (!text.trim()) return null;

  // Highlighters put the language in a class on the <code> element, and chat
  // UIs often render it as a label above the block; both are common enough
  // to be worth reading rather than dropping the language entirely.
  const classes = `${code.className ?? ''} ${el.className ?? ''}`;
  const fromClass = /(?:language|lang|highlight)-([a-z0-9+#._-]+)/i.exec(classes);
  const language = fromClass?.[1]?.toLowerCase();
  return { type: 'code', language, text };
}

function tableBlock(el: Element): ContentBlock | null {
  const rows = Array.from(el.querySelectorAll('tr')).slice(0, MAX_TABLE_ROWS);
  if (!rows.length) return null;

  let headers: string[] = [];
  const body: string[][] = [];

  rows.forEach((row, index) => {
    const cells = Array.from(row.querySelectorAll('th,td')).map((cell) => textOf(cell));
    if (!cells.length) return;
    const allHeaderCells = Array.from(row.querySelectorAll('th')).length === cells.length;
    if (index === 0 && allHeaderCells) {
      headers = cells;
    } else {
      body.push(cells);
    }
  });

  if (!headers.length && !body.length) return null;
  return { type: 'table', headers, rows: body };
}

function imageBlock(el: Element): ContentBlock | null {
  const alt = normalizeWhitespace(el.getAttribute('alt') ?? '');
  const src = el.getAttribute('src') ?? undefined;
  if (!alt && !src) return null;
  return { type: 'image', alt: alt || undefined, src };
}

/**
 * Walks a subtree in reading order and turns it into blocks.
 *
 * Depth-first over child *nodes* rather than elements, so text written
 * directly inside a container — extremely common in hand-written and in
 * framework-generated markup alike — is not silently dropped.
 */
export function extractBlocks(root: Element): { blocks: ContentBlock[]; truncated: boolean } {
  const blocks: ContentBlock[] = [];
  const seenText = new Set<string>();
  let truncated = false;

  const visit = (node: Node, depth: number): void => {
    if (blocks.length >= MAX_BLOCKS) {
      truncated = true;
      return;
    }

    if (node.nodeType === 3 /* TEXT_NODE */) {
      pushText(blocks, node.textContent ?? '', seenText);
      return;
    }
    if (node.nodeType !== 1 /* ELEMENT_NODE */) return;

    const el = node as Element;
    if (isSkippable(el)) return;

    const tag = el.tagName.toLowerCase();
    switch (tag) {
      case 'h1':
      case 'h2':
      case 'h3':
      case 'h4':
      case 'h5':
      case 'h6': {
        const text = textOf(el);
        if (text) blocks.push({ type: 'heading', level: Number(tag[1]), text });
        return;
      }
      case 'p': {
        pushText(blocks, textOf(el), seenText);
        return;
      }
      case 'ul':
      case 'ol': {
        const block = listBlock(el);
        if (block) blocks.push(block);
        return;
      }
      case 'pre': {
        const block = codeBlock(el);
        if (block) blocks.push(block);
        return;
      }
      case 'blockquote': {
        const text = textOf(el);
        if (text) blocks.push({ type: 'quote', text });
        return;
      }
      case 'table': {
        const block = tableBlock(el);
        if (block) blocks.push(block);
        return;
      }
      case 'img': {
        const block = imageBlock(el);
        if (block) blocks.push(block);
        return;
      }
      case 'br':
      case 'hr':
        return;
      default:
        break;
    }

    if (depth >= MAX_DEPTH) {
      // Pathologically nested markup: stop descending and keep the text.
      pushText(blocks, textOf(el), seenText);
      truncated = true;
      return;
    }

    if (!el.childNodes.length) {
      pushText(blocks, textOf(el), seenText);
      return;
    }

    for (const child of Array.from(el.childNodes)) visit(child, depth + 1);
  };

  for (const child of Array.from(root.childNodes)) visit(child, 1);
  return { blocks, truncated };
}

/** Removes the page furniture around an article before extracting from it. */
export function stripNoise(root: Element): Element {
  const clone = root.cloneNode(true) as Element;
  for (const el of Array.from(clone.querySelectorAll(NOISE_SELECTOR))) {
    el.remove();
  }
  return clone;
}

export function collectLinks(root: Element, baseUrl: string): { text: string; href: string }[] {
  const links: { text: string; href: string }[] = [];
  const seen = new Set<string>();

  for (const anchor of Array.from(root.querySelectorAll('a[href]'))) {
    if (links.length >= MAX_LINKS) break;
    const raw = anchor.getAttribute('href') ?? '';
    if (!raw || raw.startsWith('#')) continue;

    let href: string;
    try {
      href = new URL(raw, baseUrl).toString();
    } catch {
      continue;
    }
    // The backend drops non-http(s) targets too; doing it here as well keeps
    // them out of the payload rather than out of the artifact.
    if (!/^https?:\/\//i.test(href) || seen.has(href)) continue;
    seen.add(href);
    links.push({ text: textOf(anchor).slice(0, 200) || href, href });
  }
  return links;
}

function metaContent(doc: Document, selectors: string[]): string | undefined {
  for (const selector of selectors) {
    const el = doc.querySelector(selector);
    const value = el?.getAttribute('content') ?? el?.getAttribute('href') ?? el?.textContent ?? '';
    const cleaned = normalizeWhitespace(value).slice(0, 300);
    if (cleaned) return cleaned;
  }
  return undefined;
}

/** Reads the standard `<head>` provenance: OpenGraph, canonical, author, language. */
export function readMetadata(doc: Document) {
  return {
    canonical_url: metaContent(doc, ['link[rel="canonical"]', 'meta[property="og:url"]']),
    site_name: metaContent(doc, ['meta[property="og:site_name"]', 'meta[name="application-name"]']),
    author: metaContent(doc, [
      'meta[name="author"]',
      'meta[property="article:author"]',
      '[itemprop="author"] [itemprop="name"]',
      '[rel="author"]',
    ]),
    published_at: metaContent(doc, [
      'meta[property="article:published_time"]',
      'meta[name="date"]',
      'time[datetime]',
    ]),
    description: metaContent(doc, [
      'meta[name="description"]',
      'meta[property="og:description"]',
    ]),
    language: normalizeWhitespace(doc.documentElement?.getAttribute('lang') ?? '') || undefined,
  };
}

/** How much text a block set carries, used to judge coverage. */
export function blocksTextLength(blocks: ContentBlock[]): number {
  return blocks.reduce((total, block) => {
    switch (block.type) {
      case 'heading':
      case 'paragraph':
      case 'quote':
      case 'code':
        return total + block.text.length;
      case 'list':
        return total + block.items.join('').length;
      case 'table':
        return total + block.headers.join('').length + block.rows.flat().join('').length;
      case 'image':
        return total + (block.alt?.length ?? 0);
      default:
        return total;
    }
  }, 0);
}

/** Markers left by the common list-virtualization libraries. */
const VIRTUALIZATION_SELECTOR = [
  '[data-virtuoso-scroller]',
  '[data-testid="virtuoso-item-list"]',
  '.ReactVirtualized__Grid',
  '.rc-virtual-list',
  '[data-overlayscrollbars-viewport]',
].join(',');

export function looksVirtualized(doc: Document): boolean {
  return doc.querySelector(VIRTUALIZATION_SELECTOR) !== null;
}

/** The page's visible text length, used as the denominator for coverage. */
export function domTextLength(doc: Document): number {
  const body = doc.body as HTMLElement | null;
  if (!body) return 0;
  const rendered = typeof body.innerText === 'string' ? body.innerText : body.textContent ?? '';
  return normalizeWhitespace(rendered).length;
}

/**
 * Decides what the capture may claim about its own completeness.
 *
 * `full_document` needs evidence: essentially all of the page's visible text
 * ended up in the payload, and nothing on the page looks virtualized. Absent
 * that, the honest answer is `rendered_dom` — the DOM is not the document,
 * and a capture that says otherwise is the failure mode this whole feature
 * exists to avoid.
 */
export function assessCoverage(
  doc: Document,
  extractedLength: number,
  truncated: boolean,
): { coverage: CaptureCoverage; notes: string[] } {
  const notes: string[] = [];
  if (truncated) {
    return {
      coverage: 'partial',
      notes: ['The page was larger than the capture limits, so it was cut short.'],
    };
  }

  const total = domTextLength(doc);
  if (total === 0) {
    return { coverage: 'unknown', notes: ['The page reported no visible text.'] };
  }

  if (looksVirtualized(doc)) {
    notes.push(
      'This page renders its content in a virtualized list, so only the part that was on screen could be read.',
    );
    return { coverage: 'rendered_dom', notes };
  }

  const ratio = extractedLength / total;
  if (ratio >= 0.9) {
    return { coverage: 'full_document', notes };
  }

  notes.push(
    `About ${Math.round(ratio * 100)}% of the page's visible text was recognised as content; the rest was navigation, controls, or a layout Relay could not read.`,
  );
  return { coverage: 'rendered_dom', notes };
}
