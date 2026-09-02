import { describe, expect, it } from 'vitest';
import {
  assessCoverage,
  blocksTextLength,
  collectLinks,
  extractBlocks,
  isHidden,
  normalizeWhitespace,
  readMetadata,
} from './dom';
import { documentFrom, pageFrom } from './testing';

function blocksOf(html: string) {
  const doc = pageFrom(html);
  return extractBlocks(doc.body).blocks;
}

describe('normalizeWhitespace', () => {
  it('collapses runs of spaces and normalizes line endings', () => {
    expect(normalizeWhitespace('a  \t b\r\nc\r\n\r\n\r\nd')).toBe('a b\nc\n\nd');
  });

  it('leaves unicode alone', () => {
    expect(normalizeWhitespace(' 日本語 — ✅ ')).toBe('日本語 — ✅');
  });
});

describe('extractBlocks', () => {
  it('produces headings, paragraphs, lists, quotes and code in reading order', () => {
    const blocks = blocksOf(`
      <h2>Title</h2>
      <p>First paragraph.</p>
      <ul><li>one</li><li>two</li></ul>
      <ol><li>step</li></ol>
      <blockquote>A quotation.</blockquote>
      <pre><code class="language-rust">fn main() {}</code></pre>
    `);

    expect(blocks.map((b) => b.type)).toEqual([
      'heading',
      'paragraph',
      'list',
      'list',
      'quote',
      'code',
    ]);
    expect(blocks[0]).toMatchObject({ type: 'heading', level: 2, text: 'Title' });
    expect(blocks[2]).toMatchObject({ type: 'list', ordered: false, items: ['one', 'two'] });
    expect(blocks[3]).toMatchObject({ type: 'list', ordered: true });
    expect(blocks[5]).toMatchObject({ type: 'code', language: 'rust', text: 'fn main() {}' });
  });

  it('reads tables with headers and ragged rows', () => {
    const blocks = blocksOf(`
      <table>
        <tr><th>Key</th><th>Value</th></tr>
        <tr><td>a</td><td>1</td></tr>
        <tr><td>b</td></tr>
      </table>
    `);
    expect(blocks).toHaveLength(1);
    expect(blocks[0]).toMatchObject({
      type: 'table',
      headers: ['Key', 'Value'],
      rows: [['a', '1'], ['b']],
    });
  });

  it('keeps a table with no header row as body rows', () => {
    const blocks = blocksOf('<table><tr><td>a</td><td>b</td></tr></table>');
    expect(blocks[0]).toMatchObject({ type: 'table', headers: [], rows: [['a', 'b']] });
  });

  it('captures text written directly inside a container', () => {
    const blocks = blocksOf('<div>Loose text<span> and more</span></div>');
    const text = blocks.map((b) => (b.type === 'paragraph' ? b.text : '')).join('|');
    expect(text).toContain('Loose text');
    expect(text).toContain('and more');
  });

  it('skips hidden content', () => {
    const blocks = blocksOf(`
      <p>Visible.</p>
      <p style="display:none">Hidden by style.</p>
      <p hidden>Hidden by attribute.</p>
      <p aria-hidden="true">Hidden from assistive tech.</p>
    `);
    expect(blocks).toHaveLength(1);
    expect(blocks[0]).toMatchObject({ text: 'Visible.' });
  });

  it('skips scripts, styles and form controls', () => {
    const blocks = blocksOf(`
      <script>alert('x')</script>
      <style>.a{color:red}</style>
      <button>Click</button>
      <textarea>draft</textarea>
      <p>Real content.</p>
    `);
    expect(blocks).toHaveLength(1);
    expect(blocks[0]).toMatchObject({ text: 'Real content.' });
  });

  it('drops consecutive duplicates of the same text', () => {
    const blocks = blocksOf('<p>Repeated.</p><p>Repeated.</p><p>Different.</p>');
    expect(blocks).toHaveLength(2);
  });

  it('survives malformed and unclosed markup', () => {
    const doc = documentFrom('<body><div><p>One<p>Two<ul><li>a<li>b</div>');
    const { blocks } = extractBlocks(doc.body);
    const texts = blocks.flatMap((b) =>
      b.type === 'paragraph' ? [b.text] : b.type === 'list' ? b.items : [],
    );
    expect(texts).toContain('One');
    expect(texts).toContain('Two');
    expect(texts).toContain('a');
  });

  it('does not recurse without bound on deeply nested markup', () => {
    const depth = 200;
    const html = '<div>'.repeat(depth) + 'deep text' + '</div>'.repeat(depth);
    const doc = pageFrom(html);
    const { blocks, truncated } = extractBlocks(doc.body);
    expect(truncated).toBe(true);
    expect(blocks.some((b) => b.type === 'paragraph' && b.text.includes('deep text'))).toBe(true);
  });

  it('keeps images with a source or an alt text, and drops empty ones', () => {
    const blocks = blocksOf(
      '<img src="https://example.com/a.png" alt="A diagram"><img alt=""><img src="b.png">',
    );
    expect(blocks).toHaveLength(2);
    expect(blocks[0]).toMatchObject({ type: 'image', alt: 'A diagram' });
  });

  it('preserves unicode and emoji', () => {
    const blocks = blocksOf('<p>日本語のテキスト — ok ✅</p>');
    expect(blocks[0]).toMatchObject({ text: '日本語のテキスト — ok ✅' });
  });

  it('returns nothing for an empty document', () => {
    expect(blocksOf('')).toHaveLength(0);
  });
});

describe('isHidden', () => {
  it('recognises every common way of hiding an element', () => {
    const doc = pageFrom(
      '<p id="a" hidden>a</p><p id="b" style="visibility: hidden">b</p><p id="c">c</p>',
    );
    expect(isHidden(doc.getElementById('a')!)).toBe(true);
    expect(isHidden(doc.getElementById('b')!)).toBe(true);
    expect(isHidden(doc.getElementById('c')!)).toBe(false);
  });
});

describe('collectLinks', () => {
  it('resolves relative links and rejects anything that is not http(s)', () => {
    const doc = pageFrom(`
      <a href="/docs">Docs</a>
      <a href="https://example.org/x">External</a>
      <a href="javascript:alert(1)">Bad</a>
      <a href="mailto:a@b.c">Mail</a>
      <a href="#section">Anchor</a>
      <a href="/docs">Docs again</a>
    `);
    const links = collectLinks(doc.body, 'https://example.com/page');
    expect(links.map((l) => l.href)).toEqual([
      'https://example.com/docs',
      'https://example.org/x',
    ]);
    expect(links[0].text).toBe('Docs');
  });
});

describe('readMetadata', () => {
  it('reads canonical url, author, publication date and language', () => {
    const doc = pageFrom('<p>x</p>', {
      lang: 'fr',
      head: `
        <link rel="canonical" href="https://example.com/canonical">
        <meta name="author" content="A. Writer">
        <meta property="article:published_time" content="2026-01-02T00:00:00Z">
        <meta property="og:site_name" content="Example">
        <meta name="description" content="A description.">
      `,
    });
    expect(readMetadata(doc)).toEqual({
      canonical_url: 'https://example.com/canonical',
      site_name: 'Example',
      author: 'A. Writer',
      published_at: '2026-01-02T00:00:00Z',
      description: 'A description.',
      language: 'fr',
    });
  });

  it('returns undefined for metadata a page does not provide', () => {
    const metadata = readMetadata(pageFrom('<p>x</p>'));
    expect(metadata.author).toBeUndefined();
    expect(metadata.canonical_url).toBeUndefined();
  });
});

describe('assessCoverage', () => {
  it('claims a full document only when nearly all visible text was captured', () => {
    const doc = pageFrom('<p>' + 'word '.repeat(100) + '</p>');
    const captured = blocksTextLength(extractBlocks(doc.body).blocks);
    expect(assessCoverage(doc, captured, false).coverage).toBe('full_document');
  });

  it('falls back to rendered_dom when much of the page was not recognised', () => {
    const doc = pageFrom('<p>short</p>' + '<div>' + 'x'.repeat(5000) + '</div>');
    const assessment = assessCoverage(doc, 5, false);
    expect(assessment.coverage).toBe('rendered_dom');
    expect(assessment.notes.join(' ')).toMatch(/% of the page/);
  });

  it('never claims a full document from a virtualized list', () => {
    const doc = pageFrom('<div data-virtuoso-scroller><p>a</p></div>');
    const assessment = assessCoverage(doc, 10_000, false);
    expect(assessment.coverage).toBe('rendered_dom');
    expect(assessment.notes.join(' ')).toMatch(/virtualized/);
  });

  it('reports partial coverage when extraction was cut short', () => {
    const doc = pageFrom('<p>a</p>');
    expect(assessCoverage(doc, 1, true).coverage).toBe('partial');
  });

  it('reports unknown coverage for a page with no visible text', () => {
    const doc = pageFrom('');
    expect(assessCoverage(doc, 0, false).coverage).toBe('unknown');
  });
});
