/**
 * Bot detection helpers for the admin review dashboard (Issue #704).
 * Mirrors the signal bits, weights and thresholds in `src/bot_detection.rs`.
 */

export const BOT_SIGNALS = [
  { bit: 1 << 0, name: 'burst', weight: 20 },
  { bit: 1 << 1, name: 'rateLimit', weight: 10 },
  { bit: 1 << 2, name: 'regularTiming', weight: 15 },
  { bit: 1 << 3, name: 'repetitive', weight: 10 },
  { bit: 1 << 4, name: 'multiAccount', weight: 25 },
  { bit: 1 << 5, name: 'farming', weight: 40 },
  { bit: 1 << 6, name: 'captchaFail', weight: 15 },
  { bit: 1 << 7, name: 'manual', weight: 0 },
] as const;

export type BotSignalName = (typeof BOT_SIGNALS)[number]['name'];

export const MONITOR_THRESHOLD = 30;
export const CAPTCHA_THRESHOLD = 60;
export const BLOCK_THRESHOLD = 85;
export const MAX_SUSPICION = 100;

/** Mirrors `bot_detection::BotResponse`. */
export type BotResponse = 'Allow' | 'Monitor' | 'Captcha' | 'Block' | 'Ban';

export const decodeSignals = (mask: number): BotSignalName[] =>
  BOT_SIGNALS.filter((s) => (mask & s.bit) !== 0).map((s) => s.name);

export const encodeSignals = (names: BotSignalName[]): number =>
  BOT_SIGNALS.filter((s) => names.includes(s.name)).reduce((m, s) => m | s.bit, 0);

export const responseForScore = (score: number): BotResponse =>
  score >= BLOCK_THRESHOLD
    ? 'Block'
    : score >= CAPTCHA_THRESHOLD
      ? 'Captcha'
      : score >= MONITOR_THRESHOLD
        ? 'Monitor'
        : 'Allow';

/** Mirrors `bot_detection::CaseStatus`. */
export type CaseStatus = 'Open' | 'Cleared' | 'Banned';
export type CaseVerdict = 'Clear' | 'Ban';

/** Mirrors `bot_detection::ReviewCase` (timestamps in ms). */
export interface ReviewCaseView {
  id: number;
  player: string;
  peakScore: number;
  signals: number;
  openedAt: number;
  updatedAt: number;
  status: CaseStatus;
  /** Live score, if fetched (`get_suspicion_score`). */
  currentScore?: number;
}

export type CaseFilter = 'open' | 'resolved' | 'all';

/** Filter, then sort: highest score first, then oldest first. */
export const sortCases = (cases: ReviewCaseView[], filter: CaseFilter): ReviewCaseView[] =>
  cases
    .filter((c) =>
      filter === 'all' ? true : filter === 'open' ? c.status === 'Open' : c.status !== 'Open'
    )
    .sort(
      (a, b) =>
        (b.currentScore ?? b.peakScore) - (a.currentScore ?? a.peakScore) || a.openedAt - b.openedAt
    );
