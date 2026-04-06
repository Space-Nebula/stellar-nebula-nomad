#![cfg(test)]

use soroban_sdk::{
    testutils::{Address as _, Ledger, LedgerInfo},
    Address, Env, String, Symbol,
};

use crate::bug_bounty_payout::{
    initialize, submit_bug_report, approve_bounty, pay_bounty, fund_pool,
    get_report, get_pool_balance, get_reward_tiers, add_admin, update_config,
    emergency_pause, batch_submit_reports,
    BugReport, BountyPayoutError, Severity, ReportStatus,
    BountyPayoutConfig, RewardTier,
    DEFAULT_TIMELOCK_DURATION, HIGH_VALUE_THRESHOLD, MULTI_SIG_THRESHOLD,
};

fn setup() -> (Env, Address) {
    let env = Env::default();
    let admin = Address::generate(&env);
    (env, admin)
}

#[test]
fn test_initialize() {
    let (env, admin) = setup();

    let result = initialize(&env, &admin, 1_000_000);
    assert!(result.is_ok());

    // Verify pool balance
    let balance = get_pool_balance(&env);
    assert_eq!(balance, 1_000_000);

    // Verify reward tiers
    let tiers = get_reward_tiers(&env);
    assert!(tiers.is_some());
}

#[test]
fn test_submit_bug_report() {
    let (env, admin) = setup();
    initialize(&env, &admin, 1_000_000).unwrap();

    let reporter = Address::generate(&env);
    let description = String::from_str(&env, "Critical vulnerability in deposit function");
    let severity = Severity::Critical;

    let report = submit_bug_report(&env, &reporter, description.clone(), severity.clone()).unwrap();

    assert_eq!(report.id, 1);
    assert_eq!(report.reporter, reporter);
    assert_eq!(report.description, description);
    assert_eq!(report.severity, severity);
    assert_eq!(report.status, ReportStatus::Submitted);
    assert!(report.reward.is_none());
    assert!(report.approved_at.is_none());
    assert!(report.paid_at.is_none());
}

#[test]
fn test_approve_bounty() {
    let (env, admin) = setup();
    initialize(&env, &admin, 1_000_000).unwrap();

    let reporter = Address::generate(&env);
    let report = submit_bug_report(
        &env,
        &reporter,
        String::from_str(&env, "High severity bug"),
        Severity::High,
    ).unwrap();

    let reward = 250_000i128;
    let approved_report = approve_bounty(&env, &admin, report.id, reward).unwrap();

    assert_eq!(approved_report.status, ReportStatus::Approved);
    assert_eq!(approved_report.reward, Some(reward));
    assert!(approved_report.approved_at.is_some());
    assert_eq!(approved_report.approvers.len(), 1);
}

#[test]
fn test_pay_bounty() {
    let (env, admin) = setup();
    initialize(&env, &admin, 1_000_000).unwrap();

    let reporter = Address::generate(&env);
    let report = submit_bug_report(
        &env,
        &reporter,
        String::from_str(&env, "Medium severity bug"),
        Severity::Medium,
    ).unwrap();

    let reward = 50_000i128;
    approve_bounty(&env, &admin, report.id, reward).unwrap();

    // Set ledger timestamp to future (after timelock for low-value bounty)
    env.ledger().set(LedgerInfo {
        timestamp: 100_000,
        protocol_version: 22,
        sequence_number: 100,
        min_temp_entry_ttl: 1,
        min_persistent_entry_ttl: 1,
        max_entry_ttl: 1000000,
    });

    let paid_report = pay_bounty(&env, &admin, report.id).unwrap();

    assert_eq!(paid_report.status, ReportStatus::Paid);
    assert!(paid_report.paid_at.is_some());

    // Verify pool balance decreased
    let balance = get_pool_balance(&env);
    assert_eq!(balance, 1_000_000 - reward);
}

#[test]
fn test_multi_sig_approval() {
    let (env, admin1) = setup();
    initialize(&env, &admin1, 1_000_000).unwrap();

    // Add second admin
    let admin2 = Address::generate(&env);
    add_admin(&env, &admin1, admin2.clone()).unwrap();

    let reporter = Address::generate(&env);
    let report = submit_bug_report(
        &env,
        &reporter,
        String::from_str(&env, "High-value bounty"),
        Severity::High,
    ).unwrap();

    let reward = 250_000i128;

    // First admin approval
    let report1 = approve_bounty(&env, &admin1, report.id, reward).unwrap();
    assert_eq!(report1.approvers.len(), 1);
    assert_ne!(report1.status, ReportStatus::Approved);

    // Second admin approval (threshold met)
    let report2 = approve_bounty(&env, &admin2, report.id, reward).unwrap();
    assert_eq!(report2.approvers.len(), 2);
    assert_eq!(report2.status, ReportStatus::Approved);
}

#[test]
fn test_high_value_timelock() {
    let (env, admin1) = setup();
    initialize(&env, &admin1, 10_000_000).unwrap();

    // Add second admin for multi-sig
    let admin2 = Address::generate(&env);
    add_admin(&env, &admin1, admin2.clone()).unwrap();

    let reporter = Address::generate(&env);
    let report = submit_bug_report(
        &env,
        &reporter,
        String::from_str(&env, "Critical high-value bug"),
        Severity::Critical,
    ).unwrap();

    let reward = HIGH_VALUE_THRESHOLD + 1; // Above threshold

    // Approve with multi-sig
    approve_bounty(&env, &admin1, report.id, reward).unwrap();
    approve_bounty(&env, &admin2, report.id, reward).unwrap();

    // Try to pay immediately (should fail due to timelock)
    let result = pay_bounty(&env, &admin1, report.id);
    assert_eq!(result, Err(BountyPayoutError::TimelockNotExpired));
}

#[test]
fn test_batch_submit_reports() {
    let (env, admin) = setup();
    initialize(&env, &admin, 1_000_000).unwrap();

    let reporter = Address::generate(&env);

    let reports = vec![
        (String::from_str(&env, "Bug 1"), Severity::High),
        (String::from_str(&env, "Bug 2"), Severity::Medium),
        (String::from_str(&env, "Bug 3"), Severity::Low),
    ];

    let submitted = batch_submit_reports(&env, &reporter, reports.into()).unwrap();
    assert_eq!(submitted.len(), 3);
}

#[test]
fn test_insufficient_pool_balance() {
    let (env, admin) = setup();
    initialize(&env, &admin, 100_000).unwrap();

    let reporter = Address::generate(&env);
    let report = submit_bug_report(
        &env,
        &reporter,
        String::from_str(&env, "Expensive bug"),
        Severity::Critical,
    ).unwrap();

    let reward = 500_000i128; // More than pool balance
    approve_bounty(&env, &admin, report.id, reward).unwrap();

    let result = pay_bounty(&env, &admin, report.id);
    assert_eq!(result, Err(BountyPayoutError::InsufficientPoolBalance));
}

#[test]
fn test_unauthorized_admin() {
    let (env, admin) = setup();
    initialize(&env, &admin, 1_000_000).unwrap();

    let fake_admin = Address::generate(&env);
    let reporter = Address::generate(&env);

    let report = submit_bug_report(
        &env,
        &reporter,
        String::from_str(&env, "Bug report"),
        Severity::Low,
    ).unwrap();

    let result = approve_bounty(&env, &fake_admin, report.id, 1000);
    assert_eq!(result, Err(BountyPayoutError::NotAuthorized));
}

#[test]
fn test_emergency_pause() {
    let (env, admin) = setup();
    initialize(&env, &admin, 1_000_000).unwrap();

    // Pause the contract
    emergency_pause(&env, &admin).unwrap();

    let reporter = Address::generate(&env);
    let result = submit_bug_report(
        &env,
        &reporter,
        String::from_str(&env, "Bug during pause"),
        Severity::Medium,
    );

    assert_eq!(result, Err(BountyPayoutError::NotAuthorized));
}

#[test]
fn test_fund_pool() {
    let (env, admin) = setup();
    initialize(&env, &admin, 1_000_000).unwrap();

    let new_balance = fund_pool(&env, &admin, 500_000).unwrap();
    assert_eq!(new_balance, 1_500_000);
}

#[test]
fn test_get_reward_tiers() {
    let (env, admin) = setup();
    initialize(&env, &admin, 1_000_000).unwrap();

    let tiers = get_reward_tiers(&env).unwrap();

    assert_eq!(tiers.critical_min, 500_000);
    assert_eq!(tiers.critical_max, 1_000_000_000);
    assert_eq!(tiers.high_min, 100_000);
    assert_eq!(tiers.high_max, 500_000);
    assert_eq!(tiers.medium_min, 10_000);
    assert_eq!(tiers.medium_max, 100_000);
    assert_eq!(tiers.low_min, 1_000);
    assert_eq!(tiers.low_max, 10_000);
}

#[test]
fn test_double_approval_prevention() {
    let (env, admin) = setup();
    initialize(&env, &admin, 1_000_000).unwrap();

    let reporter = Address::generate(&env);
    let report = submit_bug_report(
        &env,
        &reporter,
        String::from_str(&env, "Bug report"),
        Severity::High,
    ).unwrap();

    let reward = 100_000i128;

    // First approval
    approve_bounty(&env, &admin, report.id, reward).unwrap();

    // Same admin tries to approve again
    let result = approve_bounty(&env, &admin, report.id, reward);
    assert_eq!(result, Err(BountyPayoutError::AlreadyApproved));
}

#[test]
fn test_report_not_found() {
    let (env, admin) = setup();
    initialize(&env, &admin, 1_000_000).unwrap();

    let result = approve_bounty(&env, &admin, 999, 1000);
    assert_eq!(result, Err(BountyPayoutError::ReportNotFound));
}

#[test]
fn test_invalid_reward_amount() {
    let (env, admin) = setup();
    initialize(&env, &admin, 1_000_000).unwrap();

    let reporter = Address::generate(&env);
    let report = submit_bug_report(
        &env,
        &reporter,
        String::from_str(&env, "Bug report"),
        Severity::Low,
    ).unwrap();

    // Zero reward
    let result = approve_bounty(&env, &admin, report.id, 0);
    assert_eq!(result, Err(BountyPayoutError::InvalidReward));

    // Negative reward
    let result = approve_bounty(&env, &admin, report.id, -100);
    assert_eq!(result, Err(BountyPayoutError::InvalidReward));
}

#[test]
fn test_update_config() {
    let (env, admin) = setup();
    initialize(&env, &admin, 1_000_000).unwrap();

    let new_config = BountyPayoutConfig {
        timelock_duration: 259_200, // 3 days
        multi_sig_threshold: 3,
        high_value_threshold: 200_000,
        emergency_pause: false,
    };

    let result = update_config(&env, &admin, new_config.clone());
    assert!(result.is_ok());
}
