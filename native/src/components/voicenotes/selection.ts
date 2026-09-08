/**
 * Turning a DOM selection into a character range in the note's stored text.
 *
 * The correction is applied in Rust as a range edit against the persisted
 * content, so the only thing the UI has to get right is *which characters* the
 * user picked. Everything here is about that translation, and it is separate
 * from the component so it can be tested without rendering one.
 */

/** A phrase the user selected, addressed the way the backend expects. */
export interface PhraseSelection {
  noteId: string;
  /** Character offsets — code points, matching Rust's `chars()`. */
  start: number;
  end: number;
  text: string;
}

/**
 * Characters, not UTF-16 code units.
 *
 * `"👍".length` is 2 in JavaScript and one `char` in Rust. Devanagari is
 * unaffected (it is all BMP), but an emoji anywhere earlier in the note would
 * shift every later offset by one and silently corrupt the wrong span.
 */
export function codePointLength(text: string): number {
  return Array.from(text).length;
}

/**
 * The character offset of `node`/`offset` within `container`.
 *
 * Walks the container's text nodes in document order rather than assuming the
 * content is one text node. React renders it as one today; a future change that
 * splits it — a highlight, a search match — would silently break an assumption
 * and not a walk.
 *
 * Returns null when the node is not inside the container, which is how a
 * selection that started elsewhere on the page is rejected.
 */
export function offsetWithin(
  container: Node,
  node: Node,
  offset: number,
): number | null {
  if (!container.contains(node)) {
    return null;
  }
  const walker = document.createTreeWalker(container, NodeFilter.SHOW_TEXT);
  let total = 0;
  let current = walker.nextNode();
  while (current) {
    if (current === node) {
      return total + codePointLength((current.textContent ?? '').slice(0, offset));
    }
    total += codePointLength(current.textContent ?? '');
    current = walker.nextNode();
  }
  // The selection anchored on an element rather than a text node — a triple
  // click, usually. Treating it as the whole container is closer to what the
  // user meant than refusing.
  return node === container ? total : null;
}

/**
 * Reads the current window selection as a range in `content`.
 *
 * Returns null unless the selection is non-empty, lies entirely inside
 * `container`, and matches `content` at the offsets computed — the last check
 * being what stops a mismatch between what is rendered and what is stored from
 * reaching the backend as a corrupting edit.
 */
export function readSelection(
  container: HTMLElement | null,
  noteId: string,
  content: string,
): PhraseSelection | null {
  if (!container) return null;
  const selection = window.getSelection();
  if (!selection || selection.rangeCount === 0 || selection.isCollapsed) {
    return null;
  }

  const range = selection.getRangeAt(0);
  const rawStart = offsetWithin(container, range.startContainer, range.startOffset);
  const rawEnd = offsetWithin(container, range.endContainer, range.endOffset);
  if (rawStart === null || rawEnd === null) return null;

  const start = Math.min(rawStart, rawEnd);
  const end = Math.max(rawStart, rawEnd);
  if (end <= start) return null;

  const characters = Array.from(content);
  if (end > characters.length) return null;

  const text = characters.slice(start, end).join('');
  // Whitespace alone is not a phrase worth correcting, and the popover
  // appearing for a stray drag is noise.
  if (!text.trim()) return null;
  // What is rendered must be what is stored. If they disagree the offsets are
  // meaningless, and refusing is the only safe answer.
  if (text !== selection.toString()) return null;

  return { noteId, start, end, text };
}

/**
 * Whether this correction *looks* like vocabulary, for a hint beside the
 * checkbox.
 *
 * It never ticks the box. The box starts empty for every correction, because
 * "Thursday" → "Tuesday" is an ordinary edit and one wrongly pre-ticked
 * default puts junk in the dictionary of a user who did not read carefully.
 * This only offers an opinion next to a decision that stays theirs.
 *
 * The shape it recognises: the replacement is the same letters respaced, the
 * same word recapitalised, or a close mishearing of a proper noun.
 */
export function looksLikeVocabulary(original: string, replacement: string): boolean {
  const a = original.trim();
  const b = replacement.trim();
  if (!a || !b) return false;

  const squashed = (s: string) => s.replace(/[\s-]/g, '').toLowerCase();
  // "Lance DB" → "LanceDB": the same letters, respaced.
  if (squashed(a) === squashed(b)) return true;

  // "super base" → "Supabase": not the same letters — a mishearing *and* a
  // join. The signal is that the phrase collapses into fewer tokens while
  // staying close letter-for-letter, which is what Whisper splitting a product
  // name looks like. "Thursday" → "Tuesday" does not match: the token count is
  // unchanged, so no amount of letter similarity reaches this rule.
  const tokens = (s: string) => s.split(/\s+/).filter(Boolean).length;
  if (tokens(a) > tokens(b) && editDistance(squashed(a), squashed(b)) <= 2) {
    return true;
  }

  // "ollama" → "Ollama": the same word, capitalised.
  if (a.toLowerCase() === b.toLowerCase()) return true;

  // "tarui" → "Tauri": one token, close in length, a likely mishearing of a
  // proper noun.
  //
  // The original must NOT already be properly capitalised, and that condition
  // is doing the real work. Without it this rule also matches "Thursday" →
  // "Tuesday": one token each, one character apart, same first letter, and a
  // capitalised replacement. The difference is that Whisper had already
  // transcribed "Thursday" correctly as a word — the user is swapping meaning,
  // not repairing a mishearing — whereas a misheard proper noun arrives
  // lowercase or oddly spaced.
  const oneToken = !a.includes(' ') && !b.includes(' ');
  const closeInLength = Math.abs(a.length - b.length) <= 2;
  const startsAlike = a[0]?.toLowerCase() === b[0]?.toLowerCase();
  const originalLooksMisheard = !/^[A-Z]/.test(a);
  return oneToken && closeInLength && startsAlike && originalLooksMisheard && /^[A-Z]/.test(b);
}

/** Levenshtein distance, for judging how close two squashed phrases are. */
function editDistance(a: string, b: string): number {
  if (a === b) return 0;
  let previous = Array.from({ length: b.length + 1 }, (_, i) => i);
  for (let i = 1; i <= a.length; i += 1) {
    const current = [i];
    for (let j = 1; j <= b.length; j += 1) {
      const substitution = previous[j - 1] + (a[i - 1] === b[j - 1] ? 0 : 1);
      current[j] = Math.min(substitution, previous[j] + 1, current[j - 1] + 1);
    }
    previous = current;
  }
  return previous[b.length];
}
