#![cfg(test)]

use soroban_sdk::testutils::{Address as _, Events, Ledger, LedgerInfo};
use soroban_sdk::{Address, Bytes, Env};
use stellar_nebula_nomad::{NebulaNomadContract, NebulaNomadContractClient};

fn setup() -> (Env, NebulaNomadContractClient<'static>, Address) {
    let env = Env::default();
    env.mock_all_auths();
    env.ledger().set(LedgerInfo {
        protocol_version: 22,
        sequence_number: 100,
        timestamp: 1_700_000_000,
        network_id: [0u8; 32],
        base_reserve: 10,
        min_temp_entry_ttl: 100,
        min_persistent_entry_ttl: 1000,
        max_entry_ttl: 10_000,
    });
    let contract_id = env.register(NebulaNomadContract, ());
    let client = NebulaNomadContractClient::new(&env, &contract_id);
    let player = Address::generate(&env);
    (env, client, player)
}

/// Mint a ship and return its ID. Required before any energy operation
/// because `require_ship_exists` checks persistent storage for the ship.
fn mint_test_ship(env: &Env, client: &NebulaNomadContractClient, owner: &Address) -> u64 {
    let metadata = Bytes::from_slice(env, &[0u8; 4]);
    let ship = client.mint_ship(owner, &soroban_sdk::symbol_short!("explorer"), &metadata);
    ship.id
}

// ─── Energy Management Tests ─────────────────────────────────────────────

#[test]
fn test_initialize_and_get_energy() {
    let (env, client, player) = setup();
    let ship_id = mint_test_ship(&env, &client, &player);

    client.initialize_energy(&ship_id, &100u32);
    let energy = client.get_energy(&ship_id);
    assert_eq!(energy, 100);
}

#[test]
fn test_consume_energy_success() {
    let (env, client, player) = setup();
    let ship_id = mint_test_ship(&env, &client, &player);

    client.initialize_energy(&ship_id, &100u32);
    let remaining = client.consume_energy(&ship_id, &30u32);
    assert_eq!(remaining, 70);

    // Verify that at least one energy event was emitted
    let events = env.events().all();
    assert!(
        !events.is_empty(),
        "Expected energy consumed event to be emitted"
    );
}

#[test]
fn test_consume_energy_insufficient() {
    let (env, client, player) = setup();
    let ship_id = mint_test_ship(&env, &client, &player);

    client.initialize_energy(&ship_id, &50u32);
    // Attempting to consume 80 from a balance of 50 must fail
    let result = client.try_consume_energy(&ship_id, &80u32);
    assert!(result.is_err());
}

#[test]
fn test_consume_energy_zero_amount() {
    let (env, client, player) = setup();
    let ship_id = mint_test_ship(&env, &client, &player);

    client.initialize_energy(&ship_id, &100u32);
    // amount=0 must be rejected as InvalidAmount
    let result = client.try_consume_energy(&ship_id, &0u32);
    assert!(result.is_err());
}

#[test]
fn test_recharge_energy_success() {
    let (env, client, player) = setup();
    let ship_id = mint_test_ship(&env, &client, &player);

    client.initialize_energy(&ship_id, &0u32);
    // Default rate = 50%, so 200 * 50 / 100 = 100 energy gained
    let new_balance = client.recharge_energy(&ship_id, &200i128);
    assert_eq!(new_balance, 100);
}

#[test]
fn test_recharge_energy_with_efficiency_bonus() {
    let (env, client, player) = setup();
    let ship_id = mint_test_ship(&env, &client, &player);

    client.initialize_energy(&ship_id, &0u32);
    // Apply 30% bonus → effective rate = min(50 + 30, 100) = 80%
    client.apply_efficiency_bonus(&ship_id, &30u32);
    // 200 * 80 / 100 = 160 energy gained
    let new_balance = client.recharge_energy(&ship_id, &200i128);
    assert_eq!(new_balance, 160);
}

#[test]
fn test_recharge_energy_overflow_capped() {
    let (env, client, player) = setup();
    let ship_id = mint_test_ship(&env, &client, &player);

    // Start near u32::MAX
    client.initialize_energy(&ship_id, &(u32::MAX - 10));
    // Set rate to 100% for 1:1 resource-to-energy mapping
    client.set_base_recharge_rate(&100u32);
    // Recharge with a large amount — saturating_add must cap at u32::MAX
    let new_balance = client.recharge_energy(&ship_id, &1_000_000i128);
    assert_eq!(new_balance, u32::MAX);
}

#[test]
fn test_consume_recharge_cycle() {
    let (env, client, player) = setup();
    let ship_id = mint_test_ship(&env, &client, &player);

    // Initialize with 100
    client.initialize_energy(&ship_id, &100u32);

    // Consume 60 → 40 remaining
    let after_consume = client.consume_energy(&ship_id, &60u32);
    assert_eq!(after_consume, 40);

    // Recharge with 200 resources at default 50% → +100 → 140
    let after_recharge = client.recharge_energy(&ship_id, &200i128);
    assert_eq!(after_recharge, 140);

    // Consume 50 → 90 remaining
    let final_balance = client.consume_energy(&ship_id, &50u32);
    assert_eq!(final_balance, 90);
}

#[test]
fn test_burst_micro_consumptions() {
    let (env, client, player) = setup();
    let ship_id = mint_test_ship(&env, &client, &player);

    client.initialize_energy(&ship_id, &100u32);

    // 25 consecutive 1-unit consumptions — must complete without error
    for i in 0..25u32 {
        let remaining = client.consume_energy(&ship_id, &1u32);
        assert_eq!(remaining, 100 - (i + 1));
    }

    let energy = client.get_energy(&ship_id);
    assert_eq!(energy, 75);
}

#[test]
fn test_energy_for_nonexistent_ship() {
    let (_env, client, _player) = setup();

    // Ship 9999 was never minted — consume must fail with ShipNotFound
    let result = client.try_consume_energy(&9999u64, &10u32);
    assert!(result.is_err());
}

#[test]
fn test_set_base_recharge_rate() {
    let (env, client, player) = setup();
    let ship_id = mint_test_ship(&env, &client, &player);

    client.initialize_energy(&ship_id, &0u32);

    // Change rate from default 50% to 25%
    client.set_base_recharge_rate(&25u32);
    // 200 * 25 / 100 = 50 energy gained
    let balance = client.recharge_energy(&ship_id, &200i128);
    assert_eq!(balance, 50);
}
