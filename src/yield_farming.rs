use soroban_sdk::{contracterror, contracttype, symbol_short, Address, Env, Vec};

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum FarmError {
    LockNotMet = 1,
    InsufficientBalance = 2,
    InvalidPool = 3,
    WhaleCapExceeded = 4,
    ArithmeticOverflow = 5,
    PoolNotActive = 6,
    RewardNotReady = 7,
    /// The yield reserve for this asset cannot cover the claim.
    YieldReserveEmpty = 8,
    /// Caller does not own the ship or position.
    NotOwner = 9,
    /// The ship or LP position is already staked.
    AlreadyStaked = 10,
    /// No such stake exists.
    NotStaked = 11,
    /// Requested lock length is not one of the offered tiers.
    InvalidTier = 12,
}

impl crate::error_standard::StandardContractError for FarmError {
    fn descriptor(self) -> crate::error_standard::ErrorDescriptor {
        use crate::error_standard::ErrorKind;
        let (kind, retryable) = match self {
            Self::LockNotMet | Self::RewardNotReady => (ErrorKind::Conflict, true),
            Self::InsufficientBalance | Self::WhaleCapExceeded | Self::ArithmeticOverflow => {
                (ErrorKind::ResourceLimit, false)
            }
            Self::InvalidPool | Self::InvalidTier => (ErrorKind::Validation, false),
            Self::PoolNotActive | Self::AlreadyStaked => (ErrorKind::Conflict, false),
            Self::YieldReserveEmpty => (ErrorKind::ResourceLimit, true),
            Self::NotOwner => (ErrorKind::Authorization, false),
            Self::NotStaked => (ErrorKind::NotFound, false),
        };
        crate::error_standard::ErrorDescriptor {
            module: "yield_farming",
            code: self as u32,
            kind,
            retryable,
        }
    }
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RewardSchedule {
    Linear,
    Accelerated,
    Stepwise(Vec<u64>),
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FarmPoolV2 {
    pub id: u64,
    pub owner: Address,
    pub amount: i128,
    pub lock_period: u32,
    pub start_time: u64,
    pub last_harvest: u64,
    pub apy_bps: u32,
    pub reward_schedule: RewardSchedule,
    pub compound_enabled: bool,
    pub penalty_bps: u32,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FarmPool {
    pub id: u64,
    pub owner: Address,
    pub amount: i128,
    pub lock_period: u32,
    pub start_time: u64,
    pub last_harvest: u64,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct YieldFarmStats {
    pub total_value_locked: i128,
    pub active_pools: u32,
    pub total_rewards_distributed: i128,
    pub average_apy_bps: u32,
}

const SECONDS_IN_YEAR: u64 = 31_536_000;
const BASE_APY_BPS: i128 = 1500;
const WHALE_CAP: i128 = 1_000_000_000_000;
const BPS_DENOMINATOR: i128 = 10_000;

fn calculate_farm_reward(amount: i128, elapsed: u64, apy_bps: i128) -> Option<i128> {
    amount
        .checked_mul(apy_bps)?
        .checked_mul(elapsed as i128)?
        .checked_div(BPS_DENOMINATOR.checked_mul(SECONDS_IN_YEAR as i128)?)
}

pub fn deposit_to_pool(
    env: Env,
    owner: Address,
    amount: i128,
    lock_period: u32,
) -> Result<u64, FarmError> {
    owner.require_auth();

    if amount > WHALE_CAP {
        return Err(FarmError::WhaleCapExceeded);
    }

    let pool_id = env
        .storage()
        .instance()
        .get::<_, u64>(&symbol_short!("next_pid"))
        .unwrap_or(0);

    let pool = FarmPool {
        id: pool_id,
        owner: owner.clone(),
        amount,
        lock_period,
        start_time: env.ledger().timestamp(),
        last_harvest: env.ledger().timestamp(),
    };

    env.storage().persistent().set(&pool_id, &pool);
    let next_pool_id = pool_id
        .checked_add(1)
        .ok_or(FarmError::ArithmeticOverflow)?;
    env.storage()
        .instance()
        .set(&symbol_short!("next_pid"), &next_pool_id);

    env.events().publish(
        (symbol_short!("farm"), symbol_short!("deposit")),
        (owner, amount, lock_period, pool_id),
    );

    Ok(pool_id)
}

pub fn deposit_to_pool_v2(
    env: Env,
    owner: Address,
    amount: i128,
    lock_period: u32,
    apy_bps: u32,
    reward_schedule: RewardSchedule,
    compound_enabled: bool,
) -> Result<u64, FarmError> {
    owner.require_auth();

    if amount > WHALE_CAP {
        return Err(FarmError::WhaleCapExceeded);
    }

    let pool_id = env
        .storage()
        .instance()
        .get::<_, u64>(&symbol_short!("next_pid"))
        .unwrap_or(0);

    let penalty_bps = if lock_period < 1000 {
        2000
    } else if lock_period < 3000 {
        1000
    } else {
        0
    };

    let pool = FarmPoolV2 {
        id: pool_id,
        owner: owner.clone(),
        amount,
        lock_period,
        start_time: env.ledger().timestamp(),
        last_harvest: env.ledger().timestamp(),
        apy_bps,
        reward_schedule,
        compound_enabled,
        penalty_bps,
    };

    env.storage().persistent().set(&(pool_id + 100_000), &pool);
    let next_pool_id = pool_id
        .checked_add(1)
        .ok_or(FarmError::ArithmeticOverflow)?;
    env.storage()
        .instance()
        .set(&symbol_short!("next_pid"), &next_pool_id);

    env.events().publish(
        (symbol_short!("farm"), symbol_short!("v2_dep")),
        (owner, amount, lock_period, apy_bps, compound_enabled),
    );

    Ok(pool_id)
}

pub fn harvest_farm_rewards(env: Env, owner: Address, pool_id: u64) -> Result<i128, FarmError> {
    owner.require_auth();

    let mut pool: FarmPool = env
        .storage()
        .persistent()
        .get(&pool_id)
        .ok_or(FarmError::InvalidPool)?;

    if pool.owner != owner {
        return Err(FarmError::InvalidPool);
    }

    let now = env.ledger().timestamp();
    let elapsed = now.saturating_sub(pool.last_harvest);

    if elapsed == 0 {
        return Ok(0);
    }

    let reward = calculate_farm_reward(pool.amount, elapsed, BASE_APY_BPS)
        .ok_or(FarmError::ArithmeticOverflow)?;

    pool.last_harvest = now;
    env.storage().persistent().set(&pool_id, &pool);

    env.events().publish(
        (symbol_short!("farm"), symbol_short!("harvest")),
        (owner, reward, pool_id),
    );

    Ok(reward)
}

pub fn harvest_v2_rewards(env: Env, owner: Address, pool_id: u64) -> Result<i128, FarmError> {
    owner.require_auth();

    let v2_pool_id = pool_id + 100_000;
    let mut pool: FarmPoolV2 = env
        .storage()
        .persistent()
        .get(&v2_pool_id)
        .ok_or(FarmError::InvalidPool)?;

    if pool.owner != owner {
        return Err(FarmError::InvalidPool);
    }

    let now = env.ledger().timestamp();
    let elapsed = now.saturating_sub(pool.last_harvest);

    if elapsed == 0 {
        return Ok(0);
    }

    let mut reward = calculate_farm_reward(pool.amount, elapsed, pool.apy_bps as i128)
        .ok_or(FarmError::ArithmeticOverflow)?;

    match pool.reward_schedule {
        RewardSchedule::Accelerated => {
            reward = reward.saturating_mul(15) / 10;
        }
        RewardSchedule::Stepwise(ref thresholds) => {
            let total_elapsed = now.saturating_sub(pool.start_time);
            for t in thresholds.iter() {
                if total_elapsed >= t {
                    reward = reward.saturating_mul(12) / 10;
                }
            }
        }
        RewardSchedule::Linear => {}
    }

    if pool.compound_enabled {
        pool.amount = pool.amount.saturating_add(reward);
    }

    pool.last_harvest = now;
    env.storage().persistent().set(&v2_pool_id, &pool);

    env.events().publish(
        (symbol_short!("farm"), symbol_short!("v2_harv")),
        (owner, reward, pool_id, pool.amount),
    );

    Ok(reward)
}

pub fn withdraw_from_pool(env: Env, owner: Address, pool_id: u64) -> Result<i128, FarmError> {
    owner.require_auth();

    let pool: FarmPool = env
        .storage()
        .persistent()
        .get(&pool_id)
        .ok_or(FarmError::InvalidPool)?;

    if pool.owner != owner {
        return Err(FarmError::InvalidPool);
    }

    let now = env.ledger().timestamp();
    let unlock_at = pool
        .start_time
        .checked_add(pool.lock_period as u64)
        .ok_or(FarmError::ArithmeticOverflow)?;
    if now < unlock_at {
        return Err(FarmError::LockNotMet);
    }

    let reward = harvest_farm_rewards(env.clone(), owner.clone(), pool_id)?;
    env.storage().persistent().remove(&pool_id);

    pool.amount
        .checked_add(reward)
        .ok_or(FarmError::ArithmeticOverflow)
}

pub fn withdraw_v2_with_penalty(env: Env, owner: Address, pool_id: u64) -> Result<i128, FarmError> {
    owner.require_auth();

    let v2_pool_id = pool_id + 100_000;
    let pool: FarmPoolV2 = env
        .storage()
        .persistent()
        .get(&v2_pool_id)
        .ok_or(FarmError::InvalidPool)?;

    if pool.owner != owner {
        return Err(FarmError::InvalidPool);
    }

    let now = env.ledger().timestamp();
    let unlock_at = pool
        .start_time
        .checked_add(pool.lock_period as u64)
        .ok_or(FarmError::ArithmeticOverflow)?;

    let reward = harvest_v2_rewards(env.clone(), owner.clone(), pool_id)?;
    let total = pool
        .amount
        .checked_add(reward)
        .ok_or(FarmError::ArithmeticOverflow)?;

    let final_amount = if now < unlock_at && pool.penalty_bps > 0 {
        let penalty = total * pool.penalty_bps as i128 / BPS_DENOMINATOR;
        total.saturating_sub(penalty)
    } else {
        total
    };

    env.storage().persistent().remove(&v2_pool_id);

    env.events().publish(
        (symbol_short!("farm"), symbol_short!("v2_wd")),
        (owner, final_amount, pool_id),
    );

    Ok(final_amount)
}

pub fn get_yield_farm_stats(env: &Env) -> YieldFarmStats {
    let ttl: i128 = env
        .storage()
        .persistent()
        .get(&DataKey::TotalLocked)
        .unwrap_or(0);
    YieldFarmStats {
        total_value_locked: ttl,
        active_pools: 0,
        total_rewards_distributed: 0,
        average_apy_bps: BASE_APY_BPS as u32,
    }
}

#[contracttype]
#[derive(Clone)]
pub enum DataKey {
    TotalLocked,
}

// ── Staking (Issue #504) ─────────────────────────────────────────────────────
//
// Four staking products share this section, all paying yield in harvested
// resource units and all tracked in the TVL counters:
//
// * **Resource staking**: lock one asset for 7 / 30 / 90 days at 5 / 15 /
//   35 % APY (`economics::apy_calculator`). Yield accrues per second, is
//   capped at the lock length, and is paid from a per-asset yield reserve
//   that anyone (the treasury, in practice) can fund. Emergency withdrawal
//   before the unlock forfeits accrued yield and half the principal; the
//   penalty tops up the reserve, so early leavers fund patient stakers.
// * **NFT staking**: a ship is frozen (transfers refused while staked) and
//   earns `10 * level * rarity` units a day of the configured yield asset.
// * **LP staking**: AMM LP tokens are moved out of the provider's balance
//   into a MasterChef-style pool; funded rewards split 80 % to stakers pro
//   rata and 20 % into an impermanent-loss pot that compensates the
//   protected fraction of divergence loss at withdrawal.
//
// Guild staking and governance slashing live in `staking.rs`.

use crate::economics::apy_calculator::{self as apy, LockTier};
use crate::resource_minter::{credit_resource_balance, debit_resource_balance};
use crate::traits::{RealTimeProvider, TimeProvider};
use soroban_sdk::Symbol;

/// Per-asset TVL above which the APY dampener starts (units).
pub const DEFAULT_TVL_TARGET: i128 = 10_000_000;
/// Fixed-point scale of the LP reward-per-share accumulator.
pub const REWARD_PRECISION: i128 = 1_000_000_000_000;
/// Share of each LP reward deposit diverted to the impermanent-loss pot.
pub const IL_POT_SHARE_BPS: i128 = 2_000;
/// Rarity multiplier (bps) for a fighter hull.
pub const RARITY_FIGHTER_BPS: u32 = 10_000;
/// Rarity multiplier (bps) for an explorer hull.
pub const RARITY_EXPLORER_BPS: u32 = 11_000;
/// Rarity multiplier (bps) for a hauler hull.
pub const RARITY_HAULER_BPS: u32 = 12_000;

/// Storage keys for the staking products.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum StakeKey {
    /// Next resource stake id.
    NextStakeId,
    /// Resource stake by id.
    ResourceStake(u64),
    /// Ship stake by ship id.
    NftStake(u64),
    /// LP stake by (pool id, provider).
    LpStake(u64, Address),
    /// Per-pool LP staking state.
    LpPool(u64),
    /// Units locked per asset (resource stakes).
    Tvl(Symbol),
    /// Units locked across every asset (resource stakes).
    TvlTotal,
    /// Ships currently staked.
    NftCount,
    /// Yield reserve per asset.
    YieldReserve(Symbol),
    /// Lifetime yield paid.
    TotalYieldPaid,
    /// Asset ship stakes pay out in (default `dust`).
    NftYieldAsset,
    /// TVL target used by the APY dampener.
    TvlTarget,
}

/// A locked resource position.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResourceStake {
    /// Stake id.
    pub id: u64,
    /// Staker.
    pub owner: Address,
    /// Locked asset.
    pub asset_id: Symbol,
    /// Locked units.
    pub amount: i128,
    /// Lock tier.
    pub tier: LockTier,
    /// APY fixed at deposit, after the TVL dampener.
    pub apy_bps: u32,
    /// Deposit timestamp.
    pub staked_at: u64,
    /// Timestamp from which the principal can leave without penalty.
    pub unlock_at: u64,
    /// Yield already claimed.
    pub yield_claimed: i128,
}

/// A staked ship.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NftStake {
    /// Staked ship.
    pub ship_id: u64,
    /// Staker.
    pub owner: Address,
    /// Ship level at deposit (yield multiplier).
    pub level: u32,
    /// Rarity multiplier in bps.
    pub rarity_bps: u32,
    /// Deposit timestamp.
    pub staked_at: u64,
    /// Yield already claimed.
    pub yield_claimed: i128,
}

/// A staked LP position.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LpStake {
    /// Provider.
    pub owner: Address,
    /// AMM pool.
    pub pool_id: u64,
    /// Staked LP units.
    pub amount: i128,
    /// Deposit timestamp (drives impermanent-loss protection vesting).
    pub staked_at: u64,
    /// `reserve_b / reserve_a` in bps at deposit.
    pub entry_ratio_bps: u64,
    /// MasterChef reward debt.
    pub reward_debt: i128,
}

/// Per-pool LP staking accounting.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LpPoolState {
    /// LP units staked in total.
    pub total_staked: i128,
    /// Accumulated reward per staked unit, scaled by `REWARD_PRECISION`.
    pub acc_reward_per_share: i128,
    /// Asset rewards and IL compensation are paid in.
    pub reward_asset: Symbol,
    /// Units reserved for impermanent-loss compensation.
    pub il_pot: i128,
}

/// Outcome of an emergency withdrawal.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EmergencyWithdrawal {
    /// Principal handed back.
    pub returned: i128,
    /// Principal forfeited to the yield reserve.
    pub penalty: i128,
    /// Accrued yield forfeited.
    pub yield_forfeited: i128,
}

/// Aggregate locked value.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TvlReport {
    /// Resource units locked across every asset.
    pub resource_units: i128,
    /// Ships staked.
    pub ships_staked: u32,
    /// Lifetime yield paid.
    pub total_yield_paid: i128,
}

fn to_u32(amount: i128) -> Result<u32, FarmError> {
    u32::try_from(amount).map_err(|_| FarmError::ArithmeticOverflow)
}

fn debit(env: &Env, owner: &Address, asset: &Symbol, amount: i128) -> Result<(), FarmError> {
    debit_resource_balance(env, owner, asset, to_u32(amount)?)
        .map(|_| ())
        .map_err(|_| FarmError::InsufficientBalance)
}

fn credit(env: &Env, owner: &Address, asset: &Symbol, amount: i128) -> Result<(), FarmError> {
    credit_resource_balance(env, owner, asset, to_u32(amount)?)
        .map(|_| ())
        .map_err(|_| FarmError::ArithmeticOverflow)
}

fn adjust_tvl(env: &Env, asset: &Symbol, delta: i128) -> Result<(), FarmError> {
    let per_asset: i128 = env
        .storage()
        .persistent()
        .get(&StakeKey::Tvl(asset.clone()))
        .unwrap_or(0);
    let total: i128 = env
        .storage()
        .persistent()
        .get(&StakeKey::TvlTotal)
        .unwrap_or(0);
    let per_asset = per_asset
        .checked_add(delta)
        .ok_or(FarmError::ArithmeticOverflow)?;
    let total = total
        .checked_add(delta)
        .ok_or(FarmError::ArithmeticOverflow)?;
    env.storage()
        .persistent()
        .set(&StakeKey::Tvl(asset.clone()), &per_asset);
    env.storage().persistent().set(&StakeKey::TvlTotal, &total);
    // Keep the legacy stats key in step so `get_yield_farm_stats` sees it.
    env.storage()
        .persistent()
        .set(&DataKey::TotalLocked, &total);
    Ok(())
}

/// Require `caller` to be the staking admin registered by
/// `staking::initialize`.
pub fn require_staking_admin(env: &Env, caller: &Address) -> Result<(), FarmError> {
    caller.require_auth();
    let admin: Address = env
        .storage()
        .instance()
        .get(&crate::staking::DataKey::Admin)
        .ok_or(FarmError::NotOwner)?;
    if admin != *caller {
        return Err(FarmError::NotOwner);
    }
    Ok(())
}

/// Units of `asset` locked in resource stakes.
pub fn get_tvl(env: &Env, asset: Symbol) -> i128 {
    env.storage()
        .persistent()
        .get(&StakeKey::Tvl(asset))
        .unwrap_or(0)
}

/// Locked value across every product.
pub fn get_tvl_report(env: &Env) -> TvlReport {
    TvlReport {
        resource_units: env
            .storage()
            .persistent()
            .get(&StakeKey::TvlTotal)
            .unwrap_or(0),
        ships_staked: env
            .storage()
            .persistent()
            .get(&StakeKey::NftCount)
            .unwrap_or(0),
        total_yield_paid: env
            .storage()
            .persistent()
            .get(&StakeKey::TotalYieldPaid)
            .unwrap_or(0),
    }
}

/// TVL target for the APY dampener (default [`DEFAULT_TVL_TARGET`]).
pub fn get_tvl_target(env: &Env) -> i128 {
    env.storage()
        .instance()
        .get(&StakeKey::TvlTarget)
        .unwrap_or(DEFAULT_TVL_TARGET)
}

/// Set the TVL target. Authorisation is the caller's responsibility (the
/// contract wrapper restricts it to the staking admin).
pub fn set_tvl_target(env: &Env, target: i128) {
    env.storage().instance().set(&StakeKey::TvlTarget, &target);
}

/// Units of `asset` available to pay yield.
pub fn get_yield_reserve(env: &Env, asset: Symbol) -> i128 {
    env.storage()
        .persistent()
        .get(&StakeKey::YieldReserve(asset))
        .unwrap_or(0)
}

/// Move `amount` of `asset` from `funder`'s balance into the yield reserve.
pub fn fund_yield_reserve(
    env: &Env,
    funder: &Address,
    asset: Symbol,
    amount: u32,
) -> Result<i128, FarmError> {
    funder.require_auth();
    if amount == 0 {
        return Err(FarmError::InsufficientBalance);
    }
    debit(env, funder, &asset, i128::from(amount))?;
    let reserve = get_yield_reserve(env, asset.clone())
        .checked_add(i128::from(amount))
        .ok_or(FarmError::ArithmeticOverflow)?;
    env.storage()
        .persistent()
        .set(&StakeKey::YieldReserve(asset.clone()), &reserve);
    env.events().publish(
        (symbol_short!("stake"), symbol_short!("fund")),
        (funder.clone(), asset, amount),
    );
    Ok(reserve)
}

fn add_to_reserve(env: &Env, asset: &Symbol, amount: i128) -> Result<(), FarmError> {
    let reserve = get_yield_reserve(env, asset.clone())
        .checked_add(amount)
        .ok_or(FarmError::ArithmeticOverflow)?;
    env.storage()
        .persistent()
        .set(&StakeKey::YieldReserve(asset.clone()), &reserve);
    Ok(())
}

/// Pay `amount` of `asset` yield to `to` from the reserve.
fn pay_yield(env: &Env, to: &Address, asset: &Symbol, amount: i128) -> Result<(), FarmError> {
    if amount <= 0 {
        return Ok(());
    }
    let reserve = get_yield_reserve(env, asset.clone());
    if reserve < amount {
        return Err(FarmError::YieldReserveEmpty);
    }
    env.storage()
        .persistent()
        .set(&StakeKey::YieldReserve(asset.clone()), &(reserve - amount));
    credit(env, to, asset, amount)?;
    let paid: i128 = env
        .storage()
        .persistent()
        .get(&StakeKey::TotalYieldPaid)
        .unwrap_or(0);
    env.storage()
        .persistent()
        .set(&StakeKey::TotalYieldPaid, &paid.saturating_add(amount));
    Ok(())
}

// ── Resource staking ─────────────────────────────────────────────────────────

/// Yield a resource stake has earned but not yet claimed, as of `time`.
pub fn resource_yield_with<T: TimeProvider>(time: &T, stake: &ResourceStake) -> i128 {
    apy::accrued_yield(
        time,
        stake.amount,
        stake.apy_bps,
        stake.staked_at,
        stake.tier.duration_secs(),
    )
    .unwrap_or(0)
    .saturating_sub(stake.yield_claimed)
    .max(0)
}

/// Read a resource stake.
pub fn get_resource_stake(env: &Env, stake_id: u64) -> Option<ResourceStake> {
    env.storage()
        .persistent()
        .get(&StakeKey::ResourceStake(stake_id))
}

fn load_resource_stake(
    env: &Env,
    owner: &Address,
    stake_id: u64,
) -> Result<ResourceStake, FarmError> {
    let stake = get_resource_stake(env, stake_id).ok_or(FarmError::NotStaked)?;
    if stake.owner != *owner {
        return Err(FarmError::NotOwner);
    }
    Ok(stake)
}

/// Lock `amount` of `asset_id` for `tier`. Returns the stake id.
pub fn stake_resource(
    env: &Env,
    owner: &Address,
    asset_id: Symbol,
    amount: u32,
    tier: LockTier,
) -> Result<u64, FarmError> {
    owner.require_auth();
    if amount == 0 {
        return Err(FarmError::InsufficientBalance);
    }
    let amount_i = i128::from(amount);
    debit(env, owner, &asset_id, amount_i)?;

    let tvl = get_tvl(env, asset_id.clone());
    let apy_bps = apy::tvl_adjusted_apy_bps(tier.apy_bps(), tvl, get_tvl_target(env));
    let now = env.ledger().timestamp();
    let id: u64 = env
        .storage()
        .instance()
        .get(&StakeKey::NextStakeId)
        .unwrap_or(1);
    let stake = ResourceStake {
        id,
        owner: owner.clone(),
        asset_id: asset_id.clone(),
        amount: amount_i,
        tier,
        apy_bps,
        staked_at: now,
        unlock_at: now.saturating_add(tier.duration_secs()),
        yield_claimed: 0,
    };
    env.storage()
        .persistent()
        .set(&StakeKey::ResourceStake(id), &stake);
    env.storage().instance().set(
        &StakeKey::NextStakeId,
        &(id.checked_add(1).ok_or(FarmError::ArithmeticOverflow)?),
    );
    adjust_tvl(env, &asset_id, amount_i)?;

    env.events().publish(
        (symbol_short!("stake"), symbol_short!("res_lock")),
        (owner.clone(), id, asset_id, amount, tier, apy_bps),
    );
    Ok(id)
}

/// Pay out the yield accrued so far on a resource stake.
pub fn claim_resource_yield(env: &Env, owner: &Address, stake_id: u64) -> Result<i128, FarmError> {
    owner.require_auth();
    let mut stake = load_resource_stake(env, owner, stake_id)?;
    let due = resource_yield_with(&RealTimeProvider::new(env), &stake);
    if due == 0 {
        return Ok(0);
    }
    pay_yield(env, owner, &stake.asset_id, due)?;
    stake.yield_claimed = stake.yield_claimed.saturating_add(due);
    env.storage()
        .persistent()
        .set(&StakeKey::ResourceStake(stake_id), &stake);
    env.events().publish(
        (symbol_short!("stake"), symbol_short!("res_yld")),
        (owner.clone(), stake_id, due),
    );
    Ok(due)
}

/// Return the principal plus any unclaimed yield once the lock has passed.
pub fn unstake_resource(env: &Env, owner: &Address, stake_id: u64) -> Result<i128, FarmError> {
    owner.require_auth();
    unstake_resource_inner(env, owner, stake_id)
}

/// Body of [`unstake_resource`] without the auth check, so the emergency
/// path can reuse it after authorising once.
fn unstake_resource_inner(env: &Env, owner: &Address, stake_id: u64) -> Result<i128, FarmError> {
    let stake = load_resource_stake(env, owner, stake_id)?;
    if env.ledger().timestamp() < stake.unlock_at {
        return Err(FarmError::LockNotMet);
    }
    let due = resource_yield_with(&RealTimeProvider::new(env), &stake);
    pay_yield(env, owner, &stake.asset_id, due)?;
    env.storage()
        .persistent()
        .remove(&StakeKey::ResourceStake(stake_id));
    adjust_tvl(env, &stake.asset_id, -stake.amount)?;
    credit(env, owner, &stake.asset_id, stake.amount)?;
    env.events().publish(
        (symbol_short!("stake"), symbol_short!("res_out")),
        (owner.clone(), stake_id, stake.amount, due),
    );
    Ok(stake.amount.saturating_add(due))
}

/// Leave a resource stake before its unlock: half the principal and all
/// accrued yield are forfeited to the asset's yield reserve. After the
/// unlock this is a plain `unstake_resource`.
pub fn emergency_unstake_resource(
    env: &Env,
    owner: &Address,
    stake_id: u64,
) -> Result<EmergencyWithdrawal, FarmError> {
    owner.require_auth();
    let stake = load_resource_stake(env, owner, stake_id)?;
    if env.ledger().timestamp() >= stake.unlock_at {
        let total = unstake_resource_inner(env, owner, stake_id)?;
        return Ok(EmergencyWithdrawal {
            returned: total,
            penalty: 0,
            yield_forfeited: 0,
        });
    }
    let forfeited = resource_yield_with(&RealTimeProvider::new(env), &stake);
    let (returned, penalty) = apy::emergency_withdrawal(stake.amount);
    env.storage()
        .persistent()
        .remove(&StakeKey::ResourceStake(stake_id));
    adjust_tvl(env, &stake.asset_id, -stake.amount)?;
    add_to_reserve(env, &stake.asset_id, penalty)?;
    credit(env, owner, &stake.asset_id, returned)?;
    env.events().publish(
        (symbol_short!("stake"), symbol_short!("res_emrg")),
        (owner.clone(), stake_id, returned, penalty, forfeited),
    );
    Ok(EmergencyWithdrawal {
        returned,
        penalty,
        yield_forfeited: forfeited,
    })
}

// ── NFT staking ──────────────────────────────────────────────────────────────

/// Rarity multiplier for a hull type.
pub fn nft_rarity_bps(ship_type: &Symbol) -> u32 {
    if *ship_type == symbol_short!("hauler") {
        RARITY_HAULER_BPS
    } else if *ship_type == symbol_short!("explorer") {
        RARITY_EXPLORER_BPS
    } else {
        RARITY_FIGHTER_BPS
    }
}

/// Asset ship stakes pay out in.
pub fn get_nft_yield_asset(env: &Env) -> Symbol {
    env.storage()
        .instance()
        .get(&StakeKey::NftYieldAsset)
        .unwrap_or(symbol_short!("dust"))
}

/// Set the asset ship stakes pay out in. Authorisation is the wrapper's job.
pub fn set_nft_yield_asset(env: &Env, asset: Symbol) {
    env.storage()
        .instance()
        .set(&StakeKey::NftYieldAsset, &asset);
}

/// Whether `ship_id` is currently staked.
pub fn is_ship_staked(env: &Env, ship_id: u64) -> bool {
    env.storage().persistent().has(&StakeKey::NftStake(ship_id))
}

/// Read a ship stake.
pub fn get_ship_stake(env: &Env, ship_id: u64) -> Option<NftStake> {
    env.storage().persistent().get(&StakeKey::NftStake(ship_id))
}

/// Yield a ship stake has earned but not yet claimed, as of `time`.
pub fn nft_yield_with<T: TimeProvider>(time: &T, stake: &NftStake) -> i128 {
    apy::nft_accrued_yield(
        stake.level,
        stake.rarity_bps,
        time.elapsed_since(stake.staked_at),
    )
    .unwrap_or(0)
    .saturating_sub(stake.yield_claimed)
    .max(0)
}

fn set_nft_count(env: &Env, delta: i32) {
    let count: u32 = env
        .storage()
        .persistent()
        .get(&StakeKey::NftCount)
        .unwrap_or(0);
    let next = if delta >= 0 {
        count.saturating_add(delta as u32)
    } else {
        count.saturating_sub(delta.unsigned_abs())
    };
    env.storage().persistent().set(&StakeKey::NftCount, &next);
}

/// Stake a ship. The ship stays in the owner's wallet but transfers are
/// refused until it is unstaked. Yield scales with the ship's level at
/// deposit; re-stake after levelling up to pick up the new rate.
pub fn stake_ship(env: &Env, owner: &Address, ship_id: u64) -> Result<NftStake, FarmError> {
    owner.require_auth();
    let ship = crate::ship_nft::get_ship(env, ship_id).map_err(|_| FarmError::NotStaked)?;
    if ship.owner != *owner {
        return Err(FarmError::NotOwner);
    }
    if is_ship_staked(env, ship_id) {
        return Err(FarmError::AlreadyStaked);
    }
    let stake = NftStake {
        ship_id,
        owner: owner.clone(),
        level: crate::ship_upgrade::get_ship_level(env, ship_id).max(1),
        rarity_bps: nft_rarity_bps(&ship.ship_type),
        staked_at: env.ledger().timestamp(),
        yield_claimed: 0,
    };
    env.storage()
        .persistent()
        .set(&StakeKey::NftStake(ship_id), &stake);
    set_nft_count(env, 1);
    env.events().publish(
        (symbol_short!("stake"), symbol_short!("nft_lock")),
        (owner.clone(), ship_id, stake.level, stake.rarity_bps),
    );
    Ok(stake)
}

fn load_nft_stake(env: &Env, owner: &Address, ship_id: u64) -> Result<NftStake, FarmError> {
    let stake = get_ship_stake(env, ship_id).ok_or(FarmError::NotStaked)?;
    if stake.owner != *owner {
        return Err(FarmError::NotOwner);
    }
    Ok(stake)
}

/// Pay out the yield a staked ship has earned so far.
pub fn claim_ship_yield(env: &Env, owner: &Address, ship_id: u64) -> Result<i128, FarmError> {
    owner.require_auth();
    let mut stake = load_nft_stake(env, owner, ship_id)?;
    let due = nft_yield_with(&RealTimeProvider::new(env), &stake);
    if due == 0 {
        return Ok(0);
    }
    pay_yield(env, owner, &get_nft_yield_asset(env), due)?;
    stake.yield_claimed = stake.yield_claimed.saturating_add(due);
    env.storage()
        .persistent()
        .set(&StakeKey::NftStake(ship_id), &stake);
    env.events().publish(
        (symbol_short!("stake"), symbol_short!("nft_yld")),
        (owner.clone(), ship_id, due),
    );
    Ok(due)
}

/// Unstake a ship, paying any unclaimed yield.
pub fn unstake_ship(env: &Env, owner: &Address, ship_id: u64) -> Result<i128, FarmError> {
    owner.require_auth();
    let stake = load_nft_stake(env, owner, ship_id)?;
    let due = nft_yield_with(&RealTimeProvider::new(env), &stake);
    pay_yield(env, owner, &get_nft_yield_asset(env), due)?;
    env.storage()
        .persistent()
        .remove(&StakeKey::NftStake(ship_id));
    set_nft_count(env, -1);
    env.events().publish(
        (symbol_short!("stake"), symbol_short!("nft_out")),
        (owner.clone(), ship_id, due),
    );
    Ok(due)
}

// ── LP staking ───────────────────────────────────────────────────────────────

/// Per-pool LP staking state (zeroed if nobody has staked yet).
pub fn get_lp_pool_state(env: &Env, pool_id: u64) -> Option<LpPoolState> {
    env.storage().persistent().get(&StakeKey::LpPool(pool_id))
}

/// Read an LP stake.
pub fn get_lp_stake(env: &Env, pool_id: u64, provider: &Address) -> Option<LpStake> {
    env.storage()
        .persistent()
        .get(&StakeKey::LpStake(pool_id, provider.clone()))
}

fn pool_state_or_init(env: &Env, pool_id: u64) -> Result<LpPoolState, FarmError> {
    if let Some(state) = get_lp_pool_state(env, pool_id) {
        return Ok(state);
    }
    let pool = crate::trading::get_pool(env, pool_id).ok_or(FarmError::InvalidPool)?;
    Ok(LpPoolState {
        total_staked: 0,
        acc_reward_per_share: 0,
        reward_asset: pool.resource_a,
        il_pot: 0,
    })
}

fn current_ratio_bps(env: &Env, pool_id: u64) -> Result<u64, FarmError> {
    let pool = crate::trading::get_pool(env, pool_id).ok_or(FarmError::InvalidPool)?;
    if pool.reserve_a <= 0 {
        return Ok(10_000);
    }
    let ratio = pool
        .reserve_b
        .saturating_mul(10_000)
        .checked_div(pool.reserve_a)
        .unwrap_or(10_000);
    Ok(u64::try_from(ratio).unwrap_or(u64::MAX))
}

fn pending_lp_reward(stake: &LpStake, state: &LpPoolState) -> i128 {
    stake
        .amount
        .saturating_mul(state.acc_reward_per_share)
        .checked_div(REWARD_PRECISION)
        .unwrap_or(0)
        .saturating_sub(stake.reward_debt)
        .max(0)
}

fn move_lp(env: &Env, pool_id: u64, provider: &Address, delta: i128) -> Result<(), FarmError> {
    let key = crate::trading::AmmKey::LpBalance(pool_id, provider.clone());
    let balance: i128 = env.storage().persistent().get(&key).unwrap_or(0);
    let next = balance
        .checked_add(delta)
        .ok_or(FarmError::ArithmeticOverflow)?;
    if next < 0 {
        return Err(FarmError::InsufficientBalance);
    }
    env.storage().persistent().set(&key, &next);
    Ok(())
}

/// Stake `amount` LP units of `pool_id`. Pending rewards on an existing
/// position are paid out first.
pub fn stake_lp(
    env: &Env,
    provider: &Address,
    pool_id: u64,
    amount: i128,
) -> Result<LpStake, FarmError> {
    provider.require_auth();
    if amount <= 0 {
        return Err(FarmError::InsufficientBalance);
    }
    let mut state = pool_state_or_init(env, pool_id)?;
    move_lp(env, pool_id, provider, -amount)?;

    let now = env.ledger().timestamp();
    let stake = match get_lp_stake(env, pool_id, provider) {
        Some(mut existing) => {
            let pending = pending_lp_reward(&existing, &state);
            pay_yield(env, provider, &state.reward_asset, pending)?;
            existing.amount = existing
                .amount
                .checked_add(amount)
                .ok_or(FarmError::ArithmeticOverflow)?;
            existing.reward_debt =
                existing.amount.saturating_mul(state.acc_reward_per_share) / REWARD_PRECISION;
            existing
        }
        None => LpStake {
            owner: provider.clone(),
            pool_id,
            amount,
            staked_at: now,
            entry_ratio_bps: current_ratio_bps(env, pool_id)?,
            reward_debt: amount.saturating_mul(state.acc_reward_per_share) / REWARD_PRECISION,
        },
    };
    state.total_staked = state
        .total_staked
        .checked_add(amount)
        .ok_or(FarmError::ArithmeticOverflow)?;
    env.storage()
        .persistent()
        .set(&StakeKey::LpStake(pool_id, provider.clone()), &stake);
    env.storage()
        .persistent()
        .set(&StakeKey::LpPool(pool_id), &state);
    env.events().publish(
        (symbol_short!("stake"), symbol_short!("lp_lock")),
        (provider.clone(), pool_id, amount),
    );
    Ok(stake)
}

/// Deposit `amount` of the pool's reward asset: 80 % is distributed to
/// current LP stakers pro rata, 20 % tops up the impermanent-loss pot. With
/// nobody staked the whole deposit goes to the pot.
pub fn fund_lp_rewards(
    env: &Env,
    funder: &Address,
    pool_id: u64,
    amount: u32,
) -> Result<(), FarmError> {
    funder.require_auth();
    if amount == 0 {
        return Err(FarmError::InsufficientBalance);
    }
    let mut state = pool_state_or_init(env, pool_id)?;
    let amount_i = i128::from(amount);
    debit(env, funder, &state.reward_asset, amount_i)?;
    add_to_reserve(env, &state.reward_asset, amount_i)?;
    let to_pot = if state.total_staked > 0 {
        amount_i * IL_POT_SHARE_BPS / 10_000
    } else {
        amount_i
    };
    let to_stakers = amount_i - to_pot;
    if to_stakers > 0 {
        state.acc_reward_per_share = state
            .acc_reward_per_share
            .checked_add(
                to_stakers
                    .checked_mul(REWARD_PRECISION)
                    .ok_or(FarmError::ArithmeticOverflow)?
                    / state.total_staked,
            )
            .ok_or(FarmError::ArithmeticOverflow)?;
    }
    state.il_pot = state
        .il_pot
        .checked_add(to_pot)
        .ok_or(FarmError::ArithmeticOverflow)?;
    env.storage()
        .persistent()
        .set(&StakeKey::LpPool(pool_id), &state);
    env.events().publish(
        (symbol_short!("stake"), symbol_short!("lp_fund")),
        (funder.clone(), pool_id, to_stakers, to_pot),
    );
    Ok(())
}

/// Pay out pending LP rewards.
pub fn claim_lp_rewards(env: &Env, provider: &Address, pool_id: u64) -> Result<i128, FarmError> {
    provider.require_auth();
    let state = get_lp_pool_state(env, pool_id).ok_or(FarmError::NotStaked)?;
    let mut stake = get_lp_stake(env, pool_id, provider).ok_or(FarmError::NotStaked)?;
    let pending = pending_lp_reward(&stake, &state);
    if pending == 0 {
        return Ok(0);
    }
    pay_yield(env, provider, &state.reward_asset, pending)?;
    stake.reward_debt = stake.amount.saturating_mul(state.acc_reward_per_share) / REWARD_PRECISION;
    env.storage()
        .persistent()
        .set(&StakeKey::LpStake(pool_id, provider.clone()), &stake);
    Ok(pending)
}

/// Impermanent-loss compensation owed on `stake` right now, in reward-asset
/// units: `amount * IL(entry -> current) * protection(elapsed)`, capped by
/// the pool's IL pot.
pub fn il_compensation_with<T: TimeProvider>(
    time: &T,
    stake: &LpStake,
    current_ratio_bps: u64,
    il_pot: i128,
) -> i128 {
    if stake.entry_ratio_bps == 0 {
        return 0;
    }
    let change = u128::from(current_ratio_bps) * 10_000 / u128::from(stake.entry_ratio_bps);
    let change_bps = u64::try_from(change).unwrap_or(u64::MAX);
    let il = apy::impermanent_loss_bps(change_bps);
    let protected = il.saturating_sub(apy::unprotected_il_bps(
        change_bps,
        time.elapsed_since(stake.staked_at),
    ));
    let owed = stake.amount.saturating_mul(i128::from(protected)) / 10_000;
    owed.min(il_pot).max(0)
}

/// Withdraw `amount` LP units: pays pending rewards, returns the LP tokens
/// and compensates the protected share of impermanent loss from the pot.
/// Returns `(pending_rewards, il_compensation)`.
pub fn unstake_lp(
    env: &Env,
    provider: &Address,
    pool_id: u64,
    amount: i128,
) -> Result<(i128, i128), FarmError> {
    provider.require_auth();
    if amount <= 0 {
        return Err(FarmError::InsufficientBalance);
    }
    let mut state = get_lp_pool_state(env, pool_id).ok_or(FarmError::NotStaked)?;
    let mut stake = get_lp_stake(env, pool_id, provider).ok_or(FarmError::NotStaked)?;
    if amount > stake.amount {
        return Err(FarmError::InsufficientBalance);
    }
    let pending = pending_lp_reward(&stake, &state);
    pay_yield(env, provider, &state.reward_asset, pending)?;

    let share = LpStake {
        amount,
        ..stake.clone()
    };
    let compensation = il_compensation_with(
        &RealTimeProvider::new(env),
        &share,
        current_ratio_bps(env, pool_id)?,
        state.il_pot,
    );
    if compensation > 0 {
        state.il_pot -= compensation;
        pay_yield(env, provider, &state.reward_asset, compensation)?;
    }

    stake.amount -= amount;
    stake.reward_debt = stake.amount.saturating_mul(state.acc_reward_per_share) / REWARD_PRECISION;
    state.total_staked -= amount;
    move_lp(env, pool_id, provider, amount)?;
    if stake.amount == 0 {
        env.storage()
            .persistent()
            .remove(&StakeKey::LpStake(pool_id, provider.clone()));
    } else {
        env.storage()
            .persistent()
            .set(&StakeKey::LpStake(pool_id, provider.clone()), &stake);
    }
    env.storage()
        .persistent()
        .set(&StakeKey::LpPool(pool_id), &state);
    env.events().publish(
        (symbol_short!("stake"), symbol_short!("lp_out")),
        (provider.clone(), pool_id, amount, pending, compensation),
    );
    Ok((pending, compensation))
}

// ── Tests (Issue #239: arithmetic safety) ───────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;
    use soroban_sdk::testutils::Address as _;

    fn make_env() -> Env {
        let env = Env::default();
        env.mock_all_auths();
        env
    }

    // // #[test]
    fn test_calculate_farm_reward_matches_worked_example() {
        // 15% APY, 100 staked, 1 full year elapsed => 15 units of reward.
        let reward = calculate_farm_reward(100, SECONDS_IN_YEAR, BASE_APY_BPS).unwrap();
        assert_eq!(reward, 15);
    }

    // // #[test]
    fn test_calculate_farm_reward_zero_elapsed_is_zero() {
        assert_eq!(calculate_farm_reward(1_000_000, 0, BASE_APY_BPS), Some(0));
    }

    // // #[test]
    fn test_calculate_farm_reward_overflow_reported_not_wrapped() {
        // i128::MAX * BASE_APY_BPS overflows the first checked_mul.
        assert_eq!(
            calculate_farm_reward(i128::MAX, SECONDS_IN_YEAR, BASE_APY_BPS),
            None
        );
    }

    proptest! {
        /// The reward helper never panics for any amount within the whale
        /// cap and any realistic elapsed duration, and never returns a
        /// negative reward for a non-negative stake.
        // // #[test]
        fn farm_reward_never_panics_within_whale_cap(
            amount in 0i128..=WHALE_CAP,
            elapsed in 0u64..=(SECONDS_IN_YEAR * 100),
        ) {
            let reward = calculate_farm_reward(amount, elapsed, BASE_APY_BPS);
            if let Some(r) = reward {
                prop_assert!(r >= 0);
            }
        }
    }

    // // #[test]
    fn test_deposit_withdraw_pool_id_increments_safely() {
        let env = make_env();
        let owner = Address::generate(&env);

        let id1 = deposit_to_pool(env.clone(), owner.clone(), 100, 0).unwrap();
        let id2 = deposit_to_pool(env.clone(), owner.clone(), 100, 0).unwrap();
        assert_eq!(id2, id1 + 1);
    }

    // // #[test]
    fn test_withdraw_before_lock_period_rejected() {
        let env = make_env();
        let owner = Address::generate(&env);

        let id = deposit_to_pool(env.clone(), owner.clone(), 100, 1_000).unwrap();
        let result = withdraw_from_pool(env.clone(), owner, id);
        assert_eq!(result, Err(FarmError::LockNotMet));
    }
}
