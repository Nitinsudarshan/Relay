/**
 * Claude conversation extractor.
 *
 * Claude marks human turns with `data-testid="user-message"` and renders
 * assistant turns inside a response container. There is no single attribute
 * that states the role for both, so the role comes from *which selector
 * matched* — which is why the selectors are listed per role rather than
 * as one query with role inference afterwards.
 */

import { extractConversation } from './conversation';
import type { ExtractionResult, SiteExtractor } from '../types';

export const claudeExtractor: SiteExtractor = {
  id: 'claude',
  version: 1,

  matches(url: URL): boolean {
    return url.hostname.replace(/^www\./, '') === 'claude.ai';
  },

  extract(doc: Document): ExtractionResult | null {
    return extractConversation(doc, {
      extractorId: 'claude',
      extractorVersion: 1,
      strategies: [
        [
          { selector: '[data-testid="user-message"]', role: 'user' },
          { selector: '.font-claude-response', role: 'assistant' },
        ],
        [
          { selector: '[data-testid="user-message"]', role: 'user' },
          { selector: '[data-testid="assistant-message"]', role: 'assistant' },
        ],
        [
          { selector: '[data-test-render-count] [data-testid="user-message"]', role: 'user' },
          { selector: '.font-claude-message', role: 'assistant' },
        ],
      ],
      scrollerSelector: '[data-test-render-count], main',
    });
  },
};
