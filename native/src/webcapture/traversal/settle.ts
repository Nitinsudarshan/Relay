/**
 * Deciding when the page has stopped changing.
 *
 * The alternative — sleeping for a fixed interval after every move — is either
 * too short (and loses content that was still arriving) or too long (and turns
 * a forty-step traversal into a minute). So settling is *observed*: a
 * `MutationObserver` reports whether anything changed, a cheap signature
 * catches changes an observer on one root would miss, and the wait ends as
 * soon as both go quiet.
 *
 * A typical settle costs one or two 50ms polls. The ceiling exists only for
 * pages where content never stops arriving.
 */

import { mountedItems } from './surface';
import type { ScrollSurface, TraversalBudget, TraversalDeps, TraversalPlan } from './types';

export interface SettleResult {
  /** False when the ceiling was reached with the page still changing. */
  settled: boolean
  ms: number;
  mutations: number;
  /** True while a loading indicator the plan knows about was on screen. */
  loading: boolean;
}

/**
 * A cheap fingerprint of "how much is here".
 *
 * Deliberately not the text: reading `textContent` of a large subtree on every
 * poll is the one thing that would make settling more expensive than the
 * traversal it serves. Height, item count and element count move whenever
 * content does, and all three are O(1)-ish reads the browser already
 * maintains.
 */
export function contentSignature(
  root: Element,
  plan: TraversalPlan,
  scrollHeight = 0,
): string {
  let elements = 0;
  try {
    elements = root.getElementsByTagName('*').length;
  } catch {
    elements = root.childElementCount ?? 0;
  }
  return `${scrollHeight}:${mountedItems(root, plan).length}:${elements}`;
}

function isLoading(doc: Document, plan: TraversalPlan): boolean {
  const selector = plan.loadingSelectors.join(',');
  if (!selector) return false;
  try {
    return doc.querySelector(selector) !== null;
  } catch {
    return false;
  }
}

/**
 * Waits until the page stops changing, or until the ceiling.
 *
 * The observer is created per call and disconnected in a `finally`: a
 * long-lived observer over a conversation's subtree is a memory cost paid for
 * every step, and leaving one attached after a capture would be a leak on
 * someone else's page.
 */
export async function waitForSettle(
  doc: Document,
  root: Element,
  surface: ScrollSurface,
  plan: TraversalPlan,
  budget: TraversalBudget,
  deps: TraversalDeps,
): Promise<SettleResult> {
  const started = deps.now();
  let mutations = 0;
  let sawLoading = false;

  const Observer = (doc.defaultView as unknown as { MutationObserver?: typeof MutationObserver })
    ?.MutationObserver ?? (typeof MutationObserver === 'function' ? MutationObserver : undefined);

  const observer = Observer
    ? new Observer((records) => {
        mutations += records.length;
      })
    : null;

  try {
    observer?.observe(root, {
      childList: true,
      subtree: true,
      characterData: true,
      attributes: false,
    });

    let previous = contentSignature(root, plan, surface.scrollHeight());
    let quiet = 0;

    while (deps.now() - started < budget.settleMaxMs) {
      const before = mutations;
      await deps.wait(budget.settlePollMs);

      const loading = isLoading(doc, plan);
      sawLoading = sawLoading || loading;
      const signature = contentSignature(root, plan, surface.scrollHeight());
      const changed = signature !== previous || mutations !== before;
      previous = signature;

      if (changed || loading) {
        quiet = 0;
        continue;
      }

      quiet += 1;
      if (quiet >= budget.settleQuietTicks) {
        return { settled: true, ms: deps.now() - started, mutations, loading: sawLoading };
      }
    }

    return { settled: false, ms: deps.now() - started, mutations, loading: sawLoading };
  } finally {
    observer?.disconnect();
  }
}
