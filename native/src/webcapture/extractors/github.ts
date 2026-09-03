/**
 * GitHub extractor: repositories, issues, pull requests, discussions, and
 * single files.
 *
 * GitHub's URL shape says what a page is, and Relay's backend classifies from
 * the URL for exactly that reason. This extractor's job is only to pull out
 * the *structure* the page renders — a README, a threaded discussion, a
 * file's source — and to hand back `null` on anything it does not recognise,
 * so the generic extractor still produces something useful.
 */

import { extractBlocks, normalizeWhitespace, textOf } from '../dom';
import { budget } from '../traversal/budget';
import type { TraversalPlan } from '../traversal/types';
import type { CaptureMessage, ContentBlock, ExtractionResult, SiteExtractor } from '../types';

/**
 * How to walk a GitHub page.
 *
 * The reason this plan enables expansion at all: GitHub hides the middle of a
 * long issue or pull-request thread behind "Load more…" controls, and hides
 * outdated review comments behind disclosure widgets. Both are
 * content-disclosure controls in the strict sense — they reveal text that is
 * genuinely not in the DOM — which is exactly the case expansion exists for.
 */
export const githubTraversal: TraversalPlan = {
  id: 'github',
  scrollerSelectors: [],
  contentSelectors: ['main', '[role="main"]', 'body'],
  itemSelectors: [
    '.js-comment-container',
    '.timeline-comment',
    '[data-testid="comment-viewer-outer-box"]',
    '.js-timeline-item',
    'article.markdown-body',
    '#readme',
    '.Box-row',
  ],
  expandSelectors: [
    'button.ajax-pagination-btn',
    '.js-timeline-progressive-loader button',
    'button[aria-label*="Load more" i]',
    'details.js-details-container:not([open]) > summary',
    '.outdated-comment button',
  ],
  // Review forms, the comment composer and the merge box: all action surfaces.
  forbiddenSelectors: [
    'form',
    '.js-comment-form',
    '#partial-pull-merging',
    '.merge-pr',
    '[data-testid="merge-box"]',
  ],
  loadingSelectors: ['.js-timeline-progressive-loader[aria-busy="true"]', '[aria-busy="true"]'],
  rewind: true,
  expand: true,
  budget: budget({ maxMs: 12_000 }),
};

const TITLE_SELECTORS = [
  'bdi.js-issue-title',
  '.js-issue-title',
  'h1 .markdown-title',
  '[data-testid="issue-title"]',
  'h1',
];

const COMMENT_SELECTORS = [
  '.js-comment-container',
  '.timeline-comment',
  '[data-testid="comment-viewer-outer-box"]',
];

const README_SELECTORS = ['#readme article', '#readme .markdown-body', 'article.markdown-body'];

const CODE_SELECTORS = [
  '#read-only-cursor-text-area',
  '[data-testid="read-only-cursor-text-area"]',
  '.blob-wrapper',
];

function firstMatch(doc: Document, selectors: string[]): Element | null {
  for (const selector of selectors) {
    const el = doc.querySelector(selector);
    if (el) return el;
  }
  return null;
}

function pageTitle(doc: Document): string | undefined {
  const el = firstMatch(doc, TITLE_SELECTORS);
  const text = el ? textOf(el) : '';
  return text || normalizeWhitespace(doc.title ?? '') || undefined;
}

/**
 * An issue, pull request, or discussion as a conversation.
 *
 * Threaded GitHub pages really are conversations, and representing them as
 * such is what lets "what did we decide on that PR" work the same way it
 * does for a chat log.
 */
function extractThread(doc: Document): ExtractionResult | null {
  const containers = firstMatch(doc, COMMENT_SELECTORS)
    ? Array.from(doc.querySelectorAll(COMMENT_SELECTORS.join(',')))
    : [];
  if (!containers.length) return null;

  const messages: CaptureMessage[] = [];
  for (const container of containers) {
    const authorEl = container.querySelector(
      '.author, [data-testid="avatar-link"], a[data-hovercard-type="user"]',
    );
    const author = (authorEl ? textOf(authorEl) : '') || 'participant';
    const body =
      container.querySelector('.comment-body, .markdown-body, [data-testid="markdown-body"]') ??
      container;
    const { blocks } = extractBlocks(body);
    if (!blocks.length) continue;
    const timestamp =
      container.querySelector('relative-time, time')?.getAttribute('datetime') ?? undefined;
    messages.push({ role: author, blocks, timestamp });
  }

  if (!messages.length) return null;

  return {
    kind: 'conversation',
    strategy: 'site',
    extractorId: 'github',
    extractorVersion: 1,
    blocks: [],
    messages,
    coverage: 'rendered_dom',
    notes: [
      'GitHub loads long threads in pages; only the comments already on screen were captured.',
    ],
    truncated: false,
    title: pageTitle(doc),
  };
}

function extractRepository(doc: Document): ExtractionResult | null {
  const readme = firstMatch(doc, README_SELECTORS);
  if (!readme) return null;

  const blocks: ContentBlock[] = [];
  const about = doc.querySelector('.f4.my-3, [data-testid="repository-description"]');
  const description = about ? textOf(about) : '';
  if (description) blocks.push({ type: 'paragraph', text: description });
  blocks.push(...extractBlocks(readme).blocks);

  return {
    kind: 'repository',
    strategy: 'site',
    extractorId: 'github',
    extractorVersion: 1,
    blocks,
    messages: [],
    coverage: 'full_document',
    notes: ['Captured the repository description and README as rendered.'],
    truncated: false,
    title: pageTitle(doc),
  };
}

function extractCode(doc: Document, url: URL): ExtractionResult | null {
  const el = firstMatch(doc, CODE_SELECTORS);
  if (!el) return null;

  const source = (el as HTMLTextAreaElement).value ?? el.textContent ?? '';
  if (!source.trim()) return null;

  const language = url.pathname.split('.').pop()?.toLowerCase();
  // `/owner/repo/blob/<ref>/<path>` — drop the owner, repo, "blob" and ref so
  // the heading is the file's path in the repository. A ref containing a
  // slash (`feature/x`) leaves an extra segment in the heading; the code and
  // the URL in provenance are unaffected.
  const filePath = url.pathname.split('/').filter(Boolean).slice(4).join('/');
  return {
    kind: 'article',
    strategy: 'site',
    extractorId: 'github',
    extractorVersion: 1,
    blocks: [
      { type: 'heading', level: 2, text: filePath || 'Source' },
      { type: 'code', language, text: source.replace(/\r\n?/g, '\n') },
    ],
    messages: [],
    coverage: 'full_document',
    notes: ['Captured the file as rendered by GitHub’s source viewer.'],
    truncated: false,
    title: pageTitle(doc),
  };
}

export const githubExtractor: SiteExtractor = {
  id: 'github',
  version: 1,

  traversal: githubTraversal,

  matches(url: URL): boolean {
    return url.hostname.replace(/^www\./, '') === 'github.com';
  },

  extract(doc: Document, url: URL): ExtractionResult | null {
    const segments = url.pathname.split('/').filter(Boolean);
    const section = segments[2];

    if (section === 'issues' || section === 'pull' || section === 'discussions') {
      return extractThread(doc);
    }
    if (section === 'blob') {
      return extractCode(doc, url);
    }
    if (segments.length === 2 || section === 'tree') {
      return extractRepository(doc);
    }
    return null;
  },
};
