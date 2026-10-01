//! Security bounty approval and payout accounting.
//!
use soroban_sdk::{
    contracterror, contracttype, symbol_short, Address, BytesN, Env, Map, String, Symbol, Vec,
};

const MAX_BURST_REPORTS: u32 = 10;
pub const DISCLOSURE_EMBARGO_SECONDS: u64 = 90 * 24 * 60 * 60;
pub const LOW_REWARD: i128 = 100;
pub const MEDIUM_REWARD: i128 = 500;
pub const HIGH_REWARD: i128 = 2_000;
pub const CRITICAL_REWARD: i128 = 5_000;

#[derive(Clone)]
#[contracttype]
pub struct BountyConfig {
    pub admin: Address,
    pub approval_threshold: u32,
    pub high_value_threshold: i128,
    pub timelock_seconds: u64,
}

#[derive(Clone)]
#[contracttype]
pub struct BugReport {
    pub id: u64,
    pub reporter: Address,
    pub description: String,
    pub severity: Symbol,
    pub fingerprint: BytesN<32>,
    pub submitted_at: u64,
    pub disclosure_at: u64,
    pub default_reward: i128,
    pub status: ReviewStatus,
    pub paid: bool,
    pub payout_amount: i128,
}

#[derive(Clone, Copy, Eq, PartialEq)]
#[repr(u32)]
#[contracttype]
pub enum ReviewStatus {
    Submitted = 0,
    Duplicate = 1,
    Accepted = 2,
    Rejected = 3,
    Paid = 4,
}

#[derive(Copy, Clone, Eq, PartialEq)]
#[repr(u32)]
#[contracterror]
pub enum BountyError {
    AlreadyInitialized = 1,
    InvalidSeverity = 2,
    Unauthorized = 3,
    ReportNotFound = 4,
    ReportAlreadyPaid = 5,
    DuplicateApproval = 6,
    InvalidAmount = 7,
    TimelockActive = 8,
    EmergencyPaused = 9,
    ApprovalThresholdInvalid = 10,
    TooManyReports = 11,
    DuplicateReport = 12,
}

fn cfg_key() -> Symbol {
    symbol_short!("b_cfg")
}

fn pool_key() -> Symbol {
    symbol_short!("b_pool")
}

fn id_key() -> Symbol {
    symbol_short!("b_id")
}

fn pause_key() -> Symbol {
    symbol_short!("b_pause")
}

fn community_mode_key() -> Symbol {
    symbol_short!("b_vote")
}

fn tiers_key() -> Symbol {
    symbol_short!("b_tiers")
}

fn report_key(id: u64) -> (Symbol, u64) {
    (symbol_short!("b_rpt"), id)
}

fn approver_key(addr: &Address) -> (Symbol, Address) {
    (symbol_short!("b_apr"), addr.clone())
}

fn approved_key(id: u64, addr: &Address) -> (Symbol, u64, Address) {
    (symbol_short!("b_ok"), id, addr.clone())
}

fn approval_count_key(id: u64) -> (Symbol, u64) {
    (symbol_short!("b_cnt"), id)
}

fn unlock_key(id: u64) -> (Symbol, u64) {
    (symbol_short!("b_ulck"), id)
}

fn balance_key(addr: &Address) -> (Symbol, Address) {
    (symbol_short!("b_bal"), addr.clone())
}

fn fingerprint_key(fingerprint: &BytesN<32>) -> (Symbol, BytesN<32>) {
    (symbol_short!("b_dup"), fingerprint.clone())
}

fn get_tiers(env: &Env) -> Map<Symbol, i128> {
    env.storage()
        .persistent()
        .get(&tiers_key())
        .unwrap_or_else(|| {
            let mut tiers = Map::new(env);
            tiers.set(symbol_short!("low"), LOW_REWARD);
            tiers.set(symbol_short!("medium"), MEDIUM_REWARD);
            tiers.set(symbol_short!("high"), HIGH_REWARD);
            tiers.set(symbol_short!("critical"), CRITICAL_REWARD);
            tiers
        })
}

fn is_approver(env: &Env, addr: &Address) -> bool {
    env.storage()
        .persistent()
        .get::<_, bool>(&approver_key(addr))
        .unwrap_or(false)
}

pub fn init_bounty_engine(
    env: &Env,
    admin: &Address,
    approvers: Vec<Address>,
    approval_threshold: u32,
    high_value_threshold: i128,
    timelock_seconds: u64,
) -> Result<(), BountyError> {
    admin.require_auth();

    if env.storage().persistent().has(&cfg_key()) {
        return Err(BountyError::AlreadyInitialized);
    }

    if approval_threshold == 0 {
        return Err(BountyError::ApprovalThresholdInvalid);
    }

    let config = BountyConfig {
        admin: admin.clone(),
        approval_threshold,
        high_value_threshold,
        timelock_seconds,
    };

    env.storage().persistent().set(&cfg_key(), &config);
    env.storage().persistent().set(&pool_key(), &0i128);
    env.storage().persistent().set(&id_key(), &0u64);
    env.storage().persistent().set(&pause_key(), &false);
    env.storage()
        .persistent()
        .set(&community_mode_key(), &false);

    let tiers = get_tiers(env);
    env.storage().persistent().set(&tiers_key(), &tiers);

    for i in 0..approvers.len() {
        if let Some(approver) = approvers.get(i) {
            env.storage()
                .persistent()
                .set(&approver_key(&approver), &true);
        }
    }
    env.storage().persistent().set(&approver_key(admin), &true);

    Ok(())
}

pub fn fund_bounty_pool(env: &Env, admin: &Address, amount: i128) -> Result<i128, BountyError> {
    admin.require_auth();
    if amount <= 0 {
        return Err(BountyError::InvalidAmount);
    }

    let config = env
        .storage()
        .persistent()
        .get::<_, BountyConfig>(&cfg_key())
        .ok_or(BountyError::Unauthorized)?;

    if config.admin != *admin {
        return Err(BountyError::Unauthorized);
    }

    let current = env
        .storage()
        .persistent()
        .get::<_, i128>(&pool_key())
        .unwrap_or(0);
    let updated = current + amount;
    env.storage().persistent().set(&pool_key(), &updated);
    Ok(updated)
}

pub fn submit_bug_report(
    env: &Env,
    reporter: &Address,
    description: String,
    severity: Symbol,
) -> Result<u64, BountyError> {
    let fingerprint = BytesN::from_array(env, &[0; 32]);
    submit_bug_report_internal(env, reporter, description, severity, fingerprint, false)
}

pub fn submit_bug_report_with_fingerprint(
    env: &Env,
    reporter: &Address,
    description: String,
    severity: Symbol,
    fingerprint: BytesN<32>,
) -> Result<u64, BountyError> {
    submit_bug_report_internal(env, reporter, description, severity, fingerprint, true)
}

fn submit_bug_report_internal(
    env: &Env,
    reporter: &Address,
    description: String,
    severity: Symbol,
    fingerprint: BytesN<32>,
    reject_duplicate: bool,
) -> Result<u64, BountyError> {
    reporter.require_auth();

    let tiers = get_tiers(env);
    let default_reward = tiers
        .get(severity.clone())
        .ok_or(BountyError::InvalidSeverity)?;

    let duplicate_key = fingerprint_key(&fingerprint);
    if reject_duplicate && env.storage().persistent().has(&duplicate_key) {
        return Err(BountyError::DuplicateReport);
    }

    let next_id = env
        .storage()
        .persistent()
        .get::<_, u64>(&id_key())
        .unwrap_or(0)
        + 1;
    env.storage().persistent().set(&id_key(), &next_id);

    let report = BugReport {
        id: next_id,
        reporter: reporter.clone(),
        description,
        severity,
        fingerprint: fingerprint.clone(),
        submitted_at: env.ledger().timestamp(),
        disclosure_at: env.ledger().timestamp() + DISCLOSURE_EMBARGO_SECONDS,
        default_reward,
        status: ReviewStatus::Submitted,
        paid: false,
        payout_amount: 0,
    };

    env.storage()
        .persistent()
        .set(&report_key(next_id), &report);
    if reject_duplicate {
        env.storage().persistent().set(&duplicate_key, &next_id);
    }
    Ok(next_id)
}

pub fn approve_and_pay_bounty(
    env: &Env,
    approver: &Address,
    report_id: u64,
    amount: i128,
) -> Result<bool, BountyError> {
    approver.require_auth();

    let paused = env
        .storage()
        .persistent()
        .get::<_, bool>(&pause_key())
        .unwrap_or(false);
    if paused {
        return Err(BountyError::EmergencyPaused);
    }

    if !is_approver(env, approver) {
        return Err(BountyError::Unauthorized);
    }

    if amount <= 0 {
        return Err(BountyError::InvalidAmount);
    }

    let config = env
        .storage()
        .persistent()
        .get::<_, BountyConfig>(&cfg_key())
        .ok_or(BountyError::Unauthorized)?;

    let mut report = env
        .storage()
        .persistent()
        .get::<_, BugReport>(&report_key(report_id))
        .ok_or(BountyError::ReportNotFound)?;

    if report.paid {
        return Err(BountyError::ReportAlreadyPaid);
    }

    let a_key = approved_key(report_id, approver);
    if env.storage().persistent().has(&a_key) {
        return Err(BountyError::DuplicateApproval);
    }

    env.storage().persistent().set(&a_key, &true);

    let c_key = approval_count_key(report_id);
    let count = env
        .storage()
        .persistent()
        .get::<_, u32>(&c_key)
        .unwrap_or(0)
        + 1;
    env.storage().persistent().set(&c_key, &count);

    if count < config.approval_threshold {
        return Ok(false);
    }

    if amount >= config.high_value_threshold {
        let u_key = unlock_key(report_id);
        let now = env.ledger().timestamp();
        let unlock_at = env
            .storage()
            .persistent()
            .get::<_, u64>(&u_key)
            .unwrap_or(0);
        if unlock_at == 0 {
            env.storage()
                .persistent()
                .set(&u_key, &(now + config.timelock_seconds));
            return Err(BountyError::TimelockActive);
        }
        if now < unlock_at {
            return Err(BountyError::TimelockActive);
        }
    }

    let pool = env
        .storage()
        .persistent()
        .get::<_, i128>(&pool_key())
        .unwrap_or(0);
    if pool < amount {
        return Err(BountyError::InvalidAmount);
    }

    let reporter_balance_key = balance_key(&report.reporter);
    let reporter_balance = env
        .storage()
        .persistent()
        .get::<_, i128>(&reporter_balance_key)
        .unwrap_or(0);

    env.storage()
        .persistent()
        .set(&pool_key(), &(pool - amount));
    env.storage()
        .persistent()
        .set(&reporter_balance_key, &(reporter_balance + amount));

    report.paid = true;
    report.payout_amount = amount;
    report.status = ReviewStatus::Paid;
    env.storage()
        .persistent()
        .set(&report_key(report_id), &report);

    env.events().publish(
        (symbol_short!("bounty"), symbol_short!("paid")),
        (report_id, report.reporter, amount),
    );

    Ok(true)
}

pub fn approve_and_pay_bounty_burst(
    env: &Env,
    approver: &Address,
    report_ids: Vec<u64>,
    amounts: Vec<i128>,
) -> Result<u32, BountyError> {
    if report_ids.len() != amounts.len() {
        return Err(BountyError::InvalidAmount);
    }

    if report_ids.len() > MAX_BURST_REPORTS {
        return Err(BountyError::TooManyReports);
    }

    let mut paid_count: u32 = 0;
    for i in 0..report_ids.len() {
        let report_id = report_ids.get(i).ok_or(BountyError::ReportNotFound)?;
        let amount = amounts.get(i).ok_or(BountyError::InvalidAmount)?;
        if approve_and_pay_bounty(env, approver, report_id, amount).unwrap_or(false) {
            paid_count += 1;
        }
    }

    Ok(paid_count)
}

pub fn set_emergency_pause(env: &Env, admin: &Address, paused: bool) -> Result<(), BountyError> {
    admin.require_auth();
    let config = env
        .storage()
        .persistent()
        .get::<_, BountyConfig>(&cfg_key())
        .ok_or(BountyError::Unauthorized)?;

    if config.admin != *admin {
        return Err(BountyError::Unauthorized);
    }

    env.storage().persistent().set(&pause_key(), &paused);
    Ok(())
}

pub fn set_community_voted_mode(
    env: &Env,
    admin: &Address,
    enabled: bool,
) -> Result<(), BountyError> {
    admin.require_auth();
    let config = env
        .storage()
        .persistent()
        .get::<_, BountyConfig>(&cfg_key())
        .ok_or(BountyError::Unauthorized)?;

    if config.admin != *admin {
        return Err(BountyError::Unauthorized);
    }

    env.storage()
        .persistent()
        .set(&community_mode_key(), &enabled);
    Ok(())
}

pub fn get_report(env: &Env, report_id: u64) -> Option<BugReport> {
    env.storage().persistent().get(&report_key(report_id))
}

pub fn get_bounty_balance(env: &Env, reporter: &Address) -> i128 {
    env.storage()
        .persistent()
        .get::<_, i128>(&balance_key(reporter))
        .unwrap_or(0)
}

pub fn get_bounty_pool(env: &Env) -> i128 {
    env.storage()
        .persistent()
        .get::<_, i128>(&pool_key())
        .unwrap_or(0)
}

pub fn severity_reward(env: &Env, severity: Symbol) -> Result<i128, BountyError> {
    get_tiers(env)
        .get(severity)
        .ok_or(BountyError::InvalidSeverity)
}

pub fn public_disclosure_ready(env: &Env, report_id: u64) -> Result<bool, BountyError> {
    let report = get_report(env, report_id).ok_or(BountyError::ReportNotFound)?;
    Ok(env.ledger().timestamp() >= report.disclosure_at)
}

#[cfg(test)]
mod tests {
    use super::*;
    use soroban_sdk::testutils::{Address as _, Ledger, LedgerInfo};

    fn env_at(timestamp: u64) -> Env {
        let env = Env::default();
        env.mock_all_auths();
        env.ledger().set(LedgerInfo {
            protocol_version: 22,
            sequence_number: 1,
            timestamp,
            network_id: [0; 32],
            base_reserve: 10,
            min_temp_entry_ttl: 100,
            min_persistent_entry_ttl: 1_000,
            max_entry_ttl: 10_000,
        });
        env
    }

    #[test]
    fn default_severity_rewards_match_public_program() {
        let env = Env::default();
        assert_eq!(
            severity_reward(&env, symbol_short!("low")).unwrap(),
            LOW_REWARD
        );
        assert_eq!(
            severity_reward(&env, symbol_short!("medium")).unwrap(),
            MEDIUM_REWARD
        );
        assert_eq!(
            severity_reward(&env, symbol_short!("high")).unwrap(),
            HIGH_REWARD
        );
        assert_eq!(
            severity_reward(&env, symbol_short!("critical")).unwrap(),
            CRITICAL_REWARD
        );
    }

    #[test]
    fn duplicate_fingerprints_are_rejected_and_embargo_is_tracked() {
        let env = env_at(1_700_000_000);
        let admin = Address::generate(&env);
        let reporter = Address::generate(&env);
        let approvers = Vec::new(&env);
        init_bounty_engine(&env, &admin, approvers, 1, 5_000, 60).unwrap();

        let fingerprint = BytesN::from_array(&env, &[7; 32]);
        let id = submit_bug_report_with_fingerprint(
            &env,
            &reporter,
            String::from_str(&env, "overflow in payout accounting"),
            symbol_short!("high"),
            fingerprint.clone(),
        )
        .unwrap();

        let duplicate = submit_bug_report_with_fingerprint(
            &env,
            &reporter,
            String::from_str(&env, "same issue with different wording"),
            symbol_short!("high"),
            fingerprint,
        );
        assert!(matches!(duplicate, Err(BountyError::DuplicateReport)));
        assert!(!public_disclosure_ready(&env, id).unwrap());
    }
}
