/**
 * Claude conversation extractor and traversal plan.
 *
 * Claude is deliberately *not* configured like ChatGPT, because it does not
 * behave like it (`docs/capture/RESEARCH.md` §2.3). The published record is
 * that the web UI loads every message into the DOM at once — virtual scrolling
 * was requested and closed as not planned — so the expensive part of traversal
 * buys nothing here. What Claude does do is shorten long user messages behind
 * a "Show more" control.
 *
 * And that control is the one this whole design turns on. A real capture of a
 * Claude thread with a visibly truncated message reported "the whole page was
 * captured" — and the text was in fact all there, because the shortening is
 * CSS, not omission. Chromium confirms it: a clamped box holding 2,904
 * characters returns all 2,904 from `textContent`. So Relay reads it and does
 * **not** click, and `expansions_unnecessary` counts how often that happened.
 * The bug was never the missing text. It was the claim.
 *
 * Artifacts are a genuine gap: they render in a side panel, one at a time, and
 * opening one changes the app's view state. Relay records the card and reports
 * the artifact as content it did not open.
 */

import { budget } from '../traversal/budget';
import type { TraversalPlan } from '../traversal/types';
import { attachmentBlockFrom, imageBlockFrom, normalizeWhitespace, textOf } from '../dom';
import { extractConversation, harvestConversation } from './conversation';
import type {
  ContentBlock,
  ExtractionResult,
  HarvestedItem,
  OffsetSource,
  SiteExtractor,
} from '../types';

/** Why an artifact's source is not in the capture. Stated on every artifact card. */
export const ARTIFACT_NOTE =
  'Claude shows this artifact in a side panel. Relay recorded the card but does not open panels, so the artifact’s own content was not captured.';

const ARTIFACT_SELECTOR = [
  '[data-testid="artifact-block-cell"]',
  '[class*="artifact"] button',
  'button[aria-label*="artifact" i]',
].join(',');

const FILE_SELECTOR = ['a[download]', '[data-testid*="file-thumbnail"]', '[data-testid*="attachment"]'].join(
  ',',
);

function identityOf(el: Element): string | undefined {
  return (
    el.getAttribute('data-message-uuid') ??
    el.closest('[data-message-uuid]')?.getAttribute('data-message-uuid') ??
    undefined
  );
}

/**
 * Artifact cards, uploaded files and images.
 *
 * An artifact is represented as an assistant-generated attachment with
 * `content_captured: false` and a note saying why — the honest shape for
 * something Relay can see the existence of but not the content of.
 */
function richContent(el: Element, role: 'user' | 'assistant'): ContentBlock[] {
  const blocks: ContentBlock[] = [];

  for (const img of Array.from(el.querySelectorAll('img'))) {
    const block = imageBlockFrom(img, role === 'user' ? 'user_upload' : 'assistant_generated');
    if (block) blocks.push(block);
  }

  for (const card of Array.from(el.querySelectorAll(ARTIFACT_SELECTOR))) {
    const label = normalizeWhitespace(textOf(card)).slice(0, 200);
    if (!label) continue;
    blocks.push({
      type: 'attachment',
      name: label,
      kind: 'assistant_generated',
      mime: 'artifact',
      content_captured: false,
      content_note: ARTIFACT_NOTE,
    });
  }

  for (const file of Array.from(el.querySelectorAll(FILE_SELECTOR))) {
    const block = attachmentBlockFrom(file, {
      kind: role === 'user' ? 'user_upload' : 'assistant_generated',
    });
    if (block) blocks.push(block);
  }

  return blocks;
}

function specFor(): Parameters<typeof extractConversation>[1] {
  return {
    extractorId: 'claude',
    extractorVersion: 2,
    strategies: [
      [
        { selector: '[data-testid="user-message"]', role: 'user' },
        { selector: '.font-claude-response', role: 'assistant' },
      ],
      [
        { selector: '[data-testid="user-message"]', role: 'user' },
        { selector: '[data-testid="assistant-message"]', role: 'assistant' },
      ],
      [
        { selector: '[data-test-render-count] [data-testid="user-message"]', role: 'user' },
        { selector: '.font-claude-message', role: 'assistant' },
      ],
    ],
    scrollerSelector: '[data-test-render-count], main',
    identity: identityOf,
    rich: (el) =>
      richContent(el, el.matches('[data-testid="user-message"]') ? 'user' : 'assistant'),
  };
}

/**
 * How to walk a Claude thread.
 *
 * `rewind` is on because a thread opened at the bottom still has to be read
 * from the top, and the engine restores the position afterwards. What makes
 * this cheap rather than expensive is the engine's own measurement: it checks
 * whether the first sample's turns are still attached after one step, finds
 * that they are, and goes straight to the end instead of walking there.
 */
export const claudeTraversal: TraversalPlan = {
  id: 'claude',
  scrollerSelectors: ['[data-test-render-count]', 'main [class*="overflow-y"]', 'main'],
  contentSelectors: ['main', 'body'],
  itemSelectors: ['[data-testid="user-message"]', '.font-claude-response', '[data-testid="assistant-message"]'],
  expandSelectors: [],
  forbiddenSelectors: ['form', 'fieldset', '[contenteditable="true"]', '[class*="composer"]'],
  loadingSelectors: ['[data-is-streaming="true"]', '[role="progressbar"]'],
  rewind: true,
  expand: true,
  budget: budget(),
};

export const claudeExtractor: SiteExtractor = {
  id: 'claude',
  version: 2,

  traversal: claudeTraversal,

  matches(url: URL): boolean {
    return url.hostname.replace(/^www\./, '') === 'claude.ai';
  },

  extract(doc: Document): ExtractionResult | null {
    return extractConversation(doc, specFor());
  },

  harvest(doc: Document, _url: URL, offsets?: OffsetSource): HarvestedItem[] {
    return harvestConversation(doc, specFor(), offsets);
  },
};
