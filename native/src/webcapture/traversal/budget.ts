/**
 * The bounds a traversal runs inside.
 *
 * Every one of these exists to stop a specific failure: an unbounded loop on a
 * page that fights back, a capture that takes two minutes, a page with ten
 * thousand `<details>` turning a read into a clicking marathon, and a settle
 * that waits forever for content that is never coming.
 */
export interface TraversalBudget {
  maxSteps: number;
  /** Wall-clock ceiling for the whole traversal. */
  maxMs: number;
  maxExpansions: number;
  maxExpansionsPerStep: number;
  /** Ceiling on one settle window. Never a fixed sleep — see `settle.ts`. */
  settleMaxMs: number;
  settlePollMs: number;
  /** Consecutive unchanged polls that count as settled. */
  settleQuietTicks: number;
  /**
   * Fraction of a viewport to advance when the page exposes no measurable
   * items. Below 1 so consecutive windows overlap and nothing can be unmounted
   * in the gap between two reads.
   */
  overlap: number;
  /** How many times to re-seek the boundary while it keeps moving. */
  maxRewinds: number;
  /** Consecutive steps with no new content before giving up. */
  maxIdleSteps: number;
}

/**
 * The defaults, chosen against measured behaviour rather than taste.
 *
 * `maxMs` is 10s: a Chromium run over a 500-item virtualized list with
 * mounted-extent stepping completed in 83 steps and 2.8s, so 10s covers that
 * shape with room to spare while keeping a capture something you wait for
 * rather than schedule. A thread long enough to exhaust it is reported as
 * `time_budget` and `partial`, which is the honest outcome — not a two-minute
 * capture, and not a silent one.
 */
export const DEFAULT_BUDGET: TraversalBudget = {
  maxSteps: 300,
  maxMs: 10_000,
  maxExpansions: 120,
  maxExpansionsPerStep: 12,
  settleMaxMs: 1_200,
  // 25ms rather than 50: two quiet polls is the minimum cost of every settle,
  // and the settle happens at least once per step. At 50ms a 300-turn thread
  // spent 160ms a step and ran out of its 10s budget 45 turns short; at 25ms
  // the same walk finishes inside it. The ceiling above is what covers a page
  // where content genuinely keeps arriving.
  settlePollMs: 25,
  settleQuietTicks: 2,
  overlap: 0.75,
  maxRewinds: 6,
  maxIdleSteps: 3,
};

export function budget(overrides: Partial<TraversalBudget> = {}): TraversalBudget {
  return { ...DEFAULT_BUDGET, ...overrides };
}
