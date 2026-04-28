use soroban_sdk::{contracterror, contracttype, symbol_short, Address, Env, String, Vec, Symbol, Map};

// ── Error ─────────────────────────────────────────────────────────────────────

#[contracterror]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum LeaderboardError {
    /// Category does not exist.
    InvalidCategory = 1,
    /// Time period is not recognized.
    InvalidTimePeriod = 2,
    /// Region is not valid.
    InvalidRegion = 3,
    /// Player not found in leaderboard.
    PlayerNotFound = 4,
    /// Unauthorized admin action.
    Unauthorized = 5,
    /// Max leaderboard entries exceeded.
    LeaderboardFull = 6,
}

// ── Storage Keys ──────────────────────────────────────────────────────────────

#[contracttype]
#[derive(Clone)]
pub enum LeaderboardDataKey {
    /// Leaderboard entries by (category, time_period).
    Board(Symbol, Symbol),
    /// Player's guild affiliation.
    PlayerGuild(Address),
    /// Guild leaderboard entries.
    GuildBoard(Symbol),
    /// Regional leaderboard entries.
    RegionalBoard(Symbol, Symbol),
    /// Achievement leaderboard.
    AchievementBoard,
    /// Admin address.
    Admin,
}

// ── Data Types ────────────────────────────────────────────────────────────────

#[contracttype]
#[derive(Clone, Debug)]
pub struct LeaderboardEntry {
    pub player: Address,
    pub score: u64,
    pub timestamp: u64,
    pub metadata: Map<Symbol, String>,
}

#[contracttype]
#[derive(Clone, Debug)]
pub struct GuildEntry {
    pub guild_name: String,
    pub score: u64,
    pub member_count: u32,
}

#[contracttype]
#[derive(Clone, Debug)]
pub struct RegionalEntry {
    pub player: Address,
    pub region: Symbol,
    pub score: u64,
}

#[contracttype]
#[derive(Clone, Debug)]
pub struct AchievementEntry {
    pub player: Address,
    pub achievement_count: u32,
    pub total_points: u64,
}

#[contracttype]
#[derive(Clone, Debug)]
pub struct LeaderboardRewards {
    pub top_1_reward: u64,
    pub top_2_reward: u64,
    pub top_3_reward: u64,
    pub top_10_reward: u64,
}

// ── Constants ────────────────────────────────────────────────────────────────

pub const MAX_LEADERBOARD_ENTRIES: u32 = 100;
pub const MAX_GUILD_BOARD_ENTRIES: u32 = 50;

// ── Categories (10+) ─────────────────────────────────────────────────────────

pub const CATEGORY_ESSENCE: &str = "essence";
pub const CATEGORY_SCANS: &str = "scans";
pub const CATEGORY_MISSIONS: &str = "missions";
pub const CATEGORY_NEBULAE_EXPLORED: &str = "nebulae";
pub const CATEGORY_SHIPS_MINTED: &str = "ships";
pub const CATEGORY_TRADES: &str = "trades";
pub const CATEGORY_CRAFTS: &str = "crafts";
pub const CATEGORY_BOUNTIES: &str = "bounties";
pub const CATEGORY_PVP_WINS: &str = "pvp_wins";
pub const CATEGORY_PVP_RATING: &str = "pvp_rating";
pub const CATEGORY_GUILD_CONTRIBUTION: &str = "guild_contrib";
pub const CATEGORY_ACHIEVEMENTS: &str = "achievements";

// ── Time Periods ─────────────────────────────────────────────────────────────

pub const PERIOD_DAILY: &str = "daily";
pub const PERIOD_WEEKLY: &str = "weekly";
pub const PERIOD_MONTHLY: &str = "monthly";
pub const PERIOD_ALL_TIME: &str = "all_time";

// ── Regions ──────────────────────────────────────────────────────────────────

pub const REGION_NORTH_AMERICA: &str = "namerica";
pub const REGION_EUROPE: &str = "europe";
pub const REGION_ASIA: &str = "asia";
pub const REGION_SOUTH_AMERICA: &str = "samerica";
pub const REGION_AFRICA: &str = "africa";
pub const REGION_OCEANIA: &str = "oceania";

// ── Admin Functions ──────────────────────────────────────────────────────────

pub fn set_admin(env: &Env, admin: &Address) {
    admin.require_auth();
    env.storage()
        .persistent()
        .set(&LeaderboardDataKey::Admin, admin);
}

fn get_admin(env: &Env) -> Option<Address> {
    env.storage()
        .persistent()
        .get(&LeaderboardDataKey::Admin)
}

fn require_admin(env: &Env, caller: &Address) -> Result<(), LeaderboardError> {
    caller.require_auth();
    let admin = get_admin(env).ok_or(LeaderboardError::Unauthorized)?;
    if *caller != admin {
        return Err(LeaderboardError::Unauthorized);
    }
    Ok(())
}

// ── Leaderboard Management ───────────────────────────────────────────────────

pub fn update_score(
    env: &Env,
    player: &Address,
    category: Symbol,
    time_period: Symbol,
    score: u64,
) -> Result<(), LeaderboardError> {
    player.require_auth();

    validate_category(&category)?;
    validate_time_period(&time_period)?;

    let key = LeaderboardDataKey::Board(category.clone(), time_period.clone());
    let mut entries: Vec<LeaderboardEntry> = env
        .storage()
        .persistent()
        .get(&key)
        .unwrap_or_else(|| Vec::new(env));

    // Update or insert player score
    let mut found = false;
    for i in 0..entries.len() {
        if let Some(mut entry) = entries.get(i) {
            if entry.player == *player {
                entry.score = entry.score.max(score);
                entry.timestamp = env.ledger().timestamp();
                entries.set(i, entry);
                found = true;
                break;
            }
        }
    }

    if !found {
        if entries.len() >= MAX_LEADERBOARD_ENTRIES {
            return Err(LeaderboardError::LeaderboardFull);
        }
        entries.push_back(LeaderboardEntry {
            player: player.clone(),
            score,
            timestamp: env.ledger().timestamp(),
            metadata: Map::new(env),
        });
    }

    // Sort descending by score
    sort_entries_descending(env, &mut entries);

    env.storage().persistent().set(&key, &entries);

    env.events().publish(
        (symbol_short!("lb"), symbol_short!("update")),
        (player.clone(), category, time_period, score),
    );

    Ok(())
}

pub fn get_leaderboard(
    env: &Env,
    category: Symbol,
    time_period: Symbol,
    limit: u32,
) -> Result<Vec<LeaderboardEntry>, LeaderboardError> {
    validate_category(&category)?;
    validate_time_period(&time_period)?;

    let key = LeaderboardDataKey::Board(category, time_period);
    let entries: Vec<LeaderboardEntry> = env
        .storage()
        .persistent()
        .get(&key)
        .unwrap_or_else(|| Vec::new(env));

    let limit = limit.min(entries.len());
    let mut result = Vec::new(env);
    for i in 0..limit {
        if let Some(entry) = entries.get(i) {
            result.push_back(entry);
        }
    }

    Ok(result)
}

// ── Guild Leaderboards ──────────────────────────────────────────────────────

pub fn update_guild_score(
    env: &Env,
    caller: &Address,
    guild_name: String,
    score: u64,
    member_count: u32,
) -> Result<(), LeaderboardError> {
    caller.require_auth();
    require_admin(env, caller)?;

    let key = LeaderboardDataKey::GuildBoard(symbol_short!("guild"));
    let mut entries: Vec<GuildEntry> = env
        .storage()
        .persistent()
        .get(&key)
        .unwrap_or_else(|| Vec::new(env));

    let mut found = false;
    for i in 0..entries.len() {
        if let Some(mut entry) = entries.get(i) {
            if entry.guild_name == guild_name {
                entry.score = entry.score.max(score);
                entry.member_count = member_count;
                entries.set(i, entry);
                found = true;
                break;
            }
        }
    }

    if !found {
        if entries.len() >= MAX_GUILD_BOARD_ENTRIES {
            return Err(LeaderboardError::LeaderboardFull);
        }
        entries.push_back(GuildEntry {
            guild_name,
            score,
            member_count,
        });
    }

    sort_guild_entries_descending(env, &mut entries);
    env.storage().persistent().set(&key, &entries);

    Ok(())
}

pub fn get_guild_leaderboard(env: &Env, limit: u32) -> Vec<GuildEntry> {
    let key = LeaderboardDataKey::GuildBoard(symbol_short!("guild"));
    let entries: Vec<GuildEntry> = env
        .storage()
        .persistent()
        .get(&key)
        .unwrap_or_else(|| Vec::new(env));

    let limit = limit.min(entries.len());
    let mut result = Vec::new(env);
    for i in 0..limit {
        if let Some(entry) = entries.get(i) {
            result.push_back(entry);
        }
    }

    result
}

pub fn set_player_guild(
    env: &Env,
    player: &Address,
    guild_name: String,
) -> Result<(), LeaderboardError> {
    player.require_auth();

    let key = LeaderboardDataKey::PlayerGuild(player.clone());
    env.storage().persistent().set(&key, &guild_name);

    Ok(())
}

pub fn get_player_guild(env: &Env, player: &Address) -> Option<String> {
    env.storage()
        .persistent()
        .get(&LeaderboardDataKey::PlayerGuild(player.clone()))
}

// ── Regional Leaderboards ───────────────────────────────────────────────────

pub fn update_regional_score(
    env: &Env,
    player: &Address,
    region: Symbol,
    score: u64,
) -> Result<(), LeaderboardError> {
    player.require_auth();
    validate_region(&region)?;

    let key = LeaderboardDataKey::RegionalBoard(region.clone(), symbol_short!("board"));
    let mut entries: Vec<RegionalEntry> = env
        .storage()
        .persistent()
        .get(&key)
        .unwrap_or_else(|| Vec::new(env));

    let mut found = false;
    for i in 0..entries.len() {
        if let Some(mut entry) = entries.get(i) {
            if entry.player == *player {
                entry.score = entry.score.max(score);
                entries.set(i, entry);
                found = true;
                break;
            }
        }
    }

    if !found {
        if entries.len() >= MAX_LEADERBOARD_ENTRIES {
            return Err(LeaderboardError::LeaderboardFull);
        }
        entries.push_back(RegionalEntry {
            player: player.clone(),
            region: region.clone(),
            score,
        });
    }

    sort_regional_entries_descending(env, &mut entries);
    env.storage().persistent().set(&key, &entries);

    Ok(())
}

pub fn get_regional_leaderboard(
    env: &Env,
    region: Symbol,
    limit: u32,
) -> Result<Vec<RegionalEntry>, LeaderboardError> {
    validate_region(&region)?;

    let key = LeaderboardDataKey::RegionalBoard(region, symbol_short!("board"));
    let entries: Vec<RegionalEntry> = env
        .storage()
        .persistent()
        .get(&key)
        .unwrap_or_else(|| Vec::new(env));

    let limit = limit.min(entries.len());
    let mut result = Vec::new(env);
    for i in 0..limit {
        if let Some(entry) = entries.get(i) {
            result.push_back(entry);
        }
    }

    Ok(result)
}

// ── Achievement Leaderboards ────────────────────────────────────────────────

pub fn update_achievement_score(
    env: &Env,
    player: &Address,
    achievement_count: u32,
    points: u64,
) -> Result<(), LeaderboardError> {
    player.require_auth();

    let key = LeaderboardDataKey::AchievementBoard;
    let mut entries: Vec<AchievementEntry> = env
        .storage()
        .persistent()
        .get(&key)
        .unwrap_or_else(|| Vec::new(env));

    let mut found = false;
    for i in 0..entries.len() {
        if let Some(mut entry) = entries.get(i) {
            if entry.player == *player {
                entry.achievement_count = entry.achievement_count.max(achievement_count);
                entry.total_points = entry.total_points.max(points);
                entries.set(i, entry);
                found = true;
                break;
            }
        }
    }

    if !found {
        if entries.len() >= MAX_LEADERBOARD_ENTRIES {
            return Err(LeaderboardError::LeaderboardFull);
        }
        entries.push_back(AchievementEntry {
            player: player.clone(),
            achievement_count,
            total_points: points,
        });
    }

    sort_achievement_entries_descending(env, &mut entries);
    env.storage().persistent().set(&key, &entries);

    Ok(())
}

pub fn get_achievement_leaderboard(env: &Env, limit: u32) -> Vec<AchievementEntry> {
    let key = LeaderboardDataKey::AchievementBoard;
    let entries: Vec<AchievementEntry> = env
        .storage()
        .persistent()
        .get(&key)
        .unwrap_or_else(|| Vec::new(env));

    let limit = limit.min(entries.len());
    let mut result = Vec::new(env);
    for i in 0..limit {
        if let Some(entry) = entries.get(i) {
            result.push_back(entry);
        }
    }

    result
}

// ── Rewards ─────────────────────────────────────────────────────────────────

pub fn distribute_rewards(
    env: &Env,
    caller: &Address,
    category: Symbol,
    time_period: Symbol,
    rewards: LeaderboardRewards,
) -> Result<(), LeaderboardError> {
    require_admin(env, caller)?;
    validate_category(&category)?;
    validate_time_period(&time_period)?;

    let entries = get_leaderboard(env, category.clone(), time_period.clone(), 10)?;

    for i in 0..entries.len() {
        if let Some(entry) = entries.get(i) {
            let reward = match i {
                0 => rewards.top_1_reward,
                1 => rewards.top_2_reward,
                2 => rewards.top_3_reward,
                _ => rewards.top_10_reward,
            };

            if reward > 0 {
                env.events().publish(
                    (symbol_short!("lb"), symbol_short!("reward")),
                    (entry.player.clone(), reward, category.clone(), time_period.clone()),
                );
            }
        }
    }

    Ok(())
}

// ── Validation Helpers ──────────────────────────────────────────────────────

fn validate_category(category: &Symbol) -> Result<(), LeaderboardError> {
    let cat_str = category.to_string();
    match cat_str.as_str() {
        CATEGORY_ESSENCE
        | CATEGORY_SCANS
        | CATEGORY_MISSIONS
        | CATEGORY_NEBULAE_EXPLORED
        | CATEGORY_SHIPS_MINTED
        | CATEGORY_TRADES
        | CATEGORY_CRAFTS
        | CATEGORY_BOUNTIES
        | CATEGORY_PVP_WINS
        | CATEGORY_PVP_RATING
        | CATEGORY_GUILD_CONTRIBUTION
        | CATEGORY_ACHIEVEMENTS => Ok(()),
        _ => Err(LeaderboardError::InvalidCategory),
    }
}

fn validate_time_period(period: &Symbol) -> Result<(), LeaderboardError> {
    let period_str = period.to_string();
    match period_str.as_str() {
        PERIOD_DAILY | PERIOD_WEEKLY | PERIOD_MONTHLY | PERIOD_ALL_TIME => Ok(()),
        _ => Err(LeaderboardError::InvalidTimePeriod),
    }
}

fn validate_region(region: &Symbol) -> Result<(), LeaderboardError> {
    let region_str = region.to_string();
    match region_str.as_str() {
        REGION_NORTH_AMERICA
        | REGION_EUROPE
        | REGION_ASIA
        | REGION_SOUTH_AMERICA
        | REGION_AFRICA
        | REGION_OCEANIA => Ok(()),
        _ => Err(LeaderboardError::InvalidRegion),
    }
}

// ── Sorting Helpers ─────────────────────────────────────────────────────────

fn sort_entries_descending(env: &Env, entries: &mut Vec<LeaderboardEntry>) {
    let n = entries.len();
    for i in 0..n {
        for j in (i + 1)..n {
            let ei = entries.get(i);
            let ej = entries.get(j);
            if let (Some(ei_val), Some(ej_val)) = (ei, ej) {
                if ej_val.score > ei_val.score {
                    entries.set(i, ej_val);
                    entries.set(j, ei_val);
                }
            }
        }
    }
}

fn sort_guild_entries_descending(env: &Env, entries: &mut Vec<GuildEntry>) {
    let n = entries.len();
    for i in 0..n {
        for j in (i + 1)..n {
            let ei = entries.get(i);
            let ej = entries.get(j);
            if let (Some(ei_val), Some(ej_val)) = (ei, ej) {
                if ej_val.score > ei_val.score {
                    entries.set(i, ej_val);
                    entries.set(j, ei_val);
                }
            }
        }
    }
}

fn sort_regional_entries_descending(env: &Env, entries: &mut Vec<RegionalEntry>) {
    let n = entries.len();
    for i in 0..n {
        for j in (i + 1)..n {
            let ei = entries.get(i);
            let ej = entries.get(j);
            if let (Some(ei_val), Some(ej_val)) = (ei, ej) {
                if ej_val.score > ei_val.score {
                    entries.set(i, ej_val);
                    entries.set(j, ei_val);
                }
            }
        }
    }
}

fn sort_achievement_entries_descending(env: &Env, entries: &mut Vec<AchievementEntry>) {
    let n = entries.len();
    for i in 0..n {
        for j in (i + 1)..n {
            let ei = entries.get(i);
            let ej = entries.get(j);
            if let (Some(ei_val), Some(ej_val)) = (ei, ej) {
                if ej_val.total_points > ei_val.total_points {
                    entries.set(i, ej_val);
                    entries.set(j, ei_val);
                }
            }
        }
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use soroban_sdk::{testutils::Address as _, Env, Symbol};

    // Helper: a minimal contract shell used only to activate contract storage.
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
    fn test_update_and_get_leaderboard() {
        let (env, _contract_id) = make_env();
        let player = Address::generate(&env);
        let category = Symbol::new(&env, CATEGORY_ESSENCE);
        let period = Symbol::new(&env, PERIOD_DAILY);

        env.as_contract(&_contract_id, || {
            update_score(&env, &player, category.clone(), period.clone(), 100).unwrap();
            let board = get_leaderboard(&env, category, period, 10).unwrap();
            assert_eq!(board.len(), 1);
            assert_eq!(board.get(0).unwrap().score, 100);
        });
    }

    #[test]
    fn test_invalid_category() {
        let (env, _contract_id) = make_env();
        let player = Address::generate(&env);
        let category = Symbol::new(&env, "invalid");
        let period = Symbol::new(&env, PERIOD_DAILY);

        env.as_contract(&_contract_id, || {
            let err = update_score(&env, &player, category, period, 100).unwrap_err();
            assert_eq!(err, LeaderboardError::InvalidCategory);
        });
    }

    #[test]
    fn test_invalid_time_period() {
        let (env, _contract_id) = make_env();
        let player = Address::generate(&env);
        let category = Symbol::new(&env, CATEGORY_ESSENCE);
        let period = Symbol::new(&env, "invalid");

        env.as_contract(&_contract_id, || {
            let err = update_score(&env, &player, category, period, 100).unwrap_err();
            assert_eq!(err, LeaderboardError::InvalidTimePeriod);
        });
    }

    #[test]
    fn test_guild_leaderboard() {
        let (env, _contract_id) = make_env();
        let admin = Address::generate(&env);

        env.as_contract(&_contract_id, || {
            set_admin(&env, &admin);
            update_guild_score(&env, &admin, String::from_str(&env, "Test Guild"), 1000, 10).unwrap();
            let board = get_guild_leaderboard(&env, 10);
            assert_eq!(board.len(), 1);
        });
    }

    #[test]
    fn test_achievement_leaderboard() {
        let (env, _contract_id) = make_env();
        let player = Address::generate(&env);

        env.as_contract(&_contract_id, || {
            update_achievement_score(&env, &player, 5, 500).unwrap();
            let board = get_achievement_leaderboard(&env, 10);
            assert_eq!(board.len(), 1);
            assert_eq!(board.get(0).unwrap().achievement_count, 5);
        });
    }
}
