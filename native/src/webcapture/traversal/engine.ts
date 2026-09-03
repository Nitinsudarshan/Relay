/**
 * The reveal engine.
 *
 * Inspect, then reveal only what needs revealing, then traverse only when
 * traversal is what is missing. The loop is deliberately dull: the interesting
 * decisions live in `expand.ts` (may this be activated, and is it necessary)
 * and in `settle.ts` (has the page stopped changing). What this file owns is
 * *movement* — where to go next, when to stop, and putting the page back.
 *
 * It knows nothing about messages. Content is read through the `sample`
 * callback, which the caller supplies and which returns how many new items it
 * took. That number is the engine's only notion of progress.
 */

import { emptyTraversalDiagnostics } from '../types';
import type { TraversalDiagnostics } from '../types';
import { classifyExpansion, findCandidates, installExpansionGuards } from './expand';
import { contentSignature, waitForSettle } from './settle';
import { mountedItems, resolveContentRoot, resolveSurface } from './surface';
import type { ScrollSurface, TraversalDeps, TraversalPlan } from './types';

export interface TraversalRun {
  diagnostics: TraversalDiagnostics;
}

export interface TraversalOptions {
  plan: TraversalPlan;
  /**
   * Reads whatever is mounted right now and returns how many *new* items it
   * kept. Called after every settle, because on a virtualizing page content
   * exists only while it is mounted: a read after the traversal would see the
   * end of a conversation and nothing else.
   */
  sample: (doc: Document, surface: ScrollSurface) => number;
  deps?: Partial<TraversalDeps>;
}

function defaultDeps(doc: Document): TraversalDeps {
  const view = doc.defaultView;
  return {
    now: () => (view?.performance?.now ? view.performance.now() : Date.now()),
    wait: (ms) =>
      new Promise((resolve) => {
        if (view?.setTimeout) view.setTimeout(resolve, ms);
        else setTimeout(resolve, ms);
      }),
  };
}

/**
 * Stops the traversal the moment the user touches their own page.
 *
 * A capture is a background courtesy. It does not get to fight someone for
 * control of the thing they are reading, so any wheel, touch or key event
 * ends the run, restores the position and reports what was collected.
 */
function watchForInterruption(doc: Document): { interrupted: () => boolean; stop: () => void } {
  let hit = false;
  const mark = () => {
    hit = true;
  };
  const events = ['wheel', 'touchstart', 'keydown', 'mousedown'] as const;
  for (const name of events) {
    doc.addEventListener(name, mark, { capture: true, passive: true });
  }
  return {
    interrupted: () => hit,
    stop: () => {
      for (const name of events) doc.removeEventListener(name, mark, true);
    },
  };
}

/**
 * How far to move next.
 *
 * Stepping by a fixed slice of the viewport is the obvious choice and the
 * expensive one: a 1,000-turn thread at ~400px a turn is ~400,000px, which at
 * 600px a step is 660 steps. Instead the step is measured from what is
 * actually mounted — advance to just short of the last mounted item, keeping
 * one item of overlap so nothing can be unmounted in the gap between two
 * reads.
 *
 * On a page that mounts everything this reaches the end in two or three steps.
 * On a virtualizing one it advances by the whole mounted window rather than by
 * one screen. Verified in Chromium: 500 virtualized items, 83 steps, every
 * item seen.
 */
export function nextScrollTop(
  surface: ScrollSurface,
  items: Element[],
  plan: TraversalPlan,
): number {
  const current = surface.scrollTop();
  const viewport = surface.viewportHeight();
  const fallback = current + Math.max(1, Math.round(viewport * plan.budget.overlap));

  if (items.length === 0) return fallback;

  let bottom = -Infinity;
  let lastHeight = 0;
  for (const item of items) {
    const offset = surface.offsetOf(item);
    const height = (item as HTMLElement).offsetHeight ?? 0;
    if (offset + height > bottom) {
      bottom = offset + height;
      lastHeight = height;
    }
  }
  if (!Number.isFinite(bottom)) return fallback;

  // One item of overlap, so a step cannot unmount an item before it has been
  // read.
  const target = Math.round(
    bottom - Math.max(lastHeight, Math.round(viewport * (1 - plan.budget.overlap))),
  );

  // A target *behind* the current position means the mounted items are above
  // us and whatever is below is not mounted as an item — a lazily-appended
  // article section, say. Step by a viewport instead.
  //
  // The tempting alternative, nudging forward by a pixel to guarantee
  // progress, is a treadmill: measured here at 113 steps and a full 6-second
  // budget to advance 113 pixels, on a page whose content was 900px further
  // down.
  return target > current ? target : fallback;
}

/**
 * Opens what is genuinely closed, within the current window.
 *
 * Returns counts rather than mutating the diagnostics, so the classifier's
 * decisions and the engine's bookkeeping stay separable and testable.
 */
export async function expandHere(
  doc: Document,
  contentRoot: Element,
  plan: TraversalPlan,
  activated: WeakSet<Element>,
  limits: { remaining: number; perStep: number },
): Promise<{
  found: number;
  opened: number;
  refused: number;
  failed: number;
  unnecessary: number;
  availability: Record<string, number>;
  navigated: boolean;
}> {
  const result = {
    found: 0,
    opened: 0,
    refused: 0,
    failed: 0,
    unnecessary: 0,
    availability: {} as Record<string, number>,
    navigated: false,
  };
  if (!plan.expand || limits.remaining <= 0) return result;

  const bump = (state: string) => {
    result.availability[state] = (result.availability[state] ?? 0) + 1;
  };

  const href = doc.defaultView?.location?.href ?? '';
  const removeGuards = installExpansionGuards(doc);

  try {
    let openedHere = 0;
    for (const candidate of findCandidates(contentRoot, plan)) {
      if (openedHere >= limits.perStep || result.opened >= limits.remaining) break;

      const verdict = classifyExpansion(candidate.el, plan, contentRoot, activated);
      if (verdict.decision === 'refuse') {
        // Only controls that looked like disclosure controls are worth
        // counting as refusals; every button on the page is not a "finding".
        if (verdict.reason !== 'no disclosure evidence') {
          result.found += 1;
          result.refused += 1;
        }
        continue;
      }

      result.found += 1;
      bump(verdict.availability);

      if (verdict.decision === 'unnecessary') {
        result.unnecessary += 1;
        continue;
      }

      const before = contentSignature(contentRoot, plan);
      activated.add(candidate.el);
      try {
        (candidate.el as HTMLElement).click();
      } catch {
        result.failed += 1;
        continue;
      }
      openedHere += 1;

      const now = doc.defaultView?.location?.href ?? '';
      if (now !== href) {
        // Something navigated despite the pre-flight check. Stop expanding
        // entirely: whatever this page is now, it is not the one being read.
        result.navigated = true;
        result.opened += 1;
        break;
      }

      const after = contentSignature(contentRoot, plan);
      const expandedNow = candidate.el.getAttribute('aria-expanded') === 'true';
      if (after !== before || expandedNow) {
        result.opened += 1;
      } else {
        // Clicked, nothing changed. Counted rather than retried — a control
        // that stopped working is how a site redesign announces itself.
        result.failed += 1;
      }
    }
  } finally {
    removeGuards();
  }

  return result;
}

/**
 * Runs one traversal over a document.
 *
 * Always returns diagnostics, never throws: a reveal pass that fails is a
 * capture that reports `failed` and keeps whatever it had, not a capture that
 * is lost.
 */
export async function traverse(
  doc: Document,
  options: TraversalOptions,
): Promise<TraversalDiagnostics> {
  const plan = options.plan;
  const deps: TraversalDeps = { ...defaultDeps(doc), ...options.deps };
  const diagnostics = emptyTraversalDiagnostics(plan.id);
  const started = deps.now();

  const surface = deps.surface ?? resolveSurface(doc, plan);
  const contentRoot = resolveContentRoot(doc, plan);
  const interruption = watchForInterruption(doc);

  const originalWindowScroll = doc.defaultView?.scrollY ?? 0;
  const originalSurfaceScroll = surface.scrollTop();
  const activated = new WeakSet<Element>();

  let lowest = originalSurfaceScroll;
  let highest = originalSurfaceScroll;
  let idle = 0;
  let expansionsLeft = plan.budget.maxExpansions;

  const outOfTime = () => deps.now() - started >= plan.budget.maxMs;

  const settleOnce = async () => {
    const settle = await waitForSettle(doc, contentRoot, surface, plan, plan.budget, deps);
    if (!settle.settled) diagnostics.settle_timeouts += 1;
  };

  const takeSample = async () => {
    diagnostics.samples += 1;
    const kept = options.sample(doc, surface);
    if (kept > 0) idle = 0;
    else idle += 1;
    return kept;
  };

  const expandStep = async () => {
    const outcome = await expandHere(doc, contentRoot, plan, activated, {
      remaining: expansionsLeft,
      perStep: plan.budget.maxExpansionsPerStep,
    });
    diagnostics.expansions_found += outcome.found;
    diagnostics.expansions_opened += outcome.opened;
    diagnostics.expansions_refused += outcome.refused;
    diagnostics.expansions_failed += outcome.failed;
    diagnostics.expansions_unnecessary += outcome.unnecessary;
    for (const [state, count] of Object.entries(outcome.availability)) {
      const key = state as keyof typeof diagnostics.availability;
      if (key in diagnostics.availability) diagnostics.availability[key] += count;
    }
    expansionsLeft -= outcome.opened;
    if (outcome.opened > 0) await settleOnce();
    return outcome.navigated;
  };

  // One item from the first sample, kept only to answer a question no
  // selector can: after moving on, is it still in the DOM? If it is, the page
  // mounts everything and there is nothing to harvest incrementally, so the
  // traversal can go straight to the end instead of walking there. If it is
  // gone, the page is virtualizing and every step matters. Measured rather
  // than configured, so a source that starts or stops virtualizing is
  // followed rather than assumed.
  let witness: Element | null = null;
  let mountsEverything = false;

  try {
    diagnostics.performed = true;

    await settleOnce();

    // Rewind: seek the boundary, and seek it again while it keeps moving.
    // A single seek is not enough where reaching the top triggers a fetch of
    // older history — the boundary itself moves, and landing once puts the
    // read in the middle of the thread rather than at its start.
    if (plan.rewind && surface.maxScroll() > 0) {
      let settledAtStart = false;
      for (let rewind = 0; rewind < plan.budget.maxRewinds; rewind += 1) {
        if (outOfTime() || interruption.interrupted()) break;
        const before = surface.scrollHeight();
        surface.scrollTo(0);
        await settleOnce();
        if (surface.scrollTop() <= 0 && surface.scrollHeight() === before) {
          settledAtStart = true;
          break;
        }
      }

      // The boundary was still moving when the attempts ran out, so the read
      // began somewhere in the middle. This has to be recorded, because
      // nothing later can detect it: walking down from turn 120 to turn 300
      // yields a contiguous run of ordinals and terminates `reached_end`, and
      // a capture missing the first 119 turns would otherwise be entitled to
      // call itself complete.
      if (!settledAtStart) {
        diagnostics.inaccessible.push(
          'The beginning of this page could not be reached — it kept loading earlier content — so the capture starts part-way through.',
        );
      }
    }

    lowest = Math.min(lowest, surface.scrollTop());
    highest = Math.max(highest, surface.scrollTop());

    if (plan.expand) {
      if (await expandStep()) diagnostics.termination = 'navigation_detected';
    }
    await takeSample();
    witness = mountedItems(contentRoot, plan)[0] ?? null;

    if (diagnostics.termination === 'navigation_detected') {
      return diagnostics;
    }

    if (surface.maxScroll() <= 0) {
      diagnostics.termination = 'reached_end';
      return diagnostics;
    }

    let step = 0;
    for (; step < plan.budget.maxSteps; step += 1) {
      if (interruption.interrupted()) {
        diagnostics.termination = 'user_interrupted';
        break;
      }
      if (outOfTime()) {
        diagnostics.termination = 'time_budget';
        break;
      }
      // Idle is only a reason to stop once there is nothing left to scroll
      // through. Treating it as one earlier was a bug with a clear symptom: a
      // page with two tall empty spacers before its lazily-loaded sections
      // yielded nothing for a few steps, the engine concluded the page was
      // done, and it stopped 900px short of the content it came for. What
      // bounds a page that genuinely never yields anything is the step and
      // time budgets, not a guess made halfway down.
      if (idle >= plan.budget.maxIdleSteps && surface.scrollTop() >= surface.maxScroll()) {
        diagnostics.termination = 'reached_end';
        break;
      }

      const before = surface.scrollTop();
      const target = mountsEverything
        ? surface.maxScroll()
        : nextScrollTop(surface, mountedItems(contentRoot, plan), plan);
      const atEnd = target >= surface.maxScroll();
      surface.scrollTo(atEnd ? surface.maxScroll() : target);

      if (surface.scrollTop() <= before && !atEnd) {
        // The page refused to move. Not an error, and not a loop: stop.
        diagnostics.termination = 'no_progress';
        break;
      }

      lowest = Math.min(lowest, surface.scrollTop());
      highest = Math.max(highest, surface.scrollTop());

      await settleOnce();

      if (witness) {
        // `isConnected` is the whole test: a virtualized list detaches items
        // as they leave its window, and nothing else about the page says so
        // as directly.
        mountsEverything = witness.isConnected !== false;
        diagnostics.virtualized = !mountsEverything;
        witness = null;
      }

      if (plan.expand && expansionsLeft > 0) {
        if (await expandStep()) {
          diagnostics.termination = 'navigation_detected';
          break;
        }
      }
      await takeSample();

      if (atEnd && surface.scrollTop() >= surface.maxScroll()) {
        // Reaching the bottom is not the same as reaching the end. On a page
        // that appends content as you approach it, arriving at the bottom is
        // what *causes* the next section to load — so the bottom moves, and
        // treating the first arrival as the end stops one screen short of
        // everything that was about to appear.
        //
        // The end is where the page stops growing and stops yielding.
        const reachBefore = surface.maxScroll();
        await settleOnce();
        const kept = await takeSample();
        if (surface.maxScroll() <= reachBefore && kept === 0) {
          diagnostics.termination = 'reached_end';
          step += 1;
          break;
        }
      }
    }

    if (diagnostics.termination === 'not_needed') {
      diagnostics.termination = step >= plan.budget.maxSteps ? 'step_budget' : 'reached_end';
    }
    if (expansionsLeft <= 0 && diagnostics.termination === 'reached_end') {
      diagnostics.termination = 'expansion_budget';
    }
    diagnostics.steps = step;
  } catch {
    diagnostics.termination = 'error';
  } finally {
    interruption.stop();
    diagnostics.scroll_span_px = Math.max(0, highest - lowest);
    diagnostics.duration_ms = Math.round(deps.now() - started);

    try {
      surface.scrollTo(originalSurfaceScroll);
      doc.defaultView?.scrollTo?.(0, originalWindowScroll);
      diagnostics.scroll_restored =
        Math.abs(surface.scrollTop() - originalSurfaceScroll) <= 2;
    } catch {
      diagnostics.scroll_restored = false;
    }
  }

  return diagnostics;
}
