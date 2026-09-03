/**
 * Tests for the reveal engine.
 *
 * jsdom has no layout, so every box measures zero and the real
 * surface-resolution path would always take the "nothing scrolls" branch. The
 * engine therefore accepts an injected surface (`stubSurface`), which is what
 * lets the loop — stepping, termination, virtualization detection, scroll
 * restoration — be tested at all. The geometry these tests simulate is taken
 * from what Chromium actually did: see `scripts/capture-validation/`.
 */

import { beforeEach, describe, expect, it, vi } from 'vitest';
import { budget } from './budget';
import { expandHere, nextScrollTop, traverse } from './engine';
import { GENERIC_TRAVERSAL } from './plans';
import { stubSurface } from './surface';
import type { ScrollSurface, TraversalPlan } from './types';

/** A plan with tiny budgets, so a test never waits on a real settle window. */
function plan(overrides: Partial<TraversalPlan> = {}): TraversalPlan {
  return {
    ...GENERIC_TRAVERSAL,
    id: 'generic',
    itemSelectors: ['.item'],
    contentSelectors: ['#content'],
    budget: budget({ settlePollMs: 1, settleQuietTicks: 1, settleMaxMs: 20 }),
    ...overrides,
  };
}

const deps = {
  now: () => Date.now(),
  wait: () => Promise.resolve(),
};

function docWith(body: string): Document {
  return new DOMParser().parseFromString(
    `<!doctype html><html><body><div id="content">${body}</div></body></html>`,
    'text/html',
  );
}

/**
 * A list that unmounts what scrolls past it, the way ChatGPT does.
 *
 * The behaviour this simulates is the reason the engine harvests during
 * traversal rather than after it: an item that has left the window is not in
 * the DOM to be read later.
 */
function virtualizedDoc(total: number, windowSize: number, itemHeight: number) {
  const doc = docWith('');
  const content = doc.getElementById('content')!;
  const surface = stubSurface({ scrollHeight: total * itemHeight, viewportHeight: 400 });
  const offsets = new WeakMap<Element, number>();

  const render = () => {
    const first = Math.max(0, Math.floor(surface.scrollTop() / itemHeight));
    const last = Math.min(total - 1, first + windowSize - 1);
    content.textContent = '';
    for (let i = first; i <= last; i += 1) {
      const el = doc.createElement('div');
      el.className = 'item';
      el.dataset.index = String(i);
      el.textContent = `ITEM_${i}`;
      Object.defineProperty(el, 'offsetHeight', { value: itemHeight, configurable: true });
      offsets.set(el, i * itemHeight);
      content.appendChild(el);
    }
  };

  const scrollTo = surface.scrollTo;
  const wired: ScrollSurface = {
    ...surface,
    scrollTo: (top: number) => {
      scrollTo(top);
      render();
    },
    offsetOf: (el) => offsets.get(el) ?? 0,
  };
  render();
  return { doc, surface: wired };
}

describe('nextScrollTop', () => {
  it('steps by the mounted extent rather than by one viewport', () => {
    // The performance decision that makes a long thread tractable: with eight
    // 200px items mounted, the next step is ~1,400px, not one 400px screen.
    const doc = docWith(
      Array.from({ length: 8 }, (_, i) => `<div class="item">${i}</div>`).join(''),
    );
    const offsets = new WeakMap<Element, number>();
    const items = Array.from(doc.querySelectorAll('.item'));
    items.forEach((el, i) => {
      Object.defineProperty(el, 'offsetHeight', { value: 200 });
      offsets.set(el, i * 200);
    });
    const surface = { ...stubSurface({ scrollHeight: 10_000, viewportHeight: 400 }), offsetOf: (el: Element) => offsets.get(el) ?? 0 };

    expect(nextScrollTop(surface, items, plan())).toBe(1400);
  });

  it('keeps overlap so a step cannot unmount what it has not read', () => {
    const doc = docWith('<div class="item">only</div>');
    const item = doc.querySelector('.item')!;
    Object.defineProperty(item, 'offsetHeight', { value: 900 });
    const offsets = new WeakMap<Element, number>([[item, 0]]);
    const surface = { ...stubSurface({ scrollHeight: 10_000, viewportHeight: 400 }), offsetOf: (el: Element) => offsets.get(el) ?? 0 };

    // The item is taller than the viewport, so the naive target would be its
    // bottom edge; the overlap pulls it back by a whole item.
    expect(nextScrollTop(surface, [item], plan())).toBeLessThan(900);
    expect(nextScrollTop(surface, [item], plan())).toBeGreaterThan(0);
  });

  it('falls back to a fraction of the viewport when nothing is measurable', () => {
    const surface = stubSurface({ scrollHeight: 10_000, viewportHeight: 400 });
    expect(nextScrollTop(surface, [], plan())).toBe(300);
  });

  it('steps by a viewport when the mounted items are behind the reader', () => {
    // The treadmill this prevents: a page whose only measurable items sit
    // above the current position puts the computed target behind it. Nudging
    // forward by a pixel to guarantee progress burns a whole budget to advance
    // a hundred pixels — measured at 113 steps for 113px.
    const doc = docWith('<div class="item">above</div>');
    const item = doc.querySelector('.item')!;
    Object.defineProperty(item, 'offsetHeight', { value: 200 });
    const offsets = new WeakMap<Element, number>([[item, 100]]);
    const surface = {
      ...stubSurface({ scrollHeight: 10_000, viewportHeight: 400, scrollTop: 2_000 }),
      offsetOf: (el: Element) => offsets.get(el) ?? 0,
    };

    expect(nextScrollTop(surface, [item], plan())).toBe(2_300);
  });
});

describe('traverse', () => {
  it('reads every item of a virtualized list, in order, with no duplicates', async () => {
    const { doc, surface } = virtualizedDoc(500, 8, 200);
    const seen: number[] = [];

    const diagnostics = await traverse(doc, {
      plan: plan(),
      deps: { ...deps, surface },
      sample: (document) => {
        let added = 0;
        for (const el of Array.from(document.querySelectorAll('.item'))) {
          const index = Number((el as HTMLElement).dataset.index);
          if (!seen.includes(index)) {
            seen.push(index);
            added += 1;
          }
        }
        return added;
      },
    });

    expect(diagnostics.termination).toBe('reached_end');
    expect(seen).toHaveLength(500);
    expect(seen).toEqual([...seen].sort((a, b) => a - b));
    expect(diagnostics.virtualized).toBe(true);
  });

  it('goes straight to the end on a page that mounts everything', async () => {
    // Claude's shape. The engine measures this rather than being told: the
    // first sample's items are still attached after one step, so there is
    // nothing to harvest incrementally and walking would be waste.
    const doc = docWith(Array.from({ length: 60 }, (_, i) => `<div class="item">${i}</div>`).join(''));
    const surface = stubSurface({ scrollHeight: 12_000, viewportHeight: 400 });

    const diagnostics = await traverse(doc, {
      plan: plan(),
      deps: { ...deps, surface },
      sample: () => 0,
    });

    expect(diagnostics.virtualized).toBe(false);
    expect(diagnostics.steps).toBeLessThanOrEqual(3);
    expect(diagnostics.termination).toBe('reached_end');
  });

  it('restores the scroll position it started from', async () => {
    const { doc, surface } = virtualizedDoc(80, 8, 200);
    surface.scrollTo(4_000);

    const diagnostics = await traverse(doc, {
      plan: plan(),
      deps: { ...deps, surface },
      sample: () => 1,
    });

    expect(surface.scrollTop()).toBe(4_000);
    expect(diagnostics.scroll_restored).toBe(true);
    expect(diagnostics.scroll_span_px).toBeGreaterThan(0);
  });

  it('stops on a page that refuses to scroll instead of looping', async () => {
    const doc = docWith('<div class="item">stuck</div>');
    const frozen: ScrollSurface = {
      element: null,
      kind: 'stub',
      scrollTop: () => 0,
      scrollTo: () => {},
      viewportHeight: () => 400,
      scrollHeight: () => 40_000,
      maxScroll: () => 39_600,
      offsetOf: () => 0,
    };

    const diagnostics = await traverse(doc, {
      plan: plan(),
      deps: { ...deps, surface: frozen },
      sample: () => 0,
    });

    expect(diagnostics.termination).toBe('no_progress');
    expect(diagnostics.steps).toBeLessThan(5);
  });

  it('stops at its time budget and says so', async () => {
    const { doc, surface } = virtualizedDoc(5_000, 4, 200);
    let clock = 0;
    const diagnostics = await traverse(doc, {
      plan: plan({ budget: budget({ settlePollMs: 1, settleQuietTicks: 1, settleMaxMs: 5, maxMs: 50 }) }),
      deps: { now: () => (clock += 10), wait: () => Promise.resolve(), surface },
      sample: () => 1,
    });

    expect(diagnostics.termination).toBe('time_budget');
  });

  it('stops at its step budget rather than reading forever', async () => {
    const { doc, surface } = virtualizedDoc(100_000, 4, 200);
    const diagnostics = await traverse(doc, {
      plan: plan({ budget: budget({ settlePollMs: 1, settleQuietTicks: 1, settleMaxMs: 5, maxSteps: 6 }) }),
      deps: { ...deps, surface },
      sample: () => 1,
    });

    expect(diagnostics.termination).toBe('step_budget');
    expect(diagnostics.steps).toBe(6);
  });

  it('stops when the user touches their own page', async () => {
    const { doc, surface } = virtualizedDoc(500, 8, 200);
    let samples = 0;

    const diagnostics = await traverse(doc, {
      plan: plan(),
      deps: { ...deps, surface },
      sample: () => {
        samples += 1;
        if (samples === 2) {
          doc.dispatchEvent(new Event('wheel'));
        }
        return 1;
      },
    });

    expect(diagnostics.termination).toBe('user_interrupted');
    expect(surface.scrollTop()).toBe(0);
  });

  it('reports an error rather than losing the capture', async () => {
    const doc = docWith('<div class="item">x</div>');
    const exploding: ScrollSurface = {
      element: null,
      kind: 'stub',
      scrollTop: () => 0,
      scrollTo: () => {
        throw new Error('the page replaced its scroller');
      },
      viewportHeight: () => 400,
      scrollHeight: () => 40_000,
      maxScroll: () => 39_600,
      offsetOf: () => 0,
    };

    const diagnostics = await traverse(doc, {
      plan: plan({ rewind: true }),
      deps: { ...deps, surface: exploding },
      sample: () => 1,
    });

    expect(diagnostics.termination).toBe('error');
    expect(diagnostics.performed).toBe(true);
  });

  it('re-seeks a boundary that keeps moving, up to its limit', async () => {
    // ChatGPT's shape: reaching the top fetches older history, so the top
    // itself moves and one seek lands mid-thread.
    const doc = docWith('<div class="item">x</div>');
    let height = 4_000;
    let top = 3_000;
    let seeks = 0;
    const surface: ScrollSurface = {
      element: null,
      kind: 'stub',
      scrollTop: () => top,
      scrollTo: (next) => {
        top = next;
        if (next === 0 && seeks < 3) {
          // Older history arrived above: the page grows and pushes us down.
          seeks += 1;
          height += 2_000;
          top = 500;
        }
      },
      viewportHeight: () => 400,
      scrollHeight: () => height,
      maxScroll: () => height - 400,
      offsetOf: () => 0,
    };

    await traverse(doc, {
      plan: plan({ rewind: true }),
      deps: { ...deps, surface },
      sample: () => 1,
    });

    expect(seeks).toBe(3);
    expect(height).toBe(10_000);
  });

  it('records that it never reached the start when the boundary kept moving', () => {
    // Nothing downstream can detect this on its own: walking down from
    // part-way through yields a contiguous run of turns and terminates
    // `reached_end`, so a capture missing the first hundred would otherwise be
    // entitled to call itself complete.
    const doc = docWith('<div class="item">x</div>');
    let height = 4_000;
    let top = 0;
    const surface: ScrollSurface = {
      element: null,
      kind: 'stub',
      scrollTop: () => top,
      scrollTo: (next) => {
        top = next;
        // Every arrival at the top loads more history, for ever.
        if (next === 0) {
          height += 2_000;
          top = 500;
        }
      },
      viewportHeight: () => 400,
      scrollHeight: () => height,
      maxScroll: () => height - 400,
      offsetOf: () => 0,
    };

    return traverse(doc, {
      plan: plan({ rewind: true, budget: budget({ settlePollMs: 1, settleQuietTicks: 1, settleMaxMs: 5, maxSteps: 4 }) }),
      deps: { ...deps, surface },
      sample: () => 1,
    }).then((diagnostics) => {
      expect(diagnostics.inaccessible.join(' ')).toMatch(/beginning of this page could not be reached/i);
    });
  });
});

describe('expandHere', () => {
  let activated: WeakSet<Element>;

  beforeEach(() => {
    activated = new WeakSet();
  });

  it('opens a control whose content is genuinely absent', async () => {
    const doc = docWith(
      '<div id="t">short</div><button aria-expanded="false" aria-controls="t">Show more</button>',
    );
    const target = doc.getElementById('t')!;
    const button = doc.querySelector('button')!;
    button.addEventListener('click', () => {
      target.textContent = 'the whole thing';
      button.setAttribute('aria-expanded', 'true');
    });

    const result = await expandHere(doc, doc.getElementById('content')!, plan(), activated, {
      remaining: 10,
      perStep: 10,
    });

    expect(result.opened).toBe(1);
    expect(result.availability.collapsed).toBe(1);
    expect(target.textContent).toBe('the whole thing');
  });

  it('never clicks the same control twice', async () => {
    const doc = docWith('<div id="t">short</div><button aria-expanded="false" aria-controls="t">Show more</button>');
    const clicks = vi.fn();
    doc.querySelector('button')!.addEventListener('click', clicks);

    const args = [doc, doc.getElementById('content')!, plan(), activated, { remaining: 10, perStep: 10 }] as const;
    await expandHere(...args);
    await expandHere(...args);

    expect(clicks).toHaveBeenCalledTimes(1);
  });

  it('counts a control that was clicked and did nothing', async () => {
    const doc = docWith('<div id="t">short</div><button aria-expanded="false" aria-controls="t">Show more</button>');
    const result = await expandHere(doc, doc.getElementById('content')!, plan(), activated, {
      remaining: 10,
      perStep: 10,
    });

    // The early warning that a site redesigned: counted, never retried.
    expect(result.failed).toBe(1);
    expect(result.opened).toBe(0);
  });

  it('honours the per-step cap', async () => {
    const buttons = Array.from(
      { length: 8 },
      (_, i) => `<div id="t${i}">s</div><button aria-expanded="false" aria-controls="t${i}">Show more</button>`,
    ).join('');
    const doc = docWith(buttons);
    for (const button of Array.from(doc.querySelectorAll('button'))) {
      button.addEventListener('click', () => button.setAttribute('aria-expanded', 'true'));
    }

    const result = await expandHere(doc, doc.getElementById('content')!, plan(), activated, {
      remaining: 100,
      perStep: 3,
    });

    expect(result.opened).toBe(3);
  });

  it('cancels a form submission the page attempts while expanding', async () => {
    // The runtime guard behind the classifier: even if a click gets through to
    // something that submits, the submission does not happen.
    const doc = docWith(
      '<div id="t">s</div><button aria-expanded="false" aria-controls="t">Show more</button>',
    );
    const form = doc.createElement('form');
    doc.getElementById('content')!.appendChild(form);
    const submitted = vi.fn();
    form.addEventListener('submit', submitted);

    doc.querySelector('button')!.addEventListener('click', () => {
      form.dispatchEvent(new Event('submit', { bubbles: true, cancelable: true }));
    });

    await expandHere(doc, doc.getElementById('content')!, plan(), activated, {
      remaining: 10,
      perStep: 10,
    });

    expect(submitted).not.toHaveBeenCalled();
  });

  it('does nothing at all on a plan with expansion switched off', async () => {
    const doc = docWith('<div id="t">s</div><button aria-expanded="false" aria-controls="t">Show more</button>');
    const clicks = vi.fn();
    doc.querySelector('button')!.addEventListener('click', clicks);

    const result = await expandHere(doc, doc.getElementById('content')!, plan({ expand: false }), activated, {
      remaining: 10,
      perStep: 10,
    });

    expect(result.found).toBe(0);
    expect(clicks).not.toHaveBeenCalled();
  });
});
