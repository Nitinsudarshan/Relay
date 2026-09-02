/**
 * ChatGPT conversation extractor.
 *
 * `data-message-author-role` is the attribute ChatGPT puts on every message
 * element, and it carries the role directly — no inference, no alternation
 * assumption. The fallbacks below exist because that attribute is an
 * implementation detail of a site that redesigns often; each one is weaker
 * than the last, and the last one hands over to the generic extractor.
 */

import { extractConversation } from './conversation';
import type { ExtractionResult, SiteExtractor } from '../types';

const HOSTS = new Set(['chatgpt.com', 'chat.openai.com']);

/** Screen-reader labels ChatGPT renders above each turn. */
const SPOKEN_ROLE = /(you|chatgpt)\s+said/i;

function roleFromSpokenLabel(el: Element): string | null {
  const label = el.querySelector('h5, h6, [class*="sr-only"]');
  const text = label?.textContent ?? '';
  const match = SPOKEN_ROLE.exec(text);
  if (!match) return null;
  return match[1].toLowerCase() === 'you' ? 'user' : 'assistant';
}

export const chatgptExtractor: SiteExtractor = {
  id: 'chatgpt',
  version: 1,

  matches(url: URL): boolean {
    return HOSTS.has(url.hostname.replace(/^www\./, ''));
  },

  extract(doc: Document): ExtractionResult | null {
    return extractConversation(doc, {
      extractorId: 'chatgpt',
      extractorVersion: 1,
      strategies: [
        // The role is stated on the element. Nothing is inferred.
        [{ selector: '[data-message-author-role]', role: null }],
        // The role is stated in the turn's own accessibility label.
        [{ selector: 'main article', role: null }],
      ],
      resolveRole: (el) =>
        el.getAttribute('data-message-author-role') ?? roleFromSpokenLabel(el),
      scrollerSelector: 'main [class*="overflow-y-auto"], main',
    });
  },
};
