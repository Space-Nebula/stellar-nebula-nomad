//! Fuzz testing for navigation planner module

#![cfg(test)]

use proptest::prelude::*;
use soroban_sdk::testutils::{Address as _, Ledger, LedgerInfo};
use soroban_sdk::{Address, Env, Vec};
use stellar_nebula_nomad::{NavError, NebulaNomadContract, NebulaNomadContractClient};

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
        min_persistent_entry_ttl: 1_000,
        max_entry_ttl: 10_000,
    });
    let contract_id = env.register(NebulaNomadContract, ());
    let client = NebulaNomadContractClient::new(&env, &contract_id);
    let admin = Address::generate(&env);
    (env, client, admin)
}

proptest! {
    #[test]
    fn prop_same_nebula_rejected(nebula_id in 1u64..1000u64) {
        let (_, client, admin) = setup();
        client.initialize_nav_graph(&admin);
        let result = client.try_calculate_optimal_route(&nebula_id, &nebula_id);
        prop_assert!(matches!(result, Err(Ok(NavError::SameNebula))));
    }

    #[test]
    fn prop_fuel_cost_non_negative(fuel in 0u32..10000u32) {
        let (_, client, admin) = setup();
        client.initialize_nav_graph(&admin);
        client.add_nebula_connection(&admin, &1u64, &2u64, &fuel, &50u32);
        let conn = client.get_nav_connection(&1u64, &2u64);
        prop_assert!(conn.is_some());
        prop_assert_eq!(conn.unwrap().fuel_cost, fuel);
    }

    #[test]
    fn prop_hazard_clamped_at_100(hazard in 0u32..200u32) {
        let (_, client, admin) = setup();
        client.initialize_nav_graph(&admin);
        client.add_nebula_connection(&admin, &1u64, &2u64, &100u32, &hazard);
        let conn = client.get_nav_connection(&1u64, &2u64);
        prop_assert!(conn.is_some());
        prop_assert!(conn.unwrap().hazard_level <= 100);
    }
}

#[test]
fn fuzz_route_validation_empty() {
    let (env, client, admin) = setup();
    client.initialize_nav_graph(&admin);
    let empty_route = Vec::<u64>::new(&env);
    let result = client.try_validate_route_safety(&empty_route);
    assert!(matches!(result, Err(Ok(NavError::RouteEmpty))));
}

#[test]
fn fuzz_batch_connections_limit() {
    let (env, client, admin) = setup();
    client.initialize_nav_graph(&admin);
    let mut edges = Vec::new(&env);
    for i in 0..25 {
        edges.push_back(stellar_nebula_nomad::RouteEdge {
            from: i,
            to: i + 1,
            fuel_cost: 100,
            hazard_level: 10,
        });
    }
    let result = client.try_add_nebula_connections_batch(&admin, &edges);
    assert!(matches!(result, Err(Ok(NavError::BatchTooLarge))));
}
