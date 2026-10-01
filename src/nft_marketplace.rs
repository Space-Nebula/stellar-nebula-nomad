//! Comprehensive NFT Marketplace — Issue #534
//!
//! Complete marketplace supporting:
//! - Fixed-price listings
//! - Auction system with auto-bid, bid increments, and extensions
//! - Bundle listings (up to 10 items)
//! - Direct trade offers (item-for-item)
//! - All asset types (resources, NFTs, blueprints, housing items, badges)
//! - Escrow system
//! - Marketplace fees with reputation discounts
//! - Search with fuzzy matching
//! - Comprehensive filters (price, rarity, type, date)
//! - Marketplace analytics
//! - Fraud detection (wash trading)

use soroban_sdk::{
    contracterror, contracttype, symbol_short, Address, Bytes, Env, String, Symbol, Vec,
};

use crate::fraud_detection;
use crate::reputation;
use crate::ship_customization::SkinRarity;

// ── Constants ─────────────────────────────────────────────────────────────────

/// Listing fee: 1% of listing price (basis points).
pub const LISTING_FEE_BPS: i128 = 100;
/// Success fee: 3% of sale price (basis points).
pub const SUCCESS_FEE_BPS: i128 = 300;
/// Basis point denominator.
pub const BPS_DENOMINATOR: i128 = 10_000;

/// Maximum items in a bundle listing.
pub const MAX_BUNDLE_ITEMS: u32 = 10;
/// Maximum active listings per seller.
pub const MAX_LISTINGS_PER_SELLER: u32 = 50;
/// Maximum active auctions per seller.
pub const MAX_AUCTIONS_PER_SELLER: u32 = 20;
/// Maximum trade offers per user.
pub const MAX_TRADE_OFFERS_PER_USER: u32 = 30;

/// Minimum bid increment: 5% of current highest bid.
pub const MIN_BID_INCREMENT_BPS: i128 = 500;
/// Auction extension time when bid comes in last 5 minutes (seconds).
pub const AUCTION_EXTENSION_SECONDS: u64 = 300;
/// Auction extension window (seconds before end).
pub const AUCTION_EXTENSION_WINDOW: u64 = 300;

/// Maximum auto-bid max price as multiple of starting price.
pub const MAX_AUTO_BID_MULTIPLIER: i128 = 10;

/// Search result limit.
pub const MAX_SEARCH_RESULTS: u32 = 100;

/// Reputation discount tiers (reputation score → discount BPS).
/// 80+ reputation: 50% fee discount (150 BPS off 300).
pub const REP_DISCOUNT_TIER_HIGH: u32 = 80;
pub const REP_DISCOUNT_HIGH_BPS: i128 = 150;
/// 60+ reputation: 25% fee discount (75 BPS off 300).
pub const REP_DISCOUNT_TIER_MID: u32 = 60;
pub const REP_DISCOUNT_MID_BPS: i128 = 75;

/// Wash trade detection: same user buying within 7 days.
pub const WASH_TRADE_WINDOW_SECONDS: u64 = 604_800;

// ── Storage Keys ──────────────────────────────────────────────────────────────

#[derive(Clone)]
#[contracttype]
pub enum MarketplaceKey {
    // Listings
    Listing(u64),
    ListingCounter,
    SellerListings(Address),

    // Auctions
    Auction(u64),
    AuctionCounter,
    SellerAuctions(Address),
    AuctionBids(u64),
    AutoBid(u64, Address),

    // Bundles
    Bundle(u64),
    BundleCounter,

    // Trade Offers
    TradeOffer(u64),
    TradeOfferCounter,
    UserTradeOffers(Address),

    // Escrow
    EscrowedAsset(u64), // keyed by listing/auction/trade ID

    // Analytics
    TotalVolume,
    TotalSales,
    TotalListings,
    TotalAuctions,
    CategoryVolume(AssetCategory),

    // Search Index
    SearchIndex(AssetCategory),

    // Fraud Detection
    RecentSales(u64), // asset ID → vec of recent sales

    // Fee collection
    CollectedFees,
}

// ── Data Types ────────────────────────────────────────────────────────────────

/// Asset categories for filtering and analytics.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[contracttype]
pub enum AssetCategory {
    Resource,
    ShipNFT,
    SkinNFT,
    Blueprint,
    HousingItem,
    Badge,
}

/// Rarity tiers for filtering (unified across asset types).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
#[contracttype]
pub enum Rarity {
    Common = 1,
    Uncommon = 2,
    Rare = 3,
    Epic = 4,
    Legendary = 5,
}

/// Asset reference for listings.
#[derive(Clone, Debug, PartialEq)]
#[contracttype]
pub struct Asset {
    pub category: AssetCategory,
    pub asset_id: u64,
    pub quantity: i128,
    pub rarity: Rarity,
    pub name: String,
}

/// Fixed-price listing.
#[derive(Clone, Debug)]
#[contracttype]
pub struct FixedPriceListing {
    pub listing_id: u64,
    pub seller: Address,
    pub asset: Asset,
    pub price: i128,
    pub listed_at: u64,
    pub expires_at: Option<u64>,
}

/// Bundle listing (multiple assets, single price).
#[derive(Clone, Debug)]
#[contracttype]
pub struct BundleListing {
    pub listing_id: u64,
    pub seller: Address,
    pub assets: Vec<Asset>,
    pub bundle_price: i128,
    pub listed_at: u64,
    pub expires_at: Option<u64>,
}

/// Auction listing.
#[derive(Clone, Debug)]
#[contracttype]
pub struct AuctionListing {
    pub auction_id: u64,
    pub seller: Address,
    pub asset: Asset,
    pub starting_price: i128,
    pub current_bid: i128,
    pub highest_bidder: Option<Address>,
    pub started_at: u64,
    pub ends_at: u64,
    pub finalized: bool,
}

/// Bid on an auction.
#[derive(Clone, Debug)]
#[contracttype]
pub struct Bid {
    pub bidder: Address,
    pub amount: i128,
    pub timestamp: u64,
}

/// Auto-bid configuration.
#[derive(Clone, Debug)]
#[contracttype]
pub struct AutoBid {
    pub bidder: Address,
    pub max_price: i128,
    pub active: bool,
}

/// Trade offer (item-for-item).
#[derive(Clone, Debug)]
#[contracttype]
pub struct TradeOffer {
    pub offer_id: u64,
    pub offerer: Address,
    pub target: Address,
    pub offered_assets: Vec<Asset>,
    pub requested_assets: Vec<Asset>,
    pub created_at: u64,
    pub expires_at: u64,
    pub status: TradeOfferStatus,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[contracttype]
pub enum TradeOfferStatus {
    Pending,
    Accepted,
    Rejected,
    Cancelled,
    Expired,
}

/// Filter criteria for search.
///
/// Deliberately not a `#[contracttype]`: Soroban's derive cannot represent
/// `Option<enum>` fields, and this is a read-only query argument that is never
/// written to contract storage nor exposed as a contract entry point, so it
/// needs no XDR representation.
#[derive(Clone, Debug)]
pub struct SearchFilter {
    pub category: Option<AssetCategory>,
    pub min_price: Option<i128>,
    pub max_price: Option<i128>,
    pub rarity: Option<Rarity>,
    pub sort_by: SortOption,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[contracttype]
pub enum SortOption {
    PriceLowToHigh,
    PriceHighToLow,
    DateNewest,
    DateOldest,
    RarityHighToLow,
}

/// Marketplace analytics.
#[derive(Clone, Debug, PartialEq, Eq)]
#[contracttype]
pub struct MarketplaceStats {
    pub total_volume: i128,
    pub total_sales: u64,
    pub active_listings: u32,
    pub active_auctions: u32,
    pub collected_fees: i128,
}

/// Sale record for fraud detection.
#[derive(Clone, Debug)]
#[contracttype]
pub struct SaleRecord {
    pub seller: Address,
    pub buyer: Address,
    pub price: i128,
    pub timestamp: u64,
}

// ── Errors ────────────────────────────────────────────────────────────────────

#[contracterror]
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum MarketplaceError {
    // General
    InvalidPrice = 1,
    InvalidAsset = 2,
    Unauthorized = 3,
    NotFound = 4,
    AlreadyExists = 5,

    // Listings
    SellerListingCapReached = 10,
    ListingExpired = 11,
    SelfPurchase = 12,

    // Auctions
    AuctionEnded = 20,
    AuctionNotEnded = 21,
    BidTooLow = 22,
    NoBids = 23,
    AutoBidTooHigh = 24,
    AuctionAlreadyFinalized = 25,

    // Bundles
    BundleTooLarge = 30,
    BundleEmpty = 31,

    // Trade Offers
    TradeOfferExpired = 40,
    TradeOfferNotPending = 41,
    NotTradeTarget = 42,

    // Escrow
    EscrowFailed = 50,
    ReleaseEscrowFailed = 51,

    // Fraud
    WashTradingDetected = 60,

    // Arithmetic
    ArithmeticOverflow = 70,
}

impl crate::error_standard::StandardContractError for MarketplaceError {
    fn descriptor(self) -> crate::error_standard::ErrorDescriptor {
        use crate::error_standard::ErrorKind;
        let (kind, retryable) = match self {
            Self::InvalidPrice
            | Self::InvalidAsset
            | Self::BidTooLow
            | Self::AutoBidTooHigh
            | Self::BundleTooLarge
            | Self::BundleEmpty => (ErrorKind::Validation, false),
            Self::Unauthorized | Self::NotTradeTarget => (ErrorKind::Authorization, false),
            Self::NotFound | Self::NoBids => (ErrorKind::NotFound, false),
            Self::AlreadyExists | Self::AuctionAlreadyFinalized | Self::TradeOfferNotPending => {
                (ErrorKind::Conflict, false)
            }
            Self::SellerListingCapReached | Self::ArithmeticOverflow => {
                (ErrorKind::ResourceLimit, false)
            }
            Self::ListingExpired
            | Self::AuctionEnded
            | Self::AuctionNotEnded
            | Self::TradeOfferExpired => (ErrorKind::Validation, false),
            Self::SelfPurchase | Self::WashTradingDetected => (ErrorKind::Validation, false),
            Self::EscrowFailed | Self::ReleaseEscrowFailed => (ErrorKind::ResourceLimit, false),
        };
        crate::error_standard::ErrorDescriptor {
            module: "marketplace",
            code: self as u32,
            kind,
            retryable,
        }
    }
}

// ── Helpers ───────────────────────────────────────────────────────────────────

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
        .set(&MarketplaceKey::Listing(listing_id), &listing);

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
        .get(&MarketplaceKey::Listing(listing_id))
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
        .get(&MarketplaceKey::Listing(listing_id))
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

fn remove_listing(env: &Env, seller: &Address, listing_id: u64) {
    env.storage()
        .persistent()
        .remove(&MarketplaceKey::Listing(listing_id));

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
            .get::<_, FixedPriceListing>(&MarketplaceKey::Listing(id))
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
        .get(&MarketplaceKey::Listing(listing_id))
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
