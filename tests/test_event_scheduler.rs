#![cfg(test)]

use soroban_sdk::{symbol_short, testutils::Address as _, Address, Env, Symbol};
use stellar_nebula_nomad::{
    EventError, EventResult, ScheduledEvent, MAX_ACTIVE_EVENTS, WEEKLY_FESTIVAL_INTERVAL,
};

fn create_test_env() -> (Env, Address) {
    let env = Env::default();
    env.mock_all_auths();
    let admin = Address::generate(&env);
    (env, admin)
}

#[test]
fn test_initialize_scheduler() {
    let (env, admin) = create_test_env();
    
    stellar_nebula_nomad::NebulaNomadContract::initialize_scheduler(env.clone(), admin.clone());
    
    let count = stellar_nebula_nomad::NebulaNomadContract::get_event_count(env.clone());
    assert_eq!(count, 0);
    
    let active = stellar_nebula_nomad::NebulaNomadContract::get_active_events(env.clone());
    assert_eq!(active.len(), 0);
}

#[test]
fn test_schedule_event_success() {
    let (env, admin) = create_test_env();
    
    stellar_nebula_nomad::NebulaNomadContract::initialize_scheduler(env.clone(), admin.clone());
    
    let current_time = env.ledger().timestamp();
    let start_time = current_time + 3600; // 1 hour from now
    let reward_pool = 10000i128;
    
    let event_id = stellar_nebula_nomad::NebulaNomadContract::schedule_event(
        env.clone(),
        admin.clone(),
        symbol_short!("festival"),
        start_time,
        reward_pool,
    )
    .unwrap();
    
    assert_eq!(event_id, 1);
    
    let event = stellar_nebula_nomad::NebulaNomadContract::get_event(env.clone(), event_id).unwrap();
    assert_eq!(event.event_id, 1);
    assert_eq!(event.event_type, symbol_short!("festival"));
    assert_eq!(event.start_time, start_time);
    assert_eq!(event.reward_pool, reward_pool);
    assert_eq!(event.executed, false);
    
    let active = stellar_nebula_nomad::NebulaNomadContract::get_active_events(env.clone());
    assert_eq!(active.len(), 1);
    assert_eq!(active.get(0).unwrap(), 1);
}

#[test]
fn test_schedule_event_past_time_fails() {
    let (env, admin) = create_test_env();
    
    stellar_nebula_nomad::NebulaNomadContract::initialize_scheduler(env.clone(), admin.clone());
    
    let current_time = env.ledger().timestamp();
    let past_time = current_time - 3600; // 1 hour ago
    
    let result = stellar_nebula_nomad::NebulaNomadContract::schedule_event(
        env.clone(),
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
    
    stellar_nebula_nomad::NebulaNomadContract::initialize_scheduler(env.clone(), admin.clone());
    
    let current_time = env.ledger().timestamp();
    let start_time = current_time + 3600;
    
    let result = stellar_nebula_nomad::NebulaNomadContract::schedule_event(
        env.clone(),
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
    
    stellar_nebula_nomad::NebulaNomadContract::initialize_scheduler(env.clone(), admin.clone());
    
    let current_time = env.ledger().timestamp();
    let start_time = current_time + 100;
    
    let event_id = stellar_nebula_nomad::NebulaNomadContract::schedule_event(
        env.clone(),
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
    
    let result = stellar_nebula_nomad::NebulaNomadContract::trigger_scheduled_event(
        env.clone(),
        event_id,
    )
    .unwrap();
    
    assert_eq!(result.event_id, event_id);
    assert_eq!(result.rewards_distributed, 5000i128);
    
    let event = stellar_nebula_nomad::NebulaNomadContract::get_event(env.clone(), event_id).unwrap();
    assert_eq!(event.executed, true);
    
    let active = stellar_nebula_nomad::NebulaNomadContract::get_active_events(env.clone());
    assert_eq!(active.len(), 0);
}

#[test]
fn test_trigger_event_too_early_fails() {
    let (env, admin) = create_test_env();
    
    stellar_nebula_nomad::NebulaNomadContract::initialize_scheduler(env.clone(), admin.clone());
    
    let current_time = env.ledger().timestamp();
    let start_time = current_time + 3600;
    
    let event_id = stellar_nebula_nomad::NebulaNomadContract::schedule_event(
        env.clone(),
        admin.clone(),
        symbol_short!("harvest"),
        start_time,
        2000i128,
    )
    .unwrap();
    
    let result = stellar_nebula_nomad::NebulaNomadContract::trigger_scheduled_event(
        env.clone(),
        event_id,
    );
    
    assert_eq!(result, Err(EventError::EventNotReady));
}

#[test]
fn test_trigger_event_already_executed_fails() {
    let (env, admin) = create_test_env();
    
    stellar_nebula_nomad::NebulaNomadContract::initialize_scheduler(env.clone(), admin.clone());
    
    let current_time = env.ledger().timestamp();
    let start_time = current_time + 100;
    
    let event_id = stellar_nebula_nomad::NebulaNomadContract::schedule_event(
        env.clone(),
        admin.clone(),
        symbol_short!("pvp"),
        start_time,
        3000i128,
    )
    .unwrap();
    
    env.ledger().with_mut(|li| {
        li.timestamp = start_time + 1;
    });
    
    stellar_nebula_nomad::NebulaNomadContract::trigger_scheduled_event(env.clone(), event_id)
        .unwrap();
    
    let result = stellar_nebula_nomad::NebulaNomadContract::trigger_scheduled_event(
        env.clone(),
        event_id,
    );
    
    assert_eq!(result, Err(EventError::EventAlreadyExecuted));
}

#[test]
fn test_schedule_weekly_festival() {
    let (env, admin) = create_test_env();
    
    stellar_nebula_nomad::NebulaNomadContract::initialize_scheduler(env.clone(), admin.clone());
    
    let current_time = env.ledger().timestamp();
    let reward_pool = 50000i128;
    
    let event_id = stellar_nebula_nomad::NebulaNomadContract::schedule_weekly_festival(
        env.clone(),
        admin.clone(),
        reward_pool,
    )
    .unwrap();
    
    let event = stellar_nebula_nomad::NebulaNomadContract::get_event(env.clone(), event_id).unwrap();
    assert_eq!(event.event_type, symbol_short!("festival"));
    assert_eq!(event.start_time, current_time + WEEKLY_FESTIVAL_INTERVAL);
    assert_eq!(event.reward_pool, reward_pool);
}

#[test]
fn test_cancel_event() {
    let (env, admin) = create_test_env();
    
    stellar_nebula_nomad::NebulaNomadContract::initialize_scheduler(env.clone(), admin.clone());
    
    let current_time = env.ledger().timestamp();
    let start_time = current_time + 3600;
    
    let event_id = stellar_nebula_nomad::NebulaNomadContract::schedule_event(
        env.clone(),
        admin.clone(),
        symbol_short!("explore"),
        start_time,
        1000i128,
    )
    .unwrap();
    
    stellar_nebula_nomad::NebulaNomadContract::cancel_event(
        env.clone(),
        admin.clone(),
        event_id,
    )
    .unwrap();
    
    let event = stellar_nebula_nomad::NebulaNomadContract::get_event(env.clone(), event_id).unwrap();
    assert_eq!(event.executed, true);
    
    let active = stellar_nebula_nomad::NebulaNomadContract::get_active_events(env.clone());
    assert_eq!(active.len(), 0);
}

#[test]
fn test_multiple_events() {
    let (env, admin) = create_test_env();
    
    stellar_nebula_nomad::NebulaNomadContract::initialize_scheduler(env.clone(), admin.clone());
    
    let current_time = env.ledger().timestamp();
    
    // Schedule 5 events
    for i in 1..=5 {
        let start_time = current_time + (i * 1000);
        stellar_nebula_nomad::NebulaNomadContract::schedule_event(
            env.clone(),
            admin.clone(),
            symbol_short!("festival"),
            start_time,
            (i as i128) * 1000,
        )
        .unwrap();
    }
    
    let count = stellar_nebula_nomad::NebulaNomadContract::get_event_count(env.clone());
    assert_eq!(count, 5);
    
    let active = stellar_nebula_nomad::NebulaNomadContract::get_active_events(env.clone());
    assert_eq!(active.len(), 5);
}

#[test]
fn test_max_active_events_limit() {
    let (env, admin) = create_test_env();
    
    stellar_nebula_nomad::NebulaNomadContract::initialize_scheduler(env.clone(), admin.clone());
    
    let current_time = env.ledger().timestamp();
    
    // Schedule MAX_ACTIVE_EVENTS events
    for i in 1..=MAX_ACTIVE_EVENTS {
        let start_time = current_time + (i as u64 * 1000);
        stellar_nebula_nomad::NebulaNomadContract::schedule_event(
            env.clone(),
            admin.clone(),
            symbol_short!("raid"),
            start_time,
            1000i128,
        )
        .unwrap();
    }
    
    // Try to schedule one more - should fail
    let result = stellar_nebula_nomad::NebulaNomadContract::schedule_event(
        env.clone(),
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
    
    stellar_nebula_nomad::NebulaNomadContract::initialize_scheduler(env.clone(), admin.clone());
    
    let current_time = env.ledger().timestamp();
    let start_time = current_time + 3600;
    
    let event_id = stellar_nebula_nomad::NebulaNomadContract::schedule_event(
        env.clone(),
        admin.clone(),
        symbol_short!("pvp"),
        start_time,
        10000i128,
    )
    .unwrap();
    
    stellar_nebula_nomad::NebulaNomadContract::update_event_participants(
        env.clone(),
        event_id,
        42,
    )
    .unwrap();
    
    let event = stellar_nebula_nomad::NebulaNomadContract::get_event(env.clone(), event_id).unwrap();
    assert_eq!(event.participants, 42);
}

#[test]
fn test_event_lifecycle() {
    let (env, admin) = create_test_env();
    
    // Initialize
    stellar_nebula_nomad::NebulaNomadContract::initialize_scheduler(env.clone(), admin.clone());
    
    let current_time = env.ledger().timestamp();
    let start_time = current_time + 1000;
    
    // Schedule
    let event_id = stellar_nebula_nomad::NebulaNomadContract::schedule_event(
        env.clone(),
        admin.clone(),
        symbol_short!("festival"),
        start_time,
        25000i128,
    )
    .unwrap();
    
    // Update participants
    stellar_nebula_nomad::NebulaNomadContract::update_event_participants(
        env.clone(),
        event_id,
        100,
    )
    .unwrap();
    
    // Fast forward time
    env.ledger().with_mut(|li| {
        li.timestamp = start_time + 1;
    });
    
    // Trigger
    let result = stellar_nebula_nomad::NebulaNomadContract::trigger_scheduled_event(
        env.clone(),
        event_id,
    )
    .unwrap();
    
    assert_eq!(result.event_id, event_id);
    assert_eq!(result.rewards_distributed, 25000i128);
    assert_eq!(result.participants, 100);
    
    // Verify executed
    let event = stellar_nebula_nomad::NebulaNomadContract::get_event(env.clone(), event_id).unwrap();
    assert_eq!(event.executed, true);
}

#[test]
fn test_all_event_types() {
    let (env, admin) = create_test_env();
    
    stellar_nebula_nomad::NebulaNomadContract::initialize_scheduler(env.clone(), admin.clone());
    
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
        let event_id = stellar_nebula_nomad::NebulaNomadContract::schedule_event(
            env.clone(),
            admin.clone(),
            event_type.clone(),
            start_time,
            1000i128,
        )
        .unwrap();
        
        let event = stellar_nebula_nomad::NebulaNomadContract::get_event(env.clone(), event_id).unwrap();
        assert_eq!(event.event_type, *event_type);
    }
}
