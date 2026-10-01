//! Ship progression model: level cost curves, psychological pricing and a
//! deterministic time-to-level simulator.
//!
//! Pure integer math with no storage access, so the same code prices
//! upgrades on-chain (`ship_upgrade::level_upgrade_cost`) and drives the
//! balance tests and off-chain calculators. The reasoning behind every
//! constant is in `docs/adr/011-ship-progression-balance.md`.
//!
//! # Income model
//!
//! Resource income is anchored to the live rate limits and generator odds:
//! `rate_limiter::default_nebula_scan` allows [`SCANS_PER_HOUR`] scans and a
//! 16x16 layout yields [`UNITS_PER_SCAN`] units of any one common asset on
//! average, so an active hour is worth [`UNITS_PER_HOUR`] units. Play styles
//! differ only in hours per day.
//!
//! # Retention targets
//!
//! | Style    | Hours/day | Level 5 | Level 10 | Level 20      |
//! |----------|-----------|---------|----------|---------------|
//! | Casual   | 1         | 7 days  | 30 days  | months        |
//! | Regular  | 2         | 3 days  | 14 days  | months        |
//! | Hardcore | 4         | 1 day   | 7 days   | at least 60 d |
//!
//! [`ProgressionCurve::default_rebalanced`] (exponential, base 1 449, +35 %
//! per level) meets every row; see [`meets_retention_targets`].

use soroban_sdk::contracttype;

/// Highest ship level.
pub const MAX_LEVEL: u32 = 20;
/// Basis-point denominator.
pub const BPS: u64 = 10_000;
/// Scans per hour permitted by the nebula-scan rate limit.
pub const SCANS_PER_HOUR: u64 = 10;
/// Expected units of one common asset harvested from a single 16x16 scan.
///
/// Derived from the generator odds: 15 % of 256 cells are asteroids (ore)
/// with energy `5 + 0..9`, giving `256 * 0.15 * 9.5 = 364.8`.
pub const UNITS_PER_SCAN: u64 = 365;
/// Units of one asset an active hour is worth.
pub const UNITS_PER_HOUR: u64 = SCANS_PER_HOUR * UNITS_PER_SCAN;

/// Base cost (level 1) of the rebalanced curve.
pub const DEFAULT_BASE_COST: u64 = 1_449;
/// Per-level growth of the rebalanced curve, in basis points (+35 %).
pub const DEFAULT_GROWTH_BPS: u64 = 3_500;

/// Below this raw price the 49/99 rounding is skipped: rounding 30 up to 49
/// would be a 63 % markup, which is not "psychological", it is a tax.
pub const PSYCHOLOGICAL_PRICING_FLOOR: u64 = 49;

/// `log2(level) * 256` for levels 0..=20, used by the logarithmic curve.
const LOG2_Q8: [u64; 21] = [
    0, 0, 256, 406, 512, 594, 662, 719, 768, 811, 850, 886, 918, 947, 974, 1000, 1024, 1047, 1067,
    1087, 1105,
];

/// Shape of a level cost curve.
#[contracttype]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CurveKind {
    /// `base * (1 + growth * (level - 1))`.
    Linear,
    /// `base * (1 + growth) ^ (level - 1)`.
    Exponential,
    /// `base * (1 + growth * log2(level))`.
    Logarithmic,
}

/// Daily play intensity used by the simulator.
#[contracttype]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PlayStyle {
    /// One hour a day.
    Casual,
    /// Two hours a day.
    Regular,
    /// Four hours a day.
    Hardcore,
}

impl PlayStyle {
    /// Active hours per day.
    pub const fn hours_per_day(self) -> u64 {
        match self {
            Self::Casual => 1,
            Self::Regular => 2,
            Self::Hardcore => 4,
        }
    }

    /// Units of one asset earned per day.
    pub const fn daily_income(self) -> u64 {
        self.hours_per_day() * UNITS_PER_HOUR
    }

    /// Every style, least intense first.
    pub const fn all() -> [PlayStyle; 3] {
        [Self::Casual, Self::Regular, Self::Hardcore]
    }
}

/// A level cost curve. Stored on-chain by `ship_upgrade` so the admin can
/// retune the ladder without redeploying.
#[contracttype]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProgressionCurve {
    /// Curve shape.
    pub kind: CurveKind,
    /// Cost of the first level, in units of the upgrade asset.
    pub base_cost: u64,
    /// Per-level growth in basis points; its meaning depends on `kind`.
    pub growth_bps: u64,
}

impl ProgressionCurve {
    /// The rebalanced default: exponential, base 1 449, +35 % per level.
    pub const fn default_rebalanced() -> Self {
        Self {
            kind: CurveKind::Exponential,
            base_cost: DEFAULT_BASE_COST,
            growth_bps: DEFAULT_GROWTH_BPS,
        }
    }

    /// Same base and growth with a different shape, for curve comparisons.
    pub const fn with_kind(self, kind: CurveKind) -> Self {
        Self { kind, ..self }
    }

    /// Whether the curve can be priced without degenerate values.
    pub const fn is_valid(&self) -> bool {
        self.base_cost > 0 && self.growth_bps <= 10 * BPS
    }
}

impl Default for ProgressionCurve {
    fn default() -> Self {
        Self::default_rebalanced()
    }
}

/// Snap a price to the nearest value ending in 49 or 99.
///
/// Prices below [`PSYCHOLOGICAL_PRICING_FLOOR`] are returned unchanged. Ties
/// round down, so the snapped price is never more than 25 units above the
/// raw curve and on average sits slightly below it.
pub fn psychological_price(raw: u64) -> u64 {
    if raw < PSYCHOLOGICAL_PRICING_FLOOR {
        return raw;
    }
    let hundreds = raw / 100;
    let remainder = raw % 100;
    let lower = if remainder >= 99 {
        hundreds * 100 + 99
    } else if remainder >= 49 {
        hundreds * 100 + 49
    } else {
        (hundreds - 1) * 100 + 99
    };
    let upper = lower + 50;
    if raw - lower <= upper - raw {
        lower
    } else {
        upper
    }
}

/// Unrounded cost of reaching `level` from `level - 1`. Zero outside
/// `1..=MAX_LEVEL`.
pub fn raw_level_cost(curve: &ProgressionCurve, level: u32) -> u64 {
    if level == 0 || level > MAX_LEVEL || !curve.is_valid() {
        return 0;
    }
    let steps = u64::from(level - 1);
    match curve.kind {
        CurveKind::Linear => {
            let increment = curve.base_cost.saturating_mul(curve.growth_bps) / BPS;
            curve
                .base_cost
                .saturating_add(increment.saturating_mul(steps))
        }
        CurveKind::Exponential => {
            let multiplier = BPS.saturating_add(curve.growth_bps);
            let mut cost = curve.base_cost;
            for _ in 0..steps {
                cost = cost.saturating_mul(multiplier) / BPS;
            }
            cost
        }
        CurveKind::Logarithmic => {
            let log_term = curve.growth_bps.saturating_mul(LOG2_Q8[level as usize]) / 256;
            curve.base_cost.saturating_mul(BPS.saturating_add(log_term)) / BPS
        }
    }
}

/// Price of reaching `level`, after psychological rounding.
pub fn level_cost(curve: &ProgressionCurve, level: u32) -> u64 {
    psychological_price(raw_level_cost(curve, level))
}

/// Total spend to climb from level 0 to `level`.
pub fn cumulative_cost(curve: &ProgressionCurve, level: u32) -> u64 {
    let top = level.min(MAX_LEVEL);
    (1..=top).fold(0u64, |acc, l| acc.saturating_add(level_cost(curve, l)))
}

/// Whole days a player of `style` needs to bank the cumulative cost of
/// `level`, rounding up. Zero for level 0.
pub fn days_to_level(curve: &ProgressionCurve, style: PlayStyle, level: u32) -> u64 {
    let total = cumulative_cost(curve, level);
    let income = style.daily_income();
    if total == 0 || income == 0 {
        return 0;
    }
    total.div_ceil(income)
}

/// Highest level a player of `style` can afford after `days` of play.
pub fn level_after_days(curve: &ProgressionCurve, style: PlayStyle, days: u64) -> u32 {
    let banked = style.daily_income().saturating_mul(days);
    let mut level = 0;
    while level < MAX_LEVEL && cumulative_cost(curve, level + 1) <= banked {
        level += 1;
    }
    level
}

/// Days-to-level for every style and milestone, in one struct so a frontend
/// or a test can show the whole picture from a single call.
#[contracttype]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProgressionReport {
    /// Days for a casual player to reach level 5.
    pub casual_level_5: u64,
    /// Days for a casual player to reach level 10.
    pub casual_level_10: u64,
    /// Days for a regular player to reach level 5.
    pub regular_level_5: u64,
    /// Days for a regular player to reach level 10.
    pub regular_level_10: u64,
    /// Days for a hardcore player to reach level 5.
    pub hardcore_level_5: u64,
    /// Days for a hardcore player to reach level 10.
    pub hardcore_level_10: u64,
    /// Days for a hardcore player to reach the maximum level.
    pub hardcore_max_level: u64,
    /// Total units spent climbing to the maximum level.
    pub total_cost_to_max: u64,
}

/// Run the simulator for every play style.
pub fn simulate(curve: &ProgressionCurve) -> ProgressionReport {
    ProgressionReport {
        casual_level_5: days_to_level(curve, PlayStyle::Casual, 5),
        casual_level_10: days_to_level(curve, PlayStyle::Casual, 10),
        regular_level_5: days_to_level(curve, PlayStyle::Regular, 5),
        regular_level_10: days_to_level(curve, PlayStyle::Regular, 10),
        hardcore_level_5: days_to_level(curve, PlayStyle::Hardcore, 5),
        hardcore_level_10: days_to_level(curve, PlayStyle::Hardcore, 10),
        hardcore_max_level: days_to_level(curve, PlayStyle::Hardcore, MAX_LEVEL),
        total_cost_to_max: cumulative_cost(curve, MAX_LEVEL),
    }
}

/// Upper bound, in days, on each milestone from the retention targets, plus
/// the lower bound on the hardcore max-level grind.
pub const TARGET_CASUAL_LEVEL_5_DAYS: u64 = 7;
/// Casual players should hit level 10 within a month.
pub const TARGET_CASUAL_LEVEL_10_DAYS: u64 = 30;
/// Regular players should hit level 5 within three days.
pub const TARGET_REGULAR_LEVEL_5_DAYS: u64 = 3;
/// Regular players should hit level 10 within two weeks.
pub const TARGET_REGULAR_LEVEL_10_DAYS: u64 = 14;
/// Hardcore players should hit level 5 on day one.
pub const TARGET_HARDCORE_LEVEL_5_DAYS: u64 = 1;
/// Hardcore players should hit level 10 within a week.
pub const TARGET_HARDCORE_LEVEL_10_DAYS: u64 = 7;
/// Even hardcore players need at least two months to max out.
pub const TARGET_HARDCORE_MAX_LEVEL_MIN_DAYS: u64 = 60;

/// Whether `curve` satisfies every retention target.
pub fn meets_retention_targets(curve: &ProgressionCurve) -> bool {
    let r = simulate(curve);
    r.casual_level_5 <= TARGET_CASUAL_LEVEL_5_DAYS
        && r.casual_level_10 <= TARGET_CASUAL_LEVEL_10_DAYS
        && r.regular_level_5 <= TARGET_REGULAR_LEVEL_5_DAYS
        && r.regular_level_10 <= TARGET_REGULAR_LEVEL_10_DAYS
        && r.hardcore_level_5 <= TARGET_HARDCORE_LEVEL_5_DAYS
        && r.hardcore_level_10 <= TARGET_HARDCORE_LEVEL_10_DAYS
        && r.hardcore_max_level >= TARGET_HARDCORE_MAX_LEVEL_MIN_DAYS
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn psychological_pricing_snaps_to_49_and_99() {
        assert_eq!(psychological_price(0), 0);
        assert_eq!(psychological_price(30), 30);
        assert_eq!(psychological_price(49), 49);
        assert_eq!(psychological_price(60), 49);
        assert_eq!(psychological_price(74), 49);
        assert_eq!(psychological_price(75), 99);
        assert_eq!(psychological_price(100), 99);
        assert_eq!(psychological_price(120), 99);
        assert_eq!(psychological_price(125), 149);
        assert_eq!(psychological_price(200), 199);
        assert_eq!(psychological_price(1_449), 1_449);
        assert_eq!(psychological_price(1_956), 1_949);
        assert_eq!(psychological_price(1_975), 1_999);
    }

    #[test]
    fn snapped_price_is_within_25_units_of_raw() {
        for raw in 49..5_000u64 {
            let p = psychological_price(raw);
            assert!(p % 100 == 49 || p % 100 == 99, "{raw} -> {p}");
            assert!(p.abs_diff(raw) <= 25, "{raw} -> {p}");
        }
    }

    #[test]
    fn rebalanced_ladder_matches_the_adr_table() {
        let curve = ProgressionCurve::default_rebalanced();
        let expected: [u64; 20] = [
            1_449, 1_949, 2_649, 3_549, 4_799, 6_499, 8_749, 11_849, 15_999, 21_549, 29_099,
            39_299, 53_049, 71_649, 96_699, 130_549, 176_249, 237_899, 321_199, 433_599,
        ];
        for (i, want) in expected.iter().enumerate() {
            let level = u32::try_from(i + 1).unwrap();
            assert_eq!(level_cost(&curve, level), *want, "level {level}");
        }
        assert_eq!(cumulative_cost(&curve, 5), 14_395);
        assert_eq!(cumulative_cost(&curve, 10), 79_040);
        assert_eq!(cumulative_cost(&curve, MAX_LEVEL), 1_668_330);
        assert_eq!(level_cost(&curve, 0), 0);
        assert_eq!(level_cost(&curve, MAX_LEVEL + 1), 0);
    }

    #[test]
    fn rebalanced_curve_meets_every_retention_target() {
        let curve = ProgressionCurve::default_rebalanced();
        let r = simulate(&curve);
        assert_eq!(r.casual_level_5, 4);
        assert_eq!(r.casual_level_10, 22);
        assert_eq!(r.regular_level_5, 2);
        assert_eq!(r.regular_level_10, 11);
        assert_eq!(r.hardcore_level_5, 1);
        assert_eq!(r.hardcore_level_10, 6);
        assert_eq!(r.hardcore_max_level, 115);
        assert!(meets_retention_targets(&curve));
    }

    #[test]
    fn linear_and_logarithmic_curves_fail_the_max_level_target() {
        let base = ProgressionCurve::default_rebalanced();
        let linear = base.with_kind(CurveKind::Linear);
        let log = base.with_kind(CurveKind::Logarithmic);
        // Both shapes grow too slowly: a hardcore player maxes out in days.
        assert!(simulate(&linear).hardcore_max_level < TARGET_HARDCORE_MAX_LEVEL_MIN_DAYS);
        assert!(simulate(&log).hardcore_max_level < TARGET_HARDCORE_MAX_LEVEL_MIN_DAYS);
        assert!(!meets_retention_targets(&linear));
        assert!(!meets_retention_targets(&log));
        // Scaling their base up to fix the max-level grind breaks the early
        // milestones instead: the shape, not the scale, is the problem.
        let steep_linear = ProgressionCurve {
            base_cost: 15_000,
            ..linear
        };
        assert!(simulate(&steep_linear).hardcore_level_5 > TARGET_HARDCORE_LEVEL_5_DAYS);
        assert!(!meets_retention_targets(&steep_linear));
    }

    #[test]
    fn level_after_days_is_the_inverse_of_days_to_level() {
        let curve = ProgressionCurve::default_rebalanced();
        for style in PlayStyle::all() {
            for level in 1..=MAX_LEVEL {
                let days = days_to_level(&curve, style, level);
                assert!(level_after_days(&curve, style, days) >= level);
                if days > 1 {
                    assert!(level_after_days(&curve, style, days - 1) < level);
                }
            }
        }
        assert_eq!(level_after_days(&curve, PlayStyle::Casual, 0), 0);
        assert_eq!(
            level_after_days(&curve, PlayStyle::Hardcore, 10_000),
            MAX_LEVEL
        );
    }

    #[test]
    fn invalid_curves_price_at_zero() {
        let zero_base = ProgressionCurve {
            base_cost: 0,
            ..ProgressionCurve::default_rebalanced()
        };
        assert!(!zero_base.is_valid());
        assert_eq!(level_cost(&zero_base, 3), 0);
        let absurd_growth = ProgressionCurve {
            growth_bps: 11 * BPS,
            ..ProgressionCurve::default_rebalanced()
        };
        assert!(!absurd_growth.is_valid());
        assert_eq!(cumulative_cost(&absurd_growth, MAX_LEVEL), 0);
    }

    #[test]
    fn exponential_curve_never_overflows_at_max_growth() {
        let curve = ProgressionCurve {
            kind: CurveKind::Exponential,
            base_cost: u64::MAX / 2,
            growth_bps: 10 * BPS,
        };
        // Each step saturates before the division, so the cost settles at
        // `u64::MAX / BPS` instead of wrapping.
        assert_eq!(raw_level_cost(&curve, MAX_LEVEL), u64::MAX / BPS);
        assert!(cumulative_cost(&curve, MAX_LEVEL) > u64::MAX / 2);
    }
}
