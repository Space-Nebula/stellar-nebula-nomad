#![cfg(test)]
//! Gas sponsorship (Issue #81): first-scan subsidy, one-time eligibility,
//! the profile-creation trigger, daily cap accounting, and admin replenishment.

use soroban_sdk::testutils::Address as _;
use soroban_sdk::{Address, Env};
use stellar_nebula_nomad::{
    claim_sponsorship_fund, get_fund_balance, get_remaining_daily_slots, has_been_sponsored,
    initialize_sponsorship, mark_profile_verified, sponsor_first_scan, NebulaNomadContract,
    NebulaNomadContractClient, SponsorError, MAX_DAILY_SPONSORSHIPS,
};

/// Matches `SponsorConfig::default().sponsor_amount`.
const SPONSOR_AMOUNT: i128 = 100_000;

/// Registers the contract and funds the sponsorship pool with `fund` stroops.
fn setup(fund: i128) -> (Env, Address, Address) {
    let env = Env::default();
    env.mock_all_auths();
    let contract = env.register(NebulaNomadContract, ());
    let admin = Address::generate(&env);
    env.as_contract(&contract, || {
        initialize_sponsorship(&env, &admin, fund).unwrap();
    });
    (env, contract, admin)
}

#[test]
fn verified_profile_gets_one_sponsored_scan() {
    let (env, contract, _admin) = setup(10_000_000);
    let player = Address::generate(&env);

    env.as_contract(&contract, || {
        mark_profile_verified(&env, &player);

        let granted = sponsor_first_scan(&env, &player).unwrap();

        assert_eq!(granted, SPONSOR_AMOUNT);
        assert!(has_been_sponsored(&env, &player));
        assert_eq!(get_fund_balance(&env), 10_000_000 - SPONSOR_AMOUNT);
    });
}

#[test]
fn sponsorship_is_one_time_per_address() {
    let (env, contract, _admin) = setup(10_000_000);
    let player = Address::generate(&env);

    env.as_contract(&contract, || {
        mark_profile_verified(&env, &player);
        sponsor_first_scan(&env, &player).unwrap();

        let second = sponsor_first_scan(&env, &player);
        assert_eq!(second, Err(SponsorError::AlreadySponsored));
        // The rejected call must not drain the pool.
        assert_eq!(get_fund_balance(&env), 10_000_000 - SPONSOR_AMOUNT);
    });
}

#[test]
fn unverified_profile_is_rejected() {
    let (env, contract, _admin) = setup(10_000_000);
    let stranger = Address::generate(&env);

    env.as_contract(&contract, || {
        let result = sponsor_first_scan(&env, &stranger);
        assert_eq!(result, Err(SponsorError::ProfileNotVerified));
        assert!(!has_been_sponsored(&env, &stranger));
    });
}

#[test]
fn empty_pool_rejects_sponsorship() {
    // Fund smaller than one sponsorship amount.
    let (env, contract, _admin) = setup(50_000);
    let player = Address::generate(&env);

    env.as_contract(&contract, || {
        mark_profile_verified(&env, &player);
        let result = sponsor_first_scan(&env, &player);
        assert_eq!(result, Err(SponsorError::InsufficientFunds));
        assert!(!has_been_sponsored(&env, &player));
    });
}

#[test]
fn daily_slots_decrease_per_sponsorship() {
    let (env, contract, _admin) = setup(10_000_000);
    let player = Address::generate(&env);

    env.as_contract(&contract, || {
        assert_eq!(get_remaining_daily_slots(&env), MAX_DAILY_SPONSORSHIPS);

        mark_profile_verified(&env, &player);
        sponsor_first_scan(&env, &player).unwrap();

        assert_eq!(get_remaining_daily_slots(&env), MAX_DAILY_SPONSORSHIPS - 1);
    });
}

#[test]
fn admin_can_replenish_fund_and_others_cannot() {
    let (env, contract, admin) = setup(10_000_000);
    let stranger = Address::generate(&env);

    env.as_contract(&contract, || {
        let new_balance = claim_sponsorship_fund(&env, &admin, 500).unwrap();
        assert_eq!(new_balance, 10_000_000 + 500);

        let unauthorized = claim_sponsorship_fund(&env, &stranger, 500);
        assert_eq!(unauthorized, Err(SponsorError::Unauthorized));

        let zero = claim_sponsorship_fund(&env, &admin, 0);
        assert_eq!(zero, Err(SponsorError::InvalidAmount));
    });
}

#[test]
fn profile_creation_unlocks_first_scan_end_to_end() {
    let (env, contract, _admin) = setup(10_000_000);
    let client = NebulaNomadContractClient::new(&env, &contract);
    let player = Address::generate(&env);

    // Profile creation marks the owner as verified (the onboarding trigger).
    client.initialize_profile(&player);

    assert_eq!(client.sponsor_first_scan(&player), SPONSOR_AMOUNT);
    assert_eq!(
        client.try_sponsor_first_scan(&player),
        Err(Ok(SponsorError::AlreadySponsored))
    );
}
