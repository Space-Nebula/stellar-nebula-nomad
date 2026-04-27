use soroban_sdk::{contracttype, contracterror, symbol_short, Address, Env, Vec};
use crate::seasons::{get_current_season};

pub const XP_PER_SCAN: u32 = 10;
pub const XP_PER_ESSENCE: u32 = 1;

#[derive(Clone)]
#[contracttype]
pub enum BattlePassKey {
    /// Player's battle pass state: (profile_id, season_id) -> BattlePassState
    State(u64, u64),
    /// Reward template: tier -> BattlePassReward
    Reward(u32),
}

#[derive(Clone, Debug, PartialEq)]
#[contracttype]
pub struct BattlePassState {
    pub profile_id: u64,
    pub season_id: u64,
    pub xp: u64,
    pub rewards_claimed: u32, // Bitmask for up to 32 tiers
}

#[derive(Clone, Debug, PartialEq)]
#[contracttype]
pub struct BattlePassReward {
    pub tier: u32,
    pub xp_required: u64,
    pub reward_amount: i128,
    pub is_premium: bool,
}

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum BattlePassError {
    NotEnoughXP = 1,
    AlreadyClaimed = 2,
    InvalidTier = 3,
    NoActiveSeason = 4,
}

pub fn add_xp(env: &Env, profile_id: u64, scans: u32, essence: i128) -> Result<u64, BattlePassError> {
    let season = get_current_season(env).map_err(|_| BattlePassError::NoActiveSeason)?;
    let key = BattlePassKey::State(profile_id, season.id);

    let mut state: BattlePassState = env.storage().persistent().get(&key).unwrap_or(BattlePassState {
        profile_id,
        season_id: season.id,
        xp: 0,
        rewards_claimed: 0,
    });

    let gained_xp = (scans as u64 * XP_PER_SCAN as u64) + (essence.max(0) as u64 * XP_PER_ESSENCE as u64);
    state.xp += gained_xp;

    env.storage().persistent().set(&key, &state);

    env.events().publish(
        (symbol_short!("bp"), symbol_short!("xp_gain")),
        (profile_id, state.xp, gained_xp),
    );

    Ok(state.xp)
}

pub fn claim_reward(env: &Env, player: Address, profile_id: u64, tier: u32) -> Result<i128, BattlePassError> {
    player.require_auth();

    let season = get_current_season(env).map_err(|_| BattlePassError::NoActiveSeason)?;
    let key = BattlePassKey::State(profile_id, season.id);

    let mut state: BattlePassState = env.storage().persistent().get(&key).ok_or(BattlePassError::NotEnoughXP)?;

    // Check if already claimed
    if (state.rewards_claimed & (1 << tier)) != 0 {
        return Err(BattlePassError::AlreadyClaimed);
    }

    // Get reward template
    let reward: BattlePassReward = env.storage().instance().get(&BattlePassKey::Reward(tier))
        .ok_or(BattlePassError::InvalidTier)?;

    // Check XP requirements
    if state.xp < reward.xp_required {
        return Err(BattlePassError::NotEnoughXP);
    }

    // Mark as claimed
    state.rewards_claimed |= 1 << tier;
    env.storage().persistent().set(&key, &state);

    env.events().publish(
        (symbol_short!("bp"), symbol_short!("claimed")),
        (profile_id, tier, reward.reward_amount),
    );

    Ok(reward.reward_amount)
}

pub fn get_battle_pass_state(env: &Env, profile_id: u64) -> Result<BattlePassState, BattlePassError> {
    let season = get_current_season(env).map_err(|_| BattlePassError::NoActiveSeason)?;
    let key = BattlePassKey::State(profile_id, season.id);

    env.storage().persistent().get(&key).ok_or(BattlePassError::NotEnoughXP)
}

pub fn init_battle_pass_rewards(env: &Env, admin: Address) {
    admin.require_auth();

    // Setup some default rewards for the MVP
    let rewards = [
        BattlePassReward { tier: 1, xp_required: 100, reward_amount: 50, is_premium: false },
        BattlePassReward { tier: 2, xp_required: 500, reward_amount: 150, is_premium: false },
        BattlePassReward { tier: 3, xp_required: 1000, reward_amount: 300, is_premium: false },
    ];

    for r in rewards.iter() {
        env.storage().instance().set(&BattlePassKey::Reward(r.tier), r);
    }
}
