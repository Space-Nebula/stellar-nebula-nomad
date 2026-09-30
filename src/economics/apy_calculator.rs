//! Pure APY and yield arithmetic shared by the staking and yield-farming
//! modules.
//!
//! Nothing here touches storage or needs a contract context: every function
//! is integer math over plain inputs, so the economics can be unit-tested on
//! their own and reproduced by off-chain simulators. The on-chain modules
//! (`staking`, `yield_farming`) call in here and only add persistence.
//!
//! # Duration tiers
//!
//! | Tier     | Lock    | APY  |
//! |----------|---------|------|
//! | `Days7`  | 7 days  |  5 % |
//! | `Days30` | 30 days | 15 % |
//! | `Days90` | 90 days | 35 % |
//!
//! Yield is simple (non-compounding) interest, pro-rated by the seconds the
//! position has been open, and always paid in the staked resource.
//!
//! # NFT staking
//!
//! A staked ship earns [`NFT_BASE_DAILY_YIELD`] units per day per level,
//! multiplied by a rarity factor in basis points (10 000 = common). A level 5
//! explorer (11 000 bps) therefore earns `10 * 5 * 1.1 = 55` units a day.
//!
//! # Emergency withdrawal
//!
//! Leaving a lock early forfeits [`EMERGENCY_PENALTY_BPS`] of the principal
//! (50 %). Accrued yield is forfeited entirely.
//!
//! # Impermanent loss protection
//!
//! Liquidity positions accrue protection linearly from 0 % at deposit to
//! 100 % after [`IL_PROTECTION_FULL_SECS`] (90 days). The protected fraction
//! of the divergence loss is compensated from pool fees at withdrawal.

use crate::traits::time::TimeProvider;
use soroban_sdk::contracttype;

/// Basis-point denominator used by every rate in this module.
pub const BPS_DENOMINATOR: u32 = 10_000;
/// Seconds in one day.
pub const SECONDS_PER_DAY: u64 = 86_400;
/// Seconds in a 365-day year, the APY reference period.
pub const SECONDS_PER_YEAR: u64 = 365 * SECONDS_PER_DAY;

/// APY of the 7-day tier (5 %).
pub const APY_7_DAYS_BPS: u32 = 500;
/// APY of the 30-day tier (15 %).
pub const APY_30_DAYS_BPS: u32 = 1_500;
/// APY of the 90-day tier (35 %).
pub const APY_90_DAYS_BPS: u32 = 3_500;

/// Principal forfeited on an emergency withdrawal (50 %).
pub const EMERGENCY_PENALTY_BPS: u32 = 5_000;

/// Units per day earned by a level-1, common-rarity staked ship.
pub const NFT_BASE_DAILY_YIELD: i128 = 10;

/// Seconds after which a liquidity position is fully protected against
/// impermanent loss (90 days).
pub const IL_PROTECTION_FULL_SECS: u64 = 90 * SECONDS_PER_DAY;

/// Floor applied by [`tvl_adjusted_apy_bps`]: the APY never drops below
/// 20 % of its base value however large the pool grows.
pub const TVL_APY_FLOOR_BPS: u32 = 2_000;

/// Fixed lock durations offered for resource staking.
#[contracttype]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LockTier {
    /// 7-day lock, 5 % APY.
    Days7,
    /// 30-day lock, 15 % APY.
    Days30,
    /// 90-day lock, 35 % APY.
    Days90,
}

impl LockTier {
    /// Lock length in days.
    pub const fn days(self) -> u64 {
        match self {
            Self::Days7 => 7,
            Self::Days30 => 30,
            Self::Days90 => 90,
        }
    }

    /// Lock length in seconds.
    pub const fn duration_secs(self) -> u64 {
        self.days() * SECONDS_PER_DAY
    }

    /// Annual percentage yield in basis points.
    pub const fn apy_bps(self) -> u32 {
        match self {
            Self::Days7 => APY_7_DAYS_BPS,
            Self::Days30 => APY_30_DAYS_BPS,
            Self::Days90 => APY_90_DAYS_BPS,
        }
    }

    /// Every tier, shortest first.
    pub const fn all() -> [LockTier; 3] {
        [Self::Days7, Self::Days30, Self::Days90]
    }
}

/// Map a requested lock length in days to a tier. Only the three offered
/// durations are accepted; anything else is `None`.
pub fn tier_for_days(days: u64) -> Option<LockTier> {
    match days {
        7 => Some(LockTier::Days7),
        30 => Some(LockTier::Days30),
        90 => Some(LockTier::Days90),
        _ => None,
    }
}

/// Simple interest earned by `principal` at `apy_bps` over `elapsed_secs`.
///
/// `principal * apy * elapsed / (10_000 * seconds_per_year)`, computed with
/// checked arithmetic. Returns `None` on overflow rather than wrapping so a
/// caller can surface `ArithmeticOverflow`. Negative principals yield `None`
/// too: there is no such thing as negative stake.
pub fn simple_yield(principal: i128, apy_bps: u32, elapsed_secs: u64) -> Option<i128> {
    if principal < 0 {
        return None;
    }
    let denominator = i128::from(BPS_DENOMINATOR).checked_mul(i128::from(SECONDS_PER_YEAR))?;
    principal
        .checked_mul(i128::from(apy_bps))?
        .checked_mul(i128::from(elapsed_secs))?
        .checked_div(denominator)
}

/// Yield accrued between `opened_at` and the provider's current time.
///
/// Clamps `elapsed` to `max_secs` so a position that has passed its unlock
/// time stops accruing at the tier's full duration: leaving a matured stake
/// unclaimed must not mint yield forever.
pub fn accrued_yield<T: TimeProvider>(
    time: &T,
    principal: i128,
    apy_bps: u32,
    opened_at: u64,
    max_secs: u64,
) -> Option<i128> {
    let elapsed = time.elapsed_since(opened_at).min(max_secs);
    simple_yield(principal, apy_bps, elapsed)
}

/// Scale a base APY down as pool TVL grows past `target_tvl`.
///
/// Below target the base rate applies unchanged. Above it the rate falls in
/// proportion (`base * target / tvl`) with a floor of 20 % of base, which is
/// the economic-model lever that keeps emissions bounded when a pool becomes
/// popular. A zero or negative target disables the adjustment.
pub fn tvl_adjusted_apy_bps(base_apy_bps: u32, tvl: i128, target_tvl: i128) -> u32 {
    if target_tvl <= 0 || tvl <= target_tvl {
        return base_apy_bps;
    }
    let scaled = i128::from(base_apy_bps)
        .saturating_mul(target_tvl)
        .checked_div(tvl)
        .unwrap_or(0);
    let floor = i128::from(base_apy_bps)
        .saturating_mul(i128::from(TVL_APY_FLOOR_BPS))
        .checked_div(i128::from(BPS_DENOMINATOR))
        .unwrap_or(0);
    let bounded = scaled.max(floor).min(i128::from(base_apy_bps));
    u32::try_from(bounded).unwrap_or(base_apy_bps)
}

/// Daily yield of a staked ship: `base * level * rarity / 10_000`.
///
/// Level 0 (unregistered ship) earns nothing.
pub fn nft_daily_yield(level: u32, rarity_bps: u32) -> i128 {
    NFT_BASE_DAILY_YIELD
        .saturating_mul(i128::from(level))
        .saturating_mul(i128::from(rarity_bps))
        / i128::from(BPS_DENOMINATOR)
}

/// NFT yield accrued over `elapsed_secs`, pro-rated by the second.
pub fn nft_accrued_yield(level: u32, rarity_bps: u32, elapsed_secs: u64) -> Option<i128> {
    nft_daily_yield(level, rarity_bps)
        .checked_mul(i128::from(elapsed_secs))?
        .checked_div(i128::from(SECONDS_PER_DAY))
}

/// Split `principal` into `(returned, penalty)` for an emergency withdrawal.
///
/// The penalty is rounded up so the staker can never keep more than 50 %.
pub fn emergency_withdrawal(principal: i128) -> (i128, i128) {
    if principal <= 0 {
        return (0, 0);
    }
    let numerator = principal.saturating_mul(i128::from(EMERGENCY_PENALTY_BPS));
    let denominator = i128::from(BPS_DENOMINATOR);
    let penalty = (numerator + denominator - 1) / denominator;
    (principal - penalty, penalty)
}

/// `total * part / whole`, the share of a reward pool owed to one
/// participant. Zero when `whole` is zero so an empty pool distributes
/// nothing instead of dividing by zero.
pub fn pro_rata_share(total: i128, part: i128, whole: i128) -> i128 {
    if whole <= 0 || part <= 0 || total <= 0 {
        return 0;
    }
    total.saturating_mul(part) / whole
}

/// Integer square root (floor).
pub fn isqrt(n: u128) -> u128 {
    if n < 2 {
        return n;
    }
    let mut x = n;
    let mut y = x.div_ceil(2);
    while y < x {
        x = y;
        y = (x + n / x) / 2;
    }
    x
}

/// Impermanent loss, in basis points of the position's value, for a price
/// ratio `r` (in bps, 10 000 = no change) between the two pooled assets.
///
/// Uses the constant-product divergence formula
/// `IL = 1 - 2*sqrt(r) / (1 + r)`.
pub fn impermanent_loss_bps(price_ratio_bps: u64) -> u32 {
    if price_ratio_bps == 0 {
        return BPS_DENOMINATOR;
    }
    let bps = u128::from(BPS_DENOMINATOR);
    let r = u128::from(price_ratio_bps);
    // sqrt(r / bps) scaled to bps: sqrt(r * bps).
    let sqrt_r = isqrt(r * bps);
    let held = bps + r;
    let lp = 2 * sqrt_r * bps / held;
    u32::try_from(bps.saturating_sub(lp)).unwrap_or(BPS_DENOMINATOR)
}

/// Fraction of impermanent loss covered for a position open `elapsed_secs`.
/// Linear from 0 to 100 % over [`IL_PROTECTION_FULL_SECS`].
pub fn il_protection_bps(elapsed_secs: u64) -> u32 {
    if elapsed_secs >= IL_PROTECTION_FULL_SECS {
        return BPS_DENOMINATOR;
    }
    let scaled = u128::from(elapsed_secs) * u128::from(BPS_DENOMINATOR)
        / u128::from(IL_PROTECTION_FULL_SECS);
    u32::try_from(scaled).unwrap_or(BPS_DENOMINATOR)
}

/// Impermanent loss that remains after protection, in bps.
pub fn unprotected_il_bps(price_ratio_bps: u64, elapsed_secs: u64) -> u32 {
    let il = u64::from(impermanent_loss_bps(price_ratio_bps));
    let uncovered = u64::from(BPS_DENOMINATOR) - u64::from(il_protection_bps(elapsed_secs));
    u32::try_from(il * uncovered / u64::from(BPS_DENOMINATOR)).unwrap_or(BPS_DENOMINATOR)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::traits::time::MockTimeProvider;

    #[test]
    fn tiers_expose_documented_durations_and_rates() {
        assert_eq!(LockTier::Days7.duration_secs(), 7 * SECONDS_PER_DAY);
        assert_eq!(LockTier::Days30.duration_secs(), 30 * SECONDS_PER_DAY);
        assert_eq!(LockTier::Days90.duration_secs(), 90 * SECONDS_PER_DAY);
        assert_eq!(LockTier::Days7.apy_bps(), 500);
        assert_eq!(LockTier::Days30.apy_bps(), 1_500);
        assert_eq!(LockTier::Days90.apy_bps(), 3_500);
        assert_eq!(tier_for_days(30), Some(LockTier::Days30));
        assert_eq!(tier_for_days(31), None);
        assert_eq!(LockTier::all().len(), 3);
    }

    #[test]
    fn simple_yield_matches_worked_examples() {
        // 10 000 units at 35 % for a full year = 3 500.
        assert_eq!(simple_yield(10_000, 3_500, SECONDS_PER_YEAR), Some(3_500));
        // 10 000 units at 5 % for 7 days = 10000 * 0.05 * 7/365 = 9.58 -> 9.
        assert_eq!(simple_yield(10_000, 500, 7 * SECONDS_PER_DAY), Some(9));
        // 100 000 at 15 % for 30 days = 1232.87 -> 1232.
        assert_eq!(
            simple_yield(100_000, 1_500, 30 * SECONDS_PER_DAY),
            Some(1_232)
        );
        assert_eq!(simple_yield(10_000, 500, 0), Some(0));
        assert_eq!(simple_yield(-1, 500, 10), None);
        assert_eq!(simple_yield(i128::MAX, 500, 10), None);
    }

    #[test]
    fn longer_locks_always_pay_more_for_the_same_principal() {
        let p = 50_000;
        let y7 = simple_yield(
            p,
            LockTier::Days7.apy_bps(),
            LockTier::Days7.duration_secs(),
        )
        .unwrap();
        let y30 = simple_yield(
            p,
            LockTier::Days30.apy_bps(),
            LockTier::Days30.duration_secs(),
        )
        .unwrap();
        let y90 = simple_yield(
            p,
            LockTier::Days90.apy_bps(),
            LockTier::Days90.duration_secs(),
        )
        .unwrap();
        assert!(y7 < y30 && y30 < y90, "{y7} {y30} {y90}");
    }

    #[test]
    fn accrual_stops_at_the_lock_duration() {
        let clock = MockTimeProvider::new(1_000, 1);
        let opened = 1_000;
        let tier = LockTier::Days7;
        clock.advance_days(3);
        let partial = accrued_yield(
            &clock,
            100_000,
            tier.apy_bps(),
            opened,
            tier.duration_secs(),
        );
        assert_eq!(partial, simple_yield(100_000, 500, 3 * SECONDS_PER_DAY));
        clock.advance_days(30);
        let capped = accrued_yield(
            &clock,
            100_000,
            tier.apy_bps(),
            opened,
            tier.duration_secs(),
        );
        assert_eq!(capped, simple_yield(100_000, 500, 7 * SECONDS_PER_DAY));
    }

    #[test]
    fn tvl_adjustment_scales_down_with_a_floor() {
        assert_eq!(tvl_adjusted_apy_bps(3_500, 1_000, 10_000), 3_500);
        assert_eq!(tvl_adjusted_apy_bps(3_500, 10_000, 10_000), 3_500);
        assert_eq!(tvl_adjusted_apy_bps(3_500, 20_000, 10_000), 1_750);
        assert_eq!(tvl_adjusted_apy_bps(3_500, 1_000_000, 10_000), 700);
        assert_eq!(tvl_adjusted_apy_bps(3_500, 1_000_000, 0), 3_500);
    }

    #[test]
    fn nft_yield_scales_with_level_and_rarity() {
        assert_eq!(nft_daily_yield(0, 10_000), 0);
        assert_eq!(nft_daily_yield(1, 10_000), 10);
        assert_eq!(nft_daily_yield(5, 11_000), 55);
        assert_eq!(nft_daily_yield(20, 12_000), 240);
        assert_eq!(nft_accrued_yield(5, 11_000, SECONDS_PER_DAY), Some(55));
        assert_eq!(nft_accrued_yield(5, 11_000, SECONDS_PER_DAY / 2), Some(27));
    }

    #[test]
    fn emergency_withdrawal_takes_half_rounded_against_the_staker() {
        assert_eq!(emergency_withdrawal(1_000), (500, 500));
        assert_eq!(emergency_withdrawal(1_001), (500, 501));
        assert_eq!(emergency_withdrawal(1), (0, 1));
        assert_eq!(emergency_withdrawal(0), (0, 0));
        assert_eq!(emergency_withdrawal(-5), (0, 0));
    }

    #[test]
    fn pro_rata_share_handles_empty_pools() {
        assert_eq!(pro_rata_share(1_000, 250, 1_000), 250);
        assert_eq!(pro_rata_share(1_000, 1, 3), 333);
        assert_eq!(pro_rata_share(1_000, 5, 0), 0);
        assert_eq!(pro_rata_share(0, 5, 10), 0);
    }

    #[test]
    fn isqrt_is_floor_sqrt() {
        assert_eq!(isqrt(0), 0);
        assert_eq!(isqrt(1), 1);
        assert_eq!(isqrt(15), 3);
        assert_eq!(isqrt(16), 4);
        assert_eq!(isqrt(1_000_000), 1_000);
        assert_eq!(isqrt(u128::from(u64::MAX)), 4_294_967_295);
    }

    #[test]
    fn impermanent_loss_matches_reference_values() {
        // No price change: no loss.
        assert_eq!(impermanent_loss_bps(10_000), 0);
        // 2x price move: 5.72 % divergence loss.
        assert_eq!(impermanent_loss_bps(20_000), 572);
        // 4x: 20 %.
        assert_eq!(impermanent_loss_bps(40_000), 2_000);
        // 0.5x is symmetric with 2x.
        assert_eq!(impermanent_loss_bps(5_000), 572);
        assert_eq!(impermanent_loss_bps(0), 10_000);
    }

    #[test]
    fn il_protection_vests_linearly_over_ninety_days() {
        assert_eq!(il_protection_bps(0), 0);
        assert_eq!(il_protection_bps(45 * SECONDS_PER_DAY), 5_000);
        assert_eq!(il_protection_bps(90 * SECONDS_PER_DAY), 10_000);
        assert_eq!(il_protection_bps(400 * SECONDS_PER_DAY), 10_000);
        assert_eq!(unprotected_il_bps(40_000, 0), 2_000);
        assert_eq!(unprotected_il_bps(40_000, 45 * SECONDS_PER_DAY), 1_000);
        assert_eq!(unprotected_il_bps(40_000, 90 * SECONDS_PER_DAY), 0);
    }
}
