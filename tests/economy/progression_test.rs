#![cfg(test)]
//! Ship progression through the contract client: the rebalanced level
//! ladder, the on-chain simulator, the level upgrade flow, admin retuning,
//! and the interplay with anti-whale diminishing returns.

use soroban_sdk::testutils::{Address as _, Ledger};
use soroban_sdk::{symbol_short, Address, Bytes, Env, Map, Symbol};
use stellar_nebula_nomad::resource_minter::credit_resource_balance;
use stellar_nebula_nomad::{
    cumulative_cost, days_to_level, diminishing_returns, CurveKind, NebulaNomadContract,
    NebulaNomadContractClient, PlayStyle, ProgressionCurve, ShipUpgradeError, UpgradeBlueprint,
    DEFAULT_TIER_WIDTH, MAX_LEVEL, UNITS_PER_HOUR,
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

const LADDER: [u64; 20] = [
    1_449, 1_949, 2_649, 3_549, 4_799, 6_499, 8_749, 11_849, 15_999, 21_549, 29_099, 39_299,
    53_049, 71_649, 96_699, 130_549, 176_249, 237_899, 321_199, 433_599,
];

#[test]
fn contract_quotes_the_rebalanced_ladder() {
    let (_, client, _) = setup();
    let curve = client.get_progression_curve();
    assert_eq!(curve, ProgressionCurve::default_rebalanced());
    for (i, want) in LADDER.iter().enumerate() {
        let level = u32::try_from(i + 1).unwrap();
        assert_eq!(client.level_cost_at(&level), *want, "level {level}");
        assert!(want % 100 == 49 || want % 100 == 99, "psychological price");
    }
    assert_eq!(client.level_cost_at(&0), 0);
    assert_eq!(client.level_cost_at(&(MAX_LEVEL + 1)), 0);
    assert_eq!(
        cumulative_cost(&curve, MAX_LEVEL),
        LADDER.iter().sum::<u64>()
    );
}

#[test]
fn on_chain_simulator_meets_every_retention_target() {
    let (_, client, _) = setup();
    let report = client.simulate_progression();
    assert_eq!(report.casual_level_5, 4);
    assert!(report.casual_level_5 <= 7);
    assert_eq!(report.casual_level_10, 22);
    assert!(report.casual_level_10 <= 30);
    assert_eq!(report.regular_level_5, 2);
    assert!(report.regular_level_5 <= 3);
    assert_eq!(report.regular_level_10, 11);
    assert!(report.regular_level_10 <= 14);
    assert_eq!(report.hardcore_level_5, 1);
    assert_eq!(report.hardcore_level_10, 6);
    assert!(report.hardcore_level_10 <= 7);
    assert_eq!(report.hardcore_max_level, 115);
    assert!(report.hardcore_max_level >= 60, "max level takes months");
    assert_eq!(report.total_cost_to_max, 1_668_330);
}

#[test]
fn level_upgrade_burns_the_quoted_cost_and_tracks_the_sink() {
    let (env, client, id) = setup();
    let owner = Address::generate(&env);
    let other = Address::generate(&env);
    let ship = client.mint_ship(&owner, &symbol_short!("fighter"), &Bytes::new(&env));

    assert_eq!(client.get_ship_level(&ship.id), 0);
    assert_eq!(client.level_upgrade_cost(&ship.id), 1_449);
    assert_eq!(
        client.try_upgrade_ship_level(&owner, &ship.id, &ore()),
        Err(Ok(ShipUpgradeError::InsufficientResources))
    );

    give(&env, &id, &owner, &ore(), 1_449 + 1_949);
    let result = client.upgrade_ship_level(&owner, &ship.id, &ore());
    assert_eq!(result.level, 1);
    assert_eq!(result.cost, 1_449);
    assert_eq!(result.asset_id, ore());
    assert_eq!(client.get_ship_level(&ship.id), 1);
    assert_eq!(client.get_resource_balance(&owner, &ore()), 1_949);
    assert_eq!(client.level_upgrade_cost(&ship.id), 1_949);
    assert_eq!(client.get_total_level_spend(), 1_449);

    assert_eq!(
        client.try_upgrade_ship_level(&other, &ship.id, &ore()),
        Err(Ok(ShipUpgradeError::NotShipOwner))
    );
    assert_eq!(
        client.try_level_upgrade_cost(&0),
        Err(Ok(ShipUpgradeError::InvalidShipId))
    );

    advance(&env, 400);
    let result = client.upgrade_ship_level(&owner, &ship.id, &ore());
    assert_eq!(result.level, 2);
    assert_eq!(client.get_resource_balance(&owner, &ore()), 0);
    assert_eq!(client.get_total_level_spend(), 1_449 + 1_949);
}

#[test]
fn ladder_stops_at_max_level() {
    let (env, client, id) = setup();
    let owner = Address::generate(&env);
    let ship = client.mint_ship(&owner, &symbol_short!("hauler"), &Bytes::new(&env));
    give(&env, &id, &owner, &ore(), 1_668_330);
    for level in 1..=MAX_LEVEL {
        // Stay under the 3-per-5-minutes upgrade rate limit.
        advance(&env, 400);
        assert_eq!(
            client.upgrade_ship_level(&owner, &ship.id, &ore()).level,
            level
        );
    }
    assert_eq!(client.get_resource_balance(&owner, &ore()), 0);
    assert_eq!(client.get_total_level_spend(), 1_668_330);
    assert_eq!(
        client.try_level_upgrade_cost(&ship.id),
        Err(Ok(ShipUpgradeError::MaxLevelReached))
    );
    advance(&env, 400);
    give(&env, &id, &owner, &ore(), 1_000_000);
    assert_eq!(
        client.try_upgrade_ship_level(&owner, &ship.id, &ore()),
        Err(Ok(ShipUpgradeError::MaxLevelReached))
    );
}

#[test]
fn upgrade_admin_can_retune_the_curve() {
    let (env, client, _) = setup();
    let admin = Address::generate(&env);
    let stranger = Address::generate(&env);
    let mut blueprints: Map<Symbol, UpgradeBlueprint> = Map::new(&env);
    blueprints.set(
        symbol_short!("thruster"),
        UpgradeBlueprint {
            asset_id: ore(),
            resource_cost: 100,
            mass: 10,
            scanner_bonus: 1,
            hull_bonus: 1,
            regen_bonus: 0,
        },
    );
    client.init_upgrade_config(&admin, &blueprints);

    let linear = ProgressionCurve::default_rebalanced().with_kind(CurveKind::Linear);
    assert_eq!(
        client.try_set_progression_curve(&stranger, &linear),
        Err(Ok(ShipUpgradeError::NotInitialized))
    );
    let broken = ProgressionCurve {
        base_cost: 0,
        ..linear
    };
    assert_eq!(
        client.try_set_progression_curve(&admin, &broken),
        Err(Ok(ShipUpgradeError::InvalidProgressionCurve))
    );

    client.set_progression_curve(&admin, &linear);
    assert_eq!(client.get_progression_curve(), linear);
    // Linear: 1449 + 507 per level -> 1956 -> snapped to 1949.
    assert_eq!(client.level_cost_at(&2), 1_949);
    assert_eq!(client.level_cost_at(&20), 11_099);
    let report = client.simulate_progression();
    assert!(report.hardcore_max_level < 60, "linear curve is too flat");
}

/// The anti-whale diminishing-returns bands start at 120 000 units gathered
/// per day across all assets. A hardcore session gathers about 114 000, so
/// none of the three modelled play styles loses income to the bands and the
/// simulator's targets hold as published.
#[test]
fn diminishing_returns_do_not_touch_the_modelled_play_styles() {
    const UNITS_PER_HARVEST_ALL_ASSETS: u64 = 2_848;
    const ORE_PER_HARVEST: u64 = 365;
    let curve = ProgressionCurve::default_rebalanced();
    for style in PlayStyle::all() {
        let raw = style.hours_per_day() * 10 * UNITS_PER_HARVEST_ALL_ASSETS;
        assert_eq!(
            diminishing_returns(0, raw, DEFAULT_TIER_WIDTH),
            raw,
            "{style:?}"
        );
        assert_eq!(style.daily_income(), style.hours_per_day() * UNITS_PER_HOUR);
        let ore_per_day = raw * ORE_PER_HARVEST / UNITS_PER_HARVEST_ALL_ASSETS;
        // The model's per-asset income is within rounding of the derived one.
        assert!(ore_per_day.abs_diff(style.daily_income()) <= 4 * style.hours_per_day());
    }
    let hardcore = PlayStyle::Hardcore.daily_income();
    assert_eq!(cumulative_cost(&curve, 5).div_ceil(hardcore), 1);
    assert_eq!(cumulative_cost(&curve, 10).div_ceil(hardcore), 6);
    // Farming two hardcore sessions a day is where the bands start to bite.
    let double = 2 * PlayStyle::Hardcore.hours_per_day() * 10 * UNITS_PER_HARVEST_ALL_ASSETS;
    assert!(diminishing_returns(0, double, DEFAULT_TIER_WIDTH) < double);
    assert_eq!(days_to_level(&curve, PlayStyle::Casual, 5), 4);
    assert_eq!(DAY, 86_400);
}
