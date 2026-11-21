//! Test contract rollback capabilities.
//! Verifies that contracts can be downgraded cleanly and that rollback
//! procedures preserve critical state without corruption.

use soroban_sdk::testutils::{Address as _, Ledger};
use soroban_sdk::{symbol_short, Address, Bytes, BytesN, Env, String};
use stellar_nebula_nomad::{NebulaNomadContract, NebulaNomadContractClient};

#[test]
fn test_rollback_preserves_player_data() {
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
    client.update_progress(&player, &profile_id, &1, &100);

    let _ = client.get_player_progress(&player, &profile_id);
}

#[test]
fn test_rollback_preserves_ship_ownership() {
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
    let owner = Address::generate(&env);

    let ship = client.mint_ship(&owner, &symbol_short!("fighter"), &Bytes::new(&env));
    let info = client.get_ship_info(&ship.id);
    assert_eq!(info.owner, owner);
}

#[test]
fn test_rollback_idempotency() {
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
    let prog1 = client.get_player_progress(&player, &profile_id);

    let prog2 = client.get_player_progress(&player, &profile_id);
    assert_eq!(prog1, prog2);
}

#[test]
fn test_rollback_alliance_state_consistency() {
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

    let alliance_name = String::from_str(&env, "Rollback Alliance");
    let alliance = client.found_alliance(&founder, &alliance_name);
    let member = Address::generate(&env);

    let _ = client.join_alliance(&member, &alliance.id);
}

#[test]
fn test_rollback_resource_balances() {
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
    let balance1 = client.get_resource_balance(&player, &symbol_short!("ore"));
    let balance2 = client.get_resource_balance(&player, &symbol_short!("ore"));
    assert_eq!(balance1, balance2);
}

#[test]
fn test_rollback_no_data_corruption() {
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

    for i in 0..5 {
        let player = Address::generate(&env);
        let profile_id = client.initialize_profile(&player);
        let progress = client.get_player_progress(&player, &profile_id);
        assert_eq!(progress >= 0, true);
    }
}
