/**
 * The capture payload contract, shared by the Relay browser extension and the
 * Rust backend (`native/src-tauri/src/capture/web/mod.rs`).
 *
 * Three rules make this contract safe to hand across a trust boundary:
 * text only — there is no field here that can carry HTML, markup or binary
 * content into Relay — a closed vocabulary of block types, so the receiving
 * side can be a total function over anything a page produces — and nothing
 * the page says about itself is authoritative: Relay re-derives the source,
 * the capture type and the trust level from the URL and the content.
 *
 * Change this file and the Rust `WebCapturePayload` together. `PROTOCOL_VERSION`
 * is bumped only for a *breaking* change: additive fields and new block types
 * degrade on their own (unknown fields take their defaults, unknown blocks are
 * skipped and counted), so a newer extension against an older Relay and an
 * older extension against a newer Relay both survive.
 */

import type { TraversalPlan } from './traversal/types';

export const PROTOCOL_VERSION = 1;

/** Where a file came from, relative to the conversation that referenced it. */
export type AttachmentKind = 'user_upload' | 'assistant_generated' | 'linked' | 'unknown';

/** Where an image came from. Distinct from its content, which is never fetched. */
export type ImageOrigin = 'user_upload' | 'assistant_generated' | 'page' | 'unknown';

/**
 * The ordered content vocabulary.
 *
 * `attachment` and `image` are deliberately blocks rather than side-tables:
 * blocks already carry reading order and already sit inside the message they
 * belong to, so association and ordering come for free and cannot drift from
 * the content.
 *
 * Neither carries file bytes. `content_captured` is the honest field — it says
 * whether the referenced thing itself was acquired, and in this version the
 * answer is always `false`, with `content_note` saying why in plain language.
 */
export type ContentBlock =
  | { type: 'heading'; level: number; text: string }
  | { type: 'paragraph'; text: string }
  | { type: 'list'; ordered: boolean; items: string[] }
  | { type: 'code'; language?: string; text: string }
  | { type: 'quote'; text: string }
  | { type: 'table'; headers: string[]; rows: string[][] }
  | {
      type: 'image';
      alt?: string;
      caption?: string;
      /** An ordinary `http(s)` reference. Anything else goes in `reference`. */
      src?: string;
      /** A `blob:`, `data:` or site-internal reference: evidence, not a target. */
      reference?: string;
      width?: number;
      height?: number;
      origin?: ImageOrigin;
      content_captured?: boolean;
      content_note?: string;
    }
  | {
      type: 'attachment';
      name?: string;
      /** MIME type or the site's own type label, whichever was exposed. */
      mime?: string;
      size_bytes?: number;
      /** An ordinary `http(s)` reference. */
      href?: string;
      /** An opaque site reference such as `sandbox:/mnt/data/report.csv`. */
      reference?: string;
      kind?: AttachmentKind;
      /** Any text the file card itself showed — a snippet, a row count. */
      preview?: string;
      content_captured?: boolean;
      content_note?: string;
    };

/** What the extractor believes it found. Relay decides the final capture type. */
export type CaptureContentKind = 'conversation' | 'article' | 'repository' | 'generic';

/**
 * How much of the page the capture can honestly claim to have read.
 *
 * The product's four completeness states, in the vocabulary that is persisted
 * on artifacts:
 *
 * | State | Value | Means |
 * |---|---|---|
 * | FULL | `full_document` | Positive evidence the whole thing was seen |
 * | PARTIAL | `partial` | Content demonstrably missing |
 * | LOADED_ONLY | `rendered_dom` | Only what was available; completeness unproven |
 * | FAILED | `failed` | Traversal errored or was cut short mid-way |
 * | — | `unknown` | Nothing measurable |
 *
 * `full_document` requires positive evidence and is never a default. It is
 * also never reachable by the generic extractor on a page Relay has a site
 * extractor for: if the site extractor did not win, the page is not the shape
 * Relay understands, and that is evidence *against* completeness, not for it.
 */
export type CaptureCoverage =
  | 'full_document'
  | 'rendered_dom'
  | 'partial'
  | 'failed'
  | 'unknown';

/** How the content was obtained. Relay maps this onto its fidelity ladder. */
export type ExtractionStrategy = 'site' | 'article' | 'text';

/**
 * Why content that exists was not simply sitting in the DOM.
 *
 * These are kept apart because they call for different — and differently
 * invasive — answers, and conflating them is what produces either a thin
 * capture or a browser agent:
 *
 * - `outside_viewport` — present in the DOM, just not on screen. Extract it.
 *   No interaction needed, and none is performed.
 * - `visually_truncated` — present in the DOM in full, clipped by CSS
 *   (`-webkit-line-clamp`, `max-height` with `overflow: hidden`). Extract it
 *   directly. **Do not click the control**: the text is already here, and
 *   clicking it buys nothing but a side effect on someone's page.
 * - `collapsed` — genuinely absent until a disclosure interaction. Controlled
 *   expansion is warranted, and only here.
 * - `not_loaded` — not in the DOM yet; loads on approach or on demand.
 *   Traversal is warranted.
 * - `virtualized` — only a moving window is mounted. Traversal is required,
 *   and content must be harvested while it is mounted.
 * - `inaccessible` — the browser legitimately cannot reach it (a side panel
 *   Relay will not open, a cross-origin frame, a privileged page). Reported,
 *   never bypassed.
 */
export type ContentAvailability =
  | 'outside_viewport'
  | 'visually_truncated'
  | 'collapsed'
  | 'not_loaded'
  | 'virtualized'
  | 'inaccessible';

export const CONTENT_AVAILABILITY_STATES: ContentAvailability[] = [
  'outside_viewport',
  'visually_truncated',
  'collapsed',
  'not_loaded',
  'virtualized',
  'inaccessible',
];

/** How many elements were observed in each availability state. Counted, never estimated. */
export type AvailabilityCounts = Record<ContentAvailability, number>;

export function emptyAvailabilityCounts(): AvailabilityCounts {
  return {
    outside_viewport: 0,
    visually_truncated: 0,
    collapsed: 0,
    not_loaded: 0,
    virtualized: 0,
    inaccessible: 0,
  };
}

/** Why the traversal loop stopped. `reached_end` is the only one that supports a FULL claim. */
export type TraversalTermination =
  | 'not_needed'
  | 'reached_end'
  | 'no_progress'
  | 'step_budget'
  | 'time_budget'
  | 'expansion_budget'
  | 'user_interrupted'
  | 'navigation_detected'
  | 'error';

/**
 * What the reveal pass actually did, in numbers.
 *
 * Every field here is counted or absent — none is inferred. The pairs matter
 * most: 340 messages discovered and 340 captured is a different artifact from
 * 340 discovered and 90 captured, and v1 could not tell them apart.
 */
export interface TraversalDiagnostics {
  /** False when traversal did not run at all, in which case the rest is zero. */
  performed: boolean;
  /** Which traversal plan ran. */
  plan: string;
  termination: TraversalTermination;
  steps: number;
  samples: number;
  /** How far the surface was moved, in CSS pixels. */
  scroll_span_px: number;
  duration_ms: number;
  /** Whether the user's scroll position was put back where it was. */
  scroll_restored: boolean;
  /** Whether anything on the page reported itself as virtualized. */
  virtualized: boolean;
  /** Settle windows that hit their ceiling rather than going quiet. */
  settle_timeouts: number;

  expansions_found: number;
  /** Passed the classifier and were activated. */
  expansions_opened: number;
  /** Rejected by the classifier — a deny-list hit, chrome, or a navigating target. */
  expansions_refused: number;
  /** Activated, but nothing changed. The early warning that a site redesigned. */
  expansions_failed: number;
  /** Content already present in full, so no click was performed. */
  expansions_unnecessary: number;

  messages_discovered: number;
  messages_captured: number;
  /** Gaps in an explicit turn ordinal. Absent when the page exposes no ordinal. */
  messages_missing?: number;
  duplicates_dropped: number;

  attachments_discovered: number;
  attachments_captured: number;
  images_discovered: number;
  images_captured: number;

  availability: AvailabilityCounts;
  /** Named content the browser could not reach, in plain sentences. */
  inaccessible: string[];
}

export function emptyTraversalDiagnostics(plan = 'none'): TraversalDiagnostics {
  return {
    performed: false,
    plan,
    termination: 'not_needed',
    steps: 0,
    samples: 0,
    scroll_span_px: 0,
    duration_ms: 0,
    scroll_restored: true,
    virtualized: false,
    settle_timeouts: 0,
    expansions_found: 0,
    expansions_opened: 0,
    expansions_refused: 0,
    expansions_failed: 0,
    expansions_unnecessary: 0,
    messages_discovered: 0,
    messages_captured: 0,
    duplicates_dropped: 0,
    attachments_discovered: 0,
    attachments_captured: 0,
    images_discovered: 0,
    images_captured: 0,
    availability: emptyAvailabilityCounts(),
    inaccessible: [],
  };
}

export interface CaptureMessage {
  role: string;
  blocks: ContentBlock[];
  timestamp?: string;
  /**
   * The page's own turn ordinal, where it exposes one (ChatGPT's
   * `conversation-turn-N`). Used to order merged samples and to detect gaps;
   * absent rather than invented.
   */
  ordinal?: number;
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
  traversal?: TraversalDiagnostics;
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
 *
 * `harvest` is the one point where extraction meets traversal, and it exists
 * so a site's selectors are written once rather than twice: it answers "what
 * is mounted *right now*", the reveal engine calls it after every settle, and
 * `extract` is that same harvest assembled into a result. The engine itself
 * knows nothing about messages or roles.
 */
export interface SiteExtractor {
  id: string;
  version: number;
  matches(url: URL): boolean;
  /**
   * How to reveal this source's content, when it needs revealing. Shipped
   * beside the extractor so one file holds one site's knowledge, and optional
   * because a source may need no traversal at all.
   */
  traversal?: TraversalPlan;
  extract(doc: Document, url: URL): ExtractionResult | null;
  harvest?(doc: Document, url: URL, offsets?: OffsetSource): HarvestedItem[];
}

/**
 * Anything that can say where an element sits in a scrolling coordinate space.
 *
 * Declared structurally rather than by importing the traversal engine's
 * `ScrollSurface`: an extractor needs one number from the surface and should
 * not gain a dependency on the thing that moves the page.
 */
export interface OffsetSource {
  offsetOf(el: Element): number;
}

/**
 * One piece of content, as seen in one sample.
 *
 * Carries everything the merge step needs to recognise it again in a later
 * sample and to put it back in the right place: a stable identity where the
 * page provides one, the page's own ordinal where it exposes one, and the
 * item's offset within the scrolling surface as the fallback ordering key.
 */
export interface HarvestedItem {
  /** Stable page identity (`data-message-id`), when there is one. */
  identity?: string;
  ordinal?: number;
  /** Offset within the scrolling surface, in CSS pixels. */
  offset: number;
  role?: string;
  blocks: ContentBlock[];
  timestamp?: string;
  /** Hash of the whole normalized text — never a prefix, which collapses distinct items. */
  fingerprint: string;
  /** Total text length, used to prefer the richer of two versions of one item. */
  weight: number;
}
