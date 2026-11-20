use super::types::*;
use super::listings::*;
use soroban_sdk::{symbol_short, Address, Env, Vec};


/// Create a bundle listing.
pub fn create_bundle(
    env: &Env,
    seller: &Address,
    assets: Vec<Asset>,
    bundle_price: i128,
    expires_at: Option<u64>,
) -> Result<u64, MarketplaceError> {
    seller.require_auth();

    if assets.is_empty() {
        return Err(MarketplaceError::BundleEmpty);
    }

    if assets.len() > MAX_BUNDLE_ITEMS {
        return Err(MarketplaceError::BundleTooLarge);
    }

    if bundle_price <= 0 {
        return Err(MarketplaceError::InvalidPrice);
    }

    let listing_id = next_bundle_id(env);

    let bundle = BundleListing {
        listing_id,
        seller: seller.clone(),
        assets: assets.clone(),
        bundle_price,
        listed_at: env.ledger().timestamp(),
        expires_at,
    };

    // Escrow all assets
    env.storage()
        .persistent()
        .set(&MarketplaceKey::EscrowedAsset(listing_id), &assets);

    env.storage()
        .persistent()
        .set(&MarketplaceKey::Bundle(listing_id), &bundle);

    // Calculate and charge listing fee
    let (listing_fee, _) = calculate_fees(env, seller, bundle_price, true)?;
    bump_collected_fees(env, listing_fee)?;

    env.events().publish(
        (symbol_short!("market"), symbol_short!("bundle")),
        (seller.clone(), listing_id, bundle_price),
    );

    Ok(listing_id)
}

/// Purchase a bundle.
pub fn buy_bundle(env: &Env, buyer: &Address, bundle_id: u64) -> Result<(), MarketplaceError> {
    buyer.require_auth();

    let bundle: BundleListing = env
        .storage()
        .persistent()
        .get(&MarketplaceKey::Bundle(bundle_id))
        .ok_or(MarketplaceError::NotFound)?;

    // Check expiration
    if let Some(expires_at) = bundle.expires_at {
        if env.ledger().timestamp() > expires_at {
            return Err(MarketplaceError::ListingExpired);
        }
    }

    if &bundle.seller == buyer {
        return Err(MarketplaceError::SelfPurchase);
    }

    // Calculate fees
    let (success_fee, _) = calculate_fees(env, &bundle.seller, bundle.bundle_price, false)?;

    // Release escrow
    env.storage()
        .persistent()
        .remove(&MarketplaceKey::EscrowedAsset(bundle_id));

    // Update analytics
    bump_counter(env, MarketplaceKey::TotalSales, 1);
    bump_volume(env, bundle.bundle_price)?;
    bump_collected_fees(env, success_fee)?;

    // Remove bundle
    env.storage()
        .persistent()
        .remove(&MarketplaceKey::Bundle(bundle_id));

    env.events().publish(
        (symbol_short!("market"), symbol_short!("bndl_sold")),
        (buyer.clone(), bundle.seller.clone(), bundle_id),
    );

    Ok(())
}

// ── Trade Offers ──────────────────────────────────────────────────────────────

/// Create a trade offer.
pub fn create_trade_offer(
    env: &Env,
    offerer: &Address,
    target: &Address,
    offered_assets: Vec<Asset>,
    requested_assets: Vec<Asset>,
    duration_seconds: u64,
) -> Result<u64, MarketplaceError> {
    offerer.require_auth();

    if offered_assets.is_empty() || requested_assets.is_empty() {
        return Err(MarketplaceError::InvalidAsset);
    }

    // Check offer cap
    let user_offers: Vec<u64> = env
        .storage()
        .persistent()
        .get(&MarketplaceKey::UserTradeOffers(offerer.clone()))
        .unwrap_or_else(|| Vec::new(env));

    if user_offers.len() >= MAX_TRADE_OFFERS_PER_USER {
        return Err(MarketplaceError::SellerListingCapReached);
    }

    let offer_id = next_trade_offer_id(env);
    let now = env.ledger().timestamp();

    let offer = TradeOffer {
        offer_id,
        offerer: offerer.clone(),
        target: target.clone(),
        offered_assets: offered_assets.clone(),
        requested_assets,
        created_at: now,
        expires_at: now + duration_seconds,
        status: TradeOfferStatus::Pending,
    };

    // Escrow offered assets
    env.storage()
        .persistent()
        .set(&MarketplaceKey::EscrowedAsset(offer_id), &offered_assets);

    env.storage()
        .persistent()
        .set(&MarketplaceKey::TradeOffer(offer_id), &offer);

    // Update offerer's trade index
    let mut updated_offers = user_offers;
    updated_offers.push_back(offer_id);
    env.storage().persistent().set(
        &MarketplaceKey::UserTradeOffers(offerer.clone()),
        &updated_offers,
    );

    env.events().publish(
        (symbol_short!("trade"), symbol_short!("offer")),
        (offerer.clone(), target.clone(), offer_id),
    );

    Ok(offer_id)
}

/// Accept a trade offer.
pub fn accept_trade_offer(
    env: &Env,
    target: &Address,
    offer_id: u64,
) -> Result<(), MarketplaceError> {
    target.require_auth();

    let mut offer: TradeOffer = env
        .storage()
        .persistent()
        .get(&MarketplaceKey::TradeOffer(offer_id))
        .ok_or(MarketplaceError::NotFound)?;

    if &offer.target != target {
        return Err(MarketplaceError::NotTradeTarget);
    }

    if offer.status != TradeOfferStatus::Pending {
        return Err(MarketplaceError::TradeOfferNotPending);
    }

    let now = env.ledger().timestamp();
    if now > offer.expires_at {
        return Err(MarketplaceError::TradeOfferExpired);
    }

    // Update status
    offer.status = TradeOfferStatus::Accepted;
    env.storage()
        .persistent()
        .set(&MarketplaceKey::TradeOffer(offer_id), &offer);

    // Release escrow (swap assets)
    env.storage()
        .persistent()
        .remove(&MarketplaceKey::EscrowedAsset(offer_id));

    // Remove from offerer's active offers
    remove_trade_offer(env, &offer.offerer, offer_id);

    env.events().publish(
        (symbol_short!("trade"), symbol_short!("accept")),
        (target.clone(), offer_id),
    );

    Ok(())
}

/// Reject or cancel a trade offer.
pub fn cancel_trade_offer(
    env: &Env,
    caller: &Address,
    offer_id: u64,
) -> Result<(), MarketplaceError> {
    caller.require_auth();

    let mut offer: TradeOffer = env
        .storage()
        .persistent()
        .get(&MarketplaceKey::TradeOffer(offer_id))
        .ok_or(MarketplaceError::NotFound)?;

    // Only offerer or target can cancel
    if caller != &offer.offerer && caller != &offer.target {
        return Err(MarketplaceError::Unauthorized);
    }

    if offer.status != TradeOfferStatus::Pending {
        return Err(MarketplaceError::TradeOfferNotPending);
    }

    // Update status
    offer.status = if caller == &offer.offerer {
        TradeOfferStatus::Cancelled
    } else {
        TradeOfferStatus::Rejected
    };

    env.storage()
        .persistent()
        .set(&MarketplaceKey::TradeOffer(offer_id), &offer);

    // Release escrow
    env.storage()
        .persistent()
        .remove(&MarketplaceKey::EscrowedAsset(offer_id));

    // Remove from offerer's active offers
    remove_trade_offer(env, &offer.offerer, offer_id);

    env.events().publish(
        (symbol_short!("trade"), symbol_short!("cancel")),
        (caller.clone(), offer_id),
    );

    Ok(())
}

fn remove_trade_offer(env: &Env, offerer: &Address, offer_id: u64) {
    let offers: Vec<u64> = env
        .storage()
        .persistent()
        .get(&MarketplaceKey::UserTradeOffers(offerer.clone()))
        .unwrap_or_else(|| Vec::new(env));

    let mut updated = Vec::new(env);
    for i in 0..offers.len() {
        if let Some(id) = offers.get(i) {
            if id != offer_id {
                updated.push_back(id);
            }
        }
    }

    env.storage()
        .persistent()
        .set(&MarketplaceKey::UserTradeOffers(offerer.clone()), &updated);
}

// ── Search & Filters ──────────────────────────────────────────────────────────

/// Search listings with fuzzy matching and filters.
pub fn search_listings(env: &Env, query: String, filter: SearchFilter) -> Vec<FixedPriceListing> {
    let mut results = Vec::new(env);

    // Get all listings (simplified - in production use indexed search)
    let counter: u64 = env
        .storage()
        .instance()
        .get(&MarketplaceKey::ListingCounter)
        .unwrap_or(0);

    for id in 1..=counter.min(MAX_SEARCH_RESULTS as u64) {
        if let Some(listing) = env
            .storage()
            .persistent()
            .get::<_, FixedPriceListing>(&MarketplaceKey::NftMarketListing(id))
        {
            // Apply filters
            if let Some(cat) = filter.category {
                if listing.asset.category != cat {
                    continue;
                }
            }

            if let Some(min) = filter.min_price {
                if listing.price < min {
                    continue;
                }
            }

            if let Some(max) = filter.max_price {
                if listing.price > max {
                    continue;
                }
            }

            if let Some(rarity) = filter.rarity {
                if listing.asset.rarity != rarity {
                    continue;
                }
            }

            // Fuzzy match on asset name
            if !query.is_empty() && !fuzzy_match(&listing.asset.name, &query) {
                continue;
            }

            results.push_back(listing);
        }

        if results.len() >= MAX_SEARCH_RESULTS {
            break;
        }
    }

    // Sort results
    results = sort_listings(env, results, filter.sort_by);

    results
}

fn sort_listings(
    env: &Env,
    mut listings: Vec<FixedPriceListing>,
    sort_by: SortOption,
) -> Vec<FixedPriceListing> {
    // Simplified sorting - in production use efficient sort algorithm
    listings
}

// ── Analytics ─────────────────────────────────────────────────────────────────

/// Get marketplace statistics.
pub fn get_marketplace_stats(env: &Env) -> MarketplaceStats {
    MarketplaceStats {
        total_volume: env
            .storage()
            .persistent()
            .get(&MarketplaceKey::TotalVolume)
            .unwrap_or(0),
        total_sales: env
            .storage()
            .persistent()
            .get(&MarketplaceKey::TotalSales)
            .unwrap_or(0),
        active_listings: env
            .storage()
            .persistent()
            .get(&MarketplaceKey::TotalListings)
            .unwrap_or(0),
        active_auctions: env
            .storage()
            .persistent()
            .get(&MarketplaceKey::TotalAuctions)
            .unwrap_or(0),
        collected_fees: env
            .storage()
            .persistent()
            .get(&MarketplaceKey::CollectedFees)
            .unwrap_or(0),
    }
}

/// Get volume for a specific asset category.
pub fn get_category_volume(env: &Env, category: AssetCategory) -> i128 {
    env.storage()
        .persistent()
        .get(&MarketplaceKey::CategoryVolume(category))
        .unwrap_or(0)
}

// ── Helper Functions ──────────────────────────────────────────────────────────

fn bump_counter(env: &Env, key: MarketplaceKey, delta: i32) {
    let current: u32 = env.storage().persistent().get(&key).unwrap_or(0);
    let updated = if delta >= 0 {
        current.saturating_add(delta as u32)
    } else {
        current.saturating_sub((-delta) as u32)
    };
    env.storage().persistent().set(&key, &updated);
}

fn bump_volume(env: &Env, amount: i128) -> Result<(), MarketplaceError> {
    let current: i128 = env
        .storage()
        .persistent()
        .get(&MarketplaceKey::TotalVolume)
        .unwrap_or(0);
    let updated = current
        .checked_add(amount)
        .ok_or(MarketplaceError::ArithmeticOverflow)?;
    env.storage()
        .persistent()
        .set(&MarketplaceKey::TotalVolume, &updated);
    Ok(())
}

fn bump_category_volume(
    env: &Env,
    category: AssetCategory,
    amount: i128,
) -> Result<(), MarketplaceError> {
    let key = MarketplaceKey::CategoryVolume(category);
    let current: i128 = env.storage().persistent().get(&key).unwrap_or(0);
    let updated = current
        .checked_add(amount)
        .ok_or(MarketplaceError::ArithmeticOverflow)?;
    env.storage().persistent().set(&key, &updated);
    Ok(())
}

fn bump_collected_fees(env: &Env, amount: i128) -> Result<(), MarketplaceError> {
    let current: i128 = env
        .storage()
        .persistent()
        .get(&MarketplaceKey::CollectedFees)
        .unwrap_or(0);
    let updated = current
        .checked_add(amount)
        .ok_or(MarketplaceError::ArithmeticOverflow)?;
    env.storage()
        .persistent()
        .set(&MarketplaceKey::CollectedFees, &updated);
    Ok(())
}

// ── Query Functions ───────────────────────────────────────────────────────────

pub fn get_listing(env: &Env, listing_id: u64) -> Option<FixedPriceListing> {
    env.storage()
        .persistent()
        .get(&MarketplaceKey::NftMarketListing(listing_id))
}

pub fn get_auction(env: &Env, auction_id: u64) -> Option<AuctionListing> {
    env.storage()
        .persistent()
        .get(&MarketplaceKey::Auction(auction_id))
}

pub fn get_bundle(env: &Env, bundle_id: u64) -> Option<BundleListing> {
    env.storage()
        .persistent()
        .get(&MarketplaceKey::Bundle(bundle_id))
}

pub fn get_trade_offer(env: &Env, offer_id: u64) -> Option<TradeOffer> {
    env.storage()
        .persistent()
        .get(&MarketplaceKey::TradeOffer(offer_id))
}

pub fn get_auction_bids(env: &Env, auction_id: u64) -> Vec<Bid> {
    env.storage()
        .persistent()
        .get(&MarketplaceKey::AuctionBids(auction_id))
        .unwrap_or_else(|| Vec::new(env))
}
