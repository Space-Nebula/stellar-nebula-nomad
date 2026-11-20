//! Cooperative guild quests — Issue #595
//!
//! Guild quests are shared objectives: every member of an alliance contributes
//! to the same [`GuildQuest::current_progress`] counter, and the quest completes
//! once the target is met *and* enough distinct members have helped. Rewards are
//! split between the alliance treasury (spendable through
//! [`crate::guild_economy`] proposals) and claimable member shares.
//!
//! Three extensions sit on top of the original one-off quest API:
//!
//! * **Config** — [`GuildQuestConfig`] carries a frequency, a reward tier, a
//!   minimum contributor count, an optional chain and a treasury share in bps.
//! * **Rotation** — daily / weekly / seasonal quests are bucketed by ledger
//!   time; [`rotate_guild_quests`] spawns the current period's instances from
//!   template quests, and queries only surface the current period.
//! * **Chains** — quests in a [`GuildQuestChain`] unlock in insertion order, so
//!   a completed predecessor is required before a successor accepts progress.
//!
//! Progress contributions are normally forwarded here by gameplay code (for
//! example `alliance_manager::attack_guild_boss`), which authorizes the player
//! itself. The contract entry point in `lib.rs` performs `require_auth` before
//! calling [`contribute_quest_progress`]; the module function intentionally
//! does not, so it can be called from an already-authorized invocation.

use soroban_sdk::{contracterror, contracttype, symbol_short, Address, Env, Symbol, Vec};

use crate::alliance_manager::{
    add_alliance_xp, credit_alliance_treasury, get_alliance, get_player_alliance,
};
use crate::guild_economy::{self, QuestRewardSplit};

// ─── Constants ────────────────────────────────────────────────────────────────

pub const GUILD_QUEST_DAY_SECS: u64 = 86_400;
pub const GUILD_QUEST_WEEK_SECS: u64 = GUILD_QUEST_DAY_SECS * 7;
pub const GUILD_QUEST_SEASON_SECS: u64 = GUILD_QUEST_DAY_SECS * 90;
pub const GUILD_QUEST_BPS_DENOMINATOR: u32 = 10_000;
/// Default split for configured quests: 60% treasury, 40% contributors.
pub const GUILD_QUEST_DEFAULT_TREASURY_BPS: u32 = 6_000;
/// Legacy `create_guild_quest` quests keep their original 100% treasury payout.
pub const GUILD_QUEST_LEGACY_TREASURY_BPS: u32 = GUILD_QUEST_BPS_DENOMINATOR;
pub const GUILD_QUEST_MAX_CHAIN_LENGTH: u32 = 12;
/// Sentinel for "no quest" / "no predecessor" in chain metadata.
pub const GUILD_QUEST_CHAIN_TERMINUS: u64 = 0;

// Tier multipliers in bps: 1=normal, 2=+25%, 3=+50%, 4=double.
const TIER_BPS: [u32; 4] = [10_000, 12_500, 15_000, 20_000];

// ─── Data Types ───────────────────────────────────────────────────────────────

#[derive(Clone, Debug, PartialEq)]
#[contracttype]
pub struct GuildQuest {
    pub quest_id: u64,
    pub alliance_id: u64,
    pub quest_type: Symbol, // scan, harvest, boss_kill
    pub target_count: u64,
    pub current_progress: u64,
    pub reward_essence: i128,
    pub reward_xp: u64,
    pub expires_at: u64,
    pub completed: bool,
}

#[derive(Clone)]
#[contracttype]
pub enum GuildQuestKey {
    Quest(u64, u64), // (alliance_id, quest_id) -> GuildQuest
    QuestCount(u64), // alliance_id -> u64
    Meta(u64, u64),  // (alliance_id, quest_id) -> GuildQuestMeta
    Contribution(u64, u64, Address), // (alliance_id, quest_id, player) -> u64
    Contributors(u64, u64), // (alliance_id, quest_id) -> Vec<Address>
    Assignment(u64, u64), // (alliance_id, quest_id) -> QuestAssignment
    Chain(u64),           // chain_id -> GuildQuestChain
    ChainCount,           // -> u64
    Rotation(u64, u64, u64), // (alliance_id, template_id, period) -> quest_id
}

/// How often a quest recurs. `OneOff` quests never rotate.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[contracttype]
pub enum QuestFrequency {
    OneOff,
    Daily,
    Weekly,
    Seasonal,
}

/// Options supplied when creating a quest. The original eight-argument
/// `create_guild_quest` uses a legacy config so its payout behaviour is
/// unchanged; new callers can opt into tiers and cooperative requirements.
#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct GuildQuestConfig {
    pub frequency: QuestFrequency,
    /// Reward tier from 1 (standard) to 4 (legendary).
    pub tier: u32,
    /// Distinct members who must contribute before the quest can complete.
    pub min_contributors: u32,
    /// 0 for a standalone quest, otherwise an existing chain ID.
    pub chain_id: u64,
    /// Share of the scaled essence that goes to the guild treasury, in bps.
    pub treasury_share_bps: u32,
}

/// Extra per-quest state kept alongside the original [`GuildQuest`] record.
#[derive(Clone, Debug, PartialEq)]
#[contracttype]
pub struct GuildQuestMeta {
    pub quest_id: u64,
    pub alliance_id: u64,
    pub created_by: Address,
    pub frequency: QuestFrequency,
    pub tier: u32,
    pub min_contributors: u32,
    pub chain_id: u64,
    /// Zero-based position in the chain.
    pub chain_index: u32,
    /// Quest that must be completed first; 0 for the chain root.
    pub previous_quest_id: u64,
    pub treasury_share_bps: u32,
    /// Rotation bucket this instance belongs to; 0 for one-off quests.
    pub rotation_period: u64,
    /// Quest this instance was spawned from; 0 for authored quests.
    pub source_template_id: u64,
}

#[derive(Clone, Debug, PartialEq)]
#[contracttype]
pub struct GuildQuestChain {
    pub chain_id: u64,
    pub alliance_id: u64,
    pub name: Symbol,
    pub quest_ids: Vec<u64>,
    pub completed_count: u32,
}

/// An optional roster: when present, only listed members may contribute.
#[derive(Clone, Debug, PartialEq)]
#[contracttype]
pub struct QuestAssignment {
    pub alliance_id: u64,
    pub quest_id: u64,
    pub assignees: Vec<Address>,
    pub assigned_by: Address,
    pub assigned_at: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[contracttype]
pub enum GuildQuestStatus {
    Active,
    Completed,
    Expired,
}

/// Compact view used by the guild quest board frontend.
#[derive(Clone, Debug, PartialEq)]
#[contracttype]
pub struct GuildQuestSummary {
    pub quest_id: u64,
    pub alliance_id: u64,
    pub quest_type: Symbol,
    pub target_count: u64,
    pub current_progress: u64,
    pub status: GuildQuestStatus,
    pub frequency: QuestFrequency,
    pub tier: u32,
    pub chain_id: u64,
    pub rotation_period: u64,
    pub min_contributors: u32,
    pub contributor_count: u32,
    pub assignee_count: u32,
    pub reward_essence: i128,
    pub reward_xp: u64,
    pub expires_at: u64,
}

#[contracterror]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum GuildQuestError {
    QuestNotFound = 1,
    QuestExpired = 2,
    QuestAlreadyCompleted = 3,
    NotAllianceMember = 4,
    Unauthorized = 5,
    /// Quest parameters are invalid (e.g. a zero target).
    InvalidParameters = 6,
    /// A configured quest references a chain that does not exist.
    ChainNotFound = 7,
    /// The chain is full.
    QuestChainFull = 8,
    /// A rotating quest was created for a future period.
    InvalidRotation = 9,
}

impl crate::error_standard::StandardContractError for GuildQuestError {
    fn descriptor(self) -> crate::error_standard::ErrorDescriptor {
        use crate::error_standard::ErrorKind;
        let (kind, retryable) = match self {
            Self::QuestNotFound | Self::ChainNotFound => (ErrorKind::NotFound, false),
            Self::QuestExpired
            | Self::QuestAlreadyCompleted
            | Self::QuestChainFull
            | Self::InvalidRotation => (ErrorKind::Conflict, false),
            Self::NotAllianceMember | Self::Unauthorized => (ErrorKind::Authorization, false),
            Self::InvalidParameters => (ErrorKind::Validation, false),
        };
        crate::error_standard::ErrorDescriptor {
            module: "guild_quests",
            code: self as u32,
            kind,
            retryable,
        }
    }
}

// ─── Pure helpers ─────────────────────────────────────────────────────────────

/// Reward multiplier for a tier, in bps. Out-of-range tiers clamp to the
/// nearest supported tier so bad stored data cannot mint unbounded essence.
pub fn tier_reward_bps(tier: u32) -> u32 {
    if tier == 0 {
        TIER_BPS[0]
    } else if tier > TIER_BPS.len() as u32 {
        TIER_BPS[TIER_BPS.len() - 1]
    } else {
        TIER_BPS[(tier - 1) as usize]
    }
}

/// Apply a bps multiplier to a non-negative amount. Negative "rewards" are
/// treated as zero, matching the treasury module's behaviour.
pub fn apply_reward_bps(amount: i128, bps: u32) -> i128 {
    if amount <= 0 {
        return 0;
    }
    let bps = if bps > GUILD_QUEST_BPS_DENOMINATOR {
        GUILD_QUEST_BPS_DENOMINATOR
    } else {
        bps
    };
    amount
        .saturating_mul(i128::from(bps))
        / i128::from(GUILD_QUEST_BPS_DENOMINATOR)
}

/// Split a quest's base reward into a treasury amount and equal member shares.
/// The division remainder is returned separately for the first claimant so the
/// distributed amounts add up exactly to `scaled_essence`.
pub fn calculate_reward_split(
    base_essence: i128,
    tier: u32,
    treasury_share_bps: u32,
    base_xp: u64,
    member_count: u32,
) -> QuestRewardSplit {
    let tier_bps = tier_reward_bps(tier);
    let scaled_essence = apply_reward_bps(base_essence, tier_bps);
    let treasury_amount = apply_reward_bps(scaled_essence, treasury_share_bps);
    let member_pool = scaled_essence.saturating_sub(treasury_amount);
    let members = if member_count == 0 { 1 } else { member_count };
    let per_member = member_pool / i128::from(members);
    let remainder_to_first = member_pool % i128::from(members);

    let scaled_xp = apply_reward_bps(i128::from(base_xp), tier_bps);
    let xp = if scaled_xp > i128::from(u64::MAX) {
        u64::MAX
    } else {
        scaled_xp as u64
    };

    QuestRewardSplit {
        gross_essence: base_essence,
        tier_bps,
        scaled_essence,
        treasury_amount,
        member_pool,
        per_member,
        remainder_to_first,
        xp,
    }
}

/// Length of one rotation window for a frequency.
pub fn rotation_period_secs(frequency: QuestFrequency) -> u64 {
    match frequency {
        QuestFrequency::OneOff => u64::MAX,
        QuestFrequency::Daily => GUILD_QUEST_DAY_SECS,
        QuestFrequency::Weekly => GUILD_QUEST_WEEK_SECS,
        QuestFrequency::Seasonal => GUILD_QUEST_SEASON_SECS,
    }
}

/// Zero-based rotation bucket containing `timestamp`.
pub fn rotation_period(frequency: QuestFrequency, timestamp: u64) -> u64 {
    match frequency {
        QuestFrequency::OneOff => 0,
        _ => timestamp / rotation_period_secs(frequency),
    }
}

/// Start of the next rotation window, i.e. when a recurring quest expires.
pub fn rotation_expires_at(frequency: QuestFrequency, timestamp: u64) -> u64 {
    match frequency {
        QuestFrequency::OneOff => u64::MAX,
        _ => rotation_period(frequency, timestamp)
            .saturating_add(1)
            .saturating_mul(rotation_period_secs(frequency)),
    }
}

fn quest_expires_at(frequency: QuestFrequency, now: u64, duration_secs: u64) -> u64 {
    match frequency {
        QuestFrequency::OneOff => now.saturating_add(duration_secs),
        _ => rotation_expires_at(frequency, now),
    }
}

/// A chain quest accepts progress only at its own step (or when it has no
/// predecessor). `completed_count` is the number of earlier chain steps done.
pub fn is_chain_unlocked(previous_quest_id: u64, completed_count: u32, chain_index: u32) -> bool {
    previous_quest_id == GUILD_QUEST_CHAIN_TERMINUS || chain_index <= completed_count
}

fn frequency_symbol(frequency: QuestFrequency) -> Symbol {
    match frequency {
        QuestFrequency::OneOff => symbol_short!("oneoff"),
        QuestFrequency::Daily => symbol_short!("daily"),
        QuestFrequency::Weekly => symbol_short!("weekly"),
        QuestFrequency::Seasonal => symbol_short!("season"),
    }
}

// ─── Storage helpers ──────────────────────────────────────────────────────────

fn is_member(env: &Env, alliance_id: u64, player: &Address) -> bool {
    let alliance = match get_alliance(env, alliance_id) {
        Ok(alliance) => alliance,
        Err(_) => return false,
    };
    for member in alliance.members.iter() {
        if member == player.clone() {
            return true;
        }
    }
    false
}

fn authorize_founder(env: &Env, admin: &Address, alliance_id: u64) -> Result<(), GuildQuestError> {
    let alliance =
        get_alliance(env, alliance_id).map_err(|_| GuildQuestError::NotAllianceMember)?;
    if alliance.founder != admin.clone() {
        return Err(GuildQuestError::Unauthorized);
    }
    Ok(())
}

fn validate_config(config: &GuildQuestConfig) -> Result<(), GuildQuestError> {
    if config.tier == 0
        || config.tier > TIER_BPS.len() as u32
        || config.min_contributors == 0
        || config.treasury_share_bps > GUILD_QUEST_BPS_DENOMINATOR
    {
        return Err(GuildQuestError::InvalidParameters);
    }
    Ok(())
}

fn get_chain_internal(env: &Env, chain_id: u64) -> Option<GuildQuestChain> {
    env.storage()
        .persistent()
        .get(&GuildQuestKey::Chain(chain_id))
}

fn get_meta(env: &Env, alliance_id: u64, quest_id: u64) -> Option<GuildQuestMeta> {
    env.storage()
        .persistent()
        .get(&GuildQuestKey::Meta(alliance_id, quest_id))
}

fn get_assignment(env: &Env, alliance_id: u64, quest_id: u64) -> Option<QuestAssignment> {
    env.storage()
        .persistent()
        .get(&GuildQuestKey::Assignment(alliance_id, quest_id))
}

fn assignment_contains(assignment: &QuestAssignment, player: &Address) -> bool {
    for assignee in assignment.assignees.iter() {
        if assignee == player.clone() {
            return true;
        }
    }
    false
}

fn contributors_for(env: &Env, alliance_id: u64, quest_id: u64) -> Vec<Address> {
    env.storage()
        .persistent()
        .get(&GuildQuestKey::Contributors(alliance_id, quest_id))
        .unwrap_or_else(|| Vec::new(env))
}

fn is_current_period(env: &Env, alliance_id: u64, quest_id: u64, now: u64) -> bool {
    match get_meta(env, alliance_id, quest_id) {
        None => true, // legacy one-off quest
        Some(meta) => {
            meta.frequency == QuestFrequency::OneOff
                || rotation_period(meta.frequency, now) == meta.rotation_period
        }
    }
}

/// Create and store a quest without authenticating. Callers must have already
/// authorized `admin` as the alliance founder.
#[allow(clippy::too_many_arguments)]
fn insert_quest(
    env: &Env,
    admin: &Address,
    alliance_id: u64,
    quest_type: Symbol,
    target_count: u64,
    reward_essence: i128,
    reward_xp: u64,
    duration_secs: u64,
    config: GuildQuestConfig,
) -> Result<(u64, GuildQuest), GuildQuestError> {
    if target_count == 0 {
        return Err(GuildQuestError::InvalidParameters);
    }
    validate_config(&config)?;

    let (chain_index, previous_quest_id) = if config.chain_id == GUILD_QUEST_CHAIN_TERMINUS {
        (0, GUILD_QUEST_CHAIN_TERMINUS)
    } else {
        let chain_key = GuildQuestKey::Chain(config.chain_id);
        let mut chain: GuildQuestChain = env
            .storage()
            .persistent()
            .get(&chain_key)
            .ok_or(GuildQuestError::ChainNotFound)?;
        if chain.alliance_id != alliance_id {
            return Err(GuildQuestError::ChainNotFound);
        }
        let index = chain.quest_ids.len();
        if index >= GUILD_QUEST_MAX_CHAIN_LENGTH {
            return Err(GuildQuestError::QuestChainFull);
        }
        let previous = if index == 0 {
            GUILD_QUEST_CHAIN_TERMINUS
        } else {
            chain.quest_ids.get(index - 1).unwrap_or(GUILD_QUEST_CHAIN_TERMINUS)
        };
        (index, previous)
    };

    let count_key = GuildQuestKey::QuestCount(alliance_id);
    let quest_id = env
        .storage()
        .persistent()
        .get::<_, u64>(&count_key)
        .unwrap_or(0)
        .saturating_add(1);
    env.storage().persistent().set(&count_key, &quest_id);

    let now = env.ledger().timestamp();
    let quest = GuildQuest {
        quest_id,
        alliance_id,
        quest_type: quest_type.clone(),
        target_count,
        current_progress: 0,
        reward_essence,
        reward_xp,
        expires_at: quest_expires_at(config.frequency, now, duration_secs),
        completed: false,
    };
    env.storage()
        .persistent()
        .set(&GuildQuestKey::Quest(alliance_id, quest_id), &quest);

    let meta = GuildQuestMeta {
        quest_id,
        alliance_id,
        created_by: admin.clone(),
        frequency: config.frequency,
        tier: config.tier,
        min_contributors: config.min_contributors,
        chain_id: config.chain_id,
        chain_index,
        previous_quest_id,
        treasury_share_bps: config.treasury_share_bps,
        rotation_period: rotation_period(config.frequency, now),
        source_template_id: GUILD_QUEST_CHAIN_TERMINUS,
    };
    env.storage()
        .persistent()
        .set(&GuildQuestKey::Meta(alliance_id, quest_id), &meta);

    if config.chain_id != GUILD_QUEST_CHAIN_TERMINUS {
        let chain_key = GuildQuestKey::Chain(config.chain_id);
        if let Some(mut chain) = get_chain_internal(env, config.chain_id) {
            chain.quest_ids.push_back(quest_id);
            env.storage().persistent().set(&chain_key, &chain);
        }
    }

    env.events().publish(
        (symbol_short!("g_quest"), symbol_short!("created")),
        (alliance_id, quest_id, quest_type, target_count),
    );
    env.events().publish(
        (symbol_short!("g_quest"), symbol_short!("config")),
        (
            alliance_id,
            quest_id,
            frequency_symbol(config.frequency),
            config.tier,
            config.min_contributors,
        ),
    );

    Ok((quest_id, quest))
}

/// True when a quest uses the original payout (full treasury, no member pool).
fn is_legacy_payout(meta: Option<&GuildQuestMeta>) -> bool {
    meta.map_or(true, |meta| {
        meta.tier == 1 && meta.treasury_share_bps == GUILD_QUEST_BPS_DENOMINATOR
    })
}

/// Finalize a quest once its target and cooperative requirement are met.
fn complete_quest(
    env: &Env,
    alliance_id: u64,
    quest: &mut GuildQuest,
    meta: Option<&GuildQuestMeta>,
    contributors: &Vec<Address>,
) {
    quest.completed = true;

    if is_legacy_payout(meta) {
        // Preserve the original payout exactly: the full essence goes to the
        // shared treasury and the full XP to the guild level.
        let _ = credit_alliance_treasury(env, alliance_id, quest.reward_essence);
        let _ = add_alliance_xp(env, alliance_id, quest.reward_xp);
    } else if let Some(meta) = meta {
        let split = calculate_reward_split(
            quest.reward_essence,
            meta.tier,
            meta.treasury_share_bps,
            quest.reward_xp,
            contributors.len(),
        );
        let _ = guild_economy::record_quest_rewards(
            env,
            alliance_id,
            quest.quest_id,
            &split,
            contributors,
        );

        env.events().publish(
            (symbol_short!("g_quest"), symbol_short!("tierdone")),
            (
                alliance_id,
                quest.quest_id,
                split.tier_bps,
                split.scaled_essence,
            ),
        );
    }

    // Chains advance regardless of which payout path was used.
    if let Some(meta) = meta {
        if meta.chain_id != GUILD_QUEST_CHAIN_TERMINUS {
            let chain_key = GuildQuestKey::Chain(meta.chain_id);
            if let Some(mut chain) = get_chain_internal(env, meta.chain_id) {
                chain.completed_count = chain.completed_count.saturating_add(1);
                env.storage().persistent().set(&chain_key, &chain);
            }
        }
    }

    // Keep the original completion event topics and payload for indexers.
    env.events().publish(
        (symbol_short!("g_quest"), symbol_short!("done")),
        (
            alliance_id,
            quest.quest_id,
            quest.reward_essence,
            quest.reward_xp,
        ),
    );
}

// ─── Functions ────────────────────────────────────────────────────────────────

/// Create a new cooperative guild quest with legacy (one-off, 100% treasury)
/// behaviour. Retained unchanged for existing callers.
#[allow(clippy::too_many_arguments)]
pub fn create_guild_quest(
    env: &Env,
    admin: Address,
    alliance_id: u64,
    quest_type: Symbol,
    target_count: u64,
    reward_essence: i128,
    reward_xp: u64,
    duration_secs: u64,
) -> Result<u64, GuildQuestError> {
    let config = GuildQuestConfig {
        frequency: QuestFrequency::OneOff,
        tier: 1,
        min_contributors: 1,
        chain_id: GUILD_QUEST_CHAIN_TERMINUS,
        treasury_share_bps: GUILD_QUEST_LEGACY_TREASURY_BPS,
    };
    create_guild_quest_with_config(
        env,
        admin,
        alliance_id,
        quest_type,
        target_count,
        reward_essence,
        reward_xp,
        duration_secs,
        config,
    )
}

/// Create a guild quest with a frequency, reward tier, chain link and
/// cooperative requirement. Only the alliance founder may create quests: they
/// can mint treasury essence and chain rewards.
#[allow(clippy::too_many_arguments)]
pub fn create_guild_quest_with_config(
    env: &Env,
    admin: Address,
    alliance_id: u64,
    quest_type: Symbol,
    target_count: u64,
    reward_essence: i128,
    reward_xp: u64,
    duration_secs: u64,
    config: GuildQuestConfig,
) -> Result<u64, GuildQuestError> {
    admin.require_auth();
    authorize_founder(env, &admin, alliance_id)?;

    let (quest_id, _) = insert_quest(
        env,
        &admin,
        alliance_id,
        quest_type,
        target_count,
        reward_essence,
        reward_xp,
        duration_secs,
        config,
    )?;
    Ok(quest_id)
}

/// Define an ordered chain of guild quests. Quests are linked to it through
/// [`GuildQuestConfig::chain_id`]; progress unlocks strictly in order.
pub fn create_guild_quest_chain(
    env: &Env,
    admin: Address,
    alliance_id: u64,
    name: Symbol,
) -> Result<u64, GuildQuestError> {
    admin.require_auth();
    authorize_founder(env, &admin, alliance_id)?;

    let chain_id = env
        .storage()
        .persistent()
        .get::<_, u64>(&GuildQuestKey::ChainCount)
        .unwrap_or(0)
        .saturating_add(1);
    env.storage()
        .persistent()
        .set(&GuildQuestKey::ChainCount, &chain_id);

    let chain = GuildQuestChain {
        chain_id,
        alliance_id,
        name: name.clone(),
        quest_ids: Vec::new(env),
        completed_count: 0,
    };
    env.storage()
        .persistent()
        .set(&GuildQuestKey::Chain(chain_id), &chain);

    env.events().publish(
        (symbol_short!("g_quest"), symbol_short!("chainnew")),
        (alliance_id, chain_id, name),
    );
    Ok(chain_id)
}

/// Restrict a quest to an explicit roster of alliance members. An empty roster
/// clears the assignment and returns the quest to open participation.
pub fn assign_guild_quest(
    env: &Env,
    admin: Address,
    alliance_id: u64,
    quest_id: u64,
    assignees: Vec<Address>,
) -> Result<(), GuildQuestError> {
    admin.require_auth();
    authorize_founder(env, &admin, alliance_id)?;

    let quest_key = GuildQuestKey::Quest(alliance_id, quest_id);
    let _quest: GuildQuest = env
        .storage()
        .persistent()
        .get(&quest_key)
        .ok_or(GuildQuestError::QuestNotFound)?;

    let assignment_key = GuildQuestKey::Assignment(alliance_id, quest_id);
    if assignees.len() == 0 {
        env.storage().persistent().remove(&assignment_key);
        env.events().publish(
            (symbol_short!("g_quest"), symbol_short!("unassign")),
            (alliance_id, quest_id),
        );
        return Ok(());
    }

    for assignee in assignees.iter() {
        if !is_member(env, alliance_id, &assignee) {
            return Err(GuildQuestError::NotAllianceMember);
        }
    }

    let assignment = QuestAssignment {
        alliance_id,
        quest_id,
        assignees: assignees.clone(),
        assigned_by: admin,
        assigned_at: env.ledger().timestamp(),
    };
    env.storage()
        .persistent()
        .set(&assignment_key, &assignment);
    env.events().publish(
        (symbol_short!("g_quest"), symbol_short!("assigned")),
        (alliance_id, quest_id, assignees.len()),
    );
    Ok(())
}

/// Remove a quest's roster, making it open to every alliance member again.
pub fn clear_guild_quest_assignment(
    env: &Env,
    admin: Address,
    alliance_id: u64,
    quest_id: u64,
) -> Result<(), GuildQuestError> {
    admin.require_auth();
    authorize_founder(env, &admin, alliance_id)?;

    let quest_key = GuildQuestKey::Quest(alliance_id, quest_id);
    let _quest: GuildQuest = env
        .storage()
        .persistent()
        .get(&quest_key)
        .ok_or(GuildQuestError::QuestNotFound)?;
    env.storage()
        .persistent()
        .remove(&GuildQuestKey::Assignment(alliance_id, quest_id));
    env.events().publish(
        (symbol_short!("g_quest"), symbol_short!("unassign")),
        (alliance_id, quest_id),
    );
    Ok(())
}

/// Increment progress on active quests for a player's alliance.
/// Called automatically when players scan, harvest, or complete boss encounters.
///
/// The caller is responsible for authenticating `player`; this function is also
/// invoked from already-authorized gameplay code such as guild boss attacks.
/// Quests that are expired, chain-locked, out of rotation, or assigned to other
/// members are skipped so a single gameplay action cannot be blocked by them.
pub fn contribute_quest_progress(
    env: &Env,
    player: Address,
    quest_type: Symbol,
    amount: u64,
) -> Result<(), GuildQuestError> {
    let alliance_id =
        get_player_alliance(env, player.clone()).ok_or(GuildQuestError::NotAllianceMember)?;

    let count_key = GuildQuestKey::QuestCount(alliance_id);
    let total_quests = env.storage().persistent().get::<_, u64>(&count_key).unwrap_or(0);

    let now = env.ledger().timestamp();

    for qid in 1..=total_quests {
        let quest_key = GuildQuestKey::Quest(alliance_id, qid);
        let Some(mut quest) = env
            .storage()
            .persistent()
            .get::<_, GuildQuest>(&quest_key)
        else {
            continue;
        };
        if amount == 0 || quest.completed || now > quest.expires_at || quest.quest_type != quest_type
        {
            continue;
        }

        let meta = get_meta(env, alliance_id, qid);
        if let Some(ref meta) = meta {
            if meta.alliance_id != alliance_id {
                continue;
            }
            if meta.frequency != QuestFrequency::OneOff
                && rotation_period(meta.frequency, now) != meta.rotation_period
            {
                continue;
            }
            if meta.chain_id != GUILD_QUEST_CHAIN_TERMINUS {
                let chain = match get_chain_internal(env, meta.chain_id) {
                    Some(chain) => chain,
                    None => continue,
                };
                if !is_chain_unlocked(meta.previous_quest_id, chain.completed_count, meta.chain_index)
                {
                    continue;
                }
            }
        }

        if let Some(assignment) = get_assignment(env, alliance_id, qid) {
            if !assignment_contains(&assignment, &player) {
                continue;
            }
        }

        let contribution_key = GuildQuestKey::Contribution(alliance_id, qid, player.clone());
        let mut contributors = contributors_for(env, alliance_id, qid);
        if !env.storage().persistent().has(&contribution_key) {
            contributors.push_back(player.clone());
        }
        let prior = env
            .storage()
            .persistent()
            .get::<_, u64>(&contribution_key)
            .unwrap_or(0);
        env.storage()
            .persistent()
            .set(&contribution_key, &prior.saturating_add(amount));
        env.storage()
            .persistent()
            .set(&GuildQuestKey::Contributors(alliance_id, qid), &contributors);

        quest.current_progress = quest.current_progress.saturating_add(amount);

        env.events().publish(
            (symbol_short!("g_quest"), symbol_short!("progress")),
            (
                alliance_id,
                qid,
                player.clone(),
                amount,
                contributors.len(),
            ),
        );

        let min_contributors = meta.as_ref().map_or(1, |m| m.min_contributors);
        if quest.current_progress >= quest.target_count && contributors.len() >= min_contributors {
            complete_quest(env, alliance_id, &mut quest, meta.as_ref(), &contributors);
        }

        env.storage().persistent().set(&quest_key, &quest);
    }

    Ok(())
}

/// Spawn this period's instances of daily, weekly or seasonal template quests.
/// Idempotent per (template, period); returns the number of new quests created.
pub fn rotate_guild_quests(
    env: &Env,
    admin: Address,
    alliance_id: u64,
    template_ids: Vec<u64>,
) -> Result<u64, GuildQuestError> {
    admin.require_auth();
    authorize_founder(env, &admin, alliance_id)?;

    let now = env.ledger().timestamp();
    let mut created = 0u64;

    for template_id in template_ids.iter() {
        let template: GuildQuest = env
            .storage()
            .persistent()
            .get(&GuildQuestKey::Quest(alliance_id, template_id))
            .ok_or(GuildQuestError::QuestNotFound)?;
        let meta = get_meta(env, alliance_id, template_id)
            .ok_or(GuildQuestError::InvalidParameters)?;
        if meta.rotation_period > rotation_period(meta.frequency, now) {
            return Err(GuildQuestError::InvalidRotation);
        }
        if meta.frequency == QuestFrequency::OneOff
            || meta.chain_id != GUILD_QUEST_CHAIN_TERMINUS
            || template.alliance_id != alliance_id
        {
            return Err(GuildQuestError::InvalidParameters);
        }

        let period = rotation_period(meta.frequency, now);
        let rotation_key = GuildQuestKey::Rotation(alliance_id, template_id, period);
        if env.storage().persistent().has(&rotation_key) {
            continue;
        }

        let config = GuildQuestConfig {
            frequency: meta.frequency,
            tier: meta.tier,
            min_contributors: meta.min_contributors,
            chain_id: GUILD_QUEST_CHAIN_TERMINUS,
            treasury_share_bps: meta.treasury_share_bps,
        };
        let (quest_id, _) = insert_quest(
            env,
            &admin,
            alliance_id,
            template.quest_type.clone(),
            template.target_count,
            template.reward_essence,
            template.reward_xp,
            0,
            config,
        )?;

        let new_meta_key = GuildQuestKey::Meta(alliance_id, quest_id);
        if let Some(mut new_meta) = get_meta(env, alliance_id, quest_id) {
            new_meta.source_template_id = template_id;
            env.storage().persistent().set(&new_meta_key, &new_meta);
        }
        env.storage().persistent().set(&rotation_key, &quest_id);

        created = created.saturating_add(1);
        env.events().publish(
            (symbol_short!("g_quest"), symbol_short!("rotated")),
            (alliance_id, template_id, quest_id, period),
        );
    }

    Ok(created)
}

/// Get all active (non-expired, uncompleted, in-rotation) cooperative quests
/// for an alliance.
pub fn get_active_guild_quests(env: &Env, alliance_id: u64) -> Vec<GuildQuest> {
    let mut active = Vec::new(env);
    let count_key = GuildQuestKey::QuestCount(alliance_id);
    let total_quests = env.storage().persistent().get::<_, u64>(&count_key).unwrap_or(0);
    let now = env.ledger().timestamp();

    for qid in 1..=total_quests {
        let quest_key = GuildQuestKey::Quest(alliance_id, qid);
        if let Some(quest) = env
            .storage()
            .persistent()
            .get::<_, GuildQuest>(&quest_key)
        {
            if !quest.completed
                && now <= quest.expires_at
                && is_current_period(env, alliance_id, qid, now)
            {
                active.push_back(quest);
            }
        }
    }

    active
}

/// Status-rich quest list for the guild quest board UI. Completed quests from
/// the current period are included so their rewards remain visible.
pub fn get_guild_quest_board(env: &Env, alliance_id: u64) -> Vec<GuildQuestSummary> {
    let mut board = Vec::new(env);
    let count_key = GuildQuestKey::QuestCount(alliance_id);
    let total_quests = env.storage().persistent().get::<_, u64>(&count_key).unwrap_or(0);
    let now = env.ledger().timestamp();

    for qid in 1..=total_quests {
        let Some(quest) = env
            .storage()
            .persistent()
            .get::<_, GuildQuest>(&GuildQuestKey::Quest(alliance_id, qid))
        else {
            continue;
        };
        if !is_current_period(env, alliance_id, qid, now) {
            continue;
        }

        let meta = get_meta(env, alliance_id, qid);
        let status = if quest.completed {
            GuildQuestStatus::Completed
        } else if now > quest.expires_at {
            GuildQuestStatus::Expired
        } else {
            GuildQuestStatus::Active
        };

        board.push_back(GuildQuestSummary {
            quest_id: quest.quest_id,
            alliance_id,
            quest_type: quest.quest_type,
            target_count: quest.target_count,
            current_progress: quest.current_progress,
            status,
            frequency: meta.as_ref().map_or(QuestFrequency::OneOff, |m| m.frequency),
            tier: meta.as_ref().map_or(1, |m| m.tier),
            chain_id: meta
                .as_ref()
                .map_or(GUILD_QUEST_CHAIN_TERMINUS, |m| m.chain_id),
            rotation_period: meta.as_ref().map_or(0, |m| m.rotation_period),
            min_contributors: meta.as_ref().map_or(1, |m| m.min_contributors),
            contributor_count: contributors_for(env, alliance_id, qid).len(),
            assignee_count: get_assignment(env, alliance_id, qid).map_or(0, |a| a.assignees.len()),
            reward_essence: quest.reward_essence,
            reward_xp: quest.reward_xp,
            expires_at: quest.expires_at,
        });
    }

    board
}

/// Fetch one guild quest.
pub fn get_guild_quest(env: &Env, alliance_id: u64, quest_id: u64) -> Option<GuildQuest> {
    env.storage()
        .persistent()
        .get(&GuildQuestKey::Quest(alliance_id, quest_id))
}

/// Fetch the configuration record for a quest. Legacy quests have none.
pub fn get_guild_quest_meta(env: &Env, alliance_id: u64, quest_id: u64) -> Option<GuildQuestMeta> {
    get_meta(env, alliance_id, quest_id)
}

/// Fetch a chain definition, scoped to its alliance.
pub fn get_guild_quest_chain(env: &Env, alliance_id: u64, chain_id: u64) -> Option<GuildQuestChain> {
    get_chain_internal(env, chain_id).filter(|chain| chain.alliance_id == alliance_id)
}

/// Fetch a quest's assignment roster, if any.
pub fn get_guild_quest_assignment(
    env: &Env,
    alliance_id: u64,
    quest_id: u64,
) -> Option<QuestAssignment> {
    get_assignment(env, alliance_id, quest_id)
}

/// Members who have contributed to a quest, in first-contribution order.
pub fn get_guild_quest_contributors(
    env: &Env,
    alliance_id: u64,
    quest_id: u64,
) -> Vec<Address> {
    contributors_for(env, alliance_id, quest_id)
}

// ─────────────────────────────────────────────────────────────────────────────
// Pure-logic tests (no ledger fixtures)
// ─────────────────────────────────────────────────────────────────────────────
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tiers_scale_rewards_in_bps() {
        assert_eq!(tier_reward_bps(1), 10_000);
        assert_eq!(tier_reward_bps(2), 12_500);
        assert_eq!(tier_reward_bps(3), 15_000);
        assert_eq!(tier_reward_bps(4), 20_000);
        assert_eq!(tier_reward_bps(0), 10_000);
        assert_eq!(tier_reward_bps(99), 20_000);
        assert_eq!(apply_reward_bps(1_000, 12_500), 1_250);
        assert_eq!(apply_reward_bps(-5, 12_500), 0);
    }

    #[test]
    fn reward_split_is_lossless() {
        let split = calculate_reward_split(1_000, 2, 6_000, 100, 3);
        assert_eq!(split.scaled_essence, 1_250);
        assert_eq!(split.treasury_amount, 750);
        assert_eq!(split.member_pool, 500);
        assert_eq!(split.per_member, 166);
        assert_eq!(split.remainder_to_first, 2);
        assert_eq!(split.xp, 125);
        assert_eq!(
            split.treasury_amount + split.per_member * 3 + split.remainder_to_first,
            split.scaled_essence
        );
    }

    #[test]
    fn reward_split_handles_zero_members() {
        let split = calculate_reward_split(100, 1, GUILD_QUEST_BPS_DENOMINATOR, 10, 0);
        assert_eq!(split.treasury_amount, 100);
        assert_eq!(split.member_pool, 0);
        assert_eq!(split.per_member, 0);
        assert_eq!(split.remainder_to_first, 0);
    }

    #[test]
    fn rotation_buckets_follow_frequency() {
        assert_eq!(rotation_period(QuestFrequency::Daily, 0), 0);
        assert_eq!(
            rotation_period(QuestFrequency::Daily, GUILD_QUEST_DAY_SECS - 1),
            0
        );
        assert_eq!(
            rotation_period(QuestFrequency::Daily, GUILD_QUEST_DAY_SECS),
            1
        );
        assert_eq!(
            rotation_period(QuestFrequency::Weekly, GUILD_QUEST_WEEK_SECS * 2),
            2
        );
        assert_eq!(
            rotation_period(QuestFrequency::Seasonal, GUILD_QUEST_SEASON_SECS * 3),
            3
        );
        assert_eq!(
            rotation_expires_at(QuestFrequency::Daily, GUILD_QUEST_DAY_SECS + 10),
            2 * GUILD_QUEST_DAY_SECS
        );
    }

    #[test]
    fn chains_unlock_in_insertion_order() {
        // Root has no predecessor and is always unlocked.
        assert!(is_chain_unlocked(GUILD_QUEST_CHAIN_TERMINUS, 0, 0));
        // Step 1 needs the root completed; step 2 needs two completed steps.
        assert!(!is_chain_unlocked(1, 0, 1));
        assert!(is_chain_unlocked(1, 1, 1));
        assert!(!is_chain_unlocked(2, 1, 2));
        assert!(is_chain_unlocked(2, 2, 2));
    }
}
