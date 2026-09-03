import { describe, expect, it } from 'vitest';
import { geminiExtractor } from './gemini';
import { pageFrom } from '../testing';

const URL_CONVERSATION = new URL('https://gemini.google.com/app/1a2b3c');

function userTurn(inner: string): string {
  return `<user-query><div class="query-text">${inner}</div></user-query>`;
}

function assistantTurn(inner: string): string {
  return `<model-response><div class="response-content">${inner}</div></model-response>`;
}

function conversation(body: string) {
  return pageFrom(`<main><infinite-scroller>${body}</infinite-scroller></main>`, {
    title: 'Gemini Discussion',
  });
}

describe('gemini extractor', () => {
  it('claims gemini.google.com only', () => {
    expect(geminiExtractor.matches(URL_CONVERSATION)).toBe(true);
    expect(geminiExtractor.matches(new URL('https://www.gemini.google.com/app/1'))).toBe(true);
    expect(geminiExtractor.matches(new URL('https://chatgpt.com/c/1'))).toBe(false);
    expect(geminiExtractor.matches(new URL('https://claude.ai/chat/1'))).toBe(false);
  });

  it('extracts a short conversation with roles from matching custom elements', () => {
    const doc = conversation(
      userTurn('<p>How should we structure Context Handoff?</p>') +
        assistantTurn('<p>Separate source from derived understanding.</p>'),
    );
    const result = geminiExtractor.extract(doc, URL_CONVERSATION)!;

    expect(result.messages.map((m) => m.role)).toEqual(['user', 'assistant']);
    expect(result.messages[0].blocks[0]).toMatchObject({
      text: 'How should we structure Context Handoff?',
    });
    expect(result.messages[1].blocks[0]).toMatchObject({
      text: 'Separate source from derived understanding.',
    });
  });

  it('keeps turn order across multi-turn exchanges', () => {
    const body = Array.from({ length: 6 }, (_, i) =>
      i % 2 === 0 ? userTurn(`<p>Prompt ${i}</p>`) : assistantTurn(`<p>Response ${i}</p>`),
    ).join('');
    const result = geminiExtractor.extract(conversation(body), URL_CONVERSATION)!;

    expect(result.messages).toHaveLength(6);
    expect(result.messages.map((m) => m.role)).toEqual([
      'user',
      'assistant',
      'user',
      'assistant',
      'user',
      'assistant',
    ]);
    expect(result.messages[5].blocks[0]).toMatchObject({ text: 'Response 5' });
  });

  it('preserves code blocks, lists and attachments in turns', () => {
    const doc = conversation(
      userTurn('<p>Show architecture details</p>') +
        assistantTurn(`
          <p>Here are the components:</p>
          <ul><li>Bridge</li><li>Vault</li><li>Context</li></ul>
          <pre><code class="language-rust">pub struct Context {}</code></pre>
          <a download href="https://example.com/spec.pdf">Download Specification</a>
        `),
    );
    const result = geminiExtractor.extract(doc, URL_CONVERSATION)!;
    const blocks = result.messages[1].blocks;
    const types = blocks.map((b) => b.type);

    expect(types).toContain('paragraph');
    expect(types).toContain('list');
    expect(types).toContain('code');
    expect(types).toContain('attachment');
  });

  it('supports fallback strategies with data-test-id or class-based markup', () => {
    const doc = pageFrom(
      `<main>
        <div data-test-id="user-query"><p>Test query</p></div>
        <div data-test-id="model-response"><p>Test response</p></div>
      </main>`,
      { title: 'Gemini Test' },
    );
    const result = geminiExtractor.extract(doc, URL_CONVERSATION)!;

    expect(result.messages).toHaveLength(2);
    expect(result.messages[0].role).toBe('user');
    expect(result.messages[1].role).toBe('assistant');
  });
});
