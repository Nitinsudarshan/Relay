/**
 * Expansion: deciding what may be activated, and proving it was necessary.
 *
 * Two rules govern this file, and they are the difference between a reader and
 * a browser agent.
 *
 * **Necessity.** A control is activated only when the content it governs is
 * genuinely absent from the DOM. Content that is merely clipped by CSS is
 * already captured — verified in Chromium, where a `-webkit-line-clamp: 3` box
 * holding 2,904 characters returned all 2,904 from both `textContent` and
 * `innerText` — so clicking its "Show more" buys nothing and costs a side
 * effect on someone else's page. `INSPECT FIRST, INTERACT ONLY WHEN NECESSARY`
 * is enforced here, not documented here.
 *
 * **Safety.** The classifier is a deny-first funnel, not a text match. A
 * candidate has to survive a structural test, then a label deny-list, and only
 * then does positive evidence count. "Show more actions" is not "Show more",
 * and no allow-list entry can rescue it.
 */

import { classifyAvailability, disclosureTarget } from '../dom';
import type { ContentAvailability } from '../types';
import type { TraversalPlan } from './types';

/**
 * Controls whose purpose is to reveal content that is already written.
 *
 * Anchored so a label has to *be* one of these rather than contain one:
 * `showMoreActionsMenu` must not match `show more`.
 */
const DISCLOSURE_LABELS: RegExp[] = [
  /^show\s+(more|all|full|original|details|thinking|reasoning|work|earlier|previous|hidden)\b/,
  /^show\s+\d+\s+more\b/,
  /^show\s+\d+\s+(hidden|collapsed|previous|earlier)\b/,
  /^(read|see|view)\s+(more|all|full)\b/,
  /^expand(\s+(all|thread|comment|section|code|file))?$/,
  /^continue\s+reading$/,
  /^load\s+(more|earlier|previous)\b/,
  /^more\s+(details|context)$/,
  /^\d+\s+more\s+(comments?|replies|messages?|items?|lines?)$/,
  /^view\s+entire\s+(message|response|conversation)$/,
];

/**
 * Controls that act rather than reveal.
 *
 * A deny match is final. The list is long on purpose: the cost of denying a
 * genuine disclosure control is a slightly less complete capture, and the cost
 * of allowing one action is unbounded.
 */
const ACTION_LABELS: RegExp[] = [
  /\b(delete|remove|discard|clear|erase|trash|destroy)\b/,
  /\b(regenerate|retry|rerun|re-?run|resend|refresh)\b/,
  /\b(submit|send|post|reply|comment|publish|commit|push)\b/,
  /\b(share|export|download|save|print|copy|duplicate)\b/,
  /\b(approve|authorize|authorise|confirm|accept|reject|decline|deny)\b/,
  /\b(purchase|buy|checkout|pay|order|subscribe|upgrade|renew|billing)\b/,
  /\b(install|uninstall|deploy|execute|run|build|activate|enable|disable)\b/,
  /\b(settings?|preferences?|configure|options|admin|account|profile|billing)\b/,
  /\b(sign\s?(in|out|up)|log\s?(in|out)|logout|login)\b/,
  /\b(edit|rename|move|merge|revert|restore|archive|close|reopen|resolve)\b/,
  /\b(report|flag|block|mute|unfollow|follow|like|vote|upvote|downvote)\b/,
  /\b(new\s+(chat|conversation|thread|file|project)|start\s+over)\b/,
  /\bmore\s+(actions?|options?|settings?)\b/,
  /\b(request\s+changes|change\s+(password|email|plan|settings?))\b/,
  /\b(upload|attach|choose\s+file|browse)\b/,
];

/** Regions that are page chrome or user input, never content. */
const CHROME_SELECTOR = [
  'nav',
  'header',
  'footer',
  'form',
  '[role="navigation"]',
  '[role="banner"]',
  '[role="toolbar"]',
  '[role="menu"]',
  '[role="menubar"]',
  '[role="menuitem"]',
  '[role="dialog"]',
  '[role="alertdialog"]',
  '[role="tablist"]',
  '[role="search"]',
  '[contenteditable="true"]',
  '[aria-hidden="true"]',
].join(',');

/** What a candidate is, once the classifier has looked at it. */
export type ExpansionVerdict =
  | { decision: 'activate'; availability: ContentAvailability }
  | { decision: 'unnecessary'; availability: ContentAvailability; reason: string }
  | { decision: 'refuse'; reason: string };

export interface ExpansionCandidate {
  el: Element;
  label: string;
}

/**
 * The accessible name, as close as a content script can get to it cheaply.
 *
 * `aria-label` wins because a page that sets one means it; the visible text is
 * next; `title` last. Nothing here walks `aria-labelledby` chains — the extra
 * accuracy is not worth the traversal on every button of every step, and a
 * label Relay cannot read is simply a control it will not activate.
 */
export function accessibleName(el: Element): string {
  const aria = el.getAttribute('aria-label') ?? '';
  const text = el.textContent ?? '';
  const title = el.getAttribute('title') ?? '';
  return (aria || text || title)
    .replace(/\s+/g, ' ')
    .trim()
    .toLowerCase()
    .slice(0, 120);
}

function isVisible(el: Element): boolean {
  const box = el as HTMLElement;
  if (box.hasAttribute('hidden') || el.getAttribute('aria-hidden') === 'true') return false;
  if (typeof box.getBoundingClientRect === 'function') {
    try {
      const rect = box.getBoundingClientRect();
      // A zero-sized box in a real browser is not clickable. In a layout-free
      // document every box is zero-sized, so absence of geometry is not taken
      // as absence of the element.
      const hasGeometry = rect.width !== 0 || rect.height !== 0 || rect.top !== 0;
      if (hasGeometry && rect.width <= 0 && rect.height <= 0) return false;
    } catch {
      // Fall through: an element that will not measure is judged on attributes.
    }
  }
  try {
    const view = el.ownerDocument?.defaultView;
    const style = view?.getComputedStyle?.(el);
    if (style && (style.display === 'none' || style.visibility === 'hidden')) return false;
  } catch {
    // No computed styles: attribute checks above stand.
  }
  return true;
}

/** Would activating this anchor take the user somewhere? */
function navigates(el: Element): boolean {
  if (el.tagName.toLowerCase() !== 'a') return false;
  const href = el.getAttribute('href');
  if (href === null) return false;
  const trimmed = href.trim();
  if (trimmed === '' || trimmed === '#') return false;
  // A same-page fragment is not navigation; anything else is.
  return !trimmed.startsWith('#');
}

/**
 * Judges one candidate.
 *
 * Order is the safety property: structure, then denial, then necessity, then
 * positive evidence. Necessity comes before the allow-list so that a control
 * whose content is already present is reported as `unnecessary` — a fact worth
 * counting — rather than quietly passing the allow-list and being clicked.
 */
export function classifyExpansion(
  el: Element,
  plan: TraversalPlan,
  contentRoot: Element,
  activated: WeakSet<Element>,
): ExpansionVerdict {
  if (activated.has(el)) return { decision: 'refuse', reason: 'already activated' };

  const tag = el.tagName.toLowerCase();
  const isSummary = tag === 'summary';

  if (!contentRoot.contains(el)) {
    return { decision: 'refuse', reason: 'outside the content region' };
  }
  if (el.closest(CHROME_SELECTOR)) {
    return { decision: 'refuse', reason: 'inside page chrome or an input' };
  }
  for (const selector of plan.forbiddenSelectors) {
    try {
      if (el.closest(selector)) {
        return { decision: 'refuse', reason: `inside a region this source forbids (${selector})` };
      }
    } catch {
      continue;
    }
  }
  if ((el as HTMLButtonElement).disabled || el.getAttribute('aria-disabled') === 'true') {
    return { decision: 'refuse', reason: 'disabled' };
  }
  const type = (el.getAttribute('type') ?? '').toLowerCase();
  if (type === 'submit' || type === 'reset') {
    return { decision: 'refuse', reason: `a ${type} control` };
  }
  if (el.hasAttribute('aria-haspopup')) {
    return { decision: 'refuse', reason: 'opens a menu or dialog' };
  }
  if (navigates(el)) {
    return { decision: 'refuse', reason: 'would navigate' };
  }
  if (!isVisible(el)) {
    return { decision: 'refuse', reason: 'not visible' };
  }

  const label = accessibleName(el);
  if (ACTION_LABELS.some((pattern) => pattern.test(label))) {
    return { decision: 'refuse', reason: `labelled as an action (“${label}”)` };
  }

  const expanded = el.getAttribute('aria-expanded');
  if (expanded === 'true') {
    return { decision: 'refuse', reason: 'already expanded' };
  }

  const target = disclosureTarget(el);
  const availability = classifyAvailability(el, target);
  if (availability === 'visually_truncated') {
    return {
      decision: 'unnecessary',
      availability,
      reason: 'the content is already in the page, only its presentation is shortened',
    };
  }

  let matchesPlan = false;
  for (const selector of plan.expandSelectors) {
    try {
      if (el.matches(selector)) {
        matchesPlan = true;
        break;
      }
    } catch {
      continue;
    }
  }

  const closedDetails = isSummary && el.parentElement?.tagName.toLowerCase() === 'details'
    && !el.parentElement.hasAttribute('open');
  const disclosureLabel = DISCLOSURE_LABELS.some((pattern) => pattern.test(label));
  const declaredDisclosure = expanded === 'false' && (disclosureLabel || el.hasAttribute('aria-controls'));

  if (matchesPlan || closedDetails || declaredDisclosure || disclosureLabel) {
    return { decision: 'activate', availability };
  }

  return { decision: 'refuse', reason: 'no disclosure evidence' };
}

/** Everything worth judging, in the current content region. */
export function findCandidates(contentRoot: Element, plan: TraversalPlan): ExpansionCandidate[] {
  const selectors = [
    'button',
    'summary',
    '[role="button"]',
    '[aria-expanded]',
    ...plan.expandSelectors,
  ].join(',');

  let found: Element[] = [];
  try {
    found = Array.from(contentRoot.querySelectorAll(selectors));
  } catch {
    return [];
  }
  return found.map((el) => ({ el, label: accessibleName(el) }));
}

/**
 * A one-shot guard against the two things a misjudged click could still do.
 *
 * A capture-phase `submit` listener cancels any form submission the page
 * attempts while expansion is running, and the caller compares the URL after
 * every activation. Neither replaces the classifier; both exist because a
 * classifier over untrusted input will eventually be wrong about something,
 * and the failure should cost a note in the diagnostics rather than a sent
 * message or a lost page.
 */
export function installExpansionGuards(doc: Document): () => void {
  const cancel = (event: Event) => {
    event.preventDefault();
    event.stopPropagation();
  };
  doc.addEventListener('submit', cancel, true);
  return () => doc.removeEventListener('submit', cancel, true);
}
