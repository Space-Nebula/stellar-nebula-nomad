//! Skin trading marketplace (#552, #643).
//!
//! Buy, sell, and barter cosmetic skins with rarity-aware price floors,
//! pre-purchase previews, search filters, escrow, and marketplace analytics.

use crate::ship_customization::{
    get_skin, set_tradeable, transfer_skin_internal, ShipSkin, SkinError, SkinRarity,
};
use crate::skins::{preview_skin, rarity_floor_price, SkinPreview};
use soroban_sdk::{contracterror, contracttype, symbol_short, Address, Env, Vec};

// ─── Marketplace Types ────────────────────────────────────────────────────────

#[derive(Clone, Debug, PartialEq, Eq)]
#[contracttype]
pub enum ListingStatus {
    Active,
    Sold,
    Cancelled,
    Expired,
}

#[derive(Clone, Debug, PartialEq, Eq)]
#[contracttype]
pub enum TradeOfferStatus {
    Pending,
    Accepted,
    Cancelled,
    Expired,
}

// ─── Storage Keys ─────────────────────────────────────────────────────────────

#[derive(Clone)]
#[contracttype]
pub enum MarketplaceKey {
    SkinMarketListing(u64),
    PlayerListings(Address),
    /// skin_id -> active listing_id
    SkinListing(u64),
    /// Monotonic listing ID counter.
    ListingCounter,
    /// Every listing ever created, used for filtered searches.
    AllListings,
    /// skin_id -> pending trade offer ID
    OfferedSkinTrade(u64),
    /// skin_id -> pending trade offer ID
    RequestedSkinTrade(u64),
    TradeOffer(u64),
    TradeOfferCounter,
    /// Cumulative settled trade volume, in stroops.
    TotalVolume,
    /// Number of settled sales.
    SalesCount,
}

// ─── Data Structs ─────────────────────────────────────────────────────────────

#[derive(Clone, Debug, PartialEq)]
#[contracttype]
pub struct SkinListing {
    pub id: u64,
    pub skin_id: u64,
    pub seller: Address,
    pub price: i128,
    pub rarity: SkinRarity,
    /// Full render payload so a buyer can preview before purchasing.
    pub preview: SkinPreview,
    pub status: ListingStatus,
    pub created_at: u64,
    pub expires_at: u64,
    pub buyer: Option<Address>,
}

#[derive(Clone, Debug, PartialEq)]
#[contracttype]
pub struct SkinTradeOffer {
    pub offer_id: u64,
    pub proposer: Address,
    pub offered_skin_id: u64,
    pub requested_skin_id: u64,
    pub status: TradeOfferStatus,
    pub created_at: u64,
    pub expires_at: u64,
}

#[derive(Clone, Debug, PartialEq)]
#[contracttype]
pub struct MarketplaceStats {
    pub total_listings: u32,
    pub active_listings: u32,
    pub total_volume: i128,
    pub avg_price: i128,
}

// ─── Errors ───────────────────────────────────────────────────────────────────

#[contracterror]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum MarketplaceError {
    ListingNotFound = 1,
    AlreadyListed = 2,
    NotSeller = 3,
    InsufficientFunds = 4,
    ListingExpired = 5,
    CannotBuyOwnListing = 6,
    SkinNotFound = 7,
    NotSkinOwner = 8,
    InvalidPrice = 9,
    PriceBelowFloor = 10,
    SkinNotTradeable = 11,
    TradeOfferNotFound = 12,
    TradeOfferNotPending = 13,
    CannotTradeOwnSkin = 14,
    NotTradeTarget = 15,
    TradeOfferExpired = 16,
    InvalidDuration = 17,
}

impl crate::error_standard::StandardContractError for MarketplaceError {
    fn descriptor(self) -> crate::error_standard::ErrorDescriptor {
        crate::error_standard::ErrorDescriptor {
            module: "skin_marketplace",
            code: self as u32,
            kind: crate::error_standard::ErrorKind::Validation,
            retryable: false,
        }
    }
}


// ─── Internal Helpers ─────────────────────────────────────────────────────────

fn map_skin_error(error: SkinError) -> MarketplaceError {
    match error {
        SkinError::SkinNotFound => MarketplaceError::SkinNotFound,
        SkinError::NotOwner => MarketplaceError::NotSkinOwner,
        SkinError::AlreadyApplied => MarketplaceError::SkinNotTradeable,
        _ => MarketplaceError::SkinNotTradeable,
    }
}

fn load_skin(env: &Env, skin_id: u64) -> Result<ShipSkin, MarketplaceError> {
    get_skin(env, skin_id).ok_or(MarketplaceError::SkinNotFound)
}

fn next_listing_id(env: &Env) -> u64 {
    let current: u64 = env
        .storage()
        .persistent()
        .get(&MarketplaceKey::ListingCounter)
        .unwrap_or(0);
    let next = current + 1;
    env.storage()
        .persistent()
        .set(&MarketplaceKey::ListingCounter, &next);
    next
}

fn next_trade_offer_id(env: &Env) -> u64 {
    let current: u64 = env
        .storage()
        .persistent()
        .get(&MarketplaceKey::TradeOfferCounter)
        .unwrap_or(0);
    let next = current + 1;
    env.storage()
        .persistent()
        .set(&MarketplaceKey::TradeOfferCounter, &next);
    next
}

fn push_listing_index(env: &Env, key: &MarketplaceKey, listing_id: u64) {
    let mut ids: Vec<u64> = env
        .storage()
        .persistent()
        .get(key)
        .unwrap_or_else(|| Vec::new(env));
    ids.push_back(listing_id);
    env.storage().persistent().set(key, &ids);
}

fn remove_listing_index(env: &Env, key: &MarketplaceKey, listing_id: u64) {
    let ids: Vec<u64> = env
        .storage()
        .persistent()
        .get(key)
        .unwrap_or_else(|| Vec::new(env));
    let mut remaining = Vec::new(env);
    for id in ids.iter() {
        if id != listing_id {
            remaining.push_back(id);
        }
    }
    env.storage().persistent().set(key, &remaining);
}

/// Mark an overdue listing expired and release its escrow.
fn expire_listing_internal(
    env: &Env,
    listing_id: u64,
    mut listing: SkinListing,
) -> Result<SkinListing, MarketplaceError> {
    listing.status = ListingStatus::Expired;
    env.storage()
        .persistent()
        .set(&MarketplaceKey::SkinMarketListing(listing_id), &listing);
    env.storage()
        .persistent()
        .remove(&MarketplaceKey::SkinListing(listing.skin_id));
    let _ = set_tradeable(env, listing.skin_id, true);
    Ok(listing)
}

fn ensure_active_listing(
    env: &Env,
    listing_id: u64,
) -> Result<SkinListing, MarketplaceError> {
    let listing: SkinListing = env
        .storage()
        .persistent()
        .get(&MarketplaceKey::SkinMarketListing(listing_id))
        .ok_or(MarketplaceError::ListingNotFound)?;

    if listing.status != ListingStatus::Active {
        return Err(MarketplaceError::ListingNotFound);
    }

    if env.ledger().timestamp() > listing.expires_at {
        expire_listing_internal(env, listing_id, listing)?;
        return Err(MarketplaceError::ListingExpired);
    }

    Ok(listing)
}

fn add_volume(env: &Env, price: i128) {
    let volume: i128 = env
        .storage()
        .persistent()
        .get(&MarketplaceKey::TotalVolume)
        .unwrap_or(0);
    let sales: u64 = env
        .storage()
        .persistent()
        .get(&MarketplaceKey::SalesCount)
        .unwrap_or(0);
    env.storage()
        .persistent()
        .set(&MarketplaceKey::TotalVolume, &(volume + price));
    env.storage()
        .persistent()
        .set(&MarketplaceKey::SalesCount, &(sales + 1));
}

// ─── Fixed-price listings ─────────────────────────────────────────────────────

/// List a skin for sale at or above its rarity floor.
///
/// The skin is escrowed by clearing its `tradeable` flag, so it cannot be
/// transferred or re-listed while the listing is open.
pub fn list_skin(
    env: &Env,
    seller: &Address,
    skin_id: u64,
    price: i128,
    duration_hours: u64,
) -> Result<u64, MarketplaceError> {
    seller.require_auth();

    if price <= 0 {
        return Err(MarketplaceError::InvalidPrice);
    }
    if duration_hours == 0 {
        return Err(MarketplaceError::InvalidDuration);
    }

    let skin = load_skin(env, skin_id)?;
    if skin.owner != *seller {
        return Err(MarketplaceError::NotSkinOwner);
    }
    if !skin.tradeable {
        return Err(MarketplaceError::SkinNotTradeable);
    }
    if price < rarity_floor_price(&skin.rarity) {
        return Err(MarketplaceError::PriceBelowFloor);
    }

    if env
        .storage()
        .persistent()
        .has(&MarketplaceKey::SkinListing(skin_id))
    {
        return Err(MarketplaceError::AlreadyListed);
    }
    if env
        .storage()
        .persistent()
        .has(&MarketplaceKey::OfferedSkinTrade(skin_id))
        || env
            .storage()
            .persistent()
            .has(&MarketplaceKey::RequestedSkinTrade(skin_id))
    {
        return Err(MarketplaceError::AlreadyListed);
    }

    // Escrow: block transfers until the listing settles or is cancelled.
    set_tradeable(env, skin_id, false).map_err(map_skin_error)?;

    let id = next_listing_id(env);
    let timestamp = env.ledger().timestamp();
    let listing = SkinListing {
        id,
        skin_id,
        seller: seller.clone(),
        price,
        rarity: skin.rarity.clone(),
        preview: preview_skin(env, &skin),
        status: ListingStatus::Active,
        created_at: timestamp,
        expires_at: timestamp + duration_hours * 3_600,
        buyer: None,
    };

    env.storage()
        .persistent()
        .set(&MarketplaceKey::SkinMarketListing(id), &listing);
    env.storage()
        .persistent()
        .set(&MarketplaceKey::SkinListing(skin_id), &id);
    push_listing_index(env, &MarketplaceKey::AllListings, id);
    push_listing_index(env, &MarketplaceKey::PlayerListings(seller.clone()), id);

    env.events().publish(
        (symbol_short!("skin"), symbol_short!("listed")),
        (seller.clone(), skin_id, id, price),
    );

    Ok(id)
}

/// Buy a listed skin. Settlement transfers the NFT to the buyer.
pub fn buy_skin(env: &Env, buyer: &Address, listing_id: u64) -> Result<(), MarketplaceError> {
    buyer.require_auth();

    let mut listing = ensure_active_listing(env, listing_id)?;

    if listing.seller == *buyer {
        return Err(MarketplaceError::CannotBuyOwnListing);
    }

    transfer_skin_internal(env, listing.skin_id, buyer).map_err(map_skin_error)?;
    // The buyer receives an unencumbered skin.
    let _ = set_tradeable(env, listing.skin_id, true);

    listing.status = ListingStatus::Sold;
    listing.buyer = Some(buyer.clone());
    env.storage()
        .persistent()
        .set(&MarketplaceKey::SkinMarketListing(listing_id), &listing);

    env.storage()
        .persistent()
        .remove(&MarketplaceKey::SkinListing(listing.skin_id));
    remove_listing_index(
        env,
        &MarketplaceKey::PlayerListings(listing.seller.clone()),
        listing_id,
    );
    add_volume(env, listing.price);

    env.events().publish(
        (symbol_short!("skin"), symbol_short!("sold")),
        (buyer.clone(), listing.seller.clone(), listing.skin_id, listing.price),
    );

    Ok(())
}

/// Cancel an active listing and return the skin to its seller.
pub fn cancel_listing(
    env: &Env,
    seller: &Address,
    listing_id: u64,
) -> Result<(), MarketplaceError> {
    seller.require_auth();

    let mut listing = ensure_active_listing(env, listing_id)?;
    if listing.seller != *seller {
        return Err(MarketplaceError::NotSeller);
    }

    listing.status = ListingStatus::Cancelled;
    env.storage()
        .persistent()
        .set(&MarketplaceKey::SkinMarketListing(listing_id), &listing);
    env.storage()
        .persistent()
        .remove(&MarketplaceKey::SkinListing(listing.skin_id));
    remove_listing_index(
        env,
        &MarketplaceKey::PlayerListings(seller.clone()),
        listing_id,
    );
    let _ = set_tradeable(env, listing.skin_id, true);

    Ok(())
}

/// Update an active listing's asking price, respecting the rarity floor.
pub fn update_listing_price(
    env: &Env,
    seller: &Address,
    listing_id: u64,
    new_price: i128,
) -> Result<(), MarketplaceError> {
    seller.require_auth();

    if new_price <= 0 {
        return Err(MarketplaceError::InvalidPrice);
    }

    let mut listing = ensure_active_listing(env, listing_id)?;
    if listing.seller != *seller {
        return Err(MarketplaceError::NotSeller);
    }
    if new_price < rarity_floor_price(&listing.rarity) {
        return Err(MarketplaceError::PriceBelowFloor);
    }

    listing.price = new_price;
    env.storage()
        .persistent()
        .set(&MarketplaceKey::SkinMarketListing(listing_id), &listing);

    env.events().publish(
        (symbol_short!("skin"), symbol_short!("reprice")),
        (seller.clone(), listing_id, new_price),
    );

    Ok(())
}

/// Permissionless cleanup of an overdue listing.
pub fn expire_listing(env: &Env, listing_id: u64) -> Result<(), MarketplaceError> {
    let listing: SkinListing = env
        .storage()
        .persistent()
        .get(&MarketplaceKey::SkinMarketListing(listing_id))
        .ok_or(MarketplaceError::ListingNotFound)?;

    if listing.status != ListingStatus::Active {
        return Err(MarketplaceError::ListingNotFound);
    }
    if env.ledger().timestamp() <= listing.expires_at {
        return Err(MarketplaceError::ListingNotFound);
    }

    let seller = listing.seller.clone();
    expire_listing_internal(env, listing_id, listing)?;
    remove_listing_index(env, &MarketplaceKey::PlayerListings(seller), listing_id);
    Ok(())
}

/// Get a listing by ID.
pub fn get_listing(env: &Env, listing_id: u64) -> Option<SkinListing> {
    env.storage()
        .persistent()
        .get(&MarketplaceKey::SkinMarketListing(listing_id))
}

/// Get the active listing for a skin, if any.
pub fn get_active_listing(env: &Env, skin_id: u64) -> Option<SkinListing> {
    let listing_id: u64 = env
        .storage()
        .persistent()
        .get(&MarketplaceKey::SkinListing(skin_id))?;
    get_listing(env, listing_id)
}

/// All listings created by a seller (including historical ones).
pub fn get_seller_listings(env: &Env, seller: &Address) -> Vec<SkinListing> {
    let ids: Vec<u64> = env
        .storage()
        .persistent()
        .get(&MarketplaceKey::PlayerListings(seller.clone()))
        .unwrap_or_else(|| Vec::new(env));

    let mut listings = Vec::new(env);
    for id in ids.iter() {
        if let Some(listing) = get_listing(env, id) {
            listings.push_back(listing);
        }
    }
    listings
}

/// Search active listings whose asking price is at most `max_price`.
pub fn search_by_price(env: &Env, max_price: i128) -> Vec<SkinListing> {
    let ids: Vec<u64> = env
        .storage()
        .persistent()
        .get(&MarketplaceKey::AllListings)
        .unwrap_or_else(|| Vec::new(env));

    let now = env.ledger().timestamp();
    let mut matches = Vec::new(env);
    for id in ids.iter() {
        if let Some(listing) = get_listing(env, id) {
            if listing.status == ListingStatus::Active
                && now <= listing.expires_at
                && listing.price <= max_price
            {
                matches.push_back(listing);
            }
        }
    }
    matches
}

/// Search active listings within an inclusive price range and rarity.
pub fn search_skins(
    env: &Env,
    min_price: i128,
    max_price: i128,
    rarity: SkinRarity,
) -> Vec<SkinListing> {
    let ids: Vec<u64> = env
        .storage()
        .persistent()
        .get(&MarketplaceKey::AllListings)
        .unwrap_or_else(|| Vec::new(env));

    let now = env.ledger().timestamp();
    let mut matches = Vec::new(env);
    for id in ids.iter() {
        if let Some(listing) = get_listing(env, id) {
            if listing.status == ListingStatus::Active
                && now <= listing.expires_at
                && listing.price >= min_price
                && listing.price <= max_price
                && listing.rarity == rarity
            {
                matches.push_back(listing);
            }
        }
    }
    matches
}

/// Preview a listing before buying it.
pub fn get_listing_preview(env: &Env, listing_id: u64) -> Option<SkinPreview> {
    let listing = get_listing(env, listing_id)?;
    let skin = get_skin(env, listing.skin_id)?;
    Some(preview_skin(env, &skin))
}

/// Aggregate marketplace statistics.
pub fn get_marketplace_stats(env: &Env) -> MarketplaceStats {
    let ids: Vec<u64> = env
        .storage()
        .persistent()
        .get(&MarketplaceKey::AllListings)
        .unwrap_or_else(|| Vec::new(env));
    let now = env.ledger().timestamp();

    let mut active = 0u32;
    for id in ids.iter() {
        if let Some(listing) = get_listing(env, id) {
            if listing.status == ListingStatus::Active && now <= listing.expires_at {
                active += 1;
            }
        }
    }

    let volume: i128 = env
        .storage()
        .persistent()
        .get(&MarketplaceKey::TotalVolume)
        .unwrap_or(0);
    let sales: u64 = env
        .storage()
        .persistent()
        .get(&MarketplaceKey::SalesCount)
        .unwrap_or(0);
    let avg_price = if sales == 0 { 0 } else { volume / sales as i128 };

    MarketplaceStats {
        total_listings: ids.len(),
        active_listings: active,
        total_volume: volume,
        avg_price,
    }
}

// ─── Player-to-player barter ──────────────────────────────────────────────────

/// Offer one owned skin for another player's skin.
///
/// The offered skin is escrowed (made non-tradeable) while the offer is open;
/// the requested skin is only checked for availability, not escrowed.
pub fn create_trade_offer(
    env: &Env,
    proposer: &Address,
    offered_skin_id: u64,
    requested_skin_id: u64,
    duration_hours: u64,
) -> Result<u64, MarketplaceError> {
    proposer.require_auth();

    if offered_skin_id == requested_skin_id {
        return Err(MarketplaceError::CannotTradeOwnSkin);
    }
    if duration_hours == 0 {
        return Err(MarketplaceError::InvalidDuration);
    }

    let offered = load_skin(env, offered_skin_id)?;
    if offered.owner != *proposer {
        return Err(MarketplaceError::NotSkinOwner);
    }
    if !offered.tradeable {
        return Err(MarketplaceError::SkinNotTradeable);
    }

    let requested = load_skin(env, requested_skin_id)?;
    if !requested.tradeable {
        return Err(MarketplaceError::SkinNotTradeable);
    }

    for key in [
        MarketplaceKey::SkinListing(offered_skin_id),
        MarketplaceKey::SkinListing(requested_skin_id),
        MarketplaceKey::OfferedSkinTrade(offered_skin_id),
        MarketplaceKey::OfferedSkinTrade(requested_skin_id),
        MarketplaceKey::RequestedSkinTrade(offered_skin_id),
        MarketplaceKey::RequestedSkinTrade(requested_skin_id),
    ] {
        if env.storage().persistent().has(&key) {
            return Err(MarketplaceError::AlreadyListed);
        }
    }

    set_tradeable(env, offered_skin_id, false).map_err(map_skin_error)?;

    let offer_id = next_trade_offer_id(env);
    let now = env.ledger().timestamp();
    let offer = SkinTradeOffer {
        offer_id,
        proposer: proposer.clone(),
        offered_skin_id,
        requested_skin_id,
        status: TradeOfferStatus::Pending,
        created_at: now,
        expires_at: now + duration_hours * 3_600,
    };

    env.storage()
        .persistent()
        .set(&MarketplaceKey::TradeOffer(offer_id), &offer);
    env.storage().persistent().set(
        &MarketplaceKey::OfferedSkinTrade(offered_skin_id),
        &offer_id,
    );
    env.storage().persistent().set(
        &MarketplaceKey::RequestedSkinTrade(requested_skin_id),
        &offer_id,
    );

    env.events().publish(
        (symbol_short!("skin"), symbol_short!("offer")),
        (proposer.clone(), offered_skin_id, requested_skin_id, offer_id),
    );

    Ok(offer_id)
}

/// Accept a pending barter offer. Both skins swap owners atomically.
pub fn accept_trade_offer(
    env: &Env,
    accepter: &Address,
    offer_id: u64,
) -> Result<(), MarketplaceError> {
    accepter.require_auth();

    let mut offer: SkinTradeOffer = env
        .storage()
        .persistent()
        .get(&MarketplaceKey::TradeOffer(offer_id))
        .ok_or(MarketplaceError::TradeOfferNotFound)?;

    if offer.status != TradeOfferStatus::Pending {
        return Err(MarketplaceError::TradeOfferNotPending);
    }
    if env.ledger().timestamp() > offer.expires_at {
        offer.status = TradeOfferStatus::Expired;
        env.storage()
            .persistent()
            .set(&MarketplaceKey::TradeOffer(offer_id), &offer);
        let _ = set_tradeable(env, offer.offered_skin_id, true);
        clear_trade_indexes(env, &offer);
        return Err(MarketplaceError::TradeOfferExpired);
    }

    let requested = load_skin(env, offer.requested_skin_id)?;
    if requested.owner != *accepter {
        return Err(MarketplaceError::NotTradeTarget);
    }
    if requested.owner == offer.proposer {
        return Err(MarketplaceError::CannotTradeOwnSkin);
    }

    // Swap ownership. The offered skin was escrowed when the offer opened.
    transfer_skin_internal(env, offer.offered_skin_id, accepter).map_err(map_skin_error)?;
    transfer_skin_internal(env, offer.requested_skin_id, &offer.proposer)
        .map_err(map_skin_error)?;
    let _ = set_tradeable(env, offer.offered_skin_id, true);
    let _ = set_tradeable(env, offer.requested_skin_id, true);

    offer.status = TradeOfferStatus::Accepted;
    env.storage()
        .persistent()
        .set(&MarketplaceKey::TradeOffer(offer_id), &offer);
    clear_trade_indexes(env, &offer);

    env.events().publish(
        (symbol_short!("skin"), symbol_short!("trade")),
        (accepter.clone(), offer.proposer.clone(), offer_id),
    );

    Ok(())
}

/// Cancel a pending offer (proposer only) and release the escrowed skin.
pub fn cancel_trade_offer(
    env: &Env,
    proposer: &Address,
    offer_id: u64,
) -> Result<(), MarketplaceError> {
    proposer.require_auth();

    let mut offer: SkinTradeOffer = env
        .storage()
        .persistent()
        .get(&MarketplaceKey::TradeOffer(offer_id))
        .ok_or(MarketplaceError::TradeOfferNotFound)?;

    if offer.proposer != *proposer {
        return Err(MarketplaceError::NotSeller);
    }
    if offer.status != TradeOfferStatus::Pending {
        return Err(MarketplaceError::TradeOfferNotPending);
    }

    offer.status = TradeOfferStatus::Cancelled;
    env.storage()
        .persistent()
        .set(&MarketplaceKey::TradeOffer(offer_id), &offer);
    let _ = set_tradeable(env, offer.offered_skin_id, true);
    clear_trade_indexes(env, &offer);
    Ok(())
}

/// Get a barter offer by ID.
pub fn get_trade_offer(env: &Env, offer_id: u64) -> Option<SkinTradeOffer> {
    env.storage()
        .persistent()
        .get(&MarketplaceKey::TradeOffer(offer_id))
}

fn clear_trade_indexes(env: &Env, offer: &SkinTradeOffer) {
    env.storage()
        .persistent()
        .remove(&MarketplaceKey::OfferedSkinTrade(offer.offered_skin_id));
    env.storage()
        .persistent()
        .remove(&MarketplaceKey::RequestedSkinTrade(offer.requested_skin_id));
}
