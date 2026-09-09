import { describe, expect, it, beforeEach } from 'vitest';
import {
  codePointLength,
  findSelection,
  offsetWithin,
  readSelection,
  looksLikeVocabulary,
} from './selection';

describe('Voice Note selection → character range', () => {
  let container: HTMLElement;

  const render = (text: string) => {
    container = document.createElement('p');
    container.textContent = text;
    document.body.appendChild(container);
    return container;
  };

  const select = (node: Node, start: number, end: number) => {
    const range = document.createRange();
    range.setStart(node, start);
    range.setEnd(node, end);
    const selection = window.getSelection();
    selection?.removeAllRanges();
    selection?.addRange(range);
  };

  beforeEach(() => {
    document.body.innerHTML = '';
    window.getSelection()?.removeAllRanges();
  });

  it('counts characters rather than UTF-16 code units', () => {
    // An emoji is two code units and one Rust char. Getting this wrong shifts
    // every later offset by one and corrupts the wrong span.
    expect('👍'.length).toBe(2);
    expect(codePointLength('👍')).toBe(1);
    expect(codePointLength('मैं')).toBe(3);
  });

  it('reads a multi-word phrase as a range', () => {
    const content = 'I was testing super base yesterday.';
    const el = render(content);
    const start = content.indexOf('super base');
    select(el.firstChild!, start, start + 'super base'.length);

    expect(readSelection(el, 'note_1', content)).toEqual({
      noteId: 'note_1',
      start,
      end: start + 10,
      text: 'super base',
    });
  });

  it('reads the second occurrence as its own range', () => {
    // The range is what makes "only the selected occurrence" possible.
    const content = 'opened super base, then super base crashed';
    const el = render(content);
    const second = content.lastIndexOf('super base');
    select(el.firstChild!, second, second + 10);

    const result = readSelection(el, 'note_1', content);
    expect(result?.start).toBe(second);
    expect(result?.text).toBe('super base');
  });

  it('offsets survive an emoji earlier in the note', () => {
    const content = '👍 super base is fast';
    const el = render(content);
    // The DOM offset is in code units; the reported one must be in characters.
    const domStart = content.indexOf('super base');
    select(el.firstChild!, domStart, domStart + 10);

    const result = readSelection(el, 'note_1', content);
    expect(result?.text).toBe('super base');
    expect(Array.from(content).slice(result!.start, result!.end).join('')).toBe('super base');
  });

  it('handles Devanagari without shifting', () => {
    const content = 'मैं super base चला रहा हूं';
    const el = render(content);
    const start = content.indexOf('super base');
    select(el.firstChild!, start, start + 10);

    const result = readSelection(el, 'note_1', content);
    expect(result?.text).toBe('super base');
  });

  it('ignores a collapsed selection', () => {
    const content = 'nothing selected here';
    const el = render(content);
    select(el.firstChild!, 5, 5);
    expect(readSelection(el, 'note_1', content)).toBeNull();
  });

  it('ignores a whitespace-only selection', () => {
    const content = 'a    b';
    const el = render(content);
    select(el.firstChild!, 1, 5);
    expect(readSelection(el, 'note_1', content)).toBeNull();
  });

  it('ignores a selection outside the note', () => {
    const content = 'the note';
    const el = render(content);
    const other = document.createElement('p');
    other.textContent = 'somewhere else entirely';
    document.body.appendChild(other);
    select(other.firstChild!, 0, 9);

    expect(readSelection(el, 'note_1', content)).toBeNull();
  });

  it('refuses when what is rendered is not what is stored', () => {
    // Offsets are meaningless against content the view does not match, and a
    // mismatch reaching the backend would replace the wrong characters.
    const el = render('rendered text that drifted');
    select(el.firstChild!, 0, 8);
    expect(readSelection(el, 'note_1', 'the stored content is different')).toBeNull();
  });

  it('walks multiple text nodes rather than assuming one', () => {
    const el = document.createElement('p');
    el.appendChild(document.createTextNode('I tested '));
    el.appendChild(document.createTextNode('super base'));
    el.appendChild(document.createTextNode(' today'));
    document.body.appendChild(el);

    expect(offsetWithin(el, el.childNodes[1], 0)).toBe(9);
    expect(offsetWithin(el, el.childNodes[2], 0)).toBe(19);
  });
});

describe('finding which rendered note holds the selection', () => {
  const paragraph = (text: string) => {
    const el = document.createElement('p');
    el.textContent = text;
    document.body.appendChild(el);
    return el;
  };

  const select = (node: Node, start: number, end: number) => {
    const range = document.createRange();
    range.setStart(node, start);
    range.setEnd(node, end);
    const selection = window.getSelection();
    selection?.removeAllRanges();
    selection?.addRange(range);
  };

  beforeEach(() => {
    document.body.innerHTML = '';
    window.getSelection()?.removeAllRanges();
  });

  it('picks the note the selection is actually in', () => {
    const first = paragraph('I was testing super base yesterday.');
    const second = paragraph('Standup notes for tarui.');
    const containers = { note_1: first, note_2: second };
    const contents: Record<string, string> = {
      note_1: 'I was testing super base yesterday.',
      note_2: 'Standup notes for tarui.',
    };

    const at = contents.note_2.indexOf('tarui');
    select(second.firstChild!, at, at + 5);

    expect(findSelection(containers, (id) => contents[id])).toEqual({
      noteId: 'note_2',
      start: at,
      end: at + 5,
      text: 'tarui',
    });
  });

  it('finds a selection made without the mouse', () => {
    // Extending a range is what shift+arrow does. The page listens on
    // `selectionchange`, so a keyboard selection arrives the same way a
    // dragged one does — a `<p>` cannot be focused, so a keyup on it never
    // would have fired.
    const el = paragraph('we should try super base');
    const range = document.createRange();
    range.setStart(el.firstChild!, 14);
    range.setEnd(el.firstChild!, 14);
    const selection = window.getSelection()!;
    selection.removeAllRanges();
    selection.addRange(range);
    for (let i = 0; i < 10; i += 1) {
      selection.extend(el.firstChild!, 15 + i);
    }

    expect(findSelection({ note_1: el }, () => 'we should try super base')).toEqual({
      noteId: 'note_1',
      start: 14,
      end: 24,
      text: 'super base',
    });
  });

  it('ignores a container that is no longer rendered', () => {
    const el = paragraph('I was testing super base yesterday.');
    select(el.firstChild!, 14, 24);
    expect(findSelection({ note_1: null }, () => 'I was testing super base yesterday.')).toBeNull();
  });

  it('ignores a note whose content the caller no longer has', () => {
    const el = paragraph('I was testing super base yesterday.');
    select(el.firstChild!, 14, 24);
    expect(findSelection({ note_1: el }, () => undefined)).toBeNull();
  });

  it('returns null when nothing is selected', () => {
    paragraph('I was testing super base yesterday.');
    expect(findSelection({}, () => 'anything')).toBeNull();
  });
});

describe('suggesting whether a correction is vocabulary', () => {
  it('suggests learning a respaced product name', () => {
    expect(looksLikeVocabulary('super base', 'Supabase')).toBe(true);
    expect(looksLikeVocabulary('Lance DB', 'LanceDB')).toBe(true);
  });

  it('suggests learning a capitalisation fix', () => {
    expect(looksLikeVocabulary('ollama', 'Ollama')).toBe(true);
  });

  it('suggests learning a close mishearing of a proper noun', () => {
    expect(looksLikeVocabulary('tarui', 'Tauri')).toBe(true);
  });

  it('does NOT suggest learning an ordinary word swap', () => {
    // The requirement that keeps the dictionary clean: this is a normal edit,
    // not vocabulary, and must never be pre-ticked.
    expect(looksLikeVocabulary('Thursday', 'Tuesday')).toBe(false);
    expect(looksLikeVocabulary('increase', 'decrease')).toBe(false);
    expect(looksLikeVocabulary('we should ship', 'we should wait')).toBe(false);
  });

  it('ignores empty input', () => {
    expect(looksLikeVocabulary('', 'Supabase')).toBe(false);
    expect(looksLikeVocabulary('super base', '  ')).toBe(false);
  });
});
