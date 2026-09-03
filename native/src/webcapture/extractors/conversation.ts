/**
 * Shared machinery for conversation extractors.
 *
 * Every chat interface Relay knows about has the same shape underneath —
 * turns, in order, each attributable to a role, each possibly carrying files
 * and images — and the same hazard: the DOM holds only the turns the app has
 * chosen to render. That hazard is handled once, here, so a site extractor is
 * a list of selectors, a role rule, and the two or three site-specific things
 * a generic block walk cannot recognise.
 *
 * The important structural point is `harvest`: it answers "what is mounted
 * right now", the reveal engine calls it after every settle, and `extract` is
 * that same harvest assembled into a result. One set of selectors, used by
 * both paths, so a site redesign breaks one thing rather than two.
 */

import { extractBlocks, isHidden, looksVirtualized, normalizeWhitespace } from '../dom';
import { fingerprint, weigh } from '../merge';
import type {
  CaptureMessage,
  ContentBlock,
  ExtractionResult,
  HarvestedItem,
  OffsetSource,
} from '../types';

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
  /** Ordered strategies; the one that recognises the most turns wins. */
  strategies: TurnSelector[][];
  /** Reads a role off an element when the selector alone does not fix it. */
  resolveRole?: (el: Element) => string | null;
  /** The element that scrolls, used to tell whether earlier turns are loaded. */
  scrollerSelector?: string;
  /**
   * The page's own stable identifier for a turn, where it has one. Beats a
   * content fingerprint for recognising the same turn across samples, because
   * it survives the turn's text changing when the turn is expanded.
   */
  identity?: (el: Element) => string | undefined;
  /** The page's own turn number, where it exposes one. Never invented. */
  ordinal?: (el: Element) => number | undefined;
  /**
   * Content a generic block walk cannot classify: file cards, generated
   * images, artifact cards. Returned as blocks so it lands in the message it
   * belongs to, in reading order, with no parallel association table.
   */
  rich?: (el: Element) => ContentBlock[];
  /**
   * Text the site renders for screen readers or as chrome inside a turn, which
   * is not content. Matched against a whole block's text.
   */
  dropText?: RegExp;
}

function turnsFor(
  doc: Document,
  selectors: TurnSelector[],
  failures: string[],
): { el: Element; role: string }[] {
  const combined = selectors.map((s) => s.selector).join(',');
  if (!combined) return [];

  const found: { el: Element; role: string }[] = [];
  // One query for every selector at once, because `querySelectorAll` returns
  // document order — which is turn order, and is not recoverable by querying
  // each role separately and merging.
  let matched: Element[] = [];
  try {
    matched = Array.from(doc.querySelectorAll(combined));
  } catch {
    // A strategy whose selectors the browser will not accept costs that
    // strategy, not the capture — but it is recorded, because a selector that
    // stopped parsing is a site redesign announcing itself.
    failures.push(combined);
    return [];
  }

  for (const el of matched) {
    if (isHidden(el)) continue;
    // A turn nested inside another matched turn is the same turn seen twice —
    // which happens the moment a strategy names both a wrapper and the element
    // inside it.
    if (found.some((other) => other.el.contains(el))) continue;
    const selector = selectors.find((s) => el.matches(s.selector));
    if (!selector) continue;
    found.push({ el, role: selector.role ?? '' });
  }
  return found;
}

/** Picks the strategy that recognises the most turns. */
export function selectStrategy(
  doc: Document,
  spec: ConversationSpec,
): { turns: { el: Element; role: string }[]; index: number; failures: string[] } {
  let turns: { el: Element; role: string }[] = [];
  let index = -1;
  const failures: string[] = [];

  // Every strategy is tried, and the one that recognises the most turns wins
  // — not simply the first that matches anything. A redesign usually leaves
  // *part* of the old markup in place, and a strategy that finds only the
  // user's turns would otherwise silently produce a half conversation.
  for (let i = 0; i < spec.strategies.length; i += 1) {
    const found = turnsFor(doc, spec.strategies[i], failures);
    if (found.length > turns.length) {
      turns = found;
      index = i;
    }
  }
  return { turns, index, failures };
}

/** Blocks for one turn: the generic walk, plus what only the site knows. */
export function blocksForTurn(el: Element, spec: ConversationSpec): ContentBlock[] {
  const { blocks } = extractBlocks(el);
  const rich = spec.rich?.(el) ?? [];

  const usable = [...blocks, ...rich].filter((block) => {
    if (block.type === 'image') {
      return Boolean(block.alt || block.src || block.reference || block.caption);
    }
    if (block.type === 'attachment') {
      return Boolean(block.name || block.href || block.reference);
    }
    if (spec.dropText && 'text' in block) {
      return !spec.dropText.test(normalizeWhitespace(block.text));
    }
    return true;
  });

  // The rich pass and the block walk can both see the same image; the walk
  // finds `<img>` elements and the rich pass classifies them, so a turn that
  // has both keeps the classified one.
  const seen = new Set<string>();
  const deduped: ContentBlock[] = [];
  for (const block of usable.slice().reverse()) {
    const key = fingerprint(block.type, [block]);
    if (seen.has(key)) continue;
    seen.add(key);
    deduped.unshift(block);
  }
  return deduped;
}

/**
 * What is mounted right now, as harvestable items.
 *
 * Called once per sample by the reveal engine, and once by `extract` for the
 * single-pass case. Returns `[]` rather than throwing when no strategy
 * matches, which is what lets the page fall to the generic extractor.
 */
export function harvestConversation(
  doc: Document,
  spec: ConversationSpec,
  offsets?: OffsetSource,
): HarvestedItem[] {
  const { turns } = selectStrategy(doc, spec);
  const items: HarvestedItem[] = [];

  for (const { el, role } of turns) {
    const blocks = blocksForTurn(el, spec);
    if (!blocks.length) continue;

    const resolved = role || spec.resolveRole?.(el) || 'participant';
    items.push({
      identity: spec.identity?.(el),
      ordinal: spec.ordinal?.(el),
      offset: offsets?.offsetOf(el) ?? 0,
      role: resolved,
      blocks,
      fingerprint: fingerprint(resolved, blocks),
      weight: weigh(blocks),
    });
  }
  return items;
}

/**
 * Notes what the page's own state implies about completeness.
 *
 * These are the statements a *single-pass* capture can make. When a reveal
 * pass ran, the orchestrator replaces them with what it measured, because a
 * measurement beats an implication.
 */
export function coverageNotes(doc: Document, spec: ConversationSpec, turnCount: number): string[] {
  const notes: string[] = [
    'Only the turns the page had rendered were captured. Long conversations load earlier turns as you scroll up.',
  ];

  if (looksVirtualized(doc)) {
    notes.push(
      'This conversation is rendered as a virtualized list, so off-screen turns were not in the page at all.',
    );
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
 * Runs a conversation spec against a document, in one pass.
 *
 * Returns `null` when no strategy matched, which hands the page to the
 * generic extractor rather than producing an empty conversation — a site
 * redesign should cost structure, not the capture.
 */
export function extractConversation(doc: Document, spec: ConversationSpec): ExtractionResult | null {
  const { turns, index, failures } = selectStrategy(doc, spec);
  if (!turns.length) return null;

  const truncated = turns.length > MAX_MESSAGES;
  const messages: CaptureMessage[] = [];

  for (const { el, role } of turns.slice(0, MAX_MESSAGES)) {
    const blocks = blocksForTurn(el, spec);
    if (!blocks.length) continue;
    messages.push({
      role: role || spec.resolveRole?.(el) || 'participant',
      blocks,
      ordinal: spec.ordinal?.(el),
    });
  }

  if (!messages.length) return null;

  const notes = coverageNotes(doc, spec, messages.length);
  if (index > 0) {
    // The primary selectors did not match: the site has changed shape and
    // the capture is running on a fallback. Worth saying out loud, because
    // it is the early warning that an extractor needs updating.
    notes.push(
      'The page did not match Relay’s primary layout for this site, so a fallback was used and roles may be less reliable.',
    );
  }
  if (failures.length > 0) {
    notes.push(
      `${failures.length} of Relay’s selectors for this site are no longer valid in this browser, so part of the page may have been read a weaker way.`,
    );
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
