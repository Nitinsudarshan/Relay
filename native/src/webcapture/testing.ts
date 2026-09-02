/**
 * Fixture helpers for the capture tests.
 *
 * Not part of the extension bundle — nothing under `extractors/` or the entry
 * points import it. It exists so every extractor test builds its document the
 * same way, from HTML that reads like the markup the real site ships.
 */

/** Parses fixture HTML into a Document, the way a page would be parsed. */
export function documentFrom(html: string): Document {
  return new DOMParser().parseFromString(html, 'text/html');
}

/** Wraps fixture body markup in a minimal page with a title and metadata. */
export function pageFrom(
  bodyHtml: string,
  options: { title?: string; head?: string; lang?: string } = {},
): Document {
  return documentFrom(
    `<!doctype html><html lang="${options.lang ?? 'en'}"><head>` +
      `<title>${options.title ?? 'Untitled'}</title>${options.head ?? ''}` +
      `</head><body>${bodyHtml}</body></html>`,
  );
}
