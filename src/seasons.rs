use soroban_sdk::{contracttype, contracterror, symbol_short, Address, Env};

pub const SEASON_DURATION_SECS: u64 = 30 * 24 * 60 * 60; // 30 days

/// Essence reward per scan during a season.
pub const REWARD_PER_SCAN: i128 = 10;
/// Bonus multiplier (in bps) applied to essence collected as a reward.
pub const ESSENCE_REWARD_BPS: i128 = 500; // 5%

#[derive(Clone)]
#[contracttype]
pub enum SeasonKey {
    /// Active season metadata.
    CurrentSeason,
    /// Season count.
    SeasonCount,
    /// Player's seasonal participation: (profile_id, season_id) -> ParticipantStats
    ParticipantStats(u64, u64),
    /// Archived season snapshot: season_id -> SeasonArchive
    ArchivedSeason(u64),
    /// Per-player season reward ready to claim: (profile_id, season_id) -> i128
    SeasonReward(u64, u64),
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

/// Snapshot of an ended season stored for historical reference.
#[derive(Clone, Debug, PartialEq)]
#[contracttype]
pub struct SeasonArchive {
    pub season: Season,
    pub ended_at: u64,
    pub total_participants: u32,
    pub total_essence_collected: i128,
    pub total_rewards_distributed: i128,
}

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum SeasonError {
    NoActiveSeason = 1,
    SeasonAlreadyStarted = 2,
    Unauthorized = 3,
    SeasonNotExpired = 4,
    NoRewardToClaim = 5,
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

/// End the current season, distribute rewards, archive data, and start the next season.
///
/// Reward per participant = (total_scans * REWARD_PER_SCAN)
///                        + (essence_collected * ESSENCE_REWARD_BPS / 10_000)
///
/// The caller must pass the profile IDs of all participants so their rewards
/// can be computed and stored individually for later claiming via
/// `claim_season_reward`.
pub fn end_season(
    env: &Env,
    admin: Address,
    new_title: soroban_sdk::String,
    participant_ids: soroban_sdk::Vec<u64>,
) -> Result<u64, SeasonError> {
    admin.require_auth();

    let season = get_current_season(env)?;
    let now = env.ledger().timestamp();

    if now < season.end_time {
        return Err(SeasonError::SeasonNotExpired);
    }

    let mut total_essence: i128 = 0;
    let mut total_rewards: i128 = 0;

    // Compute and store per-player rewards; clear participation records.
    for profile_id in participant_ids.iter() {
        let stats_key = SeasonKey::ParticipantStats(profile_id, season.id);
        if let Some(stats) = env
            .storage()
            .persistent()
            .get::<SeasonKey, ParticipantStats>(&stats_key)
        {
            let reward = (stats.total_scans as i128) * REWARD_PER_SCAN
                + stats.essence_collected * ESSENCE_REWARD_BPS / 10_000;

            env.storage()
                .persistent()
                .set(&SeasonKey::SeasonReward(profile_id, season.id), &reward);

            // Remove the participation record (reset seasonal progress).
            env.storage().persistent().remove(&stats_key);

            total_essence += stats.essence_collected;
            total_rewards += reward;
        }
    }

    // Archive the ended season.
    let archive = SeasonArchive {
        season: season.clone(),
        ended_at: now,
        total_participants: participant_ids.len(),
        total_essence_collected: total_essence,
        total_rewards_distributed: total_rewards,
    };
    env.storage()
        .instance()
        .set(&SeasonKey::ArchivedSeason(season.id), &archive);

    env.events().publish(
        (symbol_short!("season"), symbol_short!("ended")),
        (season.id, now, total_rewards),
    );

    // Start the next season immediately.
    let new_id = season.id + 1;
    let new_season = Season {
        id: new_id,
        start_time: now,
        end_time: now + SEASON_DURATION_SECS,
        title: new_title,
    };
    env.storage()
        .instance()
        .set(&SeasonKey::CurrentSeason, &new_season);
    env.storage()
        .instance()
        .set(&SeasonKey::SeasonCount, &new_id);

    env.events().publish(
        (symbol_short!("season"), symbol_short!("started")),
        (new_id, new_season.start_time, new_season.end_time),
    );

    Ok(new_id)
}

/// Claim the reward earned by `profile_id` for a completed season.
pub fn claim_season_reward(env: &Env, season_id: u64, profile_id: u64) -> Result<i128, SeasonError> {
    let key = SeasonKey::SeasonReward(profile_id, season_id);
    let reward: i128 = env
        .storage()
        .persistent()
        .get(&key)
        .ok_or(SeasonError::NoRewardToClaim)?;

    if reward == 0 {
        return Err(SeasonError::NoRewardToClaim);
    }

    env.storage().persistent().remove(&key);

    env.events().publish(
        (symbol_short!("season"), symbol_short!("claimed")),
        (profile_id, season_id, reward),
    );

    Ok(reward)
}

/// Get an archived season by ID.
pub fn get_archived_season(env: &Env, season_id: u64) -> Option<SeasonArchive> {
    env.storage()
        .instance()
        .get(&SeasonKey::ArchivedSeason(season_id))
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
