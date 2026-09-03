/**
 * Tests for reconstructing a document from overlapping samples.
 *
 * The failure modes here are quiet ones — a lost turn, a duplicated one, a
 * conversation in the wrong order — so each is asserted directly rather than
 * inferred from a payload looking plausible.
 */

import { describe, expect, it } from 'vitest';
import { BlockMerger, SampleMerger, fingerprint, weigh } from './merge';
import type { ContentBlock, HarvestedItem } from './types';

function para(text: string): ContentBlock {
  return { type: 'paragraph', text };
}

function item(overrides: Partial<HarvestedItem> & { blocks: ContentBlock[] }): HarvestedItem {
  const role = overrides.role ?? 'user';
  return {
    offset: 0,
    role,
    fingerprint: fingerprint(role, overrides.blocks),
    weight: weigh(overrides.blocks),
    ...overrides,
  };
}

describe('fingerprint', () => {
  it('separates two messages that open with the same sentence', () => {
    // The trap this exists to avoid: keying on a prefix collapses a redrafted
    // reply into the one before it, and the capture silently loses a turn.
    const a = [para('Here is the plan. We start with pricing.')];
    const b = [para('Here is the plan. We start with packaging.')];
    expect(fingerprint('assistant', a)).not.toBe(fingerprint('assistant', b));
  });

  it('separates identical text from different speakers', () => {
    expect(fingerprint('user', [para('ok')])).not.toBe(fingerprint('assistant', [para('ok')]));
  });

  it('matches the same content seen twice', () => {
    expect(fingerprint('user', [para('same')])).toBe(fingerprint('user', [para('same')]));
  });

  it('distinguishes two files with the same name from different places', () => {
    const one: ContentBlock = { type: 'attachment', name: 'report.csv', href: 'https://a.example/1' };
    const two: ContentBlock = { type: 'attachment', name: 'report.csv', href: 'https://b.example/2' };
    expect(fingerprint('assistant', [one])).not.toBe(fingerprint('assistant', [two]));
  });
});

describe('SampleMerger', () => {
  it('keeps one copy of a turn seen in several overlapping samples', () => {
    const merger = new SampleMerger();
    const a = item({ blocks: [para('first')], offset: 0 });
    const b = item({ blocks: [para('second')], role: 'assistant', offset: 200 });

    expect(merger.add([a, b])).toBe(2);
    expect(merger.add([b, item({ blocks: [para('third')], offset: 400 })])).toBe(1);

    const result = merger.result();
    expect(result.messages.map((m) => (m.blocks[0] as { text: string }).text)).toEqual([
      'first',
      'second',
      'third',
    ]);
    expect(result.duplicatesDropped).toBe(1);
  });

  it('prefers the richer version of a turn that was expanded between samples', () => {
    // The same message, identified by the page, seen truncated and then full.
    const merger = new SampleMerger();
    merger.add([item({ identity: 'm1', blocks: [para('short…')] })]);
    merger.add([item({ identity: 'm1', blocks: [para('the whole message, expanded')] })]);

    const result = merger.result();
    expect(result.messages).toHaveLength(1);
    expect((result.messages[0].blocks[0] as { text: string }).text).toBe(
      'the whole message, expanded',
    );
  });

  it('keeps a turn in its original position when a later sample replaces it', () => {
    const merger = new SampleMerger();
    merger.add([
      item({ identity: 'a', blocks: [para('first…')] }),
      item({ identity: 'b', blocks: [para('second')] }),
    ]);
    merger.add([item({ identity: 'a', blocks: [para('first, in full, at length')] })]);

    const texts = merger.result().messages.map((m) => (m.blocks[0] as { text: string }).text);
    expect(texts[0]).toMatch(/^first, in full/);
    expect(texts[1]).toBe('second');
  });

  it('orders by the page’s own turn numbers when every turn has one', () => {
    const merger = new SampleMerger();
    // Deliberately out of order: a virtualized list can hand back a window
    // that starts anywhere.
    merger.add([
      item({ ordinal: 5, blocks: [para('e')] }),
      item({ ordinal: 3, blocks: [para('c')] }),
      item({ ordinal: 4, blocks: [para('d')] }),
    ]);
    merger.add([item({ ordinal: 2, blocks: [para('b')] })]);

    const result = merger.result();
    expect(result.ordering).toBe('ordinal');
    expect(result.messages.map((m) => (m.blocks[0] as { text: string }).text)).toEqual([
      'b',
      'c',
      'd',
      'e',
    ]);
  });

  it('measures gaps in the page’s own numbering rather than guessing at them', () => {
    const merger = new SampleMerger();
    merger.add([
      item({ ordinal: 2, blocks: [para('b')] }),
      item({ ordinal: 3, blocks: [para('c')] }),
      item({ ordinal: 9, blocks: [para('i')] }),
    ]);

    // 2..9 is eight positions; three are present, so five are missing.
    expect(merger.result().missing).toBe(5);
  });

  it('reports no gaps for a contiguous run, and none at all without ordinals', () => {
    const contiguous = new SampleMerger();
    contiguous.add([item({ ordinal: 2, blocks: [para('b')] }), item({ ordinal: 3, blocks: [para('c')] })]);
    expect(contiguous.result().missing).toBe(0);

    const unnumbered = new SampleMerger();
    unnumbered.add([item({ blocks: [para('x')] })]);
    expect(unnumbered.result().missing).toBeUndefined();
  });

  it('falls back to position within the scroller when there are no ordinals', () => {
    const merger = new SampleMerger();
    merger.add([
      item({ offset: 900, blocks: [para('last')] }),
      item({ offset: 100, blocks: [para('first')] }),
    ]);

    const result = merger.result();
    expect(result.ordering).toBe('offset');
    expect(result.messages.map((m) => (m.blocks[0] as { text: string }).text)).toEqual([
      'first',
      'last',
    ]);
  });

  it('falls back to first-seen order when the page offers neither', () => {
    const merger = new SampleMerger();
    merger.add([item({ blocks: [para('one')] }), item({ blocks: [para('two')] })]);

    const result = merger.result();
    expect(result.ordering).toBe('first_seen');
    expect(result.messages.map((m) => (m.blocks[0] as { text: string }).text)).toEqual(['one', 'two']);
  });

  it('ignores ordinals the page repeats rather than trusting them', () => {
    // Duplicate ordinals mean the attribute is not what Relay thought it was;
    // falling back is safer than sorting a conversation by a broken key.
    const merger = new SampleMerger();
    merger.add([
      item({ ordinal: 1, offset: 10, blocks: [para('a')] }),
      item({ ordinal: 1, offset: 20, blocks: [para('b')] }),
    ]);
    expect(merger.result().ordering).toBe('offset');
  });

  it('reconstructs a 1,000-turn conversation from overlapping windows', () => {
    const merger = new SampleMerger();
    const total = 1_000;
    const windowSize = 12;

    for (let start = 0; start < total; start += windowSize - 4) {
      const window: HarvestedItem[] = [];
      for (let i = start; i < Math.min(total, start + windowSize); i += 1) {
        window.push(
          item({
            ordinal: i + 2,
            offset: i * 300,
            role: i % 2 === 0 ? 'user' : 'assistant',
            blocks: [para(`Turn ${i} of the conversation.`)],
          }),
        );
      }
      merger.add(window);
    }

    const result = merger.result();
    expect(result.messages).toHaveLength(total);
    expect(result.missing).toBe(0);
    expect(result.duplicatesDropped).toBeGreaterThan(0);
    expect((result.messages[0].blocks[0] as { text: string }).text).toBe('Turn 0 of the conversation.');
    expect((result.messages[total - 1].blocks[0] as { text: string }).text).toBe(
      `Turn ${total - 1} of the conversation.`,
    );
  });
});

describe('BlockMerger', () => {
  it('appends newly revealed blocks and keeps reading order', () => {
    const merger = new BlockMerger();
    merger.add([para('intro'), para('middle')]);
    merger.add([para('intro'), para('middle'), para('revealed by scrolling')]);

    const result = merger.result();
    expect(result.blocks.map((b) => (b as { text: string }).text)).toEqual([
      'intro',
      'middle',
      'revealed by scrolling',
    ]);
    expect(result.duplicatesDropped).toBe(2);
  });

  it('keeps two paragraphs that merely start alike', () => {
    const merger = new BlockMerger();
    merger.add([
      para('The pricing model has three tiers and a free trial.'),
      para('The pricing model has three tiers and no free trial.'),
    ]);
    expect(merger.result().blocks).toHaveLength(2);
  });
});
