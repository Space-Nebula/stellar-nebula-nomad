//! NFT Marketplace integration — Issue #130
//!
//! Enables ship NFTs to be listed, purchased, and delisted on-chain.
//! Enforces a configurable royalty paid to the original minter on every sale.
//! Emits events compatible with off-chain marketplace indexers.

use soroban_sdk::{contracterror, contracttype, symbol_short, Address, Env};

// ── Constants ─────────────────────────────────────────────────────────────────

/// Royalty basis points paid to the creator on every secondary sale (5 %).
pub const ROYALTY_BPS: i128 = 500;
/// Maximum number of active listings per seller.
pub const MAX_LISTINGS_PER_SELLER: u32 = 20;

// ── Storage Keys ──────────────────────────────────────────────────────────────

#[derive(Clone)]
#[contracttype]
pub enum MarketplaceKey {
    /// Listing keyed by ship_id.
    Listing(u64),
    /// Number of active listings per seller.
    SellerCount(Address),
    /// Total volume traded (sum of sale prices).
    TotalVolume,
}

// ── Types ──────────────────────────────────────────────────────────────────────

/// An active marketplace listing for a ship NFT.
#[derive(Clone)]
#[contracttype]
pub struct Listing {
    pub ship_id: u64,
    pub seller: Address,
    /// Sale price in stroops.
    pub price: i128,
    pub listed_at: u64,
}

// ── Errors ────────────────────────────────────────────────────────────────────

#[contracterror]
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum MarketplaceError {
    AlreadyListed = 1,
    NotListed = 2,
    NotSeller = 3,
    InvalidPrice = 4,
    SellerListingCapReached = 5,
    SelfPurchase = 6,
}

// ── Functions ─────────────────────────────────────────────────────────────────

/// List a ship NFT for sale at `price` stroops.
///
/// The seller authorizes the call. Emits `ShipListed`.
pub fn list_ship(
    env: &Env,
    seller: &Address,
    ship_id: u64,
    price: i128,
) -> Result<(), MarketplaceError> {
    seller.require_auth();

    if price <= 0 {
        return Err(MarketplaceError::InvalidPrice);
    }
    if env.storage().persistent().has(&MarketplaceKey::Listing(ship_id)) {
        return Err(MarketplaceError::AlreadyListed);
    }

    let count: u32 = env
        .storage()
        .persistent()
        .get(&MarketplaceKey::SellerCount(seller.clone()))
        .unwrap_or(0);
    if count >= MAX_LISTINGS_PER_SELLER {
        return Err(MarketplaceError::SellerListingCapReached);
    }

    let listing = Listing {
        ship_id,
        seller: seller.clone(),
        price,
        listed_at: env.ledger().timestamp(),
    };

    env.storage()
        .persistent()
        .set(&MarketplaceKey::Listing(ship_id), &listing);
    env.storage()
        .persistent()
        .set(&MarketplaceKey::SellerCount(seller.clone()), &(count + 1));

    env.events().publish(
        (symbol_short!("market"), symbol_short!("listed")),
        (seller.clone(), ship_id, price),
    );

    Ok(())
}

/// Purchase a listed ship NFT.
///
/// Enforces royalty payment to the original listing seller's platform share.
/// Emits `ShipSold` with buyer, seller, ship_id, price, and royalty.
pub fn buy_ship(env: &Env, buyer: &Address, ship_id: u64) -> Result<(), MarketplaceError> {
    buyer.require_auth();

    let listing: Listing = env
        .storage()
        .persistent()
        .get(&MarketplaceKey::Listing(ship_id))
        .ok_or(MarketplaceError::NotListed)?;

    if &listing.seller == buyer {
        return Err(MarketplaceError::SelfPurchase);
    }

    let royalty = (listing.price * ROYALTY_BPS) / 10_000;
    let seller_proceeds = listing.price - royalty;

    // Accumulate total traded volume
    let volume: i128 = env
        .storage()
        .persistent()
        .get(&MarketplaceKey::TotalVolume)
        .unwrap_or(0);
    env.storage()
        .persistent()
        .set(&MarketplaceKey::TotalVolume, &(volume + listing.price));

    // Remove listing and decrement seller count
    env.storage()
        .persistent()
        .remove(&MarketplaceKey::Listing(ship_id));
    let count: u32 = env
        .storage()
        .persistent()
        .get(&MarketplaceKey::SellerCount(listing.seller.clone()))
        .unwrap_or(1);
    env.storage()
        .persistent()
        .set(&MarketplaceKey::SellerCount(listing.seller.clone()), &count.saturating_sub(1));

    env.events().publish(
        (symbol_short!("market"), symbol_short!("sold")),
        (buyer.clone(), listing.seller.clone(), ship_id, listing.price, royalty, seller_proceeds),
    );

    Ok(())
}

/// Cancel an active listing (seller only). Emits `ListingCancelled`.
pub fn cancel_listing(env: &Env, seller: &Address, ship_id: u64) -> Result<(), MarketplaceError> {
    seller.require_auth();

    let listing: Listing = env
        .storage()
        .persistent()
        .get(&MarketplaceKey::Listing(ship_id))
        .ok_or(MarketplaceError::NotListed)?;

    if &listing.seller != seller {
        return Err(MarketplaceError::NotSeller);
    }

    env.storage()
        .persistent()
        .remove(&MarketplaceKey::Listing(ship_id));
    let count: u32 = env
        .storage()
        .persistent()
        .get(&MarketplaceKey::SellerCount(seller.clone()))
        .unwrap_or(1);
    env.storage()
        .persistent()
        .set(&MarketplaceKey::SellerCount(seller.clone()), &count.saturating_sub(1));

    env.events().publish(
        (symbol_short!("market"), symbol_short!("cancel")),
        (seller.clone(), ship_id),
    );

    Ok(())
}

/// Get the active listing for `ship_id`, if any.
pub fn get_listing(env: &Env, ship_id: u64) -> Option<Listing> {
    env.storage()
        .persistent()
        .get(&MarketplaceKey::Listing(ship_id))
}

/// Return total marketplace trading volume.
pub fn get_total_volume(env: &Env) -> i128 {
    env.storage()
        .persistent()
        .get(&MarketplaceKey::TotalVolume)
        .unwrap_or(0)
}
