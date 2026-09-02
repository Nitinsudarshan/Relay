import { describe, expect, it } from 'vitest';
import { chatgptExtractor } from './chatgpt';
import { pageFrom } from '../testing';

const URL_CONVERSATION = new URL('https://chatgpt.com/c/2b1f');

/** Markup shaped like a ChatGPT turn, with the role stated on the element. */
function turn(role: 'user' | 'assistant', inner: string, testId = 0): string {
  const label = role === 'user' ? 'You said:' : 'ChatGPT said:';
  return `
    <article data-testid="conversation-turn-${testId}">
      <h5 class="sr-only">${label}</h5>
      <div data-message-author-role="${role}">
        <div class="markdown">${inner}</div>
      </div>
    </article>`;
}

function conversation(body: string) {
  return pageFrom(`<main>${body}</main>`, { title: 'Designing capture' });
}

describe('chatgpt extractor', () => {
  it('claims chatgpt.com and chat.openai.com only', () => {
    expect(chatgptExtractor.matches(URL_CONVERSATION)).toBe(true);
    expect(chatgptExtractor.matches(new URL('https://chat.openai.com/c/1'))).toBe(true);
    expect(chatgptExtractor.matches(new URL('https://claude.ai/chat/1'))).toBe(false);
  });

  it('extracts a short conversation with roles and order intact', () => {
    const doc = conversation(
      turn('user', '<p>How should capture work?</p>', 2) +
        turn('assistant', '<p>Extract structure, not pixels.</p>', 3),
    );
    const result = chatgptExtractor.extract(doc, URL_CONVERSATION)!;

    expect(result.kind).toBe('conversation');
    expect(result.strategy).toBe('site');
    expect(result.messages.map((m) => m.role)).toEqual(['user', 'assistant']);
    expect(result.messages[0].blocks[0]).toMatchObject({
      type: 'paragraph',
      text: 'How should capture work?',
    });
  });

  it('keeps ordering across many turns', () => {
    const turns = Array.from({ length: 20 }, (_, i) =>
      turn(i % 2 === 0 ? 'user' : 'assistant', `<p>Turn ${i}</p>`, i),
    ).join('');
    const result = chatgptExtractor.extract(conversation(turns), URL_CONVERSATION)!;

    expect(result.messages).toHaveLength(20);
    expect(result.messages[0].role).toBe('user');
    expect(result.messages[19].role).toBe('assistant');
    result.messages.forEach((message, index) => {
      expect(message.blocks[0]).toMatchObject({ text: `Turn ${index}` });
    });
  });

  it('preserves code blocks and their language', () => {
    const doc = conversation(
      turn('user', '<p>Show me the struct.</p>') +
        turn(
          'assistant',
          '<p>Here:</p><pre><code class="language-rust">struct Capture { url: String }</code></pre>',
        ),
    );
    const result = chatgptExtractor.extract(doc, URL_CONVERSATION)!;
    const code = result.messages[1].blocks.find((b) => b.type === 'code');
    expect(code).toMatchObject({
      type: 'code',
      language: 'rust',
      text: 'struct Capture { url: String }',
    });
  });

  it('preserves tables inside an answer', () => {
    const doc = conversation(
      turn('assistant', '<table><tr><th>Option</th><th>Cost</th></tr><tr><td>A</td><td>Low</td></tr></table>'),
    );
    const result = chatgptExtractor.extract(doc, URL_CONVERSATION)!;
    expect(result.messages[0].blocks[0]).toMatchObject({
      type: 'table',
      headers: ['Option', 'Cost'],
      rows: [['A', 'Low']],
    });
  });

  it('always discloses that only rendered turns were read', () => {
    const result = chatgptExtractor.extract(
      conversation(turn('user', '<p>Hi</p>')),
      URL_CONVERSATION,
    )!;
    expect(result.coverage).toBe('rendered_dom');
    expect(result.notes.join(' ')).toMatch(/only the turns the page had rendered/i);
  });

  it('notes when the conversation is virtualized', () => {
    const doc = pageFrom(
      `<main><div data-virtuoso-scroller>${turn('user', '<p>Hi</p>')}</div></main>`,
    );
    const result = chatgptExtractor.extract(doc, URL_CONVERSATION)!;
    expect(result.notes.join(' ')).toMatch(/virtualized/i);
  });

  it('falls back to the accessibility labels when the role attribute is gone', () => {
    const doc = pageFrom(`
      <main>
        <article><h5 class="sr-only">You said:</h5><div class="markdown"><p>Question?</p></div></article>
        <article><h6 class="sr-only">ChatGPT said:</h6><div class="markdown"><p>Answer.</p></div></article>
      </main>
    `);
    const result = chatgptExtractor.extract(doc, URL_CONVERSATION)!;

    expect(result.messages.map((m) => m.role)).toEqual(['user', 'assistant']);
    expect(result.notes.join(' ')).toMatch(/primary layout/i);
  });

  it('gives up rather than guessing when nothing matches', () => {
    const doc = pageFrom('<main><div class="chat"><p>Some text</p></div></main>');
    expect(chatgptExtractor.extract(doc, URL_CONVERSATION)).toBeNull();
  });

  it('gives up on an empty conversation shell', () => {
    const doc = conversation('');
    expect(chatgptExtractor.extract(doc, URL_CONVERSATION)).toBeNull();
  });

  it('skips a turn that renders nothing readable', () => {
    const doc = conversation(
      turn('user', '<p>Real question.</p>') + turn('assistant', '<div></div>'),
    );
    const result = chatgptExtractor.extract(doc, URL_CONVERSATION)!;
    expect(result.messages).toHaveLength(1);
  });

  it('does not capture hidden turns', () => {
    const doc = pageFrom(`
      <main>
        ${turn('user', '<p>Visible question.</p>')}
        <article><div data-message-author-role="assistant" hidden><p>Draft answer.</p></div></article>
      </main>
    `);
    const result = chatgptExtractor.extract(doc, URL_CONVERSATION)!;
    expect(result.messages).toHaveLength(1);
    expect(JSON.stringify(result)).not.toContain('Draft answer');
  });
});
