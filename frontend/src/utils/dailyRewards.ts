/**
 * Daily login reward maths (Issue #703). Mirrors `src/daily_rewards.rs` so the
 * client can draw the calendar and preview payouts without extra reads.
 */

export const CALENDAR_DAYS = 28;
export const BASE_ESSENCE = 50;
export const MILESTONE_MULTIPLIER = 4;
export const CYCLE_COMPLETE_MULTIPLIER = 10;
export const STREAK_BONUS_BPS_PER_DAY = 500;
export const MAX_STREAK_BONUS_BPS = 30_000;
export const DAY_MS = 86_400_000;

/** Mirrors `daily_rewards::RewardKind`. */
export type RewardKind =
  | 'Essence'
  | 'StellarDust'
  | 'DarkMatter'
  | 'ExoticMatter'
  | 'Energy'
  | 'CosmeticRoll';

/** Mirrors `daily_rewards::RewardEvent` (days converted to ms timestamps). */
export interface RewardEventView {
  id: number;
  name: string;
  /** Start of the first day, ms. */
  startsAt: number;
  /** End of the last day (exclusive), ms. */
  endsAt: number;
  multiplierBps: number;
  bonus: number;
}

export interface DailyRewardView {
  calendarDay: number;
  kind: RewardKind;
  baseAmount: number;
  amount: number;
  streakBonusBps: number;
  isMilestone: boolean;
  eventMultiplierBps: number;
  eventBonus: number;
}

export const isMilestoneDay = (day: number) => day % 7 === 0;

export const calendarKind = (day: number): RewardKind => {
  if (day === CALENDAR_DAYS) return 'CosmeticRoll';
  if (isMilestoneDay(day)) return 'ExoticMatter';
  switch (day % 6) {
    case 0:
      return 'Energy';
    case 1:
      return 'Essence';
    case 2:
      return 'StellarDust';
    case 3:
      return 'Essence';
    case 4:
      return 'DarkMatter';
    default:
      return 'Energy';
  }
};

const baseAmount = (day: number) =>
  day === CALENDAR_DAYS
    ? BASE_ESSENCE * CYCLE_COMPLETE_MULTIPLIER
    : isMilestoneDay(day)
      ? BASE_ESSENCE * MILESTONE_MULTIPLIER
      : BASE_ESSENCE;

export const streakBonusBps = (streak: number) =>
  Math.min(Math.max(streak - 1, 0) * STREAK_BONUS_BPS_PER_DAY, MAX_STREAK_BONUS_BPS);

/** `resolve_reward` + `apply_event`, with the same integer flooring. */
export const resolveReward = (
  calendarDay: number,
  streak: number,
  event?: Pick<RewardEventView, 'multiplierBps' | 'bonus'>
): DailyRewardView => {
  if (calendarDay < 1 || calendarDay > CALENDAR_DAYS) {
    throw new RangeError(`calendar day ${calendarDay} out of range`);
  }
  const base = baseAmount(calendarDay);
  const bonusBps = streakBonusBps(streak);
  let amount = Math.floor((base * (10_000 + bonusBps)) / 10_000);
  const eventMultiplierBps = event?.multiplierBps ?? 10_000;
  const eventBonus = event?.bonus ?? 0;
  amount = Math.floor((amount * eventMultiplierBps) / 10_000) + eventBonus;
  return {
    calendarDay,
    kind: calendarKind(calendarDay),
    baseAmount: base,
    amount,
    streakBonusBps: bonusBps,
    isMilestone: isMilestoneDay(calendarDay),
    eventMultiplierBps,
    eventBonus,
  };
};

export type CalendarSlotState = 'claimed' | 'next' | 'upcoming';

export interface CalendarSlot extends DailyRewardView {
  state: CalendarSlotState;
}

/**
 * The 28-slot month view. Slots before `nextCalendarDay` are claimed this
 * cycle; upcoming slots are previewed at the streak the player would have on
 * that day if they keep logging in (event applied only to the next claim).
 */
export const buildCalendar = (
  nextCalendarDay: number,
  streakIfClaimed: number,
  event?: Pick<RewardEventView, 'multiplierBps' | 'bonus'>
): CalendarSlot[] =>
  Array.from({ length: CALENDAR_DAYS }, (_, i) => {
    const day = i + 1;
    if (day < nextCalendarDay) {
      return { ...resolveReward(day, 1), state: 'claimed' as const };
    }
    const offset = day - nextCalendarDay;
    const reward = resolveReward(day, streakIfClaimed + offset, offset === 0 ? event : undefined);
    return { ...reward, state: offset === 0 ? ('next' as const) : ('upcoming' as const) };
  });

/** Split a duration into hours and minutes (rounded up to the minute). */
export const splitDuration = (ms: number): { hours: number; minutes: number } => {
  const totalMinutes = Math.max(0, Math.ceil(ms / 60_000));
  return { hours: Math.floor(totalMinutes / 60), minutes: totalMinutes % 60 };
};
