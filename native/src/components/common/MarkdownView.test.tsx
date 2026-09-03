import React from 'react';
import { describe, expect, test } from 'vitest';
import { render, screen } from '@testing-library/react';
import { MarkdownView } from './MarkdownView';

describe('MarkdownView layout and containment', () => {
  test('renders markdown tables into structured HTML table with horizontal scroll container', () => {
    const markdown = `
| Header A | Header B | Header C |
| --- | --- | --- |
| Row 1 A | Row 1 B | Row 1 C |
| Row 2 A | Row 2 B | Row 2 C |
    `.trim();

    const { container } = render(<MarkdownView content={markdown} />);

    // Table elements exist
    expect(screen.getByText('Header A')).toBeDefined();
    expect(screen.getByText('Row 1 B')).toBeDefined();
    expect(screen.getByText('Row 2 C')).toBeDefined();

    // Contains table element
    const table = container.querySelector('table');
    expect(table).not.toBeNull();

    // Contains scrollable container preventing viewport overflow
    const tableWrapper = container.querySelector('.overflow-x-auto.max-w-full');
    expect(tableWrapper).not.toBeNull();
  });

  test('renders markdown images with responsive containment', () => {
    const markdown = '![Diagram Preview](https://example.com/preview.png)';

    const { container } = render(<MarkdownView content={markdown} />);
    const img = container.querySelector('img');
    expect(img).not.toBeNull();
    expect(img?.getAttribute('src')).toBe('https://example.com/preview.png');
    expect(img?.getAttribute('alt')).toBe('Diagram Preview');
    expect(img?.className).toContain('max-w-full');
  });

  test('renders paragraphs and code blocks with word break containment', () => {
    const markdown = `
A very long unbroken URL like https://github.com/stablyai/orca/blob/main/deep/nested/path/to/some/source/file/with/extreme/length.ts

\`\`\`typescript
const veryLongSymbol = "unbreakable-string-value-that-should-never-blow-out-the-editor-viewport";
\`\`\`
    `.trim();

    const { container } = render(<MarkdownView content={markdown} />);
    const paragraph = container.querySelector('p');
    expect(paragraph?.className).toContain('break-words');

    const codeWrapper = container.querySelector('.overflow-x-auto.max-w-full');
    expect(codeWrapper).not.toBeNull();
  });
});
