/**
 * Referral program helpers (Issue #701). Mirrors `src/referral_system.rs`.
 */

/** Mirrors `referral_system::DEFAULT_TIERS`. */
export const REFERRAL_TIERS = [
  { level: 0, name: 'bronze', minReferrals: 0, multiplierBps: 10_000 },
  { level: 1, name: 'silver', minReferrals: 5, multiplierBps: 12_000 },
  { level: 2, name: 'gold', minReferrals: 20, multiplierBps: 15_000 },
  { level: 3, name: 'platinum', minReferrals: 50, multiplierBps: 20_000 },
  { level: 4, name: 'diamond', minReferrals: 100, multiplierBps: 25_000 },
] as const;

export type ReferralTier = (typeof REFERRAL_TIERS)[number];
export type ReferralTierName = ReferralTier['name'];

export const REFERRAL_CODE_LENGTH = 8;
/** Code alphabet: no 0/O, 1/I/L or U. Mirrors `REFERRAL_CODE_ALPHABET[..30]`. */
export const REFERRAL_CODE_ALPHABET = '23456789ABCDEFGHJKMNPQRSTVWXYZ';

export const tierForCount = (count: number): ReferralTier => {
  let best: ReferralTier = REFERRAL_TIERS[0];
  for (const tier of REFERRAL_TIERS) if (count >= tier.minReferrals) best = tier;
  return best;
};

export const nextTier = (count: number): ReferralTier | undefined =>
  REFERRAL_TIERS.find((tier) => tier.minReferrals > count);

/** Progress (0–1) from the current tier's threshold to the next one. */
export const tierProgress = (count: number): number => {
  const current = tierForCount(count);
  const next = nextTier(count);
  if (!next) return 1;
  return (count - current.minReferrals) / (next.minReferrals - current.minReferrals);
};

/** Referrer payout for one claim: `base * multiplier / 10_000` (floored, as on-chain). */
export const referrerReward = (base: number, multiplierBps: number): number =>
  Math.floor((base * multiplierBps) / 10_000);

/**
 * Normalise what a player typed: uppercase and drop spaces and dashes. Codes
 * never contain look-alikes (0/O, 1/I/L), so those are rejected by
 * `isValidReferralCode` rather than guessed at.
 */
export const normalizeReferralCode = (input: string): string =>
  input.toUpperCase().replace(/[\s-]/g, '');

export const isValidReferralCode = (input: string): boolean => {
  const code = normalizeReferralCode(input);
  return (
    code.length === REFERRAL_CODE_LENGTH &&
    [...code].every((ch) => REFERRAL_CODE_ALPHABET.includes(ch))
  );
};

/** `ABCD2345` → `ABCD-2345` for display. */
export const formatReferralCode = (code: string): string => {
  const clean = normalizeReferralCode(code);
  return clean.length === REFERRAL_CODE_LENGTH ? `${clean.slice(0, 4)}-${clean.slice(4)}` : clean;
};

export const buildReferralLink = (baseUrl: string, code: string): string => {
  const url = new URL(baseUrl);
  url.searchParams.set('ref', normalizeReferralCode(code));
  return url.toString();
};

/** Read a referral code from a URL's `?ref=` parameter, if valid. */
export const referralCodeFromUrl = (href: string): string | undefined => {
  try {
    const code = new URL(href).searchParams.get('ref');
    return code && isValidReferralCode(code) ? normalizeReferralCode(code) : undefined;
  } catch {
    return undefined;
  }
};

/** Status of one referee, derived from the on-chain `Referral` record. */
export type RefereeStatus = 'pending' | 'claimable' | 'claimed';

export const refereeStatus = (r: { firstScanDone: boolean; claimed: boolean }): RefereeStatus =>
  r.claimed ? 'claimed' : r.firstScanDone ? 'claimable' : 'pending';
