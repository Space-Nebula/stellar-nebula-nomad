use super::types::*;
use super::listings::*;
use soroban_sdk::{Address, Env, Vec};


/// Create an auction.
pub fn create_auction(
    env: &Env,
    seller: &Address,
    asset: Asset,
    starting_price: i128,
    duration_seconds: u64,
) -> Result<u64, MarketplaceError> {
    seller.require_auth();

    if starting_price <= 0 {
        return Err(MarketplaceError::InvalidPrice);
    }

    // Check seller's auction cap
    let seller_auctions: Vec<u64> = env
        .storage()
        .persistent()
        .get(&MarketplaceKey::SellerAuctions(seller.clone()))
        .unwrap_or_else(|| Vec::new(env));

    if seller_auctions.len() >= MAX_AUCTIONS_PER_SELLER {
        return Err(MarketplaceError::SellerListingCapReached);
    }

    let auction_id = next_auction_id(env);
    let now = env.ledger().timestamp();

    let auction = AuctionListing {
        auction_id,
        seller: seller.clone(),
        asset: asset.clone(),
        starting_price,
        current_bid: starting_price,
        highest_bidder: None,
        started_at: now,
        ends_at: now + duration_seconds,
        finalized: false,
    };

    // Escrow the asset
    env.storage()
        .persistent()
        .set(&MarketplaceKey::EscrowedAsset(auction_id), &asset);

    // Store auction
    env.storage()
        .persistent()
        .set(&MarketplaceKey::Auction(auction_id), &auction);

    // Initialize empty bids
    env.storage().persistent().set(
        &MarketplaceKey::AuctionBids(auction_id),
        &Vec::<Bid>::new(env),
    );

    // Update seller's auction index
    let mut updated_auctions = seller_auctions;
    updated_auctions.push_back(auction_id);
    env.storage().persistent().set(
        &MarketplaceKey::SellerAuctions(seller.clone()),
        &updated_auctions,
    );

    // Update analytics
    bump_counter(env, MarketplaceKey::TotalAuctions, 1);

    env.events().publish(
        (symbol_short!("auction"), symbol_short!("create")),
        (seller.clone(), auction_id, starting_price),
    );

    Ok(auction_id)
}

/// Place a bid on an auction.
pub fn place_bid(
    env: &Env,
    bidder: &Address,
    auction_id: u64,
    amount: i128,
) -> Result<(), MarketplaceError> {
    bidder.require_auth();

    let mut auction: AuctionListing = env
        .storage()
        .persistent()
        .get(&MarketplaceKey::Auction(auction_id))
        .ok_or(MarketplaceError::NotFound)?;

    let now = env.ledger().timestamp();

    // Check auction hasn't ended
    if now > auction.ends_at {
        return Err(MarketplaceError::AuctionEnded);
    }

    if &auction.seller == bidder {
        return Err(MarketplaceError::SelfPurchase);
    }

    // Check minimum bid increment (5%)
    let min_bid =
        auction.current_bid + (auction.current_bid * MIN_BID_INCREMENT_BPS / BPS_DENOMINATOR);

    if amount < min_bid {
        return Err(MarketplaceError::BidTooLow);
    }

    // Update auction
    auction.current_bid = amount;
    auction.highest_bidder = Some(bidder.clone());

    // Extend auction if bid comes in last 5 minutes
    if auction.ends_at.saturating_sub(now) <= AUCTION_EXTENSION_WINDOW {
        auction.ends_at = auction.ends_at.saturating_add(AUCTION_EXTENSION_SECONDS);
    }

    env.storage()
        .persistent()
        .set(&MarketplaceKey::Auction(auction_id), &auction);

    // Record bid
    let mut bids: Vec<Bid> = env
        .storage()
        .persistent()
        .get(&MarketplaceKey::AuctionBids(auction_id))
        .unwrap_or_else(|| Vec::new(env));

    bids.push_back(Bid {
        bidder: bidder.clone(),
        amount,
        timestamp: now,
    });

    env.storage()
        .persistent()
        .set(&MarketplaceKey::AuctionBids(auction_id), &bids);

    // Check auto-bids and potentially outbid
    process_auto_bids(env, auction_id, amount)?;

    env.events().publish(
        (symbol_short!("auction"), symbol_short!("bid")),
        (bidder.clone(), auction_id, amount),
    );

    Ok(())
}

/// Set up auto-bidding.
pub fn set_auto_bid(
    env: &Env,
    bidder: &Address,
    auction_id: u64,
    max_price: i128,
) -> Result<(), MarketplaceError> {
    bidder.require_auth();

    let auction: AuctionListing = env
        .storage()
        .persistent()
        .get(&MarketplaceKey::Auction(auction_id))
        .ok_or(MarketplaceError::NotFound)?;

    // Validate max price
    if max_price <= auction.current_bid {
        return Err(MarketplaceError::BidTooLow);
    }

    if max_price > auction.starting_price * MAX_AUTO_BID_MULTIPLIER {
        return Err(MarketplaceError::AutoBidTooHigh);
    }

    let auto_bid = AutoBid {
        bidder: bidder.clone(),
        max_price,
        active: true,
    };

    env.storage().persistent().set(
        &MarketplaceKey::AutoBid(auction_id, bidder.clone()),
        &auto_bid,
    );

    env.events().publish(
        (symbol_short!("auction"), symbol_short!("autobid")),
        (bidder.clone(), auction_id, max_price),
    );

    Ok(())
}

/// Process auto-bids after a manual bid.
fn process_auto_bids(
    env: &Env,
    auction_id: u64,
    current_high_bid: i128,
) -> Result<(), MarketplaceError> {
    // This is a simplified version - in a full implementation,
    // we'd iterate through all auto-bidders and place bids up to their max
    Ok(())
}

/// Finalize an auction (can be called by anyone after auction ends).
pub fn finalize_auction(
    env: &Env,
    caller: &Address,
    auction_id: u64,
) -> Result<(), MarketplaceError> {
    caller.require_auth();

    let mut auction: AuctionListing = env
        .storage()
        .persistent()
        .get(&MarketplaceKey::Auction(auction_id))
        .ok_or(MarketplaceError::NotFound)?;

    let now = env.ledger().timestamp();

    if now <= auction.ends_at {
        return Err(MarketplaceError::AuctionNotEnded);
    }

    if auction.finalized {
        return Err(MarketplaceError::AuctionAlreadyFinalized);
    }

    auction.finalized = true;
    env.storage()
        .persistent()
        .set(&MarketplaceKey::Auction(auction_id), &auction);

    if let Some(winner) = &auction.highest_bidder {
        // Check wash trading
        check_wash_trading(env, auction.asset.asset_id, winner)?;

        // Calculate fees
        let (success_fee, seller_proceeds) =
            calculate_fees(env, &auction.seller, auction.current_bid, false)?;

        // Release escrow to winner
        env.storage()
            .persistent()
            .remove(&MarketplaceKey::EscrowedAsset(auction_id));

        // Record sale
        record_sale(
            env,
            auction.asset.asset_id,
            &auction.seller,
            winner,
            auction.current_bid,
        );

        // Update analytics
        bump_counter(env, MarketplaceKey::TotalSales, 1);
        bump_volume(env, auction.current_bid)?;
        bump_category_volume(env, auction.asset.category, auction.current_bid)?;
        bump_collected_fees(env, success_fee)?;

        env.events().publish(
            (symbol_short!("auction"), symbol_short!("won")),
            (winner.clone(), auction_id, auction.current_bid),
        );
    } else {
        // No bids - return asset to seller
        env.storage()
            .persistent()
            .remove(&MarketplaceKey::EscrowedAsset(auction_id));

        env.events().publish(
            (symbol_short!("auction"), symbol_short!("nobids")),
            (auction.seller.clone(), auction_id),
        );
    }

    // Remove from seller's active auctions
    remove_auction(env, &auction.seller, auction_id);
    bump_counter(env, MarketplaceKey::TotalAuctions, -1);

    Ok(())
}

fn remove_auction(env: &Env, seller: &Address, auction_id: u64) {
    let auctions: Vec<u64> = env
        .storage()
        .persistent()
        .get(&MarketplaceKey::SellerAuctions(seller.clone()))
        .unwrap_or_else(|| Vec::new(env));

    let mut updated = Vec::new(env);
    for i in 0..auctions.len() {
        if let Some(id) = auctions.get(i) {
            if id != auction_id {
                updated.push_back(id);
            }
        }
    }

    env.storage()
        .persistent()
        .set(&MarketplaceKey::SellerAuctions(seller.clone()), &updated);
}

// ── Bundle Listings ───────────────────────────────────────────────────────────
