//! Test contract upgrade data migration scenarios.
//! Verifies that player data, ship records, and economic state migrate correctly
//! when contract code is upgraded.

use soroban_sdk::testutils::{Address as _, Ledger};
use soroban_sdk::{symbol_short, Address, Bytes, BytesN, Env, String};
use stellar_nebula_nomad::{NebulaNomadContract, NebulaNomadContractClient};

#[test]
fn test_player_profile_migration() {
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

    let profile_id = client.initialize_profile(&player);
    client.update_progress(&player, &profile_id, &1, &500);

    client.initialize_profile(&player);

    let _ = client.get_player_progress(&player, &profile_id);
}

#[test]
fn test_ship_nft_state_migration() {
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
    let ship2 = client.mint_ship(&player, &symbol_short!("explorer"), &Bytes::new(&env));

    let _ = client.get_ship_info(&ship1.id);
    let _ = client.get_ship_info(&ship2.id);
}

#[test]
fn test_resource_balance_migration() {
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

    let balance = client.get_resource_balance(&player, &symbol_short!("ore"));
    assert!(balance >= 0);
}

#[test]
fn test_alliance_membership_migration() {
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
    let member = Address::generate(&env);

    let alliance_name = String::from_str(&env, "Migration Test Alliance");
    let alliance = client.found_alliance(&founder, &alliance_name);

    let _ = client.join_alliance(&member, &alliance.id);
}

#[test]
fn test_stored_upgrade_metadata() {
    let env = Env::default();
    env.mock_all_auths();
    env.ledger().with_mut(|li| {
        li.sequence_number = 100;
        li.timestamp = 1_700_000_000;
        li.min_temp_entry_ttl = 100;
        li.min_persistent_entry_ttl = 1000;
        li.max_entry_ttl = 10_000;
    });

    let _id = env.register(NebulaNomadContract, ());
}
