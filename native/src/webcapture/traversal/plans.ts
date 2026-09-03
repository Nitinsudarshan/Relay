/**
 * The default traversal plan, and the rules for choosing one.
 *
 * A source's plan ships next to its extractor (`extractors/chatgpt.ts` exports
 * both), so one file holds one site's knowledge and there is no second
 * URL-matching table to keep in step with the first. This module holds only
 * the plan for everything else — which is most of the web.
 */

import { budget } from './budget';
import type { TraversalPlan } from './types';

/**
 * The plan for a page no site extractor claims.
 *
 * Conservative on purpose. A document page does not unmount its content, so
 * the payoff from traversal is narrow and specific: lazily-loaded images gain
 * a `src` as they approach the viewport, and `<details>` sections and "read
 * more" controls disclose text that is genuinely not in the DOM. Feeds that
 * virtualize exist, so the same sample-and-merge path runs here too — a page
 * that needs one sample simply takes one.
 */
export const GENERIC_TRAVERSAL: TraversalPlan = {
  id: 'generic',
  scrollerSelectors: ['main', '[role="main"]', 'article'],
  contentSelectors: ['main', '[role="main"]', 'article', 'body'],
  itemSelectors: ['article', 'section', '[role="listitem"]', '[role="article"]'],
  expandSelectors: [],
  forbiddenSelectors: ['form', '[role="dialog"]', '[role="alertdialog"]'],
  loadingSelectors: ['[role="progressbar"]', '[aria-busy="true"]', '.loading', '.spinner'],
  // Most pages are already at the top when captured, and a page that is not
  // has to be read from the top anyway; the position is restored afterwards.
  rewind: true,
  expand: true,
  budget: budget({ maxMs: 6_000, maxSteps: 200, maxExpansions: 60 }),
};

/**
 * The plan for a page Relay will not traverse.
 *
 * Used when the document does not scroll and nothing looks virtualized: there
 * is nothing to reveal, so nothing is moved. `INSPECT FIRST` in its simplest
 * form.
 */
export const NO_TRAVERSAL: TraversalPlan = {
  ...GENERIC_TRAVERSAL,
  id: 'none',
  rewind: false,
  expand: false,
  budget: budget({ maxSteps: 0, maxMs: 0, maxExpansions: 0 }),
};
