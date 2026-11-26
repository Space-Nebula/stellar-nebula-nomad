//! Time-locked treasure vaults and the loot system (Issues #239, #472, #655).
//!
//! Life cycle of a vault:
//!
//! 1. **Deposit** — [`deposit_treasure`] locks an amount for the minimum lock
//!    duration.
//! 2. **Claim principal** — after the lock, [`claim_treasure`] pays out the
//!    principal plus the 10% bonus and forges one **vault key** for the owner.
//! 3. **Discover** — after the lock, [`discover_vault`] explores the vault and
//!    rolls its loot once on the [`LOOT_TABLE`]. The result is stored and can
//!    never be re-rolled.
//! 4. **Claim loot** — [`claim_vault_loot`] spends one key and pays the loot.
//!    Legendary loot refunds the key plus one bonus key.
//!
//! Fairness: the loot roll is uniform over `0..10_000` and the table weights
//! sum to exactly 10_000, so each tier's odds equal its weight. A pity timer
//! guarantees at least Epic loot after [`PITY_THRESHOLD`] consecutive
//! discoveries below Epic.

use soroban_sdk::{contracterror, contracttype, symbol_short, xdr::ToXdr, Address, Bytes, Env};

use crate::randomness_oracle::request_random_seed;
use crate::reentrancy_guard::{with_guard, ReentrancyError};

/// Default minimum lock duration: 7 days in seconds.
pub const DEFAULT_MIN_LOCK_DURATION: u64 = 604_800;

/// Bonus multiplier: 10% bonus on locked amount (in basis points).
const BONUS_BPS: u64 = 1_000;
const BPS_DENOM: u64 = 10_000;

#[derive(Clone)]
#[contracttype]
pub enum VaultKey {
    /// Vault data keyed by vault_id.
    Vault(u64),
    /// Auto-incrementing vault counter.
    VaultCounter,
    /// Minimum lock duration in seconds (configurable).
    MinLockDuration,
    /// Discovery (rolled loot) keyed by vault_id.
    Discovery(u64),
    /// Unspent vault keys held by an owner.
    KeyBalance(Address),
    /// Consecutive below-Epic discoveries for an owner (pity timer).
    Pity(Address),
}

#[contracterror]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum VaultError {
    /// Vault not found.
    VaultNotFound = 1,
    /// Caller is not the vault owner.
    NotOwner = 2,
    /// Vault is still within its lock period.
    StillLocked = 3,
    /// Vault has already been claimed.
    AlreadyClaimed = 4,
    /// Deposit amount must be positive.
    InvalidAmount = 5,
    /// A checked arithmetic operation overflowed (Issue #239).
    ArithmeticOverflow = 6,
    /// A guarded section was re-entered (Issue #472).
    Reentrancy = 7,
    /// The vault has not been discovered yet.
    NotDiscovered = 8,
    /// The owner holds no vault key.
    KeyRequired = 9,
    /// The vault's loot has already been claimed.
    LootAlreadyClaimed = 10,
    /// The vault has already been discovered; loot cannot be re-rolled.
    AlreadyDiscovered = 11,
}

impl crate::error_standard::StandardContractError for VaultError {
    fn descriptor(self) -> crate::error_standard::ErrorDescriptor {
        use crate::error_standard::ErrorKind;
        let (kind, retryable) = match self {
            Self::VaultNotFound => (ErrorKind::NotFound, false),
            Self::NotOwner => (ErrorKind::Authorization, false),
            Self::StillLocked => (ErrorKind::Conflict, true),
            Self::AlreadyClaimed => (ErrorKind::Conflict, false),
            Self::InvalidAmount => (ErrorKind::Validation, false),
            Self::ArithmeticOverflow => (ErrorKind::ResourceLimit, false),
            Self::Reentrancy => (ErrorKind::Conflict, false),
            Self::NotDiscovered | Self::KeyRequired => (ErrorKind::Conflict, true),
            Self::LootAlreadyClaimed | Self::AlreadyDiscovered => (ErrorKind::Conflict, false),
        };
        crate::error_standard::ErrorDescriptor {
            module: "treasure_vault",
            code: self as u32,
            kind,
            retryable,
        }
    }
}

impl From<ReentrancyError> for VaultError {
    fn from(_: ReentrancyError) -> Self {
        VaultError::Reentrancy
    }
}

/// A time-locked treasure vault.
#[derive(Clone)]
#[contracttype]
pub struct TreasureVault {
    pub vault_id: u64,
    pub owner: Address,
    pub ship_id: u64,
    pub amount: u64,
    pub lock_until: u64,
    pub bonus_multiplier: u64,
    pub claimed: bool,
}

use crate::{ensure_auth, storage_get_default, storage_set};

fn next_vault_id(env: &Env) -> Result<u64, VaultError> {
    let current: u64 = storage_get_default!(env, VaultKey::VaultCounter, 0);
    let next = current
        .checked_add(1)
        .ok_or(VaultError::ArithmeticOverflow)?;
    storage_set!(env, VaultKey::VaultCounter, next);
    Ok(next)
}

/// Bonus yield for a claimed vault: `amount * bonus_multiplier / BPS_DENOM`,
/// computed with checked arithmetic so an extreme `amount` reports overflow
/// instead of silently wrapping (Issue #239).
fn calculate_bonus_payout(amount: u64, bonus_multiplier: u64) -> Option<u64> {
    let bonus = amount
        .checked_mul(bonus_multiplier)?
        .checked_div(BPS_DENOM)?;
    amount.checked_add(bonus)
}

fn get_min_lock_duration(env: &Env) -> u64 {
    storage_get_default!(env, VaultKey::MinLockDuration, DEFAULT_MIN_LOCK_DURATION)
}

/// Deposit resources into a time-locked treasure vault.
///
/// The vault locks the specified `amount` until `lock_until`, which is
/// calculated as the current timestamp plus the minimum lock duration.
/// A bonus multiplier is applied at claim time.
///
/// # Reentrancy
/// Runs under the global reentrancy guard (Issue #472): every check and state
/// effect completes while the lock is held, following checks-effects-
/// interactions, so a nested call into any guarded entry point while this one
/// is in flight is rejected with a `Reentrancy` error.
pub fn deposit_treasure(
    env: &Env,
    owner: &Address,
    ship_id: u64,
    amount: u64,
) -> Result<TreasureVault, VaultError> {
    with_guard(env, || {
        deposit_treasure_unguarded(env, owner, ship_id, amount)
    })
}

/// Unguarded body of [`deposit_treasure`]; callers must already hold the
/// reentrancy lock (e.g. another guarded entry point composing it).
fn deposit_treasure_unguarded(
    env: &Env,
    owner: &Address,
    ship_id: u64,
    amount: u64,
) -> Result<TreasureVault, VaultError> {
    ensure_auth!(owner);

    if amount == 0 {
        return Err(VaultError::InvalidAmount);
    }

    let min_lock = get_min_lock_duration(env);
    let lock_until = env
        .ledger()
        .timestamp()
        .checked_add(min_lock)
        .ok_or(VaultError::ArithmeticOverflow)?;
    let vault_id = next_vault_id(env)?;

    let vault = TreasureVault {
        vault_id,
        owner: owner.clone(),
        ship_id,
        amount,
        lock_until,
        bonus_multiplier: BONUS_BPS,
        claimed: false,
    };

    storage_set!(env, VaultKey::Vault(vault_id), vault);

    // Emit VaultDeposited event
    env.events().publish(
        (symbol_short!("vault"), symbol_short!("deposit")),
        (vault_id, owner.clone(), ship_id, amount, lock_until),
    );

    Ok(vault)
}

/// Claim a treasure vault after its lock period has expired.
///
/// Returns the original amount plus bonus yield.
/// The bonus is calculated as: `amount * bonus_multiplier / 10_000`.
///
/// # Reentrancy
/// Runs under the global reentrancy guard (Issue #472): every check and state
/// effect completes while the lock is held, following checks-effects-
/// interactions, so a nested call into any guarded entry point while this one
/// is in flight is rejected with a `Reentrancy` error.
pub fn claim_treasure(env: &Env, owner: &Address, vault_id: u64) -> Result<u64, VaultError> {
    with_guard(env, || claim_treasure_unguarded(env, owner, vault_id))
}

/// Unguarded body of [`claim_treasure`]; callers must already hold the
/// reentrancy lock (e.g. another guarded entry point composing it).
fn claim_treasure_unguarded(env: &Env, owner: &Address, vault_id: u64) -> Result<u64, VaultError> {
    ensure_auth!(owner);

    let mut vault: TreasureVault = env
        .storage()
        .instance()
        .get(&VaultKey::Vault(vault_id))
        .ok_or(VaultError::VaultNotFound)?;

    if vault.owner != *owner {
        return Err(VaultError::NotOwner);
    }

    if vault.claimed {
        return Err(VaultError::AlreadyClaimed);
    }

    let now = env.ledger().timestamp();
    if now < vault.lock_until {
        return Err(VaultError::StillLocked);
    }

    // Calculate bonus yield (checked: Issue #239)
    let total_payout = calculate_bonus_payout(vault.amount, vault.bonus_multiplier)
        .ok_or(VaultError::ArithmeticOverflow)?;

    vault.claimed = true;
    storage_set!(env, VaultKey::Vault(vault_id), vault);
    // Completing a lock forges one vault key (Issue #655).
    grant_keys(env, owner, 1);

    // Emit VaultClaimed event
    env.events().publish(
        (symbol_short!("vault"), symbol_short!("claimed")),
        (vault_id, owner.clone(), total_payout),
    );

    Ok(total_payout)
}

/// Read a vault by ID.
pub fn get_vault(env: &Env, vault_id: u64) -> Option<TreasureVault> {
    env.storage().instance().get(&VaultKey::Vault(vault_id))
}

// ── Loot system (Issue #655) ────────────────────────────────────────────────

/// Loot rarity rolled when a vault is discovered.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[contracttype]
pub enum LootRarity {
    Common,
    Rare,
    Epic,
    Legendary,
}

/// Extra reward attached to high-tier loot.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[contracttype]
pub enum SpecialReward {
    None,
    /// Epic: a star chart revealing a hidden sector.
    StarChart,
    /// Legendary: an ancient relic, plus the spent key back and a bonus key.
    AncientRelic,
}

/// One row of the loot table.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LootTier {
    pub rarity: LootRarity,
    /// Odds in basis points; all weights sum to [`LOOT_ROLL_SPACE`].
    pub weight_bps: u32,
    /// Loot = vault amount × multiplier / 100.
    pub multiplier_pct: u64,
    pub special: SpecialReward,
}

pub const LOOT_ROLL_SPACE: u32 = 10_000;

/// The loot table, ordered from most to least common.
pub const LOOT_TABLE: [LootTier; 4] = [
    LootTier {
        rarity: LootRarity::Common,
        weight_bps: 6_000,
        multiplier_pct: 20,
        special: SpecialReward::None,
    },
    LootTier {
        rarity: LootRarity::Rare,
        weight_bps: 2_800,
        multiplier_pct: 50,
        special: SpecialReward::None,
    },
    LootTier {
        rarity: LootRarity::Epic,
        weight_bps: 1_000,
        multiplier_pct: 120,
        special: SpecialReward::StarChart,
    },
    LootTier {
        rarity: LootRarity::Legendary,
        weight_bps: 200,
        multiplier_pct: 300,
        special: SpecialReward::AncientRelic,
    },
];

/// Consecutive below-Epic discoveries after which Epic is guaranteed.
pub const PITY_THRESHOLD: u32 = 10;
/// Bonus keys granted by Legendary loot (on top of refunding the spent key).
pub const LEGENDARY_BONUS_KEYS: u32 = 1;

/// The stored outcome of discovering a vault.
#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct VaultDiscovery {
    pub vault_id: u64,
    pub owner: Address,
    pub discovered_at: u64,
    pub rarity: LootRarity,
    pub loot_amount: u64,
    pub special: SpecialReward,
    pub loot_claimed: bool,
}

/// What [`claim_vault_loot`] paid out.
#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct LootReward {
    pub vault_id: u64,
    pub rarity: LootRarity,
    pub amount: u64,
    pub special: SpecialReward,
    /// Keys credited back to the owner by this claim.
    pub keys_granted: u32,
}

/// Map a uniform roll in `0..LOOT_ROLL_SPACE` to a loot tier.
///
/// When `pity_triggered`, rolls that would land below Epic are promoted to
/// Epic; a Legendary roll stays Legendary.
pub fn select_loot_tier(roll: u32, pity_triggered: bool) -> LootTier {
    let roll = roll % LOOT_ROLL_SPACE;
    let mut cumulative = 0u32;
    let mut selected = LOOT_TABLE[0];
    for tier in LOOT_TABLE.iter() {
        cumulative += tier.weight_bps;
        if roll < cumulative {
            selected = *tier;
            break;
        }
    }
    if pity_triggered && matches!(selected.rarity, LootRarity::Common | LootRarity::Rare) {
        selected = LOOT_TABLE[2];
    }
    selected
}

/// Loot amount for a tier, with checked arithmetic.
pub fn loot_amount(vault_amount: u64, tier: &LootTier) -> Option<u64> {
    vault_amount
        .checked_mul(tier.multiplier_pct)?
        .checked_div(100)
}

/// Uniform roll in `0..LOOT_ROLL_SPACE`, unique per (ledger, entropy pool,
/// vault, owner). A u32 drawn mod 10_000 has a bias below 3e-6 per tier.
fn roll_loot(env: &Env, vault_id: u64, owner: &Address) -> u32 {
    let seed: Bytes = request_random_seed(env).into();
    let mut input = seed;
    input.append(&Bytes::from_slice(env, &vault_id.to_be_bytes()));
    input.append(&owner.clone().to_xdr(env));
    let digest: [u8; 32] = env.crypto().sha256(&input).to_array();
    let raw = u32::from_be_bytes([digest[0], digest[1], digest[2], digest[3]]);
    raw % LOOT_ROLL_SPACE
}

fn grant_keys(env: &Env, owner: &Address, count: u32) {
    let balance: u32 = storage_get_default!(env, VaultKey::KeyBalance(owner.clone()), 0u32);
    storage_set!(
        env,
        VaultKey::KeyBalance(owner.clone()),
        balance.saturating_add(count)
    );
    env.events().publish(
        (symbol_short!("vault"), symbol_short!("key")),
        (owner.clone(), count),
    );
}

/// Unspent vault keys held by `owner`.
pub fn get_key_balance(env: &Env, owner: &Address) -> u32 {
    storage_get_default!(env, VaultKey::KeyBalance(owner.clone()), 0u32)
}

/// Current pity counter for `owner`.
pub fn get_pity_counter(env: &Env, owner: &Address) -> u32 {
    storage_get_default!(env, VaultKey::Pity(owner.clone()), 0u32)
}

/// Read a vault's discovery, if it has been discovered.
pub fn get_vault_discovery(env: &Env, vault_id: u64) -> Option<VaultDiscovery> {
    env.storage().instance().get(&VaultKey::Discovery(vault_id))
}

/// Discover an unlocked vault and roll its loot (once, immutably).
pub fn discover_vault(
    env: &Env,
    owner: &Address,
    vault_id: u64,
) -> Result<VaultDiscovery, VaultError> {
    with_guard(env, || {
        ensure_auth!(owner);
        let vault = get_vault(env, vault_id).ok_or(VaultError::VaultNotFound)?;
        if vault.owner != *owner {
            return Err(VaultError::NotOwner);
        }
        let now = env.ledger().timestamp();
        if now < vault.lock_until {
            return Err(VaultError::StillLocked);
        }
        if env.storage().instance().has(&VaultKey::Discovery(vault_id)) {
            return Err(VaultError::AlreadyDiscovered);
        }

        let pity = get_pity_counter(env, owner);
        let tier = select_loot_tier(roll_loot(env, vault_id, owner), pity >= PITY_THRESHOLD);
        let amount = loot_amount(vault.amount, &tier).ok_or(VaultError::ArithmeticOverflow)?;
        let next_pity = match tier.rarity {
            LootRarity::Epic | LootRarity::Legendary => 0,
            _ => pity.saturating_add(1),
        };

        let discovery = VaultDiscovery {
            vault_id,
            owner: owner.clone(),
            discovered_at: now,
            rarity: tier.rarity,
            loot_amount: amount,
            special: tier.special,
            loot_claimed: false,
        };
        storage_set!(env, VaultKey::Discovery(vault_id), discovery.clone());
        storage_set!(env, VaultKey::Pity(owner.clone()), next_pity);

        env.events().publish(
            (symbol_short!("vault"), symbol_short!("discover")),
            (vault_id, tier.rarity, amount),
        );
        Ok(discovery)
    })
}

/// Spend one vault key to claim a discovered vault's loot.
pub fn claim_vault_loot(
    env: &Env,
    owner: &Address,
    vault_id: u64,
) -> Result<LootReward, VaultError> {
    with_guard(env, || {
        ensure_auth!(owner);
        let vault = get_vault(env, vault_id).ok_or(VaultError::VaultNotFound)?;
        if vault.owner != *owner {
            return Err(VaultError::NotOwner);
        }
        let mut discovery = get_vault_discovery(env, vault_id).ok_or(VaultError::NotDiscovered)?;
        if discovery.loot_claimed {
            return Err(VaultError::LootAlreadyClaimed);
        }
        let keys = get_key_balance(env, owner);
        if keys == 0 {
            return Err(VaultError::KeyRequired);
        }

        // Effects before events (checks-effects-interactions, Issue #472).
        let keys_granted = if discovery.rarity == LootRarity::Legendary {
            1 + LEGENDARY_BONUS_KEYS
        } else {
            0
        };
        storage_set!(
            env,
            VaultKey::KeyBalance(owner.clone()),
            (keys - 1).saturating_add(keys_granted)
        );
        discovery.loot_claimed = true;
        storage_set!(env, VaultKey::Discovery(vault_id), discovery.clone());

        env.events().publish(
            (symbol_short!("vault"), symbol_short!("loot")),
            (vault_id, discovery.rarity, discovery.loot_amount),
        );
        Ok(LootReward {
            vault_id,
            rarity: discovery.rarity,
            amount: discovery.loot_amount,
            special: discovery.special,
            keys_granted,
        })
    })
}

// ── Tests (Issue #239: arithmetic safety) ───────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;
    use soroban_sdk::{contract, contractimpl, testutils::Address as _};

    #[contract]
    struct Stub;
    #[contractimpl]
    impl Stub {}

    fn make_env() -> (Env, Address) {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register(Stub, ());
        (env, contract_id)
    }

    #[test]
    fn test_calculate_bonus_payout_matches_default_bonus() {
        // BONUS_BPS = 1_000 (10%): 100 + 10% = 110.
        assert_eq!(calculate_bonus_payout(100, BONUS_BPS), Some(110));
    }

    #[test]
    fn test_calculate_bonus_payout_overflow_reported_not_wrapped() {
        assert_eq!(calculate_bonus_payout(u64::MAX, BONUS_BPS), None);
    }

    proptest! {
        /// The payout is always >= the original amount for any non-overflowing
        /// input, and the helper never panics across the full u64 domain.
        #[test]
        fn bonus_payout_never_below_principal(amount in 0u64..=(u64::MAX / 10_000)) {
            if let Some(payout) = calculate_bonus_payout(amount, BONUS_BPS) {
                prop_assert!(payout >= amount);
            }
        }

        #[test]
        fn bonus_payout_never_panics(amount in any::<u64>(), multiplier in any::<u64>()) {
            let _ = calculate_bonus_payout(amount, multiplier);
        }
    }

    #[test]
    fn test_deposit_then_claim_before_unlock_is_still_locked() {
        let (env, contract_id) = make_env();
        let owner = Address::generate(&env);

        // Separate frames: the host rejects a second `require_auth()` for the
        // same address within one frame.
        let vault = env.as_contract(&contract_id, || {
            deposit_treasure(&env, &owner, 1, 500).unwrap()
        });
        env.as_contract(&contract_id, || {
            let result = claim_treasure(&env, &owner, vault.vault_id);
            assert_eq!(result, Err(VaultError::StillLocked));
        });
    }

    #[test]
    fn test_vault_ops_rejected_while_guard_held() {
        // A re-entrant claim must not pay out twice or observe a vault whose
        // `claimed` flag has not yet been written (Issue #472).
        use soroban_sdk::testutils::Ledger as _;

        let (env, contract_id) = make_env();
        let owner = Address::generate(&env);

        let vault_id = env.as_contract(&contract_id, || {
            deposit_treasure(&env, &owner, 1, 500).unwrap().vault_id
        });
        // Past the lock period, so only the guard can reject the claim.
        env.ledger()
            .with_mut(|l| l.timestamp += DEFAULT_MIN_LOCK_DURATION);

        env.as_contract(&contract_id, || {
            crate::reentrancy_guard::acquire(&env).expect("lock should be free");
            assert_eq!(
                deposit_treasure(&env, &owner, 1, 10).map(|v| v.vault_id),
                Err(VaultError::Reentrancy)
            );
            assert_eq!(
                claim_treasure(&env, &owner, vault_id),
                Err(VaultError::Reentrancy)
            );
            crate::reentrancy_guard::release(&env);
            assert!(!get_vault(&env, vault_id).unwrap().claimed);
        });

        env.as_contract(&contract_id, || {
            assert_eq!(claim_treasure(&env, &owner, vault_id), Ok(550));
        });

        // Effects were committed before returning: a replay is rejected.
        env.as_contract(&contract_id, || {
            assert_eq!(
                claim_treasure(&env, &owner, vault_id),
                Err(VaultError::AlreadyClaimed)
            );
        });
    }

    #[test]
    fn test_vault_ids_increment_safely() {
        let (env, contract_id) = make_env();
        let owner = Address::generate(&env);

        let v1 = env.as_contract(&contract_id, || {
            deposit_treasure(&env, &owner, 1, 100).unwrap()
        });
        let v2 = env.as_contract(&contract_id, || {
            deposit_treasure(&env, &owner, 1, 100).unwrap()
        });
        assert_eq!(v2.vault_id, v1.vault_id + 1);
    }

    // ── Loot system (Issue #655) ──

    use soroban_sdk::testutils::Ledger as _;

    /// Deposit, wait out the lock, and return the vault id.
    fn unlocked_vault(env: &Env, contract_id: &Address, owner: &Address, amount: u64) -> u64 {
        let id = env.as_contract(contract_id, || {
            deposit_treasure(env, owner, 1, amount).unwrap().vault_id
        });
        env.ledger()
            .with_mut(|l| l.timestamp += DEFAULT_MIN_LOCK_DURATION);
        id
    }

    #[test]
    fn loot_table_weights_cover_roll_space() {
        let total: u32 = LOOT_TABLE.iter().map(|t| t.weight_bps).sum();
        assert_eq!(total, LOOT_ROLL_SPACE);
    }

    #[test]
    fn distribution_matches_weights_exactly() {
        // Every roll in the space is equally likely, so counting tiers over the
        // whole space gives the exact odds.
        let mut counts = [0u32; 4];
        for roll in 0..LOOT_ROLL_SPACE {
            let idx = LOOT_TABLE
                .iter()
                .position(|t| t.rarity == select_loot_tier(roll, false).rarity)
                .unwrap();
            counts[idx] += 1;
        }
        for (i, tier) in LOOT_TABLE.iter().enumerate() {
            assert_eq!(counts[i], tier.weight_bps);
        }
    }

    #[test]
    fn rarer_tiers_pay_more_and_carry_specials() {
        for pair in LOOT_TABLE.windows(2) {
            assert!(pair[1].multiplier_pct > pair[0].multiplier_pct);
            assert!(pair[1].weight_bps < pair[0].weight_bps);
        }
        assert_eq!(
            select_loot_tier(9_000, false).special,
            SpecialReward::StarChart
        );
        assert_eq!(
            select_loot_tier(9_999, false).special,
            SpecialReward::AncientRelic
        );
        assert_eq!(select_loot_tier(0, false).special, SpecialReward::None);
    }

    #[test]
    fn pity_promotes_low_rolls_but_keeps_legendary() {
        assert_eq!(select_loot_tier(0, true).rarity, LootRarity::Epic);
        assert_eq!(select_loot_tier(7_000, true).rarity, LootRarity::Epic);
        assert_eq!(select_loot_tier(9_999, true).rarity, LootRarity::Legendary);
    }

    #[test]
    fn loot_amount_is_checked() {
        assert_eq!(loot_amount(1_000, &LOOT_TABLE[3]), Some(3_000));
        assert_eq!(loot_amount(u64::MAX, &LOOT_TABLE[3]), None);
    }

    #[test]
    fn vault_cannot_be_discovered_while_locked() {
        let (env, contract_id) = make_env();
        let owner = Address::generate(&env);
        let id = env.as_contract(&contract_id, || {
            deposit_treasure(&env, &owner, 1, 100).unwrap().vault_id
        });
        env.as_contract(&contract_id, || {
            assert_eq!(
                discover_vault(&env, &owner, id).map(|d| d.vault_id),
                Err(VaultError::StillLocked)
            );
        });
    }

    #[test]
    fn full_loot_flow_discover_key_claim() {
        let (env, contract_id) = make_env();
        let owner = Address::generate(&env);
        let id = unlocked_vault(&env, &contract_id, &owner, 1_000);

        env.as_contract(&contract_id, || {
            let discovery = discover_vault(&env, &owner, id).unwrap();
            let tier = LOOT_TABLE
                .iter()
                .find(|t| t.rarity == discovery.rarity)
                .unwrap();
            assert_eq!(discovery.loot_amount, loot_amount(1_000, tier).unwrap());
            assert_eq!(get_vault_discovery(&env, id), Some(discovery.clone()));
        });

        // Loot is rolled once.
        env.as_contract(&contract_id, || {
            assert_eq!(
                discover_vault(&env, &owner, id).map(|d| d.vault_id),
                Err(VaultError::AlreadyDiscovered)
            );
        });

        env.as_contract(&contract_id, || {
            // No key yet.
            assert_eq!(
                claim_vault_loot(&env, &owner, id).map(|r| r.amount),
                Err(VaultError::KeyRequired)
            );
        });

        // Claiming the principal forges a key.
        env.as_contract(&contract_id, || {
            assert_eq!(claim_treasure(&env, &owner, id), Ok(1_100));
            assert_eq!(get_key_balance(&env, &owner), 1);
        });

        env.as_contract(&contract_id, || {
            let reward = claim_vault_loot(&env, &owner, id).unwrap();
            let expected_keys = if reward.rarity == LootRarity::Legendary {
                2
            } else {
                0
            };
            assert_eq!(reward.keys_granted, expected_keys);
            assert_eq!(get_key_balance(&env, &owner), expected_keys);
            assert!(get_vault_discovery(&env, id).unwrap().loot_claimed);
        });

        env.as_contract(&contract_id, || {
            assert_eq!(
                claim_vault_loot(&env, &owner, id).map(|r| r.amount),
                Err(VaultError::LootAlreadyClaimed)
            );
        });
    }

    #[test]
    fn only_owner_can_discover_or_claim_loot() {
        let (env, contract_id) = make_env();
        let owner = Address::generate(&env);
        let thief = Address::generate(&env);
        let id = unlocked_vault(&env, &contract_id, &owner, 100);
        env.as_contract(&contract_id, || {
            assert_eq!(
                discover_vault(&env, &thief, id).map(|d| d.vault_id),
                Err(VaultError::NotOwner)
            );
        });
        env.as_contract(&contract_id, || {
            discover_vault(&env, &owner, id).unwrap();
        });
        env.as_contract(&contract_id, || {
            assert_eq!(
                claim_vault_loot(&env, &thief, id).map(|r| r.amount),
                Err(VaultError::NotOwner)
            );
        });
    }

    #[test]
    fn claiming_undiscovered_vault_fails() {
        let (env, contract_id) = make_env();
        let owner = Address::generate(&env);
        let id = unlocked_vault(&env, &contract_id, &owner, 100);
        env.as_contract(&contract_id, || {
            assert_eq!(
                claim_vault_loot(&env, &owner, id).map(|r| r.amount),
                Err(VaultError::NotDiscovered)
            );
        });
    }

    #[test]
    fn pity_timer_guarantees_epic() {
        let (env, contract_id) = make_env();
        let owner = Address::generate(&env);
        env.as_contract(&contract_id, || {
            storage_set!(env, VaultKey::Pity(owner.clone()), PITY_THRESHOLD);
        });
        let id = unlocked_vault(&env, &contract_id, &owner, 100);
        env.as_contract(&contract_id, || {
            let d = discover_vault(&env, &owner, id).unwrap();
            assert!(matches!(d.rarity, LootRarity::Epic | LootRarity::Legendary));
            assert_eq!(get_pity_counter(&env, &owner), 0);
        });
    }

    #[test]
    fn discovery_respects_reentrancy_guard() {
        let (env, contract_id) = make_env();
        let owner = Address::generate(&env);
        let id = unlocked_vault(&env, &contract_id, &owner, 100);
        env.as_contract(&contract_id, || {
            crate::reentrancy_guard::acquire(&env).expect("lock should be free");
            assert_eq!(
                discover_vault(&env, &owner, id).map(|d| d.vault_id),
                Err(VaultError::Reentrancy)
            );
            assert_eq!(
                claim_vault_loot(&env, &owner, id).map(|r| r.amount),
                Err(VaultError::Reentrancy)
            );
            crate::reentrancy_guard::release(&env);
            assert!(get_vault_discovery(&env, id).is_none());
        });
    }
}
