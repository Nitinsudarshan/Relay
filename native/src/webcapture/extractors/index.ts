/**
 * The extractor registry.
 *
 * Adding support for a site is a new module and one line here. Nothing else
 * in the capture path knows which sites exist, which is what keeps this from
 * turning into one function full of hostname branches.
 */

import { chatgptExtractor } from './chatgpt';
import { claudeExtractor } from './claude';
import { geminiExtractor } from './gemini';
import { githubExtractor } from './github';
import type { SiteExtractor } from '../types';

export const SITE_EXTRACTORS: SiteExtractor[] = [
  chatgptExtractor,
  claudeExtractor,
  geminiExtractor,
  githubExtractor,
];

/** The first registered extractor that claims this URL, if any. */
export function selectExtractor(url: URL): SiteExtractor | null {
  return SITE_EXTRACTORS.find((extractor) => extractor.matches(url)) ?? null;
}

export { chatgptExtractor, claudeExtractor, geminiExtractor, githubExtractor };
export { chatgptTraversal } from './chatgpt';
export { claudeTraversal } from './claude';
export { geminiTraversal } from './gemini';
export { githubTraversal } from './github';
export { extractGeneric, extractVisibleText, harvestGeneric } from './generic';
