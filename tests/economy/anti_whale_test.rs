#![cfg(test)]
//! Anti-whale mechanisms through the contract client: scan and mint caps,
//! diminishing returns on harvests, the trade cap and progressive fee, the
//! guild contribution cap, tiered leaderboards and governance overrides.

use soroban_sdk::testutils::{Address as _, Ledger};
use soroban_sdk::{symbol_short, Address, Bytes, BytesN, Env, Error, InvokeError, String, Symbol};
use stellar_nebula_nomad::governance::GovernanceDataKey;
use stellar_nebula_nomad::{
    diminishing_returns, AllianceError, AmmError, AntiWhaleConfig, AntiWhaleError,
    NebulaNomadContract, NebulaNomadContractClient, OpKind, PlayerTier,
};

const DAY: u64 = 86_400;

fn setup() -> (Env, NebulaNomadContractClient<'static>, Address, Address) {
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
    let admin = Address::generate(&env);
    client.init_anti_whale_admin(&admin);
    (env, client, id, admin)
}

fn advance(env: &Env, secs: u64) {
    env.ledger().with_mut(|li| {
        li.timestamp += secs;
        li.sequence_number += u32::try_from(secs / 5).unwrap_or(1).max(1);
    });
}

fn seed(env: &Env, byte: u8) -> BytesN<32> {
    BytesN::from_array(env, &[byte; 32])
}

/// Error code a capped `scan_nebula` traps with (no `Result` channel).
fn capped() -> Error {
    Error::from_contract_error(AntiWhaleError::OperationCapExceeded as u32)
}

/// The same code as seen through a `Result<_, ShipError>` entrypoint: the
/// code is not a `ShipError`, so the client reports it as a raw contract
/// error.
fn capped_invoke() -> InvokeError {
    InvokeError::Contract(AntiWhaleError::OperationCapExceeded as u32)
}

fn assert_scan_capped(
    result: Result<
        Result<
            (
                stellar_nebula_nomad::NebulaLayout,
                stellar_nebula_nomad::Rarity,
            ),
            Error,
        >,
        Result<Error, InvokeError>,
    >,
) {
    match result {
        Err(Ok(e)) => assert_eq!(e, capped()),
        Err(Err(e)) => panic!("unexpected invoke error {e:?}"),
        Ok(_) => panic!("scan should have been capped"),
    }
}

#[test]
fn scan_cap_refuses_the_extra_scan_and_resets_next_day() {
    let (env, client, _, admin) = setup();
    let player = Address::generate(&env);
    client.set_anti_whale_config(
        &admin,
        &AntiWhaleConfig {
            max_scans_per_day: 2,
            ..AntiWhaleConfig::defaults()
        },
    );
    assert_eq!(client.get_anti_whale_config().max_scans_per_day, 2);

    client.scan_nebula(&seed(&env, 1), &player);
    client.scan_nebula(&seed(&env, 2), &player);
    assert_eq!(client.get_user_daily_ops(&player, &OpKind::Scan), 2);
    assert_scan_capped(client.try_scan_nebula(&seed(&env, 3), &player));
    assert_eq!(client.get_player_activity(&player), 2);

    advance(&env, DAY);
    client.scan_nebula(&seed(&env, 4), &player);
    assert_eq!(client.get_user_daily_ops(&player, &OpKind::Scan), 1);
}

#[test]
fn mint_cap_counts_every_ship_in_a_batch() {
    let (env, client, _, admin) = setup();
    let player = Address::generate(&env);
    client.set_anti_whale_config(
        &admin,
        &AntiWhaleConfig {
            max_mints_per_day: 2,
            ..AntiWhaleConfig::defaults()
        },
    );
    let three = soroban_sdk::vec![
        &env,
        symbol_short!("fighter"),
        symbol_short!("explorer"),
        symbol_short!("hauler"),
    ];
    assert_eq!(
        client
            .try_batch_mint_ships(&player, &three, &Bytes::new(&env))
            .err(),
        Some(Err(capped_invoke()))
    );
    // The refused batch consumed nothing.
    client.mint_ship(&player, &symbol_short!("fighter"), &Bytes::new(&env));
    client.mint_ship(&player, &symbol_short!("explorer"), &Bytes::new(&env));
    assert_eq!(
        client
            .try_mint_ship(&player, &symbol_short!("hauler"), &Bytes::new(&env))
            .err(),
        Some(Err(capped_invoke()))
    );
    assert_eq!(client.get_user_daily_ops(&player, &OpKind::Mint), 2);
    assert_eq!(client.get_player_activity(&player), 10);
}

#[test]
fn harvests_are_reduced_by_daily_diminishing_returns() {
    let (env, client, _, admin) = setup();
    let player = Address::generate(&env);
    client.set_anti_whale_config(
        &admin,
        &AntiWhaleConfig {
            tier_width: 500,
            ..AntiWhaleConfig::defaults()
        },
    );
    let ship = client.mint_ship(&player, &symbol_short!("fighter"), &Bytes::new(&env));
    let layout = client.generate_nebula_layout(&seed(&env, 7), &player);

    let harvest = client.harvest_resources(&ship.id, &layout);
    let raw = client.get_user_daily_gathered(&player);
    assert!(raw > 500, "a full layout gathers well past one band: {raw}");
    let expected = diminishing_returns(0, raw, 500);
    let credited = u64::from(harvest.total_harvested);
    // Pro-rata flooring across cells loses at most one unit per cell.
    assert!(credited <= expected, "{credited} > {expected}");
    assert!(credited + u64::from(harvest.resources.len()) >= expected);
    let stats = client.get_anti_whale_impact();
    assert_eq!(stats.units_gathered_raw, raw);
    assert_eq!(stats.units_gathered_effective, expected);

    // The second harvest of the day starts deeper in the bands.
    let second = client.harvest_resources(&ship.id, &layout);
    assert!(second.total_harvested < harvest.total_harvested);

    // Exempt accounts are credited in full.
    client.set_anti_whale_exempt(&admin, &player, &true);
    let before = client.get_user_daily_gathered(&player);
    let full = client.harvest_resources(&ship.id, &layout);
    assert_eq!(client.get_user_daily_gathered(&player), before);
    assert!(full.total_harvested > harvest.total_harvested);
}

#[test]
fn trade_cap_and_progressive_fee_apply_to_swaps() {
    let (env, client, _, admin) = setup();
    let provider = Address::generate(&env);
    let trader = Address::generate(&env);
    let gas = symbol_short!("gas");
    let ore = symbol_short!("ore");
    let pool_id = client.create_pool(&provider, &ore, &gas);
    client.add_liquidity(&provider, &pool_id, &10_000_000, &10_000_000);
    let route = soroban_sdk::vec![&env, pool_id];

    // A trade below the first fee band pays only the pool fee.
    let quoted = client.quote_swap(&pool_id, &ore, &1_000);
    assert_eq!(
        client.swap_exact_input(&trader, &ore, &1_000, &1, &route),
        quoted
    );
    assert_eq!(
        client.get_user_daily_trade_volume(&trader),
        u64::try_from(quoted).unwrap()
    );
    assert_eq!(client.get_user_daily_ops(&trader, &OpKind::Trade), 1);

    // Push the trader's daily volume into the 50 bps band: the extra fee is
    // taken from the output and left in the pool.
    let quoted = client.quote_swap(&pool_id, &ore, &200_000);
    let reserve_before = client.get_pool(&pool_id).unwrap().reserve_b;
    let received = client.swap_exact_input(&trader, &ore, &200_000, &1, &route);
    assert!(received < quoted, "{received} >= {quoted}");
    let pool_after = client.get_pool(&pool_id).unwrap();
    assert_eq!(reserve_before - pool_after.reserve_b, received);
    let fees = client.get_anti_whale_impact().progressive_fees;
    assert_eq!(fees, u64::try_from(quoted - received).unwrap());
    assert!(fees > 0);

    client.set_anti_whale_config(
        &admin,
        &AntiWhaleConfig {
            max_trades_per_day: 2,
            ..AntiWhaleConfig::defaults()
        },
    );
    assert_eq!(
        client.try_swap_exact_input(&trader, &ore, &10, &1, &route),
        Err(Ok(AmmError::DailyTradeCapExceeded))
    );
}

#[test]
fn guild_contributions_are_capped_per_member_per_day() {
    let (env, client, _, admin) = setup();
    let member = Address::generate(&env);
    client.set_anti_whale_config(
        &admin,
        &AntiWhaleConfig {
            guild_daily_cap: 1_000,
            ..AntiWhaleConfig::defaults()
        },
    );
    let alliance_id = client.found_alliance(&member, &String::from_str(&env, "Nomads"));
    client.contribute_to_treasury(&member, &600);
    client.contribute_to_treasury(&member, &400);
    assert_eq!(client.get_alliance_treasury(&alliance_id), 1_000);
    assert_eq!(client.get_daily_guild_contribution(&member), 1_000);
    assert_eq!(
        client.try_contribute_to_treasury(&member, &1),
        Err(Ok(AllianceError::ContributionCapExceeded))
    );
    advance(&env, DAY);
    client.contribute_to_treasury(&member, &1);
    assert_eq!(client.get_alliance_treasury(&alliance_id), 1_001);
}

#[test]
fn scores_land_on_the_board_of_the_players_tier() {
    let (env, client, _, admin) = setup();
    let casual = Address::generate(&env);
    let whale = Address::generate(&env);
    client.set_anti_whale_config(
        &admin,
        &AntiWhaleConfig {
            dedicated_threshold: 5,
            hardcore_threshold: 10,
            ..AntiWhaleConfig::defaults()
        },
    );
    // Two mints = 10 activity points = hardcore under this config.
    client.mint_ship(&whale, &symbol_short!("fighter"), &Bytes::new(&env));
    client.mint_ship(&whale, &symbol_short!("fighter"), &Bytes::new(&env));
    assert_eq!(client.get_player_tier(&whale), PlayerTier::Hardcore);
    assert_eq!(client.get_player_tier(&casual), PlayerTier::Casual);

    let category = Symbol::new(&env, "scans");
    let period = Symbol::new(&env, "all_time");
    client.update_leaderboard_score(&casual, &category, &period, &10);
    client.update_leaderboard_score(&whale, &category, &period, &9_999);
    client.update_leaderboard_score(&casual, &category, &period, &12);

    let casual_board = client.get_tier_leaderboard(&PlayerTier::Casual, &10);
    assert_eq!(casual_board.len(), 1);
    assert_eq!(casual_board.get(0).unwrap().player, casual);
    assert_eq!(casual_board.get(0).unwrap().score, 12);
    let hardcore_board = client.get_tier_leaderboard(&PlayerTier::Hardcore, &10);
    assert_eq!(hardcore_board.len(), 1);
    assert_eq!(hardcore_board.get(0).unwrap().player, whale);
    assert!(client
        .get_tier_leaderboard(&PlayerTier::Dedicated, &10)
        .is_empty());
    // The shared board still ranks everyone together.
    let shared = client.get_leaderboard(&category, &period, &10);
    assert_eq!(shared.len(), 2);
}

#[test]
fn admin_gating_and_validation() {
    let (env, client, _, admin) = setup();
    let stranger = Address::generate(&env);
    assert_eq!(
        client.try_init_anti_whale_admin(&stranger),
        Err(Ok(AntiWhaleError::AlreadyInitialized))
    );
    assert_eq!(
        client.try_set_anti_whale_config(&stranger, &AntiWhaleConfig::defaults()),
        Err(Ok(AntiWhaleError::Unauthorized))
    );
    assert_eq!(
        client.try_set_anti_whale_cap(&stranger, &1),
        Err(Ok(AntiWhaleError::Unauthorized))
    );
    assert_eq!(
        client.try_set_anti_whale_exempt(&stranger, &stranger, &true),
        Err(Ok(AntiWhaleError::Unauthorized))
    );
    assert_eq!(
        client.try_set_anti_whale_config(
            &admin,
            &AntiWhaleConfig {
                max_scans_per_day: 0,
                ..AntiWhaleConfig::defaults()
            }
        ),
        Err(Ok(AntiWhaleError::InvalidConfig))
    );
    client.set_anti_whale_cap(&admin, &42);
    assert_eq!(client.get_anti_whale_cap(), 42);
    assert!(!client.is_anti_whale_exempt(&stranger));
    client.set_anti_whale_exempt(&admin, &stranger, &true);
    assert!(client.is_anti_whale_exempt(&stranger));
}

#[test]
fn governance_parameters_override_admin_limits() {
    let (env, client, id, admin) = setup();
    client.set_anti_whale_config(
        &admin,
        &AntiWhaleConfig {
            max_scans_per_day: 50,
            ..AntiWhaleConfig::defaults()
        },
    );
    // `governance::set_game_parameter` is DAO-only; write the parameter the
    // way an executed proposal would land it.
    env.as_contract(&id, || {
        env.storage().instance().set(
            &GovernanceDataKey::GameParameter(symbol_short!("aw_scans")),
            &3i128,
        );
    });
    let cfg = client.get_anti_whale_config();
    assert_eq!(cfg.max_scans_per_day, 3, "governance wins");
    assert_eq!(
        cfg.max_mints_per_day,
        AntiWhaleConfig::defaults().max_mints_per_day
    );

    let player = Address::generate(&env);
    for byte in 1..=3u8 {
        client.scan_nebula(&seed(&env, byte), &player);
    }
    assert_scan_capped(client.try_scan_nebula(&seed(&env, 9), &player));
}
