import { describe, expect, it } from 'vitest';
import { githubExtractor } from './github';
import { pageFrom } from '../testing';

describe('github extractor', () => {
  it('claims github.com only', () => {
    expect(githubExtractor.matches(new URL('https://github.com/relay/relay'))).toBe(true);
    expect(githubExtractor.matches(new URL('https://gitlab.com/relay/relay'))).toBe(false);
  });

  it('extracts a repository as its description plus README', () => {
    const doc = pageFrom(
      `
      <p class="f4 my-3">A local-first capture system.</p>
      <div id="readme"><article class="markdown-body">
        <h1>Relay</h1>
        <p>Capture what you are looking at.</p>
        <pre><code class="language-bash">npm run dev</code></pre>
      </article></div>
    `,
      { title: 'relay/relay: A local-first capture system' },
    );

    const result = githubExtractor.extract(doc, new URL('https://github.com/relay/relay'))!;
    expect(result.kind).toBe('repository');
    expect(result.blocks[0]).toMatchObject({ text: 'A local-first capture system.' });
    expect(result.blocks.some((b) => b.type === 'code')).toBe(true);
    expect(result.coverage).toBe('full_document');
  });

  it('extracts an issue thread as a conversation attributed to its authors', () => {
    const doc = pageFrom(
      `
      <bdi class="js-issue-title">Capture is slow on long pages</bdi>
      <div class="js-comment-container">
        <a class="author">alice</a>
        <relative-time datetime="2026-02-01T10:00:00Z"></relative-time>
        <div class="comment-body"><p>It takes four seconds.</p></div>
      </div>
      <div class="js-comment-container">
        <a class="author">bob</a>
        <relative-time datetime="2026-02-01T11:00:00Z"></relative-time>
        <div class="comment-body"><p>Reproduced on a 40k word page.</p></div>
      </div>
    `,
      { title: 'Capture is slow on long pages · Issue #12' },
    );

    const result = githubExtractor.extract(
      doc,
      new URL('https://github.com/relay/relay/issues/12'),
    )!;

    expect(result.kind).toBe('conversation');
    expect(result.messages.map((m) => m.role)).toEqual(['alice', 'bob']);
    expect(result.messages[0].timestamp).toBe('2026-02-01T10:00:00Z');
    expect(result.title).toBe('Capture is slow on long pages');
  });

  it('extracts a pull request the same way as an issue', () => {
    const doc = pageFrom(`
      <h1 class="markdown-title">Add capture bridge</h1>
      <div class="timeline-comment">
        <a class="author">carol</a>
        <div class="markdown-body"><p>Adds the loopback listener.</p></div>
      </div>
    `);
    const result = githubExtractor.extract(
      doc,
      new URL('https://github.com/relay/relay/pull/9'),
    )!;
    expect(result.messages).toHaveLength(1);
    expect(result.messages[0].role).toBe('carol');
  });

  it('extracts a source file as one code block named after its path', () => {
    const doc = pageFrom(
      '<textarea id="read-only-cursor-text-area">fn main() {\n    println!("hi");\n}</textarea>',
    );
    const result = githubExtractor.extract(
      doc,
      new URL('https://github.com/relay/relay/blob/main/src/main.rs'),
    )!;

    const code = result.blocks.find((b) => b.type === 'code');
    expect(code).toMatchObject({ type: 'code', language: 'rs' });
    expect(code && code.type === 'code' && code.text).toContain('println!');
    expect(result.blocks[0]).toMatchObject({ type: 'heading', text: 'src/main.rs' });
  });

  it('hands unrecognised github pages to the generic extractor', () => {
    const doc = pageFrom('<div><p>Settings page</p></div>');
    expect(githubExtractor.extract(doc, new URL('https://github.com/settings/profile'))).toBeNull();
    expect(
      githubExtractor.extract(doc, new URL('https://github.com/relay/relay/issues/12')),
    ).toBeNull();
  });

  it('hands over a repository page with no README', () => {
    const doc = pageFrom('<p class="f4 my-3">Just a description.</p>');
    expect(githubExtractor.extract(doc, new URL('https://github.com/relay/relay'))).toBeNull();
  });
});
