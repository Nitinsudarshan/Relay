/**
 * Shared machinery for conversation extractors.
 *
 * Every chat interface Relay knows about has the same shape underneath —
 * turns, in document order, each attributable to a role — and the same
 * hazard: the DOM holds only the turns the app has chosen to render. That
 * hazard is handled once, here, so a site extractor is only ever a list of
 * selectors and a role rule.
 */

import { extractBlocks, isHidden, looksVirtualized } from '../dom';
import type { CaptureMessage, ContentBlock, ExtractionResult } from '../types';

export const MAX_MESSAGES = 2000;

/** A selector paired with the role every element it matches represents. */
export interface TurnSelector {
  selector: string;
  /** `null` means "read the role from the element" via `resolveRole`. */
  role: string | null;
}

export interface ConversationSpec {
  extractorId: string;
  extractorVersion: number;
  /** Ordered strategies; the first that yields turns wins. */
  strategies: TurnSelector[][];
  /** Reads a role off an element when the selector alone does not fix it. */
  resolveRole?: (el: Element) => string | null;
  /** The element that scrolls, used to tell whether earlier turns are loaded. */
  scrollerSelector?: string;
}

function turnsFor(doc: Document, selectors: TurnSelector[]): { el: Element; role: string }[] {
  const combined = selectors.map((s) => s.selector).join(',');
  if (!combined) return [];

  const found: { el: Element; role: string }[] = [];
  // One query for every selector at once, because `querySelectorAll` returns
  // document order — which is turn order, and is not recoverable by querying
  // each role separately and merging.
  for (const el of Array.from(doc.querySelectorAll(combined))) {
    if (isHidden(el)) continue;
    const matched = selectors.find((s) => el.matches(s.selector));
    if (!matched) continue;
    found.push({ el, role: matched.role ?? '' });
  }
  return found;
}

/**
 * Notes what the page's own scroll state implies about completeness.
 *
 * A conversation scrolled to the bottom of a long thread usually means the
 * earlier turns were never rendered. Relay reports that rather than
 * scrolling the page itself: scrolling someone's page to read more of it is
 * a side effect a capture should not have, and it still would not prove the
 * result was complete.
 */
function coverageNotes(doc: Document, spec: ConversationSpec, turnCount: number): string[] {
  const notes: string[] = [
    'Only the turns the page had rendered were captured. Long conversations load earlier turns as you scroll up.',
  ];

  if (looksVirtualized(doc)) {
    notes.push('This conversation is rendered as a virtualized list, so off-screen turns were not in the page at all.');
  }

  if (spec.scrollerSelector) {
    const scroller = doc.querySelector(spec.scrollerSelector) as HTMLElement | null;
    if (scroller && typeof scroller.scrollTop === 'number' && scroller.scrollTop > 0) {
      notes.push('The conversation was not scrolled to its beginning when it was captured.');
    }
  }

  if (turnCount === 0) {
    notes.push('No conversation turns were recognised.');
  }
  return notes;
}

/**
 * Runs a conversation spec against a document.
 *
 * Returns `null` when no strategy matched, which hands the page to the
 * generic extractor rather than producing an empty conversation — a site
 * redesign should cost structure, not the capture.
 */
export function extractConversation(doc: Document, spec: ConversationSpec): ExtractionResult | null {
  // Every strategy is tried, and the one that recognises the most turns wins
  // — not simply the first that matches anything. A redesign usually leaves
  // *part* of the old markup in place, and a strategy that finds only the
  // user's turns would otherwise silently produce a half conversation.
  let turns: { el: Element; role: string }[] = [];
  let strategyIndex = -1;

  for (let i = 0; i < spec.strategies.length; i += 1) {
    const found = turnsFor(doc, spec.strategies[i]);
    if (found.length > turns.length) {
      turns = found;
      strategyIndex = i;
    }
  }

  if (!turns.length) return null;

  const truncated = turns.length > MAX_MESSAGES;
  const messages: CaptureMessage[] = [];

  for (const { el, role } of turns.slice(0, MAX_MESSAGES)) {
    const resolved = role || spec.resolveRole?.(el) || 'participant';
    const { blocks } = extractBlocks(el);
    const usable: ContentBlock[] = blocks.filter(
      (block) => block.type !== 'image' || Boolean(block.alt) || Boolean(block.src),
    );
    if (!usable.length) continue;
    messages.push({ role: resolved, blocks: usable });
  }

  if (!messages.length) return null;

  const notes = coverageNotes(doc, spec, messages.length);
  if (strategyIndex > 0) {
    // The primary selectors did not match: the site has changed shape and
    // the capture is running on a fallback. Worth saying out loud, because
    // it is the early warning that an extractor needs updating.
    notes.push('The page did not match Relay’s primary layout for this site, so a fallback was used and roles may be less reliable.');
  }

  return {
    kind: 'conversation',
    strategy: 'site',
    extractorId: spec.extractorId,
    extractorVersion: spec.extractorVersion,
    blocks: [],
    messages,
    coverage: 'rendered_dom',
    notes,
    truncated,
  };
}
