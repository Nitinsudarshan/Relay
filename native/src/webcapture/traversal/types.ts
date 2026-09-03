/**
 * The traversal contract.
 *
 * The engine's whole job is to *expose* content: move the page, wait for it to
 * settle, open what is genuinely closed, and tell someone else to read. It has
 * no concept of a message, a role, an attachment or a conversation — those
 * live in the extractors, on the other side of the `sample` callback. Keeping
 * that line clean is what stops this file growing into a site scraper.
 */

import type { TraversalBudget } from './budget';

export type { TraversalBudget } from './budget';

/**
 * The thing that scrolls.
 *
 * Abstracted because it is either an element or the window, because a page can
 * lie about both, and because a layout-free document (jsdom, and therefore
 * every unit test) has to be able to stand in for one.
 */
export interface ScrollSurface {
  /** The scrolling element, or `null` when the document scrolls. */
  readonly element: HTMLElement | null;
  /** How the surface was found, for the diagnostics record. */
  readonly kind: 'element' | 'window' | 'stub';
  scrollTop(): number;
  scrollTo(top: number): void;
  viewportHeight(): number;
  scrollHeight(): number;
  /** The largest meaningful `scrollTop`. Zero when nothing scrolls. */
  maxScroll(): number;
  /** Offset of an element within this surface's scrolling coordinate space. */
  offsetOf(el: Element): number;
}

/**
 * A source's traversal strategy.
 *
 * Everything site-specific about *revealing* a page is here, as data. A plan
 * ships next to its extractor so one file holds one site's knowledge.
 */
export interface TraversalPlan {
  id: string;
  /** Candidate scrolling elements, best first. Falls back to the window. */
  scrollerSelectors: string[];
  /** The region that holds content. Expansion never reaches outside it. */
  contentSelectors: string[];
  /**
   * The repeating unit of content. Its count is the cheap progress signal, and
   * its extent is what makes the step size adaptive rather than a fixed guess.
   */
  itemSelectors: string[];
  /** Disclosure controls this source is known to use. Verified per site. */
  expandSelectors: string[];
  /** Regions nothing may ever be activated inside, beyond the generic rules. */
  forbiddenSelectors: string[];
  /** Elements whose presence means content is still arriving. */
  loadingSelectors: string[];
  /**
   * Whether to seek to the start of the document before reading. Off for
   * sources that mount everything, where it would only move someone's page.
   */
  rewind: boolean;
  /** Whether this source has disclosure controls worth looking for at all. */
  expand: boolean;
  budget: TraversalBudget;
}

/** Injected clock, waiter and mutation observer, so the loop is testable. */
export interface TraversalDeps {
  now: () => number;
  wait: (ms: number) => Promise<void>;
  /** Overrides surface resolution. Tests supply a layout-free stand-in. */
  surface?: ScrollSurface;
}
