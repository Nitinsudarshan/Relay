/**
 * DOM → structured blocks. The shared machinery every extractor builds on.
 *
 * Kept free of any site knowledge and of any browser-extension API so it can
 * be unit-tested against fixture documents in jsdom, which is where all of
 * the interesting cases live: malformed markup, hidden nodes, virtualized
 * lists, tables with ragged rows.
 */

import type {
  AttachmentKind,
  AvailabilityCounts,
  CaptureCoverage,
  ContentAvailability,
  ContentBlock,
  ImageOrigin,
} from './types';
import { emptyAvailabilityCounts } from './types';

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

/** Why an image's own bytes are not in the payload. Stated on every image. */
export const IMAGE_CONTENT_NOTE =
  'Relay recorded where this image came from but did not download the image itself.';

/** Why a file's own bytes are not in the payload. Stated on every attachment. */
export const ATTACHMENT_CONTENT_NOTE =
  'Relay recorded this file\u2019s details but did not download the file itself.';

function numericAttribute(el: Element, name: string): number | undefined {
  const raw = el.getAttribute(name);
  if (!raw) return undefined;
  const value = Number.parseInt(raw, 10);
  return Number.isFinite(value) && value > 0 ? value : undefined;
}

/**
 * The caption an image is sitting in, if any.
 *
 * `<figcaption>` is the only markup that states this unambiguously, so it is
 * the only one read. Guessing a caption from a neighbouring paragraph is how
 * a capture ends up attributing body text to a picture.
 */
function captionFor(el: Element): string | undefined {
  const figure = el.closest('figure');
  const caption = figure?.querySelector('figcaption');
  const text = caption ? textOf(caption) : '';
  return text ? text.slice(0, 500) : undefined;
}

/**
 * Turns an `<img>` into an image block.
 *
 * `src` carries only ordinary `http(s)` references. A `blob:` or `data:`
 * source, or a site-internal scheme, is preserved in `reference` instead:
 * evidence of where the image came from, without emitting a link target that
 * could never resolve outside the page that made it.
 *
 * Dimensions come from what the browser actually loaded (`naturalWidth`)
 * where available, falling back to the declared attributes. `content_captured`
 * is always false — Relay does not fetch page resources (see
 * `docs/capture/RESEARCH.md` §5.1).
 */
export function imageBlockFrom(el: Element, origin: ImageOrigin = 'page'): ContentBlock | null {
  const alt = normalizeWhitespace(el.getAttribute('alt') ?? '');
  const raw = (el.getAttribute('src') ?? el.getAttribute('data-src') ?? '').trim();
  const isHttp = /^https?:\/\//i.test(raw);
  const isRelative = raw.length > 0 && !isHttp && !/^[a-z][a-z0-9+.-]*:/i.test(raw);
  const src = isHttp || isRelative ? raw : undefined;
  const reference = !src && raw ? raw.slice(0, 300) : undefined;
  const caption = captionFor(el);

  if (!alt && !src && !reference && !caption) return null;

  const image = el as HTMLImageElement;
  const width = (typeof image.naturalWidth === 'number' && image.naturalWidth) || numericAttribute(el, 'width');
  const height =
    (typeof image.naturalHeight === 'number' && image.naturalHeight) || numericAttribute(el, 'height');

  return {
    type: 'image',
    alt: alt || undefined,
    caption,
    src,
    reference,
    width: width || undefined,
    height: height || undefined,
    origin,
    content_captured: false,
    content_note: IMAGE_CONTENT_NOTE,
  };
}

function imageBlock(el: Element): ContentBlock | null {
  return imageBlockFrom(el, 'page');
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
      case 'a': {
        // `download` is the page asserting that this link is a file. Nothing
        // else about an anchor is read here — a link's text stays prose.
        if (el.hasAttribute('download')) {
          const block = attachmentBlockFrom(el, { kind: 'linked' });
          if (block) {
            blocks.push(block);
            return;
          }
        }
        break;
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
        return total + (block.alt?.length ?? 0) + (block.caption?.length ?? 0);
      case 'attachment':
        return total + (block.name?.length ?? 0) + (block.preview?.length ?? 0);
      default:
        return total;
    }
  }, 0);
}

/**
 * How much of a subtree's text an extractor would actually take.
 *
 * The like-for-like denominator for coverage, and the reason v1's coverage
 * verdict could not be trusted: `innerText` is layout-derived and omits text
 * that is present in the DOM but not rendered — the body of a closed
 * `<details>`, a screen-reader-only label positioned off-screen — while the
 * extractor's numerator comes from `textContent` and includes exactly that
 * text. Dividing one by the other produced ratios above 1 on ordinary pages,
 * which sailed past a 0.9 threshold and claimed a full document.
 *
 * Verified in Chromium: on a Claude-shaped fixture with a collapsed thinking
 * block and one screen-reader label, `body.innerText` was 5,230 characters
 * against 5,297 of extractable text — enough on its own to invert the ratio.
 */
export function extractableTextLength(root: Element): number {
  let total = 0;

  const visit = (node: Node): void => {
    if (node.nodeType === 3 /* TEXT_NODE */) {
      total += normalizeWhitespace(node.textContent ?? '').length;
      return;
    }
    if (node.nodeType !== 1 /* ELEMENT_NODE */) return;
    const el = node as Element;
    if (isSkippable(el)) return;
    for (const child of Array.from(el.childNodes)) visit(child);
  };

  visit(root);
  return total;
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

/** The page's visible text length, reported as a diagnostic alongside the capture. */
export function domTextLength(doc: Document): number {
  const body = doc.body as HTMLElement | null;
  if (!body) return 0;
  const rendered = typeof body.innerText === 'string' ? body.innerText : body.textContent ?? '';
  return normalizeWhitespace(rendered).length;
}

/**
 * Slack allowed before an overflowing box counts as clipped.
 *
 * Sub-pixel layout and one-pixel borders routinely put `scrollHeight` a
 * couple of pixels above `clientHeight` on boxes that are not clipping
 * anything.
 */
const CLIP_TOLERANCE_PX = 4;

/**
 * Whether an element is showing less than it contains.
 *
 * This is the measurement that stops Relay clicking things it does not need
 * to click. Verified in Chromium: a `-webkit-line-clamp: 3` box holding 2,904
 * characters reported `scrollHeight` 777 against `clientHeight` 63 — while
 * `textContent` *and* `innerText` both returned all 2,904 characters. The text
 * was never missing. Only its presentation was.
 */
export function isVisuallyClipped(el: Element): boolean {
  const box = el as HTMLElement;
  if (typeof box.scrollHeight !== 'number' || typeof box.clientHeight !== 'number') return false;
  if (box.clientHeight <= 0) return false;
  if (box.scrollHeight <= box.clientHeight + CLIP_TOLERANCE_PX) return false;

  try {
    const view = el.ownerDocument?.defaultView;
    if (view?.getComputedStyle) {
      const style = view.getComputedStyle(el);
      const clipping = /hidden|clip/.test(`${style.overflow} ${style.overflowY}`);
      const clamped = (style.webkitLineClamp ?? '') !== '' && style.webkitLineClamp !== 'none';
      const capped = style.maxHeight !== 'none' && style.maxHeight !== '';
      return clipping || clamped || capped;
    }
  } catch {
    // No computed styles available: the overflow measurement above stands.
  }
  return true;
}

/**
 * Which availability state a disclosure control's content is actually in.
 *
 * Called before any click, and the whole point of the least-invasive rule: a
 * control whose content is merely clipped reports `visually_truncated`, and
 * the engine extracts rather than interacts. Only `collapsed` — the content is
 * genuinely not in the DOM — earns an activation.
 *
 * `target` is the element the control governs, resolved from `aria-controls`
 * where the page states it and from the nearest clipped ancestor otherwise.
 */
export function classifyAvailability(control: Element, target: Element | null): ContentAvailability {
  if (!target) return 'collapsed';
  if (isVisuallyClipped(target)) return 'visually_truncated';

  // A closed `<details>` usually keeps its content in the DOM — verified in
  // Chromium, where `textContent` returned the collapsed body and `innerText`
  // did not. Relay's block walk reads `textContent`, so that content is
  // already captured and opening it would be a side effect for nothing.
  //
  // "Usually", because a component can render its children only on open. So
  // the DOM is asked rather than assumed: a closed `<details>` holding text
  // beyond its own summary is present; an empty one is genuinely collapsed.
  const details = target.closest('details');
  if (details && !details.hasAttribute('open')) {
    const summary = details.querySelector('summary');
    const body = (details.textContent ?? '').length - (summary?.textContent ?? '').length;
    return body > 0 ? 'visually_truncated' : 'collapsed';
  }

  return 'collapsed';
}

/** The element a disclosure control governs, as far as the page states it. */
export function disclosureTarget(control: Element): Element | null {
  const controls = control.getAttribute('aria-controls');
  if (controls) {
    for (const id of controls.split(/\s+/)) {
      const target = id && control.ownerDocument?.getElementById(id);
      if (target) return target;
    }
  }

  if (control.tagName.toLowerCase() === 'summary') {
    return control.parentElement;
  }

  // Otherwise: the nearest ancestor that is showing less than it holds. Walking
  // up rather than guessing at siblings, because a control's own container is
  // the only relationship the markup actually asserts.
  let node: Element | null = control.parentElement;
  for (let depth = 0; node && depth < 6; depth += 1) {
    if (isVisuallyClipped(node)) return node;
    node = node.parentElement;
  }
  return null;
}

/**
 * A filename inside a file card's label.
 *
 * Deliberately allows no spaces. Cards render the size beside the name
 * (`report.csv 2.4 MB`), and a space-permitting pattern matched
 * `report.csv 2.4` with `4` as the extension. A filename that genuinely
 * contains a space is read from the `download` or `title` attribute instead,
 * which is where a page that means it puts one.
 */
const ATTACHMENT_NAME_PATTERN = /([\w][\w\-.()+]{0,120}\.[a-z0-9]{1,8})(?![\w.])/i;
const SIZE_PATTERN = /(\d+(?:[.,]\d+)?)\s*(bytes?|[kmg]i?b)\b/i;

const SIZE_UNITS: Record<string, number> = {
  byte: 1,
  bytes: 1,
  kb: 1000,
  kib: 1024,
  mb: 1000 ** 2,
  mib: 1024 ** 2,
  gb: 1000 ** 3,
  gib: 1024 ** 3,
};

/** `"2.4 MB"` → bytes. Returns undefined rather than a guess. */
export function parseSizeLabel(text: string): number | undefined {
  const match = SIZE_PATTERN.exec(text);
  if (!match) return undefined;
  const value = Number.parseFloat(match[1].replace(',', '.'));
  const unit = SIZE_UNITS[match[2].toLowerCase().replace(/s$/, '')] ?? SIZE_UNITS[match[2].toLowerCase()];
  if (!Number.isFinite(value) || !unit) return undefined;
  return Math.round(value * unit);
}

/**
 * Turns a file card or file link into an attachment block.
 *
 * `href` takes only ordinary `http(s)` targets. A site-internal reference —
 * ChatGPT's `sandbox:/mnt/data/report.csv` is the case this exists for — goes
 * in `reference`, so it is preserved as evidence without ever reaching a link
 * renderer that would present it as something the user could open.
 *
 * `content_captured` is always false. A filename is not a file, and this field
 * is what stops the artifact implying otherwise.
 */
export function attachmentBlockFrom(
  el: Element,
  options: { kind?: AttachmentKind; name?: string; mime?: string } = {},
): ContentBlock | null {
  const label = textOf(el).slice(0, 400);
  const raw = (el.getAttribute('href') ?? el.getAttribute('data-href') ?? '').trim();
  const isHttp = /^https?:\/\//i.test(raw);
  const isRelative = raw.length > 0 && !isHttp && !/^[a-z][a-z0-9+.-]*:/i.test(raw);

  const explicit = options.name ?? el.getAttribute('download') ?? el.getAttribute('title') ?? '';
  const name =
    normalizeWhitespace(explicit) ||
    ATTACHMENT_NAME_PATTERN.exec(label)?.[1] ||
    (isHttp || isRelative ? decodeURIComponent(raw.split(/[?#]/)[0].split('/').pop() ?? '') : '');

  const href = isHttp || isRelative ? raw : undefined;
  const reference = !href && raw ? raw.slice(0, 300) : undefined;

  if (!name && !href && !reference) return null;

  const extension = /\.([a-z0-9]{1,8})$/i.exec(name)?.[1]?.toLowerCase();

  return {
    type: 'attachment',
    name: name || undefined,
    mime: options.mime ?? extension,
    size_bytes: parseSizeLabel(label),
    href,
    reference,
    kind: options.kind ?? 'unknown',
    preview: label && label !== name ? label : undefined,
    content_captured: false,
    content_note: ATTACHMENT_CONTENT_NOTE,
  };
}

/** Counts the attachment and image blocks in a block list, for diagnostics. */
export function countRichBlocks(blocks: ContentBlock[]): { attachments: number; images: number } {
  let attachments = 0;
  let images = 0;
  for (const block of blocks) {
    if (block.type === 'attachment') attachments += 1;
    if (block.type === 'image') images += 1;
  }
  return { attachments, images };
}

/** Evidence the coverage verdict is allowed to rely on. */
export interface CoverageEvidence {
  /**
   * True when Relay has a site extractor for this URL. A generic capture of a
   * page Relay claims to understand specifically is evidence *against*
   * completeness: the page is not the shape Relay expected.
   */
  siteExtractorExists?: boolean;
  /** Whether the reveal pass ran, and whether it finished. */
  traversalPerformed?: boolean;
  traversalReachedEnd?: boolean;
  /** Content the reveal pass knows it could not obtain. */
  knownGaps?: number;
  availability?: AvailabilityCounts;
}

/**
 * Decides what the capture may claim about its own completeness.
 *
 * `full_document` needs positive evidence, and after v0.26.0 it needs *more*
 * of it. The rules, in order:
 *
 * 1. Content was dropped → `partial`. Nothing overrides this.
 * 2. No measurable text → `unknown`.
 * 3. Relay has a site extractor for this URL but the capture came from the
 *    generic path → never `full_document`. This is the case that produced a
 *    false "the whole page was captured" on a Claude conversation: the
 *    conversation selectors did not match, the generic extractor read the
 *    page's text, and a ratio computed against `innerText` cleared 0.9.
 * 4. Anything virtualized, or any known gap → `rendered_dom` at best.
 * 5. A reveal pass that ran and did not reach the end → `partial`.
 * 6. Otherwise, and only otherwise: ~90%+ of the *extractable* text was
 *    recognised as content → `full_document`.
 */
export function assessCoverage(
  doc: Document,
  extractedLength: number,
  truncated: boolean,
  evidence: CoverageEvidence = {},
): { coverage: CaptureCoverage; notes: string[] } {
  const notes: string[] = [];
  if (truncated) {
    return {
      coverage: 'partial',
      notes: ['The page was larger than the capture limits, so it was cut short.'],
    };
  }

  const body = doc.body as Element | null;
  const total = body ? extractableTextLength(body) : 0;
  if (total === 0) {
    return { coverage: 'unknown', notes: ['The page reported no visible text.'] };
  }

  const availability = evidence.availability ?? emptyAvailabilityCounts();
  const ratio = Math.min(extractedLength / total, 1);

  if (evidence.traversalPerformed && evidence.traversalReachedEnd === false) {
    notes.push(
      'Relay stopped reading before it reached the end of this page, so there is more of it than was captured.',
    );
    return { coverage: 'partial', notes };
  }

  if ((evidence.knownGaps ?? 0) > 0) {
    notes.push(
      `${evidence.knownGaps} part(s) of this page could not be read, so the capture has gaps.`,
    );
    return { coverage: 'partial', notes };
  }

  if (looksVirtualized(doc) || availability.virtualized > 0) {
    notes.push(
      'This page renders its content in a virtualized list, so only the parts Relay could reach while reading were captured.',
    );
    return { coverage: 'rendered_dom', notes };
  }

  if (availability.collapsed > 0 || availability.inaccessible > 0) {
    notes.push(
      'Some content on this page stayed collapsed or out of reach, so the capture is not the whole page.',
    );
    return { coverage: 'rendered_dom', notes };
  }

  if (evidence.siteExtractorExists) {
    // Relay knows this site and did not recognise this page. Whatever the text
    // ratio says, the honest verdict is that the page was read generically.
    notes.push(
      'Relay knows this site but did not recognise this page’s layout, so it was read as a plain document and may be incomplete.',
    );
    return { coverage: 'rendered_dom', notes };
  }

  if (ratio >= 0.9) {
    return { coverage: 'full_document', notes };
  }

  notes.push(
    `About ${Math.round(ratio * 100)}% of the page's text was recognised as content; the rest was navigation, controls, or a layout Relay could not read.`,
  );
  return { coverage: 'rendered_dom', notes };
}
