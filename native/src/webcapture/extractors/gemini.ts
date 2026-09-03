/**
 * Gemini conversation extractor and traversal plan.
 *
 * Designed around Google Gemini's web application structure (gemini.google.com).
 *
 * Turns in Gemini are structured as:
 * - User query: `user-query`, `[data-test-id="user-query"]`, `.user-query-container`
 * - Model response: `model-response`, `[data-test-id="model-response"]`, `.response-container-content`
 *
 * Each strategy is layered so structural redesigns degrade gracefully rather
 * than costing the capture.
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

const HOSTS = new Set(['gemini.google.com']);

/** Selector for images inside turns. */
const IMAGE_SELECTOR = 'img:not([class*="avatar"]):not([aria-hidden="true"])';

/** Selector for uploaded or generated files. */
const FILE_SELECTOR = [
  'a[download]',
  '[data-test-id*="file"]',
  '[class*="file-attachment"]',
  '[class*="attachment-chip"]',
].join(',');

function roleOf(el: Element): string | null {
  if (
    el.matches('user-query, [data-test-id="user-query"], [class*="user-query"]') ||
    el.querySelector('user-query, [data-test-id="user-query"]')
  ) {
    return 'user';
  }
  if (
    el.matches('model-response, [data-test-id="model-response"], [class*="model-response"]') ||
    el.querySelector('model-response, [data-test-id="model-response"]')
  ) {
    return 'assistant';
  }
  return null;
}

function identityOf(el: Element): string | undefined {
  const own = el.getAttribute('data-message-id') ?? el.getAttribute('id');
  if (own) return own;
  const inner = el.querySelector('[data-message-id]')?.getAttribute('data-message-id');
  return inner ?? undefined;
}

function richContent(el: Element): ContentBlock[] {
  const role = roleOf(el) ?? (el.matches('[class*="user"]') ? 'user' : 'assistant');
  const blocks: ContentBlock[] = [];

  for (const img of Array.from(el.querySelectorAll(IMAGE_SELECTOR))) {
    const origin = role === 'user' ? 'user_upload' : 'assistant_generated';
    const block = imageBlockFrom(img, origin);
    if (block) blocks.push(block);
  }

  for (const file of Array.from(el.querySelectorAll(FILE_SELECTOR))) {
    const kind = role === 'user' ? 'user_upload' : 'assistant_generated';
    const block = attachmentBlockFrom(file, { kind });
    if (block) blocks.push(block);
  }

  return blocks;
}

const SPEC = {
  extractorId: 'gemini',
  extractorVersion: 1,
  strategies: [
    // Primary custom element turn structure
    [
      { selector: 'user-query', role: 'user' },
      { selector: 'model-response', role: 'assistant' },
    ],
    // Test ID anchored structure
    [
      { selector: '[data-test-id="user-query"]', role: 'user' },
      { selector: '[data-test-id="model-response"]', role: 'assistant' },
    ],
    // Class name fallback structure
    [
      { selector: 'div[class*="user-query"]', role: 'user' },
      { selector: 'div[class*="model-response"]', role: 'assistant' },
    ],
    // Container-level fallback
    [
      { selector: '.query-content, [class*="query-content"]', role: 'user' },
      { selector: '.response-content, [class*="response-content"]', role: 'assistant' },
    ],
  ],
  resolveRole: roleOf,
  scrollerSelector: 'infinite-scroller, [class*="chat-history"], [class*="conversation-container"], main',
  identity: identityOf,
  rich: richContent,
};

/**
 * How to walk a Gemini thread.
 */
export const geminiTraversal: TraversalPlan = {
  id: 'gemini',
  scrollerSelectors: [
    'infinite-scroller',
    '[class*="chat-history"]',
    '[class*="conversation-container"]',
    'main [class*="overflow-y"]',
    'main',
  ],
  contentSelectors: ['main', 'body'],
  itemSelectors: [
    'user-query',
    'model-response',
    '[data-test-id="user-query"]',
    '[data-test-id="model-response"]',
    'div[class*="user-query"]',
    'div[class*="model-response"]',
  ],
  expandSelectors: ['button[aria-expanded="false"]', '[class*="expand-button"]'],
  forbiddenSelectors: [
    'form',
    'fieldset',
    '[contenteditable="true"]',
    'rich-textarea',
    '[class*="input-area"]',
    '#prompt-textarea',
  ],
  loadingSelectors: ['[role="progressbar"]', 'mat-progress-bar', '[class*="loading"]', '[class*="sparkle"]'],
  rewind: true,
  expand: true,
  budget: budget(),
};

export const geminiExtractor: SiteExtractor = {
  id: 'gemini',
  version: 1,

  traversal: geminiTraversal,

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
