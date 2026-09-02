import { describe, expect, it } from 'vitest';
import { extractGeneric, extractVisibleText, findMainRegion } from './generic';
import { pageFrom } from '../testing';

const NAV = `
  <nav><a href="/a">Home</a><a href="/b">Docs</a><a href="/c">Blog</a><a href="/d">About</a></nav>
`;
const SIDEBAR = `
  <aside class="sidebar">
    <ul><li><a href="/x1">Related one</a></li><li><a href="/x2">Related two</a></li></ul>
  </aside>
`;

describe('findMainRegion', () => {
  it('prefers a semantic container', () => {
    const doc = pageFrom(
      `${NAV}<article id="post"><p>${'Body sentence. '.repeat(30)}</p></article>${SIDEBAR}`,
    );
    expect(findMainRegion(doc).id).toBe('post');
  });

  it('scores its way to the article when the page has no semantic markup', () => {
    const doc = pageFrom(`
      ${NAV}
      <div class="wrap">
        <div id="rail"><a href="/1">One</a><a href="/2">Two</a><a href="/3">Three</a></div>
        <div id="body"><p>${'Real prose here. '.repeat(40)}</p><p>More prose.</p></div>
      </div>
    `);
    expect(findMainRegion(doc).id).toBe('body');
  });

  it('falls back to the body when there is nothing to choose between', () => {
    const doc = pageFrom('');
    expect(findMainRegion(doc).tagName.toLowerCase()).toBe('body');
  });
});

describe('extractGeneric', () => {
  it('captures an article without its navigation and sidebar', () => {
    const doc = pageFrom(
      `${NAV}<article><h1>How capture works</h1><p>${'Body sentence. '.repeat(20)}</p></article>${SIDEBAR}`,
      { title: 'How capture works — Example' },
    );
    const result = extractGeneric(doc);
    const text = JSON.stringify(result.blocks);

    expect(result.kind).toBe('article');
    expect(result.strategy).toBe('article');
    expect(result.title).toBe('How capture works');
    expect(text).toContain('Body sentence.');
    expect(text).not.toContain('Related one');
    expect(text).not.toContain('Docs');
  });

  it('captures a documentation page with headings, code and tables', () => {
    const doc = pageFrom(`
      <main>
        <h1>Configuration</h1>
        <h2>Ports</h2>
        <p>Relay listens on loopback.</p>
        <pre><code class="language-json">{ "port": 8765 }</code></pre>
        <table><tr><th>Setting</th><th>Default</th></tr><tr><td>port</td><td>8765</td></tr></table>
        <ol><li>Enable capture</li><li>Pair the extension</li></ol>
      </main>
    `);
    const types = extractGeneric(doc).blocks.map((b) => b.type);
    expect(types).toEqual(
      expect.arrayContaining(['heading', 'paragraph', 'code', 'table', 'list']),
    );
  });

  it('captures a blog post with a blockquote and an image', () => {
    const doc = pageFrom(`
      <article>
        <h1>On capture</h1>
        <p>${'Prose. '.repeat(30)}</p>
        <blockquote>Preserve the source.</blockquote>
        <img src="https://example.com/diagram.png" alt="The capture ladder">
      </article>
    `);
    const result = extractGeneric(doc);
    expect(result.blocks.some((b) => b.type === 'quote')).toBe(true);
    expect(result.blocks.find((b) => b.type === 'image')).toMatchObject({
      alt: 'The capture ladder',
      src: 'https://example.com/diagram.png',
    });
  });

  it('reports honest coverage for a page it mostly could not read', () => {
    const doc = pageFrom(
      `<main><p>tiny</p></main><div>${'unreadable filler '.repeat(400)}</div>`,
    );
    const result = extractGeneric(doc);
    expect(['rendered_dom', 'full_document']).toContain(result.coverage);
    if (result.coverage === 'rendered_dom') {
      expect(result.notes.join(' ')).toMatch(/of the page/i);
    }
  });

  it('produces almost nothing for a page with almost no text', () => {
    const doc = pageFrom('<main></main>');
    const result = extractGeneric(doc);
    expect(result.blocks).toHaveLength(0);
  });

  it('titles a page from its <title> when it has no heading', () => {
    const doc = pageFrom(`<main><p>${'Body. '.repeat(40)}</p></main>`, {
      title: 'Fallback title',
    });
    expect(extractGeneric(doc).title).toBe('Fallback title');
  });
});

describe('extractVisibleText', () => {
  it('captures the page text as paragraphs and says that is what it did', () => {
    const doc = pageFrom('<div><span>Line one</span><div>Line two</div></div>');
    const result = extractVisibleText(doc);

    expect(result.strategy).toBe('text');
    expect(result.coverage).toBe('rendered_dom');
    expect(result.notes.join(' ')).toMatch(/could not recognise/i);
    expect(JSON.stringify(result.blocks)).toContain('Line one');
  });

  it('returns no blocks for an empty page rather than an empty paragraph', () => {
    expect(extractVisibleText(pageFrom('')).blocks).toHaveLength(0);
  });
});
