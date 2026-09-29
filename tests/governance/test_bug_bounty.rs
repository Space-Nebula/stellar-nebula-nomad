#![cfg(test)]

use soroban_sdk::testutils::{Address as _, Ledger, LedgerInfo};
use soroban_sdk::{symbol_short, vec, Address, Env, String};
use stellar_nebula_nomad::{
    BugReport, BugReportStatus, NebulaNomadContract, NebulaNomadContractClient,
    EMBARGO_PERIOD_SECONDS,
};

fn setup_test_env() -> (Env, NebulaNomadContractClient<'static>, Address, Address, Address, Address) {
    let env = Env::default();
    env.mock_all_auths();
    let mut ledger_info = env.ledger().get();
    ledger_info.timestamp = 1_700_000_000;
    env.ledger().set(ledger_info);
    let contract_id = env.register(NebulaNomadContract, ());
    let client = NebulaNomadContractClient::new(&env, &contract_id);
    let admin = Address::generate(&env);
    let approver_2 = Address::generate(&env);
    let reporter_1 = Address::generate(&env);
    let reporter_2 = Address::generate(&env);

    let approvers = vec![&env, approver_2.clone()];
    client.init_bounty_engine(&admin, &approvers, &2, &10_000, &0);
    client.fund_bounty_pool(&admin, &50_000);

    (env, client, admin, approver_2, reporter_1, reporter_2)
}

#[test]
fn test_initialization_and_reward_tiers() {
    let (env, client, admin, _, _, _) = setup_test_env();

    assert_eq!(client.get_reward_tier(&symbol_short!("critical")), Some(5_000));
    assert_eq!(client.get_reward_tier(&symbol_short!("high")), Some(2_000));
    assert_eq!(client.get_reward_tier(&symbol_short!("medium")), Some(500));
    assert_eq!(client.get_reward_tier(&symbol_short!("low")), Some(100));

    client.set_reward_tier(&admin, &symbol_short!("critical"), &6_000);
    assert_eq!(client.get_reward_tier(&symbol_short!("critical")), Some(6_000));

    let unauthorized = Address::generate(&env);
    let err = client.try_set_reward_tier(&unauthorized, &symbol_short!("critical"), &10_000);
    assert!(err.is_err());
}

#[test]
fn test_duplicate_detection_rejects_identical_reports() {
    let (env, client, _, _, reporter_1, reporter_2) = setup_test_env();

    let desc = String::from_str(&env, "buffer overflow in nebula generation packet");
    let report_id = client.submit_bug_report(&reporter_1, &desc, &symbol_short!("high"));
    assert_eq!(report_id, 1);

    let duplicate = client.try_submit_bug_report(&reporter_2, &desc, &symbol_short!("high"));
    assert!(duplicate.is_err());

    let different_desc = String::from_str(&env, "integer underflow in craft xp accumulator");
    let second_report_id = client.submit_bug_report(&reporter_2, &different_desc, &symbol_short!("medium"));
    assert_eq!(second_report_id, 2);
}

#[test]
fn test_review_workflow_rejection_with_feedback() {
    let (env, client, admin, approver_2, reporter_1, _) = setup_test_env();

    let desc = String::from_str(&env, "informational observation about variable naming");
    let report_id = client.submit_bug_report(&reporter_1, &desc, &symbol_short!("low"));

    let feedback = String::from_str(&env, "style choices not eligible for bug bounty payout");
    client.reject_bug_report(&admin, &report_id, &feedback);

    let report: BugReport = client.get_report(&report_id).expect("report exists");
    assert_eq!(report.status, BugReportStatus::Rejected);
    assert_eq!(report.feedback, feedback);

    let pay_attempt = client.try_approve_and_pay_bounty(&approver_2, &report_id, &100);
    assert!(pay_attempt.is_err());
}

#[test]
fn test_review_workflow_reclassify_severity() {
    let (env, client, admin, approver_2, reporter_1, _) = setup_test_env();

    let desc = String::from_str(&env, "reentrancy allowing balance extraction");
    let report_id = client.submit_bug_report(&reporter_1, &desc, &symbol_short!("low"));

    let initial_report = client.get_report(&report_id).expect("report exists");
    assert_eq!(initial_report.severity, symbol_short!("low"));
    assert_eq!(initial_report.default_reward, 100);

    client.reclassify_severity(&admin, &report_id, &symbol_short!("critical"));

    let reclassified = client.get_report(&report_id).expect("report exists");
    assert_eq!(reclassified.severity, symbol_short!("critical"));
    assert_eq!(reclassified.default_reward, 5_000);

    client.approve_and_pay_bounty(&admin, &report_id, &5_000);
    client.approve_and_pay_bounty(&approver_2, &report_id, &5_000);

    let finalized = client.get_report(&report_id).expect("report exists");
    assert_eq!(finalized.status, BugReportStatus::Approved);
    assert!(finalized.paid);
    assert_eq!(finalized.payout_amount, 5_000);
    assert_eq!(client.get_bounty_balance(&reporter_1), 5_000);
}

#[test]
fn test_cumulative_bounties_and_hall_of_fame() {
    let (env, client, admin, approver_2, reporter_1, reporter_2) = setup_test_env();

    let desc_1 = String::from_str(&env, "critical vulnerability in energy regeneration");
    let r1 = client.submit_bug_report(&reporter_1, &desc_1, &symbol_short!("critical"));
    client.approve_and_pay_bounty(&admin, &r1, &5_000);
    client.approve_and_pay_bounty(&approver_2, &r1, &5_000);

    let desc_2 = String::from_str(&env, "medium severity state race in mission generator");
    let r2 = client.submit_bug_report(&reporter_2, &desc_2, &symbol_short!("medium"));
    client.approve_and_pay_bounty(&admin, &r2, &500);
    client.approve_and_pay_bounty(&approver_2, &r2, &500);

    let desc_3 = String::from_str(&env, "high severity replay issue in wormhole traveler");
    let r3 = client.submit_bug_report(&reporter_1, &desc_3, &symbol_short!("high"));
    client.approve_and_pay_bounty(&admin, &r3, &2_000);
    client.approve_and_pay_bounty(&approver_2, &r3, &2_000);

    assert_eq!(client.get_total_bounties_paid(), 7_500);

    let stats_1 = client.get_researcher_stats(&reporter_1);
    assert_eq!(stats_1.total_bounties, 7_000);
    assert_eq!(stats_1.bugs_resolved, 2);

    let stats_2 = client.get_researcher_stats(&reporter_2);
    assert_eq!(stats_2.total_bounties, 500);
    assert_eq!(stats_2.bugs_resolved, 1);

    let hof = client.get_hall_of_fame();
    assert_eq!(hof.len(), 2);
    assert!(hof.contains(&reporter_1));
    assert!(hof.contains(&reporter_2));
}

#[test]
fn test_90_day_embargo_disclosure_enforcement() {
    let (env, client, admin, _, reporter_1, _) = setup_test_env();

    let desc = String::from_str(&env, "exploit in trade escrow verification");
    let report_id = client.submit_bug_report(&reporter_1, &desc, &symbol_short!("high"));

    assert!(!client.can_disclose_report(&report_id));

    let early_disclosure = client.try_disclose_bug_report(&reporter_1, &report_id);
    assert!(early_disclosure.is_err());

    let current_ledger = env.ledger().get();
    env.ledger().set(LedgerInfo {
        timestamp: current_ledger.timestamp + EMBARGO_PERIOD_SECONDS + 1,
        ..current_ledger
    });

    assert!(client.can_disclose_report(&report_id));

    client.disclose_bug_report(&reporter_1, &report_id);
    let disclosed_report = client.get_report(&report_id).expect("report exists");
    assert_eq!(disclosed_report.status, BugReportStatus::Disclosed);

    let already_disclosed = client.try_disclose_bug_report(&admin, &report_id);
    assert!(already_disclosed.is_err());
}

#[test]
fn test_governance_budget_allocation() {
    let (env, client, admin, _, _, _) = setup_test_env();

    let dao = Address::generate(&env);
    client.set_governance_contract(&admin, &dao);

    let initial_pool = client.get_bounty_pool();
    let updated_pool = client.allocate_bounty_gov_budget(&dao, &25_000);
    assert_eq!(updated_pool, initial_pool + 25_000);
    assert_eq!(client.get_bounty_pool(), initial_pool + 25_000);

    let unauthorized = Address::generate(&env);
    let fail_alloc = client.try_allocate_bounty_gov_budget(&unauthorized, &10_000);
    assert!(fail_alloc.is_err());
}

#[test]
fn test_emergency_pause_controls() {
    let (env, client, admin, approver_2, reporter_1, _) = setup_test_env();

    client.set_emergency_pause(&admin, &true);

    let desc = String::from_str(&env, "submission while paused");
    let submit_fail = client.try_submit_bug_report(&reporter_1, &desc, &symbol_short!("low"));
    assert!(submit_fail.is_err());

    client.set_emergency_pause(&admin, &false);

    let report_id = client.submit_bug_report(&reporter_1, &desc, &symbol_short!("low"));
    assert_eq!(report_id, 1);

    client.set_emergency_pause(&admin, &true);
    let pay_fail = client.try_approve_and_pay_bounty(&approver_2, &report_id, &100);
    assert!(pay_fail.is_err());

    client.set_emergency_pause(&admin, &false);
    client.approve_and_pay_bounty(&admin, &report_id, &100);
    let approved = client.approve_and_pay_bounty(&approver_2, &report_id, &100);
    assert!(approved);
}

#[test]
fn test_high_value_bounty_timelock_enforcement() {
    let env = Env::default();
    env.mock_all_auths();
    let mut ledger_info = env.ledger().get();
    ledger_info.timestamp = 1_700_000_000;
    env.ledger().set(ledger_info);
    let contract_id = env.register(NebulaNomadContract, ());
    let client = NebulaNomadContractClient::new(&env, &contract_id);
    let admin = Address::generate(&env);
    let approver_2 = Address::generate(&env);
    let reporter = Address::generate(&env);

    let approvers = vec![&env, approver_2.clone()];
    client.init_bounty_engine(&admin, &approvers, &2, &5_000, &3600);
    client.fund_bounty_pool(&admin, &50_000);

    let desc = String::from_str(&env, "critical smart contract reentrancy");
    let report_id = client.submit_bug_report(&reporter, &desc, &symbol_short!("critical"));

    client.approve_and_pay_bounty(&admin, &report_id, &5_000);

    let early_approval = client.try_approve_and_pay_bounty(&approver_2, &report_id, &5_000);
    assert!(early_approval.is_err());

    let mut updated_ledger = env.ledger().get();
    updated_ledger.timestamp += 3601;
    env.ledger().set(updated_ledger);

    let final_approval = client.approve_and_pay_bounty(&approver_2, &report_id, &5_000);
    assert!(final_approval);
    assert_eq!(client.get_bounty_balance(&reporter), 5_000);
}
