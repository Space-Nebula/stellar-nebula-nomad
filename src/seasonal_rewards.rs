//! Seasonal content rotation, season-pass tracks and seasonal challenges.
//!
//! This module is the user-facing layer over `seasons`, `battle_pass`,
//! `leaderboards` and `event_scheduler`. It owns:
//!
//! - the exclusive cosmetic/nebula bundle that rotates with each 90-day season,
//! - free and premium season-pass tier math (delegating storage to
//!   `battle_pass`),
//! - season-scoped challenge wrappers (delegating tracking to
//!   `event_scheduler`), and
//! - one atomic, idempotent transition helper that archives the old season,
//!   resets rankings, and provisions the new season.
//!
//! Premium rewards are cosmetic only; they never grant a stat advantage.

use soroban_sdk::{contracterror, contracttype, symbol_short, Address, Env, String, Symbol, Vec};

use crate::battle_pass::{
    self, BattlePassError, PassTier, MAX_FREE_TIERS, MAX_PREMIUM_TIERS,
};
use crate::event_scheduler::{self, EventError, SeasonalChallengeSpec, TimeLimitedChallenge};
use crate::seasons::{
    self, Season, SeasonError, SeasonNebulaType, SeasonPhase, SeasonTheme, SeasonTransition,
};

// ─── Tier Math ────────────────────────────────────────────────────────────────

/// XP required for free tier 1 (matches `battle_pass` defaults).
pub const FREE_TIER_XP_BASE: u64 = 100;
/// XP added per free tier.
pub const FREE_TIER_XP_STEP: u64 = 150;
/// XP required for premium tier 1 (matches `battle_pass` defaults).
pub const PREMIUM_TIER_XP_BASE: u64 = 200;
/// XP added per premium tier.
pub const PREMIUM_TIER_XP_STEP: u64 = 100;

/// Default length of a season-limited challenge (14 days).
pub const SEASONAL_CHALLENGE_WINDOW_SECS: u64 = 14 * 24 * 60 * 60;

/// XP threshold for a tier on the given track, or `None` for an invalid tier.
///
/// Uses saturating integer math so even a garbage tier cannot wrap the ledger.
pub fn tier_xp_required(tier: u32, pass_tier: &PassTier) -> Option<u64> {
    let (base, step, max) = match pass_tier {
        PassTier::Free => (FREE_TIER_XP_BASE, FREE_TIER_XP_STEP, MAX_FREE_TIERS),
        PassTier::Premium => (PREMIUM_TIER_XP_BASE, PREMIUM_TIER_XP_STEP, MAX_PREMIUM_TIERS),
    };
    if tier == 0 || tier > max {
        return None;
    }
    Some(
        base.saturating_add(
            u64::from(tier - 1)
                .saturating_mul(step)
                .min(u64::MAX - base),
        ),
    )
}

/// Highest tier fully unlocked at `xp` (0 when even tier 1 is out of reach).
pub fn highest_unlocked_tier(xp: u64, pass_tier: &PassTier) -> u32 {
    let (base, step, max) = match pass_tier {
        PassTier::Free => (FREE_TIER_XP_BASE, FREE_TIER_XP_STEP, MAX_FREE_TIERS),
        PassTier::Premium => (PREMIUM_TIER_XP_BASE, PREMIUM_TIER_XP_STEP, MAX_PREMIUM_TIERS),
    };
    if xp < base {
        return 0;
    }
    (1u32).saturating_add(u32::try_from((xp - base) / step).unwrap_or(u32::MAX)).min(max)
}

/// Next claimable tier for a player, or `None` when the track is complete.
pub fn next_unlocked_tier(xp: u64, pass_tier: &PassTier) -> Option<u32> {
    let highest = highest_unlocked_tier(xp, pass_tier);
    let max = match pass_tier {
        PassTier::Free => MAX_FREE_TIERS,
        PassTier::Premium => MAX_PREMIUM_TIERS,
    };
    if highest >= max {
        None
    } else {
        Some(highest.saturating_add(1))
    }
}

/// Pro-rata share of a reward pool, rounded down (never exceeds the pool).
pub fn pro_rata_reward(pool: i128, points: u64, total_points: u64) -> i128 {
    if points == 0 || total_points == 0 || pool <= 0 {
        return 0;
    }
    pool.checked_mul(i128::from(points))
        .and_then(|v| v.checked_div(i128::from(total_points)))
        .unwrap_or(pool)
}

// ─── Rotating Content ─────────────────────────────────────────────────────────

/// Short, on-chain-safe label for a season theme.
pub fn theme_symbol(theme: &SeasonTheme) -> Symbol {
    match theme {
        SeasonTheme::EmberNebula => symbol_short!("ember"),
        SeasonTheme::VoidTide => symbol_short!("void"),
        SeasonTheme::StellarApex => symbol_short!("apex"),
        SeasonTheme::CrimsonDrift => symbol_short!("crimson"),
        SeasonTheme::AzureVeil => symbol_short!("azure"),
        SeasonTheme::NovaBurst => symbol_short!("nova"),
    }
}

/// Stable cosmetic ID for a season. The visual palette rotates every six
/// seasons, but each season's collectible ID is unique (FOMO by season).
pub fn exclusive_cosmetic_id(season_id: u64) -> u32 {
    u32::try_from(season_id).unwrap_or(u32::MAX)
}

/// The exclusive content bundle active for a season.
///
/// Theme, nebula type, event multiplier and cosmetic palette all rotate on the
/// six-season cycle defined in `seasons`; the cosmetic ID is season-specific.
pub fn exclusive_content_for(_env: &Env, season_id: u64) -> ExclusiveContent {
    let theme = SeasonTheme::from_season_id(season_id);
    let nebula_type = theme.seasonal_nebula_type();
    let rotation = seasons::rotation_index(season_id);
    ExclusiveContent {
        season_id,
        theme_symbol: theme_symbol(&theme),
        theme,
        nebula_type,
        cosmetic_id: exclusive_cosmetic_id(season_id),
        event_reward_multiplier_bps: theme.event_reward_multiplier_bps(),
        // Alternating tracks make the free/premium cosmetic focus rotate too.
        featured_track: if rotation % 2 == 0 {
            PassTier::Free
        } else {
            PassTier::Premium
        },
    }
}

/// Season-exclusive cosmetic/nebula bundle served to the frontend.
#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub struct ExclusiveContent {
    pub season_id: u64,
    pub theme: SeasonTheme,
    pub theme_symbol: Symbol,
    pub nebula_type: SeasonNebulaType,
    pub cosmetic_id: u32,
    pub event_reward_multiplier_bps: u32,
    pub featured_track: PassTier,
}

// ─── Views ────────────────────────────────────────────────────────────────────

/// Condensed pass progress for a player in the active season.
#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub struct SeasonPassSummary {
    pub season_id: u64,
    pub xp: u64,
    pub free_tier: u32,
    pub premium_tier: u32,
    pub has_premium: bool,
    pub next_free_tier: Option<u32>,
    pub next_premium_tier: Option<u32>,
    pub next_free_tier_xp: Option<u64>,
}

/// Status of one season challenge for one player.
#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub struct ChallengeStatus {
    pub challenge_id: u64,
    pub current: u64,
    pub target: u64,
    pub completed: bool,
    pub claimed: bool,
    pub reward: i128,
    pub time_remaining_secs: u64,
}

/// Everything a client needs to render the seasonal dashboard in one read.
#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub struct SeasonDashboard {
    pub season: Season,
    pub phase: SeasonPhase,
    pub time_remaining_secs: u64,
    pub progress_bps: u32,
    pub chapter: u32,
    pub exclusive_content: ExclusiveContent,
    pub pass: Option<SeasonPassSummary>,
    pub next_season_id: u64,
    pub next_season_start: u64,
}

/// Result of an atomic season transition.
#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub struct SeasonTransitionView {
    pub transition: SeasonTransition,
    pub season: Season,
    pub exclusive_content: ExclusiveContent,
    pub leaderboards_reset: bool,
    pub rankings_reset: bool,
}

// ─── Storage & Errors ─────────────────────────────────────────────────────────

#[derive(Clone)]
#[contracttype]
pub enum SeasonalRewardsKey {
    /// Exclusive content bundle for a season: `season_id -> ExclusiveContent`.
    ExclusiveContent(u64),
    /// Idempotency guard for season-pass reward table installation.
    SeasonPassInitialized(u64),
}

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum SeasonalRewardsError {
    NoActiveSeason = 1,
    SeasonNotExpired = 2,
    InvalidTier = 3,
    InvalidChallenge = 4,
    ChallengeNotFound = 5,
    ChallengeNotComplete = 6,
    AlreadyClaimed = 7,
    PremiumRequired = 8,
    NotEnoughXP = 9,
    SeasonFailed = 10,
    BattlePassFailed = 11,
    EventFailed = 12,
}

impl crate::error_standard::StandardContractError for SeasonalRewardsError {
    fn descriptor(self) -> crate::error_standard::ErrorDescriptor {
        use crate::error_standard::ErrorKind;
        let (kind, retryable) = match self {
            Self::NoActiveSeason | Self::ChallengeNotFound => (ErrorKind::NotFound, false),
            Self::SeasonNotExpired | Self::ChallengeNotComplete => (ErrorKind::Conflict, true),
            Self::AlreadyClaimed | Self::InvalidChallenge => (ErrorKind::Conflict, false),
            Self::InvalidTier | Self::NotEnoughXP => (ErrorKind::Validation, false),
            Self::PremiumRequired => (ErrorKind::Authorization, false),
            Self::SeasonFailed | Self::BattlePassFailed | Self::EventFailed => {
                (ErrorKind::Internal, false)
            }
        };
        crate::error_standard::ErrorDescriptor {
            module: "seasonal_rewards",
            code: self as u32,
            kind,
            retryable,
        }
    }
}

impl From<SeasonError> for SeasonalRewardsError {
    fn from(err: SeasonError) -> Self {
        match err {
            SeasonError::NoActiveSeason => Self::NoActiveSeason,
            SeasonError::SeasonNotExpired => Self::SeasonNotExpired,
            _ => Self::SeasonFailed,
        }
    }
}

impl From<BattlePassError> for SeasonalRewardsError {
    fn from(err: BattlePassError) -> Self {
        match err {
            BattlePassError::NotEnoughXP => Self::NotEnoughXP,
            BattlePassError::AlreadyClaimed | BattlePassError::ChallengeXpAlreadyGranted => {
                Self::AlreadyClaimed
            }
            BattlePassError::InvalidTier | BattlePassError::TierOutOfRange => Self::InvalidTier,
            BattlePassError::NoActiveSeason => Self::NoActiveSeason,
            BattlePassError::PremiumRequired => Self::PremiumRequired,
        }
    }
}

impl From<EventError> for SeasonalRewardsError {
    fn from(err: EventError) -> Self {
        match err {
            EventError::EventNotFound | EventError::ChallengeNotFound => Self::ChallengeNotFound,
            EventError::ChallengeNotComplete => Self::ChallengeNotComplete,
            EventError::AlreadyClaimed => Self::AlreadyClaimed,
            EventError::NoActiveSeason => Self::NoActiveSeason,
            EventError::InvalidEventWindow
            | EventError::EventAlreadyPassed
            | EventError::ChallengeExpired
            | EventError::ChallengeNotStarted => Self::InvalidChallenge,
            _ => Self::EventFailed,
        }
    }
}

// ─── Season Pass ──────────────────────────────────────────────────────────────

/// Install the default reward tables and exclusive content for `season_id`.
///
/// Idempotent: returns `false` when the season was already initialized.
pub fn init_season_pass(
    env: &Env,
    admin: Address,
    season_id: u64,
) -> Result<bool, SeasonalRewardsError> {
    admin.require_auth();
    init_season_pass_after_auth(env, season_id)
}

fn init_season_pass_after_auth(
    env: &Env,
    season_id: u64,
) -> Result<bool, SeasonalRewardsError> {
    let key = SeasonalRewardsKey::SeasonPassInitialized(season_id);
    if env.storage().instance().has(&key) {
        return Ok(false);
    }

    battle_pass::init_default_season_rewards_inner(env, season_id)?;
    let content = exclusive_content_for(env, season_id);
    env.storage()
        .instance()
        .set(&SeasonalRewardsKey::ExclusiveContent(season_id), &content);
    env.storage().instance().set(&key, &true);

    env.events().publish(
        (symbol_short!("season"), symbol_short!("pass_new")),
        (season_id, content.cosmetic_id, content.featured_track == PassTier::Premium),
    );

    Ok(true)
}

/// Admin: force a refresh of the rotated content bundle (e.g. after a theme
/// hotfix). The cosmetic ID stays tied to the season.
pub fn rotate_exclusive_content(
    env: &Env,
    admin: Address,
    season_id: u64,
) -> Result<ExclusiveContent, SeasonalRewardsError> {
    admin.require_auth();
    let content = exclusive_content_for(env, season_id);
    env.storage()
        .instance()
        .set(&SeasonalRewardsKey::ExclusiveContent(season_id), &content);
    env.events().publish(
        (symbol_short!("season"), symbol_short!("content")),
        (season_id, content.cosmetic_id, content.theme_symbol.clone()),
    );
    Ok(content)
}

/// Exclusive content for a season, if the pass has been initialized.
pub fn get_exclusive_content(env: &Env, season_id: u64) -> Option<ExclusiveContent> {
    env.storage()
        .instance()
        .get(&SeasonalRewardsKey::ExclusiveContent(season_id))
}

/// Player-facing pass summary for the active season.
pub fn get_season_pass_summary(
    env: &Env,
    profile_id: u64,
) -> Result<SeasonPassSummary, SeasonalRewardsError> {
    let season = seasons::get_current_season(env)?;
    let state = battle_pass::get_pass_progress(env, profile_id, season.id);
    let xp = state.as_ref().map_or(0, |s| s.xp);
    let free_tier = highest_unlocked_tier(xp, &PassTier::Free);
    let premium_tier = highest_unlocked_tier(xp, &PassTier::Premium);
    let next_free_tier = next_unlocked_tier(xp, &PassTier::Free);
    Ok(SeasonPassSummary {
        season_id: season.id,
        xp,
        free_tier,
        premium_tier,
        has_premium: battle_pass::is_premium_holder(env, profile_id, season.id),
        next_free_tier,
        next_premium_tier: next_unlocked_tier(xp, &PassTier::Premium),
        next_free_tier_xp: next_free_tier.and_then(|t| tier_xp_required(t, &PassTier::Free)),
    })
}

/// Claim a tier reward from the active season's free or premium track.
pub fn claim_season_pass_reward(
    env: &Env,
    player: Address,
    profile_id: u64,
    tier: u32,
    pass_tier: PassTier,
) -> Result<i128, SeasonalRewardsError> {
    if tier_xp_required(tier, &pass_tier).is_none() {
        return Err(SeasonalRewardsError::InvalidTier);
    }
    Ok(battle_pass::claim_reward_v2(
        env,
        player,
        profile_id,
        tier,
        pass_tier,
    )?)
}

/// Unlock the premium (cosmetic-only) track for the active season.
pub fn unlock_premium_pass(
    env: &Env,
    player: Address,
    profile_id: u64,
) -> Result<(), SeasonalRewardsError> {
    Ok(battle_pass::unlock_premium_pass(env, player, profile_id)?)
}

/// Convert seasonal scan/essence activity into season-pass XP.
///
/// Called by the `record_seasonal_participation` contract method so one player
/// action updates both the season stats and the pass ladder.
pub fn record_seasonal_activity(
    env: &Env,
    profile_id: u64,
    scans: u32,
    essence: i128,
) -> Result<u64, SeasonalRewardsError> {
    Ok(battle_pass::add_xp(env, profile_id, scans, essence)?)
}

/// Record discovery of the active season's exclusive nebula and award the
/// associated battle-pass XP bonus.
pub fn record_nebula_discovery(
    env: &Env,
    profile_id: u64,
) -> Result<(), SeasonalRewardsError> {
    seasons::record_seasonal_nebula_discovery(env, profile_id)?;
    battle_pass::add_xp_for_seasonal_nebula(env, profile_id)?;
    Ok(())
}

// ─── Seasonal Challenges ──────────────────────────────────────────────────────

/// Create a challenge whose window is clamped to the active 90-day season.
///
/// Authentication is performed by `event_scheduler::create_time_limited_challenge`.
pub fn create_seasonal_challenge(
    env: &Env,
    admin: &Address,
    spec: SeasonalChallengeSpec,
    duration_secs: u64,
) -> Result<u64, SeasonalRewardsError> {
    let season = seasons::get_current_season(env)?;
    if spec.target_value == 0 || duration_secs == 0 {
        return Err(SeasonalRewardsError::InvalidChallenge);
    }
    let now = env.ledger().timestamp();
    if now.saturating_add(duration_secs) > season.end_time {
        return Err(SeasonalRewardsError::InvalidChallenge);
    }
    Ok(event_scheduler::create_time_limited_challenge(
        env,
        admin,
        season.id,
        duration_secs,
        spec,
    )?)
}

/// Record progress, rejecting challenges that belong to another season.
pub fn record_seasonal_challenge_progress(
    env: &Env,
    profile_id: u64,
    challenge_id: u64,
    metric_delta: u64,
) -> Result<u64, SeasonalRewardsError> {
    require_current_season_challenge(env, challenge_id)?;
    Ok(event_scheduler::record_challenge_progress(
        env,
        profile_id,
        challenge_id,
        metric_delta,
    )?)
}

/// Claim a completed challenge, paying the premium rate to premium holders.
///
/// Challenges from the season that just ended remain claimable during the
/// event scheduler's seven-day grace window.
pub fn claim_seasonal_challenge_reward(
    env: &Env,
    player: Address,
    profile_id: u64,
    challenge_id: u64,
) -> Result<i128, SeasonalRewardsError> {
    let challenge = challenge_for_active_or_just_ended_season(env, challenge_id)?;
    let is_premium = battle_pass::is_premium_holder(env, profile_id, challenge.season_id);
    let reward = event_scheduler::claim_challenge_reward(
        env,
        &player,
        profile_id,
        challenge_id,
        is_premium,
    )?;

    // Completing a challenge also feeds the season pass. The battle-pass helper
    // deduplicates by challenge ID; a prior grant is not an error here.
    let _ = battle_pass::add_xp_for_challenge(env, profile_id, challenge_id);

    Ok(reward)
}

fn require_current_season_challenge(
    env: &Env,
    challenge_id: u64,
) -> Result<TimeLimitedChallenge, SeasonalRewardsError> {
    let season = seasons::get_current_season(env)?;
    let challenge = event_scheduler::get_challenge(env, challenge_id)?;
    if challenge.season_id != season.id {
        return Err(SeasonalRewardsError::InvalidChallenge);
    }
    Ok(challenge)
}

/// Accept the active season and the one archived by the latest transition.
///
/// The event scheduler still enforces its seven-day claim grace period, so this
/// only prevents players from claiming challenges from unrelated seasons.
fn challenge_for_active_or_just_ended_season(
    env: &Env,
    challenge_id: u64,
) -> Result<TimeLimitedChallenge, SeasonalRewardsError> {
    let challenge = event_scheduler::get_challenge(env, challenge_id)?;
    let season = seasons::get_current_season(env)?;
    let just_ended = seasons::get_last_transition(env)
        .map_or(false, |t| t.from_season_id == challenge.season_id);
    if challenge.season_id != season.id && !just_ended {
        return Err(SeasonalRewardsError::InvalidChallenge);
    }
    Ok(challenge)
}

/// All still-open challenges belonging to `season_id`.
pub fn seasonal_challenges_for_season(
    env: &Env,
    season_id: u64,
) -> Vec<TimeLimitedChallenge> {
    let mut out = Vec::new(env);
    for id in event_scheduler::get_active_challenges(env).iter() {
        if let Ok(challenge) = event_scheduler::get_challenge(env, id) {
            if challenge.season_id == season_id {
                out.push_back(challenge);
            }
        }
    }
    out
}

/// Player-facing progress for one challenge.
pub fn get_challenge_status(
    env: &Env,
    profile_id: u64,
    challenge_id: u64,
) -> Result<ChallengeStatus, SeasonalRewardsError> {
    let challenge = challenge_for_active_or_just_ended_season(env, challenge_id)?;
    let progress = event_scheduler::get_challenge_progress(env, challenge_id, profile_id);
    let current = progress.as_ref().map_or(0, |p| p.current_value);
    let completed = progress.as_ref().map_or(false, |p| p.completed);
    let now = env.ledger().timestamp();
    let is_premium = battle_pass::is_premium_holder(env, profile_id, challenge.season_id);
    Ok(ChallengeStatus {
        challenge_id,
        current,
        target: challenge.target_value,
        completed,
        claimed: event_scheduler::is_challenge_claimed(env, challenge_id, profile_id),
        reward: if is_premium {
            challenge.reward_premium
        } else {
            challenge.reward_free
        },
        time_remaining_secs: challenge.end_time.saturating_sub(now),
    })
}

// ─── Dashboard & Transition ───────────────────────────────────────────────────

/// Single-read view for the seasonal UI. Pass `None` for a logged-out viewer.
pub fn get_season_dashboard(
    env: &Env,
    profile_id: Option<u64>,
) -> Result<SeasonDashboard, SeasonalRewardsError> {
    let season = seasons::get_current_season(env)?;
    let exclusive_content = get_exclusive_content(env, season.id)
        .unwrap_or_else(|| exclusive_content_for(env, season.id));
    Ok(SeasonDashboard {
        chapter: seasons::get_current_chapter(env)?,
        phase: seasons::get_season_phase(env)?,
        time_remaining_secs: seasons::get_season_time_remaining(env)?,
        progress_bps: seasons::get_season_progress_bps(env)?,
        next_season_id: season.id.saturating_add(1),
        next_season_start: season.end_time,
        pass: match profile_id {
            Some(id) => Some(get_season_pass_summary(env, id)?),
            None => None,
        },
        season,
        exclusive_content,
    })
}

fn transition_after_auth(
    env: &Env,
    new_title: String,
    participant_ids: &Vec<u64>,
    ranked_players: Vec<Address>,
) -> Result<SeasonTransitionView, SeasonalRewardsError> {
    let now = env.ledger().timestamp();
    let (new_id, transition) =
        seasons::rollover_season_inner(env, now, new_title, participant_ids, true)?;

    // Soft-reset ELO ratings and align its season counter with `seasons`.
    let advanced = crate::elo_rating::advance_season(env);
    let mut elo_config = crate::elo_rating::get_season_config(env);
    elo_config.current_season = u32::try_from(new_id).unwrap_or(advanced);
    env.storage()
        .persistent()
        .set(&crate::elo_rating::EloKey::SeasonConfig, &elo_config);
    crate::elo_rating::apply_seasonal_reset(env, ranked_players);

    // Provision the incoming season's pass and rotating exclusive content.
    init_season_pass_after_auth(env, new_id)?;
    let season = seasons::get_current_season(env)?;
    let exclusive_content = get_exclusive_content(env, new_id)
        .unwrap_or_else(|| exclusive_content_for(env, new_id));

    env.events().publish(
        (symbol_short!("season"), symbol_short!("trans")),
        (
            transition.from_season_id,
            new_id,
            transition.transitioned_at,
        ),
    );

    Ok(SeasonTransitionView {
        transition,
        season,
        exclusive_content,
        leaderboards_reset: true,
        rankings_reset: true,
    })
}

/// Admin/keeper: perform a full 90-day season transition.
///
/// One call archives the ended season, resets seasonal leaderboards and ELO
/// ratings, starts the next season, and provisions its pass and exclusive
/// content. The admin authenticates once for the whole operation.
pub fn transition_season(
    env: &Env,
    admin: Address,
    new_title: String,
    participant_ids: Vec<u64>,
    ranked_players: Vec<Address>,
) -> Result<SeasonTransitionView, SeasonalRewardsError> {
    admin.require_auth();
    transition_after_auth(env, new_title, &participant_ids, ranked_players)
}

/// Idempotent variant: returns `None` while the current season is still active.
/// Safe for a keeper to call on every ledger tick.
pub fn ensure_season_transition(
    env: &Env,
    admin: Address,
    new_title: String,
    participant_ids: Vec<u64>,
    ranked_players: Vec<Address>,
) -> Result<Option<SeasonTransitionView>, SeasonalRewardsError> {
    admin.require_auth();
    if !seasons::is_transition_due(env)? {
        return Ok(None);
    }
    transition_after_auth(env, new_title, &participant_ids, ranked_players).map(Some)
}

// ─── Pure Logic Tests ─────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tier_xp_matches_battle_pass_tables() {
        assert_eq!(tier_xp_required(1, &PassTier::Free), Some(100));
        assert_eq!(tier_xp_required(30, &PassTier::Free), Some(100 + 29 * 150));
        assert_eq!(tier_xp_required(1, &PassTier::Premium), Some(200));
        assert_eq!(tier_xp_required(50, &PassTier::Premium), Some(200 + 49 * 100));
        assert_eq!(tier_xp_required(0, &PassTier::Free), None);
        assert_eq!(tier_xp_required(31, &PassTier::Free), None);
        assert_eq!(tier_xp_required(51, &PassTier::Premium), None);
    }

    #[test]
    fn tier_math_is_monotonic_and_bounded() {
        assert_eq!(highest_unlocked_tier(99, &PassTier::Free), 0);
        assert_eq!(highest_unlocked_tier(100, &PassTier::Free), 1);
        assert_eq!(highest_unlocked_tier(249, &PassTier::Free), 1);
        assert_eq!(highest_unlocked_tier(250, &PassTier::Free), 2);
        assert_eq!(highest_unlocked_tier(u64::MAX, &PassTier::Premium), 50);
        assert_eq!(next_unlocked_tier(99, &PassTier::Free), Some(1));
        assert_eq!(next_unlocked_tier(250, &PassTier::Free), Some(3));
        assert_eq!(next_unlocked_tier(u64::MAX, &PassTier::Free), None);
    }

    #[test]
    fn exclusive_content_rotates_on_six_season_cycle() {
        assert_eq!(exclusive_cosmetic_id(1), 1);
        assert_eq!(exclusive_cosmetic_id(7), 7);
        assert_eq!(seasons::rotation_index(1), seasons::rotation_index(7));
        assert_eq!(
            SeasonTheme::from_season_id(1),
            SeasonTheme::from_season_id(7)
        );
        assert_eq!(
            SeasonTheme::from_season_id(2).seasonal_nebula_type(),
            SeasonNebulaType::VoidRift
        );
    }

    #[test]
    fn pro_rata_reward_rounds_down_and_caps_at_pool() {
        assert_eq!(pro_rata_reward(1_000, 1, 3), 333);
        assert_eq!(pro_rata_reward(1_000, 3, 3), 1_000);
        assert_eq!(pro_rata_reward(1_000, 0, 3), 0);
        assert_eq!(pro_rata_reward(0, 1, 3), 0);
        assert_eq!(pro_rata_reward(i128::MAX, u64::MAX, 1), i128::MAX);
    }
}
