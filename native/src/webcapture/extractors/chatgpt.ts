/**
 * ChatGPT conversation extractor and traversal plan.
 *
 * Two things changed here in v2, both from published evidence about the live
 * site (cited in `docs/capture/RESEARCH.md` §2.2):
 *
 * **The turn is the wrapper, not the role element.** v1 keyed on
 * `[data-message-author-role]`, which is where the *text* lives. Generated
 * images are rendered inside the `conversation-turn-N` wrapper but *outside*
 * that element, so an extractor anchored to the role attribute never sees
 * them. v2 anchors to the wrapper and reads the role from the descendant,
 * which also yields the turn's ordinal for free.
 *
 * **ChatGPT virtualizes long threads.** Turns scrolled out of view are removed
 * from the DOM, so a single pass over a long conversation can only ever return
 * a fragment. The plan below is what the reveal engine uses to walk it.
 *
 * Everything site-specific is unvalidated against the live site — this
 * container has no access to it — so each selector is one strategy in a
 * layered list, and a miss costs structure rather than the capture.
 */

import { budget } from '../traversal/budget';
import type { TraversalPlan } from '../traversal/types';
import { attachmentBlockFrom, imageBlockFrom } from '../dom';
import { extractConversation, harvestConversation } from './conversation';
import type {
  ContentBlock,
  ExtractionResult,
  HarvestedItem,
  OffsetSource,
  SiteExtractor,
} from '../types';

const HOSTS = new Set(['chatgpt.com', 'chat.openai.com']);

/** Screen-reader labels ChatGPT renders above each turn. */
const SPOKEN_ROLE = /(you|chatgpt)\s+said/i;

/** The same labels as content, which they are not. */
const SPOKEN_LABEL_ONLY = /^(you|chatgpt)\s+said:?$/i;

/** Authenticated asset endpoints images are served from. */
const GENERATED_IMAGE_SELECTOR = [
  'img[src*="/backend-api/estuary/content"]',
  'img[src*="/backend-api/files/"]',
  'img[alt^="Generated image" i]',
].join(',');

/** Files: the model's own output arrives as an opaque `sandbox:` reference. */
const FILE_SELECTOR = [
  'a[href^="sandbox:"]',
  'a[download]',
  'a[href*="/backend-api/files/"]',
  '[data-testid*="file-attachment"]',
].join(',');

function roleFromSpokenLabel(el: Element): string | null {
  const label = el.querySelector('h5, h6, [class*="sr-only"]');
  const match = SPOKEN_ROLE.exec(label?.textContent ?? '');
  if (!match) return null;
  return match[1].toLowerCase() === 'you' ? 'user' : 'assistant';
}

function roleOf(el: Element): string | null {
  const stated =
    el.getAttribute('data-message-author-role') ??
    el.querySelector('[data-message-author-role]')?.getAttribute('data-message-author-role');
  return stated ?? roleFromSpokenLabel(el);
}

/** `conversation-turn-7` → 7. The page's own ordering, never inferred. */
function ordinalOf(el: Element): number | undefined {
  const testid =
    el.getAttribute('data-testid') ??
    el.closest('[data-testid^="conversation-turn-"]')?.getAttribute('data-testid') ??
    '';
  const match = /conversation-turn-(\d+)/.exec(testid);
  if (!match) return undefined;
  const value = Number.parseInt(match[1], 10);
  return Number.isFinite(value) ? value : undefined;
}

function identityOf(el: Element): string | undefined {
  const own = el.getAttribute('data-message-id');
  if (own) return own;
  const inner = el.querySelector('[data-message-id]')?.getAttribute('data-message-id');
  return inner ?? undefined;
}

/**
 * Files and images, classified by where they came from.
 *
 * Provenance comes from the turn's role rather than from the image itself
 * wherever possible: an image inside a user turn was uploaded, an image inside
 * an assistant turn served from the model's asset endpoint was generated.
 * Nothing here fetches anything — see `docs/capture/RESEARCH.md` §5.1.
 */
function richContent(el: Element): ContentBlock[] {
  const role = roleOf(el);
  const blocks: ContentBlock[] = [];

  for (const img of Array.from(el.querySelectorAll('img'))) {
    const generated = img.matches(GENERATED_IMAGE_SELECTOR);
    const origin = generated
      ? 'assistant_generated'
      : role === 'user'
        ? 'user_upload'
        : role === 'assistant'
          ? 'assistant_generated'
          : 'unknown';
    const block = imageBlockFrom(img, origin);
    if (block) blocks.push(block);
  }

  for (const file of Array.from(el.querySelectorAll(FILE_SELECTOR))) {
    const href = file.getAttribute('href') ?? '';
    const kind = href.startsWith('sandbox:')
      ? 'assistant_generated'
      : role === 'user'
        ? 'user_upload'
        : 'linked';
    const block = attachmentBlockFrom(file, { kind });
    if (block) blocks.push(block);
  }

  return blocks;
}

const SPEC = {
  extractorId: 'chatgpt',
  extractorVersion: 2,
  strategies: [
    // The turn wrapper, with the role stated on a descendant. Also the only
    // strategy that sees generated images, which live in the wrapper.
    [
      { selector: 'article[data-testid^="conversation-turn-"]', role: null },
      { selector: 'section[data-testid^="conversation-turn-"]', role: null },
    ],
    // The role element itself: loses images rendered beside it, keeps the text.
    [{ selector: '[data-message-author-role]', role: null }],
    // The role is stated only in the turn's accessibility label.
    [{ selector: 'main article', role: null }],
  ],
  resolveRole: roleOf,
  scrollerSelector: 'main [class*="overflow-y-auto"], main',
  identity: identityOf,
  ordinal: ordinalOf,
  rich: richContent,
  dropText: SPOKEN_LABEL_ONLY,
};

/**
 * How to walk a ChatGPT thread.
 *
 * `rewind` matters more here than anywhere else: pinning to the top of a long
 * thread triggers a network fetch of older history, so the boundary itself
 * moves and one seek lands in the middle of the conversation.
 */
export const chatgptTraversal: TraversalPlan = {
  id: 'chatgpt',
  scrollerSelectors: [
    'main [class*="overflow-y-auto"]',
    'main [class*="react-scroll-to-bottom"]',
    '[role="presentation"] > div',
    'main',
  ],
  contentSelectors: ['main', 'body'],
  itemSelectors: ['[data-testid^="conversation-turn-"]', '[data-message-author-role]'],
  expandSelectors: [],
  // The composer is a message waiting to be sent. Nothing inside it is ever
  // activated, whatever it is labelled.
  forbiddenSelectors: ['form', '[data-testid="composer"]', '[class*="composer"]', '#prompt-textarea'],
  loadingSelectors: ['[role="progressbar"]', '.result-streaming', '[data-testid="loading"]'],
  rewind: true,
  expand: true,
  budget: budget(),
};

export const chatgptExtractor: SiteExtractor = {
  id: 'chatgpt',
  version: 2,

  traversal: chatgptTraversal,

  matches(url: URL): boolean {
    return HOSTS.has(url.hostname.replace(/^www\./, ''));
  },

  extract(doc: Document): ExtractionResult | null {
    return extractConversation(doc, SPEC);
  },

  harvest(doc: Document, _url: URL, offsets?: OffsetSource): HarvestedItem[] {
    return harvestConversation(doc, SPEC, offsets);
  },
};
