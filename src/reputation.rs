use soroban_sdk::{contracterror, contracttype, symbol_short, Address, Env, String, Vec};

pub const MIN_REPUTATION: u32 = 1;
pub const MAX_REPUTATION: u32 = 100;
pub const INITIAL_REPUTATION: u32 = 50;
pub const MAX_ACTIVE_REPORTS: u32 = 1000;
pub const REPORT_RESOLUTION_DAYS: u64 = 604_800; // 7 days in seconds
pub const REPUTATION_DECAY_BPS_PER_MONTH: u32 = 500;

#[derive(Clone)]
#[contracttype]
pub enum ReputationKey {
    Score(Address),
    ReputationHistory(Address),
    Behavior(Address),
    ReportCount(Address),
    DisputeList,
    AdminList,
    BanList(Address),
}

#[contracterror]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum ReputationError {
    Unauthorized = 1,
    ReputationNotFound = 2,
    InvalidScore = 3,
    ReportNotFound = 4,
    MaxReportsExceeded = 5,
    InvalidBehavior = 6,
    AlreadyBanned = 7,
    NotInitialized = 8,
}

impl crate::error_standard::StandardContractError for ReputationError {
    fn descriptor(self) -> crate::error_standard::ErrorDescriptor {
        use crate::error_standard::ErrorKind;
        let (kind, retryable) = match self {
            Self::Unauthorized | Self::AlreadyBanned => (ErrorKind::Authorization, false),
            Self::ReputationNotFound | Self::ReportNotFound | Self::NotInitialized => {
                (ErrorKind::NotFound, false)
            }
            Self::InvalidScore | Self::InvalidBehavior => (ErrorKind::Validation, false),
            Self::MaxReportsExceeded => (ErrorKind::ResourceLimit, false),
        };
        crate::error_standard::ErrorDescriptor {
            module: "reputation",
            code: self as u32,
            kind,
            retryable,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[contracttype]
#[repr(u32)]
pub enum BehaviorType {
    Positive = 0,
    Negative = 1,
    Neutral = 2,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[contracttype]
#[repr(u32)]
pub enum ReportStatus {
    Pending = 0,
    Resolved = 1,
    Dismissed = 2,
    Appealed = 3,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[contracttype]
#[repr(u32)]
pub enum ReputationSource {
    GameAchievement = 0,
    CommunityContribution = 1,
    EconomicActivity = 2,
    GovernanceParticipation = 3,
    SocialBehavior = 4,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[contracttype]
#[repr(u32)]
pub enum ReputationLevel {
    Newcomer = 0,
    Scout = 1,
    Contributor = 2,
    Veteran = 3,
    Legend = 4,
}

#[derive(Clone, Debug, PartialEq, Eq)]
#[contracttype]
pub struct ReputationBenefit {
    pub level: ReputationLevel,
    pub voting_weight_bonus_bps: u32,
    pub fee_discount_bps: u32,
    pub priority_support: bool,
}

#[derive(Clone, Debug)]
#[contracttype]
pub struct ReputationScore {
    pub player: Address,
    pub score: u32,
    pub total_reports: u32,
    pub positive_actions: u32,
    pub negative_actions: u32,
    pub last_updated: u64,
}

#[derive(Clone, Debug)]
#[contracttype]
pub struct BehaviorRecord {
    pub id: u64,
    pub player: Address,
    pub behavior_type: BehaviorType,
    pub description: String,
    pub points_change: i32,
    pub timestamp: u64,
    pub reporter: Address,
}

#[derive(Clone, Debug)]
#[contracttype]
pub struct DisputeReport {
    pub id: u64,
    pub reporter: Address,
    pub accused: Address,
    pub reason: String,
    pub evidence: String,
    pub status: ReportStatus,
    pub created_at: u64,
    pub resolved_at: u64,
}

pub fn source_points(source: ReputationSource) -> i32 {
    match source {
        ReputationSource::GameAchievement => 4,
        ReputationSource::CommunityContribution => 8,
        ReputationSource::EconomicActivity => 3,
        ReputationSource::GovernanceParticipation => 5,
        ReputationSource::SocialBehavior => 2,
    }
}

pub fn level_for_score(score: u32) -> ReputationLevel {
    match score {
        0..=20 => ReputationLevel::Newcomer,
        21..=40 => ReputationLevel::Scout,
        41..=65 => ReputationLevel::Contributor,
        66..=85 => ReputationLevel::Veteran,
        _ => ReputationLevel::Legend,
    }
}

pub fn benefits_for_score(score: u32) -> ReputationBenefit {
    match level_for_score(score) {
        ReputationLevel::Newcomer => ReputationBenefit {
            level: ReputationLevel::Newcomer,
            voting_weight_bonus_bps: 0,
            fee_discount_bps: 0,
            priority_support: false,
        },
        ReputationLevel::Scout => ReputationBenefit {
            level: ReputationLevel::Scout,
            voting_weight_bonus_bps: 100,
            fee_discount_bps: 25,
            priority_support: false,
        },
        ReputationLevel::Contributor => ReputationBenefit {
            level: ReputationLevel::Contributor,
            voting_weight_bonus_bps: 250,
            fee_discount_bps: 50,
            priority_support: true,
        },
        ReputationLevel::Veteran => ReputationBenefit {
            level: ReputationLevel::Veteran,
            voting_weight_bonus_bps: 500,
            fee_discount_bps: 100,
            priority_support: true,
        },
        ReputationLevel::Legend => ReputationBenefit {
            level: ReputationLevel::Legend,
            voting_weight_bonus_bps: 750,
            fee_discount_bps: 150,
            priority_support: true,
        },
    }
}

pub fn apply_monthly_decay(score: u32, inactive_months: u32) -> u32 {
    let mut decayed = score.max(MIN_REPUTATION).min(MAX_REPUTATION);
    for _ in 0..inactive_months {
        let loss = ((decayed as u64) * (REPUTATION_DECAY_BPS_PER_MONTH as u64) / 10_000) as u32;
        decayed = decayed.saturating_sub(loss.max(1)).max(MIN_REPUTATION);
    }
    decayed
}

pub fn initialize_reputation(env: &Env, admin: &Address) -> Result<(), ReputationError> {
    admin.require_auth();

    let mut admins: Vec<Address> = env
        .storage()
        .persistent()
        .get(&ReputationKey::AdminList)
        .unwrap_or_else(|| Vec::new(env));

    admins.push_back(admin.clone());
    env.storage()
        .persistent()
        .set(&ReputationKey::AdminList, &admins);

    env.events()
        .publish((symbol_short!("rep"), symbol_short!("init")), admin.clone());

    Ok(())
}

pub fn create_player_reputation(env: &Env, player: &Address) -> Result<(), ReputationError> {
    if env
        .storage()
        .persistent()
        .has(&ReputationKey::Score(player.clone()))
    {
        return Ok(());
    }

    let score = ReputationScore {
        player: player.clone(),
        score: INITIAL_REPUTATION,
        total_reports: 0,
        positive_actions: 0,
        negative_actions: 0,
        last_updated: env.ledger().timestamp(),
    };

    env.storage()
        .persistent()
        .set(&ReputationKey::Score(player.clone()), &score);

    env.events().publish(
        (symbol_short!("rep"), symbol_short!("create")),
        (player.clone(), INITIAL_REPUTATION),
    );

    Ok(())
}

pub fn get_reputation_score(env: &Env, player: &Address) -> Result<u32, ReputationError> {
    let score: ReputationScore = env
        .storage()
        .persistent()
        .get(&ReputationKey::Score(player.clone()))
        .ok_or(ReputationError::ReputationNotFound)?;

    Ok(score.score)
}

pub fn get_reputation_details(
    env: &Env,
    player: &Address,
) -> Result<ReputationScore, ReputationError> {
    env.storage()
        .persistent()
        .get(&ReputationKey::Score(player.clone()))
        .ok_or(ReputationError::ReputationNotFound)
}

pub fn record_behavior(
    env: &Env,
    player: &Address,
    behavior_type: BehaviorType,
    description: String,
    points: i32,
    reporter: Address,
) -> Result<(), ReputationError> {
    reporter.require_auth();

    if points.abs() > 20 {
        return Err(ReputationError::InvalidBehavior);
    }

    let mut score: ReputationScore = env
        .storage()
        .persistent()
        .get(&ReputationKey::Score(player.clone()))
        .ok_or(ReputationError::ReputationNotFound)?;

    let new_score = if points > 0 {
        ((score.score as i32) + points).max(MIN_REPUTATION as i32) as u32
    } else {
        ((score.score as i32) + points)
            .min(MAX_REPUTATION as i32)
            .max(MIN_REPUTATION as i32) as u32
    };

    if new_score > MAX_REPUTATION || new_score < MIN_REPUTATION {
        return Err(ReputationError::InvalidScore);
    }

    score.score = new_score;
    score.last_updated = env.ledger().timestamp();

    match behavior_type {
        BehaviorType::Positive => score.positive_actions += 1,
        BehaviorType::Negative => score.negative_actions += 1,
        BehaviorType::Neutral => {}
    }

    let record = BehaviorRecord {
        id: env.ledger().sequence().into(),
        player: player.clone(),
        behavior_type,
        description: description.clone(),
        points_change: points,
        timestamp: env.ledger().timestamp(),
        reporter: reporter.clone(),
    };

    let mut history: Vec<BehaviorRecord> = env
        .storage()
        .persistent()
        .get(&ReputationKey::ReputationHistory(player.clone()))
        .unwrap_or_else(|| Vec::new(env));

    history.push_back(record);
    if history.len() > 100 {
        history.pop_front();
    }

    env.storage()
        .persistent()
        .set(&ReputationKey::Score(player.clone()), &score);
    env.storage()
        .persistent()
        .set(&ReputationKey::ReputationHistory(player.clone()), &history);

    env.events().publish(
        (symbol_short!("rep"), symbol_short!("update")),
        (player.clone(), score.score, behavior_type),
    );

    Ok(())
}

pub fn submit_report(
    env: &Env,
    reporter: &Address,
    accused: &Address,
    reason: String,
    evidence: String,
) -> Result<u64, ReputationError> {
    reporter.require_auth();

    let mut disputes: Vec<DisputeReport> = env
        .storage()
        .persistent()
        .get(&ReputationKey::DisputeList)
        .unwrap_or_else(|| Vec::new(env));

    if disputes.len() >= MAX_ACTIVE_REPORTS {
        return Err(ReputationError::MaxReportsExceeded);
    }

    let report_id = env.ledger().sequence();
    let report = DisputeReport {
        id: report_id.into(),
        reporter: reporter.clone(),
        accused: accused.clone(),
        reason: reason.clone(),
        evidence: evidence.clone(),
        status: ReportStatus::Pending,
        created_at: env.ledger().timestamp(),
        resolved_at: 0,
    };

    disputes.push_back(report);
    env.storage()
        .persistent()
        .set(&ReputationKey::DisputeList, &disputes);

    let mut report_count: u32 = env
        .storage()
        .persistent()
        .get(&ReputationKey::ReportCount(accused.clone()))
        .unwrap_or(0);

    report_count += 1;
    env.storage()
        .persistent()
        .set(&ReputationKey::ReportCount(accused.clone()), &report_count);

    env.events().publish(
        (symbol_short!("rep"), symbol_short!("report")),
        (reporter.clone(), accused.clone(), report_id),
    );

    Ok(report_id.into())
}

pub fn resolve_report(
    env: &Env,
    admin: &Address,
    report_id: u64,
    resolved: bool,
) -> Result<(), ReputationError> {
    admin.require_auth();

    let admins: Vec<Address> = env
        .storage()
        .persistent()
        .get(&ReputationKey::AdminList)
        .ok_or(ReputationError::Unauthorized)?;

    if !admins.iter().any(|a| a == *admin) {
        return Err(ReputationError::Unauthorized);
    }

    let disputes: Vec<DisputeReport> = env
        .storage()
        .persistent()
        .get(&ReputationKey::DisputeList)
        .ok_or(ReputationError::ReportNotFound)?;

    for mut report in disputes.iter() {
        if report.id == report_id {
            report.status = if resolved {
                ReportStatus::Resolved
            } else {
                ReportStatus::Dismissed
            };
            report.resolved_at = env.ledger().timestamp();

            if resolved {
                let _ = record_behavior(
                    env,
                    &report.accused,
                    BehaviorType::Negative,
                    String::from_str(env, "Report resolved with sanctions"),
                    -5,
                    admin.clone(),
                );
            }

            env.storage()
                .persistent()
                .set(&ReputationKey::DisputeList, &disputes);

            env.events().publish(
                (symbol_short!("rep"), symbol_short!("resolve")),
                (report_id, resolved),
            );

            return Ok(());
        }
    }

    Err(ReputationError::ReportNotFound)
}

pub fn ban_player(env: &Env, admin: &Address, player: &Address) -> Result<(), ReputationError> {
    admin.require_auth();

    let admins: Vec<Address> = env
        .storage()
        .persistent()
        .get(&ReputationKey::AdminList)
        .ok_or(ReputationError::Unauthorized)?;

    if !admins.iter().any(|a| a == *admin) {
        return Err(ReputationError::Unauthorized);
    }

    if env
        .storage()
        .persistent()
        .has(&ReputationKey::BanList(player.clone()))
    {
        return Err(ReputationError::AlreadyBanned);
    }

    env.storage()
        .persistent()
        .set(&ReputationKey::BanList(player.clone()), &true);

    let mut score: ReputationScore = env
        .storage()
        .persistent()
        .get(&ReputationKey::Score(player.clone()))
        .ok_or(ReputationError::ReputationNotFound)?;

    score.score = MIN_REPUTATION;
    env.storage()
        .persistent()
        .set(&ReputationKey::Score(player.clone()), &score);

    env.events()
        .publish((symbol_short!("rep"), symbol_short!("ban")), player.clone());

    Ok(())
}

pub fn is_player_banned(env: &Env, player: &Address) -> bool {
    env.storage()
        .persistent()
        .get(&ReputationKey::BanList(player.clone()))
        .unwrap_or(false)
}

pub fn get_player_history(env: &Env, player: &Address) -> Vec<BehaviorRecord> {
    env.storage()
        .persistent()
        .get(&ReputationKey::ReputationHistory(player.clone()))
        .unwrap_or_else(|| Vec::new(env))
}

pub fn get_player_report_count(env: &Env, player: &Address) -> u32 {
    env.storage()
        .persistent()
        .get(&ReputationKey::ReportCount(player.clone()))
        .unwrap_or(0)
}

pub fn get_all_reports(env: &Env) -> Vec<DisputeReport> {
    env.storage()
        .persistent()
        .get(&ReputationKey::DisputeList)
        .unwrap_or_else(|| Vec::new(env))
}

pub fn claim_reputation_reward(env: &Env, player: &Address) -> Result<i128, ReputationError> {
    player.require_auth();

    let score: ReputationScore = env
        .storage()
        .persistent()
        .get(&ReputationKey::Score(player.clone()))
        .ok_or(ReputationError::ReputationNotFound)?;

    let reward = (score.score as i128) * 100; // 1 XLM per reputation point

    env.events().publish(
        (symbol_short!("rep"), symbol_short!("reward")),
        (player.clone(), reward),
    );

    Ok(reward)
}

#[cfg(test)]
mod tests {
    use super::*;
    use soroban_sdk::testutils::Address as _;

    // // #[test]
    fn test_reputation_initialization() {
        let env = Env::default();
        let admin = Address::generate(&env);

        assert!(initialize_reputation(&env, &admin).is_ok());
    }

    // // #[test]
    fn test_player_reputation_creation() {
        let env = Env::default();
        let admin = Address::generate(&env);
        let player = Address::generate(&env);

        let _ = initialize_reputation(&env, &admin);
        assert!(create_player_reputation(&env, &player).is_ok());

        let score = get_reputation_score(&env, &player);
        assert_eq!(score.unwrap(), INITIAL_REPUTATION);
    }

    // // #[test]
    fn test_behavior_recording() {
        let env = Env::default();
        let admin = Address::generate(&env);
        let player = Address::generate(&env);
        let reporter = Address::generate(&env);

        let _ = initialize_reputation(&env, &admin);
        let _ = create_player_reputation(&env, &player);

        let description = String::from_str(&env, "Test behavior");
        let result = record_behavior(
            &env,
            &player,
            BehaviorType::Positive,
            description,
            5,
            reporter,
        );

        assert!(result.is_ok());
    }

    // // #[test]
    fn test_ban_player() {
        let env = Env::default();
        let admin = Address::generate(&env);
        let player = Address::generate(&env);

        let _ = initialize_reputation(&env, &admin);
        let _ = create_player_reputation(&env, &player);

        assert!(ban_player(&env, &admin, &player).is_ok());
        assert!(is_player_banned(&env, &player));
    }

    #[test]
    fn test_reputation_sources_levels_and_decay() {
        assert_eq!(source_points(ReputationSource::CommunityContribution), 8);
        assert_eq!(level_for_score(18), ReputationLevel::Newcomer);
        assert_eq!(level_for_score(50), ReputationLevel::Contributor);
        assert_eq!(level_for_score(90), ReputationLevel::Legend);

        let benefit = benefits_for_score(90);
        assert_eq!(benefit.level, ReputationLevel::Legend);
        assert!(benefit.priority_support);
        assert!(benefit.voting_weight_bonus_bps > 0);

        assert_eq!(apply_monthly_decay(100, 1), 95);
        assert!(apply_monthly_decay(2, 12) >= MIN_REPUTATION);
    }
}
