//! Security bounty approval, severity-based reward accounting, duplicate detection, review workflow, embargo tracking, and governance budget management.

use soroban_sdk::{
    contracterror, contracttype, symbol_short, Address, Bytes, BytesN, Env, Map, String, Symbol,
    Vec,
};

pub const MAX_BURST_REPORTS: u32 = 10;
pub const EMBARGO_PERIOD_SECONDS: u64 = 7_776_000;

/// Represents lifecycle status of a submitted bug report.
#[derive(Copy, Clone, Eq, PartialEq, Debug)]
#[repr(u32)]
#[contracttype]
pub enum BugReportStatus {
    Pending = 0,
    Approved = 1,
    Rejected = 2,
    Disclosed = 3,
}

pub type ReportStatus = BugReportStatus;

/// Global configuration for the security bounty engine.
#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct BountyConfig {
    pub admin: Address,
    pub approval_threshold: u32,
    pub high_value_threshold: i128,
    pub timelock_seconds: u64,
}

/// Researcher statistics for the Hall of Fame.
#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct ResearcherStats {
    pub total_bounties: i128,
    pub bugs_resolved: u32,
}

/// Record of a single submitted vulnerability report.
#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct BugReport {
    pub id: u64,
    pub reporter: Address,
    pub description: String,
    pub severity: Symbol,
    pub submitted_at: u64,
    pub default_reward: i128,
    pub paid: bool,
    pub payout_amount: i128,
    pub status: ReportStatus,
    pub feedback: String,
    pub embargo_until: u64,
}

/// Error codes returned by the bounty management engine.
#[derive(Copy, Clone, Eq, PartialEq, Debug)]
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
    EmbargoActive = 13,
    AlreadyDisclosed = 14,
    ReportAlreadyRejected = 15,
}

pub type BugBountyError = BountyError;

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

fn dao_key() -> Symbol {
    symbol_short!("b_dao")
}

fn total_paid_key() -> Symbol {
    symbol_short!("b_totpd")
}

fn hall_of_fame_list_key() -> Symbol {
    symbol_short!("b_hoflst")
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

fn duplicate_hash_key(hash: &BytesN<32>) -> (Symbol, BytesN<32>) {
    (symbol_short!("b_dhash"), hash.clone())
}

fn researcher_stats_key(researcher: &Address) -> (Symbol, Address) {
    (symbol_short!("b_hof"), researcher.clone())
}

fn get_tiers(env: &Env) -> Map<Symbol, i128> {
    env.storage()
        .persistent()
        .get(&tiers_key())
        .unwrap_or_else(|| {
            let mut tiers = Map::new(env);
            tiers.set(symbol_short!("low"), 100);
            tiers.set(symbol_short!("medium"), 500);
            tiers.set(symbol_short!("high"), 2_000);
            tiers.set(symbol_short!("critical"), 5_000);
            tiers
        })
}

fn is_approver(env: &Env, addr: &Address) -> bool {
    env.storage()
        .persistent()
        .get::<_, bool>(&approver_key(addr))
        .unwrap_or(false)
}

/// Initialize bounty engine configuration and initial approver whitelist.
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
    env.storage().persistent().set(&total_paid_key(), &0i128);

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

/// Deposit funds into the bounty reward pool.
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
    let updated = current.saturating_add(amount);
    env.storage().persistent().set(&pool_key(), &updated);
    Ok(updated)
}

/// Submit a new bug report with duplicate detection and 90-day embargo calculation.
pub fn submit_bug_report(
    env: &Env,
    reporter: &Address,
    description: String,
    severity: Symbol,
) -> Result<u64, BountyError> {
    reporter.require_auth();

    let paused = env
        .storage()
        .persistent()
        .get::<_, bool>(&pause_key())
        .unwrap_or(false);
    if paused {
        return Err(BountyError::EmergencyPaused);
    }

    let tiers = get_tiers(env);
    let default_reward = tiers
        .get(severity.clone())
        .ok_or(BountyError::InvalidSeverity)?;

    let desc_bytes = Bytes::from(&description);
    let content_hash: BytesN<32> = env.crypto().sha256(&desc_bytes).into();
    let dup_key = duplicate_hash_key(&content_hash);
    if env.storage().persistent().has(&dup_key) {
        return Err(BountyError::DuplicateReport);
    }

    let next_id = env
        .storage()
        .persistent()
        .get::<_, u64>(&id_key())
        .unwrap_or(0)
        + 1;
    env.storage().persistent().set(&id_key(), &next_id);
    env.storage().persistent().set(&dup_key, &next_id);

    let now = env.ledger().timestamp();
    let embargo_until = now.saturating_add(EMBARGO_PERIOD_SECONDS);

    let report = BugReport {
        id: next_id,
        reporter: reporter.clone(),
        description,
        severity: severity.clone(),
        submitted_at: now,
        default_reward,
        paid: false,
        payout_amount: 0,
        status: ReportStatus::Pending,
        feedback: String::from_str(env, ""),
        embargo_until,
    };

    env.storage()
        .persistent()
        .set(&report_key(next_id), &report);

    env.events().publish(
        (symbol_short!("bounty"), symbol_short!("submit")),
        (next_id, reporter.clone(), severity),
    );

    Ok(next_id)
}

/// Approve a bug report and execute payout if the threshold is met.
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
    if report.status == ReportStatus::Rejected {
        return Err(BountyError::ReportAlreadyRejected);
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

    if amount >= config.high_value_threshold && config.timelock_seconds > 0 {
        let now = env.ledger().timestamp();
        let unlock_at = report.submitted_at.saturating_add(config.timelock_seconds);
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
        .set(&reporter_balance_key, &(reporter_balance.saturating_add(amount)));

    report.paid = true;
    report.payout_amount = amount;
    report.status = ReportStatus::Approved;
    env.storage()
        .persistent()
        .set(&report_key(report_id), &report);

    let current_total_paid = env
        .storage()
        .persistent()
        .get::<_, i128>(&total_paid_key())
        .unwrap_or(0);
    env.storage()
        .persistent()
        .set(&total_paid_key(), &(current_total_paid.saturating_add(amount)));

    let r_key = researcher_stats_key(&report.reporter);
    let mut stats = env
        .storage()
        .persistent()
        .get::<_, ResearcherStats>(&r_key)
        .unwrap_or(ResearcherStats {
            total_bounties: 0,
            bugs_resolved: 0,
        });
    stats.total_bounties = stats.total_bounties.saturating_add(amount);
    stats.bugs_resolved = stats.bugs_resolved.saturating_add(1);
    env.storage().persistent().set(&r_key, &stats);

    let hof_key = hall_of_fame_list_key();
    let mut hof_list: Vec<Address> = env
        .storage()
        .persistent()
        .get(&hof_key)
        .unwrap_or_else(|| Vec::new(env));
    let mut exists = false;
    for i in 0..hof_list.len() {
        if let Some(existing) = hof_list.get(i) {
            if existing == report.reporter {
                exists = true;
                break;
            }
        }
    }
    if !exists {
        hof_list.push_back(report.reporter.clone());
        env.storage().persistent().set(&hof_key, &hof_list);
    }

    env.events().publish(
        (symbol_short!("bounty"), symbol_short!("paid")),
        (report_id, report.reporter, amount),
    );

    Ok(true)
}

/// Reject a bug report with feedback from an authorized reviewer.
pub fn reject_bug_report(
    env: &Env,
    reviewer: &Address,
    report_id: u64,
    feedback: String,
) -> Result<(), BountyError> {
    reviewer.require_auth();

    if !is_approver(env, reviewer) {
        return Err(BountyError::Unauthorized);
    }

    let mut report = env
        .storage()
        .persistent()
        .get::<_, BugReport>(&report_key(report_id))
        .ok_or(BountyError::ReportNotFound)?;

    if report.paid {
        return Err(BountyError::ReportAlreadyPaid);
    }
    if report.status == ReportStatus::Rejected {
        return Err(BountyError::ReportAlreadyRejected);
    }

    report.status = ReportStatus::Rejected;
    report.feedback = feedback.clone();
    env.storage()
        .persistent()
        .set(&report_key(report_id), &report);

    env.events().publish(
        (symbol_short!("bounty"), symbol_short!("reject")),
        (report_id, reviewer.clone()),
    );

    Ok(())
}

/// Reclassify a bug report's severity tier and default reward.
pub fn reclassify_severity(
    env: &Env,
    reviewer: &Address,
    report_id: u64,
    new_severity: Symbol,
) -> Result<(), BountyError> {
    reviewer.require_auth();

    if !is_approver(env, reviewer) {
        return Err(BountyError::Unauthorized);
    }

    let tiers = get_tiers(env);
    let new_reward = tiers
        .get(new_severity.clone())
        .ok_or(BountyError::InvalidSeverity)?;

    let mut report = env
        .storage()
        .persistent()
        .get::<_, BugReport>(&report_key(report_id))
        .ok_or(BountyError::ReportNotFound)?;

    if report.paid {
        return Err(BountyError::ReportAlreadyPaid);
    }
    if report.status == ReportStatus::Rejected {
        return Err(BountyError::ReportAlreadyRejected);
    }

    report.severity = new_severity.clone();
    report.default_reward = new_reward;
    env.storage()
        .persistent()
        .set(&report_key(report_id), &report);

    env.events().publish(
        (symbol_short!("bounty"), symbol_short!("reclass")),
        (report_id, new_severity, new_reward),
    );

    Ok(())
}

/// Approve and pay out a batch of reports up to the burst limit.
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

/// Configure the reward amount for a severity level.
pub fn set_reward_tier(
    env: &Env,
    admin: &Address,
    severity: Symbol,
    amount: i128,
) -> Result<(), BountyError> {
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

    let mut tiers = get_tiers(env);
    tiers.set(severity, amount);
    env.storage().persistent().set(&tiers_key(), &tiers);
    Ok(())
}

/// Retrieve the configured reward amount for a severity level.
pub fn get_reward_tier(env: &Env, severity: Symbol) -> Option<i128> {
    let tiers = get_tiers(env);
    tiers.get(severity)
}

/// Check if a report is eligible for public disclosure.
pub fn can_disclose_report(env: &Env, report_id: u64) -> Result<bool, BountyError> {
    let report = env
        .storage()
        .persistent()
        .get::<_, BugReport>(&report_key(report_id))
        .ok_or(BountyError::ReportNotFound)?;
    let now = env.ledger().timestamp();
    Ok(now >= report.embargo_until || report.status == ReportStatus::Disclosed)
}

/// Mark a report as publicly disclosed after embargo expiry or by admin.
pub fn disclose_bug_report(
    env: &Env,
    caller: &Address,
    report_id: u64,
) -> Result<(), BountyError> {
    caller.require_auth();

    let mut report = env
        .storage()
        .persistent()
        .get::<_, BugReport>(&report_key(report_id))
        .ok_or(BountyError::ReportNotFound)?;

    if report.status == ReportStatus::Disclosed {
        return Err(BountyError::AlreadyDisclosed);
    }

    let config = env
        .storage()
        .persistent()
        .get::<_, BountyConfig>(&cfg_key())
        .ok_or(BountyError::Unauthorized)?;

    let is_admin = *caller == config.admin;
    let is_reporter = *caller == report.reporter;

    if !is_admin && !is_reporter {
        return Err(BountyError::Unauthorized);
    }

    let now = env.ledger().timestamp();
    if !is_admin && now < report.embargo_until {
        return Err(BountyError::EmbargoActive);
    }

    report.status = ReportStatus::Disclosed;
    env.storage()
        .persistent()
        .set(&report_key(report_id), &report);

    env.events().publish(
        (symbol_short!("bounty"), symbol_short!("disclose")),
        (report_id, caller.clone()),
    );

    Ok(())
}

/// Retrieve total bounties paid out across the system.
pub fn get_total_bounties_paid(env: &Env) -> i128 {
    env.storage()
        .persistent()
        .get::<_, i128>(&total_paid_key())
        .unwrap_or(0)
}

/// Retrieve statistics for an individual security researcher.
pub fn get_researcher_stats(env: &Env, researcher: &Address) -> ResearcherStats {
    env.storage()
        .persistent()
        .get::<_, ResearcherStats>(&researcher_stats_key(researcher))
        .unwrap_or(ResearcherStats {
            total_bounties: 0,
            bugs_resolved: 0,
        })
}

/// Retrieve the list of researchers with approved bounties.
pub fn get_hall_of_fame(env: &Env) -> Vec<Address> {
    env.storage()
        .persistent()
        .get::<_, Vec<Address>>(&hall_of_fame_list_key())
        .unwrap_or_else(|| Vec::new(env))
}

/// Allocate bounty funding from governance or contract administrator.
pub fn allocate_bounty_gov_budget(
    env: &Env,
    governance_caller: &Address,
    amount: i128,
) -> Result<i128, BountyError> {
    governance_caller.require_auth();
    if amount <= 0 {
        return Err(BountyError::InvalidAmount);
    }

    let config = env
        .storage()
        .persistent()
        .get::<_, BountyConfig>(&cfg_key())
        .ok_or(BountyError::Unauthorized)?;

    let dao_opt = env
        .storage()
        .persistent()
        .get::<_, Address>(&dao_key());

    let authorized = *governance_caller == config.admin
        || dao_opt.map(|dao| dao == *governance_caller).unwrap_or(false);

    if !authorized {
        return Err(BountyError::Unauthorized);
    }

    let current = env
        .storage()
        .persistent()
        .get::<_, i128>(&pool_key())
        .unwrap_or(0);
    let updated = current.saturating_add(amount);
    env.storage().persistent().set(&pool_key(), &updated);

    env.events().publish(
        (symbol_short!("bounty"), symbol_short!("gov_fund")),
        (governance_caller.clone(), amount, updated),
    );

    Ok(updated)
}

/// Configure the authorized governance or DAO contract address.
pub fn set_governance_contract(
    env: &Env,
    admin: &Address,
    dao: &Address,
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

    env.storage().persistent().set(&dao_key(), dao);
    Ok(())
}

/// Toggle emergency pause state for bug report submissions and payouts.
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

/// Toggle community-voted mode for bug report evaluation.
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

/// Retrieve a bug report by its identifier.
pub fn get_report(env: &Env, report_id: u64) -> Option<BugReport> {
    env.storage().persistent().get(&report_key(report_id))
}

/// Retrieve the accumulated approved bounty balance for a researcher.
pub fn get_bounty_balance(env: &Env, reporter: &Address) -> i128 {
    env.storage()
        .persistent()
        .get::<_, i128>(&balance_key(reporter))
        .unwrap_or(0)
}

/// Retrieve current available balance of the bounty reward pool.
pub fn get_bounty_pool(env: &Env) -> i128 {
    env.storage()
        .persistent()
        .get::<_, i128>(&pool_key())
        .unwrap_or(0)
}
