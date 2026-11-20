//! Challenge system for battle pass XP earning (#550, #642).
//!
//! Daily, weekly, and seasonal challenges provide additional XP opportunities
//! beyond regular gameplay activities. Challenges are indexed by period so a
//! new day/week automatically exposes a fresh set without deleting history.

use soroban_sdk::{contracterror, contracttype, symbol_short, Address, Env, String, Vec};

// ─── Challenge Types ──────────────────────────────────────────────────────────

#[derive(Clone, Debug, PartialEq, Eq)]
#[contracttype]
pub enum ChallengeFrequency {
    Daily,
    Weekly,
    Seasonal,
}

// ─── Storage Keys ─────────────────────────────────────────────────────────────

#[derive(Clone)]
#[contracttype]
pub enum ChallengeKey {
    /// Active challenge: challenge_id -> Challenge
    ChallengeSysChallenge(u64),
    /// Monotonic challenge ID counter.
    ChallengeCounter,
    /// Every challenge ever created, used for active-window scans.
    ChallengeIndex,
    /// Player progress: (profile_id, challenge_id) -> ChallengeProgress
    Progress(u64, u64),
    /// Daily challenges for a day: day -> Vec<u64>
    DailyChallenges(u32),
    /// Weekly challenges for a week: week -> Vec<u64>
    WeeklyChallenges(u32),
    /// Seasonal challenges for a season window: season -> Vec<u64>
    SeasonalChallenges(u32),
}

/// Length of a challenge "day" in seconds.
pub const DAY_SECONDS: u64 = 86_400;
/// Length of a challenge "week" in seconds.
pub const WEEK_SECONDS: u64 = 7 * DAY_SECONDS;
/// Length of a seasonal challenge window (matches the 90-day season).
pub const SEASON_SECONDS: u64 = 90 * DAY_SECONDS;

// ─── Data Structs ─────────────────────────────────────────────────────────────

#[derive(Clone, Debug, PartialEq)]
#[contracttype]
pub struct Challenge {
    pub id: u64,
    pub frequency: ChallengeFrequency,
    pub name: String,
    pub description: String,
    pub target_value: u64,
    pub xp_reward: u64,
    pub reward_item_id: Option<u32>,
    pub starts_at: u64,
    pub ends_at: u64,
}

#[derive(Clone, Debug, PartialEq)]
#[contracttype]
pub struct ChallengeProgress {
    pub profile_id: u64,
    pub challenge_id: u64,
    pub current_value: u64,
    pub completed: bool,
    pub claimed: bool,
    pub completed_at: Option<u64>,
}

/// A challenge paired with one player's progress, for single-call dashboards.
#[derive(Clone, Debug, PartialEq)]
#[contracttype]
pub struct ChallengeWithProgress {
    pub challenge: Challenge,
    pub progress: ChallengeProgress,
}

// ─── Errors ───────────────────────────────────────────────────────────────────

#[contracterror]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum ChallengeError {
    ChallengeNotFound = 1,
    AlreadyCompleted = 2,
    AlreadyClaimed = 3,
    NotCompleted = 4,
    InvalidTarget = 5,
    /// The challenge window has not opened or has already closed.
    ChallengeNotActive = 6,
    /// Challenge duration must be greater than zero.
    InvalidDuration = 7,
}

impl crate::error_standard::StandardContractError for ChallengeError {
    fn descriptor(self) -> crate::error_standard::ErrorDescriptor {
        crate::error_standard::ErrorDescriptor {
            module: "challenge_system",
            code: self as u32,
            kind: crate::error_standard::ErrorKind::Validation,
            retryable: false,
        }
    }
}


// ─── Internal Helpers ─────────────────────────────────────────────────────────

fn day_index(timestamp: u64) -> u32 {
    u32::try_from(timestamp / DAY_SECONDS).unwrap_or(u32::MAX)
}

fn week_index(timestamp: u64) -> u32 {
    u32::try_from(timestamp / WEEK_SECONDS).unwrap_or(u32::MAX)
}

fn season_index(timestamp: u64) -> u32 {
    u32::try_from(timestamp / SEASON_SECONDS).unwrap_or(u32::MAX)
}

fn period_key(frequency: &ChallengeFrequency, timestamp: u64) -> ChallengeKey {
    match frequency {
        ChallengeFrequency::Daily => ChallengeKey::DailyChallenges(day_index(timestamp)),
        ChallengeFrequency::Weekly => ChallengeKey::WeeklyChallenges(week_index(timestamp)),
        ChallengeFrequency::Seasonal => {
            ChallengeKey::SeasonalChallenges(season_index(timestamp))
        }
    }
}

fn append_index(env: &Env, key: &ChallengeKey, challenge_id: u64) {
    let mut ids: Vec<u64> = env
        .storage()
        .persistent()
        .get(key)
        .unwrap_or_else(|| Vec::new(env));
    ids.push_back(challenge_id);
    env.storage().persistent().set(key, &ids);
}

fn is_active(challenge: &Challenge, now: u64) -> bool {
    now >= challenge.starts_at && now <= challenge.ends_at
}

fn default_progress(profile_id: u64, challenge_id: u64) -> ChallengeProgress {
    ChallengeProgress {
        profile_id,
        challenge_id,
        current_value: 0,
        completed: false,
        claimed: false,
        completed_at: None,
    }
}

// ─── Challenge Management ─────────────────────────────────────────────────────

/// Create a new challenge.
///
/// The challenge is appended to the index for its frequency's current period
/// (day, week, or 90-day season), which is what makes daily and weekly boards
/// reset naturally as time advances.
pub fn create_challenge(
    env: &Env,
    admin: &Address,
    frequency: ChallengeFrequency,
    name: String,
    description: String,
    target_value: u64,
    xp_reward: u64,
    duration_hours: u64,
) -> Result<u64, ChallengeError> {
    admin.require_auth();
    if target_value == 0 {
        return Err(ChallengeError::InvalidTarget);
    }
    if duration_hours == 0 {
        return Err(ChallengeError::InvalidDuration);
    }

    let counter: u64 = env
        .storage()
        .persistent()
        .get(&ChallengeKey::ChallengeCounter)
        .unwrap_or(0);
    let id = counter + 1;
    env.storage()
        .persistent()
        .set(&ChallengeKey::ChallengeCounter, &id);

    let timestamp = env.ledger().timestamp();

    let challenge = Challenge {
        id,
        frequency: frequency.clone(),
        name,
        description,
        target_value,
        xp_reward,
        reward_item_id: None,
        starts_at: timestamp,
        ends_at: timestamp + duration_hours * 3_600,
    };

    env.storage()
        .persistent()
        .set(&ChallengeKey::ChallengeSysChallenge(id), &challenge);

    append_index(env, &ChallengeKey::ChallengeIndex, id);
    append_index(env, &period_key(&frequency, timestamp), id);

    env.events().publish(
        (symbol_short!("chal"), symbol_short!("created")),
        (id, target_value, xp_reward),
    );

    Ok(id)
}

/// Get a challenge by ID.
pub fn get_challenge(env: &Env, challenge_id: u64) -> Option<Challenge> {
    env.storage()
        .persistent()
        .get(&ChallengeKey::ChallengeSysChallenge(challenge_id))
}

/// Get a player's progress on one challenge.
pub fn get_challenge_progress(
    env: &Env,
    profile_id: u64,
    challenge_id: u64,
) -> Option<ChallengeProgress> {
    env.storage()
        .persistent()
        .get(&ChallengeKey::Progress(profile_id, challenge_id))
}

/// Update player progress on a challenge.
///
/// Progress can only be recorded while the challenge window is open. Once the
/// target is reached further increments are ignored, keeping the operation
/// idempotent for retried gameplay events.
pub fn update_progress(
    env: &Env,
    profile_id: u64,
    challenge_id: u64,
    increment: u64,
) -> Result<ChallengeProgress, ChallengeError> {
    let challenge = get_challenge(env, challenge_id).ok_or(ChallengeError::ChallengeNotFound)?;

    if !is_active(&challenge, env.ledger().timestamp()) {
        return Err(ChallengeError::ChallengeNotActive);
    }

    let key = ChallengeKey::Progress(profile_id, challenge_id);
    let mut progress: ChallengeProgress = env
        .storage()
        .persistent()
        .get(&key)
        .unwrap_or_else(|| default_progress(profile_id, challenge_id));

    if progress.completed {
        return Ok(progress);
    }

    progress.current_value = progress.current_value.saturating_add(increment);
    if progress.current_value >= challenge.target_value {
        progress.completed = true;
        progress.completed_at = Some(env.ledger().timestamp());
    }

    env.storage().persistent().set(&key, &progress);

    env.events().publish(
        (symbol_short!("chal"), symbol_short!("progress")),
        (profile_id, challenge_id, progress.current_value, progress.completed),
    );

    Ok(progress)
}

/// Update progress with caller authorization.
///
/// Public contract entry points should use this variant so a caller cannot
/// advance another player's challenge state.
pub fn update_progress_for(
    env: &Env,
    player: &Address,
    profile_id: u64,
    challenge_id: u64,
    increment: u64,
) -> Result<ChallengeProgress, ChallengeError> {
    player.require_auth();
    update_progress(env, profile_id, challenge_id, increment)
}

/// Claim a completed challenge's XP reward.
pub fn claim_reward(
    env: &Env,
    profile_id: u64,
    challenge_id: u64,
) -> Result<u64, ChallengeError> {
    let challenge = get_challenge(env, challenge_id).ok_or(ChallengeError::ChallengeNotFound)?;

    if !is_active(&challenge, env.ledger().timestamp()) {
        return Err(ChallengeError::ChallengeNotActive);
    }

    let key = ChallengeKey::Progress(profile_id, challenge_id);
    let mut progress: ChallengeProgress = env
        .storage()
        .persistent()
        .get(&key)
        .ok_or(ChallengeError::NotCompleted)?;

    if progress.claimed {
        return Err(ChallengeError::AlreadyClaimed);
    }
    if !progress.completed {
        return Err(ChallengeError::NotCompleted);
    }

    progress.claimed = true;
    env.storage().persistent().set(&key, &progress);

    env.events().publish(
        (symbol_short!("chal"), symbol_short!("claimed")),
        (profile_id, challenge_id, challenge.xp_reward),
    );

    Ok(challenge.xp_reward)
}

/// Claim a challenge reward with caller authorization.
pub fn claim_reward_for(
    env: &Env,
    player: &Address,
    profile_id: u64,
    challenge_id: u64,
) -> Result<u64, ChallengeError> {
    player.require_auth();
    claim_reward(env, profile_id, challenge_id)
}

/// Get all currently active challenges for a player, including progress.
///
/// A challenge is active while `starts_at <= now <= ends_at`; the per-period
/// indexes created in [`create_challenge`] are retained for historical queries.
pub fn get_active_challenges(env: &Env, profile_id: u64) -> Vec<ChallengeWithProgress> {
    let now = env.ledger().timestamp();
    let ids: Vec<u64> = env
        .storage()
        .persistent()
        .get(&ChallengeKey::ChallengeIndex)
        .unwrap_or_else(|| Vec::new(env));

    let mut active = Vec::new(env);
    for i in 0..ids.len() {
        if let Some(id) = ids.get(i) {
            if let Some(challenge) = get_challenge(env, id) {
                if is_active(&challenge, now) {
                    let progress = env
                        .storage()
                        .persistent()
                        .get::<ChallengeKey, ChallengeProgress>(&ChallengeKey::Progress(
                            profile_id, id,
                        ))
                        .unwrap_or_else(|| default_progress(profile_id, id));
                    active.push_back(ChallengeWithProgress {
                        challenge,
                        progress,
                    });
                }
            }
        }
    }
    active
}

/// Get the challenges created for one frequency's current period.
pub fn get_current_period_challenges(
    env: &Env,
    frequency: ChallengeFrequency,
) -> Vec<Challenge> {
    let key = period_key(&frequency, env.ledger().timestamp());
    let ids: Vec<u64> = env
        .storage()
        .persistent()
        .get(&key)
        .unwrap_or_else(|| Vec::new(env));

    let mut challenges = Vec::new(env);
    for i in 0..ids.len() {
        if let Some(id) = ids.get(i) {
            if let Some(challenge) = get_challenge(env, id) {
                challenges.push_back(challenge);
            }
        }
    }
    challenges
}
