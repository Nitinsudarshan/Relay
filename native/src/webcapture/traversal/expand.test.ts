/**
 * Tests for the expansion classifier.
 *
 * This is the file that has to be paranoid. Everything the reveal engine does
 * to a page other than scrolling it goes through `classifyExpansion`, and a
 * webpage is untrusted input: the labels, the roles and the markup are all
 * written by whoever wrote the page.
 *
 * Two properties are asserted throughout:
 *
 * - **Necessity.** A control whose content is already in the DOM is reported
 *   `unnecessary` and not clicked. This is the case the real Claude capture
 *   turned out to be: a clamped message whose full text was there all along.
 * - **Safety.** Nothing that acts rather than reveals is ever activated, no
 *   matter how it is labelled, and no allow-list entry can rescue it.
 */

import { describe, expect, it } from 'vitest';
import { accessibleName, classifyExpansion, findCandidates } from './expand';
import { GENERIC_TRAVERSAL } from './plans';
import type { TraversalPlan } from './types';

const plan: TraversalPlan = { ...GENERIC_TRAVERSAL, contentSelectors: ['#content'] };

function judge(bodyHtml: string, selector = 'button') {
  const doc = new DOMParser().parseFromString(
    `<!doctype html><html><body><main><div id="content">${bodyHtml}</div></main></body></html>`,
    'text/html',
  );
  const el = doc.querySelector(selector)!;
  const root = doc.getElementById('content')!;
  return { verdict: classifyExpansion(el, plan, root, new WeakSet()), doc, el, root };
}

/**
 * A container that reports more content than it shows.
 *
 * jsdom reports every box as zero-sized, so clipping has to be simulated. The
 * numbers are the ones Chromium produced for a `-webkit-line-clamp: 3` box:
 * `scrollHeight` 777 against `clientHeight` 63.
 */
function clipped(el: Element): Element {
  Object.defineProperty(el, 'scrollHeight', { value: 777, configurable: true });
  Object.defineProperty(el, 'clientHeight', { value: 63, configurable: true });
  return el;
}

describe('accessibleName', () => {
  it('prefers an explicit label over the visible text', () => {
    const { el } = judge('<button aria-label="Show more">···</button>');
    expect(accessibleName(el)).toBe('show more');
  });

  it('falls back to the text, then the title', () => {
    expect(accessibleName(judge('<button>Read More</button>').el)).toBe('read more');
    expect(accessibleName(judge('<button title="Expand"></button>').el)).toBe('expand');
  });
});

describe('what the classifier activates', () => {
  const disclosureLabels = [
    'Show more',
    'Read more',
    'See more',
    'View more',
    'Show all',
    'Expand',
    'Expand all',
    'Continue reading',
    'Show full response',
    'Show full message',
    'Show thinking',
    'Load more',
    'Show 12 more comments',
  ];

  for (const label of disclosureLabels) {
    it(`activates “${label}” when the content is genuinely absent`, () => {
      const { verdict } = judge(
        `<div id="t">short</div><button aria-expanded="false" aria-controls="t">${label}</button>`,
      );
      expect(verdict.decision).toBe('activate');
    });
  }

  it('activates a closed <details> whose body has not been rendered yet', () => {
    // The case that keeps the `<summary>` branch honest: a component that
    // renders its children only on open really does need opening.
    const { verdict } = judge('<details><summary>More</summary></details>', 'summary');
    expect(verdict).toMatchObject({ decision: 'activate', availability: 'collapsed' });
  });

  it('activates on aria-expanded plus aria-controls even with an opaque label', () => {
    // The strongest signal available, and the one that does not depend on
    // reading English: the page has declared this a disclosure widget.
    const { verdict } = judge('<div id="t">s</div><button aria-expanded="false" aria-controls="t">···</button>');
    expect(verdict.decision).toBe('activate');
  });

  it('activates a source-verified selector from the plan', () => {
    const doc = new DOMParser().parseFromString(
      '<!doctype html><html><body><div id="content"><div id="t">s</div><button class="ajax-pagination-btn">Older</button></div></body></html>',
      'text/html',
    );
    const verdict = classifyExpansion(
      doc.querySelector('button')!,
      { ...plan, expandSelectors: ['button.ajax-pagination-btn'] },
      doc.getElementById('content')!,
      new WeakSet(),
    );
    expect(verdict.decision).toBe('activate');
  });
});

describe('what the classifier refuses to activate', () => {
  const actionLabels = [
    'Send',
    'Submit',
    'Delete',
    'Delete conversation',
    'Remove',
    'Approve',
    'Authorize',
    'Purchase',
    'Buy now',
    'Install',
    'Execute',
    'Run',
    'Regenerate',
    'Retry',
    'Share',
    'Download',
    'Export',
    'Copy',
    'Save',
    'More actions',
    'Show more actions',
    'Settings',
    'Sign out',
    'New chat',
    'Edit',
    'Merge pull request',
    'Request changes',
    'Upgrade plan',
  ];

  for (const label of actionLabels) {
    it(`refuses “${label}” even with disclosure markup on it`, () => {
      // The deny-list wins over every positive signal, including the two
      // strongest ones. "Show more actions" is the case this ordering exists
      // for: it is not "Show more".
      const { verdict } = judge(
        `<div id="t">s</div><button aria-expanded="false" aria-controls="t">${label}</button>`,
      );
      expect(verdict.decision).toBe('refuse');
    });
  }

  it('refuses an unlabelled button with no disclosure evidence', () => {
    const { verdict } = judge('<button>···</button>');
    expect(verdict).toMatchObject({ decision: 'refuse', reason: 'no disclosure evidence' });
  });

  it('refuses anything inside a form', () => {
    const { verdict } = judge(
      '<form><div id="t">s</div><button aria-expanded="false" aria-controls="t">Show more</button></form>',
    );
    expect(verdict).toMatchObject({ decision: 'refuse' });
  });

  it('refuses a submit or reset control', () => {
    expect(judge('<button type="submit" aria-expanded="false">Show more</button>').verdict.decision).toBe('refuse');
    expect(judge('<button type="reset" aria-expanded="false">Show more</button>').verdict.decision).toBe('refuse');
  });

  it('refuses a menu or dialog opener', () => {
    const { verdict } = judge('<button aria-haspopup="menu" aria-expanded="false">Show more</button>');
    expect(verdict).toMatchObject({ decision: 'refuse', reason: 'opens a menu or dialog' });
  });

  it('refuses a link that would navigate, and allows one that would not', () => {
    expect(
      judge('<a role="button" href="/next" aria-expanded="false">Show more</a>', 'a').verdict.decision,
    ).toBe('refuse');
    expect(
      judge('<div id="t">s</div><a role="button" href="#" aria-expanded="false" aria-controls="t">Show more</a>', 'a')
        .verdict.decision,
    ).toBe('activate');
  });

  it('refuses a disabled control', () => {
    expect(judge('<button disabled aria-expanded="false">Show more</button>').verdict.decision).toBe('refuse');
    expect(judge('<button aria-disabled="true" aria-expanded="false">Show more</button>').verdict.decision).toBe(
      'refuse',
    );
  });

  it('refuses anything in page chrome, whatever it is labelled', () => {
    const doc = new DOMParser().parseFromString(
      `<!doctype html><html><body><div id="content">
         <nav><button aria-expanded="false">Show more</button></nav>
         <div role="toolbar"><button aria-expanded="false">Show all</button></div>
         <div contenteditable="true"><button aria-expanded="false">Show more</button></div>
       </div></body></html>`,
      'text/html',
    );
    const root = doc.getElementById('content')!;
    for (const button of Array.from(doc.querySelectorAll('button'))) {
      expect(classifyExpansion(button, plan, root, new WeakSet()).decision).toBe('refuse');
    }
  });

  it('refuses anything outside the content region', () => {
    const doc = new DOMParser().parseFromString(
      '<!doctype html><html><body><button aria-expanded="false">Show more</button><div id="content"></div></body></html>',
      'text/html',
    );
    const verdict = classifyExpansion(
      doc.querySelector('button')!,
      plan,
      doc.getElementById('content')!,
      new WeakSet(),
    );
    expect(verdict).toMatchObject({ decision: 'refuse', reason: 'outside the content region' });
  });

  it('refuses a region the source plan forbids', () => {
    const doc = new DOMParser().parseFromString(
      '<!doctype html><html><body><div id="content"><div class="composer"><button aria-expanded="false">Show more</button></div></div></body></html>',
      'text/html',
    );
    const verdict = classifyExpansion(
      doc.querySelector('button')!,
      { ...plan, forbiddenSelectors: ['.composer'] },
      doc.getElementById('content')!,
      new WeakSet(),
    );
    expect(verdict.decision).toBe('refuse');
  });

  it('refuses a control that is already expanded', () => {
    const { verdict } = judge('<button aria-expanded="true">Show more</button>');
    expect(verdict).toMatchObject({ decision: 'refuse', reason: 'already expanded' });
  });

  it('refuses a control it has already activated once', () => {
    const { el, root } = judge('<div id="t">s</div><button aria-expanded="false" aria-controls="t">Show more</button>');
    const activated = new WeakSet<Element>([el]);
    expect(classifyExpansion(el, plan, root, activated).decision).toBe('refuse');
  });
});

describe('necessity — the least-invasive rule', () => {
  it('does not click a control whose content is only clipped by CSS', () => {
    // The real Claude case. The message's full text is in the DOM; the UI has
    // shortened its presentation. Clicking would change someone's page to
    // obtain text Relay already has.
    const { doc, el, root } = judge(
      '<div id="t">the whole message, all of it</div><button aria-expanded="false" aria-controls="t">Show more</button>',
    );
    clipped(doc.getElementById('t')!);

    const verdict = classifyExpansion(el, plan, root, new WeakSet());
    expect(verdict.decision).toBe('unnecessary');
    expect(verdict).toMatchObject({ availability: 'visually_truncated' });
  });

  it('treats a closed <details> with a rendered body as already present', () => {
    // Verified in Chromium: `textContent` returns a closed details' body,
    // `innerText` does not. Relay's block walk reads `textContent`, so the
    // content is captured without opening anything.
    const { verdict } = judge('<details><summary>More</summary><p>body text</p></details>', 'summary');
    expect(verdict).toMatchObject({ decision: 'unnecessary', availability: 'visually_truncated' });
  });

  it('does click when the content really is missing', () => {
    const { verdict } = judge(
      '<div id="t">first line…</div><button aria-expanded="false" aria-controls="t">Show more</button>',
    );
    expect(verdict).toMatchObject({ decision: 'activate', availability: 'collapsed' });
  });
});

describe('findCandidates', () => {
  it('looks only inside the content region', () => {
    const doc = new DOMParser().parseFromString(
      `<!doctype html><html><body>
         <nav><button>Menu</button></nav>
         <div id="content"><button>Show more</button><summary>More</summary><span role="button">Expand</span></div>
       </body></html>`,
      'text/html',
    );
    const candidates = findCandidates(doc.getElementById('content')!, plan);
    expect(candidates.map((c) => c.label)).toEqual(['show more', 'more', 'expand']);
  });

  it('survives a selector the browser will not accept', () => {
    const doc = new DOMParser().parseFromString(
      '<!doctype html><html><body><div id="content"><button>Show more</button></div></body></html>',
      'text/html',
    );
    const candidates = findCandidates(doc.getElementById('content')!, {
      ...plan,
      expandSelectors: ['::::not-a-selector'],
    });
    expect(candidates).toEqual([]);
  });
});
