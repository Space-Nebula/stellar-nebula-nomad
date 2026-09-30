#![cfg(test)]
//! Staking through the contract client: resource locks, emergency exits,
//! the yield reserve, ship staking, LP staking with impermanent-loss
//! protection, guild staking and governance slashing.

use soroban_sdk::testutils::{Address as _, Ledger};
use soroban_sdk::{symbol_short, Address, Bytes, Env, String, Symbol};
use stellar_nebula_nomad::resource_minter::credit_resource_balance;
use stellar_nebula_nomad::{
    simple_yield, FarmError, LockTier, NebulaNomadContract, NebulaNomadContractClient, ShipError,
    StakingError,
};

const DAY: u64 = 86_400;

fn setup() -> (Env, NebulaNomadContractClient<'static>, Address) {
    let env = Env::default();
    env.mock_all_auths();
    env.ledger().with_mut(|li| {
        li.sequence_number = 100;
        li.timestamp = 1_700_000_000;
        li.min_temp_entry_ttl = 100;
        li.min_persistent_entry_ttl = 1000;
        li.max_entry_ttl = 10_000_000;
    });
    let id = env.register(NebulaNomadContract, ());
    let client = NebulaNomadContractClient::new(&env, &id);
    (env, client, id)
}

fn give(env: &Env, contract: &Address, who: &Address, asset: &Symbol, amount: u32) {
    env.as_contract(contract, || {
        credit_resource_balance(env, who, asset, amount).unwrap();
    });
}

fn advance(env: &Env, secs: u64) {
    env.ledger().with_mut(|li| {
        li.timestamp += secs;
        li.sequence_number += u32::try_from(secs / 5).unwrap_or(1).max(1);
    });
}

fn ore() -> Symbol {
    symbol_short!("ore")
}

#[test]
fn resource_stake_locks_principal_and_pays_tier_yield() {
    let (env, client, id) = setup();
    let owner = Address::generate(&env);
    let funder = Address::generate(&env);
    give(&env, &id, &owner, &ore(), 100_000);
    give(&env, &id, &funder, &ore(), 1_000);

    let stake_id = client.stake_resource(&owner, &ore(), &100_000, &LockTier::Days7);
    assert_eq!(
        client.get_resource_balance(&owner, &ore()),
        0,
        "principal left the wallet"
    );
    assert_eq!(client.get_staking_tvl(&ore()), 100_000);
    assert_eq!(client.get_tvl_report().resource_units, 100_000);
    let stake = client.get_resource_stake(&stake_id).unwrap();
    assert_eq!(stake.apy_bps, 500);
    assert_eq!(stake.unlock_at, stake.staked_at + 7 * DAY);

    assert_eq!(
        client.try_unstake_resource(&owner, &stake_id),
        Err(Ok(FarmError::LockNotMet))
    );

    client.fund_yield_reserve(&funder, &ore(), &1_000);
    assert_eq!(client.get_yield_reserve(&ore()), 1_000);

    advance(&env, 7 * DAY);
    let expected = simple_yield(100_000, 500, 7 * DAY).unwrap();
    assert_eq!(expected, 95);
    assert_eq!(client.claim_resource_yield(&owner, &stake_id), expected);
    assert_eq!(client.get_resource_balance(&owner, &ore()), 95);
    assert_eq!(client.get_yield_reserve(&ore()), 1_000 - 95);

    assert_eq!(client.unstake_resource(&owner, &stake_id), 100_000);
    assert_eq!(client.get_resource_balance(&owner, &ore()), 100_095);
    assert_eq!(client.get_staking_tvl(&ore()), 0);
    assert!(client.get_resource_stake(&stake_id).is_none());
    assert_eq!(client.get_tvl_report().total_yield_paid, 95);
}

#[test]
fn yield_stops_accruing_at_the_end_of_the_lock() {
    let (env, client, id) = setup();
    let owner = Address::generate(&env);
    give(&env, &id, &owner, &ore(), 100_000);
    give(&env, &id, &owner, &ore(), 5_000);
    let stake_id = client.stake_resource(&owner, &ore(), &100_000, &LockTier::Days30);
    client.fund_yield_reserve(&owner, &ore(), &5_000);

    advance(&env, 60 * DAY);
    let capped = simple_yield(100_000, 1_500, 30 * DAY).unwrap();
    assert_eq!(capped, 1_232);
    assert_eq!(client.claim_resource_yield(&owner, &stake_id), capped);
    assert_eq!(client.claim_resource_yield(&owner, &stake_id), 0);
}

#[test]
fn claims_fail_closed_when_the_reserve_is_empty() {
    let (env, client, id) = setup();
    let owner = Address::generate(&env);
    give(&env, &id, &owner, &ore(), 100_000);
    let stake_id = client.stake_resource(&owner, &ore(), &100_000, &LockTier::Days90);
    advance(&env, 90 * DAY);
    assert_eq!(
        client.try_claim_resource_yield(&owner, &stake_id),
        Err(Ok(FarmError::YieldReserveEmpty))
    );
    // Principal is never blocked by the reserve, but unclaimed yield is.
    assert_eq!(
        client.try_unstake_resource(&owner, &stake_id),
        Err(Ok(FarmError::YieldReserveEmpty))
    );
    give(&env, &id, &owner, &ore(), 10_000);
    client.fund_yield_reserve(&owner, &ore(), &10_000);
    let due = simple_yield(100_000, 3_500, 90 * DAY).unwrap();
    assert_eq!(client.unstake_resource(&owner, &stake_id), 100_000 + due);
}

#[test]
fn emergency_withdrawal_forfeits_half_to_the_reserve() {
    let (env, client, id) = setup();
    let owner = Address::generate(&env);
    give(&env, &id, &owner, &ore(), 10_000);
    let stake_id = client.stake_resource(&owner, &ore(), &10_000, &LockTier::Days90);
    advance(&env, DAY);

    let out = client.emergency_unstake_resource(&owner, &stake_id);
    assert_eq!(out.returned, 5_000);
    assert_eq!(out.penalty, 5_000);
    assert_eq!(
        out.yield_forfeited,
        simple_yield(10_000, 3_500, DAY).unwrap()
    );
    assert_eq!(client.get_resource_balance(&owner, &ore()), 5_000);
    assert_eq!(client.get_yield_reserve(&ore()), 5_000);
    assert_eq!(client.get_staking_tvl(&ore()), 0);
    assert!(client.get_resource_stake(&stake_id).is_none());

    // After the unlock the same call is a plain withdrawal.
    let stake_id = client.stake_resource(&owner, &ore(), &5_000, &LockTier::Days7);
    advance(&env, 7 * DAY);
    let out = client.emergency_unstake_resource(&owner, &stake_id);
    assert_eq!(out.penalty, 0);
    assert_eq!(
        out.returned,
        5_000 + simple_yield(5_000, 500, 7 * DAY).unwrap()
    );
}

#[test]
fn only_the_owner_touches_a_stake() {
    let (env, client, id) = setup();
    let owner = Address::generate(&env);
    let other = Address::generate(&env);
    give(&env, &id, &owner, &ore(), 1_000);
    let stake_id = client.stake_resource(&owner, &ore(), &1_000, &LockTier::Days7);
    assert_eq!(
        client.try_claim_resource_yield(&other, &stake_id),
        Err(Ok(FarmError::NotOwner))
    );
    assert_eq!(
        client.try_emergency_unstake_resource(&other, &stake_id),
        Err(Ok(FarmError::NotOwner))
    );
    assert_eq!(
        client.try_unstake_resource(&owner, &99),
        Err(Ok(FarmError::NotStaked))
    );
    assert_eq!(
        client.try_stake_resource(&owner, &ore(), &1, &LockTier::Days7),
        Err(Ok(FarmError::InsufficientBalance))
    );
}

#[test]
fn apy_is_dampened_once_tvl_passes_the_target() {
    let (env, client, id) = setup();
    let admin = Address::generate(&env);
    let owner = Address::generate(&env);
    client.init_staking(&admin, &Address::generate(&env), &1, &10);
    client.set_tvl_target(&admin, &1_000);
    assert_eq!(
        client.try_set_tvl_target(&owner, &5),
        Err(Ok(FarmError::NotOwner))
    );

    give(&env, &id, &owner, &ore(), 200_000);
    let first = client.stake_resource(&owner, &ore(), &100_000, &LockTier::Days7);
    let second = client.stake_resource(&owner, &ore(), &100_000, &LockTier::Days7);
    assert_eq!(client.get_resource_stake(&first).unwrap().apy_bps, 500);
    // 500 * 1000 / 100000 = 5, floored at 20 % of base.
    assert_eq!(client.get_resource_stake(&second).unwrap().apy_bps, 100);
}

#[test]
fn staked_ship_is_frozen_and_earns_level_based_yield() {
    let (env, client, id) = setup();
    let owner = Address::generate(&env);
    let buyer = Address::generate(&env);
    let ship = client.mint_ship(&owner, &symbol_short!("explorer"), &Bytes::new(&env));

    let stake = client.stake_ship(&owner, &ship.id);
    assert_eq!(stake.level, 1);
    assert_eq!(stake.rarity_bps, 11_000);
    assert_eq!(client.get_tvl_report().ships_staked, 1);
    assert_eq!(
        client.try_stake_ship(&owner, &ship.id),
        Err(Ok(FarmError::AlreadyStaked))
    );
    assert_eq!(
        client.try_transfer_ownership(&ship.id, &buyer),
        Err(Ok(ShipError::Staked))
    );

    let dust = symbol_short!("dust");
    assert_eq!(client.get_nft_yield_asset(), dust);
    give(&env, &id, &owner, &dust, 1_000);
    client.fund_yield_reserve(&owner, &dust, &1_000);
    advance(&env, DAY);
    assert_eq!(client.claim_ship_yield(&owner, &ship.id), 11);
    assert_eq!(client.get_resource_balance(&owner, &dust), 11);

    advance(&env, DAY);
    assert_eq!(client.unstake_ship(&owner, &ship.id), 11);
    assert!(client.get_ship_stake(&ship.id).is_none());
    assert_eq!(client.get_tvl_report().ships_staked, 0);
    let moved = client.transfer_ownership(&ship.id, &buyer);
    assert_eq!(moved.owner, buyer);
}

#[test]
fn ship_yield_scales_with_progression_level() {
    let (env, client, id) = setup();
    let owner = Address::generate(&env);
    let ship = client.mint_ship(&owner, &symbol_short!("fighter"), &Bytes::new(&env));
    give(&env, &id, &owner, &ore(), 1_449 + 1_949);
    client.upgrade_ship_level(&owner, &ship.id, &ore());
    advance(&env, 400);
    client.upgrade_ship_level(&owner, &ship.id, &ore());
    assert_eq!(client.get_ship_level(&ship.id), 2);

    let stake = client.stake_ship(&owner, &ship.id);
    assert_eq!(stake.level, 2);
    let dust = symbol_short!("dust");
    give(&env, &id, &owner, &dust, 100);
    client.fund_yield_reserve(&owner, &dust, &100);
    advance(&env, DAY);
    assert_eq!(client.claim_ship_yield(&owner, &ship.id), 20);
}

#[test]
fn lp_stake_shares_rewards_and_compensates_impermanent_loss() {
    let (env, client, id) = setup();
    let provider = Address::generate(&env);
    let funder = Address::generate(&env);
    let trader = Address::generate(&env);
    let gas = symbol_short!("gas");

    let pool_id = client.create_pool(&provider, &ore(), &gas);
    let (lp, _) = client.add_liquidity(&provider, &pool_id, &100_000, &100_000);
    assert!(lp > 0);
    assert_eq!(client.get_lp_balance(&pool_id, &provider), lp);

    let stake = client.stake_lp(&provider, &pool_id, &lp);
    assert_eq!(stake.amount, lp);
    assert_eq!(stake.entry_ratio_bps, 10_000);
    assert_eq!(
        client.get_lp_balance(&pool_id, &provider),
        0,
        "LP units left the wallet"
    );
    assert_eq!(
        client.try_stake_lp(&provider, &pool_id, &1),
        Err(Ok(FarmError::InsufficientBalance))
    );

    give(&env, &id, &funder, &ore(), 1_000);
    client.fund_lp_rewards(&funder, &pool_id, &1_000);
    let state = client.get_lp_pool_state(&pool_id).unwrap();
    assert_eq!(state.il_pot, 200);
    assert_eq!(state.reward_asset, ore());
    assert_eq!(client.claim_lp_rewards(&provider, &pool_id), 800);
    assert_eq!(client.get_resource_balance(&provider, &ore()), 800);
    assert_eq!(client.claim_lp_rewards(&provider, &pool_id), 0);

    // Move the price: a large swap of ore into the pool shifts reserve_b /
    // reserve_a well away from the entry ratio.
    let route = soroban_sdk::vec![&env, pool_id];
    client.swap_exact_input(&trader, &ore(), &100_000, &1, &route);
    let pool = client.get_pool(&pool_id).unwrap();
    assert!(pool.reserve_a > pool.reserve_b);

    // Protection is fully vested after 90 days.
    advance(&env, 90 * DAY);
    let (rewards, compensation) = client.unstake_lp(&provider, &pool_id, &lp);
    assert_eq!(rewards, 0);
    assert!(compensation > 0 && compensation <= 200, "{compensation}");
    assert_eq!(client.get_lp_balance(&pool_id, &provider), lp);
    assert!(client.get_lp_stake(&pool_id, &provider).is_none());
    assert_eq!(
        client.get_lp_pool_state(&pool_id).unwrap().il_pot,
        200 - compensation
    );
}

#[test]
fn guild_stake_backs_the_treasury_and_shares_rewards() {
    let (env, client, id) = setup();
    let member = Address::generate(&env);
    let funder = Address::generate(&env);
    let outsider = Address::generate(&env);
    let alliance_id = client.found_alliance(&member, &String::from_str(&env, "Nomads"));
    give(&env, &id, &member, &ore(), 5_000);
    give(&env, &id, &funder, &ore(), 1_000);
    give(&env, &id, &outsider, &ore(), 100);

    assert_eq!(
        client.try_stake_to_guild(&outsider, &ore(), &100),
        Err(Ok(StakingError::NotGuildMember))
    );
    let stake = client.stake_to_guild(&member, &ore(), &5_000);
    assert_eq!(stake.alliance_id, alliance_id);
    assert_eq!(client.get_alliance_treasury(&alliance_id), 5_000);
    assert_eq!(client.get_guild_stake_total(&alliance_id), 5_000);
    assert_eq!(client.get_resource_balance(&member, &ore()), 0);
    assert_eq!(
        client.try_stake_to_guild(&member, &symbol_short!("gas"), &1),
        Err(Ok(StakingError::AssetMismatch))
    );
    assert_eq!(
        client.try_unstake_from_guild(&member),
        Err(Ok(StakingError::TimeLockActive))
    );

    client.fund_guild_rewards(&funder, &alliance_id, &1_000);
    assert_eq!(client.claim_guild_rewards(&member), 1_000);
    assert_eq!(client.claim_guild_rewards(&member), 0);

    advance(&env, 7 * DAY);
    assert_eq!(client.unstake_from_guild(&member), (5_000, 0));
    assert_eq!(client.get_alliance_treasury(&alliance_id), 0);
    assert_eq!(client.get_guild_stake_total(&alliance_id), 0);
    assert_eq!(client.get_resource_balance(&member, &ore()), 6_000);
    assert!(client.get_guild_stake(&member).is_none());
}

#[test]
fn slashing_cuts_the_stake_and_the_voting_power() {
    let (env, client, _) = setup();
    let admin = Address::generate(&env);
    let staker = Address::generate(&env);
    let stranger = Address::generate(&env);
    client.init_staking(&admin, &Address::generate(&env), &100, &10);
    client.stake_for_voting(&staker, &1_000);
    advance(&env, 10);
    assert_eq!(client.get_voting_power(&staker), 1_000);

    assert_eq!(
        client.try_slash_stake(&stranger, &staker, &5_000),
        Err(Ok(StakingError::Unauthorized))
    );
    assert_eq!(
        client.try_slash_stake(&admin, &staker, &0),
        Err(Ok(StakingError::InvalidSlash))
    );
    assert_eq!(client.slash_stake(&admin, &staker, &5_000), 500);
    assert_eq!(client.get_voting_power(&staker), 500);
    assert_eq!(client.get_stake_v2(&staker).unwrap().amount, 500);
    assert_eq!(client.get_total_slashed(), 500);
    assert_eq!(client.get_global_staking_stats().total_staked, 500);

    assert_eq!(client.slash_stake(&admin, &staker, &10_000), 500);
    assert!(client.get_stake_v2(&staker).is_none());
    assert_eq!(client.get_voting_power(&staker), 0);
}
