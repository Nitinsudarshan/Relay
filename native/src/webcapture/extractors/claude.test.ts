import { describe, expect, it } from 'vitest';
import { claudeExtractor } from './claude';
import { pageFrom } from '../testing';

const URL_CONVERSATION = new URL('https://claude.ai/chat/7c31');

function userTurn(inner: string): string {
  return `<div data-testid="user-message">${inner}</div>`;
}

function assistantTurn(inner: string): string {
  return `<div class="font-claude-response"><div class="grid-cols-1">${inner}</div></div>`;
}

function conversation(body: string) {
  return pageFrom(`<main><div data-test-render-count="1">${body}</div></main>`, {
    title: 'Relay capture design',
  });
}

describe('claude extractor', () => {
  it('claims claude.ai only', () => {
    expect(claudeExtractor.matches(URL_CONVERSATION)).toBe(true);
    expect(claudeExtractor.matches(new URL('https://www.claude.ai/chat/1'))).toBe(true);
    expect(claudeExtractor.matches(new URL('https://chatgpt.com/c/1'))).toBe(false);
  });

  it('extracts a short conversation with roles from the matching selector', () => {
    const doc = conversation(
      userTurn('<p>What should capture preserve?</p>') +
        assistantTurn('<p>Provenance, structure and the raw source.</p>'),
    );
    const result = claudeExtractor.extract(doc, URL_CONVERSATION)!;

    expect(result.messages.map((m) => m.role)).toEqual(['user', 'assistant']);
    expect(result.messages[1].blocks[0]).toMatchObject({
      text: 'Provenance, structure and the raw source.',
    });
  });

  it('keeps document order across a multi-turn thread', () => {
    const body = Array.from({ length: 12 }, (_, i) =>
      i % 2 === 0 ? userTurn(`<p>Q${i}</p>`) : assistantTurn(`<p>A${i}</p>`),
    ).join('');
    const result = claudeExtractor.extract(conversation(body), URL_CONVERSATION)!;

    expect(result.messages).toHaveLength(12);
    expect(result.messages.map((m) => m.role).slice(0, 4)).toEqual([
      'user',
      'assistant',
      'user',
      'assistant',
    ]);
    expect(result.messages[11].blocks[0]).toMatchObject({ text: 'A11' });
  });

  it('preserves code blocks, lists and tables inside an answer', () => {
    const doc = conversation(
      userTurn('<p>Show me.</p>') +
        assistantTurn(`
          <ul><li>first</li><li>second</li></ul>
          <pre><code class="language-typescript">const a = 1;</code></pre>
          <table><tr><th>A</th></tr><tr><td>1</td></tr></table>
        `),
    );
    const result = claudeExtractor.extract(doc, URL_CONVERSATION)!;
    const types = result.messages[1].blocks.map((b) => b.type);

    expect(types).toContain('list');
    expect(types).toContain('code');
    expect(types).toContain('table');
    expect(result.messages[1].blocks.find((b) => b.type === 'code')).toMatchObject({
      language: 'typescript',
    });
  });

  it('uses the secondary selectors when the response class changes', () => {
    const doc = conversation(
      userTurn('<p>Question.</p>') +
        '<div data-testid="assistant-message"><p>Answer.</p></div>',
    );
    const result = claudeExtractor.extract(doc, URL_CONVERSATION)!;

    expect(result.messages.map((m) => m.role)).toEqual(['user', 'assistant']);
    expect(result.notes.join(' ')).toMatch(/primary layout/i);
  });

  it('discloses that only rendered turns were captured', () => {
    const result = claudeExtractor.extract(
      conversation(userTurn('<p>Hi</p>')),
      URL_CONVERSATION,
    )!;
    expect(result.coverage).toBe('rendered_dom');
    expect(result.notes.join(' ')).toMatch(/load earlier turns as you scroll up/i);
  });

  it('gives up rather than guessing on an unrecognised layout', () => {
    const doc = pageFrom('<main><div class="thread"><p>text</p></div></main>');
    expect(claudeExtractor.extract(doc, URL_CONVERSATION)).toBeNull();
  });

  it('gives up on a conversation with no turns rendered yet', () => {
    expect(claudeExtractor.extract(conversation(''), URL_CONVERSATION)).toBeNull();
  });

  it('captures unicode-heavy answers verbatim', () => {
    const doc = conversation(assistantTurn('<p>日本語 — ✅ Ünïcødé</p>'));
    const result = claudeExtractor.extract(doc, URL_CONVERSATION)!;
    expect(result.messages[0].blocks[0]).toMatchObject({ text: '日本語 — ✅ Ünïcødé' });
  });
});
