use super::types::*;
use soroban_sdk::{symbol_short, Address, Env, String, Symbol, Vec};


fn next_listing_id(env: &Env) -> u64 {
    let n: u64 = env
        .storage()
        .instance()
        .get(&MarketplaceKey::ListingCounter)
        .unwrap_or(0);
    env.storage()
        .instance()
        .set(&MarketplaceKey::ListingCounter, &(n + 1));
    n + 1
}

fn next_auction_id(env: &Env) -> u64 {
    let n: u64 = env
        .storage()
        .instance()
        .get(&MarketplaceKey::AuctionCounter)
        .unwrap_or(0);
    env.storage()
        .instance()
        .set(&MarketplaceKey::AuctionCounter, &(n + 1));
    n + 1
}

fn next_bundle_id(env: &Env) -> u64 {
    let n: u64 = env
        .storage()
        .instance()
        .get(&MarketplaceKey::BundleCounter)
        .unwrap_or(0);
    env.storage()
        .instance()
        .set(&MarketplaceKey::BundleCounter, &(n + 1));
    n + 1
}

fn next_trade_offer_id(env: &Env) -> u64 {
    let n: u64 = env
        .storage()
        .instance()
        .get(&MarketplaceKey::TradeOfferCounter)
        .unwrap_or(0);
    env.storage()
        .instance()
        .set(&MarketplaceKey::TradeOfferCounter, &(n + 1));
    n + 1
}

/// Calculate marketplace fees with reputation discount.
fn calculate_fees(
    env: &Env,
    seller: &Address,
    price: i128,
    is_listing: bool,
) -> Result<(i128, i128), MarketplaceError> {
    let base_fee_bps = if is_listing {
        LISTING_FEE_BPS
    } else {
        SUCCESS_FEE_BPS
    };

    // Get seller's reputation for discount
    let discount_bps = if let Ok(score) = reputation::get_reputation_score(env, seller) {
        if score >= REP_DISCOUNT_TIER_HIGH {
            REP_DISCOUNT_HIGH_BPS
        } else if score >= REP_DISCOUNT_TIER_MID {
            REP_DISCOUNT_MID_BPS
        } else {
            0
        }
    } else {
        0
    };

    let effective_fee_bps = base_fee_bps.saturating_sub(discount_bps).max(0);

    let fee = price
        .checked_mul(effective_fee_bps)
        .ok_or(MarketplaceError::ArithmeticOverflow)?
        / BPS_DENOMINATOR;

    let proceeds = price
        .checked_sub(fee)
        .ok_or(MarketplaceError::ArithmeticOverflow)?;

    Ok((fee, proceeds))
}

/// Check for wash trading patterns.
fn check_wash_trading(env: &Env, asset_id: u64, buyer: &Address) -> Result<(), MarketplaceError> {
    let key = MarketplaceKey::RecentSales(asset_id);
    let sales: Vec<SaleRecord> = env
        .storage()
        .persistent()
        .get(&key)
        .unwrap_or_else(|| Vec::new(env));

    let now = env.ledger().timestamp();

    // Check if buyer was a recent seller (within wash trade window)
    for i in 0..sales.len() {
        if let Some(sale) = sales.get(i) {
            if now.saturating_sub(sale.timestamp) <= WASH_TRADE_WINDOW_SECONDS {
                if &sale.seller == buyer {
                    // `record_event` reports to the fraud subsystem; it is not
                    // fallible, so the wash-trade verdict below is what rejects
                    // this purchase.
                    fraud_detection::record_event(env, buyer, sale.price as u64);
                    return Err(MarketplaceError::WashTradingDetected);
                }
            }
        }
    }

    Ok(())
}

/// Record a sale for fraud detection.
fn record_sale(env: &Env, asset_id: u64, seller: &Address, buyer: &Address, price: i128) {
    let key = MarketplaceKey::RecentSales(asset_id);
    let mut sales: Vec<SaleRecord> = env
        .storage()
        .persistent()
        .get(&key)
        .unwrap_or_else(|| Vec::new(env));

    let record = SaleRecord {
        seller: seller.clone(),
        buyer: buyer.clone(),
        price,
        timestamp: env.ledger().timestamp(),
    };

    sales.push_back(record);

    // Keep only last 10 sales for the asset
    if sales.len() > 10 {
        let mut trimmed = Vec::new(env);
        for i in (sales.len().saturating_sub(10))..sales.len() {
            if let Some(s) = sales.get(i) {
                trimmed.push_back(s);
            }
        }
        sales = trimmed;
    }

    env.storage().persistent().set(&key, &sales);
}

/// Maximum name length (bytes) that [`fuzzy_match`] will search.
///
/// On-chain matching has no allocator, so both sides are copied into fixed
/// stack buffers. Names are normalized and stored capped at this length when
/// the search index is built, so a longer input here is a caller bug rather
/// than a normal case.
pub const MAX_FUZZY_NAME_LEN: u32 = 128;

/// Lowercase a single ASCII byte. Non-ASCII bytes pass through unchanged,
/// which is fine for the case-insensitivity this index needs.
fn ascii_lower(b: u8) -> u8 {
    if b.is_ascii_uppercase() {
        b + 32
    } else {
        b
    }
}

/// Case-insensitive substring match: does `needle` occur inside `haystack`?
///
/// Implemented over raw bytes because `soroban_sdk::String` is a host-backed
/// `ScVal` and deliberately exposes no `to_lowercase`/`to_string` (there is
/// no `core::fmt` in a `no_std` contract build).
fn fuzzy_match(haystack: &String, needle: &String) -> bool {
    let (h_len, n_len) = (haystack.len(), needle.len());

    // An empty needle matches everything.
    if n_len == 0 {
        return true;
    }
    if n_len > h_len {
        return false;
    }
    if h_len > MAX_FUZZY_NAME_LEN || n_len > MAX_FUZZY_NAME_LEN {
        return false;
    }

    let h_len = h_len as usize;
    let n_len = n_len as usize;

    let mut h_bytes = [0u8; MAX_FUZZY_NAME_LEN as usize];
    let mut n_bytes = [0u8; MAX_FUZZY_NAME_LEN as usize];
    haystack.copy_into_slice(&mut h_bytes[..h_len]);
    needle.copy_into_slice(&mut n_bytes[..n_len]);

    // Sliding window over the haystack.
    let first = ascii_lower(n_bytes[0]);
    let last_start = h_len - n_len;
    let mut start = 0usize;
    while start <= last_start {
        if ascii_lower(h_bytes[start]) == first {
            let mut matched = true;
            let mut i = 1usize;
            while i < n_len {
                if ascii_lower(h_bytes[start + i]) != n_bytes[i] {
                    matched = false;
                    break;
                }
                i += 1;
            }
            if matched {
                return true;
            }
        }
        start += 1;
    }
    false
}

// ── Fixed-Price Listings ──────────────────────────────────────────────────────

/// Create a fixed-price listing.
pub fn create_listing(
    env: &Env,
    seller: &Address,
    asset: Asset,
    price: i128,
    expires_at: Option<u64>,
) -> Result<u64, MarketplaceError> {
    seller.require_auth();

    if price <= 0 {
        return Err(MarketplaceError::InvalidPrice);
    }

    // Check seller's listing cap
    let seller_listings: Vec<u64> = env
        .storage()
        .persistent()
        .get(&MarketplaceKey::SellerListings(seller.clone()))
        .unwrap_or_else(|| Vec::new(env));

    if seller_listings.len() >= MAX_LISTINGS_PER_SELLER {
        return Err(MarketplaceError::SellerListingCapReached);
    }

    // Calculate and charge listing fee
    let (listing_fee, _) = calculate_fees(env, seller, price, true)?;

    let listing_id = next_listing_id(env);

    let listing = FixedPriceListing {
        listing_id,
        seller: seller.clone(),
        asset: asset.clone(),
        price,
        listed_at: env.ledger().timestamp(),
        expires_at,
    };

    // Escrow the asset
    env.storage()
        .persistent()
        .set(&MarketplaceKey::EscrowedAsset(listing_id), &asset);

    // Store listing
    env.storage()
        .persistent()
        .set(&MarketplaceKey::NftMarketListing(listing_id), &listing);

    // Update seller's listing index
    let mut updated_listings = seller_listings;
    updated_listings.push_back(listing_id);
    env.storage().persistent().set(
        &MarketplaceKey::SellerListings(seller.clone()),
        &updated_listings,
    );

    // Update analytics
    bump_counter(env, MarketplaceKey::TotalListings, 1);
    bump_collected_fees(env, listing_fee)?;

    env.events().publish(
        (symbol_short!("market"), symbol_short!("list")),
        (seller.clone(), listing_id, price),
    );

    Ok(listing_id)
}

/// Purchase a fixed-price listing.
pub fn buy_listing(env: &Env, buyer: &Address, listing_id: u64) -> Result<(), MarketplaceError> {
    buyer.require_auth();

    let listing: FixedPriceListing = env
        .storage()
        .persistent()
        .get(&MarketplaceKey::NftMarketListing(listing_id))
        .ok_or(MarketplaceError::NotFound)?;

    // Check expiration
    if let Some(expires_at) = listing.expires_at {
        if env.ledger().timestamp() > expires_at {
            return Err(MarketplaceError::ListingExpired);
        }
    }

    if &listing.seller == buyer {
        return Err(MarketplaceError::SelfPurchase);
    }

    // Check wash trading
    check_wash_trading(env, listing.asset.asset_id, buyer)?;

    // Calculate fees
    let (success_fee, seller_proceeds) =
        calculate_fees(env, &listing.seller, listing.price, false)?;

    // Release escrow to buyer
    env.storage()
        .persistent()
        .remove(&MarketplaceKey::EscrowedAsset(listing_id));

    // Record sale
    record_sale(
        env,
        listing.asset.asset_id,
        &listing.seller,
        buyer,
        listing.price,
    );

    // Update analytics
    bump_counter(env, MarketplaceKey::TotalSales, 1);
    bump_volume(env, listing.price)?;
    bump_category_volume(env, listing.asset.category, listing.price)?;
    bump_collected_fees(env, success_fee)?;
    bump_counter(env, MarketplaceKey::TotalListings, -1);

    // Remove listing
    remove_listing(env, &listing.seller, listing_id);

    env.events().publish(
        (symbol_short!("market"), symbol_short!("sold")),
        (
            buyer.clone(),
            listing.seller.clone(),
            listing_id,
            listing.price,
        ),
    );

    Ok(())
}

/// Cancel a listing.
pub fn cancel_listing(
    env: &Env,
    seller: &Address,
    listing_id: u64,
) -> Result<(), MarketplaceError> {
    seller.require_auth();

    let listing: FixedPriceListing = env
        .storage()
        .persistent()
        .get(&MarketplaceKey::NftMarketListing(listing_id))
        .ok_or(MarketplaceError::NotFound)?;

    if &listing.seller != seller {
        return Err(MarketplaceError::Unauthorized);
    }

    // Release escrow
    env.storage()
        .persistent()
        .remove(&MarketplaceKey::EscrowedAsset(listing_id));

    // Remove listing
    remove_listing(env, seller, listing_id);
    bump_counter(env, MarketplaceKey::TotalListings, -1);

    env.events().publish(
        (symbol_short!("market"), symbol_short!("cancel")),
        (seller.clone(), listing_id),
    );

    Ok(())
}

/// Read a fixed-price listing by ID.
pub fn get_listing(env: &Env, listing_id: u64) -> Option<FixedPriceListing> {
    env.storage()
        .persistent()
        .get(&MarketplaceKey::NftMarketListing(listing_id))
}

/// All fixed-price listings created by a seller (active and historical).
pub fn get_seller_listings(env: &Env, seller: &Address) -> Vec<FixedPriceListing> {
    let ids: Vec<u64> = env
        .storage()
        .persistent()
        .get(&MarketplaceKey::SellerListings(seller.clone()))
        .unwrap_or_else(|| Vec::new(env));

    let mut listings = Vec::new(env);
    for id in ids.iter() {
        if let Some(listing) = get_listing(env, id) {
            listings.push_back(listing);
        }
    }
    listings
}

/// Change the asking price of an active fixed-price listing.
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

    let mut listing: FixedPriceListing = env
        .storage()
        .persistent()
        .get(&MarketplaceKey::NftMarketListing(listing_id))
        .ok_or(MarketplaceError::NotFound)?;

    if &listing.seller != seller {
        return Err(MarketplaceError::Unauthorized);
    }
    if let Some(expires_at) = listing.expires_at {
        if env.ledger().timestamp() > expires_at {
            return Err(MarketplaceError::ListingExpired);
        }
    }

    listing.price = new_price;
    env.storage()
        .persistent()
        .set(&MarketplaceKey::NftMarketListing(listing_id), &listing);

    env.events().publish(
        (symbol_short!("market"), symbol_short!("reprice")),
        (seller.clone(), listing_id, new_price),
    );

    Ok(())
}

/// Permissionless cleanup of an expired listing: releases escrow and removes it.
pub fn expire_listing(env: &Env, listing_id: u64) -> Result<(), MarketplaceError> {
    let listing: FixedPriceListing = env
        .storage()
        .persistent()
        .get(&MarketplaceKey::NftMarketListing(listing_id))
        .ok_or(MarketplaceError::NotFound)?;

    let expires_at = listing.expires_at.ok_or(MarketplaceError::ListingNotExpired)?;
    if env.ledger().timestamp() <= expires_at {
        return Err(MarketplaceError::ListingNotExpired);
    }

    env.storage()
        .persistent()
        .remove(&MarketplaceKey::EscrowedAsset(listing_id));
    remove_listing(env, &listing.seller, listing_id);
    bump_counter(env, MarketplaceKey::TotalListings, -1);

    env.events().publish(
        (symbol_short!("market"), symbol_short!("expire")),
        (listing_id, listing.seller),
    );

    Ok(())
}

/// Create a fixed-price listing for a ship NFT.
///
/// The category is set by this helper so callers cannot accidentally list a
/// ship under the wrong market filter.
pub fn create_ship_listing(
    env: &Env,
    seller: &Address,
    ship_id: u64,
    rarity: Rarity,
    name: String,
    price: i128,
    expires_at: Option<u64>,
) -> Result<u64, MarketplaceError> {
    let asset = Asset {
        category: AssetCategory::ShipNFT,
        asset_id: ship_id,
        quantity: 1,
        rarity,
        name,
    };
    create_listing(env, seller, asset, price, expires_at)
}

/// Create a fixed-price listing for a skin NFT.
pub fn create_skin_listing(
    env: &Env,
    seller: &Address,
    skin_id: u64,
    rarity: Rarity,
    name: String,
    price: i128,
    expires_at: Option<u64>,
) -> Result<u64, MarketplaceError> {
    let asset = Asset {
        category: AssetCategory::SkinNFT,
        asset_id: skin_id,
        quantity: 1,
        rarity,
        name,
    };
    create_listing(env, seller, asset, price, expires_at)
}

/// Create a fixed-price listing for a badge NFT.
pub fn create_badge_listing(
    env: &Env,
    seller: &Address,
    badge_id: u64,
    rarity: Rarity,
    name: String,
    price: i128,
    expires_at: Option<u64>,
) -> Result<u64, MarketplaceError> {
    let asset = Asset {
        category: AssetCategory::Badge,
        asset_id: badge_id,
        quantity: 1,
        rarity,
        name,
    };
    create_listing(env, seller, asset, price, expires_at)
}

fn remove_listing(env: &Env, seller: &Address, listing_id: u64) {
    env.storage()
        .persistent()
        .remove(&MarketplaceKey::NftMarketListing(listing_id));

    let listings: Vec<u64> = env
        .storage()
        .persistent()
        .get(&MarketplaceKey::SellerListings(seller.clone()))
        .unwrap_or_else(|| Vec::new(env));

    let mut updated = Vec::new(env);
    for i in 0..listings.len() {
        if let Some(id) = listings.get(i) {
            if id != listing_id {
                updated.push_back(id);
            }
        }
    }

    env.storage()
        .persistent()
        .set(&MarketplaceKey::SellerListings(seller.clone()), &updated);
}

// ── Auctions ──────────────────────────────────────────────────────────────────
