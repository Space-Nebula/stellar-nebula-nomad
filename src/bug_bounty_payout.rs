use soroban_sdk::{
    contracterror, contracttype, symbol_short, Address, BytesN, Env, String, Symbol, Vec, Map,
};

/// Default timelock duration for high-value bounties: 48 hours in seconds.
pub const DEFAULT_TIMELOCK_DURATION: u64 = 172_800;

/// Maximum reports processed per transaction (burst protection).
pub const MAX_REPORTS_PER_TX: u32 = 10;

/// Minimum bounty reward (to prevent spam).
pub const MIN_BOUNTY_REWARD: i128 = 100;

/// Maximum bounty reward (safety limit).
pub const MAX_BOUNTY_REWARD: i128 = 1_000_000_000;

/// High-value bounty threshold (requires timelock).
pub const HIGH_VALUE_THRESHOLD: i128 = 100_000;

/// Multi-sig approval threshold (number of approvals required).
pub const MULTI_SIG_THRESHOLD: u32 = 2;

/// ─── Storage Keys ─────────────────────────────────────────────────────────────

#[derive(Clone)]
#[contracttype]
pub enum BountyPayoutKey {
    /// Auto-incrementing report ID counter.
    ReportCounter,
    /// Bug report data keyed by report ID.
    Report(u64),
    /// Bounty pool balance.
    BountyPool,
    /// Admin addresses authorized to approve bounties.
    Admins,
    /// Multi-sig approvals: (report_id, admin_address) → bool.
    Approval(u64, Address),
    /// Timelocked payouts: report_id → unlock_timestamp.
    Timelock(u64),
    /// Severity reward tiers.
    RewardTiers,
    /// Global configuration.
    Config,
}

/// ─── Errors ─────────────────────────────────────────────────────────────────

#[contracterror]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum BountyPayoutError {
    /// Report does not exist.
    ReportNotFound = 1,
    /// Invalid severity level.
    InvalidSeverity = 2,
    /// Caller is not authorized (admin).
    NotAuthorized = 3,
    /// Bounty already paid.
    AlreadyPaid = 4,
    /// Report is still pending approval.
    PendingApproval = 5,
    /// Insufficient bounty pool balance.
    InsufficientPoolBalance = 6,
    /// Invalid reward amount.
    InvalidReward = 7,
    /// Timelock not yet expired.
    TimelockNotExpired = 8,
    /// Multi-sig threshold not met.
    InsufficientApprovals = 9,
    /// Maximum reports per transaction exceeded.
    TooManyReports = 10,
    /// Admin already approved.
    AlreadyApproved = 11,
}

/// ─── Data Types ─────────────────────────────────────────────────────────────

/// Severity levels for bug reports.
#[derive(Clone, Debug, PartialEq)]
#[contracttype]
pub enum Severity {
    Critical,
    High,
    Medium,
    Low,
}

/// Bug report status.
#[derive(Clone, Debug, PartialEq)]
#[contracttype]
pub enum ReportStatus {
    Submitted,
    Approved,
    Paid,
    Rejected,
}

/// Bug report structure.
#[derive(Clone, Debug, PartialEq)]
#[contracttype]
pub struct BugReport {
    pub id: u64,
    pub reporter: Address,
    pub description: String,
    pub severity: Severity,
    pub status: ReportStatus,
    pub reward: Option<i128>,
    pub created_at: u64,
    pub approved_at: Option<u64>,
    pub paid_at: Option<u64>,
    pub approvers: Vec<Address>,
}

/// Reward tier configuration.
#[derive(Clone, Debug, PartialEq)]
#[contracttype]
pub struct RewardTier {
    pub critical_min: i128,
    pub critical_max: i128,
    pub high_min: i128,
    pub high_max: i128,
    pub medium_min: i128,
    pub medium_max: i128,
    pub low_min: i128,
    pub low_max: i128,
}

/// Configuration for the bounty payout system.
#[derive(Clone, Debug, PartialEq)]
#[contracttype]
pub struct BountyPayoutConfig {
    pub timelock_duration: u64,
    pub multi_sig_threshold: u32,
    pub high_value_threshold: i128,
    pub emergency_pause: bool,
}

/// ─── Events ─────────────────────────────────────────────────────────────────

/// Event emitted when a bug report is submitted.
pub const EVENT_REPORT_SUBMITTED: Symbol = symbol_short!("rpt_sub");

/// Event emitted when a bounty is approved.
pub const EVENT_BOUNTY_APPROVED: Symbol = symbol_short!("bty_appr");

/// Event emitted when a bounty is paid.
pub const EVENT_BOUNTY_PAID: Symbol = symbol_short!("bty_paid");

/// Event emitted when the pool is funded.
pub const EVENT_POOL_FUNDED: Symbol = symbol_short!("pool_fund");

/// ─── Initialization ───────────────────────────────────────────────────────────

/// Initialize the bug bounty payout system.
pub fn initialize(
    env: &Env,
    admin: &Address,
    initial_pool: i128,
) -> Result<(), BountyPayoutError> {
    admin.require_auth();

    // Initialize counter
    env.storage()
        .instance()
        .set(&BountyPayoutKey::ReportCounter, &0u64);

    // Initialize bounty pool
    env.storage()
        .instance()
        .set(&BountyPayoutKey::BountyPool, &initial_pool);

    // Initialize admin list
    let mut admins = Vec::new(env);
    admins.push(admin.clone());
    env.storage()
        .instance()
        .set(&BountyPayoutKey::Admins, &admins);

    // Initialize default reward tiers
    let default_tiers = RewardTier {
        critical_min: 500_000,
        critical_max: 1_000_000_000,
        high_min: 100_000,
        high_max: 500_000,
        medium_min: 10_000,
        medium_max: 100_000,
        low_min: 1_000,
        low_max: 10_000,
    };
    env.storage()
        .instance()
        .set(&BountyPayoutKey::RewardTiers, &default_tiers);

    // Initialize default config
    let default_config = BountyPayoutConfig {
        timelock_duration: DEFAULT_TIMELOCK_DURATION,
        multi_sig_threshold: MULTI_SIG_THRESHOLD,
        high_value_threshold: HIGH_VALUE_THRESHOLD,
        emergency_pause: false,
    };
    env.storage()
        .instance()
        .set(&BountyPayoutKey::Config, &default_config);

    Ok(())
}

/// ─── Bug Report Submission ─────────────────────────────────────────────────────

/// Submit a bug report.
pub fn submit_bug_report(
    env: &Env,
    reporter: &Address,
    description: String,
    severity: Severity,
) -> Result<BugReport, BountyPayoutError> {
    reporter.require_auth();

    // Check burst protection
    let counter: u64 = env
        .storage()
        .instance()
        .get(&BountyPayoutKey::ReportCounter)
        .unwrap_or(0);

    // Get config
    let config: BountyPayoutConfig = env
        .storage()
        .instance()
        .get(&BountyPayoutKey::Config)
        .ok_or(BountyPayoutError::NotAuthorized)?;

    if config.emergency_pause {
        return Err(BountyPayoutError::NotAuthorized);
    }

    // Generate new report ID
    let report_id = counter + 1;
    env.storage()
        .instance()
        .set(&BountyPayoutKey::ReportCounter, &report_id);

    // Create bug report
    let report = BugReport {
        id: report_id,
        reporter: reporter.clone(),
        description,
        severity,
        status: ReportStatus::Submitted,
        reward: None,
        created_at: env.ledger().timestamp(),
        approved_at: None,
        paid_at: None,
        approvers: Vec::new(env),
    };

    // Store report
    env.storage()
        .instance()
        .set(&BountyPayoutKey::Report(report_id), &report);

    // Emit event
    env.events().publish(
        (symbol_short!("bounty"), EVENT_REPORT_SUBMITTED),
        (reporter.clone(), report_id, report.severity, report.created_at),
    );

    Ok(report)
}

/// ─── Multi-Sig Approval System ─────────────────────────────────────────────────

/// Approve a bug report (admin only).
pub fn approve_bounty(
    env: &Env,
    admin: &Address,
    report_id: u64,
    reward: i128,
) -> Result<BugReport, BountyPayoutError> {
    admin.require_auth();

    // Verify admin
    let admins: Vec<Address> = env
        .storage()
        .instance()
        .get(&BountyPayoutKey::Admins)
        .ok_or(BountyPayoutError::NotAuthorized)?;

    if !admins.contains(admin) {
        return Err(BountyPayoutError::NotAuthorized);
    }

    // Get report
    let mut report: BugReport = env
        .storage()
        .instance()
        .get(&BountyPayoutKey::Report(report_id))
        .ok_or(BountyPayoutError::ReportNotFound)?;

    // Check if already paid
    if report.status == ReportStatus::Paid {
        return Err(BountyPayoutError::AlreadyPaid);
    }

    // Check if admin already approved
    if report.approvers.contains(admin) {
        return Err(BountyPayoutError::AlreadyApproved);
    }

    // Validate reward
    if reward < MIN_BOUNTY_REWARD || reward > MAX_BOUNTY_REWARD {
        return Err(BountyPayoutError::InvalidReward);
    }

    // Get config
    let config: BountyPayoutConfig = env
        .storage()
        .instance()
        .get(&BountyPayoutKey::Config)
        .ok_or(BountyPayoutError::NotAuthorized)?;

    if config.emergency_pause {
        return Err(BountyPayoutError::NotAuthorized);
    }

    // Add approval
    report.approvers.push(admin.clone());
    report.reward = Some(reward);

    // Store approval
    env.storage()
        .instance()
        .set(&BountyPayoutKey::Approval(report_id, admin.clone()), &true);

    // Check multi-sig threshold
    if report.approvers.len() >= config.multi_sig_threshold as u32 {
        report.status = ReportStatus::Approved;
        report.approved_at = Some(env.ledger().timestamp());

        // Set timelock for high-value bounties
        if reward >= config.high_value_threshold {
            let unlock_time = env.ledger().timestamp() + config.timelock_duration;
            env.storage()
                .instance()
                .set(&BountyPayoutKey::Timelock(report_id), &unlock_time);
        }

        // Emit approval event
        env.events().publish(
            (symbol_short!("bounty"), EVENT_BOUNTY_APPROVED),
            (report_id, reward, report.approvers.len()),
        );
    }

    // Update report
    env.storage()
        .instance()
        .set(&BountyPayoutKey::Report(report_id), &report);

    Ok(report)
}

/// ─── Bounty Payout ─────────────────────────────────────────────────────────────

/// Pay an approved bounty.
pub fn pay_bounty(
    env: &Env,
    admin: &Address,
    report_id: u64,
) -> Result<BugReport, BountyPayoutError> {
    admin.require_auth();

    // Verify admin
    let admins: Vec<Address> = env
        .storage()
        .instance()
        .get(&BountyPayoutKey::Admins)
        .ok_or(BountyPayoutError::NotAuthorized)?;

    if !admins.contains(admin) {
        return Err(BountyPayoutError::NotAuthorized);
    }

    // Get report
    let mut report: BugReport = env
        .storage()
        .instance()
        .get(&BountyPayoutKey::Report(report_id))
        .ok_or(BountyPayoutError::ReportNotFound)?;

    // Verify approved status
    if report.status != ReportStatus::Approved {
        return Err(BountyPayoutError::PendingApproval);
    }

    // Check if already paid
    if report.status == ReportStatus::Paid {
        return Err(BountyPayoutError::AlreadyPaid);
    }

    // Get reward amount
    let reward = report.reward.ok_or(BountyPayoutError::InvalidReward)?;

    // Check pool balance
    let pool_balance: i128 = env
        .storage()
        .instance()
        .get(&BountyPayoutKey::BountyPool)
        .unwrap_or(0);

    if pool_balance < reward {
        return Err(BountyPayoutError::InsufficientPoolBalance);
    }

    // Get config
    let config: BountyPayoutConfig = env
        .storage()
        .instance()
        .get(&BountyPayoutKey::Config)
        .ok_or(BountyPayoutError::NotAuthorized)?;

    if config.emergency_pause {
        return Err(BountyPayoutError::NotAuthorized);
    }

    // Check timelock for high-value bounties
    if reward >= config.high_value_threshold {
        if let Some(unlock_time) = env
            .storage()
            .instance()
            .get(&BountyPayoutKey::Timelock(report_id))
        {
            if env.ledger().timestamp() < unlock_time {
                return Err(BountyPayoutError::TimelockNotExpired);
            }
        }
    }

    // Deduct from pool
    let new_balance = pool_balance - reward;
    env.storage()
        .instance()
        .set(&BountyPayoutKey::BountyPool, &new_balance);

    // Update report status
    report.status = ReportStatus::Paid;
    report.paid_at = Some(env.ledger().timestamp());

    // Store updated report
    env.storage()
        .instance()
        .set(&BountyPayoutKey::Report(report_id), &report);

    // Emit payment event
    env.events().publish(
        (symbol_short!("bounty"), EVENT_BOUNTY_PAID),
        (report.reporter.clone(), report_id, reward, report.paid_at),
    );

    Ok(report)
}

/// ─── Batch Operations ─────────────────────────────────────────────────────────

/// Submit multiple bug reports in one transaction (burst protection applies).
pub fn batch_submit_reports(
    env: &Env,
    reporter: &Address,
    reports: Vec<(String, Severity)>,
) -> Result<Vec<BugReport>, BountyPayoutError> {
    reporter.require_auth();

    // Check burst limit
    if reports.len() > MAX_REPORTS_PER_TX as usize {
        return Err(BountyPayoutError::TooManyReports);
    }

    let mut submitted_reports = Vec::new(env);

    for (description, severity) in reports.iter() {
        let report = submit_bug_report(env, reporter, description.clone(), severity.clone())?;
        submitted_reports.push(report);
    }

    Ok(submitted_reports)
}

/// ─── Pool Management ───────────────────────────────────────────────────────────

/// Fund the bounty pool (admin only).
pub fn fund_pool(env: &Env, admin: &Address, amount: i128) -> Result<i128, BountyPayoutError> {
    admin.require_auth();

    // Verify admin
    let admins: Vec<Address> = env
        .storage()
        .instance()
        .get(&BountyPayoutKey::Admins)
        .ok_or(BountyPayoutError::NotAuthorized)?;

    if !admins.contains(admin) {
        return Err(BountyPayoutError::NotAuthorized);
    }

    // Get current balance
    let current_balance: i128 = env
        .storage()
        .instance()
        .get(&BountyPayoutKey::BountyPool)
        .unwrap_or(0);

    // Add funds
    let new_balance = current_balance + amount;
    env.storage()
        .instance()
        .set(&BountyPayoutKey::BountyPool, &new_balance);

    // Emit event
    env.events().publish(
        (symbol_short!("bounty"), EVENT_POOL_FUNDED),
        (admin.clone(), amount, new_balance),
    );

    Ok(new_balance)
}

/// Get current pool balance.
pub fn get_pool_balance(env: &Env) -> i128 {
    env.storage()
        .instance()
        .get(&BountyPayoutKey::BountyPool)
        .unwrap_or(0)
}

/// ─── Query Functions ───────────────────────────────────────────────────────────

/// Get a bug report by ID.
pub fn get_report(env: &Env, report_id: u64) -> Option<BugReport> {
    env.storage()
        .instance()
        .get(&BountyPayoutKey::Report(report_id))
}

/// Get all reports by status (pagination support would be added in production).
pub fn get_reports_by_status(
    env: &Env,
    status: ReportStatus,
) -> Vec<BugReport> {
    let counter: u64 = env
        .storage()
        .instance()
        .get(&BountyPayoutKey::ReportCounter)
        .unwrap_or(0);

    let mut reports = Vec::new(env);

    for i in 1..=counter {
        if let Some(report) = get_report(env, i) {
            if report.status == status {
                reports.push(report);
            }
        }
    }

    reports
}

/// Get reward tiers.
pub fn get_reward_tiers(env: &Env) -> Option<RewardTier> {
    env.storage()
        .instance()
        .get(&BountyPayoutKey::RewardTiers)
}

/// ─── Admin Functions ───────────────────────────────────────────────────────────

/// Add a new admin (requires multi-sig in production).
pub fn add_admin(env: &Env, admin: &Address, new_admin: Address) -> Result<(), BountyPayoutError> {
    admin.require_auth();

    let mut admins: Vec<Address> = env
        .storage()
        .instance()
        .get(&BountyPayoutKey::Admins)
        .ok_or(BountyPayoutError::NotAuthorized)?;

    if !admins.contains(admin) {
        return Err(BountyPayoutError::NotAuthorized);
    }

    if !admins.contains(&new_admin) {
        admins.push(new_admin);
        env.storage()
            .instance()
            .set(&BountyPayoutKey::Admins, &admins);
    }

    Ok(())
}

/// Update configuration (admin only).
pub fn update_config(
    env: &Env,
    admin: &Address,
    config: BountyPayoutConfig,
) -> Result<(), BountyPayoutError> {
    admin.require_auth();

    let admins: Vec<Address> = env
        .storage()
        .instance()
        .get(&BountyPayoutKey::Admins)
        .ok_or(BountyPayoutError::NotAuthorized)?;

    if !admins.contains(admin) {
        return Err(BountyPayoutError::NotAuthorized);
    }

    env.storage()
        .instance()
        .set(&BountyPayoutKey::Config, &config);

    Ok(())
}

/// Emergency pause (admin only).
pub fn emergency_pause(env: &Env, admin: &Address) -> Result<(), BountyPayoutError> {
    admin.require_auth();

    let mut config: BountyPayoutConfig = env
        .storage()
        .instance()
        .get(&BountyPayoutKey::Config)
        .ok_or(BountyPayoutError::NotAuthorized)?;

    let admins: Vec<Address> = env
        .storage()
        .instance()
        .get(&BountyPayoutKey::Admins)
        .ok_or(BountyPayoutError::NotAuthorized)?;

    if !admins.contains(admin) {
        return Err(BountyPayoutError::NotAuthorized);
    }

    config.emergency_pause = true;
    env.storage()
        .instance()
        .set(&BountyPayoutKey::Config, &config);

    Ok(())
}

/// ─── Severity Helpers ───────────────────────────────────────────────────────────

/// Convert severity to symbol.
pub fn severity_to_symbol(severity: &Severity) -> Symbol {
    match severity {
        Severity::Critical => symbol_short!("critical"),
        Severity::High => symbol_short!("high"),
        Severity::Medium => symbol_short!("medium"),
        Severity::Low => symbol_short!("low"),
    }
}

/// Get recommended reward for a severity level.
pub fn get_recommended_reward(env: &Env, severity: &Severity) -> Option<(i128, i128)> {
    let tiers: RewardTier = env
        .storage()
        .instance()
        .get(&BountyPayoutKey::RewardTiers)?;

    match severity {
        Severity::Critical => Some((tiers.critical_min, tiers.critical_max)),
        Severity::High => Some((tiers.high_min, tiers.high_max)),
        Severity::Medium => Some((tiers.medium_min, tiers.medium_max)),
        Severity::Low => Some((tiers.low_min, tiers.low_max)),
    }
}
