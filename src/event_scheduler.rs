use soroban_sdk::{
    contracterror, contracttype, symbol_short, Address, Env, Symbol, Vec,
};

/// Maximum number of active scheduled events.
pub const MAX_ACTIVE_EVENTS: u32 = 20;

/// Weekly nebula festival interval (7 days in seconds).
pub const WEEKLY_FESTIVAL_INTERVAL: u64 = 7 * 24 * 60 * 60;

// ─── Storage Keys ─────────────────────────────────────────────────────────────

#[derive(Clone)]
#[contracttype]
pub enum EventKey {
    /// Event data keyed by event ID.
    Event(u64),
    /// Global event counter.
    EventCounter,
    /// Admin address for scheduling permissions.
    Admin,
    /// Active event IDs list.
    ActiveEvents,
    /// Burst counter for rate limiting.
    BurstCounter,
}

// ─── Data Types ───────────────────────────────────────────────────────────────

/// Scheduled event record.
#[derive(Clone, Debug, PartialEq)]
#[contracttype]
pub struct ScheduledEvent {
    pub event_id: u64,
    pub event_type: Symbol,
    pub start_time: u64,
    pub creator: Address,
    pub executed: bool,
    pub reward_pool: i128,
    pub participants: u32,
}

/// Event execution result.
#[derive(Clone, Debug, PartialEq)]
#[contracttype]
pub struct EventResult {
    pub event_id: u64,
    pub executed_at: u64,
    pub rewards_distributed: i128,
    pub participants: u32,
}

// ─── Errors ───────────────────────────────────────────────────────────────────

#[contracterror]
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum EventError {
    /// Event start time is in the past.
    EventAlreadyPassed = 1,
    /// Event not found.
    EventNotFound = 2,
    /// Event already executed.
    EventAlreadyExecuted = 3,
    /// Event not yet ready to execute.
    EventNotReady = 4,
    /// Unauthorized - admin only.
    Unauthorized = 5,
    /// Too many active events (max 20).
    TooManyActiveEvents = 6,
    /// Burst limit exceeded.
    BurstLimitExceeded = 7,
    /// Invalid event type.
    InvalidEventType = 8,
}

// ─── Helper Functions ─────────────────────────────────────────────────────────

/// Check if caller is admin.
fn require_admin(env: &Env, caller: &Address) -> Result<(), EventError> {
    caller.require_auth();
    let admin: Address = env
        .storage()
        .instance()
        .get(&EventKey::Admin)
        .ok_or(EventError::Unauthorized)?;
    
    if caller != &admin {
        return Err(EventError::Unauthorized);
    }
    Ok(())
}

/// Get current ledger timestamp.
fn get_current_timestamp(env: &Env) -> u64 {
    env.ledger().timestamp()
}

/// Increment and return next event ID.
fn next_event_id(env: &Env) -> u64 {
    let current: u64 = env
        .storage()
        .instance()
        .get(&EventKey::EventCounter)
        .unwrap_or(0);
    let next = current + 1;
    env.storage()
        .instance()
        .set(&EventKey::EventCounter, &next);
    next
}

/// Check burst limit for scheduling operations.
fn check_burst_limit(env: &Env) -> Result<(), EventError> {
    let counter: u32 = env
        .storage()
        .instance()
        .get(&EventKey::BurstCounter)
        .unwrap_or(0);
    
    if counter >= MAX_ACTIVE_EVENTS {
        return Err(EventError::BurstLimitExceeded);
    }
    
    env.storage()
        .instance()
        .set(&EventKey::BurstCounter, &(counter + 1));
    Ok(())
}

/// Reset burst counter.
pub fn reset_burst_counter(env: &Env) {
    env.storage().instance().set(&EventKey::BurstCounter, &0u32);
}

/// Add event to active events list.
fn add_to_active_events(env: &Env, event_id: u64) -> Result<(), EventError> {
    let mut active: Vec<u64> = env
        .storage()
        .instance()
        .get(&EventKey::ActiveEvents)
        .unwrap_or(Vec::new(env));
    
    if active.len() >= MAX_ACTIVE_EVENTS {
        return Err(EventError::TooManyActiveEvents);
    }
    
    active.push_back(event_id);
    env.storage()
        .instance()
        .set(&EventKey::ActiveEvents, &active);
    Ok(())
}

/// Remove event from active events list.
fn remove_from_active_events(env: &Env, event_id: u64) {
    let active: Vec<u64> = env
        .storage()
        .instance()
        .get(&EventKey::ActiveEvents)
        .unwrap_or(Vec::new(env));
    
    let mut new_active = Vec::new(env);
    for i in 0..active.len() {
        let id = active.get(i).unwrap();
        if id != event_id {
            new_active.push_back(id);
        }
    }
    
    env.storage()
        .instance()
        .set(&EventKey::ActiveEvents, &new_active);
}

// ─── Public API ───────────────────────────────────────────────────────────────

/// Initialize the event scheduler with an admin address.
pub fn initialize_scheduler(env: &Env, admin: &Address) {
    admin.require_auth();
    env.storage().instance().set(&EventKey::Admin, admin);
    env.storage().instance().set(&EventKey::EventCounter, &0u64);
    env.storage()
        .instance()
        .set(&EventKey::ActiveEvents, &Vec::<u64>::new(env));
    
    // Emit initialization event
    env.events().publish(
        (symbol_short!("init_sch"),),
        admin,
    );
}

/// Schedule a new community event.
pub fn schedule_event(
    env: &Env,
    admin: Address,
    event_type: Symbol,
    start_time: u64,
    reward_pool: i128,
) -> Result<u64, EventError> {
    require_admin(env, &admin)?;
    check_burst_limit(env)?;
    
    let current_time = get_current_timestamp(env);
    
    // Validate start time is in the future
    if start_time <= current_time {
        return Err(EventError::EventAlreadyPassed);
    }
    
    // Validate event type
    let valid_types = [
        symbol_short!("festival"),
        symbol_short!("raid"),
        symbol_short!("harvest"),
        symbol_short!("pvp"),
        symbol_short!("explore"),
    ];
    
    let mut is_valid = false;
    for valid_type in valid_types.iter() {
        if &event_type == valid_type {
            is_valid = true;
            break;
        }
    }
    
    if !is_valid {
        return Err(EventError::InvalidEventType);
    }
    
    let event_id = next_event_id(env);
    
    let event = ScheduledEvent {
        event_id,
        event_type: event_type.clone(),
        start_time,
        creator: admin.clone(),
        executed: false,
        reward_pool,
        participants: 0,
    };
    
    env.storage()
        .instance()
        .set(&EventKey::Event(event_id), &event);
    
    add_to_active_events(env, event_id)?;
    
    // Emit EventScheduled event
    env.events().publish(
        (symbol_short!("evt_sched"), event_id),
        (event_type, start_time, reward_pool),
    );
    
    Ok(event_id)
}

/// Trigger a scheduled event when its time arrives.
pub fn trigger_scheduled_event(
    env: &Env,
    event_id: u64,
) -> Result<EventResult, EventError> {
    let mut event: ScheduledEvent = env
        .storage()
        .instance()
        .get(&EventKey::Event(event_id))
        .ok_or(EventError::EventNotFound)?;
    
    if event.executed {
        return Err(EventError::EventAlreadyExecuted);
    }
    
    let current_time = get_current_timestamp(env);
    
    if current_time < event.start_time {
        return Err(EventError::EventNotReady);
    }
    
    // Mark as executed
    event.executed = true;
    env.storage()
        .instance()
        .set(&EventKey::Event(event_id), &event);
    
    // Remove from active events
    remove_from_active_events(env, event_id);
    
    // Simulate reward distribution (in production, integrate with prize_distributor)
    let rewards_distributed = event.reward_pool;
    let participants = event.participants;
    
    let result = EventResult {
        event_id,
        executed_at: current_time,
        rewards_distributed,
        participants,
    };
    
    // Emit EventTriggered event
    env.events().publish(
        (symbol_short!("evt_trig"), event_id),
        (current_time, rewards_distributed, participants),
    );
    
    Ok(result)
}

/// Get event details by ID.
pub fn get_event(env: &Env, event_id: u64) -> Result<ScheduledEvent, EventError> {
    env.storage()
        .instance()
        .get(&EventKey::Event(event_id))
        .ok_or(EventError::EventNotFound)
}

/// Get all active event IDs.
pub fn get_active_events(env: &Env) -> Vec<u64> {
    env.storage()
        .instance()
        .get(&EventKey::ActiveEvents)
        .unwrap_or(Vec::new(env))
}

/// Schedule a weekly nebula festival (template).
pub fn schedule_weekly_festival(
    env: &Env,
    admin: Address,
    reward_pool: i128,
) -> Result<u64, EventError> {
    let current_time = get_current_timestamp(env);
    let start_time = current_time + WEEKLY_FESTIVAL_INTERVAL;
    
    schedule_event(
        env,
        admin,
        symbol_short!("festival"),
        start_time,
        reward_pool,
    )
}

/// Cancel a scheduled event (admin only).
pub fn cancel_event(
    env: &Env,
    admin: Address,
    event_id: u64,
) -> Result<(), EventError> {
    require_admin(env, &admin)?;
    
    let mut event: ScheduledEvent = env
        .storage()
        .instance()
        .get(&EventKey::Event(event_id))
        .ok_or(EventError::EventNotFound)?;
    
    if event.executed {
        return Err(EventError::EventAlreadyExecuted);
    }
    
    // Mark as executed to prevent triggering
    event.executed = true;
    env.storage()
        .instance()
        .set(&EventKey::Event(event_id), &event);
    
    remove_from_active_events(env, event_id);
    
    // Emit cancellation event
    env.events().publish(
        (symbol_short!("evt_cncl"), event_id),
        admin,
    );
    
    Ok(())
}

/// Update event participant count (for future integration).
pub fn update_participants(
    env: &Env,
    event_id: u64,
    participant_count: u32,
) -> Result<(), EventError> {
    let mut event: ScheduledEvent = env
        .storage()
        .instance()
        .get(&EventKey::Event(event_id))
        .ok_or(EventError::EventNotFound)?;
    
    event.participants = participant_count;
    env.storage()
        .instance()
        .set(&EventKey::Event(event_id), &event);
    
    Ok(())
}

/// Get total number of events scheduled.
pub fn get_event_count(env: &Env) -> u64 {
    env.storage()
        .instance()
        .get(&EventKey::EventCounter)
        .unwrap_or(0)
}
