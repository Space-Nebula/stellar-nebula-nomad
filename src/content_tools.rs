use soroban_sdk::{contracterror, contracttype, symbol_short, Address, Env, String, Vec, Symbol, Map, Bytes};

// ── Error ─────────────────────────────────────────────────────────────────────

#[contracterror]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum ContentToolsError {
    /// Content already exists.
    ContentAlreadyExists = 1,
    /// Content not found.
    ContentNotFound = 2,
    /// Unauthorized action.
    Unauthorized = 3,
    /// Invalid content data.
    InvalidContent = 4,
    /// Content limit reached.
    ContentLimitReached = 5,
    /// Invalid rating.
    InvalidRating = 6,
    /// Already voted.
    AlreadyVoted = 7,
    /// Content is under review.
    UnderReview = 8,
    /// Content has been rejected.
    ContentRejected = 9,
}

// ── Storage Keys ──────────────────────────────────────────────────────────────

#[contracttype]
#[derive(Clone)]
pub enum ContentDataKey {
    /// Content by ID.
    Content(u64),
    /// Content counter.
    ContentCounter,
    /// Content by creator.
    CreatorContent(Address),
    /// Content vote by (content_id, voter).
    Vote(u64, Address),
    /// Content votes count.
    VoteCount(u64),
    /// Content rating sum.
    RatingSum(u64),
    /// Content rating count.
    RatingCount(u64),
    /// Admin address.
    Admin,
    /// Content status.
    Status(u64),
    /// Marketplace listing.
    MarketplaceListing(u64),
    /// Marketplace counter.
    MarketplaceCounter,
}

// ── Data Types ────────────────────────────────────────────────────────────────

#[contracttype]
#[derive(Clone, Debug)]
pub struct ContentMetadata {
    pub name: String,
    pub description: String,
    pub content_type: Symbol,
    pub created_at: u64,
    pub updated_at: u64,
    pub tags: Vec<Symbol>,
}

#[contracttype]
#[derive(Clone, Debug)]
pub struct CreatedContent {
    pub content_id: u64,
    pub creator: Address,
    pub metadata: ContentMetadata,
    pub data: Bytes,
    pub is_public: bool,
    pub is_verified: bool,
    pub play_count: u64,
    pub rating: u32,
}

#[contracttype]
#[derive(Clone, Debug)]
pub struct MarketplaceListing {
    pub listing_id: u64,
    pub content_id: u64,
    pub seller: Address,
    pub price: i128,
    pub is_active: bool,
    pub created_at: u64,
}

#[contracttype]
#[derive(Clone, Debug)]
pub struct VoteResult {
    pub content_id: u64,
    pub total_votes: u64,
    pub rating_sum: u64,
    pub rating_count: u32,
    pub average_rating: u32,
}

// ── Constants ────────────────────────────────────────────────────────────────

pub const MAX_CONTENT_PER_CREATOR: u32 = 20;
pub const MAX_MARKETPLACE_LISTINGS: u32 = 100;
pub const CONTENT_TYPE_NEBULA: &str = "nebula";
pub const CONTENT_TYPE_MISSION: &str = "mission";
pub const CONTENT_TYPE_EVENT: &str = "event";

// ── Admin Functions ──────────────────────────────────────────────────────────

pub fn set_admin(env: &Env, admin: &Address) {
    admin.require_auth();
    env.storage()
        .persistent()
        .set(&ContentDataKey::Admin, admin);
}

fn get_admin(env: &Env) -> Option<Address> {
    env.storage()
        .persistent()
        .get(&ContentDataKey::Admin)
}

fn require_admin(env: &Env, caller: &Address) -> Result<(), ContentToolsError> {
    caller.require_auth();
    let admin = get_admin(env).ok_or(ContentToolsError::Unauthorized)?;
    if *caller != admin {
        return Err(ContentToolsError::Unauthorized);
    }
    Ok(())
}

// ── Content Creation ─────────────────────────────────────────────────────────

pub fn create_content(
    env: &Env,
    creator: &Address,
    name: String,
    description: String,
    content_type: Symbol,
    data: Bytes,
    is_public: bool,
    tags: Vec<Symbol>,
) -> Result<u64, ContentToolsError> {
    creator.require_auth();

    // Check creator content limit
    let creator_key = ContentDataKey::CreatorContent(creator.clone());
    let mut creator_contents: Vec<u64> = env
        .storage()
        .persistent()
        .get(&creator_key)
        .unwrap_or_else(|| Vec::new(env));

    if creator_contents.len() >= MAX_CONTENT_PER_CREATOR {
        return Err(ContentToolsError::ContentLimitReached);
    }

    // Generate content ID
    let content_counter: u64 = env
        .storage()
        .persistent()
        .get(&ContentDataKey::ContentCounter)
        .unwrap_or(0);
    let content_id = content_counter + 1;
    env.storage()
        .persistent()
        .set(&ContentDataKey::ContentCounter, &content_id);

    let now = env.ledger().timestamp();

    let metadata = ContentMetadata {
        name,
        description,
        content_type: content_type.clone(),
        created_at: now,
        updated_at: now,
        tags,
    };

    let content = CreatedContent {
        content_id,
        creator: creator.clone(),
        metadata,
        data,
        is_public,
        is_verified: false,
        play_count: 0,
        rating: 0,
    };

    env.storage()
        .persistent()
        .set(&ContentDataKey::Content(content_id), &content);

    creator_contents.push_back(content_id);
    env.storage().persistent().set(&creator_key, &creator_contents);

    // Set initial status as pending review
    env.storage()
        .persistent()
        .set(&ContentDataKey::Status(content_id), &symbol_short!("pending"));

    env.events().publish(
        (symbol_short!("ct"), symbol_short!("created")),
        (creator.clone(), content_id, content_type),
    );

    Ok(content_id)
}

pub fn update_content(
    env: &Env,
    creator: &Address,
    content_id: u64,
    name: Option<String>,
    description: Option<String>,
    data: Option<Bytes>,
    is_public: Option<bool>,
    tags: Option<Vec<Symbol>>,
) -> Result<(), ContentToolsError> {
    creator.require_auth();

    let key = ContentDataKey::Content(content_id);
    let mut content: CreatedContent = env
        .storage()
        .persistent()
        .get(&key)
        .ok_or(ContentToolsError::ContentNotFound)?;

    if content.creator != *creator {
        return Err(ContentToolsError::Unauthorized);
    }

    if let Some(n) = name {
        content.metadata.name = n;
    }
    if let Some(d) = description {
        content.metadata.description = d;
    }
    if let Some(dt) = data {
        content.data = dt;
    }
    if let Some(ip) = is_public {
        content.is_public = ip;
    }
    if let Some(t) = tags {
        content.metadata.tags = t;
    }

    content.metadata.updated_at = env.ledger().timestamp();

    env.storage().persistent().set(&key, &content);

    env.events().publish(
        (symbol_short!("ct"), symbol_short!("updated")),
        (creator.clone(), content_id),
    );

    Ok(())
}

pub fn get_content(env: &Env, content_id: u64) -> Result<CreatedContent, ContentToolsError> {
    let key = ContentDataKey::Content(content_id);
    env.storage()
        .persistent()
        .get(&key)
        .ok_or(ContentToolsError::ContentNotFound)
}

pub fn get_creator_content(env: &Env, creator: &Address) -> Vec<u64> {
    let key = ContentDataKey::CreatorContent(creator.clone());
    env.storage()
        .persistent()
        .get(&key)
        .unwrap_or_else(|| Vec::new(env))
}

pub fn delete_content(
    env: &Env,
    creator: &Address,
    content_id: u64,
) -> Result<(), ContentToolsError> {
    creator.require_auth();

    let key = ContentDataKey::Content(content_id);
    let content: CreatedContent = env
        .storage()
        .persistent()
        .get(&key)
        .ok_or(ContentToolsError::ContentNotFound)?;

    if content.creator != *creator {
        return Err(ContentToolsError::Unauthorized);
    }

    env.storage().persistent().remove(&key);

    // Remove from creator's list
    let creator_key = ContentDataKey::CreatorContent(creator.clone());
    let mut creator_contents: Vec<u64> = env
        .storage()
        .persistent()
        .get(&creator_key)
        .unwrap_or_else(|| Vec::new(env));

    let mut new_list = Vec::new(env);
    for i in 0..creator_contents.len() {
        if let Some(id) = creator_contents.get(i) {
            if id != content_id {
                new_list.push_back(id);
            }
        }
    }
    env.storage().persistent().set(&creator_key, &new_list);

    env.events().publish(
        (symbol_short!("ct"), symbol_short!("deleted")),
        (creator.clone(), content_id),
    );

    Ok(())
}

// ── Content Validation (Admin) ─────────────────────────────────────────────

pub fn approve_content(
    env: &Env,
    caller: &Address,
    content_id: u64,
) -> Result<(), ContentToolsError> {
    require_admin(env, caller)?;

    let status_key = ContentDataKey::Status(content_id);
    env.storage()
        .persistent()
        .set(&status_key, &symbol_short!("approved"));

    let key = ContentDataKey::Content(content_id);
    let mut content: CreatedContent = env
        .storage()
        .persistent()
        .get(&key)
        .ok_or(ContentToolsError::ContentNotFound)?;

    content.is_verified = true;
    env.storage().persistent().set(&key, &content);

    env.events().publish(
        (symbol_short!("ct"), symbol_short!("approved")),
        (content_id, *caller),
    );

    Ok(())
}

pub fn reject_content(
    env: &Env,
    caller: &Address,
    content_id: u64,
) -> Result<(), ContentToolsError> {
    require_admin(env, caller)?;

    let status_key = ContentDataKey::Status(content_id);
    env.storage()
        .persistent()
        .set(&status_key, &symbol_short!("rejected"));

    env.events().publish(
        (symbol_short!("ct"), symbol_short!("rejected")),
        (content_id, *caller),
    );

    Ok(())
}

pub fn get_content_status(env: &Env, content_id: u64) -> Symbol {
    env.storage()
        .persistent()
        .get(&ContentDataKey::Status(content_id))
        .unwrap_or(symbol_short!("unknown"))
}

// ── Voting & Rating ────────────────────────────────────────────────────────

pub fn vote_content(
    env: &Env,
    voter: &Address,
    content_id: u64,
    rating: u32,
) -> Result<(), ContentToolsError> {
    voter.require_auth();

    if rating < 1 || rating > 5 {
        return Err(ContentToolsError::InvalidRating);
    }

    let status = get_content_status(env, content_id);
    if status == symbol_short!("rejected") {
        return Err(ContentToolsError::ContentRejected);
    }

    let vote_key = ContentDataKey::Vote(content_id, voter.clone());
    if env.storage().persistent().has(&vote_key) {
        return Err(ContentToolsError::AlreadyVoted);
    }

    // Record vote
    env.storage().persistent().set(&vote_key, &rating);

    // Update vote count
    let vote_count_key = ContentDataKey::VoteCount(content_id);
    let vote_count: u64 = env
        .storage()
        .persistent()
        .get(&vote_count_key)
        .unwrap_or(0);
    env.storage()
        .persistent()
        .set(&vote_count_key, &(vote_count + 1));

    // Update rating sum
    let rating_sum_key = ContentDataKey::RatingSum(content_id);
    let rating_sum: u64 = env
        .storage()
        .persistent()
        .get(&rating_sum_key)
        .unwrap_or(0);
    env.storage()
        .persistent()
        .set(&rating_sum_key, &(rating_sum + rating as u64));

    // Update rating count
    let rating_count_key = ContentDataKey::RatingCount(content_id);
    let rating_count: u32 = env
        .storage()
        .persistent()
        .get(&rating_count_key)
        .unwrap_or(0);
    env.storage()
        .persistent()
        .set(&rating_count_key, &(rating_count + 1));

    // Update content rating
    let key = ContentDataKey::Content(content_id);
    let mut content: CreatedContent = env
        .storage()
        .persistent()
        .get(&key)
        .ok_or(ContentToolsError::ContentNotFound)?;

    let new_count = rating_count + 1;
    let new_sum = rating_sum + rating as u64;
    content.rating = (new_sum as u32) / new_count;
    env.storage().persistent().set(&key, &content);

    env.events().publish(
        (symbol_short!("ct"), symbol_short!("voted")),
        (voter.clone(), content_id, rating),
    );

    Ok(())
}

pub fn get_vote_result(env: &Env, content_id: u64) -> VoteResult {
    let vote_count: u64 = env
        .storage()
        .persistent()
        .get(&ContentDataKey::VoteCount(content_id))
        .unwrap_or(0);

    let rating_sum: u64 = env
        .storage()
        .persistent()
        .get(&ContentDataKey::RatingSum(content_id))
        .unwrap_or(0);

    let rating_count: u32 = env
        .storage()
        .persistent()
        .get(&ContentDataKey::RatingCount(content_id))
        .unwrap_or(0);

    let avg_rating = if rating_count > 0 {
        (rating_sum as u32) / rating_count
    } else {
        0
    };

    VoteResult {
        content_id,
        total_votes: vote_count,
        rating_sum,
        rating_count,
        average_rating: avg_rating,
    }
}

// ── Play Count ─────────────────────────────────────────────────────────────

pub fn increment_play_count(
    env: &Env,
    content_id: u64,
) -> Result<(), ContentToolsError> {
    let key = ContentDataKey::Content(content_id);
    let mut content: CreatedContent = env
        .storage()
        .persistent()
        .get(&key)
        .ok_or(ContentToolsError::ContentNotFound)?;

    content.play_count += 1;
    env.storage().persistent().set(&key, &content);

    Ok(())
}

// ── Marketplace ─────────────────────────────────────────────────────────────

pub fn list_on_marketplace(
    env: &Env,
    seller: &Address,
    content_id: u64,
    price: i128,
) -> Result<u64, ContentToolsError> {
    seller.require_auth();

    // Verify content exists and seller is creator
    let key = ContentDataKey::Content(content_id);
    let content: CreatedContent = env
        .storage()
        .persistent()
        .get(&key)
        .ok_or(ContentToolsError::ContentNotFound)?;

    if content.creator != *seller {
        return Err(ContentToolsError::Unauthorized);
    }

    let status = get_content_status(env, content_id);
    if status != symbol_short!("approved") {
        return Err(ContentToolsError::UnderReview);
    }

    // Generate listing ID
    let listing_counter: u64 = env
        .storage()
        .persistent()
        .get(&ContentDataKey::MarketplaceCounter)
        .unwrap_or(0);
    let listing_id = listing_counter + 1;
    env.storage()
        .persistent()
        .set(&ContentDataKey::MarketplaceCounter, &listing_id);

    let listing = MarketplaceListing {
        listing_id,
        content_id,
        seller: seller.clone(),
        price,
        is_active: true,
        created_at: env.ledger().timestamp(),
    };

    env.storage()
        .persistent()
        .set(&ContentDataKey::MarketplaceListing(listing_id), &listing);

    env.events().publish(
        (symbol_short!("mp"), symbol_short!("listed")),
        (seller.clone(), content_id, price, listing_id),
    );

    Ok(listing_id)
}

pub fn unlist_from_marketplace(
    env: &Env,
    seller: &Address,
    listing_id: u64,
) -> Result<(), ContentToolsError> {
    seller.require_auth();

    let key = ContentDataKey::MarketplaceListing(listing_id);
    let mut listing: MarketplaceListing = env
        .storage()
        .persistent()
        .get(&key)
        .ok_or(ContentToolsError::ContentNotFound)?;

    if listing.seller != *seller {
        return Err(ContentToolsError::Unauthorized);
    }

    listing.is_active = false;
    env.storage().persistent().set(&key, &listing);

    Ok(())
}

pub fn get_marketplace_listing(
    env: &Env,
    listing_id: u64,
) -> Result<MarketplaceListing, ContentToolsError> {
    let key = ContentDataKey::MarketplaceListing(listing_id);
    env.storage()
        .persistent()
        .get(&key)
        .ok_or(ContentToolsError::ContentNotFound)
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use soroban_sdk::{testutils::Address as _, Env, Symbol};

    use soroban_sdk::{contract, contractimpl};
    #[contract]
    struct Stub;
    #[contractimpl]
    impl Stub {}

    fn make_env() -> (Env, soroban_sdk::Address) {
        let env = Env::default();
        let id = env.register_contract(None, Stub);
        (env, id)
    }

    #[test]
    fn test_create_content() {
        let (env, _contract_id) = make_env();
        let creator = Address::generate(&env);

        env.as_contract(&_contract_id, || {
            let tags = Vec::new(&env);
            let content_id = create_content(
                &env,
                &creator,
                String::from_str(&env, "Test Nebula"),
                String::from_str(&env, "A test nebula"),
                Symbol::new(&env, CONTENT_TYPE_NEBULA),
                Bytes::new(&env),
                true,
                tags,
            )
            .unwrap();
            assert!(content_id > 0);

            let content = get_content(&env, content_id).unwrap();
            assert_eq!(content.creator, creator);
        });
    }

    #[test]
    fn test_vote_content() {
        let (env, _contract_id) = make_env();
        let creator = Address::generate(&env);
        let voter = Address::generate(&env);

        env.as_contract(&_contract_id, || {
            let tags = Vec::new(&env);
            let content_id = create_content(
                &env,
                &creator,
                String::from_str(&env, "Test"),
                String::from_str(&env, "Desc"),
                Symbol::new(&env, CONTENT_TYPE_NEBULA),
                Bytes::new(&env),
                true,
                tags,
            )
            .unwrap();

            // Need admin to approve first
            set_admin(&env, &creator);
            approve_content(&env, &creator, content_id).unwrap();

            vote_content(&env, &voter, content_id, 5).unwrap();
            let result = get_vote_result(&env, content_id);
            assert_eq!(result.total_votes, 1);
        });
    }
}
