use soroban_sdk::{contracterror, contracttype, symbol_short, Address, Env, Symbol, Vec};

// ─── Configuration ─────────────────────────────────────────────────────────

/// Maximum number of fractions allowed per transaction (burst limit).
pub const MAX_FRACTIONS_PER_TX: u32 = 50;

/// Minimum share size in base units.
pub const MIN_SHARE_SIZE: u32 = 1;

// ─── Storage Keys ─────────────────────────────────────────────────────────

#[derive(Clone)]
#[contracttype]
pub enum DataKey {
    /// Admin address.
    FracResAdmin,
    /// Global share ID counter.
    ShareCounter,
    /// Fractional share data keyed by share ID.
    Share(u64),
    /// Owner's share balance: address -> list of share IDs.
    OwnerShares(Address),
    /// Resource metadata keyed by resource symbol.
    Resource(Symbol),
    /// Original resource that was fractionalized (resource_id -> owner).
    OriginalResourceOwner(u64),
    /// Open sale listing for a share (share_id -> ShareListing).
    FracShareListing(u64),
    /// Last traded price per unit for a resource type (price discovery).
    FracLastUnitPrice(Symbol),
}

// ─── Error Handling ───────────────────────────────────────────────────────

#[contracterror]
#[derive(Clone, Debug, PartialEq, Eq, Copy)]
#[repr(u32)]
pub enum FractionalError {
    /// Resource not found.
    ResourceNotFound = 1,
    /// Not enough shares for operation.
    InsufficientShares = 2,
    /// Invalid share count specified.
    InvalidShareCount = 3,
    /// Share size below minimum.
    ShareTooSmall = 4,
    /// Not the owner of the resource/shares.
    NotOwner = 5,
    /// Share does not exist.
    ShareNotFound = 6,
    /// Cannot merge incompatible shares.
    IncompatibleShares = 7,
    /// Unauthorized caller.
    Unauthorized = 8,
    /// Resource already fractionalized.
    AlreadyFractionalized = 9,
    /// Maximum fractions per transaction exceeded.
    MaxFractionsExceeded = 10,
    /// Listing price must be positive.
    InvalidPrice = 11,
    /// The share is not listed for sale.
    NotListed = 12,
}

impl crate::error_standard::StandardContractError for FractionalError {
    fn descriptor(self) -> crate::error_standard::ErrorDescriptor {
        use crate::error_standard::ErrorKind;
        let (kind, retryable) = match self {
            Self::ResourceNotFound | Self::ShareNotFound => (ErrorKind::NotFound, false),
            Self::InsufficientShares | Self::MaxFractionsExceeded => {
                (ErrorKind::ResourceLimit, false)
            }
            Self::InvalidShareCount
            | Self::ShareTooSmall
            | Self::IncompatibleShares
            | Self::InvalidPrice => (ErrorKind::Validation, false),
            Self::NotOwner | Self::Unauthorized => (ErrorKind::Authorization, false),
            Self::AlreadyFractionalized => (ErrorKind::Conflict, false),
            Self::NotListed => (ErrorKind::NotFound, false),
        };
        crate::error_standard::ErrorDescriptor {
            module: "fractional_resources",
            code: self as u32,
            kind,
            retryable,
        }
    }
}

// ─── Data Structures ────────────────────────────────────────────────────────

/// A fractional share of a resource.
#[derive(Clone, Debug, PartialEq, Eq)]
#[contracttype]
pub struct FractionalShare {
    pub share_id: u64,
    pub owner: Address,
    pub resource_type: Symbol,
    pub amount: u32,
    pub total_shares: u32,
    pub original_resource_id: u64,
}

/// Original resource before fractionalization.
#[derive(Clone, Debug, PartialEq, Eq)]
#[contracttype]
pub struct OriginalResource {
    pub resource_id: u64,
    pub owner: Address,
    pub resource_type: Symbol,
    pub total_amount: u32,
    pub is_fractionalized: bool,
}

/// A share offered for sale on the micro-market.
#[derive(Clone, Debug, PartialEq, Eq)]
#[contracttype]
pub struct ShareListing {
    pub share_id: u64,
    pub seller: Address,
    /// Asking price for the whole share.
    pub price: i128,
    pub listed_at: u64,
}

/// Configuration for fractionalization.
#[derive(Clone, Debug, PartialEq, Eq)]
#[contracttype]
pub struct FractionalConfig {
    pub min_share_size: u32,
    pub max_fractions_per_tx: u32,
    pub admin: Address,
}

impl FractionalConfig {
    pub fn new(_env: &Env, admin: &Address) -> Self {
        Self {
            min_share_size: MIN_SHARE_SIZE,
            max_fractions_per_tx: MAX_FRACTIONS_PER_TX,
            admin: admin.clone(),
        }
    }
}

// ─── Initialization ────────────────────────────────────────────────────────

/// Initialize the fractional resource system.
pub fn initialize(env: &Env, admin: &Address) -> Result<(), FractionalError> {
    admin.require_auth();

    env.storage().instance().set(&DataKey::FracResAdmin, admin);
    env.storage()
        .instance()
        .set::<DataKey, u64>(&DataKey::ShareCounter, &0);

    let config = FractionalConfig {
        admin: admin.clone(),
        min_share_size: MIN_SHARE_SIZE,
        max_fractions_per_tx: MAX_FRACTIONS_PER_TX,
    };
    env.storage()
        .instance()
        .set(&DataKey::Resource(Symbol::new(env, "config")), &config);

    env.events().publish(
        (symbol_short!("frac"), symbol_short!("init")),
        admin.clone(),
    );

    Ok(())
}

// ─── Core Fractionalization Logic ────────────────────────────────────────

/// Fractionalize a resource into divisible shares.
///
/// # Arguments
/// * `owner` - The resource owner (must authorize)
/// * `resource_type` - Type of resource to fractionalize
/// * `total_amount` - Total amount of resource to split
/// * `shares` - Number of shares to create (max 50 per tx)
///
/// # Returns
/// Vector of created share IDs
///
/// # Security
/// - Owner authorization required
/// - Atomic operation (all shares created or none)
/// - Minimum share size enforced
pub fn fractionalize_resource(
    env: &Env,
    owner: &Address,
    resource_type: Symbol,
    total_amount: u32,
    shares: u32,
) -> Result<Vec<u64>, FractionalError> {
    owner.require_auth();

    // Validate share count
    if shares == 0 || shares > MAX_FRACTIONS_PER_TX {
        return Err(FractionalError::InvalidShareCount);
    }

    // Calculate share amount
    let share_amount = total_amount / shares;
    if share_amount < MIN_SHARE_SIZE {
        return Err(FractionalError::ShareTooSmall);
    }

    // Check for remainder
    let remainder = total_amount % shares;
    if remainder != 0 {
        return Err(FractionalError::InvalidShareCount);
    }

    // Create original resource record
    let resource_id = next_resource_id(env);
    let original = OriginalResource {
        resource_id,
        owner: owner.clone(),
        resource_type: resource_type.clone(),
        total_amount,
        is_fractionalized: true,
    };

    env.storage()
        .persistent()
        .set(&DataKey::Resource(resource_type.clone()), &original);
    env.storage()
        .persistent()
        .set(&DataKey::OriginalResourceOwner(resource_id), owner);

    // Create fractional shares
    let mut share_ids = Vec::new(env);

    for _i in 0..shares {
        let share_id = next_share_id(env);
        let share = FractionalShare {
            share_id,
            owner: owner.clone(),
            resource_type: resource_type.clone(),
            amount: share_amount,
            total_shares: shares,
            original_resource_id: resource_id,
        };

        // Store share
        env.storage()
            .persistent()
            .set(&DataKey::Share(share_id), &share);

        // Add to owner's shares
        add_share_to_owner(env, owner, share_id);

        share_ids.push_back(share_id);
    }

    // Emit event
    env.events().publish(
        (symbol_short!("frac"), symbol_short!("fraczd")),
        (owner.clone(), resource_type, total_amount, shares),
    );

    Ok(share_ids)
}

/// Merge fractional shares back into a whole resource.
///
/// # Arguments
/// * `owner` - The share owner (must authorize)
/// * `share_ids` - Vector of share IDs to merge
///
/// # Returns
/// Total merged amount
///
/// # Security
/// - Owner authorization required
/// - All shares must belong to same owner
/// - All shares must be of same resource type
/// - Shares are burned after merge
pub fn merge_fractions(
    env: &Env,
    owner: &Address,
    share_ids: Vec<u64>,
) -> Result<u32, FractionalError> {
    owner.require_auth();

    if share_ids.is_empty() {
        return Err(FractionalError::InvalidShareCount);
    }

    let mut total_amount: u32 = 0;
    let mut expected_type: Option<Symbol> = None;
    let mut expected_original_id: Option<u64> = None;

    // Validate all shares before merging (atomic check)
    for i in 0..share_ids.len() {
        let share_id = share_ids.get(i).unwrap();
        let share: FractionalShare = env
            .storage()
            .persistent()
            .get(&DataKey::Share(share_id))
            .ok_or(FractionalError::ShareNotFound)?;

        // Verify ownership
        if share.owner != *owner {
            return Err(FractionalError::NotOwner);
        }

        // Verify compatibility (same resource type)
        match &expected_type {
            None => expected_type = Some(share.resource_type.clone()),
            Some(t) if *t != share.resource_type => {
                return Err(FractionalError::IncompatibleShares);
            }
            _ => {}
        }

        // Verify same original resource
        match expected_original_id {
            None => expected_original_id = Some(share.original_resource_id),
            Some(id) if id != share.original_resource_id => {
                return Err(FractionalError::IncompatibleShares);
            }
            _ => {}
        }

        total_amount += share.amount;
    }

    // Burn shares (delete from storage)
    for i in 0..share_ids.len() {
        let share_id = share_ids.get(i).unwrap();

        // Remove share data
        env.storage().persistent().remove(&DataKey::Share(share_id));

        // Remove from owner's shares
        remove_share_from_owner(env, owner, share_id);
    }

    // Update original resource
    let resource_type = expected_type.unwrap();
    let _original_id = expected_original_id.unwrap();

    let mut original: OriginalResource = env
        .storage()
        .persistent()
        .get(&DataKey::Resource(resource_type.clone()))
        .ok_or(FractionalError::ResourceNotFound)?;

    original.is_fractionalized = false;
    env.storage()
        .persistent()
        .set(&DataKey::Resource(resource_type), &original);

    // Emit event
    env.events().publish(
        (symbol_short!("frac"), symbol_short!("merged")),
        (owner.clone(), total_amount, share_ids.len()),
    );

    Ok(total_amount)
}

/// Transfer a fractional share to another owner.
pub fn transfer_share(
    env: &Env,
    from: &Address,
    to: &Address,
    share_id: u64,
) -> Result<FractionalShare, FractionalError> {
    from.require_auth();

    let mut share: FractionalShare = env
        .storage()
        .persistent()
        .get(&DataKey::Share(share_id))
        .ok_or(FractionalError::ShareNotFound)?;

    if share.owner != *from {
        return Err(FractionalError::NotOwner);
    }

    // A transferred share can no longer be sold under the old owner's listing.
    env.storage()
        .persistent()
        .remove(&DataKey::FracShareListing(share_id));

    // Update share ownership
    remove_share_from_owner(env, from, share_id);
    share.owner = to.clone();
    env.storage()
        .persistent()
        .set(&DataKey::Share(share_id), &share);
    add_share_to_owner(env, to, share_id);

    // Emit event
    env.events().publish(
        (symbol_short!("frac"), symbol_short!("xfer")),
        (from.clone(), to.clone(), share_id),
    );

    Ok(share)
}

// ─── Micro-trading ────────────────────────────────────────────────────────

/// Lists a share for sale at `price`. Re-listing updates the price.
pub fn list_share_for_sale(
    env: &Env,
    seller: &Address,
    share_id: u64,
    price: i128,
) -> Result<ShareListing, FractionalError> {
    seller.require_auth();
    if price <= 0 {
        return Err(FractionalError::InvalidPrice);
    }
    let share = get_share(env, share_id).ok_or(FractionalError::ShareNotFound)?;
    if share.owner != *seller {
        return Err(FractionalError::NotOwner);
    }
    let listing = ShareListing {
        share_id,
        seller: seller.clone(),
        price,
        listed_at: env.ledger().timestamp(),
    };
    env.storage()
        .persistent()
        .set(&DataKey::FracShareListing(share_id), &listing);
    env.events().publish(
        (symbol_short!("frac"), symbol_short!("listed")),
        (seller.clone(), share_id, price),
    );
    Ok(listing)
}

/// Removes a listing. Only the seller can cancel it.
pub fn cancel_share_listing(
    env: &Env,
    seller: &Address,
    share_id: u64,
) -> Result<(), FractionalError> {
    seller.require_auth();
    let listing = get_share_listing(env, share_id).ok_or(FractionalError::NotListed)?;
    if listing.seller != *seller {
        return Err(FractionalError::NotOwner);
    }
    env.storage()
        .persistent()
        .remove(&DataKey::FracShareListing(share_id));
    Ok(())
}

/// Buys a listed share: ownership moves to the buyer, the listing closes and
/// the sale updates the resource type's last unit price (price discovery).
/// Settlement of `price` happens in the trading layer; this records the trade.
/// Returns the price paid.
pub fn buy_listed_share(
    env: &Env,
    buyer: &Address,
    share_id: u64,
) -> Result<i128, FractionalError> {
    buyer.require_auth();
    let listing = get_share_listing(env, share_id).ok_or(FractionalError::NotListed)?;
    let mut share = get_share(env, share_id).ok_or(FractionalError::ShareNotFound)?;
    if share.owner != listing.seller {
        // Stale listing: the share changed hands another way.
        env.storage()
            .persistent()
            .remove(&DataKey::FracShareListing(share_id));
        return Err(FractionalError::NotListed);
    }
    if share.owner == *buyer {
        return Err(FractionalError::NotOwner);
    }

    remove_share_from_owner(env, &listing.seller, share_id);
    share.owner = buyer.clone();
    env.storage()
        .persistent()
        .set(&DataKey::Share(share_id), &share);
    add_share_to_owner(env, buyer, share_id);
    env.storage()
        .persistent()
        .remove(&DataKey::FracShareListing(share_id));

    let unit_price = listing.price / i128::from(share.amount.max(1));
    env.storage().persistent().set(
        &DataKey::FracLastUnitPrice(share.resource_type.clone()),
        &unit_price,
    );

    env.events().publish(
        (symbol_short!("frac"), symbol_short!("sold")),
        (listing.seller, buyer.clone(), share_id, listing.price),
    );
    Ok(listing.price)
}

/// Open listing for a share, if any.
pub fn get_share_listing(env: &Env, share_id: u64) -> Option<ShareListing> {
    env.storage()
        .persistent()
        .get(&DataKey::FracShareListing(share_id))
}

/// Last traded price per resource unit for `resource_type`, if it has traded.
pub fn get_last_share_price(env: &Env, resource_type: Symbol) -> Option<i128> {
    env.storage()
        .persistent()
        .get(&DataKey::FracLastUnitPrice(resource_type))
}

// ─── View Functions ───────────────────────────────────────────────────────

/// Get a fractional share by ID.
pub fn get_share(env: &Env, share_id: u64) -> Option<FractionalShare> {
    env.storage().persistent().get(&DataKey::Share(share_id))
}

/// Get all share IDs owned by an address.
pub fn get_owner_shares(env: &Env, owner: &Address) -> Vec<u64> {
    env.storage()
        .persistent()
        .get(&DataKey::OwnerShares(owner.clone()))
        .unwrap_or_else(|| Vec::new(env))
}

/// Get the total number of shares created.
pub fn get_total_shares(env: &Env) -> u64 {
    env.storage()
        .instance()
        .get(&DataKey::ShareCounter)
        .unwrap_or(0)
}

/// Get original resource data.
pub fn get_original_resource(env: &Env, resource_type: Symbol) -> Option<OriginalResource> {
    env.storage()
        .persistent()
        .get(&DataKey::Resource(resource_type))
}

/// Check if an address owns a specific share.
pub fn is_share_owner(env: &Env, owner: &Address, share_id: u64) -> bool {
    let share: Option<FractionalShare> = env.storage().persistent().get(&DataKey::Share(share_id));
    match share {
        Some(s) => s.owner == *owner,
        None => false,
    }
}

// ─── Internal Helpers ─────────────────────────────────────────────────────

fn next_share_id(env: &Env) -> u64 {
    let key = DataKey::ShareCounter;
    let current: u64 = env.storage().instance().get(&key).unwrap_or(0);
    let next = current + 1;
    env.storage().instance().set(&key, &next);
    next
}

fn next_resource_id(env: &Env) -> u64 {
    let key = DataKey::ShareCounter;
    let current: u64 = env.storage().instance().get(&key).unwrap_or(0);
    let next = current + 1;
    env.storage().instance().set(&key, &next);
    next
}

fn add_share_to_owner(env: &Env, owner: &Address, share_id: u64) {
    let key = DataKey::OwnerShares(owner.clone());
    let mut shares: Vec<u64> = env
        .storage()
        .persistent()
        .get(&key)
        .unwrap_or_else(|| Vec::new(env));
    shares.push_back(share_id);
    env.storage().persistent().set(&key, &shares);
}

fn remove_share_from_owner(env: &Env, owner: &Address, share_id: u64) {
    let key = DataKey::OwnerShares(owner.clone());
    let shares: Vec<u64> = env
        .storage()
        .persistent()
        .get(&key)
        .unwrap_or_else(|| Vec::new(env));

    let mut new_shares = Vec::new(env);
    for i in 0..shares.len() {
        let id = shares.get(i).unwrap();
        if id != share_id {
            new_shares.push_back(id);
        }
    }
    env.storage().persistent().set(&key, &new_shares);
}

// ─── Admin Functions ───────────────────────────────────────────────────────

/// Update fractionalization config (admin only).
pub fn update_config(
    env: &Env,
    admin: &Address,
    min_share_size: u32,
    max_fractions: u32,
) -> Result<FractionalConfig, FractionalError> {
    admin.require_auth();

    let stored_admin: Address = env
        .storage()
        .instance()
        .get(&DataKey::FracResAdmin)
        .ok_or(FractionalError::Unauthorized)?;

    if admin != &stored_admin {
        return Err(FractionalError::Unauthorized);
    }

    let config = FractionalConfig {
        admin: admin.clone(),
        min_share_size,
        max_fractions_per_tx: max_fractions,
    };

    env.storage()
        .persistent()
        .set(&DataKey::Resource(Symbol::new(env, "config")), &config);

    Ok(config)
}
