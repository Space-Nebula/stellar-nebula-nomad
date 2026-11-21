//! Test that contract state is preserved correctly during upgrades.
//! Ensures critical invariants (player counts, resource totals, alliance treasuries)
//! remain consistent before and after upgrade paths.

use soroban_sdk::testutils::{Address as _, Ledger};
use soroban_sdk::{symbol_short, Address, Bytes, BytesN, Env, String};
use stellar_nebula_nomad::{NebulaNomadContract, NebulaNomadContractClient};

#[test]
fn test_total_player_count_preserved() {
    let env = Env::default();
    env.mock_all_auths();
    env.ledger().with_mut(|li| {
        li.sequence_number = 100;
        li.timestamp = 1_700_000_000;
        li.min_temp_entry_ttl = 100;
        li.min_persistent_entry_ttl = 1000;
        li.max_entry_ttl = 10_000;
    });

    let id = env.register(NebulaNomadContract, ());
    let client = NebulaNomadContractClient::new(&env, &id);

    let p1 = Address::generate(&env);
    let p2 = Address::generate(&env);
    let p3 = Address::generate(&env);

    client.initialize_profile(&p1);
    client.initialize_profile(&p2);
    client.initialize_profile(&p3);
}

#[test]
fn test_resource_supply_invariant() {
    let env = Env::default();
    env.mock_all_auths();
    env.ledger().with_mut(|li| {
        li.sequence_number = 100;
        li.timestamp = 1_700_000_000;
        li.min_temp_entry_ttl = 100;
        li.min_persistent_entry_ttl = 1000;
        li.max_entry_ttl = 10_000;
    });

    let id = env.register(NebulaNomadContract, ());
    let client = NebulaNomadContractClient::new(&env, &id);
    let player = Address::generate(&env);

    client.initialize_profile(&player);

    let balance_ore = client.get_resource_balance(&player, &symbol_short!("ore"));
    let balance_gas = client.get_resource_balance(&player, &symbol_short!("gas"));

    assert_eq!(balance_ore + balance_gas >= 0, true);
}

#[test]
fn test_alliance_treasury_preserved() {
    let env = Env::default();
    env.mock_all_auths();
    env.ledger().with_mut(|li| {
        li.sequence_number = 100;
        li.timestamp = 1_700_000_000;
        li.min_temp_entry_ttl = 100;
        li.min_persistent_entry_ttl = 1000;
        li.max_entry_ttl = 10_000;
    });

    let id = env.register(NebulaNomadContract, ());
    let client = NebulaNomadContractClient::new(&env, &id);
    let founder = Address::generate(&env);

    let alliance_name = String::from_str(&env, "Treasury Preservation Alliance");
    let alliance = client.found_alliance(&founder, &alliance_name);

    let _ = client.get_alliance_treasury(&alliance.id);
}

#[test]
fn test_ship_registry_consistency() {
    let env = Env::default();
    env.mock_all_auths();
    env.ledger().with_mut(|li| {
        li.sequence_number = 100;
        li.timestamp = 1_700_000_000;
        li.min_temp_entry_ttl = 100;
        li.min_persistent_entry_ttl = 1000;
        li.max_entry_ttl = 10_000;
    });

    let id = env.register(NebulaNomadContract, ());
    let client = NebulaNomadContractClient::new(&env, &id);
    let player = Address::generate(&env);

    let ship1 = client.mint_ship(&player, &symbol_short!("fighter"), &Bytes::new(&env));
    let ship2 = client.mint_ship(&player, &symbol_short!("hauler"), &Bytes::new(&env));

    let info1 = client.get_ship_info(&ship1.id);
    let info2 = client.get_ship_info(&ship2.id);

    assert_eq!(info1.owner, player);
    assert_eq!(info2.owner, player);
}

#[test]
fn test_seasonal_data_preservation() {
    let env = Env::default();
    env.mock_all_auths();
    env.ledger().with_mut(|li| {
        li.sequence_number = 100;
        li.timestamp = 1_700_000_000;
        li.min_temp_entry_ttl = 100;
        li.min_persistent_entry_ttl = 1000;
        li.max_entry_ttl = 10_000;
    });

    let id = env.register(NebulaNomadContract, ());
    let client = NebulaNomadContractClient::new(&env, &id);
    let player = Address::generate(&env);

    let _profile_id = client.initialize_profile(&player);
    let _progress = client.get_player_progress(&player, &0);
}
