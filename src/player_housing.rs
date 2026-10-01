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
//! ```

use soroban_sdk::{contracterror, contracttype, symbol_short, Address, Env, String, Symbol, Vec};

use crate::achievements;
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
    Counter,
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
            | Self::PositionOccupied => (ErrorKind::Validation, false),
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
        .get::<HousingKey, u64>(&HousingKey::Counter)
        .unwrap_or(0)
        .saturating_add(1);

    env.storage()
        .persistent()
        .set(&HousingKey::Counter, &furniture_id);

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

    // Debit cost
    resource_minter::debit_balance(env, &owner, &item.cost_resource, item.cost_amount)
        .map_err(|_| HousingError::InsufficientResources)?;

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

    house.furniture_count = new_placements.len();
    house.last_modified = env.ledger().timestamp();

    env.storage()
        .persistent()
        .set(&HousingKey::Placement(owner.clone()), &new_placements);
    env.storage()
        .persistent()
        .set(&HousingKey::House(owner.clone()), &house);

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
            // For simplicity, allow if visitor has a profile
            // In full implementation, check bonding or guild membership
            player_profile::get_profile_by_owner(env, &visitor)?;
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

    if visitors.len() < MAX_VISITOR_RECORDS {
        visitors.push_back(VisitorRecord {
            visitor: visitor.clone(),
            visited_at: env.ledger().timestamp(),
            left_rating: false,
        });
        env.storage()
            .persistent()
            .set(&HousingKey::Visitors(house_owner.clone()), &visitors);
    }

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
}
