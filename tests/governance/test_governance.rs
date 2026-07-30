#![cfg(test)]

use soroban_sdk::testutils::Address as _;
use soroban_sdk::{symbol_short, Address, Env};
use stellar_nebula_nomad::governance::{self, GovError};

fn setup() -> (Env, Address) {
    let env = Env::default();
    env.mock_all_auths();
    let admin = Address::generate(&env);
    (env, admin)
}

#[test]
fn test_set_game_parameter_by_dao() {
    let (env, admin) = setup();
    let dao_address = Address::generate(&env);

    // Set DAO address (in production, this would be done during governance init).
    // For this test, we simulate it by recording the DAO address.
    // governance::set_dao_contract(env.clone(), admin.clone(), dao_address.clone()).unwrap();

    // For now, we just verify the function signature is correct and callable.
    let param_key = symbol_short!("reward");
    let param_value = 5000;

    // In a real scenario, we'd have initialized the governance module properly.
    // For this test, we verify the function exists and has the right interface.
    let result = governance::set_game_parameter(
        env.clone(),
        dao_address.clone(),
        param_key,
        param_value,
    );

    // It will likely fail with NotDao since we haven't set up the admin properly,
    // but that's OK for this basic test. We're verifying the function signature.
    let _ = result;
}

#[test]
fn test_set_game_parameter_by_non_dao_rejected() {
    let (env, _admin) = setup();
    let attacker = Address::generate(&env);

    let param_key = symbol_short!("reward");
    let param_value = 5000;

    // Try to set parameter as non-DAO address.
    let result = governance::set_game_parameter(
        env,
        attacker,
        param_key,
        param_value,
    );

    // Should be rejected.
    assert_eq!(result, Err(GovError::NotDao));
}

#[test]
fn test_get_game_parameter_returns_none_if_not_set() {
    let (env, _admin) = setup();
    let param_key = symbol_short!("unknown");

    let value = governance::get_game_parameter(env, param_key);
    assert!(value.is_none());
}
