#![cfg(test)]

use soroban_sdk::{
    symbol_short,
    testutils::{Address as _, Ledger},
    Address, Env, Symbol, Vec,
};
use stellar_nebula_nomad::{
    event_scheduler, EventError, EventResult, NebulaNomadContract, ScheduledEvent,
    MAX_ACTIVE_EVENTS, WEEKLY_FESTIVAL_INTERVAL,
};

std::thread_local! {
    static CONTRACT_ID: std::cell::RefCell<Option<Address>> = const { std::cell::RefCell::new(None) };
}

fn create_test_env() -> (Env, Address) {
    let env = Env::default();
    env.mock_all_auths();
    env.ledger().set_timestamp(10_000);
    let admin = Address::generate(&env);
    let contract_id = env.register(NebulaNomadContract, ());
    CONTRACT_ID.with(|id| *id.borrow_mut() = Some(contract_id));
    (env, admin)
}

/// Scheduler functions touch contract storage, so each call runs in its own
/// contract frame (one `require_auth` per frame, as in a real invocation).
fn in_contract<T>(env: &Env, f: impl FnOnce() -> T) -> T {
    let contract_id = CONTRACT_ID.with(|id| id.borrow().clone().expect("contract registered"));
    env.as_contract(&contract_id, f)
}

fn initialize_scheduler(env: &Env, admin: &Address) {
    in_contract(env, || event_scheduler::initialize_scheduler(env, admin));
}

fn schedule_event(
    env: &Env,
    admin: Address,
    event_type: Symbol,
    start_time: u64,
    reward_pool: i128,
) -> Result<u64, EventError> {
    in_contract(env, || {
        event_scheduler::schedule_event(env, &admin, event_type, start_time, reward_pool)
    })
}

fn trigger_scheduled_event(env: &Env, event_id: u64) -> Result<EventResult, EventError> {
    in_contract(env, || {
        event_scheduler::trigger_scheduled_event(env, event_id)
    })
}

fn get_event(env: &Env, event_id: u64) -> Result<ScheduledEvent, EventError> {
    in_contract(env, || event_scheduler::get_event(env, event_id))
}

fn get_active_events(env: &Env) -> Vec<u64> {
    in_contract(env, || event_scheduler::get_active_events(env))
}

fn schedule_weekly_festival(
    env: &Env,
    admin: Address,
    reward_pool: i128,
) -> Result<u64, EventError> {
    in_contract(env, || {
        event_scheduler::schedule_weekly_festival(env, &admin, reward_pool)
    })
}

fn cancel_event(env: &Env, admin: Address, event_id: u64) -> Result<(), EventError> {
    in_contract(env, || event_scheduler::cancel_event(env, &admin, event_id))
}

fn update_participants(env: &Env, event_id: u64, count: u32) -> Result<(), EventError> {
    in_contract(env, || {
        event_scheduler::update_participants(env, event_id, count)
    })
}

fn get_event_count(env: &Env) -> u64 {
    in_contract(env, || event_scheduler::get_event_count(env))
}

#[test]
fn test_initialize_scheduler() {
    let (env, admin) = create_test_env();

    initialize_scheduler(&env, &admin);

    let count = get_event_count(&env);
    assert_eq!(count, 0);

    let active = get_active_events(&env);
    assert_eq!(active.len(), 0);
}

#[test]
fn test_schedule_event_success() {
    let (env, admin) = create_test_env();

    initialize_scheduler(&env, &admin);

    let current_time = env.ledger().timestamp();
    let start_time = current_time + 3600; // 1 hour from now
    let reward_pool = 10000i128;

    let event_id = schedule_event(
        &env,
        admin.clone(),
        symbol_short!("festival"),
        start_time,
        reward_pool,
    )
    .unwrap();

    assert_eq!(event_id, 1);

    let event = get_event(&env, event_id).unwrap();
    assert_eq!(event.event_id, 1);
    assert_eq!(event.event_type, symbol_short!("festival"));
    assert_eq!(event.start_time, start_time);
    assert_eq!(event.reward_pool, reward_pool);
    assert_eq!(event.executed, false);

    let active = get_active_events(&env);
    assert_eq!(active.len(), 1);
    assert_eq!(active.get(0).unwrap(), 1);
}

#[test]
fn test_schedule_event_past_time_fails() {
    let (env, admin) = create_test_env();

    initialize_scheduler(&env, &admin);

    let current_time = env.ledger().timestamp();
    let past_time = current_time - 3600; // 1 hour ago

    let result = schedule_event(
        &env,
        admin.clone(),
        symbol_short!("festival"),
        past_time,
        10000i128,
    );

    assert_eq!(result, Err(EventError::EventAlreadyPassed));
}

#[test]
fn test_schedule_event_invalid_type_fails() {
    let (env, admin) = create_test_env();

    initialize_scheduler(&env, &admin);

    let current_time = env.ledger().timestamp();
    let start_time = current_time + 3600;

    let result = schedule_event(
        &env,
        admin.clone(),
        symbol_short!("invalid"),
        start_time,
        10000i128,
    );

    assert_eq!(result, Err(EventError::InvalidEventType));
}

#[test]
fn test_trigger_event_success() {
    let (env, admin) = create_test_env();

    initialize_scheduler(&env, &admin);

    let current_time = env.ledger().timestamp();
    let start_time = current_time + 100;

    let event_id = schedule_event(
        &env,
        admin.clone(),
        symbol_short!("raid"),
        start_time,
        5000i128,
    )
    .unwrap();

    // Fast forward time
    env.ledger().with_mut(|li| {
        li.timestamp = start_time + 1;
    });

    let result = trigger_scheduled_event(&env, event_id).unwrap();

    assert_eq!(result.event_id, event_id);
    assert_eq!(result.rewards_distributed, 5000i128);

    let event = get_event(&env, event_id).unwrap();
    assert_eq!(event.executed, true);

    let active = get_active_events(&env);
    assert_eq!(active.len(), 0);
}

#[test]
fn test_trigger_event_too_early_fails() {
    let (env, admin) = create_test_env();

    initialize_scheduler(&env, &admin);

    let current_time = env.ledger().timestamp();
    let start_time = current_time + 3600;

    let event_id = schedule_event(
        &env,
        admin.clone(),
        symbol_short!("harvest"),
        start_time,
        2000i128,
    )
    .unwrap();

    let result = trigger_scheduled_event(&env, event_id);

    assert_eq!(result, Err(EventError::EventNotReady));
}

#[test]
fn test_trigger_event_already_executed_fails() {
    let (env, admin) = create_test_env();

    initialize_scheduler(&env, &admin);

    let current_time = env.ledger().timestamp();
    let start_time = current_time + 100;

    let event_id = schedule_event(
        &env,
        admin.clone(),
        symbol_short!("pvp"),
        start_time,
        3000i128,
    )
    .unwrap();

    env.ledger().with_mut(|li| {
        li.timestamp = start_time + 1;
    });

    trigger_scheduled_event(&env, event_id).unwrap();

    let result = trigger_scheduled_event(&env, event_id);

    assert_eq!(result, Err(EventError::EventAlreadyExecuted));
}

#[test]
fn test_schedule_weekly_festival() {
    let (env, admin) = create_test_env();

    initialize_scheduler(&env, &admin);

    let current_time = env.ledger().timestamp();
    let reward_pool = 50000i128;

    let event_id = schedule_weekly_festival(&env, admin.clone(), reward_pool).unwrap();

    let event = get_event(&env, event_id).unwrap();
    assert_eq!(event.event_type, symbol_short!("festival"));
    assert_eq!(event.start_time, current_time + WEEKLY_FESTIVAL_INTERVAL);
    assert_eq!(event.reward_pool, reward_pool);
}

#[test]
fn test_cancel_event() {
    let (env, admin) = create_test_env();

    initialize_scheduler(&env, &admin);

    let current_time = env.ledger().timestamp();
    let start_time = current_time + 3600;

    let event_id = schedule_event(
        &env,
        admin.clone(),
        symbol_short!("explore"),
        start_time,
        1000i128,
    )
    .unwrap();

    cancel_event(&env, admin.clone(), event_id).unwrap();

    let event = get_event(&env, event_id).unwrap();
    assert_eq!(event.executed, true);

    let active = get_active_events(&env);
    assert_eq!(active.len(), 0);
}

#[test]
fn test_multiple_events() {
    let (env, admin) = create_test_env();

    initialize_scheduler(&env, &admin);

    let current_time = env.ledger().timestamp();

    // Schedule 5 events
    for i in 1..=5 {
        let start_time = current_time + (i * 1000);
        schedule_event(
            &env,
            admin.clone(),
            symbol_short!("festival"),
            start_time,
            (i as i128) * 1000,
        )
        .unwrap();
    }

    let count = get_event_count(&env);
    assert_eq!(count, 5);

    let active = get_active_events(&env);
    assert_eq!(active.len(), 5);
}

#[test]
fn test_max_active_events_limit() {
    let (env, admin) = create_test_env();

    initialize_scheduler(&env, &admin);

    let current_time = env.ledger().timestamp();

    // Schedule MAX_ACTIVE_EVENTS events
    for i in 1..=MAX_ACTIVE_EVENTS {
        let start_time = current_time + (i as u64 * 1000);
        schedule_event(
            &env,
            admin.clone(),
            symbol_short!("raid"),
            start_time,
            1000i128,
        )
        .unwrap();
    }

    // Try to schedule one more - should fail
    let result = schedule_event(
        &env,
        admin.clone(),
        symbol_short!("raid"),
        current_time + 100000,
        1000i128,
    );

    assert_eq!(result, Err(EventError::TooManyActiveEvents));
}

#[test]
fn test_update_participants() {
    let (env, admin) = create_test_env();

    initialize_scheduler(&env, &admin);

    let current_time = env.ledger().timestamp();
    let start_time = current_time + 3600;

    let event_id = schedule_event(
        &env,
        admin.clone(),
        symbol_short!("pvp"),
        start_time,
        10000i128,
    )
    .unwrap();

    update_participants(&env, event_id, 42).unwrap();

    let event = get_event(&env, event_id).unwrap();
    assert_eq!(event.participants, 42);
}

#[test]
fn test_event_lifecycle() {
    let (env, admin) = create_test_env();

    // Initialize
    initialize_scheduler(&env, &admin);

    let current_time = env.ledger().timestamp();
    let start_time = current_time + 1000;

    // Schedule
    let event_id = schedule_event(
        &env,
        admin.clone(),
        symbol_short!("festival"),
        start_time,
        25000i128,
    )
    .unwrap();

    // Update participants
    update_participants(&env, event_id, 100).unwrap();

    // Fast forward time
    env.ledger().with_mut(|li| {
        li.timestamp = start_time + 1;
    });

    // Trigger
    let result = trigger_scheduled_event(&env, event_id).unwrap();

    assert_eq!(result.event_id, event_id);
    assert_eq!(result.rewards_distributed, 25000i128);
    assert_eq!(result.participants, 100);

    // Verify executed
    let event = get_event(&env, event_id).unwrap();
    assert_eq!(event.executed, true);
}

#[test]
fn test_all_event_types() {
    let (env, admin) = create_test_env();

    initialize_scheduler(&env, &admin);

    let current_time = env.ledger().timestamp();
    let event_types = [
        symbol_short!("festival"),
        symbol_short!("raid"),
        symbol_short!("harvest"),
        symbol_short!("pvp"),
        symbol_short!("explore"),
    ];

    for (i, event_type) in event_types.iter().enumerate() {
        let start_time = current_time + ((i as u64 + 1) * 1000);
        let event_id = schedule_event(
            &env,
            admin.clone(),
            event_type.clone(),
            start_time,
            1000i128,
        )
        .unwrap();

        let event = get_event(&env, event_id).unwrap();
        assert_eq!(event.event_type, *event_type);
    }
}
