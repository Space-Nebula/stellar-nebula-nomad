// ============================================================
// resource_minter.rs — Fix #175: Rate-limited resource minting
// Branch: security/rate-limiting
// ============================================================
//
// Changes vs baseline:
//   • Import and call check_rate_limit(Operation::ResourceMinting)
//     at the top of mint_resource() before any state mutation.
//   • RateLimitHit events are emitted inside check_rate_limit.

use crate::economics::anti_whale::{
    apply_gathering, check_operation, process_anti_whale_action, AntiWhaleError, OpKind,
};
use crate::nebula_explorer::{CellType, NebulaLayout};
use crate::nebula_gen::{NebulaError as NebulaGenError, NebulaGen};
use crate::rate_limiter::{check_rate_limit, Operation, RateLimitError};
use crate::reentrancy_guard::{with_guard, ReentrancyError};
use soroban_sdk::{
    contract, contracterror, contractimpl, contracttype, symbol_short, Address, Env, Symbol, Vec,
};

pub type AssetId = ResourceType;

#[contracttype]
#[derive(Clone)]
pub enum ResourceKey {
    ResourceBalance(Address, Symbol),
    /// Monotonic counter backing [`DexOffer::offer_id`] allocation.
    DexOfferCounter,
    /// DEX offer record by ID.
    DexOffer(u64),
}

pub fn resource_type_to_symbol(rt: &ResourceType) -> Symbol {
    match rt {
        ResourceType::StellarDust => symbol_short!("stdust"),
        ResourceType::DarkMatter => symbol_short!("drmatt"),
        ResourceType::ExoticMatter => symbol_short!("exomat"),
    }
}

// ── Resource types ────────────────────────────────────────────
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ResourceType {
    StellarDust,
    DarkMatter,
    ExoticMatter,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResourceRecord {
    pub owner: Address,
    pub resource_type: ResourceType,
    pub amount: u64,
    pub minted_at: u64,
}

/// Compact form of [`ResourceRecord`]: `amount` and `minted_at` share one
/// `u128` slot (see `storage_optim::pack_u64x2`). New storage only; existing
/// `ResourceRecord` entries are untouched for backward compatibility.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PackedResourceRecord {
    pub owner: Address,
    pub resource_type: ResourceType,
    pub packed: u128,
}

impl From<&ResourceRecord> for PackedResourceRecord {
    fn from(r: &ResourceRecord) -> Self {
        Self {
            owner: r.owner.clone(),
            resource_type: r.resource_type.clone(),
            packed: crate::storage_optim::pack_u64x2(r.amount, r.minted_at),
        }
    }
}

impl From<&PackedResourceRecord> for ResourceRecord {
    fn from(p: &PackedResourceRecord) -> Self {
        let (amount, minted_at) = crate::storage_optim::unpack_u64x2(p.packed);
        Self {
            owner: p.owner.clone(),
            resource_type: p.resource_type.clone(),
            amount,
            minted_at,
        }
    }
}

#[contracttype]
pub enum MinterKey {
    Balance(Address, ResourceType),
    TotalSupply(ResourceType),
    /// Cumulative amount ever minted, never decremented. Together with the
    /// burn counters in `token_burning` this gives the deflation rate
    /// (Issue #281): `burned / ever_minted`.
    TotalMinted(ResourceType),
}

// ── Error ─────────────────────────────────────────────────────
#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum MinterError {
    /// Amount must be > 0.
    InvalidAmount = 200,
    /// Caller exceeded the minting rate limit (DoS prevention).
    RateLimitExceeded = 201,
    /// No nebula layout found for this ship (must scan first).
    NoLayoutForShip = 202,
    /// The specified anomaly index does not contain a resource.
    NoResourceAtAnomaly = 203,
    /// A checked arithmetic operation overflowed (Issue #239).
    ArithmeticOverflow = 204,
    /// The account holds less than the requested debit amount (Issue #281).
    InsufficientBalance = 205,
    /// Requested amount exceeds anti-whale daily operation cap (Issue #455).
    DailyCapExceeded = 206,
    /// A guarded section was re-entered (Issue #472).
    Reentrancy = 207,
}

impl From<AntiWhaleError> for MinterError {
    fn from(err: AntiWhaleError) -> Self {
        match err {
            AntiWhaleError::DailyCapExceeded
            | AntiWhaleError::OperationCapExceeded
            | AntiWhaleError::GuildContributionCapExceeded => MinterError::DailyCapExceeded,
            AntiWhaleError::ArithmeticOverflow => MinterError::ArithmeticOverflow,
            AntiWhaleError::InvalidAmount
            | AntiWhaleError::InvalidConfig
            | AntiWhaleError::Unauthorized
            | AntiWhaleError::AlreadyInitialized => MinterError::InvalidAmount,
        }
    }
}

impl crate::error_standard::StandardContractError for MinterError {
    fn descriptor(self) -> crate::error_standard::ErrorDescriptor {
        use crate::error_standard::ErrorKind;
        let (kind, retryable) = match self {
            Self::InvalidAmount => (ErrorKind::Validation, false),
            Self::RateLimitExceeded => (ErrorKind::ResourceLimit, true),
            Self::NoLayoutForShip | Self::NoResourceAtAnomaly => (ErrorKind::NotFound, false),
            Self::ArithmeticOverflow | Self::InsufficientBalance | Self::DailyCapExceeded => {
                (ErrorKind::ResourceLimit, false)
            }
            Self::Reentrancy => (ErrorKind::Conflict, false),
        };
        crate::error_standard::ErrorDescriptor {
            module: "resource_minter",
            code: self as u32,
            kind,
            retryable,
        }
    }
}

impl From<ReentrancyError> for MinterError {
    fn from(_: ReentrancyError) -> Self {
        MinterError::Reentrancy
    }
}

impl From<RateLimitError> for MinterError {
    fn from(_: RateLimitError) -> Self {
        MinterError::RateLimitExceeded
    }
}

// ── Contract ─────────────────────────────────────────────────
#[contract]
pub struct ResourceMinterContract;

#[contractimpl]
impl ResourceMinterContract {
    /// Mint `amount` units of `resource_type` for `caller`.
    ///
    /// Rate-limited to prevent spam (Issue #175).
    ///
    /// # Reentrancy
    /// Runs under the global reentrancy guard (Issue #472). Balance, supply
    /// and mint counters are all written before the `minted` event is
    /// published, so a nested call can never observe a half-applied mint.
    pub fn mint_resource(
        env: &Env,
        caller: Address,
        ship_id: u64,
        anomaly_index: u32,
        resource_type: ResourceType,
        amount: u64,
    ) -> Result<ResourceRecord, MinterError> {
        // ── Auth ───────────────────────────────────────────────
        caller.require_auth();

        // ── Reentrancy guard (Issue #472) ──────────────────────
        // The guard was dropped during the upstream merge (Issue #584);
        // restored so a nested call can never observe a half-applied mint.
        with_guard(env, || {
            mint_resource_unguarded(env, caller, ship_id, anomaly_index, resource_type, amount)
        })
    }

    /// Query the balance of `owner` for `resource_type`.
    pub fn balance(env: &Env, owner: Address, resource_type: ResourceType) -> u64 {
        env.storage()
            .persistent()
            .get(&MinterKey::Balance(owner, resource_type))
            .unwrap_or(0)
    }

    /// Total supply of a given resource type.
    pub fn total_supply(env: &Env, resource_type: ResourceType) -> u64 {
        env.storage()
            .persistent()
            .get(&MinterKey::TotalSupply(resource_type))
            .unwrap_or(0)
    }

    /// Cumulative amount ever minted for `resource_type`, ignoring burns.
    pub fn total_minted(env: &Env, resource_type: ResourceType) -> u64 {
        self::total_minted(env, &resource_type)
    }
}

/// Unguarded body of [`ResourceMinterContract::mint_resource`]; callers must
/// already hold the reentrancy lock.
fn mint_resource_unguarded(
    env: &Env,
    caller: Address,
    ship_id: u64,
    anomaly_index: u32,
    resource_type: ResourceType,
    amount: u64,
) -> Result<ResourceRecord, MinterError> {
    // ── Rate limit check (Issue #175) ──────────────────────
    check_rate_limit(env, &caller, Operation::ResourceMinting).map_err(MinterError::from)?;

    // ── Basic validation ───────────────────────────────────
    if amount == 0 {
        return Err(MinterError::InvalidAmount);
    }

    // ── Confirm anomaly exists for this ship ───────────────
    NebulaGen::has_anomaly(env.clone(), ship_id, anomaly_index).map_err(|e| match e {
        NebulaGenError::LayoutNotFound => MinterError::NoLayoutForShip,
        NebulaGenError::AnomalyOutOfBounds => MinterError::NoResourceAtAnomaly,
        _ => MinterError::NoLayoutForShip,
    })?;

    // ── Anti-Whale checks (Issue #455 / #502) ──────────────
    check_operation(env, &caller, OpKind::Mint, 1)?;
    let (effective_amount, _progressive_fee) = process_anti_whale_action(env, &caller, amount)?;

    // ── Update balances (checked: Issue #239) ──────────────
    let balance_key = MinterKey::Balance(caller.clone(), resource_type.clone());
    let current: u64 = env.storage().persistent().get(&balance_key).unwrap_or(0);
    let new_balance = current
        .checked_add(effective_amount)
        .ok_or(MinterError::ArithmeticOverflow)?;
    env.storage().persistent().set(&balance_key, &new_balance);

    let supply_key = MinterKey::TotalSupply(resource_type.clone());
    let supply: u64 = env.storage().persistent().get(&supply_key).unwrap_or(0);
    let new_supply = supply
        .checked_add(effective_amount)
        .ok_or(MinterError::ArithmeticOverflow)?;
    env.storage().persistent().set(&supply_key, &new_supply);

    // ── Cumulative mint counter (Issue #281) ───────────────
    // Unlike TotalSupply this is monotonic — burning reduces supply but
    // never the historical mint total, which is the denominator of the
    // deflation rate.
    let minted_key = MinterKey::TotalMinted(resource_type.clone());
    let minted: u64 = env.storage().persistent().get(&minted_key).unwrap_or(0);
    let new_minted = minted
        .checked_add(effective_amount)
        .ok_or(MinterError::ArithmeticOverflow)?;
    env.storage().persistent().set(&minted_key, &new_minted);

    let record = ResourceRecord {
        owner: caller.clone(),
        resource_type: resource_type.clone(),
        amount: effective_amount,
        minted_at: env.ledger().timestamp(),
    };

    // ── Emit event ─────────────────────────────────────────
    env.events().publish(
        (symbol_short!("Minter"), symbol_short!("minted")),
        (caller, resource_type, effective_amount),
    );

    Ok(record)
}

// ─────────────────────────────────────────────────────────────
// Supply-reducing primitives (Issue #281)
// ─────────────────────────────────────────────────────────────
//
// `token_burning` is the only intended caller. Keeping these here rather than
// in the burning module means the balance and supply ledgers stay owned by a
// single module, so every credit and debit goes through the same checked
// arithmetic.

/// Read a holder's balance without going through the contract entrypoint.
pub fn balance_of(env: &Env, owner: &Address, resource_type: &ResourceType) -> u64 {
    env.storage()
        .persistent()
        .get(&MinterKey::Balance(owner.clone(), resource_type.clone()))
        .unwrap_or(0)
}

/// Cumulative amount ever minted for `resource_type`.
pub fn total_minted(env: &Env, resource_type: &ResourceType) -> u64 {
    env.storage()
        .persistent()
        .get(&MinterKey::TotalMinted(resource_type.clone()))
        .unwrap_or(0)
}

/// Current circulating supply for `resource_type`.
pub fn circulating_supply(env: &Env, resource_type: &ResourceType) -> u64 {
    env.storage()
        .persistent()
        .get(&MinterKey::TotalSupply(resource_type.clone()))
        .unwrap_or(0)
}
