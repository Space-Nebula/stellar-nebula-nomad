//! Test backward compatibility of upgraded contracts.
//! Ensures old contract calls still work with new code and new entry points
//! don't break existing functionality.

use soroban_sdk::testutils::{Address as _, Ledger};
use soroban_sdk::{symbol_short, Address, Bytes, BytesN, Env, String};
use stellar_nebula_nomad::{NebulaNomadContract, NebulaNomadContractClient};

#[test]
fn test_old_ship_mint_path_compatibility() {
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

    let ship = client.mint_ship(&player, &symbol_short!("fighter"), &Bytes::new(&env));
    assert!(ship.id > 0);
}

#[test]
fn test_old_resource_query_compatibility() {
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
    let balance = client.get_resource_balance(&player, &symbol_short!("ore"));
    assert!(balance >= 0);
}

#[test]
fn test_old_alliance_creation_path_compatibility() {
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

    let name = String::from_str(&env, "Old Path Alliance");
    let alliance = client.found_alliance(&founder, &name);
    assert!(alliance.id > 0);
}

#[test]
fn test_old_ship_info_query_compatibility() {
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

    let ship = client.mint_ship(&player, &symbol_short!("explorer"), &Bytes::new(&env));
    let info = client.get_ship_info(&ship.id);
    assert_eq!(info.owner, player);
}

#[test]
fn test_new_query_additions_dont_break_old() {
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
    let _ = client.get_player_progress(&player, &0);
}

#[test]
fn test_parameter_signature_preservation() {
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
    let seed = BytesN::from_array(&env, &[1u8; 32]);

    let _ = client.generate_nebula_layout(&seed, &player);
}
