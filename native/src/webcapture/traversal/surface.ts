/**
 * Resolving the thing that scrolls, and reading it safely.
 *
 * Two properties matter here. The first is that a page's own claim about its
 * scroller is not trusted: a selector match only counts if the element
 * actually overflows, because chat UIs leave several `overflow-y-auto`
 * containers in the tree and only one of them is the conversation. The second
 * is that every read is defensive — a page can replace or detach the surface
 * mid-traversal, and a capture must degrade rather than throw.
 */

import type { ScrollSurface, TraversalPlan } from './types';

/** Below this an element is not really scrolling, it just has the CSS for it. */
const MIN_OVERFLOW_PX = 40;

function elementSurface(el: HTMLElement): ScrollSurface {
  return {
    element: el,
    kind: 'element',
    scrollTop: () => el.scrollTop ?? 0,
    scrollTo: (top) => {
      el.scrollTop = top;
    },
    viewportHeight: () => el.clientHeight ?? 0,
    scrollHeight: () => el.scrollHeight ?? 0,
    maxScroll: () => Math.max(0, (el.scrollHeight ?? 0) - (el.clientHeight ?? 0)),
    offsetOf: (target) => {
      try {
        const box = target.getBoundingClientRect();
        const own = el.getBoundingClientRect();
        return Math.round(box.top - own.top + (el.scrollTop ?? 0));
      } catch {
        return 0;
      }
    },
  };
}

function windowSurface(doc: Document): ScrollSurface {
  const view = doc.defaultView;
  const root = (doc.scrollingElement ?? doc.documentElement) as HTMLElement | null;
  const height = () => view?.innerHeight ?? root?.clientHeight ?? 0;
  const total = () => Math.max(root?.scrollHeight ?? 0, doc.body?.scrollHeight ?? 0);

  return {
    element: null,
    kind: 'window',
    scrollTop: () => view?.scrollY ?? root?.scrollTop ?? 0,
    scrollTo: (top) => {
      if (view?.scrollTo) view.scrollTo(0, top);
      else if (root) root.scrollTop = top;
    },
    viewportHeight: height,
    scrollHeight: total,
    maxScroll: () => Math.max(0, total() - height()),
    offsetOf: (target) => {
      try {
        const box = target.getBoundingClientRect();
        return Math.round(box.top + (view?.scrollY ?? 0));
      } catch {
        return 0;
      }
    },
  };
}

/**
 * A surface for a document with no layout.
 *
 * jsdom reports every box as zero-sized, so without this every unit test would
 * exercise the "nothing scrolls" branch and the loop itself would go untested.
 * Tests build one of these with the geometry they want to simulate.
 */
export function stubSurface(state: {
  scrollHeight: number;
  viewportHeight: number;
  scrollTop?: number;
  offsets?: WeakMap<Element, number>;
}): ScrollSurface & { history: number[] } {
  let top = state.scrollTop ?? 0;
  const history: number[] = [top];
  return {
    element: null,
    kind: 'stub',
    history,
    scrollTop: () => top,
    scrollTo: (next) => {
      top = Math.max(0, Math.min(next, Math.max(0, state.scrollHeight - state.viewportHeight)));
      history.push(top);
    },
    viewportHeight: () => state.viewportHeight,
    scrollHeight: () => state.scrollHeight,
    maxScroll: () => Math.max(0, state.scrollHeight - state.viewportHeight),
    offsetOf: (el) => state.offsets?.get(el) ?? 0,
  };
}

/**
 * Picks the surface the plan's content actually scrolls in.
 *
 * A selector match must also overflow to win. When none does, the window is
 * the answer — which is the right answer for most of the web, and the safe one
 * everywhere else.
 */
export function resolveSurface(doc: Document, plan: TraversalPlan): ScrollSurface {
  for (const selector of plan.scrollerSelectors) {
    let candidates: Element[] = [];
    try {
      candidates = Array.from(doc.querySelectorAll(selector));
    } catch {
      // A selector the browser rejects is a bug in a plan, not a reason to
      // fail a capture.
      continue;
    }
    for (const candidate of candidates) {
      const el = candidate as HTMLElement;
      const overflow = (el.scrollHeight ?? 0) - (el.clientHeight ?? 0);
      if (overflow > MIN_OVERFLOW_PX) return elementSurface(el);
    }
  }
  return windowSurface(doc);
}

/** The plan's content region, or the body when it names none that exist. */
export function resolveContentRoot(doc: Document, plan: TraversalPlan): Element {
  for (const selector of plan.contentSelectors) {
    try {
      const el = doc.querySelector(selector);
      if (el) return el;
    } catch {
      continue;
    }
  }
  return doc.body ?? doc.documentElement;
}

/** Everything currently mounted that the plan calls an item. */
export function mountedItems(root: ParentNode, plan: TraversalPlan): Element[] {
  const selector = plan.itemSelectors.join(',');
  if (!selector) return [];
  try {
    return Array.from(root.querySelectorAll(selector));
  } catch {
    return [];
  }
}
