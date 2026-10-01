//! Anti-whale mechanisms: diminishing returns, daily operation caps,
//! progressive trading fees, guild contribution limits and tiered
//! leaderboards.
//!
//! Large holders can out-farm, out-trade and out-rank everyone else, which
//! drives newer players away. This module bounds how much economic weight a
//! single address can exert per day without touching the experience of a
//! normal session. Every limit is a plain number the admin (or the governance
//! DAO, through `governance::set_game_parameter`) can retune; nothing here is
//! hard-coded into the callers. The full write-up, with the numbers behind
//! each default, is in `docs/economy/anti-whale.md`.
//!
//! # Mechanisms
//!
//! 1. **Diminishing returns on gathering.** Units harvested in a day are
//!    split into five bands of `tier_width` units paying 100 / 80 / 60 / 40
//!    / 20 %. The counter resets every UTC day.
//! 2. **Daily operation caps.** Scans, ship mints and trades are counted per
//!    player per day and refused past the cap.
//! 3. **Progressive trading fees.** An extra fee on swap output that rises
//!    with the trader's cumulative daily volume; the fee stays in the pool.
//! 4. **Guild contribution limit.** A member can put at most
//!    `guild_daily_cap` units into a treasury per day.
//! 5. **Tiered leaderboards.** Players are classified casual / dedicated /
//!    hardcore from cumulative activity and ranked against their own tier.
//!
//! All per-day state lives in persistent storage keyed by `(player,
//! day_index)` so entries from past days can expire without a sweep.
//!
//! # Dependency injection
//!
//! The arithmetic and the state transitions are written against
//! [`StorageProvider`] and [`TimeProvider`], so the unit tests below drive
//! them with in-memory mocks and a controllable clock. The `Env` entry points
//! are thin wrappers that construct the real providers.

use crate::traits::{RealStorageProvider, RealTimeProvider, StorageProvider, TimeProvider};
use soroban_sdk::{contracterror, contracttype, symbol_short, Address, Env, Symbol, Vec};

/// Timeframe in seconds for daily activity windows (24 hours).
pub const DAILY_WINDOW_SECONDS: u64 = 86_400;

/// Default cap on the legacy volume-based `process_anti_whale_action` path.
pub const DEFAULT_DAILY_CAP: u64 = 1_000_000;

/// Number of diminishing-returns bands.
pub const TIER_COUNT: usize = 5;

/// Multiplier paid in each diminishing-returns band, in basis points.
pub const TIER_MULTIPLIERS_BPS: [u64; TIER_COUNT] = [10_000, 8_000, 6_000, 4_000, 2_000];

/// Width of each diminishing-returns band in gathered units per day.
///
/// A four-hour session at the scan rate limit gathers roughly 114 000 units
/// across all assets, so every modelled play style (casual, regular,
/// hardcore) stays inside the first band at 100 %. A bot farming around the
/// clock (about 684 000 units) collects under 60 % of its raw output.
pub const DEFAULT_TIER_WIDTH: u64 = 120_000;

/// Default daily scan cap (five times a four-hour session).
pub const DEFAULT_MAX_SCANS_PER_DAY: u32 = 200;
/// Default daily ship-mint cap.
pub const DEFAULT_MAX_MINTS_PER_DAY: u32 = 20;
/// Default daily trade cap (swaps and limit orders).
pub const DEFAULT_MAX_TRADES_PER_DAY: u32 = 100;
/// Default per-member daily guild treasury contribution cap.
pub const DEFAULT_GUILD_DAILY_CAP: i128 = 100_000;
/// Activity points at which a player becomes "dedicated".
pub const DEFAULT_DEDICATED_THRESHOLD: u64 = 200;
/// Activity points at which a player becomes "hardcore".
pub const DEFAULT_HARDCORE_THRESHOLD: u64 = 2_000;

/// Progressive fee bands on cumulative daily trade volume: `(floor, bps)`.
/// Volume above each floor pays that many extra basis points.
pub const PROGRESSIVE_FEE_BANDS: [(u64, u64); 4] =
    [(0, 0), (50_000, 50), (250_000, 150), (1_000_000, 300)];

/// Legacy flat progressive fee retained for `process_anti_whale_action`.
pub const PROGRESSIVE_FEE_BPS: u64 = 500;
/// Volume above which the legacy flat progressive fee applies.
pub const LEGACY_FEE_THRESHOLD: u64 = 500_000;

/// Maximum entries kept per tiered leaderboard.
pub const MAX_TIER_BOARD_ENTRIES: u32 = 100;

/// Activity points awarded per scan.
pub const ACTIVITY_PER_SCAN: u64 = 1;
/// Activity points awarded per minted ship.
pub const ACTIVITY_PER_MINT: u64 = 5;
/// Activity points awarded per trade.
pub const ACTIVITY_PER_TRADE: u64 = 2;
/// Gathered units per activity point.
pub const UNITS_PER_ACTIVITY_POINT: u64 = 1_000;

/// Storage keys.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AntiWhaleKey {
    /// Legacy cumulative volume for (user, day_index).
    DailyVolume(Address, u64),
    /// Legacy daily volume cap.
    DailyCap,
    /// Whitelisted account (exempt from every mechanism).
    Exempt(Address),
    /// Admin allowed to retune limits. Prefixed: unit variants of different
    /// `#[contracttype]` key enums encode to the same host value, so a bare
    /// `Admin` would alias every other module's `Admin` key in instance
    /// storage.
    WhaleAdmin,
    /// Admin-set limits (governance parameters override individual fields).
    WhaleConfig,
    /// Units gathered by (user, day_index).
    DailyGathered(Address, u64),
    /// Operations of one kind by (user, kind, day_index).
    DailyOps(Address, OpKind, u64),
    /// Trade volume by (user, day_index).
    DailyTradeVolume(Address, u64),
    /// Guild treasury contributions by (user, day_index).
    DailyGuildContribution(Address, u64),
    /// Lifetime activity points.
    Activity(Address),
    /// Leaderboard for one player tier.
    TierBoard(PlayerTier),
    /// Lifetime totals used to monitor the mechanisms' impact.
    WhaleImpact,
}

/// Operations that carry a daily cap.
#[contracttype]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OpKind {
    /// Nebula scan.
    Scan,
    /// Ship mint (one per ship, batches count each ship).
    Mint,
    /// Swap or limit order.
    Trade,
}

/// Player power tiers for separate leaderboards.
#[contracttype]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PlayerTier {
    /// Below the dedicated threshold.
    Casual,
    /// Between the dedicated and hardcore thresholds.
    Dedicated,
    /// At or above the hardcore threshold.
    Hardcore,
}

/// Tunable limits. Every field has a governance override symbol (see
/// [`effective_config`]).
#[contracttype]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AntiWhaleConfig {
    /// Units per diminishing-returns band.
    pub tier_width: u64,
    /// Scans allowed per player per day.
    pub max_scans_per_day: u32,
    /// Ship mints allowed per player per day.
    pub max_mints_per_day: u32,
    /// Trades allowed per player per day.
    pub max_trades_per_day: u32,
    /// Guild treasury units a member may contribute per day.
    pub guild_daily_cap: i128,
    /// Activity points at which a player becomes dedicated.
    pub dedicated_threshold: u64,
    /// Activity points at which a player becomes hardcore.
    pub hardcore_threshold: u64,
}

impl AntiWhaleConfig {
    /// The shipped defaults.
    pub const fn defaults() -> Self {
        Self {
            tier_width: DEFAULT_TIER_WIDTH,
            max_scans_per_day: DEFAULT_MAX_SCANS_PER_DAY,
            max_mints_per_day: DEFAULT_MAX_MINTS_PER_DAY,
            max_trades_per_day: DEFAULT_MAX_TRADES_PER_DAY,
            guild_daily_cap: DEFAULT_GUILD_DAILY_CAP,
            dedicated_threshold: DEFAULT_DEDICATED_THRESHOLD,
            hardcore_threshold: DEFAULT_HARDCORE_THRESHOLD,
        }
    }

    /// Whether every limit is usable (non-zero, ordered thresholds).
    pub const fn is_valid(&self) -> bool {
        self.tier_width > 0
            && self.max_scans_per_day > 0
            && self.max_mints_per_day > 0
            && self.max_trades_per_day > 0
            && self.guild_daily_cap > 0
            && self.dedicated_threshold < self.hardcore_threshold
    }
}

impl Default for AntiWhaleConfig {
    fn default() -> Self {
        Self::defaults()
    }
}

/// One row of a tiered leaderboard.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TierEntry {
    /// Ranked player.
    pub player: Address,
    /// Best score submitted while in this tier.
    pub score: u64,
    /// Ledger timestamp of the last update.
    pub timestamp: u64,
}

/// Lifetime counters exposed for retention monitoring: how much each
/// mechanism actually removed.
///
/// Refusals (a capped scan, mint, trade or guild deposit) are not counted
/// here: a refused call fails the invocation, and Soroban rolls back every
/// write made during it, so an on-chain counter of failures can never
/// persist. Count them off-chain from the failed transactions' diagnostic
/// events (`Whale`/`capped`).
#[contracttype]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ImpactStats {
    /// Raw units submitted to the gathering path.
    pub units_gathered_raw: u64,
    /// Units credited after diminishing returns.
    pub units_gathered_effective: u64,
    /// Extra fee units collected by progressive fees.
    pub progressive_fees: u64,
}

/// Errors.
#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum AntiWhaleError {
    /// Requested amount exceeds the legacy daily volume cap.
    DailyCapExceeded = 300,
    /// Arithmetic overflow in volume or fee calculation.
    ArithmeticOverflow = 301,
    /// Amount must be greater than zero.
    InvalidAmount = 302,
    /// The player has used up today's allowance for this operation.
    OperationCapExceeded = 303,
    /// The member has reached today's guild contribution cap.
    GuildContributionCapExceeded = 304,
    /// A submitted config has a zero limit or misordered thresholds.
    InvalidConfig = 305,
    /// Caller is not the anti-whale admin.
    Unauthorized = 306,
    /// Admin already initialised.
    AlreadyInitialized = 307,
}

impl crate::error_standard::StandardContractError for AntiWhaleError {
    fn descriptor(self) -> crate::error_standard::ErrorDescriptor {
        use crate::error_standard::ErrorKind;
        let (kind, retryable) = match self {
            Self::DailyCapExceeded
            | Self::OperationCapExceeded
            | Self::GuildContributionCapExceeded => (ErrorKind::ResourceLimit, true),
            Self::ArithmeticOverflow => (ErrorKind::Internal, false),
            Self::InvalidAmount | Self::InvalidConfig => (ErrorKind::Validation, false),
            Self::Unauthorized => (ErrorKind::Authorization, false),
            Self::AlreadyInitialized => (ErrorKind::Conflict, false),
        };
        crate::error_standard::ErrorDescriptor {
            module: "anti_whale",
            code: self as u32,
            kind,
            retryable,
        }
    }
}

// ─── Pure arithmetic ─────────────────────────────────────────────────────────

/// Effective units credited for gathering `amount` more units today, given
/// `gathered_before` units already gathered, with bands of `tier_width`.
///
/// Each band is filled in turn at its multiplier; everything past the fifth
/// band pays the last multiplier (20 %). Integer division floors per band,
/// so the result never exceeds the exact curve. A zero `tier_width` disables
/// the mechanism.
pub fn diminishing_returns(gathered_before: u64, amount: u64, tier_width: u64) -> u64 {
    if amount == 0 {
        return 0;
    }
    if tier_width == 0 {
        return amount;
    }
    let mut remaining = amount;
    let mut position = gathered_before;
    let mut effective: u64 = 0;
    for (i, multiplier) in TIER_MULTIPLIERS_BPS.iter().enumerate() {
        let band_end = tier_width.saturating_mul(i as u64 + 1);
        let last = i + 1 == TIER_COUNT;
        let take = if last {
            remaining
        } else {
            remaining.min(band_end.saturating_sub(position))
        };
        if take > 0 {
            let credited = u128::from(take) * u128::from(*multiplier) / 10_000;
            effective = effective.saturating_add(u64::try_from(credited).unwrap_or(u64::MAX));
            remaining -= take;
            position = position.saturating_add(take);
        }
        if remaining == 0 {
            break;
        }
    }
    effective
}

/// Five-band curve at the default band width; kept under its historical
/// name for `process_anti_whale_action`.
pub fn calculate_diminishing_returns(volume_before: u64, amount: u64) -> u64 {
    diminishing_returns(volume_before, amount, DEFAULT_TIER_WIDTH)
}

/// Extra fee, in units, on a trade of `amount` by a player whose cumulative
/// daily volume is already `volume_before`. Marginal across
/// [`PROGRESSIVE_FEE_BANDS`]: only the part of the trade that lands in a
/// band pays that band's rate.
pub fn progressive_trade_fee(volume_before: u64, amount: u64) -> u64 {
    if amount == 0 {
        return 0;
    }
    let start = volume_before;
    let end = volume_before.saturating_add(amount);
    let mut fee: u128 = 0;
    for (i, (floor, bps)) in PROGRESSIVE_FEE_BANDS.iter().enumerate() {
        let ceiling = PROGRESSIVE_FEE_BANDS
            .get(i + 1)
            .map_or(u64::MAX, |(next_floor, _)| *next_floor);
        let lo = start.max(*floor);
        let hi = end.min(ceiling);
        if hi > lo {
            fee += u128::from(hi - lo) * u128::from(*bps) / 10_000;
        }
    }
    u64::try_from(fee).unwrap_or(u64::MAX)
}

/// Legacy flat fee above [`LEGACY_FEE_THRESHOLD`] used by
/// `process_anti_whale_action`.
pub fn calculate_progressive_fee(amount: u64) -> u64 {
    if amount <= LEGACY_FEE_THRESHOLD {
        0
    } else {
        let excess = amount - LEGACY_FEE_THRESHOLD;
        u64::try_from(u128::from(excess) * u128::from(PROGRESSIVE_FEE_BPS) / 10_000)
            .unwrap_or(u64::MAX)
    }
}

/// Classify `activity` points against the configured thresholds.
pub fn tier_for_activity(activity: u64, config: &AntiWhaleConfig) -> PlayerTier {
    if activity >= config.hardcore_threshold {
        PlayerTier::Hardcore
    } else if activity >= config.dedicated_threshold {
        PlayerTier::Dedicated
    } else {
        PlayerTier::Casual
    }
}

/// Daily cap for an operation kind under `config`.
pub fn cap_for(config: &AntiWhaleConfig, kind: OpKind) -> u32 {
    match kind {
        OpKind::Scan => config.max_scans_per_day,
        OpKind::Mint => config.max_mints_per_day,
        OpKind::Trade => config.max_trades_per_day,
    }
}

/// Activity points earned by one operation.
pub fn activity_for(kind: OpKind) -> u64 {
    match kind {
        OpKind::Scan => ACTIVITY_PER_SCAN,
        OpKind::Mint => ACTIVITY_PER_MINT,
        OpKind::Trade => ACTIVITY_PER_TRADE,
    }
}

// ─── Provider-generic state transitions ──────────────────────────────────────

/// Day index from a time provider.
pub fn day_index_with<T: TimeProvider>(time: &T) -> u64 {
    time.day_index()
}

/// Apply diminishing returns to `amount` gathered by `user` today and record
/// the raw units. Returns the effective units.
pub fn record_gathering_with<S: StorageProvider, T: TimeProvider>(
    store: &S,
    time: &T,
    user: &Address,
    amount: u64,
    tier_width: u64,
) -> Result<u64, AntiWhaleError> {
    if amount == 0 {
        return Ok(0);
    }
    let key = AntiWhaleKey::DailyGathered(user.clone(), day_index_with(time));
    let before: u64 = store.get(&key).unwrap_or(0);
    let after = before
        .checked_add(amount)
        .ok_or(AntiWhaleError::ArithmeticOverflow)?;
    store.set(&key, &after);
    Ok(diminishing_returns(before, amount, tier_width))
}

/// Count `count` operations of `kind` for `user` today, refusing the whole
/// batch if it would exceed `cap`. Returns the new daily count.
pub fn record_operation_with<S: StorageProvider, T: TimeProvider>(
    store: &S,
    time: &T,
    user: &Address,
    kind: OpKind,
    count: u32,
    cap: u32,
) -> Result<u32, AntiWhaleError> {
    if count == 0 {
        return Err(AntiWhaleError::InvalidAmount);
    }
    let key = AntiWhaleKey::DailyOps(user.clone(), kind, day_index_with(time));
    let before: u32 = store.get(&key).unwrap_or(0);
    let after = before
        .checked_add(count)
        .ok_or(AntiWhaleError::ArithmeticOverflow)?;
    if after > cap {
        return Err(AntiWhaleError::OperationCapExceeded);
    }
    store.set(&key, &after);
    Ok(after)
}

/// Record `amount` of trade volume for `user` today and return the
/// progressive fee owed on it.
pub fn record_trade_volume_with<S: StorageProvider, T: TimeProvider>(
    store: &S,
    time: &T,
    user: &Address,
    amount: u64,
) -> Result<u64, AntiWhaleError> {
    let key = AntiWhaleKey::DailyTradeVolume(user.clone(), day_index_with(time));
    let before: u64 = store.get(&key).unwrap_or(0);
    let after = before
        .checked_add(amount)
        .ok_or(AntiWhaleError::ArithmeticOverflow)?;
    store.set(&key, &after);
    Ok(progressive_trade_fee(before, amount))
}

/// Record a guild treasury contribution, refusing it if today's total for
/// `user` would exceed `cap`. Returns the new daily total.
pub fn record_guild_contribution_with<S: StorageProvider, T: TimeProvider>(
    store: &S,
    time: &T,
    user: &Address,
    amount: i128,
    cap: i128,
) -> Result<i128, AntiWhaleError> {
    if amount <= 0 {
        return Err(AntiWhaleError::InvalidAmount);
    }
    let key = AntiWhaleKey::DailyGuildContribution(user.clone(), day_index_with(time));
    let before: i128 = store.get(&key).unwrap_or(0);
    let after = before
        .checked_add(amount)
        .ok_or(AntiWhaleError::ArithmeticOverflow)?;
    if after > cap {
        return Err(AntiWhaleError::GuildContributionCapExceeded);
    }
    store.set(&key, &after);
    Ok(after)
}

/// Add `points` of lifetime activity to `user`; returns the new total.
pub fn add_activity_with<S: StorageProvider>(store: &S, user: &Address, points: u64) -> u64 {
    let key = AntiWhaleKey::Activity(user.clone());
    let before: u64 = store.get(&key).unwrap_or(0);
    let after = before.saturating_add(points);
    store.set(&key, &after);
    after
}

/// Lifetime activity points of `user`.
pub fn activity_with<S: StorageProvider>(store: &S, user: &Address) -> u64 {
    store
        .get(&AntiWhaleKey::Activity(user.clone()))
        .unwrap_or(0)
}

// ─── Env entry points ────────────────────────────────────────────────────────

fn persistent(env: &Env) -> RealStorageProvider<'_> {
    RealStorageProvider::persistent(env)
}

/// Calculate current day index from ledger timestamp.
pub fn get_day_index(env: &Env) -> u64 {
    day_index_with(&RealTimeProvider::new(env))
}

/// Governance parameter overriding `tier_width`.
pub const GOV_TIER_WIDTH: Symbol = symbol_short!("aw_width");
/// Governance parameter overriding `max_scans_per_day`.
pub const GOV_SCANS: Symbol = symbol_short!("aw_scans");
/// Governance parameter overriding `max_mints_per_day`.
pub const GOV_MINTS: Symbol = symbol_short!("aw_mints");
/// Governance parameter overriding `max_trades_per_day`.
pub const GOV_TRADES: Symbol = symbol_short!("aw_trades");
/// Governance parameter overriding `guild_daily_cap`.
pub const GOV_GUILD: Symbol = symbol_short!("aw_guild");
/// Governance parameter overriding `dedicated_threshold`.
pub const GOV_DEDICATED: Symbol = symbol_short!("aw_dedic");
/// Governance parameter overriding `hardcore_threshold`.
pub const GOV_HARDCORE: Symbol = symbol_short!("aw_hard");

fn gov_u64(env: &Env, key: Symbol, fallback: u64) -> u64 {
    crate::governance::get_game_parameter(env.clone(), key)
        .and_then(|v| u64::try_from(v).ok())
        .filter(|v| *v > 0)
        .unwrap_or(fallback)
}

fn gov_u32(env: &Env, key: Symbol, fallback: u32) -> u32 {
    crate::governance::get_game_parameter(env.clone(), key)
        .and_then(|v| u32::try_from(v).ok())
        .filter(|v| *v > 0)
        .unwrap_or(fallback)
}

fn gov_i128(env: &Env, key: Symbol, fallback: i128) -> i128 {
    crate::governance::get_game_parameter(env.clone(), key)
        .filter(|v| *v > 0)
        .unwrap_or(fallback)
}

/// Admin-set config without governance overrides (defaults if never set).
pub fn get_stored_config(env: &Env) -> AntiWhaleConfig {
    env.storage()
        .instance()
        .get(&AntiWhaleKey::WhaleConfig)
        .unwrap_or_else(AntiWhaleConfig::defaults)
}

/// The limits in force: admin config with any governance parameter
/// (`aw_width`, `aw_scans`, `aw_mints`, `aw_trades`, `aw_guild`, `aw_dedic`,
/// `aw_hard`) applied on top. Governance wins because the DAO is the higher
/// authority; an inconsistent override (thresholds out of order) falls back
/// to the stored config for the threshold pair.
pub fn effective_config(env: &Env) -> AntiWhaleConfig {
    let base = get_stored_config(env);
    let dedicated = gov_u64(env, GOV_DEDICATED, base.dedicated_threshold);
    let hardcore = gov_u64(env, GOV_HARDCORE, base.hardcore_threshold);
    let (dedicated, hardcore) = if dedicated < hardcore {
        (dedicated, hardcore)
    } else {
        (base.dedicated_threshold, base.hardcore_threshold)
    };
    AntiWhaleConfig {
        tier_width: gov_u64(env, GOV_TIER_WIDTH, base.tier_width),
        max_scans_per_day: gov_u32(env, GOV_SCANS, base.max_scans_per_day),
        max_mints_per_day: gov_u32(env, GOV_MINTS, base.max_mints_per_day),
        max_trades_per_day: gov_u32(env, GOV_TRADES, base.max_trades_per_day),
        guild_daily_cap: gov_i128(env, GOV_GUILD, base.guild_daily_cap),
        dedicated_threshold: dedicated,
        hardcore_threshold: hardcore,
    }
}

/// One-time admin registration. The admin can retune limits, set the legacy
/// cap and manage exemptions.
pub fn init_admin(env: &Env, admin: &Address) -> Result<(), AntiWhaleError> {
    if env.storage().instance().has(&AntiWhaleKey::WhaleAdmin) {
        return Err(AntiWhaleError::AlreadyInitialized);
    }
    admin.require_auth();
    env.storage()
        .instance()
        .set(&AntiWhaleKey::WhaleAdmin, admin);
    Ok(())
}

/// Registered admin, if any.
pub fn get_admin(env: &Env) -> Option<Address> {
    env.storage().instance().get(&AntiWhaleKey::WhaleAdmin)
}

/// Require `caller` to be the registered admin.
pub fn require_admin(env: &Env, caller: &Address) -> Result<(), AntiWhaleError> {
    caller.require_auth();
    match get_admin(env) {
        Some(admin) if admin == *caller => Ok(()),
        _ => Err(AntiWhaleError::Unauthorized),
    }
}

/// Replace the admin-set limits. Admin only.
pub fn set_config(
    env: &Env,
    caller: &Address,
    config: AntiWhaleConfig,
) -> Result<(), AntiWhaleError> {
    require_admin(env, caller)?;
    if !config.is_valid() {
        return Err(AntiWhaleError::InvalidConfig);
    }
    env.storage()
        .instance()
        .set(&AntiWhaleKey::WhaleConfig, &config);
    env.events().publish(
        (symbol_short!("Whale"), symbol_short!("config")),
        (
            config.tier_width,
            config.max_scans_per_day,
            config.max_mints_per_day,
            config.max_trades_per_day,
            config.guild_daily_cap,
        ),
    );
    Ok(())
}

/// Get the legacy daily volume cap.
pub fn get_daily_cap(env: &Env) -> u64 {
    env.storage()
        .instance()
        .get(&AntiWhaleKey::DailyCap)
        .unwrap_or(DEFAULT_DAILY_CAP)
}

/// Set the legacy daily volume cap. Admin only.
pub fn set_daily_cap(env: &Env, caller: &Address, cap: u64) -> Result<(), AntiWhaleError> {
    require_admin(env, caller)?;
    if cap == 0 {
        return Err(AntiWhaleError::InvalidConfig);
    }
    env.storage().instance().set(&AntiWhaleKey::DailyCap, &cap);
    Ok(())
}

/// Check if an account is exempt from anti-whale limits.
pub fn is_exempt(env: &Env, user: &Address) -> bool {
    env.storage()
        .persistent()
        .get(&AntiWhaleKey::Exempt(user.clone()))
        .unwrap_or(false)
}

/// Set exemption status for an account. Admin only.
pub fn set_exempt(
    env: &Env,
    caller: &Address,
    user: &Address,
    exempt: bool,
) -> Result<(), AntiWhaleError> {
    require_admin(env, caller)?;
    env.storage()
        .persistent()
        .set(&AntiWhaleKey::Exempt(user.clone()), &exempt);
    Ok(())
}

/// Legacy cumulative volume for user on the current day.
pub fn get_user_daily_volume(env: &Env, user: &Address) -> u64 {
    let day = get_day_index(env);
    env.storage()
        .persistent()
        .get(&AntiWhaleKey::DailyVolume(user.clone(), day))
        .unwrap_or(0)
}

/// Units gathered by `user` today, before diminishing returns.
pub fn get_user_daily_gathered(env: &Env, user: &Address) -> u64 {
    let day = get_day_index(env);
    persistent(env)
        .get(&AntiWhaleKey::DailyGathered(user.clone(), day))
        .unwrap_or(0)
}

/// Operations of `kind` performed by `user` today.
pub fn get_user_daily_ops(env: &Env, user: &Address, kind: OpKind) -> u32 {
    let day = get_day_index(env);
    persistent(env)
        .get(&AntiWhaleKey::DailyOps(user.clone(), kind, day))
        .unwrap_or(0)
}

/// Trade volume of `user` today.
pub fn get_user_daily_trade_volume(env: &Env, user: &Address) -> u64 {
    let day = get_day_index(env);
    persistent(env)
        .get(&AntiWhaleKey::DailyTradeVolume(user.clone(), day))
        .unwrap_or(0)
}

/// Guild contributions of `user` today.
pub fn get_user_daily_guild_contribution(env: &Env, user: &Address) -> i128 {
    let day = get_day_index(env);
    persistent(env)
        .get(&AntiWhaleKey::DailyGuildContribution(user.clone(), day))
        .unwrap_or(0)
}

fn impact(env: &Env) -> ImpactStats {
    env.storage()
        .instance()
        .get(&AntiWhaleKey::WhaleImpact)
        .unwrap_or_default()
}

fn update_impact(env: &Env, f: impl FnOnce(&mut ImpactStats)) {
    let mut stats = impact(env);
    f(&mut stats);
    env.storage()
        .instance()
        .set(&AntiWhaleKey::WhaleImpact, &stats);
}

/// Lifetime counters of how often each mechanism fired.
pub fn get_impact_stats(env: &Env) -> ImpactStats {
    impact(env)
}

/// Apply diminishing returns to `amount` units gathered by `user` and
/// return the units to credit. Exempt users are credited in full.
pub fn apply_gathering(env: &Env, user: &Address, amount: u64) -> Result<u64, AntiWhaleError> {
    if amount == 0 || is_exempt(env, user) {
        return Ok(amount);
    }
    let config = effective_config(env);
    let effective = record_gathering_with(
        &persistent(env),
        &RealTimeProvider::new(env),
        user,
        amount,
        config.tier_width,
    )?;
    add_activity_with(&persistent(env), user, amount / UNITS_PER_ACTIVITY_POINT);
    update_impact(env, |s| {
        s.units_gathered_raw = s.units_gathered_raw.saturating_add(amount);
        s.units_gathered_effective = s.units_gathered_effective.saturating_add(effective);
    });
    if effective < amount {
        env.events().publish(
            (symbol_short!("Whale"), symbol_short!("gather")),
            (user.clone(), amount, effective),
        );
    }
    Ok(effective)
}

/// Count `count` operations of `kind` for `user` today against the cap.
/// Exempt users are never capped but still accrue activity.
pub fn check_operation(
    env: &Env,
    user: &Address,
    kind: OpKind,
    count: u32,
) -> Result<(), AntiWhaleError> {
    if count == 0 {
        return Err(AntiWhaleError::InvalidAmount);
    }
    let store = persistent(env);
    if !is_exempt(env, user) {
        let cap = cap_for(&effective_config(env), kind);
        let result =
            record_operation_with(&store, &RealTimeProvider::new(env), user, kind, count, cap);
        if let Err(AntiWhaleError::OperationCapExceeded) = result {
            // Surfaces in the failed invocation's diagnostic events so the
            // client can tell a cap from any other refusal.
            env.events().publish(
                (symbol_short!("Whale"), symbol_short!("capped")),
                (user.clone(), kind, cap),
            );
        }
        result?;
    }
    add_activity_with(
        &store,
        user,
        activity_for(kind).saturating_mul(u64::from(count)),
    );
    Ok(())
}

/// Record trade volume and return the progressive fee owed. Exempt users
/// pay no progressive fee.
pub fn charge_progressive_fee(
    env: &Env,
    user: &Address,
    amount: u64,
) -> Result<u64, AntiWhaleError> {
    if is_exempt(env, user) {
        return Ok(0);
    }
    let fee =
        record_trade_volume_with(&persistent(env), &RealTimeProvider::new(env), user, amount)?;
    if fee > 0 {
        update_impact(env, |s| {
            s.progressive_fees = s.progressive_fees.saturating_add(fee);
        });
        env.events().publish(
            (symbol_short!("Whale"), symbol_short!("fee")),
            (user.clone(), amount, fee),
        );
    }
    Ok(fee)
}

/// Enforce the per-member daily guild contribution cap.
pub fn check_guild_contribution(
    env: &Env,
    user: &Address,
    amount: i128,
) -> Result<(), AntiWhaleError> {
    if is_exempt(env, user) {
        return Ok(());
    }
    let cap = effective_config(env).guild_daily_cap;
    record_guild_contribution_with(
        &persistent(env),
        &RealTimeProvider::new(env),
        user,
        amount,
        cap,
    )
    .map(|_| ())
}

/// Lifetime activity points of `user`.
pub fn get_player_activity(env: &Env, user: &Address) -> u64 {
    activity_with(&persistent(env), user)
}

/// Current tier of `user`.
pub fn get_player_tier(env: &Env, user: &Address) -> PlayerTier {
    tier_for_activity(get_player_activity(env, user), &effective_config(env))
}

/// Record `score` on the leaderboard of the player's current tier. Keeps the
/// best score per player, sorted descending, truncated to
/// [`MAX_TIER_BOARD_ENTRIES`]. Returns the tier the score landed in.
pub fn record_tier_score(env: &Env, player: &Address, score: u64) -> PlayerTier {
    let tier = get_player_tier(env, player);
    let key = AntiWhaleKey::TierBoard(tier);
    let mut entries: Vec<TierEntry> = env
        .storage()
        .persistent()
        .get(&key)
        .unwrap_or_else(|| Vec::new(env));
    let now = env.ledger().timestamp();

    let mut found = false;
    for i in 0..entries.len() {
        if let Some(mut entry) = entries.get(i) {
            if entry.player == *player {
                entry.score = entry.score.max(score);
                entry.timestamp = now;
                entries.set(i, entry);
                found = true;
                break;
            }
        }
    }
    if !found {
        entries.push_back(TierEntry {
            player: player.clone(),
            score,
            timestamp: now,
        });
    }

    // Insertion sort, descending by score; boards are capped at 100 rows.
    let n = entries.len();
    for i in 1..n {
        let mut j = i;
        while j > 0 {
            let prev = entries.get(j - 1).unwrap();
            let cur = entries.get(j).unwrap();
            if prev.score >= cur.score {
                break;
            }
            entries.set(j - 1, cur);
            entries.set(j, prev);
            j -= 1;
        }
    }
    while entries.len() > MAX_TIER_BOARD_ENTRIES {
        entries.pop_back();
    }
    env.storage().persistent().set(&key, &entries);
    tier
}

/// Top `limit` rows of the leaderboard for `tier`.
pub fn get_tier_leaderboard(env: &Env, tier: PlayerTier, limit: u32) -> Vec<TierEntry> {
    let entries: Vec<TierEntry> = env
        .storage()
        .persistent()
        .get(&AntiWhaleKey::TierBoard(tier))
        .unwrap_or_else(|| Vec::new(env));
    let mut out = Vec::new(env);
    for i in 0..entries.len().min(limit) {
        if let Some(e) = entries.get(i) {
            out.push_back(e);
        }
    }
    out
}

/// Legacy combined path: checks the daily volume cap, applies diminishing
/// returns and the flat progressive fee, and records the volume. Returns
/// `Ok((effective_amount, progressive_fee))`.
pub fn process_anti_whale_action(
    env: &Env,
    user: &Address,
    amount: u64,
) -> Result<(u64, u64), AntiWhaleError> {
    if amount == 0 {
        return Err(AntiWhaleError::InvalidAmount);
    }
    if is_exempt(env, user) {
        return Ok((amount, 0));
    }

    let day = get_day_index(env);
    let key = AntiWhaleKey::DailyVolume(user.clone(), day);
    let volume_before: u64 = env.storage().persistent().get(&key).unwrap_or(0);
    let new_volume = volume_before
        .checked_add(amount)
        .ok_or(AntiWhaleError::ArithmeticOverflow)?;
    if new_volume > get_daily_cap(env) {
        return Err(AntiWhaleError::DailyCapExceeded);
    }

    let effective_amount = calculate_diminishing_returns(volume_before, amount);
    let progressive_fee = calculate_progressive_fee(amount);
    env.storage().persistent().set(&key, &new_volume);

    if effective_amount < amount || progressive_fee > 0 {
        env.events().publish(
            (symbol_short!("Whale"), symbol_short!("scale")),
            (user.clone(), amount, effective_amount, progressive_fee),
        );
    }
    Ok((effective_amount, progressive_fee))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::traits::{MockStorageProvider, MockTimeProvider};
    use soroban_sdk::testutils::Address as _;

    fn cfg() -> AntiWhaleConfig {
        AntiWhaleConfig::defaults()
    }

    #[test]
    fn diminishing_returns_fill_five_bands_in_order() {
        let w = 100;
        assert_eq!(diminishing_returns(0, 50, w), 50);
        assert_eq!(diminishing_returns(0, 100, w), 100);
        assert_eq!(diminishing_returns(0, 200, w), 180);
        assert_eq!(diminishing_returns(0, 300, w), 240);
        assert_eq!(diminishing_returns(0, 400, w), 280);
        assert_eq!(diminishing_returns(0, 500, w), 300);
        // Past the fifth band everything pays 20 %.
        assert_eq!(diminishing_returns(0, 1_000, w), 400);
        // Starting mid-band continues from that position.
        assert_eq!(diminishing_returns(150, 100, w), 40 + 30);
        assert_eq!(diminishing_returns(10_000, 100, w), 20);
        assert_eq!(diminishing_returns(0, 0, w), 0);
        assert_eq!(diminishing_returns(0, 77, 0), 77);
    }

    #[test]
    fn default_band_width_spares_engaged_players_and_bites_bots() {
        let w = DEFAULT_TIER_WIDTH;
        // Casual (1h) and regular (2h) sessions at the scan rate limit.
        assert_eq!(diminishing_returns(0, 28_500, w), 28_500);
        assert_eq!(diminishing_returns(0, 57_000, w), 57_000);
        // Hardcore (4h) is untouched too.
        assert_eq!(diminishing_returns(0, 114_000, w), 114_000);
        // Around-the-clock bot: under 60 %.
        let bot = diminishing_returns(0, 684_000, w);
        assert!(bot * 100 / 684_000 < 60, "{bot}");
        // Two hardcore sessions' worth in one day already loses a fifth.
        let double = diminishing_returns(0, 228_000, w);
        assert!(double < 228_000 && double * 100 / 228_000 >= 80, "{double}");
    }

    #[test]
    fn progressive_fee_is_marginal_across_bands() {
        assert_eq!(progressive_trade_fee(0, 10_000), 0);
        assert_eq!(progressive_trade_fee(0, 50_000), 0);
        // 10 000 units in the 50 bps band.
        assert_eq!(progressive_trade_fee(0, 60_000), 50);
        // Whole second band: 200 000 * 0.5 % = 1 000.
        assert_eq!(progressive_trade_fee(50_000, 200_000), 1_000);
        // Straddling bands two and three.
        assert_eq!(progressive_trade_fee(240_000, 20_000), 50 + 150);
        // Deep in the top band.
        assert_eq!(progressive_trade_fee(5_000_000, 100_000), 3_000);
        assert_eq!(progressive_trade_fee(0, 0), 0);
        assert_eq!(calculate_progressive_fee(500_000), 0);
        assert_eq!(calculate_progressive_fee(600_000), 5_000);
    }

    #[test]
    fn tiers_follow_thresholds() {
        let c = cfg();
        assert_eq!(tier_for_activity(0, &c), PlayerTier::Casual);
        assert_eq!(tier_for_activity(199, &c), PlayerTier::Casual);
        assert_eq!(tier_for_activity(200, &c), PlayerTier::Dedicated);
        assert_eq!(tier_for_activity(1_999, &c), PlayerTier::Dedicated);
        assert_eq!(tier_for_activity(2_000, &c), PlayerTier::Hardcore);
        assert!(c.is_valid());
        let bad = AntiWhaleConfig {
            dedicated_threshold: 5,
            hardcore_threshold: 5,
            ..c
        };
        assert!(!bad.is_valid());
        assert!(!AntiWhaleConfig { tier_width: 0, ..c }.is_valid());
    }

    #[test]
    fn gathering_counter_resets_daily_with_mocked_clock() {
        let env = Env::default();
        let store = MockStorageProvider::new(&env);
        let clock = MockTimeProvider::new(0, 1);
        let user = Address::generate(&env);

        let first = record_gathering_with(&store, &clock, &user, 100, 100).unwrap();
        let second = record_gathering_with(&store, &clock, &user, 100, 100).unwrap();
        assert_eq!(first, 100);
        assert_eq!(second, 80);

        clock.advance_days(1);
        let fresh = record_gathering_with(&store, &clock, &user, 100, 100).unwrap();
        assert_eq!(fresh, 100);
        assert_eq!(store.len(), 2, "one counter per day");
    }

    #[test]
    fn operation_caps_refuse_the_batch_that_crosses_the_line() {
        let env = Env::default();
        let store = MockStorageProvider::new(&env);
        let clock = MockTimeProvider::default();
        let user = Address::generate(&env);

        assert_eq!(
            record_operation_with(&store, &clock, &user, OpKind::Mint, 15, 20),
            Ok(15)
        );
        assert_eq!(
            record_operation_with(&store, &clock, &user, OpKind::Mint, 6, 20),
            Err(AntiWhaleError::OperationCapExceeded)
        );
        // The refused batch did not consume allowance.
        assert_eq!(
            record_operation_with(&store, &clock, &user, OpKind::Mint, 5, 20),
            Ok(20)
        );
        // Other kinds are independent.
        assert_eq!(
            record_operation_with(&store, &clock, &user, OpKind::Scan, 1, 200),
            Ok(1)
        );
        assert_eq!(
            record_operation_with(&store, &clock, &user, OpKind::Scan, 0, 200),
            Err(AntiWhaleError::InvalidAmount)
        );
        clock.advance_days(1);
        assert_eq!(
            record_operation_with(&store, &clock, &user, OpKind::Mint, 1, 20),
            Ok(1)
        );
    }

    #[test]
    fn storage_read_failure_is_treated_as_a_fresh_counter() {
        let env = Env::default();
        let store = MockStorageProvider::new(&env);
        let clock = MockTimeProvider::default();
        let user = Address::generate(&env);
        record_operation_with(&store, &clock, &user, OpKind::Trade, 99, 100).unwrap();
        // A vanished counter must not brick the player: the next call starts
        // over instead of erroring.
        store.fail_next_reads(1);
        assert_eq!(
            record_operation_with(&store, &clock, &user, OpKind::Trade, 5, 100),
            Ok(5)
        );
    }

    #[test]
    fn trade_volume_and_guild_caps_accumulate_within_a_day() {
        let env = Env::default();
        let store = MockStorageProvider::new(&env);
        let clock = MockTimeProvider::default();
        let user = Address::generate(&env);

        assert_eq!(
            record_trade_volume_with(&store, &clock, &user, 50_000),
            Ok(0)
        );
        assert_eq!(
            record_trade_volume_with(&store, &clock, &user, 10_000),
            Ok(50)
        );

        assert_eq!(
            record_guild_contribution_with(&store, &clock, &user, 60_000, 100_000),
            Ok(60_000)
        );
        assert_eq!(
            record_guild_contribution_with(&store, &clock, &user, 50_000, 100_000),
            Err(AntiWhaleError::GuildContributionCapExceeded)
        );
        assert_eq!(
            record_guild_contribution_with(&store, &clock, &user, 0, 100_000),
            Err(AntiWhaleError::InvalidAmount)
        );
        assert_eq!(
            record_guild_contribution_with(&store, &clock, &user, 40_000, 100_000),
            Ok(100_000)
        );
    }

    #[test]
    fn activity_accumulates_and_saturates() {
        let env = Env::default();
        let store = MockStorageProvider::new(&env);
        let user = Address::generate(&env);
        assert_eq!(activity_with(&store, &user), 0);
        assert_eq!(add_activity_with(&store, &user, 150), 150);
        assert_eq!(add_activity_with(&store, &user, u64::MAX), u64::MAX);
        assert_eq!(activity_with(&store, &user), u64::MAX);
    }

    #[test]
    fn env_paths_enforce_caps_and_report_impact() {
        let env = Env::default();
        env.mock_all_auths();
        let id = env.register(crate::NebulaNomadContract, ());
        let user = Address::generate(&env);
        let admin = Address::generate(&env);
        let stranger = Address::generate(&env);
        // Each authorised call runs in its own frame: the test host rejects a
        // second `require_auth` for the same address inside one frame.
        let run = |f: &dyn Fn()| env.as_contract(&id, f);

        run(&|| init_admin(&env, &admin).unwrap());
        run(&|| {
            assert_eq!(
                init_admin(&env, &admin),
                Err(AntiWhaleError::AlreadyInitialized)
            );
        });
        run(&|| {
            assert_eq!(
                set_daily_cap(&env, &stranger, 5),
                Err(AntiWhaleError::Unauthorized)
            );
        });
        run(&|| {
            set_config(
                &env,
                &admin,
                AntiWhaleConfig {
                    max_scans_per_day: 2,
                    ..AntiWhaleConfig::defaults()
                },
            )
            .unwrap();
        });
        run(&|| {
            assert!(check_operation(&env, &user, OpKind::Scan, 1).is_ok());
            assert!(check_operation(&env, &user, OpKind::Scan, 1).is_ok());
            assert_eq!(
                check_operation(&env, &user, OpKind::Scan, 1),
                Err(AntiWhaleError::OperationCapExceeded)
            );
            assert_eq!(get_user_daily_ops(&env, &user, OpKind::Scan), 2);
            assert_eq!(get_player_activity(&env, &user), 2);
        });

        run(&|| set_exempt(&env, &admin, &user, true).unwrap());
        run(&|| {
            assert!(check_operation(&env, &user, OpKind::Scan, 50).is_ok());
            assert_eq!(apply_gathering(&env, &user, 1_000_000), Ok(1_000_000));
            assert_eq!(charge_progressive_fee(&env, &user, 5_000_000), Ok(0));
        });
        run(&|| set_exempt(&env, &admin, &user, false).unwrap());

        run(&|| {
            assert_eq!(charge_progressive_fee(&env, &user, 60_000), Ok(50));
            assert_eq!(get_user_daily_trade_volume(&env, &user), 60_000);
            assert_eq!(get_impact_stats(&env).progressive_fees, 50);

            assert_eq!(apply_gathering(&env, &user, 130_000), Ok(120_000 + 8_000));
            assert_eq!(get_user_daily_gathered(&env, &user), 130_000);

            assert_eq!(check_guild_contribution(&env, &user, 100_000), Ok(()));
            assert_eq!(
                check_guild_contribution(&env, &user, 1),
                Err(AntiWhaleError::GuildContributionCapExceeded)
            );
            assert_eq!(get_user_daily_guild_contribution(&env, &user), 100_000);

            let (eff, fee) = process_anti_whale_action(&env, &user, 130_000).unwrap();
            assert_eq!(eff, 120_000 + 8_000);
            assert_eq!(fee, 0);
        });
    }

    #[test]
    fn tier_boards_rank_within_a_tier_and_keep_best_scores() {
        let env = Env::default();
        let id = env.register(crate::NebulaNomadContract, ());
        env.as_contract(&id, || {
            let a = Address::generate(&env);
            let b = Address::generate(&env);
            let whale = Address::generate(&env);
            add_activity_with(&persistent(&env), &whale, 5_000);

            assert_eq!(record_tier_score(&env, &a, 10), PlayerTier::Casual);
            assert_eq!(record_tier_score(&env, &b, 30), PlayerTier::Casual);
            assert_eq!(record_tier_score(&env, &a, 5), PlayerTier::Casual);
            assert_eq!(record_tier_score(&env, &whale, 9_999), PlayerTier::Hardcore);

            let casual = get_tier_leaderboard(&env, PlayerTier::Casual, 10);
            assert_eq!(casual.len(), 2);
            assert_eq!(casual.get(0).unwrap().player, b);
            assert_eq!(casual.get(1).unwrap().score, 10, "best score kept");
            let hardcore = get_tier_leaderboard(&env, PlayerTier::Hardcore, 1);
            assert_eq!(hardcore.len(), 1);
            assert_eq!(hardcore.get(0).unwrap().player, whale);
            assert!(get_tier_leaderboard(&env, PlayerTier::Dedicated, 10).is_empty());
        });
    }

    #[test]
    fn governance_parameters_override_stored_config() {
        let env = Env::default();
        let id = env.register(crate::NebulaNomadContract, ());
        env.as_contract(&id, || {
            assert_eq!(effective_config(&env), AntiWhaleConfig::defaults());
            env.storage().instance().set(
                &crate::governance::GovernanceDataKey::GameParameter(GOV_SCANS),
                &7i128,
            );
            env.storage().instance().set(
                &crate::governance::GovernanceDataKey::GameParameter(GOV_HARDCORE),
                &1i128,
            );
            let c = effective_config(&env);
            assert_eq!(c.max_scans_per_day, 7);
            // Misordered threshold override is ignored as a pair.
            assert_eq!(c.dedicated_threshold, DEFAULT_DEDICATED_THRESHOLD);
            assert_eq!(c.hardcore_threshold, DEFAULT_HARDCORE_THRESHOLD);
        });
    }
}
