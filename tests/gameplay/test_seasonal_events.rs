#![cfg(test)]

use soroban_sdk::{
    symbol_short,
    testutils::{Address as _, Ledger},
    Address, Env, String, Symbol,
};
use stellar_nebula_nomad::{
    seasonal_event_reward_pool, EventError, NebulaNomadContract, NebulaNomadContractClient,
    SeasonalChallengeSpec, SeasonalEventConfig, SeasonalEventStatus, MAX_CHALLENGES_PER_EVENT,
    MAX_PENDING_SEASONAL_EVENTS, MAX_SEASONAL_EVENT_DURATION, MIN_SEASONAL_EVENT_DURATION,
    SEASONAL_REWARD_CLAIM_WINDOW,
};

const HOUR: u64 = 60 * 60;
const DAY: u64 = 24 * HOUR;
const T0: u64 = 1_000;

fn setup() -> (Env, NebulaNomadContractClient<'static>, Address) {
    let env = Env::default();
    env.mock_all_auths();
    env.ledger().set_timestamp(T0);
    let contract_id = env.register(NebulaNomadContract, ());
    let client = NebulaNomadContractClient::new(&env, &contract_id);
    let admin = Address::generate(&env);
    client.initialize_scheduler(&admin);
    client.init_season(&admin, &String::from_str(&env, "Season 1"));
    (env, client, admin)
}

fn config(
    category: Symbol,
    start: u64,
    duration: u64,
    cooldown: u64,
    exclusive: bool,
) -> SeasonalEventConfig {
    SeasonalEventConfig {
        category,
        start_time: start,
        duration_secs: duration,
        cooldown_secs: cooldown,
        reward_pool: 10_000,
        exclusive,
    }
}

// ─── Scheduling & activation ─────────────────────────────────────────────────

#[test]
fn test_full_event_lifecycle() {
    let (env, client, admin) = setup();
    let start = T0 + HOUR;
    let id = client.schedule_seasonal_event(
        &admin,
        &config(symbol_short!("boss"), start, DAY, 2 * DAY, false),
    );
    assert_eq!(id, 1);

    let event = client.get_seasonal_event(&id);
    assert_eq!(event.status, SeasonalEventStatus::Scheduled);
    assert_eq!(event.end_time, start + DAY);
    assert_eq!(client.get_pending_seasonal_events().len(), 1);

    // Not yet open.
    assert_eq!(
        client.try_activate_seasonal_event(&id),
        Err(Ok(EventError::EventNotReady))
    );

    env.ledger().set_timestamp(start);
    let active = client.activate_seasonal_event(&id);
    assert_eq!(active.status, SeasonalEventStatus::Active);
    assert_eq!(
        client.try_activate_seasonal_event(&id),
        Err(Ok(EventError::InvalidEventState))
    );

    client.record_seasonal_event_points(&id, &1, &30);
    assert_eq!(client.record_seasonal_event_points(&id, &2, &10), 10);
    assert_eq!(client.record_seasonal_event_points(&id, &1, &30), 60);

    // Cannot end before the window closes.
    assert_eq!(
        client.try_end_seasonal_event(&id),
        Err(Ok(EventError::EventNotReady))
    );

    env.ledger().set_timestamp(start + DAY);
    let ended = client.end_seasonal_event(&id);
    assert_eq!(ended.status, SeasonalEventStatus::Ended);
    assert_eq!(ended.participants, 2);
    assert_eq!(ended.total_points, 70);
    assert_eq!(client.get_pending_seasonal_events().len(), 0);
    assert_eq!(
        client.get_event_category_cooldown(&symbol_short!("boss")),
        start + DAY + 2 * DAY
    );
}

#[test]
fn test_schedule_requires_active_season() {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(NebulaNomadContract, ());
    let client = NebulaNomadContractClient::new(&env, &contract_id);
    let admin = Address::generate(&env);
    client.initialize_scheduler(&admin);

    let res = client
        .try_schedule_seasonal_event(&admin, &config(symbol_short!("boss"), HOUR, DAY, 0, false));
    assert_eq!(res, Err(Ok(EventError::NoActiveSeason)));
}

#[test]
fn test_schedule_rejects_non_admin() {
    let (env, client, _admin) = setup();
    let other = Address::generate(&env);
    let res = client.try_schedule_seasonal_event(
        &other,
        &config(symbol_short!("boss"), T0 + HOUR, DAY, 0, false),
    );
    assert_eq!(res, Err(Ok(EventError::Unauthorized)));
}

#[test]
fn test_schedule_validates_window() {
    let (_env, client, admin) = setup();
    let boss = symbol_short!("boss");

    // In the past.
    let res =
        client.try_schedule_seasonal_event(&admin, &config(boss.clone(), T0 - 1, DAY, 0, false));
    assert_eq!(res, Err(Ok(EventError::EventAlreadyPassed)));
    // Too short / too long.
    let res = client.try_schedule_seasonal_event(
        &admin,
        &config(boss.clone(), T0, MIN_SEASONAL_EVENT_DURATION - 1, 0, false),
    );
    assert_eq!(res, Err(Ok(EventError::InvalidEventWindow)));
    let res = client.try_schedule_seasonal_event(
        &admin,
        &config(boss.clone(), T0, MAX_SEASONAL_EVENT_DURATION + 1, 0, false),
    );
    assert_eq!(res, Err(Ok(EventError::InvalidEventWindow)));
    // Runs past the end of the season.
    let res = client.try_schedule_seasonal_event(
        &admin,
        &config(boss.clone(), T0 + 89 * DAY, 2 * DAY, 0, false),
    );
    assert_eq!(res, Err(Ok(EventError::InvalidEventWindow)));
    // Non-positive pool.
    let mut cfg = config(boss, T0, DAY, 0, false);
    cfg.reward_pool = 0;
    assert_eq!(
        client.try_schedule_seasonal_event(&admin, &cfg),
        Err(Ok(EventError::InvalidRewardPool))
    );
}

#[test]
fn test_pending_event_cap() {
    let (_env, client, admin) = setup();
    for i in 0..u64::from(MAX_PENDING_SEASONAL_EVENTS) {
        // Distinct categories, back-to-back, non-exclusive.
        let cat = Symbol::new(&client.env, &std::format!("cat{i}"));
        client.schedule_seasonal_event(&admin, &config(cat, T0 + i * DAY, DAY, 0, false));
    }
    let res = client
        .try_schedule_seasonal_event(&admin, &config(symbol_short!("extra"), T0, DAY, 0, false));
    assert_eq!(res, Err(Ok(EventError::TooManySeasonalEvents)));
}

#[test]
fn test_unactivated_event_can_still_be_ended() {
    let (env, client, admin) = setup();
    let id =
        client.schedule_seasonal_event(&admin, &config(symbol_short!("surge"), T0, DAY, 0, false));
    env.ledger().set_timestamp(T0 + DAY);
    assert_eq!(
        client.try_activate_seasonal_event(&id),
        Err(Ok(EventError::EventAlreadyPassed))
    );
    assert_eq!(
        client.end_seasonal_event(&id).status,
        SeasonalEventStatus::Ended
    );
}

#[test]
fn test_cancel_event() {
    let (env, client, admin) = setup();
    let id =
        client.schedule_seasonal_event(&admin, &config(symbol_short!("boss"), T0, DAY, DAY, false));
    client.cancel_seasonal_event(&admin, &id);
    assert_eq!(
        client.get_seasonal_event(&id).status,
        SeasonalEventStatus::Cancelled
    );
    assert_eq!(client.get_pending_seasonal_events().len(), 0);
    assert_eq!(
        client.try_cancel_seasonal_event(&admin, &id),
        Err(Ok(EventError::InvalidEventState))
    );
    // No cooldown after a cancellation.
    assert_eq!(
        client.get_event_category_cooldown(&symbol_short!("boss")),
        0
    );
    env.ledger().set_timestamp(T0 + DAY);
    assert_eq!(
        client.try_record_seasonal_event_points(&id, &1, &5),
        Err(Ok(EventError::InvalidEventState))
    );
}

// ─── Cooldowns & exclusivity ─────────────────────────────────────────────────

#[test]
fn test_category_cooldown_after_end() {
    let (env, client, admin) = setup();
    let boss = symbol_short!("boss");
    let id = client.schedule_seasonal_event(&admin, &config(boss.clone(), T0, DAY, 3 * DAY, false));
    env.ledger().set_timestamp(T0 + DAY);
    client.end_seasonal_event(&id);

    let res = client
        .try_schedule_seasonal_event(&admin, &config(boss.clone(), T0 + 2 * DAY, DAY, 0, false));
    assert_eq!(res, Err(Ok(EventError::EventCooldownActive)));
    // Other categories are unaffected.
    client.schedule_seasonal_event(
        &admin,
        &config(symbol_short!("surge"), T0 + 2 * DAY, DAY, 0, false),
    );
    // Once the cooldown elapses the category can run again.
    client.schedule_seasonal_event(&admin, &config(boss, T0 + 4 * DAY, DAY, 0, false));
}

#[test]
fn test_same_category_pending_events_respect_cooldown() {
    let (_env, client, admin) = setup();
    let boss = symbol_short!("boss");
    client.schedule_seasonal_event(
        &admin,
        &config(boss.clone(), T0 + 10 * DAY, DAY, 2 * DAY, false),
    );

    // Overlapping, or inside the cooldown after it.
    let res = client
        .try_schedule_seasonal_event(&admin, &config(boss.clone(), T0 + 10 * DAY, DAY, 0, false));
    assert_eq!(res, Err(Ok(EventError::EventCooldownActive)));
    let res = client
        .try_schedule_seasonal_event(&admin, &config(boss.clone(), T0 + 12 * DAY, DAY, 0, false));
    assert_eq!(res, Err(Ok(EventError::EventCooldownActive)));
    // Earlier event whose own cooldown would run into the scheduled one.
    let res = client.try_schedule_seasonal_event(
        &admin,
        &config(boss.clone(), T0 + 8 * DAY, DAY, 2 * DAY, false),
    );
    assert_eq!(res, Err(Ok(EventError::EventCooldownActive)));

    client.schedule_seasonal_event(&admin, &config(boss.clone(), T0 + 13 * DAY, DAY, 0, false));
    client.schedule_seasonal_event(&admin, &config(boss, T0 + 7 * DAY, DAY, 2 * DAY, false));
}

#[test]
fn test_exclusive_event_blocks_overlaps() {
    let (_env, client, admin) = setup();
    client.schedule_seasonal_event(
        &admin,
        &config(symbol_short!("boss"), T0 + DAY, 2 * DAY, 0, true),
    );

    // Anything overlapping an exclusive event is rejected.
    let res = client.try_schedule_seasonal_event(
        &admin,
        &config(symbol_short!("surge"), T0 + 2 * DAY, DAY, 0, false),
    );
    assert_eq!(res, Err(Ok(EventError::ExclusiveEventConflict)));
    // Adjacent windows are fine.
    client.schedule_seasonal_event(
        &admin,
        &config(symbol_short!("surge"), T0 + 3 * DAY, DAY, 0, false),
    );
    // An exclusive event cannot overlap a non-exclusive one either.
    let res = client.try_schedule_seasonal_event(
        &admin,
        &config(symbol_short!("fleet"), T0 + 3 * DAY, HOUR, 0, true),
    );
    assert_eq!(res, Err(Ok(EventError::ExclusiveEventConflict)));
    // Non-exclusive events of different categories may overlap.
    client.schedule_seasonal_event(
        &admin,
        &config(symbol_short!("fleet"), T0 + 3 * DAY, HOUR, 0, false),
    );
}

// ─── Special seasonal rewards ────────────────────────────────────────────────

#[test]
fn test_seasonal_reward_bonus_applied() {
    let (_env, client, admin) = setup();
    let season = client.get_current_season();

    let id =
        client.schedule_seasonal_event(&admin, &config(symbol_short!("boss"), T0, DAY, 0, false));
    let event = client.get_seasonal_event(&id);
    assert_eq!(event.base_reward_pool, 10_000);
    assert_eq!(
        event.reward_pool,
        seasonal_event_reward_pool(&season.config.theme, 10_000, false).unwrap()
    );
    // Season 1 (EmberNebula) pays 1.1×.
    assert_eq!(event.reward_pool, 11_000);

    let ex = client.schedule_seasonal_event(
        &admin,
        &config(symbol_short!("surge"), T0 + DAY, DAY, 0, true),
    );
    // 1.1× + 0.25× exclusivity bonus.
    assert_eq!(client.get_seasonal_event(&ex).reward_pool, 13_500);
}

#[test]
fn test_reward_claims_split_pool_by_points() {
    let (env, client, admin) = setup();
    let id =
        client.schedule_seasonal_event(&admin, &config(symbol_short!("boss"), T0, DAY, 0, false));
    client.activate_seasonal_event(&id);
    client.record_seasonal_event_points(&id, &1, &3);
    client.record_seasonal_event_points(&id, &2, &1);

    let player = Address::generate(&env);
    // Rewards are only claimable after the event ends.
    assert_eq!(
        client.try_claim_seasonal_event_reward(&player, &id, &1),
        Err(Ok(EventError::InvalidEventState))
    );

    env.ledger().set_timestamp(T0 + DAY);
    client.end_seasonal_event(&id);

    assert_eq!(client.claim_seasonal_event_reward(&player, &id, &1), 8_250);
    assert_eq!(client.claim_seasonal_event_reward(&player, &id, &2), 2_750);
    assert_eq!(
        client.try_claim_seasonal_event_reward(&player, &id, &1),
        Err(Ok(EventError::AlreadyClaimed))
    );
    assert_eq!(
        client.try_claim_seasonal_event_reward(&player, &id, &3),
        Err(Ok(EventError::NoEventReward))
    );

    let event = client.get_seasonal_event(&id);
    assert_eq!(event.rewards_claimed, 11_000);
    assert!(event.rewards_claimed <= event.reward_pool);
    assert!(client.get_seasonal_event_entry(&id, &1).unwrap().claimed);
}

#[test]
fn test_reward_claim_window_closes() {
    let (env, client, admin) = setup();
    let id =
        client.schedule_seasonal_event(&admin, &config(symbol_short!("boss"), T0, DAY, 0, false));
    client.activate_seasonal_event(&id);
    client.record_seasonal_event_points(&id, &1, &5);
    env.ledger().set_timestamp(T0 + DAY);
    client.end_seasonal_event(&id);

    env.ledger()
        .set_timestamp(T0 + DAY + SEASONAL_REWARD_CLAIM_WINDOW + 1);
    let player = Address::generate(&env);
    assert_eq!(
        client.try_claim_seasonal_event_reward(&player, &id, &1),
        Err(Ok(EventError::ClaimWindowClosed))
    );
}

#[test]
fn test_points_only_recorded_while_active() {
    let (env, client, admin) = setup();
    let id = client.schedule_seasonal_event(
        &admin,
        &config(symbol_short!("boss"), T0 + HOUR, DAY, 0, false),
    );
    assert_eq!(
        client.try_record_seasonal_event_points(&id, &1, &5),
        Err(Ok(EventError::InvalidEventState))
    );

    env.ledger().set_timestamp(T0 + HOUR);
    client.activate_seasonal_event(&id);
    env.ledger().set_timestamp(T0 + HOUR + DAY);
    assert_eq!(
        client.try_record_seasonal_event_points(&id, &1, &5),
        Err(Ok(EventError::EventAlreadyPassed))
    );
}

// ─── Event-specific challenges ───────────────────────────────────────────────

fn spec(env: &Env) -> SeasonalChallengeSpec {
    SeasonalChallengeSpec {
        title: String::from_str(env, "Boss Hunter"),
        description: String::from_str(env, "Scan 10 boss anomalies"),
        target_metric: symbol_short!("scans"),
        target_value: 10,
        reward_free: 100,
        reward_premium: 250,
    }
}

#[test]
fn test_event_challenge_shares_event_window() {
    let (env, client, admin) = setup();
    let start = T0 + HOUR;
    let id = client
        .schedule_seasonal_event(&admin, &config(symbol_short!("boss"), start, DAY, 0, false));
    let challenge_id = client.add_seasonal_event_challenge(&admin, &id, &spec(&env));

    let event = client.get_seasonal_event(&id);
    assert_eq!(event.challenge_ids.len(), 1);
    assert_eq!(event.challenge_ids.get(0).unwrap(), challenge_id);

    // Not open before the event starts.
    assert_eq!(
        client.try_record_challenge_progress(&1, &challenge_id, &5),
        Err(Ok(EventError::ChallengeNotStarted))
    );

    env.ledger().set_timestamp(start);
    client.record_challenge_progress(&1, &challenge_id, &5);
    assert_eq!(client.record_challenge_progress(&1, &challenge_id, &5), 10);

    // Closed after the event ends.
    env.ledger().set_timestamp(start + DAY + 1);
    assert_eq!(
        client.try_record_challenge_progress(&1, &challenge_id, &1),
        Err(Ok(EventError::ChallengeExpired))
    );
}

#[test]
fn test_event_challenge_limits() {
    let (env, client, admin) = setup();
    let id =
        client.schedule_seasonal_event(&admin, &config(symbol_short!("boss"), T0, DAY, 0, false));
    for _ in 0..MAX_CHALLENGES_PER_EVENT {
        client.add_seasonal_event_challenge(&admin, &id, &spec(&env));
    }
    assert_eq!(
        client.try_add_seasonal_event_challenge(&admin, &id, &spec(&env)),
        Err(Ok(EventError::TooManyChallenges))
    );

    // No challenges can be added to an ended event.
    let other =
        client.schedule_seasonal_event(&admin, &config(symbol_short!("surge"), T0, DAY, 0, false));
    env.ledger().set_timestamp(T0 + DAY);
    client.end_seasonal_event(&other);
    assert_eq!(
        client.try_add_seasonal_event_challenge(&admin, &other, &spec(&env)),
        Err(Ok(EventError::InvalidEventState))
    );
}
