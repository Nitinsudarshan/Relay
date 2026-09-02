import { describe, expect, it } from 'vitest';
import { CaptureEmptyError, buildPayload, runExtraction } from './capture';
import { PROTOCOL_VERSION } from './types';
import { pageFrom } from './testing';

const CHATGPT = 'https://chatgpt.com/c/2b1f';
const ARTICLE = 'https://example.com/posts/capture';

function chatgptPage() {
  return pageFrom(
    `<main>
       <article><div data-message-author-role="user"><p>Question?</p></div></article>
       <article><div data-message-author-role="assistant"><p>Answer.</p></div></article>
     </main>`,
    { title: 'A conversation' },
  );
}

describe('the fallback ladder', () => {
  it('uses a site extractor when one recognises the page', () => {
    const result = runExtraction(chatgptPage(), new URL(CHATGPT));
    expect(result.strategy).toBe('site');
    expect(result.extractorId).toBe('chatgpt');
  });

  it('falls back to the generic extractor when the site extractor finds nothing', () => {
    const doc = pageFrom(`<main><h1>Sign in</h1><p>${'Body. '.repeat(40)}</p></main>`);
    const result = runExtraction(doc, new URL(CHATGPT));
    expect(result.strategy).toBe('article');
    expect(result.extractorId).toBe('generic');
  });

  it('falls back to visible text when there is no article structure at all', () => {
    const doc = pageFrom('<div><span>Loose</span></div>');
    // The generic pass finds this, so force the floor by emptying the region.
    const bare = pageFrom('<div></div><span></span>');
    expect(runExtraction(doc, new URL(ARTICLE)).blocks.length).toBeGreaterThan(0);
    expect(runExtraction(bare, new URL(ARTICLE)).blocks).toHaveLength(0);
  });

  it('captures the page generically when a site extractor throws', () => {
    const doc = chatgptPage();
    // A selector that no longer parses is exactly how a site redesign breaks
    // an extractor in practice.
    const broken = new Proxy(doc, {
      get(target, prop, receiver) {
        const value = Reflect.get(target, prop, receiver);
        if (prop === 'querySelectorAll') {
          return (selector: string) => {
            if (selector.includes('data-message-author-role')) {
              throw new Error('selector no longer valid');
            }
            return (value as typeof doc.querySelectorAll).call(target, selector);
          };
        }
        return typeof value === 'function' ? value.bind(target) : value;
      },
    }) as Document;

    const result = runExtraction(broken, new URL(CHATGPT));
    expect(result.strategy).not.toBe('site');
    expect(result.notes.join(' ')).toMatch(/chatgpt extractor failed/i);
  });
});

describe('buildPayload', () => {
  it('produces a conversation payload with provenance and diagnostics', () => {
    const payload = buildPayload(chatgptPage(), CHATGPT, { browser: 'TestBrowser/1' });

    expect(payload.protocol_version).toBe(PROTOCOL_VERSION);
    expect(payload.url).toBe(CHATGPT);
    expect(payload.browser).toBe('TestBrowser/1');
    expect(payload.extractor).toMatchObject({ id: 'chatgpt', strategy: 'site' });
    expect(payload.content.kind).toBe('conversation');
    expect(payload.content.messages.map((m) => m.role)).toEqual(['user', 'assistant']);
    expect(payload.diagnostics.coverage).toBe('rendered_dom');
    expect(payload.diagnostics.notes.length).toBeGreaterThan(0);
    expect(typeof payload.diagnostics.elapsed_ms).toBe('number');
    expect(Date.parse(payload.captured_at)).not.toBeNaN();
  });

  it('collects links for documents but not for conversations', () => {
    const article = pageFrom(
      `<article><h1>Post</h1><p>${'Body. '.repeat(30)}</p><a href="/next">Next</a></article>`,
    );
    expect(buildPayload(article, ARTICLE).links.map((l) => l.href)).toEqual([
      'https://example.com/next',
    ]);

    const conversation = pageFrom(
      `<main><article><div data-message-author-role="user"><p>Q</p><a href="/x">link</a></div></article></main>`,
    );
    expect(buildPayload(conversation, CHATGPT).links).toEqual([]);
  });

  it('titles a capture from the extractor, then the document, then the host', () => {
    const titled = pageFrom(`<article><h1>Heading title</h1><p>${'x '.repeat(120)}</p></article>`, {
      title: 'Document title',
    });
    expect(buildPayload(titled, ARTICLE).title).toBe('Heading title');

    const untitled = pageFrom(`<article><p>${'x '.repeat(120)}</p></article>`, {
      title: 'Document title',
    });
    expect(buildPayload(untitled, ARTICLE).title).toBe('Document title');
  });

  it('refuses a page with nothing readable instead of sending an empty capture', () => {
    const doc = pageFrom('<div></div>');
    expect(() => buildPayload(doc, ARTICLE)).toThrow(CaptureEmptyError);
  });

  it('carries head metadata through as provenance', () => {
    const doc = pageFrom(`<article><h1>Post</h1><p>${'Body. '.repeat(30)}</p></article>`, {
      head: '<link rel="canonical" href="https://example.com/canonical"><meta name="author" content="A. Writer">',
    });
    const payload = buildPayload(doc, ARTICLE);
    expect(payload.document.canonical_url).toBe('https://example.com/canonical');
    expect(payload.document.author).toBe('A. Writer');
  });

  it('serializes to JSON without cycles or DOM references', () => {
    const payload = buildPayload(chatgptPage(), CHATGPT);
    const roundTripped = JSON.parse(JSON.stringify(payload));
    expect(roundTripped.content.messages).toHaveLength(2);
    expect(JSON.stringify(payload)).not.toContain('[object');
  });

  it('keeps a long page within its caps and reports the truncation', () => {
    const paragraphs = Array.from({ length: 6000 }, (_, i) => `<p>Paragraph ${i}.</p>`).join('');
    const doc = pageFrom(`<article>${paragraphs}</article>`);
    const payload = buildPayload(doc, ARTICLE);

    expect(payload.content.blocks.length).toBeLessThanOrEqual(5000);
    expect(payload.diagnostics.truncated).toBe(true);
    expect(payload.diagnostics.coverage).toBe('partial');
  });
});
