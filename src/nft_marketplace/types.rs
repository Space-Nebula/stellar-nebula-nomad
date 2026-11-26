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
    NftMarketListing(u64),
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
    ListingNotExpired = 13,

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
