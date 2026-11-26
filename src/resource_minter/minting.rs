use super::types::*;
use soroban_sdk::{symbol_short, Address, Env, Symbol, Vec};


/// Debit `amount` from `owner`'s balance.
///
/// Does **not** require auth — the caller is responsible for having authorized
/// the holder. Returns the new balance, or `InsufficientBalance` if the debit
/// would go negative (checked, never wrapping).
pub fn debit_balance(
    env: &Env,
    owner: &Address,
    resource_type: &ResourceType,
    amount: u64,
) -> Result<u64, MinterError> {
    if amount == 0 {
        return Err(MinterError::InvalidAmount);
    }

    let key = MinterKey::Balance(owner.clone(), resource_type.clone());
    let current: u64 = env.storage().persistent().get(&key).unwrap_or(0);
    let new_balance = current
        .checked_sub(amount)
        .ok_or(MinterError::InsufficientBalance)?;
    env.storage().persistent().set(&key, &new_balance);

    Ok(new_balance)
}

/// Reduce the circulating supply of `resource_type` by `amount`.
///
/// Returns the new supply. Underflow is treated as `InsufficientBalance`: it
/// would mean the supply ledger disagreed with the sum of balances, and
/// silently saturating there would hide the inconsistency.
pub fn reduce_supply(
    env: &Env,
    resource_type: &ResourceType,
    amount: u64,
) -> Result<u64, MinterError> {
    if amount == 0 {
        return Err(MinterError::InvalidAmount);
    }

    let key = MinterKey::TotalSupply(resource_type.clone());
    let supply: u64 = env.storage().persistent().get(&key).unwrap_or(0);
    let new_supply = supply
        .checked_sub(amount)
        .ok_or(MinterError::InsufficientBalance)?;
    env.storage().persistent().set(&key, &new_supply);

    Ok(new_supply)
}

/// Move `amount` between two holders without touching supply counters.
///
/// A transfer is not a mint or a burn, so neither `TotalSupply` nor
/// `TotalMinted` changes — only the two balances do.
pub fn move_balance(
    env: &Env,
    from: &Address,
    to: &Address,
    resource_type: &ResourceType,
    amount: u64,
) -> Result<u64, MinterError> {
    if amount == 0 {
        return Err(MinterError::InvalidAmount);
    }
    if from == to {
        return Ok(balance_of(env, from, resource_type));
    }

    let from_key = MinterKey::Balance(from.clone(), resource_type.clone());
    let from_balance: u64 = env.storage().persistent().get(&from_key).unwrap_or(0);
    let remaining = from_balance
        .checked_sub(amount)
        .ok_or(MinterError::InsufficientBalance)?;

    let to_key = MinterKey::Balance(to.clone(), resource_type.clone());
    let to_balance: u64 = env.storage().persistent().get(&to_key).unwrap_or(0);
    let credited = to_balance
        .checked_add(amount)
        .ok_or(MinterError::ArithmeticOverflow)?;

    env.storage().persistent().set(&from_key, &remaining);
    env.storage().persistent().set(&to_key, &credited);

    Ok(credited)
}

/// Credit `amount` to `owner` without auth or rate limiting.
///
/// Test- and reward-path helper: used by the burn tests to seed balances and by
/// reward subsystems that grant resources they have already authorized.
pub fn credit_balance(
    env: &Env,
    owner: &Address,
    resource_type: &ResourceType,
    amount: u64,
) -> Result<u64, MinterError> {
    if amount == 0 {
        return Err(MinterError::InvalidAmount);
    }

    let key = MinterKey::Balance(owner.clone(), resource_type.clone());
    let current: u64 = env.storage().persistent().get(&key).unwrap_or(0);
    let new_balance = current
        .checked_add(amount)
        .ok_or(MinterError::ArithmeticOverflow)?;
    env.storage().persistent().set(&key, &new_balance);

    let supply_key = MinterKey::TotalSupply(resource_type.clone());
    let supply: u64 = env.storage().persistent().get(&supply_key).unwrap_or(0);
    env.storage().persistent().set(
        &supply_key,
        &supply
            .checked_add(amount)
            .ok_or(MinterError::ArithmeticOverflow)?,
    );

    let minted_key = MinterKey::TotalMinted(resource_type.clone());
    let minted: u64 = env.storage().persistent().get(&minted_key).unwrap_or(0);
    env.storage().persistent().set(
        &minted_key,
        &minted
            .checked_add(amount)
            .ok_or(MinterError::ArithmeticOverflow)?,
    );

    Ok(new_balance)
}

// ─────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────
// ─────────────────────────────────────────────────────────────────────────────
// Harvest + DEX auto-listing
// ─────────────────────────────────────────────────────────────────────────────
//
// `harvest_resources` converts a scanned nebula layout into on-chain resource
// balances, and `auto_list_on_dex` turns those balances into DEX offers. Both
// write to the same `ResourceKey::ResourceBalance(owner, asset)` ledger that
// `gifting_system` and `ship_upgrade` already use, so a harvested unit is
// immediately spendable on gifts and upgrades without a conversion step.
//
// This is deliberately separate from the `MinterKey` ledger behind
// `mint_resource`: harvesting is layout-derived and not rate-limited (the layout
// is the cost), whereas minting is caller-chosen and rate-limited.

/// Asset symbol for a harvested cell type, or `None` for non-resource cells.
///
/// `CellType::Empty` and `CellType::Star` carry no harvestable resource.
fn cell_type_to_asset(cell_type: &CellType) -> Option<Symbol> {
    match cell_type {
        CellType::StellarDust => Some(symbol_short!("dust")),
        CellType::Asteroid => Some(symbol_short!("ore")),
        CellType::GasCloud => Some(symbol_short!("gas")),
        CellType::DarkMatter => Some(symbol_short!("dark")),
        CellType::ExoticMatter => Some(symbol_short!("exotic")),
        CellType::Wormhole => Some(symbol_short!("worm")),
        CellType::Empty | CellType::Star => None,
    }
}

/// A single asset yielded by one harvest.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HarvestedResource {
    /// Asset symbol credited to the ship's owner.
    pub asset_id: Symbol,
    /// Units credited.
    pub amount: u32,
}

/// Outcome of a harvest pass over a layout.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HarvestResult {
    /// Ship the harvest was performed with.
    pub ship_id: u64,
    /// Per-asset breakdown, in layout cell order.
    pub resources: Vec<HarvestedResource>,
    /// Sum of every entry in `resources`.
    pub total_harvested: u32,
}

/// A live or cancelled DEX listing.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DexOffer {
    /// Unique, monotonically increasing offer ID.
    pub offer_id: u64,
    /// Seller whose `amount` is held in escrow. Only this address may cancel
    /// the offer, and the refund is always credited back to them.
    pub seller: Address,
    /// Asset symbol being sold.
    pub asset_id: Symbol,
    /// Units escrowed from the seller at listing time.
    pub amount: u32,
    /// Seller's floor price per unit. Always `> 0`.
    pub min_price: i128,
    /// `false` once cancelled.
    pub active: bool,
}

/// Errors from the harvest and DEX-listing paths.
///
/// Discriminants are part of the contract's public ABI — append new variants
/// rather than renumbering existing ones.
#[contracterror]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum HarvestError {
    /// No ship with the given ID exists.
    ShipNotFound = 1,
    /// The layout yielded no resources (all cells empty or non-resource).
    EmptyHarvest = 2,
    /// `min_price` was zero or negative.
    InvalidPrice = 3,
    /// The requested asset was not present in this harvest.
    AssetNotHarvested = 4,
    /// A checked arithmetic operation overflowed (Issue #239).
    PriceOverflow = 5,
    /// Generic DEX failure: unknown offer, already cancelled, or rate-limited.
    DexFailure = 6,
    /// Seller does not hold enough of `resource` to cover the listing.
    InsufficientBalance = 7,
    /// A guarded section was re-entered (Issue #472).
    Reentrancy = 8,
    /// Buyer and seller are the same address.
    SelfTrade = 9,
    /// The offer's price is above the buyer's `max_price`, or the order's
    /// price is below the seller's `min_price`.
    SlippageExceeded = 10,
    /// Requested amount is zero or larger than what the offer/order holds.
    InvalidAmount = 11,
    /// The limit order does not exist, is not a buy order, or is for a
    /// different resource.
    OrderUnavailable = 12,
}

impl crate::error_standard::StandardContractError for HarvestError {
    fn descriptor(self) -> crate::error_standard::ErrorDescriptor {
        use crate::error_standard::ErrorKind;
        let (kind, retryable) = match self {
            Self::ShipNotFound | Self::AssetNotHarvested => (ErrorKind::NotFound, false),
            Self::EmptyHarvest | Self::InvalidPrice => (ErrorKind::Validation, false),
            Self::PriceOverflow | Self::DexFailure => (ErrorKind::Internal, false),
            Self::InsufficientBalance => (ErrorKind::ResourceLimit, false),
            Self::Reentrancy | Self::SelfTrade => (ErrorKind::Conflict, false),
            Self::SlippageExceeded => (ErrorKind::Conflict, true),
            Self::InvalidAmount => (ErrorKind::Validation, false),
            Self::OrderUnavailable => (ErrorKind::NotFound, false),
        };
        crate::error_standard::ErrorDescriptor {
            module: "resource_minter",
            code: self as u32,
            kind,
            retryable,
        }
    }
}

impl From<ReentrancyError> for HarvestError {
    fn from(_: ReentrancyError) -> Self {
        HarvestError::Reentrancy
    }
}

/// Allocate the next DEX offer ID.
///
/// Shared with [`crate::dex_integration`] so both listing paths draw from one
/// counter and IDs can never collide.
pub(crate) fn next_dex_offer_id(env: &Env) -> Result<u64, HarvestError> {
    let current: u64 = env
        .storage()
        .instance()
        .get(&ResourceKey::DexOfferCounter)
        .unwrap_or(0);
    let next = current.checked_add(1).ok_or(HarvestError::PriceOverflow)?;
    env.storage()
        .instance()
        .set(&ResourceKey::DexOfferCounter, &next);
    Ok(next)
}

/// Highest DEX offer ID allocated so far (`0` when none exist). Offer IDs are
/// dense, so `1..=dex_offer_count()` enumerates every offer ever created.
pub(crate) fn dex_offer_count(env: &Env) -> u64 {
    env.storage()
        .instance()
        .get(&ResourceKey::DexOfferCounter)
        .unwrap_or(0)
}

/// Read a holder's harvest balance for `asset`.
pub fn resource_balance(env: &Env, owner: &Address, asset: &Symbol) -> u32 {
    env.storage()
        .instance()
        .get(&ResourceKey::ResourceBalance(owner.clone(), asset.clone()))
        .unwrap_or(0)
}

/// Credit `amount` of `asset` to `owner`, rejecting overflow (Issue #239).
pub fn credit_resource_balance(
    env: &Env,
    owner: &Address,
    asset: &Symbol,
    amount: u32,
) -> Result<u32, HarvestError> {
    let key = ResourceKey::ResourceBalance(owner.clone(), asset.clone());
    let current: u32 = env.storage().instance().get(&key).unwrap_or(0);
    let next = current
        .checked_add(amount)
        .ok_or(HarvestError::PriceOverflow)?;
    env.storage().instance().set(&key, &next);
    Ok(next)
}

/// Debit `amount` of `asset` from `owner`, failing with `InsufficientBalance`
/// when the holder has less than `amount`.
pub fn debit_resource_balance(
    env: &Env,
    owner: &Address,
    asset: &Symbol,
    amount: u32,
) -> Result<u32, HarvestError> {
    let key = ResourceKey::ResourceBalance(owner.clone(), asset.clone());
    let current: u32 = env.storage().instance().get(&key).unwrap_or(0);
    let next = current
        .checked_sub(amount)
        .ok_or(HarvestError::InsufficientBalance)?;
    env.storage().instance().set(&key, &next);
    Ok(next)
}

/// Harvest every resource-bearing cell in `layout` and credit the ship's owner.
///
/// The ship is resolved from `ship_id` rather than trusted from the caller, so
/// harvested units always land with the current NFT owner. Auth is not required:
/// the layout is the caller-supplied cost, and the reward is bounded by the
/// layout's own contents — a caller can only ever pay gas to credit a ship owner
/// (who may be somebody else).
///
/// Returns [`HarvestError::ShipNotFound`] if `ship_id` does not exist and
/// [`HarvestError::EmptyHarvest`] if the layout yields nothing.
///
/// # Errors
/// See [`HarvestError`].
///
/// # Reentrancy
/// Runs under the global reentrancy guard (Issue #472): every check and state
/// effect completes while the lock is held, following checks-effects-
/// interactions, so a nested call into any guarded entry point while this one
/// is in flight is rejected with a `Reentrancy` error.
pub fn harvest_resources(
    env: &Env,
    ship_id: u64,
    layout: &NebulaLayout,
) -> Result<HarvestResult, HarvestError> {
    with_guard(env, || harvest_resources_unguarded(env, ship_id, layout))
}

/// Unguarded body of [`harvest_resources`]; callers must already hold the
/// reentrancy lock (e.g. another guarded entry point composing it).
pub(crate) fn harvest_resources_unguarded(
    env: &Env,
    ship_id: u64,
    layout: &NebulaLayout,
) -> Result<HarvestResult, HarvestError> {
    let ship = crate::ship_nft::get_ship(env, ship_id).map_err(|_| HarvestError::ShipNotFound)?;

    // Pass 1: collect the raw yield so diminishing returns are applied once
    // per harvest (one storage round-trip) rather than once per cell.
    let mut raw: Vec<HarvestedResource> = Vec::new(env);
    let mut raw_total: u32 = 0;
    for i in 0..layout.cells.len() {
        let Some(cell) = layout.cells.get(i) else {
            continue;
        };
        let Some(asset_id) = cell_type_to_asset(&cell.cell_type) else {
            continue;
        };
        if cell.energy == 0 {
            continue;
        }
        raw.push_back(HarvestedResource {
            asset_id,
            amount: cell.energy,
        });
        raw_total = raw_total
            .checked_add(cell.energy)
            .ok_or(HarvestError::PriceOverflow)?;
    }

    if raw_total == 0 {
        return Err(HarvestError::EmptyHarvest);
    }

    // Anti-whale diminishing returns on the owner's daily gathering; the
    // effective total is spread across cells pro rata (floored).
    let effective_total = apply_gathering(env, &ship.owner, u64::from(raw_total))
        .map_err(|_| HarvestError::PriceOverflow)?;

    let mut resources: Vec<HarvestedResource> = Vec::new(env);
    let mut total_harvested: u32 = 0;
    for i in 0..raw.len() {
        let Some(entry) = raw.get(i) else {
            continue;
        };
        // Pro-rata scaling: multiply before dividing to preserve precision,
        // but guard both the product and the (non-zero) denominator.
        let scaled = crate::safe_math::checked_mul_div_u64(
            u64::from(entry.amount),
            effective_total,
            u64::from(raw_total),
        )
        .ok_or(HarvestError::PriceOverflow)?;
        let amount = u32::try_from(scaled).unwrap_or(u32::MAX);
        if amount == 0 {
            continue;
        }
        resources.push_back(HarvestedResource {
            asset_id: entry.asset_id.clone(),
            amount,
        });
        total_harvested = total_harvested
            .checked_add(amount)
            .ok_or(HarvestError::PriceOverflow)?;
        credit_resource_balance(env, &ship.owner, &entry.asset_id, amount)?;
    }

    if total_harvested == 0 {
        return Err(HarvestError::EmptyHarvest);
    }

    env.events().publish(
        (symbol_short!("harvest"), symbol_short!("done")),
        (ship_id, total_harvested),
    );

    Ok(HarvestResult {
        ship_id,
        resources,
        total_harvested,
    })
}

/// List the caller's entire balance of `resource` on the DEX.
///
/// Escrows the full current balance as `amount` and debits it from
/// `ResourceKey::ResourceBalance`, so a listed offer cannot be double-sold.
/// Cancelling via [`crate::dex_integration::cancel_listing`] releases the escrow.
///
/// `min_price` must be positive. [`HarvestError::InsufficientBalance`] is
/// returned when the caller holds nothing of `resource`.
///
/// # Errors
/// See [`HarvestError`].
///
/// # Reentrancy
/// Runs under the global reentrancy guard (Issue #472): every check and state
/// effect completes while the lock is held, following checks-effects-
/// interactions, so a nested call into any guarded entry point while this one
/// is in flight is rejected with a `Reentrancy` error.
pub fn auto_list_on_dex(
    env: &Env,
    player: &Address,
    resource: &Symbol,
    min_price: i128,
) -> Result<DexOffer, HarvestError> {
    with_guard(env, || {
        auto_list_on_dex_unguarded(env, player, resource, min_price)
    })
}

/// Unguarded body of [`auto_list_on_dex`]; callers must already hold the
/// reentrancy lock (e.g. another guarded entry point composing it).
fn auto_list_on_dex_unguarded(
    env: &Env,
    player: &Address,
    resource: &Symbol,
    min_price: i128,
) -> Result<DexOffer, HarvestError> {
    player.require_auth();

    if min_price <= 0 {
        return Err(HarvestError::InvalidPrice);
    }

    let amount = resource_balance(env, player, resource);
    if amount == 0 {
        return Err(HarvestError::InsufficientBalance);
    }

    // Escrow: clear the balance this offer is about to sell. The `0u32` suffix
    // matters — an unsuffixed `0` would infer as `i32` and write a value the
    // `u32` reads in `resource_balance` cannot convert back from.
    let balance_key = ResourceKey::ResourceBalance(player.clone(), resource.clone());
    env.storage().instance().set(&balance_key, &0u32);

    let offer_id = next_dex_offer_id(env)?;
    let offer = DexOffer {
        offer_id,
        seller: player.clone(),
        asset_id: resource.clone(),
        amount,
        min_price,
        active: true,
    };

    env.storage()
        .instance()
        .set(&ResourceKey::DexOffer(offer_id), &offer);
    crate::dex_integration::note_listing_opened(env, player);

    env.events().publish(
        (symbol_short!("dex"), symbol_short!("listed")),
        (
            offer_id,
            player.clone(),
            resource.clone(),
            amount,
            min_price,
        ),
    );

    Ok(offer)
}

/// Read a DEX offer by ID.
pub fn get_dex_offer(env: &Env, offer_id: u64) -> Option<DexOffer> {
    env.storage()
        .instance()
        .get(&ResourceKey::DexOffer(offer_id))
}

