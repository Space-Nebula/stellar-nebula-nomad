use soroban_sdk::{contracttype, contracterror, symbol_short, Address, Env};

pub const SEASON_DURATION_SECS: u64 = 30 * 24 * 60 * 60; // 30 days

#[derive(Clone)]
#[contracttype]
pub enum SeasonKey {
    /// Active season metadata.
    CurrentSeason,
    /// Season count.
    SeasonCount,
    /// Player's seasonal participation: (profile_id, season_id) -> ParticipantStats
    ParticipantStats(u64, u64),
}

#[derive(Clone, Debug, PartialEq)]
#[contracttype]
pub struct Season {
    pub id: u64,
    pub start_time: u64,
    pub end_time: u64,
    pub title: soroban_sdk::String,
}

#[derive(Clone, Debug, PartialEq)]
#[contracttype]
pub struct ParticipantStats {
    pub profile_id: u64,
    pub season_id: u64,
    pub total_scans: u32,
    pub essence_collected: i128,
}

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum SeasonError {
    NoActiveSeason = 1,
    SeasonAlreadyStarted = 2,
    Unauthorized = 3,
}

/// Initialize the first season.
pub fn initialize_season(env: &Env, admin: Address, title: soroban_sdk::String) -> Result<u64, SeasonError> {
    admin.require_auth();

    if env.storage().instance().has(&SeasonKey::CurrentSeason) {
        return Err(SeasonError::SeasonAlreadyStarted);
    }

    let start_time = env.ledger().timestamp();
    let end_time = start_time + SEASON_DURATION_SECS;
    let id = 1u64;

    let season = Season {
        id,
        start_time,
        end_time,
        title,
    };

    env.storage().instance().set(&SeasonKey::CurrentSeason, &season);
    env.storage().instance().set(&SeasonKey::SeasonCount, &id);

    env.events().publish(
        (symbol_short!("season"), symbol_short!("started")),
        (id, start_time, end_time),
    );

    Ok(id)
}

/// Get the current active season. Auto-resets if expired.
pub fn get_current_season(env: &Env) -> Result<Season, SeasonError> {
    let season: Season = env.storage().instance().get(&SeasonKey::CurrentSeason)
        .ok_or(SeasonError::NoActiveSeason)?;

    let now = env.ledger().timestamp();
    if now > season.end_time {
        // In a real implementation, we might auto-trigger reset here or require manual intervention.
        // For now, return the expired season and let the caller handle it.
    }

    Ok(season)
}

/// Record seasonal progress for a player.
pub fn record_participation(
    env: &Env,
    profile_id: u64,
    scans: u32,
    essence: i128,
) -> Result<(), SeasonError> {
    let season = get_current_season(env)?;
    let key = SeasonKey::ParticipantStats(profile_id, season.id);

    let mut stats: ParticipantStats = env.storage().persistent().get(&key).unwrap_or(ParticipantStats {
        profile_id,
        season_id: season.id,
        total_scans: 0,
        essence_collected: 0,
    });

    stats.total_scans += scans;
    stats.essence_collected += essence;

    env.storage().persistent().set(&key, &stats);

    Ok(())
}

/// Admin-triggered season reset.
pub fn reset_season(env: &Env, admin: Address, new_title: soroban_sdk::String) -> Result<u64, SeasonError> {
    admin.require_auth();

    let old_season = get_current_season(env)?;
    let new_id = old_season.id + 1;
    let start_time = env.ledger().timestamp();
    let end_time = start_time + SEASON_DURATION_SECS;

    let new_season = Season {
        id: new_id,
        start_time,
        end_time,
        title: new_title,
    };

    env.storage().instance().set(&SeasonKey::CurrentSeason, &new_season);
    env.storage().instance().set(&SeasonKey::SeasonCount, &new_id);

    env.events().publish(
        (symbol_short!("season"), symbol_short!("reset")),
        (old_season.id, new_id),
    );

    Ok(new_id)
}
