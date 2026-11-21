//! Player Housing System — Issue #532
//!
//! Personal space where players can showcase achievements, customize appearance,
//! and visit other players' homes. Houses are instanced areas unique to each player,
//! expandable through upgrades, and support decorative and functional items.
//!
//! ## Features
//!
//! - **Housing Instances**: Each player has a unique personal space
//! - **Size Tiers**: Small → Medium → Large with upgrade paths
//! - **Customization**: Walls, floors, lighting, furniture placement
//! - **Achievement Display**: Show off earned badges and accomplishments
//! - **Functional Items**: Crafting stations, storage expansion
//! - **Visiting System**: Public/private/friends-only access control
//! - **Rating System**: Players can rate visited houses (1-5 stars)
//! - **Discovery**: Browse trending and popular houses
//!
//! ## Storage Layout
//!
//! ```text
//! HousingKey::House(owner)       → HouseInstance
//! HousingKey::Furniture(id)      → FurnitureItem
//! HousingKey::Placement(owner)   → Vec<PlacedFurniture>
//! HousingKey::Rating(owner)      → HouseRating
//! HousingKey::Visitors(owner)    → Vec<VisitorRecord>
//! HousingKey::Discovery(tier)    → Vec<Address> (featured houses)
//! HousingKey::Inventory(owner)    → Map<u64, u32> (owned, unplaced furniture)
//! HousingKey::Storage(owner)      → u32 (storage expansions bought)
//! HousingKey::Displays(owner)     → Vec<AchievementDisplay>
//! HousingKey::Banner(owner)       → GuildBanner
//! HousingKey::Guests(owner)       → Vec<Address> (friends-only guest list)
//! ```
//!
//! Completed in #629: furniture ownership (buy → inventory → place → back to
//! inventory), theme limits per size, achievement display cases, guild
//! banners, storage expansion and a guest list for friends-only houses. See
//! `docs/PLAYER_HOUSING.md`.

use soroban_sdk::{
    contracterror, contracttype, symbol_short, Address, Env, Map, String, Symbol, Vec,
};

use crate::achievements;
use crate::alliance_manager;
use crate::player_profile::{self, ProfileError};
use crate::resource_minter::{self, ResourceType};

// ─── Constants ────────────────────────────────────────────────────────────────

/// Maximum furniture items per house (bounds iteration).
pub const MAX_FURNITURE_PER_HOUSE: u32 = 200;

/// Maximum visitors tracked per house.
pub const MAX_VISITOR_RECORDS: u32 = 50;

/// Maximum featured houses per discovery tier.
pub const MAX_FEATURED_HOUSES: u32 = 20;

/// Maximum furniture categories.
pub const MAX_FURNITURE_CATEGORIES: u32 = 10;

/// Storage slots per expansion purchase.
pub const STORAGE_PER_EXPANSION: u32 = 10;

/// Extra storage slots per placed Storage-category furniture item.
pub const STORAGE_PER_STORAGE_ITEM: u32 = 5;

/// Maximum storage expansions per house.
pub const MAX_STORAGE_EXPANSIONS: u32 = 5;

/// Stellar Dust cost of the first storage expansion; each next one costs more.
pub const STORAGE_EXPANSION_BASE_COST: u64 = 1_000;

/// Achievement slots provided by each placed Achievement-category item.
pub const SLOTS_PER_DISPLAY_CASE: u32 = 3;

/// Maximum guests on a friends-only guest list.
pub const MAX_GUESTS: u32 = 50;

// ── PR #636 compatibility constants ─────────────────────────────────────────
// These keep the symbols exported by the earlier housing branch available.
// The canonical storage model remains the upstream (#629) one above.

/// Upper bound used by the legacy display-case API.
pub const MAX_ACHIEVEMENT_DISPLAYS: u32 = 24;
/// Alias for [`MAX_STORAGE_EXPANSIONS`].
pub const MAX_STORAGE_LEVEL: u32 = MAX_STORAGE_EXPANSIONS;
/// Small-house base capacity, for legacy capacity reporting.
pub const BASE_STORAGE_CAPACITY: u32 = 10;
/// Alias for [`STORAGE_PER_EXPANSION`].
pub const STORAGE_CAPACITY_PER_LEVEL: u32 = STORAGE_PER_EXPANSION;
/// Alias for [`STORAGE_EXPANSION_BASE_COST`].
pub const STORAGE_BASE_COST: u64 = STORAGE_EXPANSION_BASE_COST;

/// Auto-increment counter for furniture IDs.
const FURNITURE_COUNTER: Symbol = symbol_short!("fnc");

// ─── Storage Keys ─────────────────────────────────────────────────────────────

#[derive(Clone)]
#[contracttype]
pub enum HousingKey {
    /// House instance for an owner.
    House(Address),
    /// Global furniture item definition.
    Furniture(u64),
    /// Placed furniture for an owner.
    Placement(Address),
    /// Rating record for a house.
    Rating(Address),
    /// Visitor records for a house.
    Visitors(Address),
    /// Featured houses for a discovery tier.
    Discovery(Symbol),
    /// Furniture counter.
    HousingCounter,
    /// Owned furniture not yet placed: furniture_id -> count.
    Inventory(Address),
    /// Storage expansions bought.
    Storage(Address),
    /// Achievements on display.
    Displays(Address),
    /// Guild banner on display.
    Banner(Address),
    /// Guest list for friends-only access.
    Guests(Address),
    /// Motto shown with the guild banner (PR #636 compatibility).
    GuildMotto(Address),
}

// ─── Data Types ───────────────────────────────────────────────────────────────

/// House size determines max furniture slots and available features.
#[contracttype]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HouseSize {
    Small = 1,
    Medium = 2,
    Large = 3,
}

/// Access control for house visiting.
#[contracttype]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HouseAccess {
    /// Anyone can visit.
    Public,
    /// Only owner can enter.
    Private,
    /// Only bonded players or guild members.
    FriendsOnly,
}

/// Categories for organizing furniture items.
#[contracttype]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FurnitureCategory {
    Wall = 1,
    Floor = 2,
    Lighting = 3,
    Seating = 4,
    Storage = 5,
    Crafting = 6,
    Achievement = 7,
    Decoration = 8,
    Outdoor = 9,
    Functional = 10,
}

/// Rarity tier for furniture (affects cost and availability).
#[contracttype]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FurnitureRarity {
    Common = 1,
    Uncommon = 2,
    Rare = 3,
    Epic = 4,
    Legendary = 5,
}

/// A purchasable or earnable furniture item.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FurnitureItem {
    pub furniture_id: u64,
    pub name: String,
    pub category: FurnitureCategory,
    pub rarity: FurnitureRarity,
    /// Cost in resources to acquire.
    pub cost_resource: ResourceType,
    pub cost_amount: u64,
    /// Whether this item provides functional benefits.
    pub is_functional: bool,
    /// Bonus description (e.g., "+5% crafting speed").
    pub bonus_description: String,
    /// Whether this is a limited edition seasonal item.
    pub is_limited_edition: bool,
}

/// Placement of furniture in a house (position + rotation).
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlacedFurniture {
    pub furniture_id: u64,
    /// Grid position X (0-999 for normalized coordinates).
    pub x: u32,
    /// Grid position Y (0-999 for normalized coordinates).
    pub y: u32,
    /// Rotation in degrees (0, 90, 180, 270).
    pub rotation: u32,
    /// Layer/z-index for rendering order.
    pub layer: u32,
}

/// A player's house instance.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HouseInstance {
    pub owner: Address,
    pub size: HouseSize,
    pub access: HouseAccess,
    pub created_at: u64,
    pub last_modified: u64,
    /// Theme selection (wall/floor style).
    pub wall_theme: u32,
    pub floor_theme: u32,
    pub lighting_theme: u32,
    /// Total furniture placed.
    pub furniture_count: u32,
    /// Total visits received.
    pub total_visits: u64,
    /// Average rating (scaled by 100, e.g., 450 = 4.5 stars).
    pub average_rating: u32,
    /// Total ratings received.
    pub rating_count: u32,
}

/// Rating and review for a house.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HouseRating {
    pub house_owner: Address,
    /// 1-5 stars (scaled by 100).
    pub average_rating: u32,
    pub total_ratings: u32,
    /// Individual ratings by visitor.
    pub ratings: Vec<(Address, u32)>,
}

/// An achievement shown in a display case.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AchievementDisplay {
    pub slot: u32,
    pub achievement_id: u64,
    pub displayed_at: u64,
}

/// The owner's guild banner.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GuildBanner {
    pub alliance_id: u64,
    pub alliance_name: String,
    pub displayed_at: u64,
}

/// Storage capacity and use for a house.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StorageSummary {
    pub capacity: u32,
    pub used: u32,
    pub expansions: u32,
    /// Stellar Dust cost of the next expansion (0 when maxed out).
    pub next_expansion_cost: u64,
}

/// Legacy storage expansion record exposed by PR #636. The values are derived
/// from the canonical [`StorageSummary`]; no separate storage is written.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HouseStorage {
    pub owner: Address,
    /// Number of completed expansions.
    pub level: u32,
    /// Capacity granted by the house size alone.
    pub base_capacity: u32,
    /// Capacity granted by expansions and Storage-category furniture.
    pub bonus_capacity: u32,
    pub last_expanded_at: u64,
}

/// Everything a visitor sees when entering a house, in one read.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HouseShowcase {
    pub house: HouseInstance,
    pub placements: Vec<PlacedFurniture>,
    pub displays: Vec<AchievementDisplay>,
    /// The guild banner, if any (empty or one entry: `Option` of a custom
    /// type can't be a `#[contracttype]` field).
    pub banner: Vec<GuildBanner>,
    pub storage: StorageSummary,
}

/// Record of a visitor to a house.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VisitorRecord {
    pub visitor: Address,
    pub visited_at: u64,
    pub left_rating: bool,
}

// ─── Errors ───────────────────────────────────────────────────────────────────

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum HousingError {
    /// House does not exist for this owner.
    HouseNotFound = 1,
    /// Furniture item does not exist.
    FurnitureNotFound = 2,
    /// House already exists for this owner.
    HouseAlreadyExists = 3,
    /// Insufficient resources to purchase/upgrade.
    InsufficientResources = 4,
    /// House is at maximum size.
    MaxSizeReached = 5,
    /// Too many furniture items placed.
    FurnitureLimitReached = 6,
    /// Invalid furniture placement (out of bounds).
    InvalidPlacement = 7,
    /// Access denied (house is private).
    AccessDenied = 8,
    /// Invalid rating value (must be 1-5).
    InvalidRating = 9,
    /// Cannot rate your own house.
    CannotRateSelf = 10,
    /// Already rated this house.
    AlreadyRated = 11,
    /// Visitor record limit reached.
    VisitorLimitReached = 12,
    /// Featured houses limit reached.
    FeaturedLimitReached = 13,
    /// Profile not found.
    ProfileNotFound = 14,
    /// Arithmetic overflow.
    ArithmeticOverflow = 15,
    /// Invalid house access mode.
    InvalidAccess = 16,
    /// Furniture already placed at this position.
    PositionOccupied = 17,
    /// Theme ID is not available at this house size.
    InvalidTheme = 18,
    /// Storage has no free slot.
    StorageFull = 19,
    /// The owner has no unplaced copy of this furniture.
    NotOwned = 20,
    /// No more storage expansions are available.
    StorageMaxed = 21,
    /// The owner has not unlocked this achievement.
    AchievementNotEarned = 22,
    /// No display case slot is available for this slot index.
    NoDisplayCase = 23,
    /// The owner is not in a guild.
    NotInGuild = 24,
    /// A visitor must visit a house before rating it.
    MustVisitFirst = 25,
    /// The guest list is full.
    GuestListFull = 26,
}

impl crate::error_standard::StandardContractError for HousingError {
    fn descriptor(self) -> crate::error_standard::ErrorDescriptor {
        use crate::error_standard::ErrorKind;
        let (kind, retryable) = match self {
            Self::HouseNotFound | Self::FurnitureNotFound | Self::ProfileNotFound => {
                (ErrorKind::NotFound, false)
            }
            Self::HouseAlreadyExists | Self::AlreadyRated | Self::CannotRateSelf => {
                (ErrorKind::Conflict, false)
            }
            Self::InsufficientResources => (ErrorKind::ResourceLimit, true),
            Self::MaxSizeReached
            | Self::FurnitureLimitReached
            | Self::VisitorLimitReached
            | Self::FeaturedLimitReached => (ErrorKind::ResourceLimit, false),
            Self::InvalidPlacement
            | Self::InvalidRating
            | Self::InvalidAccess
            | Self::PositionOccupied
            | Self::InvalidTheme
            | Self::NoDisplayCase => (ErrorKind::Validation, false),
            Self::StorageFull | Self::StorageMaxed | Self::GuestListFull => {
                (ErrorKind::ResourceLimit, false)
            }
            Self::NotOwned
            | Self::AchievementNotEarned
            | Self::NotInGuild
            | Self::MustVisitFirst => (ErrorKind::Conflict, false),
            Self::AccessDenied => (ErrorKind::Authorization, false),
            Self::ArithmeticOverflow => (ErrorKind::Internal, false),
        };
        crate::error_standard::ErrorDescriptor {
            module: "player_housing",
            code: self as u32,
            kind,
            retryable,
        }
    }
}

impl From<ProfileError> for HousingError {
    fn from(e: ProfileError) -> Self {
        match e {
            ProfileError::ArithmeticOverflow => HousingError::ArithmeticOverflow,
            _ => HousingError::ProfileNotFound,
        }
    }
}

// ─── House Management ─────────────────────────────────────────────────────────

/// Initialize a house for a player (starts at Small size).
pub fn initialize_house(env: &Env, owner: Address) -> Result<HouseInstance, HousingError> {
    owner.require_auth();

    if env
        .storage()
        .persistent()
        .has(&HousingKey::House(owner.clone()))
    {
        return Err(HousingError::HouseAlreadyExists);
    }

    // Verify player has a profile
    player_profile::get_profile_by_owner(env, &owner)?;

    let now = env.ledger().timestamp();
    let house = HouseInstance {
        owner: owner.clone(),
        size: HouseSize::Small,
        access: HouseAccess::Public,
        created_at: now,
        last_modified: now,
        wall_theme: 1,
        floor_theme: 1,
        lighting_theme: 1,
        furniture_count: 0,
        total_visits: 0,
        average_rating: 0,
        rating_count: 0,
    };

    env.storage()
        .persistent()
        .set(&HousingKey::House(owner.clone()), &house);

    // Initialize empty placement
    env.storage().persistent().set(
        &HousingKey::Placement(owner.clone()),
        &Vec::<PlacedFurniture>::new(env),
    );

    // Initialize empty rating
    env.storage().persistent().set(
        &HousingKey::Rating(owner.clone()),
        &HouseRating {
            house_owner: owner.clone(),
            average_rating: 0,
            total_ratings: 0,
            ratings: Vec::new(env),
        },
    );

    env.events().publish(
        (symbol_short!("housing"), symbol_short!("init")),
        (owner.clone(), HouseSize::Small),
    );

    Ok(house)
}

/// Upgrade house to the next size tier.
pub fn upgrade_house(env: &Env, owner: Address) -> Result<HouseInstance, HousingError> {
    owner.require_auth();

    let mut house: HouseInstance = env
        .storage()
        .persistent()
        .get(&HousingKey::House(owner.clone()))
        .ok_or(HousingError::HouseNotFound)?;

    let (new_size, cost) = match house.size {
        HouseSize::Small => (HouseSize::Medium, 5000u64),
        HouseSize::Medium => (HouseSize::Large, 15000u64),
        HouseSize::Large => return Err(HousingError::MaxSizeReached),
    };

    // Debit upgrade cost
    resource_minter::debit_balance(env, &owner, &ResourceType::StellarDust, cost)
        .map_err(|_| HousingError::InsufficientResources)?;

    house.size = new_size;
    house.last_modified = env.ledger().timestamp();

    env.storage()
        .persistent()
        .set(&HousingKey::House(owner.clone()), &house);

    env.events().publish(
        (symbol_short!("housing"), symbol_short!("upgrade")),
        (owner, new_size, cost),
    );

    Ok(house)
}

/// Set house access mode.
pub fn set_house_access(
    env: &Env,
    owner: Address,
    access: HouseAccess,
) -> Result<(), HousingError> {
    owner.require_auth();

    let mut house: HouseInstance = env
        .storage()
        .persistent()
        .get(&HousingKey::House(owner.clone()))
        .ok_or(HousingError::HouseNotFound)?;

    house.access = access;
    house.last_modified = env.ledger().timestamp();

    env.storage()
        .persistent()
        .set(&HousingKey::House(owner.clone()), &house);

    env.events().publish(
        (symbol_short!("housing"), symbol_short!("access")),
        (owner, access),
    );

    Ok(())
}

/// Customize house theme (walls, floors, lighting).
pub fn customize_theme(
    env: &Env,
    owner: Address,
    wall_theme: u32,
    floor_theme: u32,
    lighting_theme: u32,
) -> Result<(), HousingError> {
    owner.require_auth();

    let mut house: HouseInstance = env
        .storage()
        .persistent()
        .get(&HousingKey::House(owner.clone()))
        .ok_or(HousingError::HouseNotFound)?;

    let max_theme = themes_for(house.size);
    for theme in [wall_theme, floor_theme, lighting_theme] {
        if theme == 0 || theme > max_theme {
            return Err(HousingError::InvalidTheme);
        }
    }

    house.wall_theme = wall_theme;
    house.floor_theme = floor_theme;
    house.lighting_theme = lighting_theme;
    house.last_modified = env.ledger().timestamp();

    env.storage()
        .persistent()
        .set(&HousingKey::House(owner.clone()), &house);

    env.events().publish(
        (symbol_short!("housing"), symbol_short!("theme")),
        (owner, wall_theme, floor_theme, lighting_theme),
    );

    Ok(())
}

// ─── Furniture Management ─────────────────────────────────────────────────────

/// Define a new furniture item (admin/creator only).
pub fn define_furniture(
    env: &Env,
    creator: Address,
    name: String,
    category: FurnitureCategory,
    rarity: FurnitureRarity,
    cost_resource: ResourceType,
    cost_amount: u64,
    is_functional: bool,
    bonus_description: String,
    is_limited_edition: bool,
) -> Result<u64, HousingError> {
    creator.require_auth();

    let furniture_id: u64 = env
        .storage()
        .persistent()
        .get::<HousingKey, u64>(&HousingKey::HousingCounter)
        .unwrap_or(0)
        .saturating_add(1);

    env.storage()
        .persistent()
        .set(&HousingKey::HousingCounter, &furniture_id);

    let item = FurnitureItem {
        furniture_id,
        name: name.clone(),
        category,
        rarity,
        cost_resource,
        cost_amount,
        is_functional,
        bonus_description,
        is_limited_edition,
    };

    env.storage()
        .persistent()
        .set(&HousingKey::Furniture(furniture_id), &item);

    env.events().publish(
        (symbol_short!("housing"), symbol_short!("frnew")),
        (furniture_id, name, category, rarity),
    );

    Ok(furniture_id)
}

/// Purchase furniture and add to house (does not place it yet).
pub fn purchase_furniture(
    env: &Env,
    owner: Address,
    furniture_id: u64,
) -> Result<(), HousingError> {
    owner.require_auth();

    let house: HouseInstance = env
        .storage()
        .persistent()
        .get(&HousingKey::House(owner.clone()))
        .ok_or(HousingError::HouseNotFound)?;

    let item: FurnitureItem = env
        .storage()
        .persistent()
        .get(&HousingKey::Furniture(furniture_id))
        .ok_or(HousingError::FurnitureNotFound)?;

    let summary = storage_summary(env, &owner, &house);
    if summary.used >= summary.capacity {
        return Err(HousingError::StorageFull);
    }

    // Debit cost
    resource_minter::debit_balance(env, &owner, &item.cost_resource, item.cost_amount)
        .map_err(|_| HousingError::InsufficientResources)?;

    let mut inventory = get_inventory(env, &owner);
    inventory.set(furniture_id, inventory.get(furniture_id).unwrap_or(0) + 1);
    env.storage()
        .persistent()
        .set(&HousingKey::Inventory(owner.clone()), &inventory);

    env.events().publish(
        (symbol_short!("housing"), symbol_short!("buy")),
        (owner, furniture_id, item.cost_amount),
    );

    Ok(())
}

/// Place furniture in house at specified coordinates.
pub fn place_furniture(
    env: &Env,
    owner: Address,
    furniture_id: u64,
    x: u32,
    y: u32,
    rotation: u32,
    layer: u32,
) -> Result<(), HousingError> {
    owner.require_auth();

    let mut house: HouseInstance = env
        .storage()
        .persistent()
        .get(&HousingKey::House(owner.clone()))
        .ok_or(HousingError::HouseNotFound)?;

    // Verify furniture exists
    if !env
        .storage()
        .persistent()
        .has(&HousingKey::Furniture(furniture_id))
    {
        return Err(HousingError::FurnitureNotFound);
    }

    // Check placement bounds (normalized 0-999)
    if x > 999 || y > 999 {
        return Err(HousingError::InvalidPlacement);
    }

    // Check rotation is valid (0, 90, 180, 270)
    if rotation != 0 && rotation != 90 && rotation != 180 && rotation != 270 {
        return Err(HousingError::InvalidPlacement);
    }

    let max_furniture = match house.size {
        HouseSize::Small => 50u32,
        HouseSize::Medium => 100u32,
        HouseSize::Large => MAX_FURNITURE_PER_HOUSE,
    };

    if house.furniture_count >= max_furniture {
        return Err(HousingError::FurnitureLimitReached);
    }

    let mut placements: Vec<PlacedFurniture> = env
        .storage()
        .persistent()
        .get(&HousingKey::Placement(owner.clone()))
        .unwrap_or_else(|| Vec::new(env));

    for existing in placements.iter() {
        if existing.x == x && existing.y == y && existing.layer == layer {
            return Err(HousingError::PositionOccupied);
        }
    }

    let mut inventory = get_inventory(env, &owner);
    let owned = inventory.get(furniture_id).unwrap_or(0);
    if owned == 0 {
        return Err(HousingError::NotOwned);
    }
    if owned == 1 {
        inventory.remove(furniture_id);
    } else {
        inventory.set(furniture_id, owned - 1);
    }
    env.storage()
        .persistent()
        .set(&HousingKey::Inventory(owner.clone()), &inventory);

    let placed = PlacedFurniture {
        furniture_id,
        x,
        y,
        rotation,
        layer,
    };

    placements.push_back(placed);
    house.furniture_count = placements.len();
    house.last_modified = env.ledger().timestamp();

    env.storage()
        .persistent()
        .set(&HousingKey::Placement(owner.clone()), &placements);
    env.storage()
        .persistent()
        .set(&HousingKey::House(owner.clone()), &house);

    env.events().publish(
        (symbol_short!("housing"), symbol_short!("place")),
        (owner, furniture_id, x, y),
    );

    Ok(())
}

/// Remove furniture from house.
pub fn remove_furniture(env: &Env, owner: Address, furniture_id: u64) -> Result<(), HousingError> {
    owner.require_auth();

    let mut house: HouseInstance = env
        .storage()
        .persistent()
        .get(&HousingKey::House(owner.clone()))
        .ok_or(HousingError::HouseNotFound)?;

    let placements: Vec<PlacedFurniture> = env
        .storage()
        .persistent()
        .get(&HousingKey::Placement(owner.clone()))
        .unwrap_or_else(|| Vec::new(env));

    let mut new_placements = Vec::new(env);
    let mut found = false;

    for placed in placements.iter() {
        if placed.furniture_id == furniture_id && !found {
            found = true;
            continue;
        }
        new_placements.push_back(placed);
    }

    if !found {
        return Err(HousingError::FurnitureNotFound);
    }

    // The piece goes back into storage; refuse if there is no room for it.
    let summary = storage_summary(env, &owner, &house);
    let removed_storage_slots = match get_furniture(env, furniture_id) {
        Some(item) if item.category == FurnitureCategory::Storage => STORAGE_PER_STORAGE_ITEM,
        _ => 0,
    };
    if summary.used + 1 > summary.capacity.saturating_sub(removed_storage_slots) {
        return Err(HousingError::StorageFull);
    }
    let mut inventory = get_inventory(env, &owner);
    inventory.set(furniture_id, inventory.get(furniture_id).unwrap_or(0) + 1);
    env.storage()
        .persistent()
        .set(&HousingKey::Inventory(owner.clone()), &inventory);

    house.furniture_count = new_placements.len();
    house.last_modified = env.ledger().timestamp();

    env.storage()
        .persistent()
        .set(&HousingKey::Placement(owner.clone()), &new_placements);
    env.storage()
        .persistent()
        .set(&HousingKey::House(owner.clone()), &house);

    // Removing a display case may drop achievement slots.
    let slots = display_slots(env, &owner, &house);
    let displays = get_achievement_displays(env, &owner);
    let mut kept = Vec::new(env);
    for display in displays.iter() {
        if display.slot < slots {
            kept.push_back(display);
        }
    }
    env.storage()
        .persistent()
        .set(&HousingKey::Displays(owner.clone()), &kept);

    env.events().publish(
        (symbol_short!("housing"), symbol_short!("remove")),
        (owner, furniture_id),
    );

    Ok(())
}

// ─── Visiting System ──────────────────────────────────────────────────────────

/// Visit another player's house.
pub fn visit_house(env: &Env, visitor: Address, house_owner: Address) -> Result<(), HousingError> {
    visitor.require_auth();

    let mut house: HouseInstance = env
        .storage()
        .persistent()
        .get(&HousingKey::House(house_owner.clone()))
        .ok_or(HousingError::HouseNotFound)?;

    // Check access control
    match house.access {
        HouseAccess::Private => {
            if visitor != house_owner {
                return Err(HousingError::AccessDenied);
            }
        }
        HouseAccess::FriendsOnly => {
            if !is_friend(env, &house_owner, &visitor) {
                return Err(HousingError::AccessDenied);
            }
        }
        HouseAccess::Public => {}
    }

    // Record visit
    house.total_visits = house.total_visits.saturating_add(1);

    let mut visitors: Vec<VisitorRecord> = env
        .storage()
        .persistent()
        .get(&HousingKey::Visitors(house_owner.clone()))
        .unwrap_or_else(|| Vec::new(env));

    // Keep the most recent MAX_VISITOR_RECORDS visits.
    if visitors.len() >= MAX_VISITOR_RECORDS {
        visitors.pop_front();
    }
    visitors.push_back(VisitorRecord {
        visitor: visitor.clone(),
        visited_at: env.ledger().timestamp(),
        left_rating: false,
    });
    env.storage()
        .persistent()
        .set(&HousingKey::Visitors(house_owner.clone()), &visitors);

    env.storage()
        .persistent()
        .set(&HousingKey::House(house_owner.clone()), &house);

    env.events().publish(
        (symbol_short!("housing"), symbol_short!("visit")),
        (visitor, house_owner),
    );

    Ok(())
}

/// Rate a house (1-5 stars, scaled by 100).
pub fn rate_house(
    env: &Env,
    visitor: Address,
    house_owner: Address,
    rating: u32,
) -> Result<(), HousingError> {
    visitor.require_auth();

    if visitor == house_owner {
        return Err(HousingError::CannotRateSelf);
    }

    // Rating must be 100-500 (1.0 to 5.0 stars)
    if rating < 100 || rating > 500 {
        return Err(HousingError::InvalidRating);
    }

    let mut house: HouseInstance = env
        .storage()
        .persistent()
        .get(&HousingKey::House(house_owner.clone()))
        .ok_or(HousingError::HouseNotFound)?;

    let mut house_rating: HouseRating = env
        .storage()
        .persistent()
        .get(&HousingKey::Rating(house_owner.clone()))
        .unwrap_or_else(|| HouseRating {
            house_owner: house_owner.clone(),
            average_rating: 0,
            total_ratings: 0,
            ratings: Vec::new(env),
        });

    // Check if already rated
    for (rater, _) in house_rating.ratings.iter() {
        if rater == visitor {
            return Err(HousingError::AlreadyRated);
        }
    }

    // Only visitors can rate; mark their latest visit as rated.
    let mut visitors = get_visitors(env, &house_owner);
    let mut latest = None;
    for (index, record) in visitors.iter().enumerate() {
        if record.visitor == visitor {
            latest = Some(index as u32);
        }
    }
    let Some(index) = latest else {
        return Err(HousingError::MustVisitFirst);
    };
    let mut record = visitors.get(index).ok_or(HousingError::MustVisitFirst)?;
    record.left_rating = true;
    visitors.set(index, record);
    env.storage()
        .persistent()
        .set(&HousingKey::Visitors(house_owner.clone()), &visitors);

    // Add rating
    house_rating.ratings.push_back((visitor.clone(), rating));
    house_rating.total_ratings = house_rating.total_ratings.saturating_add(1);

    // Recalculate average
    let mut sum: u64 = 0;
    for (_, r) in house_rating.ratings.iter() {
        sum = sum.saturating_add(r as u64);
    }
    house_rating.average_rating = (sum / house_rating.total_ratings as u64) as u32;

    house.average_rating = house_rating.average_rating;
    house.rating_count = house_rating.total_ratings;

    env.storage()
        .persistent()
        .set(&HousingKey::Rating(house_owner.clone()), &house_rating);
    env.storage()
        .persistent()
        .set(&HousingKey::House(house_owner.clone()), &house);

    env.events().publish(
        (symbol_short!("housing"), symbol_short!("rate")),
        (visitor, house_owner, rating),
    );

    Ok(())
}

// ─── Discovery & Featured Houses ──────────────────────────────────────────────

/// Add a house to featured list (admin/curator).
pub fn feature_house(
    env: &Env,
    curator: Address,
    house_owner: Address,
    tier: Symbol,
) -> Result<(), HousingError> {
    curator.require_auth();

    // Verify house exists
    if !env
        .storage()
        .persistent()
        .has(&HousingKey::House(house_owner.clone()))
    {
        return Err(HousingError::HouseNotFound);
    }

    let mut featured: Vec<Address> = env
        .storage()
        .persistent()
        .get(&HousingKey::Discovery(tier.clone()))
        .unwrap_or_else(|| Vec::new(env));

    if featured.len() >= MAX_FEATURED_HOUSES {
        return Err(HousingError::FeaturedLimitReached);
    }

    // Check if already featured
    for addr in featured.iter() {
        if addr == house_owner {
            return Ok(()); // Already featured
        }
    }

    featured.push_back(house_owner.clone());
    env.storage()
        .persistent()
        .set(&HousingKey::Discovery(tier.clone()), &featured);

    env.events().publish(
        (symbol_short!("housing"), symbol_short!("feature")),
        (house_owner, tier),
    );

    Ok(())
}

// ─── Customization limits ─────────────────────────────────────────────────────

/// Number of wall/floor/lighting themes unlocked at a house size.
pub fn themes_for(size: HouseSize) -> u32 {
    match size {
        HouseSize::Small => 5,
        HouseSize::Medium => 10,
        HouseSize::Large => 20,
    }
}

/// Achievement display slots a house size can hold at most.
pub fn max_display_slots(size: HouseSize) -> u32 {
    match size {
        HouseSize::Small => 3,
        HouseSize::Medium => 6,
        HouseSize::Large => 12,
    }
}

/// Base storage slots by house size.
pub fn base_storage(size: HouseSize) -> u32 {
    match size {
        HouseSize::Small => 10,
        HouseSize::Medium => 20,
        HouseSize::Large => 40,
    }
}

fn require_house(env: &Env, owner: &Address) -> Result<HouseInstance, HousingError> {
    env.storage()
        .persistent()
        .get(&HousingKey::House(owner.clone()))
        .ok_or(HousingError::HouseNotFound)
}

fn count_placed(env: &Env, owner: &Address, category: FurnitureCategory) -> u32 {
    let mut count = 0;
    for placed in get_placements(env, owner).iter() {
        if let Some(item) = get_furniture(env, placed.furniture_id) {
            if item.category == category {
                count += 1;
            }
        }
    }
    count
}

// ─── Storage expansion ────────────────────────────────────────────────────────

/// Owned, unplaced furniture: furniture_id -> count.
pub fn get_inventory(env: &Env, owner: &Address) -> Map<u64, u32> {
    env.storage()
        .persistent()
        .get(&HousingKey::Inventory(owner.clone()))
        .unwrap_or_else(|| Map::new(env))
}

fn expansions_of(env: &Env, owner: &Address) -> u32 {
    env.storage()
        .persistent()
        .get(&HousingKey::Storage(owner.clone()))
        .unwrap_or(0)
}

fn expansion_cost(expansions: u32) -> u64 {
    if expansions >= MAX_STORAGE_EXPANSIONS {
        0
    } else {
        STORAGE_EXPANSION_BASE_COST * u64::from(expansions + 1)
    }
}

fn storage_summary(env: &Env, owner: &Address, house: &HouseInstance) -> StorageSummary {
    let expansions = expansions_of(env, owner);
    let capacity = base_storage(house.size)
        + expansions * STORAGE_PER_EXPANSION
        + count_placed(env, owner, FurnitureCategory::Storage) * STORAGE_PER_STORAGE_ITEM;
    let used = get_inventory(env, owner).values().iter().sum();
    StorageSummary {
        capacity,
        used,
        expansions,
        next_expansion_cost: expansion_cost(expansions),
    }
}

/// Storage capacity and use. Capacity grows with house size, expansions and
/// placed Storage-category furniture.
pub fn get_storage(env: &Env, owner: &Address) -> Result<StorageSummary, HousingError> {
    let house = require_house(env, owner)?;
    Ok(storage_summary(env, owner, &house))
}

/// Buy the next storage expansion (+[`STORAGE_PER_EXPANSION`] slots). Each
/// expansion costs [`STORAGE_EXPANSION_BASE_COST`] times its number.
pub fn expand_storage(env: &Env, owner: Address) -> Result<StorageSummary, HousingError> {
    owner.require_auth();
    let house = require_house(env, &owner)?;
    let expansions = expansions_of(env, &owner);
    if expansions >= MAX_STORAGE_EXPANSIONS {
        return Err(HousingError::StorageMaxed);
    }
    let cost = expansion_cost(expansions);
    resource_minter::debit_balance(env, &owner, &ResourceType::StellarDust, cost)
        .map_err(|_| HousingError::InsufficientResources)?;
    env.storage()
        .persistent()
        .set(&HousingKey::Storage(owner.clone()), &(expansions + 1));
    env.events().publish(
        (symbol_short!("housing"), symbol_short!("storage")),
        (owner.clone(), expansions + 1, cost),
    );
    Ok(storage_summary(env, &owner, &house))
}

// ─── Achievement display cases ────────────────────────────────────────────────

fn display_slots(env: &Env, owner: &Address, house: &HouseInstance) -> u32 {
    let cases = count_placed(env, owner, FurnitureCategory::Achievement);
    (cases * SLOTS_PER_DISPLAY_CASE).min(max_display_slots(house.size))
}

/// Achievements currently on display.
pub fn get_achievement_displays(env: &Env, owner: &Address) -> Vec<AchievementDisplay> {
    env.storage()
        .persistent()
        .get(&HousingKey::Displays(owner.clone()))
        .unwrap_or_else(|| Vec::new(env))
}

/// Put an unlocked achievement in a display case slot (replacing whatever was
/// there). Slots come from placed Achievement-category furniture,
/// [`SLOTS_PER_DISPLAY_CASE`] each, up to the house size's maximum.
pub fn display_achievement(
    env: &Env,
    owner: Address,
    achievement_id: u64,
    slot: u32,
) -> Result<Vec<AchievementDisplay>, HousingError> {
    owner.require_auth();
    let house = require_house(env, &owner)?;
    if slot >= display_slots(env, &owner, &house) {
        return Err(HousingError::NoDisplayCase);
    }
    let progress = achievements::query_progress(env, &owner, achievement_id)
        .map_err(|_| HousingError::AchievementNotEarned)?;
    if !progress.unlocked {
        return Err(HousingError::AchievementNotEarned);
    }

    let mut displays = Vec::new(env);
    for display in get_achievement_displays(env, &owner).iter() {
        if display.slot != slot && display.achievement_id != achievement_id {
            displays.push_back(display);
        }
    }
    displays.push_back(AchievementDisplay {
        slot,
        achievement_id,
        displayed_at: env.ledger().timestamp(),
    });
    env.storage()
        .persistent()
        .set(&HousingKey::Displays(owner.clone()), &displays);
    env.events().publish(
        (symbol_short!("housing"), symbol_short!("display")),
        (owner, achievement_id, slot),
    );
    Ok(displays)
}

/// Empty a display slot.
pub fn clear_achievement_display(env: &Env, owner: Address, slot: u32) -> Result<(), HousingError> {
    owner.require_auth();
    require_house(env, &owner)?;
    let mut displays = Vec::new(env);
    for display in get_achievement_displays(env, &owner).iter() {
        if display.slot != slot {
            displays.push_back(display);
        }
    }
    env.storage()
        .persistent()
        .set(&HousingKey::Displays(owner), &displays);
    Ok(())
}

// ─── Guild banners ────────────────────────────────────────────────────────────

/// Hang the banner of the owner's guild (alliance).
pub fn display_guild_banner(env: &Env, owner: Address) -> Result<GuildBanner, HousingError> {
    owner.require_auth();
    require_house(env, &owner)?;
    let alliance_id = alliance_manager::get_player_alliance(env, owner.clone())
        .ok_or(HousingError::NotInGuild)?;
    let alliance =
        alliance_manager::get_alliance(env, alliance_id).map_err(|_| HousingError::NotInGuild)?;
    let banner = GuildBanner {
        alliance_id,
        alliance_name: alliance.name,
        displayed_at: env.ledger().timestamp(),
    };
    env.storage()
        .persistent()
        .set(&HousingKey::Banner(owner.clone()), &banner);
    env.events().publish(
        (symbol_short!("housing"), symbol_short!("banner")),
        (owner, alliance_id),
    );
    Ok(banner)
}

/// Take the guild banner down.
pub fn remove_guild_banner(env: &Env, owner: Address) -> Result<(), HousingError> {
    owner.require_auth();
    require_house(env, &owner)?;
    env.storage()
        .persistent()
        .remove(&HousingKey::Banner(owner));
    Ok(())
}

/// The banner on display, hidden once the owner has left that guild.
pub fn get_guild_banner(env: &Env, owner: &Address) -> Option<GuildBanner> {
    let banner: GuildBanner = env
        .storage()
        .persistent()
        .get(&HousingKey::Banner(owner.clone()))?;
    if alliance_manager::get_player_alliance(env, owner.clone()) == Some(banner.alliance_id) {
        Some(banner)
    } else {
        None
    }
}

// ─── Guests & friends ─────────────────────────────────────────────────────────

/// The friends-only guest list.
pub fn get_guests(env: &Env, owner: &Address) -> Vec<Address> {
    env.storage()
        .persistent()
        .get(&HousingKey::Guests(owner.clone()))
        .unwrap_or_else(|| Vec::new(env))
}

/// Allow a player into a friends-only house.
pub fn add_guest(env: &Env, owner: Address, guest: Address) -> Result<(), HousingError> {
    owner.require_auth();
    require_house(env, &owner)?;
    let mut guests = get_guests(env, &owner);
    if guests.contains(&guest) {
        return Ok(());
    }
    if guests.len() >= MAX_GUESTS {
        return Err(HousingError::GuestListFull);
    }
    guests.push_back(guest.clone());
    env.storage()
        .persistent()
        .set(&HousingKey::Guests(owner.clone()), &guests);
    env.events().publish(
        (symbol_short!("housing"), symbol_short!("guest")),
        (owner, guest, true),
    );
    Ok(())
}

/// Remove a player from the guest list.
pub fn remove_guest(env: &Env, owner: Address, guest: Address) -> Result<(), HousingError> {
    owner.require_auth();
    require_house(env, &owner)?;
    let mut kept = Vec::new(env);
    for g in get_guests(env, &owner).iter() {
        if g != guest {
            kept.push_back(g);
        }
    }
    env.storage()
        .persistent()
        .set(&HousingKey::Guests(owner.clone()), &kept);
    env.events().publish(
        (symbol_short!("housing"), symbol_short!("guest")),
        (owner, guest, false),
    );
    Ok(())
}

/// A visitor counts as a friend if they are the owner, on the guest list, or
/// in the same guild as the owner.
pub fn is_friend(env: &Env, owner: &Address, visitor: &Address) -> bool {
    if owner == visitor || get_guests(env, owner).contains(visitor) {
        return true;
    }
    match alliance_manager::get_player_alliance(env, owner.clone()) {
        Some(alliance) => {
            alliance_manager::get_player_alliance(env, visitor.clone()) == Some(alliance)
        }
        None => false,
    }
}

/// House, furniture layout, achievement displays, banner and storage in one read.
pub fn get_house_showcase(env: &Env, owner: &Address) -> Option<HouseShowcase> {
    let house = get_house(env, owner)?;
    let storage = storage_summary(env, owner, &house);
    Some(HouseShowcase {
        placements: get_placements(env, owner),
        displays: get_achievement_displays(env, owner),
        banner: get_guild_banner(env, owner)
            .map_or_else(|| Vec::new(env), |b| Vec::from_array(env, [b])),
        storage,
        house,
    })
}

// ─── Queries ──────────────────────────────────────────────────────────────────

/// Get house details.
pub fn get_house(env: &Env, owner: &Address) -> Option<HouseInstance> {
    env.storage()
        .persistent()
        .get(&HousingKey::House(owner.clone()))
}

/// Get furniture item definition.
pub fn get_furniture(env: &Env, furniture_id: u64) -> Option<FurnitureItem> {
    env.storage()
        .persistent()
        .get(&HousingKey::Furniture(furniture_id))
}

/// Get all placed furniture in a house.
pub fn get_placements(env: &Env, owner: &Address) -> Vec<PlacedFurniture> {
    env.storage()
        .persistent()
        .get(&HousingKey::Placement(owner.clone()))
        .unwrap_or_else(|| Vec::new(env))
}

/// Get house rating.
pub fn get_rating(env: &Env, owner: &Address) -> Option<HouseRating> {
    env.storage()
        .persistent()
        .get(&HousingKey::Rating(owner.clone()))
}

/// Get visitor records.
pub fn get_visitors(env: &Env, owner: &Address) -> Vec<VisitorRecord> {
    env.storage()
        .persistent()
        .get(&HousingKey::Visitors(owner.clone()))
        .unwrap_or_else(|| Vec::new(env))
}

/// Get featured houses for a tier.
pub fn get_featured_houses(env: &Env, tier: Symbol) -> Vec<Address> {
    env.storage()
        .persistent()
        .get(&HousingKey::Discovery(tier))
        .unwrap_or_else(|| Vec::new(env))
}

// ─── PR #636 compatibility API ───────────────────────────────────────────────
// Thin adapters that preserve the earlier branch's public symbols while
// delegating to the canonical upstream (#629) implementation above.

/// Legacy alias for [`display_achievement`]. Returns the slot used.
pub fn install_achievement_display(
    env: &Env,
    owner: Address,
    achievement_id: u64,
    slot: u32,
) -> Result<u64, HousingError> {
    display_achievement(env, owner, achievement_id, slot)?;
    Ok(u64::from(slot))
}

/// Legacy alias for [`clear_achievement_display`]. The legacy `display_id`
/// is interpreted as the canonical display slot index.
pub fn remove_achievement_display(
    env: &Env,
    owner: Address,
    display_id: u64,
) -> Result<(), HousingError> {
    let slot = u32::try_from(display_id).map_err(|_| HousingError::NoDisplayCase)?;
    clear_achievement_display(env, owner, slot)
}

/// Hang the guild banner and store the legacy motto separately. The banner
/// itself remains the canonical alliance-derived [`GuildBanner`].
pub fn set_guild_banner(env: &Env, owner: Address, motto: String) -> Result<(), HousingError> {
    display_guild_banner(env, owner.clone())?;
    env.storage()
        .persistent()
        .set(&HousingKey::GuildMotto(owner), &motto);
    Ok(())
}

/// Legacy alias for [`remove_guild_banner`], also clearing the stored motto.
pub fn clear_guild_banner(env: &Env, owner: Address) -> Result<(), HousingError> {
    remove_guild_banner(env, owner.clone())?;
    env.storage()
        .persistent()
        .remove(&HousingKey::GuildMotto(owner));
    Ok(())
}

/// Legacy view of storage state, derived from [`get_storage`].
pub fn get_house_storage(env: &Env, owner: &Address) -> Option<HouseStorage> {
    let house: HouseInstance = env
        .storage()
        .persistent()
        .get(&HousingKey::House(owner.clone()))?;
    let summary = storage_summary(env, owner, &house);
    let base = base_storage(house.size);
    Some(HouseStorage {
        owner: owner.clone(),
        level: summary.expansions,
        base_capacity: base,
        bonus_capacity: summary.capacity.saturating_sub(base),
        last_expanded_at: env.ledger().timestamp(),
    })
}

/// Legacy total-capacity accessor.
pub fn get_storage_capacity(env: &Env, owner: &Address) -> u32 {
    get_storage(env, owner)
        .map(|summary| summary.capacity)
        .unwrap_or(0)
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use soroban_sdk::testutils::{Address as _, Ledger};
    use soroban_sdk::{contract, contractimpl};

    #[contract]
    struct Stub;
    #[contractimpl]
    impl Stub {}

    struct Fixture {
        env: Env,
        contract: Address,
        admin: Address,
        player1: Address,
        player2: Address,
    }

    impl Fixture {
        fn run<T>(&self, f: impl FnOnce() -> T) -> T {
            self.env.as_contract(&self.contract, f)
        }
    }

    fn fixture() -> Fixture {
        let env = Env::default();
        env.mock_all_auths();
        let contract = env.register(Stub, ());
        let admin = Address::generate(&env);
        let player1 = Address::generate(&env);
        let player2 = Address::generate(&env);

        // Initialize profiles
        let p1 = player1.clone();
        let p2 = player2.clone();
        env.as_contract(&contract, || {
            player_profile::initialize_profile(&env, p1).unwrap();
            player_profile::initialize_profile(&env, p2).unwrap();
        });

        Fixture {
            env,
            contract,
            admin,
            player1,
            player2,
        }
    }

    #[test]
    fn test_initialize_house_creates_small_house() {
        let f = fixture();
        let house = f.run(|| super::initialize_house(&f.env, f.player1.clone()).unwrap());

        assert_eq!(house.owner, f.player1);
        assert_eq!(house.size, HouseSize::Small);
        assert_eq!(house.access, HouseAccess::Public);
        assert_eq!(house.furniture_count, 0);
        assert_eq!(house.total_visits, 0);
    }

    #[test]
    fn test_cannot_initialize_house_twice() {
        let f = fixture();
        f.run(|| super::initialize_house(&f.env, f.player1.clone()).unwrap());
        f.run(|| {
            assert_eq!(
                super::initialize_house(&f.env, f.player1.clone()),
                Err(HousingError::HouseAlreadyExists)
            );
        });
    }

    #[test]
    fn test_upgrade_house_increases_size() {
        let f = fixture();
        f.run(|| {
            super::initialize_house(&f.env, f.player1.clone()).unwrap();
            // Give player resources
            resource_minter::credit_balance(&f.env, &f.player1, &ResourceType::StellarDust, 10000)
                .unwrap();
        });

        let upgraded = f.run(|| super::upgrade_house(&f.env, f.player1.clone()).unwrap());
        assert_eq!(upgraded.size, HouseSize::Medium);
    }

    #[test]
    fn test_upgrade_fails_without_resources() {
        let f = fixture();
        f.run(|| super::initialize_house(&f.env, f.player1.clone()).unwrap());

        f.run(|| {
            assert_eq!(
                super::upgrade_house(&f.env, f.player1.clone()),
                Err(HousingError::InsufficientResources)
            );
        });
    }

    #[test]
    fn test_set_access_mode() {
        let f = fixture();
        f.run(|| super::initialize_house(&f.env, f.player1.clone()).unwrap());

        f.run(|| super::set_house_access(&f.env, f.player1.clone(), HouseAccess::Private).unwrap());

        let house = f.run(|| super::get_house(&f.env, &f.player1).unwrap());
        assert_eq!(house.access, HouseAccess::Private);
    }

    #[test]
    fn test_customize_theme() {
        let f = fixture();
        f.run(|| super::initialize_house(&f.env, f.player1.clone()).unwrap());

        f.run(|| super::customize_theme(&f.env, f.player1.clone(), 5, 3, 2).unwrap());

        let house = f.run(|| super::get_house(&f.env, &f.player1).unwrap());
        assert_eq!(house.wall_theme, 5);
        assert_eq!(house.floor_theme, 3);
        assert_eq!(house.lighting_theme, 2);
    }

    #[test]
    fn test_define_furniture() {
        let f = fixture();
        let furniture_id = f.run(|| {
            super::define_furniture(
                &f.env,
                f.admin.clone(),
                String::from_str(&f.env, "Cozy Chair"),
                FurnitureCategory::Seating,
                FurnitureRarity::Common,
                ResourceType::StellarDust,
                100,
                false,
                String::from_str(&f.env, "A comfortable chair"),
                false,
            )
            .unwrap()
        });

        assert_eq!(furniture_id, 1);
        let item = f.run(|| super::get_furniture(&f.env, furniture_id).unwrap());
        assert_eq!(item.name, String::from_str(&f.env, "Cozy Chair"));
        assert_eq!(item.category, FurnitureCategory::Seating);
    }

    #[test]
    fn test_place_furniture_in_house() {
        let f = fixture();
        f.run(|| super::initialize_house(&f.env, f.player1.clone()).unwrap());

        let furniture_id = f.run(|| {
            super::define_furniture(
                &f.env,
                f.admin.clone(),
                String::from_str(&f.env, "Table"),
                FurnitureCategory::Decoration,
                FurnitureRarity::Common,
                ResourceType::StellarDust,
                50,
                false,
                String::from_str(&f.env, ""),
                false,
            )
            .unwrap()
        });

        f.run(|| {
            resource_minter::credit_balance(&f.env, &f.player1, &ResourceType::StellarDust, 1000)
                .unwrap();
        });
        f.run(|| super::purchase_furniture(&f.env, f.player1.clone(), furniture_id).unwrap());
        f.run(|| {
            super::place_furniture(&f.env, f.player1.clone(), furniture_id, 100, 200, 90, 1)
                .unwrap()
        });

        f.run(|| {
            let house = super::get_house(&f.env, &f.player1).unwrap();
            assert_eq!(house.furniture_count, 1);

            let placements = super::get_placements(&f.env, &f.player1);
            assert_eq!(placements.len(), 1);
            assert_eq!(placements.get(0).unwrap().x, 100);
            assert_eq!(placements.get(0).unwrap().y, 200);
            assert_eq!(placements.get(0).unwrap().rotation, 90);
        });
    }

    #[test]
    fn test_visiting_public_house_works() {
        let f = fixture();
        f.run(|| {
            super::initialize_house(&f.env, f.player1.clone()).unwrap();
            super::visit_house(&f.env, f.player2.clone(), f.player1.clone()).unwrap();

            let house = super::get_house(&f.env, &f.player1).unwrap();
            assert_eq!(house.total_visits, 1);

            let visitors = super::get_visitors(&f.env, &f.player1);
            assert_eq!(visitors.len(), 1);
            assert_eq!(visitors.get(0).unwrap().visitor, f.player2);
        });
    }

    #[test]
    fn test_visiting_private_house_fails() {
        let f = fixture();
        f.run(|| super::initialize_house(&f.env, f.player1.clone()).unwrap());
        f.run(|| super::set_house_access(&f.env, f.player1.clone(), HouseAccess::Private).unwrap());

        f.run(|| {
            assert_eq!(
                super::visit_house(&f.env, f.player2.clone(), f.player1.clone()),
                Err(HousingError::AccessDenied)
            );
        });
    }

    #[test]
    fn test_rate_house() {
        let f = fixture();
        f.run(|| super::initialize_house(&f.env, f.player1.clone()).unwrap());
        f.run(|| super::visit_house(&f.env, f.player2.clone(), f.player1.clone()).unwrap());
        f.run(|| super::rate_house(&f.env, f.player2.clone(), f.player1.clone(), 450).unwrap());

        let house = f.run(|| super::get_house(&f.env, &f.player1).unwrap());
        assert_eq!(house.average_rating, 450);
        assert_eq!(house.rating_count, 1);
    }

    #[test]
    fn test_cannot_rate_own_house() {
        let f = fixture();
        f.run(|| super::initialize_house(&f.env, f.player1.clone()).unwrap());
        f.run(|| {
            assert_eq!(
                super::rate_house(&f.env, f.player1.clone(), f.player1.clone(), 500),
                Err(HousingError::CannotRateSelf)
            );
        });
    }

    #[test]
    fn test_cannot_rate_twice() {
        let f = fixture();
        f.run(|| super::initialize_house(&f.env, f.player1.clone()).unwrap());
        f.run(|| super::visit_house(&f.env, f.player2.clone(), f.player1.clone()).unwrap());
        f.run(|| super::rate_house(&f.env, f.player2.clone(), f.player1.clone(), 400).unwrap());
        f.run(|| {
            assert_eq!(
                super::rate_house(&f.env, f.player2.clone(), f.player1.clone(), 500),
                Err(HousingError::AlreadyRated)
            );
        });
    }

    #[test]
    fn test_feature_house() {
        let f = fixture();
        f.run(|| {
            super::initialize_house(&f.env, f.player1.clone()).unwrap();
            super::feature_house(
                &f.env,
                f.admin.clone(),
                f.player1.clone(),
                symbol_short!("trending"),
            )
            .unwrap();

            let featured = super::get_featured_houses(&f.env, symbol_short!("trending"));
            assert_eq!(featured.len(), 1);
            assert_eq!(featured.get(0).unwrap(), f.player1);
        });
    }

    #[test]
    fn test_remove_furniture_from_house() {
        let f = fixture();
        f.run(|| super::initialize_house(&f.env, f.player1.clone()).unwrap());

        let furniture_id = f.run(|| {
            super::define_furniture(
                &f.env,
                f.admin.clone(),
                String::from_str(&f.env, "Lamp"),
                FurnitureCategory::Lighting,
                FurnitureRarity::Common,
                ResourceType::StellarDust,
                30,
                false,
                String::from_str(&f.env, ""),
                false,
            )
            .unwrap()
        });

        f.run(|| {
            resource_minter::credit_balance(&f.env, &f.player1, &ResourceType::StellarDust, 1000)
                .unwrap();
        });
        f.run(|| super::purchase_furniture(&f.env, f.player1.clone(), furniture_id).unwrap());
        f.run(|| {
            super::place_furniture(&f.env, f.player1.clone(), furniture_id, 50, 50, 0, 1).unwrap()
        });
        f.run(|| super::remove_furniture(&f.env, f.player1.clone(), furniture_id).unwrap());

        f.run(|| {
            let house = super::get_house(&f.env, &f.player1).unwrap();
            assert_eq!(house.furniture_count, 0);
        });
    }

    // ─── #629 completion ──────────────────────────────────────────────────────

    fn furniture(f: &Fixture, category: FurnitureCategory, cost: u64) -> u64 {
        f.run(|| {
            super::define_furniture(
                &f.env,
                f.admin.clone(),
                String::from_str(&f.env, "Item"),
                category,
                FurnitureRarity::Common,
                ResourceType::StellarDust,
                cost,
                false,
                String::from_str(&f.env, ""),
                false,
            )
            .unwrap()
        })
    }

    fn fund(f: &Fixture, who: &Address, amount: u64) {
        f.run(|| {
            resource_minter::credit_balance(&f.env, who, &ResourceType::StellarDust, amount)
                .unwrap()
        });
    }

    fn house_for(f: &Fixture, who: &Address) {
        f.run(|| super::initialize_house(&f.env, who.clone()).unwrap());
    }

    #[test]
    fn test_theme_ids_are_limited_by_house_size() {
        let f = fixture();
        house_for(&f, &f.player1);
        f.run(|| super::customize_theme(&f.env, f.player1.clone(), 5, 1, 2).unwrap());
        f.run(|| {
            assert_eq!(
                super::customize_theme(&f.env, f.player1.clone(), 6, 1, 1),
                Err(HousingError::InvalidTheme)
            );
        });
        f.run(|| {
            assert_eq!(
                super::customize_theme(&f.env, f.player1.clone(), 1, 0, 1),
                Err(HousingError::InvalidTheme)
            );
        });
        fund(&f, &f.player1, 5_000);
        f.run(|| super::upgrade_house(&f.env, f.player1.clone()).unwrap());
        f.run(|| super::customize_theme(&f.env, f.player1.clone(), 10, 10, 10).unwrap());
    }

    #[test]
    fn test_furniture_must_be_owned_to_place_and_returns_to_inventory() {
        let f = fixture();
        house_for(&f, &f.player1);
        let lamp = furniture(&f, FurnitureCategory::Lighting, 10);
        fund(&f, &f.player1, 1_000);
        f.run(|| {
            assert_eq!(
                super::place_furniture(&f.env, f.player1.clone(), lamp, 1, 1, 0, 0),
                Err(HousingError::NotOwned)
            );
        });
        f.run(|| super::purchase_furniture(&f.env, f.player1.clone(), lamp).unwrap());
        f.run(|| super::purchase_furniture(&f.env, f.player1.clone(), lamp).unwrap());
        assert_eq!(
            f.run(|| super::get_inventory(&f.env, &f.player1).get(lamp)),
            Some(2)
        );
        f.run(|| super::place_furniture(&f.env, f.player1.clone(), lamp, 1, 1, 0, 0).unwrap());
        f.run(|| {
            assert_eq!(
                super::place_furniture(&f.env, f.player1.clone(), lamp, 1, 1, 0, 0),
                Err(HousingError::PositionOccupied)
            );
        });
        f.run(|| super::place_furniture(&f.env, f.player1.clone(), lamp, 1, 1, 0, 1).unwrap());
        assert_eq!(
            f.run(|| super::get_inventory(&f.env, &f.player1).get(lamp)),
            None
        );
        f.run(|| super::remove_furniture(&f.env, f.player1.clone(), lamp).unwrap());
        assert_eq!(
            f.run(|| super::get_inventory(&f.env, &f.player1).get(lamp)),
            Some(1)
        );
    }

    #[test]
    fn test_storage_capacity_and_expansion() {
        let f = fixture();
        house_for(&f, &f.player1);
        let crate_item = furniture(&f, FurnitureCategory::Decoration, 1);
        let chest = furniture(&f, FurnitureCategory::Storage, 1);
        fund(&f, &f.player1, 100_000);
        let start = f.run(|| super::get_storage(&f.env, &f.player1).unwrap());
        assert_eq!((start.capacity, start.used, start.expansions), (10, 0, 0));
        assert_eq!(start.next_expansion_cost, STORAGE_EXPANSION_BASE_COST);
        for _ in 0..10 {
            f.run(|| super::purchase_furniture(&f.env, f.player1.clone(), crate_item).unwrap());
        }
        f.run(|| {
            assert_eq!(
                super::purchase_furniture(&f.env, f.player1.clone(), crate_item),
                Err(HousingError::StorageFull)
            );
        });
        let dust_before =
            f.run(|| resource_minter::balance_of(&f.env, &f.player1, &ResourceType::StellarDust));
        let expanded = f.run(|| super::expand_storage(&f.env, f.player1.clone()).unwrap());
        assert_eq!(expanded.capacity, 20);
        assert_eq!(
            expanded.next_expansion_cost,
            2 * STORAGE_EXPANSION_BASE_COST
        );
        let dust_after =
            f.run(|| resource_minter::balance_of(&f.env, &f.player1, &ResourceType::StellarDust));
        assert_eq!(dust_before - dust_after, STORAGE_EXPANSION_BASE_COST);

        f.run(|| super::purchase_furniture(&f.env, f.player1.clone(), chest).unwrap());
        f.run(|| super::place_furniture(&f.env, f.player1.clone(), chest, 5, 5, 0, 0).unwrap());
        let with_chest = f.run(|| super::get_storage(&f.env, &f.player1).unwrap());
        assert_eq!(with_chest.capacity, 20 + STORAGE_PER_STORAGE_ITEM);
        assert_eq!(with_chest.used, 10);

        for _ in 1..MAX_STORAGE_EXPANSIONS {
            f.run(|| super::expand_storage(&f.env, f.player1.clone()).unwrap());
        }
        f.run(|| {
            assert_eq!(
                super::expand_storage(&f.env, f.player1.clone()),
                Err(HousingError::StorageMaxed)
            );
        });
    }

    #[test]
    fn test_achievement_display_cases() {
        let f = fixture();
        house_for(&f, &f.player1);
        let case = furniture(&f, FurnitureCategory::Achievement, 1);
        fund(&f, &f.player1, 1_000);
        f.run(|| {
            assert_eq!(
                super::display_achievement(&f.env, f.player1.clone(), 1, 0),
                Err(HousingError::NoDisplayCase)
            );
        });
        f.run(|| super::purchase_furniture(&f.env, f.player1.clone(), case).unwrap());
        f.run(|| super::place_furniture(&f.env, f.player1.clone(), case, 9, 9, 0, 0).unwrap());
        f.run(|| {
            assert_eq!(
                super::display_achievement(&f.env, f.player1.clone(), 1, 0),
                Err(HousingError::AchievementNotEarned)
            );
        });

        // Unlock achievements 1 and 2 the way the achievement engine records them.
        f.run(|| {
            for id in [1u64, 2] {
                f.env.storage().persistent().set(
                    &crate::achievement_engine::AchievementKey::PlayerAchievement(
                        f.player1.clone(),
                        id,
                    ),
                    &100u64,
                );
            }
        });
        f.run(|| super::display_achievement(&f.env, f.player1.clone(), 1, 0).unwrap());
        let shown = f.run(|| super::display_achievement(&f.env, f.player1.clone(), 2, 0).unwrap());
        assert_eq!(shown.len(), 1, "a slot holds one achievement");
        assert_eq!(shown.get(0).unwrap().achievement_id, 2);
        f.run(|| {
            assert_eq!(
                super::display_achievement(&f.env, f.player1.clone(), 1, SLOTS_PER_DISPLAY_CASE),
                Err(HousingError::NoDisplayCase)
            );
        });
        f.run(|| super::display_achievement(&f.env, f.player1.clone(), 1, 2).unwrap());
        assert_eq!(
            f.run(|| super::get_achievement_displays(&f.env, &f.player1))
                .len(),
            2
        );

        f.run(|| super::remove_furniture(&f.env, f.player1.clone(), case).unwrap());
        assert_eq!(
            f.run(|| super::get_achievement_displays(&f.env, &f.player1))
                .len(),
            0
        );
    }

    #[test]
    fn test_guild_banner_follows_membership() {
        let f = fixture();
        house_for(&f, &f.player1);
        f.run(|| {
            assert_eq!(
                super::display_guild_banner(&f.env, f.player1.clone()),
                Err(HousingError::NotInGuild)
            );
        });
        let guild = f.run(|| {
            alliance_manager::found_alliance(
                &f.env,
                f.player1.clone(),
                String::from_str(&f.env, "Nomads"),
            )
            .unwrap()
        });
        let banner = f.run(|| super::display_guild_banner(&f.env, f.player1.clone()).unwrap());
        assert_eq!(banner.alliance_id, guild);
        assert_eq!(banner.alliance_name, String::from_str(&f.env, "Nomads"));
        assert_eq!(
            f.run(|| super::get_guild_banner(&f.env, &f.player1)),
            Some(banner)
        );
        f.run(|| alliance_manager::leave_alliance(&f.env, f.player1.clone()).unwrap());
        assert_eq!(f.run(|| super::get_guild_banner(&f.env, &f.player1)), None);
    }

    #[test]
    fn test_friends_only_admits_guests_and_guildmates() {
        let f = fixture();
        house_for(&f, &f.player1);
        f.run(|| {
            super::set_house_access(&f.env, f.player1.clone(), HouseAccess::FriendsOnly).unwrap()
        });
        let stranger = Address::generate(&f.env);
        f.run(|| {
            assert_eq!(
                super::visit_house(&f.env, stranger.clone(), f.player1.clone()),
                Err(HousingError::AccessDenied)
            );
        });
        f.run(|| super::add_guest(&f.env, f.player1.clone(), stranger.clone()).unwrap());
        f.run(|| super::visit_house(&f.env, stranger.clone(), f.player1.clone()).unwrap());
        f.run(|| super::remove_guest(&f.env, f.player1.clone(), stranger.clone()).unwrap());
        f.run(|| {
            assert_eq!(
                super::visit_house(&f.env, stranger.clone(), f.player1.clone()),
                Err(HousingError::AccessDenied)
            );
        });

        let guild = f.run(|| {
            alliance_manager::found_alliance(
                &f.env,
                f.player1.clone(),
                String::from_str(&f.env, "Kin"),
            )
            .unwrap()
        });
        f.run(|| alliance_manager::join_alliance(&f.env, guild, f.player2.clone()).unwrap());
        f.run(|| super::visit_house(&f.env, f.player2.clone(), f.player1.clone()).unwrap());
    }

    #[test]
    fn test_visitor_log_keeps_latest_and_rating_needs_a_visit() {
        let f = fixture();
        house_for(&f, &f.player1);
        f.run(|| {
            assert_eq!(
                super::rate_house(&f.env, f.player2.clone(), f.player1.clone(), 400),
                Err(HousingError::MustVisitFirst)
            );
        });
        let first = Address::generate(&f.env);
        f.run(|| super::visit_house(&f.env, first.clone(), f.player1.clone()).unwrap());
        for _ in 0..MAX_VISITOR_RECORDS {
            let v = Address::generate(&f.env);
            f.run(|| super::visit_house(&f.env, v.clone(), f.player1.clone()).unwrap());
        }
        let log = f.run(|| super::get_visitors(&f.env, &f.player1));
        assert_eq!(log.len(), MAX_VISITOR_RECORDS);
        assert!(
            log.iter().all(|r| r.visitor != first),
            "oldest visit dropped"
        );
        assert_eq!(
            f.run(|| super::get_house(&f.env, &f.player1).unwrap())
                .total_visits,
            51
        );

        f.run(|| super::visit_house(&f.env, f.player2.clone(), f.player1.clone()).unwrap());
        f.run(|| super::rate_house(&f.env, f.player2.clone(), f.player1.clone(), 500).unwrap());
        let log = f.run(|| super::get_visitors(&f.env, &f.player1));
        let last = log.get(log.len() - 1).unwrap();
        assert_eq!(last.visitor, f.player2);
        assert!(last.left_rating);
    }

    #[test]
    fn test_showcase_collects_everything_a_visitor_sees() {
        let f = fixture();
        assert_eq!(
            f.run(|| super::get_house_showcase(&f.env, &f.player1)),
            None
        );
        house_for(&f, &f.player1);
        let lamp = furniture(&f, FurnitureCategory::Lighting, 1);
        fund(&f, &f.player1, 100);
        f.run(|| super::purchase_furniture(&f.env, f.player1.clone(), lamp).unwrap());
        f.run(|| super::purchase_furniture(&f.env, f.player1.clone(), lamp).unwrap());
        f.run(|| super::place_furniture(&f.env, f.player1.clone(), lamp, 3, 4, 0, 0).unwrap());
        f.run(|| {
            alliance_manager::found_alliance(
                &f.env,
                f.player1.clone(),
                String::from_str(&f.env, "Nomads"),
            )
            .unwrap()
        });
        f.run(|| super::display_guild_banner(&f.env, f.player1.clone()).unwrap());

        let showcase = f.run(|| super::get_house_showcase(&f.env, &f.player1).unwrap());
        assert_eq!(showcase.house.owner, f.player1);
        assert_eq!(showcase.placements.len(), 1);
        assert_eq!(showcase.storage.used, 1);
        assert_eq!(showcase.banner.len(), 1);
        assert_eq!(
            showcase.banner.get(0).unwrap().alliance_name,
            String::from_str(&f.env, "Nomads")
        );
    }
}
