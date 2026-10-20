#![cfg(test)]
//! Predictive yield forecasting (Issue #90): data-sufficiency gating, model
//! updates, deterministic output, the forecast cache, and history capping.

use soroban_sdk::testutils::Address as _;
use soroban_sdk::{symbol_short, Address, Env};
use stellar_nebula_nomad::{
    NebulaNomadContract, NebulaNomadContractClient, YieldDataPoint, MAX_HISTORY_POINTS,
};

/// `ModelParams::default().moving_average_window`: the minimum history needed.
const WINDOW: u32 = 7;

fn setup() -> (Env, NebulaNomadContractClient<'static>, Address) {
    let env = Env::default();
    env.mock_all_auths();
    let contract = env.register(NebulaNomadContract, ());
    let client = NebulaNomadContractClient::new(&env, &contract);
    let admin = Address::generate(&env);
    client.initialize_forecast(&admin);
    (env, client, admin)
}

fn point(i: u32) -> YieldDataPoint {
    YieldDataPoint {
        timestamp: 1_000 + u64::from(i),
        yield_amount: 100 + i128::from(i) * 10,
        source: symbol_short!("harvest"),
    }
}

#[test]
fn forecast_requires_a_full_moving_average_window() {
    let (env, client, _admin) = setup();
    let player = Address::generate(&env);

    for i in 0..WINDOW - 1 {
        client.update_forecast_model(&player, &point(i));
    }
    assert!(client.try_generate_yield_forecast(&player, &30).is_err());

    client.update_forecast_model(&player, &point(WINDOW - 1));
    assert!(client.try_generate_yield_forecast(&player, &30).is_ok());
}

#[test]
fn forecast_is_deterministic_and_cached() {
    let (env, client, _admin) = setup();
    let player = Address::generate(&env);
    for i in 0..WINDOW {
        client.update_forecast_model(&player, &point(i));
    }

    let first = client.generate_yield_forecast(&player, &30);
    let second = client.generate_yield_forecast(&player, &30);

    assert_eq!(first.player, player);
    assert_eq!(first.forecast_days, 30);
    // Rising yields should project a positive forecast.
    assert!(first.predicted_yield > 0);
    assert_eq!(first, second);
    assert_eq!(client.get_cached_forecast(&player), Some(first));
}

#[test]
fn forecast_rejects_out_of_range_horizons() {
    let (env, client, _admin) = setup();
    let player = Address::generate(&env);
    for i in 0..WINDOW {
        client.update_forecast_model(&player, &point(i));
    }

    assert!(client.try_generate_yield_forecast(&player, &0).is_err());
    assert!(client.try_generate_yield_forecast(&player, &366).is_err());
}

#[test]
fn history_is_capped_at_max_points() {
    let (env, client, _admin) = setup();
    let player = Address::generate(&env);

    for i in 0..(MAX_HISTORY_POINTS + 5) {
        client.update_forecast_model(&player, &point(i));
    }

    assert_eq!(client.get_history_count(&player), MAX_HISTORY_POINTS);
}

#[test]
fn model_update_does_not_touch_other_players() {
    let (env, client, _admin) = setup();
    let player = Address::generate(&env);
    let bystander = Address::generate(&env);

    for i in 0..WINDOW {
        client.update_forecast_model(&player, &point(i));
    }

    assert_eq!(client.get_history_count(&player), WINDOW);
    assert_eq!(client.get_history_count(&bystander), 0);
    assert!(client.get_cached_forecast(&bystander).is_none());
}
