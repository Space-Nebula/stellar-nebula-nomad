//! # Referral Program (#701)
//!
//! Invite-a-friend loop for Stellar Nebula Nomad.
//!
//! * **Codes** — every referrer can mint one shareable 8-character code
//!   ([`generate_referral_code`]). Codes avoid look-alike characters
//!   (`0/O`, `1/I/L`) so they survive being read aloud or typed from a phone.
//! * **Tracking** — a new nomad joins with a code ([`register_with_code`]) or
//!   is registered directly by the referrer ([`register_referral`]). Every
//!   referral moves through `registered → converted (first scan) → claimed`.
//! * **Rewards** — once the new nomad completes their first scan the referrer
//!   claims an essence reward from an admin-funded pool, scaled by their tier
//!   multiplier. The new nomad receives an optional welcome bonus at the same
//!   time. Payouts never exceed the pool.
//! * **Statistics** — per-referrer stats ([`get_referral_stats`]), referee
//!   listing ([`get_referees`]) and global totals ([`get_global_stats`]).
//! * **Anti-abuse** — self and circular referrals, one referral per nomad,
//!   device-fingerprint multi-account detection, hourly registration velocity
//!   limit, daily claim cap, automatic blocking after repeated fraud flags and
//!   admin-only pool / config / block controls.
//!
//! See `docs/REFERRAL_PROGRAM.md` for the full flow.

use soroban_sdk::{
    contracterror, contracttype, symbol_short, xdr::ToXdr, Address, Bytes, Env, Symbol, Vec,
};

/// Default essence bonus distributed to the referrer after the new nomad's first scan.
/// Overridable at runtime via `set_reward_config`.
pub const DEFAULT_ESSENCE_REWARD: i128 = 100;
/// Maximum number of rewards a referrer may claim in a single calendar day.
pub const MAX_DAILY_CLAIMS: u32 = 10;
/// Seconds in one day — used to derive the current day bucket.
const SECS_PER_DAY: u64 = 86_400;
/// Length of a generated referral code.
pub const REFERRAL_CODE_LEN: usize = 8;
/// Code alphabet: Crockford-style base32 without `0 O 1 I L U`.
pub const REFERRAL_CODE_ALPHABET: &[u8; 32] = b"23456789ABCDEFGHJKMNPQRSTVWXYZ@#";
/// Referees remembered per referrer for the UI listing.
pub const MAX_TRACKED_REFEREES: u32 = 200;
/// Largest page returned by [`get_referees`].
pub const MAX_REFEREE_PAGE: u32 = 50;
/// Fraud flags after which an address is blocked automatically.
pub const FRAUD_BLOCK_THRESHOLD: u32 = 3;
/// Velocity window for registrations.
const VELOCITY_WINDOW: u64 = 3_600; // 1 hour
/// Registrations a referrer may make per velocity window.
pub const MAX_REFERRALS_PER_HOUR: u32 = 5;
/// Basis-point denominator.
const BPS: i128 = 10_000;

// ─── Storage Keys ─────────────────────────────────────────────────────────────

#[derive(Clone)]
#[contracttype]
pub enum ReferralKey {
    /// Referral record keyed by the new nomad's address (prevents duplicates).
    Referral(Address),
    /// Global auto-increment counter for referral IDs.
    ReferralCount,
    /// Daily claim counter: (referrer, day_number) → u32.
    DailyClaims(Address, u64),
    /// Lifetime essence rewarded per address (referrers and welcomed nomads).
    LifetimeRewards(Address),
    /// Global reward pool balance (essence held for distribution).
    RewardPool,
    /// Reward configuration: fixed amount per claim.
    RewardConfig,
    /// Program administrator.
    Admin,
    /// Code → referrer.
    CodeOwner(Symbol),
    /// Referrer → code.
    CodeOf(Address),
    /// Welcome bonus paid to the new nomad when the referrer claims.
    RefereeBonus,
    /// Registrations per (referrer, hour bucket).
    Velocity(Address, u64),
    /// Device fingerprint → first account seen on it.
    Fingerprint(u64),
    /// Referrer → recent referees (bounded).
    Referees(Address),
}

// ─── Data Types ───────────────────────────────────────────────────────────────

/// On-chain referral record linking a referrer to a newly onboarded nomad.
#[derive(Clone)]
#[contracttype]
pub struct Referral {
    pub id: u64,
    pub referrer: Address,
    pub new_nomad: Address,
    pub registered_at: u64,
    /// True once the referrer has claimed the reward.
    pub claimed: bool,
    /// True once the new nomad has completed their first scan.
    pub first_scan_done: bool,
}

/// Admin-configurable reward settings.
#[derive(Clone)]
#[contracttype]
pub struct RewardConfig {
    /// Fixed essence amount paid out per successful referral claim.
    pub reward_per_claim: i128,
}

/// Breakdown of one successful claim.
#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct ClaimReceipt {
    /// Reward before the tier multiplier.
    pub base_reward: i128,
    /// Extra paid by the tier multiplier.
    pub tier_bonus: i128,
    /// Paid to the referrer (`base_reward + tier_bonus`).
    pub referrer_reward: i128,
    /// Welcome bonus paid to the new nomad.
    pub referee_reward: i128,
    /// Tier the referrer was in when claiming.
    pub tier: u32,
}

/// Everything the referral UI needs about one referrer.
#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct ReferralStats {
    /// Referral code, if one has been generated.
    pub code: Option<Symbol>,
    pub total_referrals: u32,
    /// Referees who completed their first scan.
    pub converted_referrals: u32,
    /// Rewards claimed.
    pub claimed_referrals: u32,
    /// Registered but not yet converted.
    pub pending_referrals: u32,
    /// Converted but not yet claimed.
    pub claimable_referrals: u32,
    pub total_essence_earned: i128,
    pub current_tier: u32,
    pub multiplier_bps: u32,
    /// Referrals needed for the next tier (0 at the top tier).
    pub referrals_to_next_tier: u32,
    pub blocked: bool,
}

// ─── Errors ───────────────────────────────────────────────────────────────────

#[contracterror]
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum ReferralError {
    AlreadyReferred = 1,
    SelfReferral = 2,
    ReferralNotFound = 3,
    AlreadyClaimed = 4,
    FirstScanNotDone = 5,
    DailyClaimCapReached = 6,
    InsufficientRewardPool = 7,
    /// Caller is not the program admin (or no admin is set).
    Unauthorized = 8,
    /// The referrer is blocked for fraud.
    ReferrerBlocked = 9,
    /// The code does not exist or is malformed.
    InvalidCode = 10,
    /// The referrer was themselves referred by the new nomad.
    CircularReferral = 11,
    /// The new nomad's device is already bound to another account.
    SuspectedMultiAccount = 12,
    /// Too many registrations in the velocity window.
    VelocityTooHigh = 13,
    /// Admin already initialised.
    AlreadyInitialized = 14,
    /// Amount must be positive.
    InvalidAmount = 15,
    /// Arithmetic overflow on a balance.
    Overflow = 16,
    /// Could not find a free code (astronomically unlikely).
    CodeSpaceExhausted = 17,
}

impl crate::error_standard::StandardContractError for ReferralError {
    fn descriptor(self) -> crate::error_standard::ErrorDescriptor {
        use crate::error_standard::ErrorKind;
        let (kind, retryable) = match self {
            Self::AlreadyReferred
            | Self::AlreadyClaimed
            | Self::FirstScanNotDone
            | Self::AlreadyInitialized
            | Self::CircularReferral => (ErrorKind::Conflict, false),
            Self::SelfReferral | Self::InvalidCode | Self::InvalidAmount => {
                (ErrorKind::Validation, false)
            }
            Self::ReferralNotFound => (ErrorKind::NotFound, false),
            Self::DailyClaimCapReached | Self::VelocityTooHigh => (ErrorKind::ResourceLimit, true),
            Self::InsufficientRewardPool | Self::CodeSpaceExhausted => {
                (ErrorKind::ResourceLimit, false)
            }
            Self::Unauthorized | Self::ReferrerBlocked | Self::SuspectedMultiAccount => {
                (ErrorKind::Authorization, false)
            }
            Self::Overflow => (ErrorKind::Internal, false),
        };
        crate::error_standard::ErrorDescriptor {
            module: "referral_system",
            code: self as u32,
            kind,
            retryable,
        }
    }
}

// ─── Admin ────────────────────────────────────────────────────────────────────

/// Set the program admin. One-shot.
pub fn init_referral_admin(env: &Env, admin: Address) -> Result<(), ReferralError> {
    admin.require_auth();
    if env.storage().instance().has(&ReferralKey::Admin) {
        return Err(ReferralError::AlreadyInitialized);
    }
    env.storage().instance().set(&ReferralKey::Admin, &admin);
    init_tiers(env);
    Ok(())
}

/// The program admin, if initialised.
pub fn get_referral_admin(env: &Env) -> Option<Address> {
    env.storage().instance().get(&ReferralKey::Admin)
}

fn require_admin(env: &Env, admin: &Address) -> Result<(), ReferralError> {
    admin.require_auth();
    match get_referral_admin(env) {
        Some(stored) if stored == *admin => Ok(()),
        _ => Err(ReferralError::Unauthorized),
    }
}

/// Deposit `amount` essence into the global reward pool. Admin only.
pub fn fund_reward_pool(env: &Env, admin: Address, amount: i128) -> Result<i128, ReferralError> {
    require_admin(env, &admin)?;
    if amount <= 0 {
        return Err(ReferralError::InvalidAmount);
    }

    let new_balance = get_reward_pool_balance(env)
        .checked_add(amount)
        .ok_or(ReferralError::Overflow)?;
    env.storage()
        .instance()
        .set(&ReferralKey::RewardPool, &new_balance);

    env.events().publish(
        (symbol_short!("referral"), symbol_short!("funded")),
        (admin, amount, new_balance),
    );

    Ok(new_balance)
}

/// Update the per-claim reward amount. Admin only.
pub fn set_reward_config(
    env: &Env,
    admin: Address,
    reward_per_claim: i128,
) -> Result<(), ReferralError> {
    require_admin(env, &admin)?;
    if reward_per_claim <= 0 {
        return Err(ReferralError::InvalidAmount);
    }

    env.storage().instance().set(
        &ReferralKey::RewardConfig,
        &RewardConfig { reward_per_claim },
    );

    env.events().publish(
        (symbol_short!("referral"), symbol_short!("cfg")),
        (admin, reward_per_claim),
    );

    Ok(())
}

/// Set the welcome bonus paid to new nomads (0 disables it). Admin only.
pub fn set_referee_bonus(env: &Env, admin: Address, bonus: i128) -> Result<(), ReferralError> {
    require_admin(env, &admin)?;
    if bonus < 0 {
        return Err(ReferralError::InvalidAmount);
    }
    env.storage()
        .instance()
        .set(&ReferralKey::RefereeBonus, &bonus);
    Ok(())
}

/// Admin: manually block a referrer for fraud.
pub fn admin_block_referrer(
    env: &Env,
    admin: Address,
    target: Address,
    reason: Symbol,
) -> Result<(), ReferralError> {
    require_admin(env, &admin)?;

    let fraud = FraudRecord {
        address: target.clone(),
        flag_count: get_fraud_record(env, &target)
            .map(|f| f.flag_count)
            .unwrap_or(0)
            .max(FRAUD_BLOCK_THRESHOLD),
        is_blocked: true,
        reason: reason.clone(),
    };
    env.storage()
        .persistent()
        .set(&ReferralV2Key::FraudFlag(target.clone()), &fraud);

    env.events().publish(
        (symbol_short!("referral"), symbol_short!("blocked")),
        (target, reason),
    );
    Ok(())
}

/// Admin: lift a fraud block and clear the flags.
pub fn admin_unblock_referrer(
    env: &Env,
    admin: Address,
    target: Address,
) -> Result<(), ReferralError> {
    require_admin(env, &admin)?;
    env.storage()
        .persistent()
        .remove(&ReferralV2Key::FraudFlag(target.clone()));
    env.events().publish(
        (symbol_short!("referral"), symbol_short!("unblock")),
        target,
    );
    Ok(())
}

// ─── View Helpers ─────────────────────────────────────────────────────────────

/// Return the current reward pool balance.
pub fn get_reward_pool_balance(env: &Env) -> i128 {
    env.storage()
        .instance()
        .get(&ReferralKey::RewardPool)
        .unwrap_or(0i128)
}

/// Current per-claim base reward.
pub fn get_reward_per_claim(env: &Env) -> i128 {
    env.storage()
        .instance()
        .get::<ReferralKey, RewardConfig>(&ReferralKey::RewardConfig)
        .map(|c| c.reward_per_claim)
        .unwrap_or(DEFAULT_ESSENCE_REWARD)
}

/// Current welcome bonus for new nomads.
pub fn get_referee_bonus(env: &Env) -> i128 {
    env.storage()
        .instance()
        .get(&ReferralKey::RefereeBonus)
        .unwrap_or(0i128)
}

/// Return lifetime essence rewards earned by `referrer`.
pub fn get_lifetime_rewards(env: &Env, referrer: Address) -> i128 {
    env.storage()
        .persistent()
        .get(&ReferralKey::LifetimeRewards(referrer))
        .unwrap_or(0i128)
}

/// Retrieve a referral record by the new nomad's address.
pub fn get_referral(env: &Env, new_nomad: Address) -> Result<Referral, ReferralError> {
    env.storage()
        .persistent()
        .get(&ReferralKey::Referral(new_nomad))
        .ok_or(ReferralError::ReferralNotFound)
}

// ─── Referral codes ───────────────────────────────────────────────────────────

/// True when `code` has the shape of a generated code (length and alphabet).
pub fn is_well_formed_code(code: &[u8]) -> bool {
    code.len() == REFERRAL_CODE_LEN
        && code
            .iter()
            .all(|c| REFERRAL_CODE_ALPHABET[..30].contains(c))
}

fn code_bytes(env: &Env, referrer: &Address, nonce: u32) -> [u8; REFERRAL_CODE_LEN] {
    let mut seed = Bytes::new(env);
    seed.append(&referrer.clone().to_xdr(env));
    seed.extend_from_array(&nonce.to_be_bytes());
    let digest = env.crypto().sha256(&seed).to_array();

    let mut out = [0u8; REFERRAL_CODE_LEN];
    for (i, slot) in out.iter_mut().enumerate() {
        // 30 symbols: the two trailing alphabet entries are never used so
        // codes stay valid `Symbol` characters.
        *slot = REFERRAL_CODE_ALPHABET[(digest[i] % 30) as usize];
    }
    out
}

fn code_candidate(env: &Env, referrer: &Address, nonce: u32) -> Symbol {
    let bytes = code_bytes(env, referrer, nonce);
    // Alphabet is ASCII, so this cannot fail.
    Symbol::new(env, core::str::from_utf8(&bytes).unwrap_or("INVALID0"))
}

/// Mint (or return the existing) referral code for `referrer`.
///
/// Deterministic per address and idempotent: calling again returns the same
/// code. Blocked referrers cannot mint codes.
pub fn generate_referral_code(env: &Env, referrer: Address) -> Result<Symbol, ReferralError> {
    referrer.require_auth();
    if is_blocked(env, &referrer) {
        return Err(ReferralError::ReferrerBlocked);
    }
    if let Some(code) = get_referral_code(env, &referrer) {
        return Ok(code);
    }

    let persistent = env.storage().persistent();
    for nonce in 0..16u32 {
        let code = code_candidate(env, &referrer, nonce);
        if persistent.has(&ReferralKey::CodeOwner(code.clone())) {
            continue;
        }
        persistent.set(&ReferralKey::CodeOwner(code.clone()), &referrer);
        persistent.set(&ReferralKey::CodeOf(referrer.clone()), &code);
        env.events().publish(
            (symbol_short!("referral"), symbol_short!("code")),
            (referrer, code.clone()),
        );
        return Ok(code);
    }
    Err(ReferralError::CodeSpaceExhausted)
}

/// `referrer`'s code, if generated.
pub fn get_referral_code(env: &Env, referrer: &Address) -> Option<Symbol> {
    env.storage()
        .persistent()
        .get(&ReferralKey::CodeOf(referrer.clone()))
}

/// Owner of `code`, if it exists.
pub fn resolve_referral_code(env: &Env, code: Symbol) -> Option<Address> {
    env.storage()
        .persistent()
        .get(&ReferralKey::CodeOwner(code))
}

// ─── Core Functions ───────────────────────────────────────────────────────────

/// Shared registration path. Runs every anti-abuse check, writes the record
/// and updates statistics. Callers handle authorisation.
fn record_referral(
    env: &Env,
    referrer: &Address,
    new_nomad: &Address,
    fingerprint: Option<u64>,
) -> Result<u64, ReferralError> {
    if referrer == new_nomad {
        return Err(ReferralError::SelfReferral);
    }
    if is_blocked(env, referrer) {
        return Err(ReferralError::ReferrerBlocked);
    }

    let persistent = env.storage().persistent();
    if persistent.has(&ReferralKey::Referral(new_nomad.clone())) {
        return Err(ReferralError::AlreadyReferred);
    }
    // A → B then B → A would let two accounts farm each other.
    if let Some(upstream) = persistent.get::<_, Referral>(&ReferralKey::Referral(referrer.clone()))
    {
        if upstream.referrer == *new_nomad {
            return Err(ReferralError::CircularReferral);
        }
    }

    check_fingerprint(env, new_nomad, fingerprint)?;
    check_velocity(env, referrer)?;

    let instance = env.storage().instance();
    let id: u64 = instance.get(&ReferralKey::ReferralCount).unwrap_or(0u64) + 1;
    instance.set(&ReferralKey::ReferralCount, &id);

    let now = env.ledger().timestamp();
    let referral = Referral {
        id,
        referrer: referrer.clone(),
        new_nomad: new_nomad.clone(),
        registered_at: now,
        claimed: false,
        first_scan_done: false,
    };
    persistent.set(&ReferralKey::Referral(new_nomad.clone()), &referral);

    // Per-referrer count, analytics and referee list.
    let count_key = ReferralV2Key::ReferralCount(referrer.clone());
    let new_count: u32 = persistent.get(&count_key).unwrap_or(0u32).saturating_add(1);
    persistent.set(&count_key, &new_count);

    let mut analytics = load_analytics(env, referrer);
    analytics.total_referrals = analytics.total_referrals.saturating_add(1);
    analytics.current_tier = tier_for_count(env, new_count).0;
    analytics.last_referral_at = now;
    store_analytics(env, referrer, &analytics);

    let referees_key = ReferralKey::Referees(referrer.clone());
    let mut referees: Vec<Address> = persistent.get(&referees_key).unwrap_or(Vec::new(env));
    if referees.len() >= MAX_TRACKED_REFEREES {
        referees.pop_front();
    }
    referees.push_back(new_nomad.clone());
    persistent.set(&referees_key, &referees);

    let total: u64 = instance.get(&ReferralV2Key::TotalReferrals).unwrap_or(0);
    instance.set(&ReferralV2Key::TotalReferrals, &(total + 1));

    env.events().publish(
        (symbol_short!("referral"), symbol_short!("register")),
        (referrer.clone(), new_nomad.clone(), id),
    );

    Ok(id)
}

/// Record a referral from `referrer` for `new_nomad`, authorised by the referrer.
///
/// Prevents self, duplicate and circular referrals. Returns the referral ID.
pub fn register_referral(
    env: &Env,
    referrer: Address,
    new_nomad: Address,
) -> Result<u64, ReferralError> {
    referrer.require_auth();
    record_referral(env, &referrer, &new_nomad, None)
}

/// A new nomad joins using a referral `code`. The new nomad authorises.
///
/// `fingerprint` is an optional device hash supplied by the client; when the
/// same device is already bound to another account the registration is
/// rejected as a suspected multi-account.
pub fn register_with_code(
    env: &Env,
    new_nomad: Address,
    code: Symbol,
    fingerprint: Option<u64>,
) -> Result<u64, ReferralError> {
    new_nomad.require_auth();
    let referrer = resolve_referral_code(env, code).ok_or(ReferralError::InvalidCode)?;
    record_referral(env, &referrer, &new_nomad, fingerprint)
}

/// Mark that `nomad` has completed their first scan, unlocking the referral reward.
///
/// Called by the scan flow after a successful `scan_nebula`. The nomad
/// must authorize this call. Idempotent.
pub fn mark_first_scan(env: &Env, nomad: Address) -> Result<(), ReferralError> {
    nomad.require_auth();

    let mut referral = get_referral(env, nomad.clone())?;
    if referral.first_scan_done {
        return Ok(());
    }

    referral.first_scan_done = true;
    env.storage()
        .persistent()
        .set(&ReferralKey::Referral(nomad.clone()), &referral);

    let mut analytics = load_analytics(env, &referral.referrer);
    analytics.converted_referrals = analytics.converted_referrals.saturating_add(1);
    store_analytics(env, &referral.referrer, &analytics);

    env.events().publish(
        (symbol_short!("referral"), symbol_short!("convert")),
        (referral.referrer, nomad),
    );
    Ok(())
}

/// Pay out a referral: base reward × tier multiplier to the referrer plus the
/// welcome bonus to the new nomad. The pool must cover the full payout.
pub fn claim_referral(
    env: &Env,
    referrer: Address,
    new_nomad: Address,
) -> Result<ClaimReceipt, ReferralError> {
    referrer.require_auth();

    if is_blocked(env, &referrer) {
        return Err(ReferralError::ReferrerBlocked);
    }

    let mut referral = get_referral(env, new_nomad.clone())?;
    if referral.referrer != referrer {
        return Err(ReferralError::ReferralNotFound);
    }
    if !referral.first_scan_done {
        return Err(ReferralError::FirstScanNotDone);
    }
    if referral.claimed {
        return Err(ReferralError::AlreadyClaimed);
    }

    let base_reward = get_reward_per_claim(env);
    let count: u32 = env
        .storage()
        .persistent()
        .get(&ReferralV2Key::ReferralCount(referrer.clone()))
        .unwrap_or(0);
    let (tier, multiplier) = tier_for_count(env, count);
    let referrer_reward = base_reward
        .checked_mul(i128::from(multiplier))
        .ok_or(ReferralError::Overflow)?
        / BPS;
    let tier_bonus = referrer_reward - base_reward;
    let referee_reward = get_referee_bonus(env);
    let payout = referrer_reward
        .checked_add(referee_reward)
        .ok_or(ReferralError::Overflow)?;

    let pool = get_reward_pool_balance(env);
    if pool < payout {
        return Err(ReferralError::InsufficientRewardPool);
    }

    // Enforce daily claim cap using temporary storage keyed by day bucket.
    let day = env.ledger().timestamp() / SECS_PER_DAY;
    let daily_key = ReferralKey::DailyClaims(referrer.clone(), day);
    let daily_count: u32 = env.storage().temporary().get(&daily_key).unwrap_or(0u32);
    if daily_count >= MAX_DAILY_CLAIMS {
        return Err(ReferralError::DailyClaimCapReached);
    }
    env.storage()
        .temporary()
        .set(&daily_key, &(daily_count + 1));

    env.storage()
        .instance()
        .set(&ReferralKey::RewardPool, &(pool - payout));

    credit_lifetime(env, &referrer, referrer_reward)?;
    if referee_reward > 0 {
        credit_lifetime(env, &new_nomad, referee_reward)?;
    }

    referral.claimed = true;
    env.storage()
        .persistent()
        .set(&ReferralKey::Referral(new_nomad.clone()), &referral);

    let mut analytics = load_analytics(env, &referrer);
    analytics.successful_referrals = analytics.successful_referrals.saturating_add(1);
    analytics.total_essence_earned = analytics
        .total_essence_earned
        .saturating_add(referrer_reward);
    analytics.current_tier = tier;
    store_analytics(env, &referrer, &analytics);

    let instance = env.storage().instance();
    let total: i128 = instance
        .get(&ReferralV2Key::ReferralTotalRewardsDistributed)
        .unwrap_or(0);
    instance.set(
        &ReferralV2Key::ReferralTotalRewardsDistributed,
        &total.saturating_add(payout),
    );

    env.events().publish(
        (symbol_short!("referral"), symbol_short!("claimed")),
        (referrer, new_nomad, referrer_reward, referee_reward),
    );

    Ok(ClaimReceipt {
        base_reward,
        tier_bonus,
        referrer_reward,
        referee_reward,
        tier,
    })
}

fn credit_lifetime(env: &Env, who: &Address, amount: i128) -> Result<(), ReferralError> {
    let key = ReferralKey::LifetimeRewards(who.clone());
    let lifetime: i128 = env.storage().persistent().get(&key).unwrap_or(0i128);
    let updated = lifetime
        .checked_add(amount)
        .ok_or(ReferralError::Overflow)?;
    env.storage().persistent().set(&key, &updated);
    Ok(())
}

/// Distribute the essence bonus to the referrer. Returns the referrer's payout
/// (tier multiplier included). See [`claim_referral`] for the full receipt.
pub fn claim_referral_reward(
    env: &Env,
    referrer: Address,
    new_nomad: Address,
) -> Result<i128, ReferralError> {
    claim_referral(env, referrer, new_nomad).map(|r| r.referrer_reward)
}

// ═══════════════════════════════════════════════════════════════════════════════
// TIERS, ANALYTICS AND FRAUD (#279, #701)
// ═══════════════════════════════════════════════════════════════════════════════

/// Additional storage keys for V2 features.
#[derive(Clone)]
#[contracttype]
pub enum ReferralV2Key {
    /// Multi-tier reward config per tier level.
    TierConfig(u32),
    /// Referral count per referrer (for tier calculation).
    ReferralCount(Address),
    /// Referrer analytics: total earned, total referrals, conversion rate.
    ReferrerAnalytics(Address),
    /// Fraud flags per address.
    FraudFlag(Address),
    /// Legacy fingerprint key (kept so the enum stays decodable).
    FingerprintHash(u64),
    /// Global analytics.
    TotalReferrals,
    ReferralTotalRewardsDistributed,
}

/// Tier configuration for multi-tier rewards.
#[derive(Clone)]
#[contracttype]
pub struct TierConfig {
    /// Minimum referrals to qualify for this tier.
    pub min_referrals: u32,
    /// Reward multiplier in basis points (10000 = 1x, 15000 = 1.5x).
    pub multiplier_bps: u32,
    /// Tier name for display.
    pub tier_level: u32,
}

/// Referrer analytics record.
#[derive(Clone)]
#[contracttype]
pub struct ReferrerAnalytics {
    pub total_referrals: u32,
    pub successful_referrals: u32,
    pub total_essence_earned: i128,
    pub current_tier: u32,
    pub joined_at: u64,
    pub last_referral_at: u64,
    /// Referees who completed their first scan.
    pub converted_referrals: u32,
}

/// Fraud detection flags.
#[derive(Clone)]
#[contracttype]
pub struct FraudRecord {
    pub address: Address,
    pub flag_count: u32,
    pub is_blocked: bool,
    pub reason: Symbol,
}

/// Errors kept for ABI compatibility with the #279 API.
#[contracterror]
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum ReferralV2Error {
    /// Player is blocked due to fraud detection.
    PlayerBlocked = 10,
    /// Referral velocity too high — possible fraud.
    VelocityTooHigh = 11,
    /// Tier not found.
    TierNotFound = 12,
}

impl crate::error_standard::StandardContractError for ReferralV2Error {
    fn descriptor(self) -> crate::error_standard::ErrorDescriptor {
        use crate::error_standard::ErrorKind;
        let (kind, retryable) = match self {
            Self::PlayerBlocked => (ErrorKind::Authorization, false),
            Self::VelocityTooHigh => (ErrorKind::Validation, false),
            Self::TierNotFound => (ErrorKind::NotFound, false),
        };
        crate::error_standard::ErrorDescriptor {
            module: "referral_system",
            code: self as u32,
            kind,
            retryable,
        }
    }
}

/// Default tier table: Bronze, Silver, Gold, Platinum, Diamond.
pub const DEFAULT_TIERS: [(u32, u32); 5] = [
    (0, 10_000),   // Bronze: 1x
    (5, 12_000),   // Silver: 1.2x
    (20, 15_000),  // Gold: 1.5x
    (50, 20_000),  // Platinum: 2x
    (100, 25_000), // Diamond: 2.5x
];

/// Store the default tier table. Called by [`init_referral_admin`].
pub fn init_tiers(env: &Env) {
    for (level, (min_referrals, multiplier_bps)) in DEFAULT_TIERS.iter().enumerate() {
        let tier = TierConfig {
            min_referrals: *min_referrals,
            multiplier_bps: *multiplier_bps,
            tier_level: level as u32,
        };
        env.storage()
            .instance()
            .set(&ReferralV2Key::TierConfig(level as u32), &tier);
    }
}

/// Get the tier config for a given tier level (stored table, else defaults).
pub fn get_tier_config(env: &Env, tier_level: u32) -> Option<TierConfig> {
    env.storage()
        .instance()
        .get(&ReferralV2Key::TierConfig(tier_level))
        .or_else(|| {
            DEFAULT_TIERS
                .get(tier_level as usize)
                .map(|(min_referrals, multiplier_bps)| TierConfig {
                    min_referrals: *min_referrals,
                    multiplier_bps: *multiplier_bps,
                    tier_level,
                })
        })
}

/// Calculate the referrer's current tier based on their referral count.
pub fn calculate_tier(env: &Env, referrer: &Address) -> u32 {
    let count: u32 = env
        .storage()
        .persistent()
        .get(&ReferralV2Key::ReferralCount(referrer.clone()))
        .unwrap_or(0);
    tier_for_count(env, count).0
}

/// Resolve `(tier_level, multiplier_bps)` for a referral count.
fn tier_for_count(env: &Env, count: u32) -> (u32, u32) {
    let mut best = (0u32, 10_000u32);
    for level in 0..DEFAULT_TIERS.len() as u32 {
        if let Some(tier) = get_tier_config(env, level) {
            if count >= tier.min_referrals {
                best = (tier.tier_level, tier.multiplier_bps);
            }
        }
    }
    best
}

/// Referrals still needed to reach the tier above `count` (0 at the top).
fn referrals_to_next_tier(env: &Env, count: u32) -> u32 {
    let (tier, _) = tier_for_count(env, count);
    match get_tier_config(env, tier + 1) {
        Some(next) => next.min_referrals.saturating_sub(count),
        None => 0,
    }
}

fn load_analytics(env: &Env, referrer: &Address) -> ReferrerAnalytics {
    env.storage()
        .persistent()
        .get(&ReferralV2Key::ReferrerAnalytics(referrer.clone()))
        .unwrap_or(ReferrerAnalytics {
            total_referrals: 0,
            successful_referrals: 0,
            total_essence_earned: 0,
            current_tier: 0,
            joined_at: env.ledger().timestamp(),
            last_referral_at: 0,
            converted_referrals: 0,
        })
}

fn store_analytics(env: &Env, referrer: &Address, analytics: &ReferrerAnalytics) {
    env.storage().persistent().set(
        &ReferralV2Key::ReferrerAnalytics(referrer.clone()),
        analytics,
    );
}

/// Register a referral (referrer-authorised) with an optional device
/// fingerprint for the new nomad.
pub fn register_referral_v2(
    env: &Env,
    referrer: Address,
    new_nomad: Address,
    fingerprint: Option<u64>,
) -> Result<u64, ReferralError> {
    referrer.require_auth();
    record_referral(env, &referrer, &new_nomad, fingerprint)
}

/// Claim referral reward with tier multiplier (alias of [`claim_referral_reward`]).
pub fn claim_referral_reward_v2(
    env: &Env,
    referrer: Address,
    new_nomad: Address,
) -> Result<i128, ReferralError> {
    claim_referral_reward(env, referrer, new_nomad)
}

/// Get referrer analytics.
pub fn get_referrer_analytics(env: &Env, referrer: &Address) -> Option<ReferrerAnalytics> {
    env.storage()
        .persistent()
        .get(&ReferralV2Key::ReferrerAnalytics(referrer.clone()))
}

/// Everything the referral dashboard shows for `referrer`.
pub fn get_referral_stats(env: &Env, referrer: Address) -> ReferralStats {
    let analytics = load_analytics(env, &referrer);
    let count: u32 = env
        .storage()
        .persistent()
        .get(&ReferralV2Key::ReferralCount(referrer.clone()))
        .unwrap_or(0);
    let (tier, multiplier_bps) = tier_for_count(env, count);
    ReferralStats {
        code: get_referral_code(env, &referrer),
        total_referrals: analytics.total_referrals,
        converted_referrals: analytics.converted_referrals,
        claimed_referrals: analytics.successful_referrals,
        pending_referrals: analytics
            .total_referrals
            .saturating_sub(analytics.converted_referrals),
        claimable_referrals: analytics
            .converted_referrals
            .saturating_sub(analytics.successful_referrals),
        total_essence_earned: analytics.total_essence_earned,
        current_tier: tier,
        multiplier_bps,
        referrals_to_next_tier: referrals_to_next_tier(env, count),
        blocked: is_blocked(env, &referrer),
    }
}

/// Page through `referrer`'s most recent referrals (oldest first).
pub fn get_referees(env: &Env, referrer: Address, start: u32, limit: u32) -> Vec<Referral> {
    let referees: Vec<Address> = env
        .storage()
        .persistent()
        .get(&ReferralKey::Referees(referrer))
        .unwrap_or(Vec::new(env));
    let mut out = Vec::new(env);
    let end = start
        .saturating_add(limit.min(MAX_REFEREE_PAGE))
        .min(referees.len());
    for i in start..end {
        if let Some(addr) = referees.get(i) {
            if let Ok(referral) = get_referral(env, addr) {
                out.push_back(referral);
            }
        }
    }
    out
}

/// Get global referral statistics: `(total_referrals, total_rewards_paid)`.
pub fn get_global_stats(env: &Env) -> (u64, i128) {
    let total_referrals: u64 = env
        .storage()
        .instance()
        .get(&ReferralV2Key::TotalReferrals)
        .unwrap_or(0);
    let total_rewards: i128 = env
        .storage()
        .instance()
        .get(&ReferralV2Key::ReferralTotalRewardsDistributed)
        .unwrap_or(0);
    (total_referrals, total_rewards)
}

// ─── Fraud Prevention ─────────────────────────────────────────────────────

/// Bind the new nomad's device fingerprint, rejecting devices already bound to
/// another account (including the referrer's own device — the classic
/// "refer yourself" farm).
///
/// This only rejects: a failed Soroban call rolls back its writes, so a fraud
/// flag set here would never persist. Flags are recorded through
/// [`report_fingerprint`], which succeeds.
fn check_fingerprint(
    env: &Env,
    new_nomad: &Address,
    fingerprint: Option<u64>,
) -> Result<(), ReferralError> {
    let Some(fp) = fingerprint else {
        return Ok(());
    };
    let key = ReferralKey::Fingerprint(fp);
    match env.storage().persistent().get::<_, Address>(&key) {
        Some(owner) if owner == *new_nomad => Ok(()),
        Some(_) => Err(ReferralError::SuspectedMultiAccount),
        None => {
            env.storage().persistent().set(&key, new_nomad);
            Ok(())
        }
    }
}

/// Bind `fingerprint` to `account` (e.g. at sign-up). If the device is already
/// bound to another account, both addresses receive a fraud flag. Admin only,
/// because fingerprints come from the trusted backend. Returns `true` when a
/// collision was found.
pub fn report_fingerprint(
    env: &Env,
    admin: Address,
    account: Address,
    fingerprint: u64,
) -> Result<bool, ReferralError> {
    require_admin(env, &admin)?;
    let key = ReferralKey::Fingerprint(fingerprint);
    match env.storage().persistent().get::<_, Address>(&key) {
        Some(owner) if owner == account => Ok(false),
        Some(owner) => {
            flag_fraud(env, &owner, symbol_short!("multiacct"));
            flag_fraud(env, &account, symbol_short!("multiacct"));
            Ok(true)
        }
        None => {
            env.storage().persistent().set(&key, &account);
            Ok(false)
        }
    }
}

/// `true` when `address` has a fraud record marked as blocked.
pub fn is_blocked(env: &Env, address: &Address) -> bool {
    get_fraud_record(env, address).is_some_and(|f| f.is_blocked)
}

/// Fraud record for `address`, if any.
pub fn get_fraud_record(env: &Env, address: &Address) -> Option<FraudRecord> {
    env.storage()
        .persistent()
        .get(&ReferralV2Key::FraudFlag(address.clone()))
}

/// Check referral velocity (too many registrations in an hour).
///
/// The check rejects without writing so a failed registration leaves no
/// state behind (Soroban rolls back failed calls anyway).
fn check_velocity(env: &Env, referrer: &Address) -> Result<(), ReferralError> {
    let now = env.ledger().timestamp();
    let window_key = ReferralKey::Velocity(referrer.clone(), now / VELOCITY_WINDOW);
    let count: u32 = env.storage().temporary().get(&window_key).unwrap_or(0);

    if count >= MAX_REFERRALS_PER_HOUR {
        return Err(ReferralError::VelocityTooHigh);
    }

    env.storage().temporary().set(&window_key, &(count + 1));
    Ok(())
}

/// Flag an address for fraud; blocks after [`FRAUD_BLOCK_THRESHOLD`] flags.
fn flag_fraud(env: &Env, address: &Address, reason: Symbol) {
    let mut fraud = get_fraud_record(env, address).unwrap_or(FraudRecord {
        address: address.clone(),
        flag_count: 0,
        is_blocked: false,
        reason: symbol_short!("clean"),
    });

    fraud.flag_count = fraud.flag_count.saturating_add(1);
    fraud.reason = reason;
    if fraud.flag_count >= FRAUD_BLOCK_THRESHOLD {
        fraud.is_blocked = true;
    }

    env.storage()
        .persistent()
        .set(&ReferralV2Key::FraudFlag(address.clone()), &fraud);

    env.events().publish(
        (symbol_short!("referral"), symbol_short!("fraud")),
        (address.clone(), fraud.flag_count, fraud.is_blocked),
    );
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use soroban_sdk::testutils::{Address as _, Ledger};
    use soroban_sdk::{contract, contractimpl};

    #[contract]
    struct Stub;
    #[contractimpl]
    impl Stub {}

    struct H {
        env: Env,
        id: Address,
        admin: Address,
    }

    impl H {
        fn new() -> Self {
            let env = Env::default();
            env.mock_all_auths();
            env.ledger().with_mut(|l| l.timestamp = 1_000_000);
            let id = env.register(Stub, ());
            let admin = Address::generate(&env);
            let h = H { env, id, admin };
            // `require_auth` can be satisfied once per address per frame,
            // so every authorised call gets its own `run`.
            h.run(|| init_referral_admin(&h.env, h.admin.clone()).unwrap());
            h.run(|| fund_reward_pool(&h.env, h.admin.clone(), 100_000).unwrap());
            h
        }
        fn run<T>(&self, f: impl FnOnce() -> T) -> T {
            self.env.as_contract(&self.id, f)
        }
        fn addr(&self) -> Address {
            Address::generate(&self.env)
        }
        fn advance(&self, secs: u64) {
            self.env.ledger().with_mut(|l| l.timestamp += secs);
        }
        /// Register, convert and return the new nomad.
        fn converted(&self, referrer: &Address) -> Address {
            let nomad = self.addr();
            self.run(|| register_referral(&self.env, referrer.clone(), nomad.clone()).unwrap());
            self.run(|| mark_first_scan(&self.env, nomad.clone()).unwrap());
            nomad
        }
    }

    // ── Codes ─────────────────────────────────────────────────────────────

    #[test]
    fn code_is_generated_well_formed_and_stable() {
        let h = H::new();
        let r = h.addr();
        let code = h.run(|| generate_referral_code(&h.env, r.clone()).unwrap());
        let again = h.run(|| generate_referral_code(&h.env, r.clone()).unwrap());
        assert_eq!(code, again);

        let bytes = code_bytes(&h.env, &r, 0);
        assert!(is_well_formed_code(&bytes), "bad code {:?}", bytes);
        assert_eq!(
            code,
            Symbol::new(&h.env, core::str::from_utf8(&bytes).unwrap())
        );
        h.run(|| assert_eq!(resolve_referral_code(&h.env, code.clone()), Some(r.clone())));
    }

    #[test]
    fn codes_differ_between_referrers() {
        let h = H::new();
        let a = h.addr();
        let b = h.addr();
        let ca = h.run(|| generate_referral_code(&h.env, a.clone()).unwrap());
        let cb = h.run(|| generate_referral_code(&h.env, b.clone()).unwrap());
        assert_ne!(ca, cb);
    }

    #[test]
    fn well_formed_rejects_ambiguous_characters() {
        assert!(is_well_formed_code(b"ABCD2345"));
        assert!(!is_well_formed_code(b"ABCD0345")); // zero
        assert!(!is_well_formed_code(b"ABCDO345")); // letter O
        assert!(!is_well_formed_code(b"ABC2345")); // short
        assert!(!is_well_formed_code(b"abcd2345")); // lowercase
    }

    #[test]
    fn register_with_code_tracks_the_referral() {
        let h = H::new();
        let r = h.addr();
        let nomad = h.addr();
        let code = h.run(|| generate_referral_code(&h.env, r.clone()).unwrap());
        let id = h.run(|| register_with_code(&h.env, nomad.clone(), code, Some(77)).unwrap());
        h.run(|| {
            let referral = get_referral(&h.env, nomad.clone()).unwrap();
            assert_eq!(referral.id, id);
            assert_eq!(referral.referrer, r);
            assert!(!referral.first_scan_done);
        });
    }

    #[test]
    fn unknown_code_is_rejected() {
        let h = H::new();
        let nomad = h.addr();
        h.run(|| {
            assert_eq!(
                register_with_code(&h.env, nomad, Symbol::new(&h.env, "ZZZZZZZZ"), None),
                Err(ReferralError::InvalidCode)
            );
        });
    }

    // ── Full flow & rewards ──────────────────────────────────────────────

    #[test]
    fn full_flow_pays_referrer_and_referee_from_pool() {
        let h = H::new();
        let r = h.addr();
        h.run(|| set_referee_bonus(&h.env, h.admin.clone(), 25).unwrap());
        let nomad = h.converted(&r);

        let receipt = h.run(|| claim_referral(&h.env, r.clone(), nomad.clone()).unwrap());
        assert_eq!(receipt.base_reward, DEFAULT_ESSENCE_REWARD);
        assert_eq!(receipt.tier, 0);
        assert_eq!(receipt.tier_bonus, 0);
        assert_eq!(receipt.referee_reward, 25);

        h.run(|| {
            assert_eq!(get_reward_pool_balance(&h.env), 100_000 - 125);
            assert_eq!(get_lifetime_rewards(&h.env, r.clone()), 100);
            assert_eq!(get_lifetime_rewards(&h.env, nomad.clone()), 25);
            assert!(get_referral(&h.env, nomad.clone()).unwrap().claimed);
            assert_eq!(get_global_stats(&h.env), (1, 125));
        });

        h.run(|| {
            assert_eq!(
                claim_referral(&h.env, r.clone(), nomad.clone()),
                Err(ReferralError::AlreadyClaimed)
            );
        });
    }

    #[test]
    fn claim_requires_first_scan() {
        let h = H::new();
        let r = h.addr();
        let nomad = h.addr();
        h.run(|| register_referral(&h.env, r.clone(), nomad.clone()).unwrap());
        h.run(|| {
            assert_eq!(
                claim_referral_reward(&h.env, r, nomad),
                Err(ReferralError::FirstScanNotDone)
            );
        });
    }

    #[test]
    fn only_the_actual_referrer_can_claim() {
        let h = H::new();
        let r = h.addr();
        let thief = h.addr();
        let nomad = h.converted(&r);
        h.run(|| {
            assert_eq!(
                claim_referral(&h.env, thief, nomad),
                Err(ReferralError::ReferralNotFound)
            );
        });
    }

    #[test]
    fn tier_multiplier_is_applied_and_fully_funded_by_pool() {
        let h = H::new();
        let r = h.addr();
        // Five referrals → Silver (1.2x). Spread over two hours for velocity.
        let mut nomads = std::vec::Vec::new();
        for i in 0..5 {
            if i == 4 {
                h.advance(VELOCITY_WINDOW);
            }
            nomads.push(h.converted(&r));
        }
        let receipt = h.run(|| claim_referral(&h.env, r.clone(), nomads[0].clone()).unwrap());
        assert_eq!(receipt.tier, 1);
        assert_eq!(receipt.referrer_reward, 120);
        assert_eq!(receipt.tier_bonus, 20);
        h.run(|| assert_eq!(get_reward_pool_balance(&h.env), 100_000 - 120));
    }

    #[test]
    fn claim_fails_when_pool_cannot_cover_multiplied_payout() {
        let env = Env::default();
        env.mock_all_auths();
        let id = env.register(Stub, ());
        let admin = Address::generate(&env);
        let r = Address::generate(&env);
        let nomad = Address::generate(&env);
        env.as_contract(&id, || init_referral_admin(&env, admin.clone()).unwrap());
        env.as_contract(&id, || fund_reward_pool(&env, admin.clone(), 99).unwrap());
        env.as_contract(&id, || {
            register_referral(&env, r.clone(), nomad.clone()).unwrap()
        });
        env.as_contract(&id, || mark_first_scan(&env, nomad.clone()).unwrap());
        env.as_contract(&id, || {
            assert_eq!(
                claim_referral(&env, r, nomad),
                Err(ReferralError::InsufficientRewardPool)
            );
            assert_eq!(get_reward_pool_balance(&env), 99);
        });
    }

    #[test]
    fn statistics_track_each_stage() {
        let h = H::new();
        let r = h.addr();
        let code = h.run(|| generate_referral_code(&h.env, r.clone()).unwrap());
        let a = h.converted(&r);
        let _b = h.converted(&r);
        let c = h.addr();
        h.run(|| register_referral(&h.env, r.clone(), c.clone()).unwrap());
        h.run(|| claim_referral(&h.env, r.clone(), a.clone()).unwrap());

        h.run(|| {
            let stats = get_referral_stats(&h.env, r.clone());
            assert_eq!(stats.code, Some(code.clone()));
            assert_eq!(stats.total_referrals, 3);
            assert_eq!(stats.converted_referrals, 2);
            assert_eq!(stats.claimed_referrals, 1);
            assert_eq!(stats.pending_referrals, 1);
            assert_eq!(stats.claimable_referrals, 1);
            assert_eq!(stats.total_essence_earned, 100);
            assert_eq!(stats.current_tier, 0);
            assert_eq!(stats.multiplier_bps, 10_000);
            assert_eq!(stats.referrals_to_next_tier, 2);
            assert!(!stats.blocked);

            let page = get_referees(&h.env, r.clone(), 0, 10);
            assert_eq!(page.len(), 3);
            assert_eq!(page.get(0).unwrap().new_nomad, a);
            assert_eq!(get_referees(&h.env, r.clone(), 2, 10).len(), 1);
        });
    }

    #[test]
    fn mark_first_scan_is_idempotent_for_stats() {
        let h = H::new();
        let r = h.addr();
        let nomad = h.converted(&r);
        h.run(|| mark_first_scan(&h.env, nomad.clone()).unwrap());
        h.run(|| assert_eq!(get_referral_stats(&h.env, r.clone()).converted_referrals, 1));
    }

    // ── Anti-abuse ───────────────────────────────────────────────────────

    #[test]
    fn self_duplicate_and_circular_referrals_are_rejected() {
        let h = H::new();
        let a = h.addr();
        let b = h.addr();
        let c = h.addr();
        h.run(|| {
            assert_eq!(
                register_referral(&h.env, a.clone(), a.clone()),
                Err(ReferralError::SelfReferral)
            );
        });
        h.run(|| register_referral(&h.env, a.clone(), b.clone()).unwrap());
        h.run(|| {
            assert_eq!(
                register_referral(&h.env, c.clone(), b.clone()),
                Err(ReferralError::AlreadyReferred)
            );
        });
        h.run(|| {
            assert_eq!(
                register_referral(&h.env, b.clone(), a.clone()),
                Err(ReferralError::CircularReferral)
            );
        });
    }

    #[test]
    fn own_code_cannot_be_used() {
        let h = H::new();
        let r = h.addr();
        let code = h.run(|| generate_referral_code(&h.env, r.clone()).unwrap());
        h.run(|| {
            assert_eq!(
                register_with_code(&h.env, r.clone(), code, None),
                Err(ReferralError::SelfReferral)
            );
        });
    }

    #[test]
    fn shared_device_is_rejected_as_multi_account() {
        let h = H::new();
        let r = h.addr();
        let first = h.addr();
        let second = h.addr();
        let code = h.run(|| generate_referral_code(&h.env, r.clone()).unwrap());
        h.run(|| register_with_code(&h.env, first.clone(), code.clone(), Some(42)).unwrap());
        h.run(|| {
            assert_eq!(
                register_with_code(&h.env, second.clone(), code.clone(), Some(42)),
                Err(ReferralError::SuspectedMultiAccount)
            );
        });
    }

    #[test]
    fn velocity_limit_caps_hourly_registrations() {
        let h = H::new();
        let r = h.addr();
        for _ in 0..MAX_REFERRALS_PER_HOUR {
            let n = h.addr();
            h.run(|| register_referral(&h.env, r.clone(), n).unwrap());
        }
        let n = h.addr();
        h.run(|| {
            assert_eq!(
                register_referral(&h.env, r.clone(), n.clone()),
                Err(ReferralError::VelocityTooHigh)
            );
        });
        h.advance(VELOCITY_WINDOW);
        h.run(|| register_referral(&h.env, r.clone(), n).unwrap());
    }

    #[test]
    fn daily_claim_cap_is_enforced() {
        let h = H::new();
        let r = h.addr();
        let mut nomads = std::vec::Vec::new();
        for i in 0..(MAX_DAILY_CLAIMS + 1) {
            if i > 0 && i % MAX_REFERRALS_PER_HOUR == 0 {
                h.advance(VELOCITY_WINDOW);
            }
            nomads.push(h.converted(&r));
        }
        // All claims happen on one day.
        h.advance(SECS_PER_DAY - (h.env.ledger().timestamp() % SECS_PER_DAY) + 10);
        for n in nomads.iter().take(MAX_DAILY_CLAIMS as usize) {
            h.run(|| claim_referral(&h.env, r.clone(), n.clone()).unwrap());
        }
        h.run(|| {
            assert_eq!(
                claim_referral(&h.env, r.clone(), nomads[MAX_DAILY_CLAIMS as usize].clone()),
                Err(ReferralError::DailyClaimCapReached)
            );
        });
    }

    #[test]
    fn fingerprint_collisions_flag_and_eventually_block() {
        let h = H::new();
        let a = h.addr();
        let b = h.addr();
        h.run(|| assert!(!report_fingerprint(&h.env, h.admin.clone(), a.clone(), 9).unwrap()));
        for _ in 0..FRAUD_BLOCK_THRESHOLD {
            h.run(|| assert!(report_fingerprint(&h.env, h.admin.clone(), b.clone(), 9).unwrap()));
        }
        h.run(|| {
            assert!(is_blocked(&h.env, &a));
            assert!(is_blocked(&h.env, &b));
            assert_eq!(
                register_referral(&h.env, a.clone(), h.addr()),
                Err(ReferralError::ReferrerBlocked)
            );
        });
        h.run(|| {
            assert_eq!(
                generate_referral_code(&h.env, a.clone()),
                Err(ReferralError::ReferrerBlocked)
            );
        });
    }

    #[test]
    fn blocked_referrer_cannot_claim_until_unblocked() {
        let h = H::new();
        let r = h.addr();
        let nomad = h.converted(&r);
        h.run(|| {
            admin_block_referrer(&h.env, h.admin.clone(), r.clone(), symbol_short!("manual"))
                .unwrap()
        });
        h.run(|| {
            assert_eq!(
                claim_referral(&h.env, r.clone(), nomad.clone()),
                Err(ReferralError::ReferrerBlocked)
            );
        });
        h.run(|| admin_unblock_referrer(&h.env, h.admin.clone(), r.clone()).unwrap());
        h.run(|| claim_referral(&h.env, r.clone(), nomad.clone()).unwrap());
    }

    #[test]
    fn admin_functions_reject_non_admins() {
        let h = H::new();
        let mallory = h.addr();
        h.run(|| {
            assert_eq!(
                fund_reward_pool(&h.env, mallory.clone(), 1_000),
                Err(ReferralError::Unauthorized)
            )
        });
        h.run(|| {
            assert_eq!(
                set_reward_config(&h.env, mallory.clone(), 1_000),
                Err(ReferralError::Unauthorized)
            )
        });
        h.run(|| {
            assert_eq!(
                admin_block_referrer(&h.env, mallory.clone(), h.admin.clone(), symbol_short!("x")),
                Err(ReferralError::Unauthorized)
            )
        });
        h.run(|| {
            assert_eq!(
                init_referral_admin(&h.env, mallory.clone()),
                Err(ReferralError::AlreadyInitialized)
            )
        });
        h.run(|| {
            assert_eq!(
                fund_reward_pool(&h.env, h.admin.clone(), 0),
                Err(ReferralError::InvalidAmount)
            )
        });
    }

    #[test]
    fn reward_config_changes_the_base_reward() {
        let h = H::new();
        let r = h.addr();
        h.run(|| set_reward_config(&h.env, h.admin.clone(), 250).unwrap());
        let nomad = h.converted(&r);
        h.run(|| assert_eq!(claim_referral_reward(&h.env, r, nomad).unwrap(), 250));
    }

    #[test]
    fn tiers_follow_default_table_without_init() {
        let env = Env::default();
        let id = env.register(Stub, ());
        env.as_contract(&id, || {
            assert_eq!(tier_for_count(&env, 0), (0, 10_000));
            assert_eq!(tier_for_count(&env, 5), (1, 12_000));
            assert_eq!(tier_for_count(&env, 20), (2, 15_000));
            assert_eq!(tier_for_count(&env, 50), (3, 20_000));
            assert_eq!(tier_for_count(&env, 1_000), (4, 25_000));
            assert_eq!(referrals_to_next_tier(&env, 7), 13);
            assert_eq!(referrals_to_next_tier(&env, 500), 0);
        });
    }
}
