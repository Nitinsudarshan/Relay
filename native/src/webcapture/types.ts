/**
 * The capture payload contract, shared by the Relay browser extension and the
 * Rust backend (`native/src-tauri/src/capture/web/mod.rs`).
 *
 * Two rules make this contract safe to hand across a trust boundary:
 * text only — there is no field here that can carry HTML into Relay — and a
 * closed vocabulary of block types, so the receiving side can be a total
 * function over anything a page produces.
 *
 * Change this file and the Rust `WebCapturePayload` together, and bump
 * `PROTOCOL_VERSION` only for a breaking change: additive fields and new
 * block types degrade on their own (unknown blocks are skipped and counted).
 */

export const PROTOCOL_VERSION = 1;

export type ContentBlock =
  | { type: 'heading'; level: number; text: string }
  | { type: 'paragraph'; text: string }
  | { type: 'list'; ordered: boolean; items: string[] }
  | { type: 'code'; language?: string; text: string }
  | { type: 'quote'; text: string }
  | { type: 'table'; headers: string[]; rows: string[][] }
  | { type: 'image'; alt?: string; src?: string };

/** What the extractor believes it found. Relay decides the final capture type. */
export type CaptureContentKind = 'conversation' | 'article' | 'repository' | 'generic';

/**
 * How much of the page the extractor can honestly claim to have read.
 *
 * `full_document` requires positive evidence — never a default. Anything
 * derived from a live DOM without that evidence is `rendered_dom`, because
 * a DOM only ever holds what the page has chosen to render.
 */
export type CaptureCoverage = 'full_document' | 'rendered_dom' | 'partial' | 'unknown';

/** How the content was obtained. Relay maps this onto its fidelity ladder. */
export type ExtractionStrategy = 'site' | 'article' | 'text';

export interface CaptureMessage {
  role: string;
  blocks: ContentBlock[];
  timestamp?: string;
}

export interface DocumentMetadata {
  canonical_url?: string;
  site_name?: string;
  author?: string;
  published_at?: string;
  description?: string;
  language?: string;
}

export interface CapturedLink {
  text: string;
  href: string;
}

export interface CaptureDiagnostics {
  coverage: CaptureCoverage;
  notes: string[];
  dom_text_length?: number;
  truncated: boolean;
  elapsed_ms?: number;
}

/** What an extractor returns; the page-level payload is assembled around it. */
export interface ExtractionResult {
  kind: CaptureContentKind;
  strategy: ExtractionStrategy;
  extractorId: string;
  extractorVersion: number;
  blocks: ContentBlock[];
  messages: CaptureMessage[];
  coverage: CaptureCoverage;
  notes: string[];
  truncated: boolean;
  title?: string;
  /**
   * Links belonging to the content, not to the page around it. Set by
   * document-shaped extractors, which are the only ones that know where the
   * content ended and the navigation began.
   */
  links?: CapturedLink[];
}

export interface CapturePayload {
  protocol_version: number;
  captured_at: string;
  url: string;
  title: string;
  browser?: string;
  extractor: { id: string; version: number; strategy: ExtractionStrategy };
  document: DocumentMetadata;
  content: {
    kind: CaptureContentKind;
    blocks: ContentBlock[];
    messages: CaptureMessage[];
  };
  links: CapturedLink[];
  diagnostics: CaptureDiagnostics;
}

/**
 * A site-specific extractor.
 *
 * Extractors are independent modules registered in `extractors/index.ts`, not
 * branches inside one function: adding support for a new site is a new file
 * and a registry line, and a broken selector in one can never take another
 * down. `extract` returning `null` means "this is not my page after all" and
 * hands over to the next strategy, which is what keeps a site redesign a
 * degradation rather than a failure.
 */
export interface SiteExtractor {
  id: string;
  version: number;
  matches(url: URL): boolean;
  extract(doc: Document, url: URL): ExtractionResult | null;
}
