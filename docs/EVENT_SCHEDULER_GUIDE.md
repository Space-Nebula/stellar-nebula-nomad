# Automated Community Event Scheduler Guide

## Overview

The Event Scheduler module enables automated, time-based community events in Nebula Nomad. It provides a robust system for scheduling recurring events like weekly festivals, raids, harvests, PvP tournaments, and exploration challenges.

## Features

- **Time-Based Scheduling**: Schedule events with precise start times
- **Multiple Event Types**: Support for festivals, raids, harvests, PvP, and exploration events
- **Admin Controls**: Secure admin-only scheduling and cancellation
- **Burst Protection**: Rate limiting with max 20 active events
- **Event Lifecycle**: Complete lifecycle from scheduling to execution
- **Integration Ready**: Designed to integrate with prize distributor and other modules

## Architecture

### Storage Keys

```rust
pub enum EventKey {
    Event(u64),           // Event data by ID
    EventCounter,         // Global event counter
    Admin,                // Admin address
    ActiveEvents,         // List of active event IDs
    BurstCounter,         // Rate limiting counter
}
```

### Data Types

#### ScheduledEvent

```rust
pub struct ScheduledEvent {
    pub event_id: u64,
    pub event_type: Symbol,
    pub start_time: u64,
    pub creator: Address,
    pub executed: bool,
    pub reward_pool: i128,
    pub participants: u32,
}
```

#### EventResult

```rust
pub struct EventResult {
    pub event_id: u64,
    pub executed_at: u64,
    pub rewards_distributed: i128,
    pub participants: u32,
}
```

## Event Types

The scheduler supports five event types:

1. **festival** - Weekly community celebrations with large reward pools
2. **raid** - Cooperative boss battles
3. **harvest** - Resource gathering competitions
4. **pvp** - Player vs player tournaments
5. **explore** - Nebula exploration challenges

## API Reference

### Initialization

```rust
pub fn initialize_scheduler(env: Env, admin: Address)
```

Initialize the event scheduler with an admin address. Must be called once before scheduling events.

**Parameters:**

- `admin`: Address with scheduling permissions

**Events Emitted:**

- `init_sch`: Scheduler initialized

### Scheduling Events

```rust
pub fn schedule_event(
    env: Env,
    admin: Address,
    event_type: Symbol,
    start_time: u64,
    reward_pool: i128,
) -> Result<u64, EventError>
```

Schedule a new community event.

**Parameters:**

- `admin`: Admin address (requires auth)
- `event_type`: One of: festival, raid, harvest, pvp, explore
- `start_time`: Unix timestamp when event should start
- `reward_pool`: Amount of rewards to distribute

**Returns:**

- Event ID on success

**Errors:**

- `EventAlreadyPassed`: Start time is in the past
- `InvalidEventType`: Unknown event type
- `TooManyActiveEvents`: Max 20 active events reached
- `BurstLimitExceeded`: Rate limit exceeded
- `Unauthorized`: Caller is not admin

**Events Emitted:**

- `evt_sched`: Event scheduled with (event_type, start_time, reward_pool)

### Weekly Festival Template

```rust
pub fn schedule_weekly_festival(
    env: Env,
    admin: Address,
    reward_pool: i128,
) -> Result<u64, EventError>
```

Convenience method to schedule a weekly festival event (7 days from now).

**Parameters:**

- `admin`: Admin address
- `reward_pool`: Reward amount

**Returns:**

- Event ID

### Triggering Events

```rust
pub fn trigger_scheduled_event(
    env: Env,
    event_id: u64,
) -> Result<EventResult, EventError>
```

Execute a scheduled event when its start time arrives.

**Parameters:**

- `event_id`: ID of event to trigger

**Returns:**

- EventResult with execution details

**Errors:**

- `EventNotFound`: Event doesn't exist
- `EventAlreadyExecuted`: Event already triggered
- `EventNotReady`: Current time < start time

**Events Emitted:**

- `evt_trig`: Event triggered with (executed_at, rewards_distributed, participants)

### Event Management

```rust
pub fn cancel_event(
    env: Env,
    admin: Address,
    event_id: u64,
) -> Result<(), EventError>
```

Cancel a scheduled event (admin only).

```rust
pub fn update_event_participants(
    env: Env,
    event_id: u64,
    participant_count: u32,
) -> Result<(), EventError>
```

Update the participant count for an event.

### Query Functions

```rust
pub fn get_event(env: Env, event_id: u64) -> Result<ScheduledEvent, EventError>
```

Get event details by ID.

```rust
pub fn get_active_events(env: Env) -> Vec<u64>
```

Get list of all active (non-executed) event IDs.

```rust
pub fn get_event_count(env: Env) -> u64
```

Get total number of events ever scheduled.

## Usage Examples

### Example 1: Schedule a Weekly Festival

```rust
use soroban_sdk::{symbol_short, Env, Address};

// Initialize scheduler
let admin = Address::generate(&env);
NebulaNomadContract::initialize_scheduler(env.clone(), admin.clone());

// Schedule weekly festival with 50,000 reward pool
let event_id = NebulaNomadContract::schedule_weekly_festival(
    env.clone(),
    admin.clone(),
    50000i128,
).unwrap();

println!("Festival scheduled with ID: {}", event_id);
```

### Example 2: Schedule a Custom Raid Event

```rust
// Get current time and schedule raid for 24 hours from now
let current_time = env.ledger().timestamp();
let raid_time = current_time + (24 * 60 * 60); // 24 hours

let event_id = NebulaNomadContract::schedule_event(
    env.clone(),
    admin.clone(),
    symbol_short!("raid"),
    raid_time,
    25000i128,
).unwrap();
```

### Example 3: Event Lifecycle

```rust
// 1. Schedule event
let start_time = current_time + 3600; // 1 hour from now
let event_id = NebulaNomadContract::schedule_event(
    env.clone(),
    admin.clone(),
    symbol_short!("pvp"),
    start_time,
    10000i128,
).unwrap();

// 2. Update participants as players join
NebulaNomadContract::update_event_participants(
    env.clone(),
    event_id,
    150, // 150 players joined
).unwrap();

// 3. Wait for start time...
// (In production, this would be triggered by a cron job or oracle)

// 4. Trigger event when time arrives
let result = NebulaNomadContract::trigger_scheduled_event(
    env.clone(),
    event_id,
).unwrap();

println!("Event executed! Distributed {} rewards to {} participants",
    result.rewards_distributed,
    result.participants
);
```

### Example 4: Query Active Events

```rust
// Get all active events
let active_events = NebulaNomadContract::get_active_events(env.clone());

for i in 0..active_events.len() {
    let event_id = active_events.get(i).unwrap();
    let event = NebulaNomadContract::get_event(env.clone(), event_id).unwrap();

    println!("Event {}: {} at timestamp {}",
        event.event_id,
        event.event_type,
        event.start_time
    );
}
```

## Security Considerations

### Admin-Only Operations

The following operations require admin authentication:

- `schedule_event`
- `schedule_weekly_festival`
- `cancel_event`

### Rate Limiting

- Maximum 20 active events at any time
- Burst counter prevents rapid scheduling
- Reset burst counter between transactions

### Time Validation

- Events cannot be scheduled in the past
- Events can only be triggered after start time
- Executed events cannot be triggered again

### Event Types

- Only predefined event types are allowed
- Invalid types are rejected at scheduling time

## Integration with Prize Distributor

The event scheduler is designed to integrate with the prize distributor module:

```rust
// Future integration example
pub fn trigger_scheduled_event(env: &Env, event_id: u64) -> Result<EventResult, EventError> {
    // ... existing code ...

    // Integrate with prize distributor
    if event.reward_pool > 0 {
        prize_distributor::distribute_event_rewards(
            env,
            event_id,
            event.reward_pool,
            event.participants,
        )?;
    }

    // ... rest of code ...
}
```

## Future Enhancements

### Player-Voted Events

Future versions will support community voting on event types and schedules:

```rust
// Proposed API
pub fn propose_event(
    env: Env,
    proposer: Address,
    event_type: Symbol,
    start_time: u64,
) -> Result<u64, EventError>;

pub fn vote_on_event(
    env: Env,
    voter: Address,
    proposal_id: u64,
    support: bool,
) -> Result<(), EventError>;

pub fn finalize_proposal(
    env: Env,
    proposal_id: u64,
) -> Result<u64, EventError>; // Returns event_id if approved
```

### Recurring Events

Automatic rescheduling of recurring events:

```rust
pub fn schedule_recurring_event(
    env: Env,
    admin: Address,
    event_type: Symbol,
    interval: u64, // seconds between occurrences
    reward_pool: i128,
) -> Result<u64, EventError>;
```

### Event Rewards Distribution

Direct integration with reward distribution:

```rust
pub struct EventRewards {
    pub first_place: i128,
    pub second_place: i128,
    pub third_place: i128,
    pub participation: i128,
}
```

## Testing

Run the comprehensive test suite:

```bash
cargo test test_event_scheduler
```

### Test Coverage

- ✅ Initialization
- ✅ Event scheduling (success and failure cases)
- ✅ Time validation
- ✅ Event type validation
- ✅ Event triggering
- ✅ Event cancellation
- ✅ Participant updates
- ✅ Multiple events
- ✅ Max active events limit
- ✅ Complete event lifecycle
- ✅ All event types

## Error Handling

| Error                  | Code | Description                  |
| ---------------------- | ---- | ---------------------------- |
| `EventAlreadyPassed`   | 1    | Start time is in the past    |
| `EventNotFound`        | 2    | Event ID doesn't exist       |
| `EventAlreadyExecuted` | 3    | Event already triggered      |
| `EventNotReady`        | 4    | Current time < start time    |
| `Unauthorized`         | 5    | Caller is not admin          |
| `TooManyActiveEvents`  | 6    | Max 20 active events reached |
| `BurstLimitExceeded`   | 7    | Rate limit exceeded          |
| `InvalidEventType`     | 8    | Unknown event type           |

## Events Emitted

| Event       | Topics   | Data                                             |
| ----------- | -------- | ------------------------------------------------ |
| `init_sch`  | -        | admin                                            |
| `evt_sched` | event_id | (event_type, start_time, reward_pool)            |
| `evt_trig`  | event_id | (executed_at, rewards_distributed, participants) |
| `evt_cncl`  | event_id | admin                                            |

## Best Practices

1. **Initialize Once**: Call `initialize_scheduler` only once during contract deployment
2. **Time Buffer**: Schedule events with sufficient lead time (recommended: 1+ hours)
3. **Monitor Active Events**: Keep track of active events to avoid hitting the 20-event limit
4. **Participant Updates**: Update participant counts as players join for accurate reward distribution
5. **Event Triggering**: Use automated systems (oracles, cron jobs) to trigger events at the right time
6. **Error Handling**: Always handle potential errors when scheduling and triggering events

## Community Building

The event scheduler is a powerful tool for community engagement:

- **Regular Festivals**: Build anticipation with weekly or monthly festivals
- **Seasonal Events**: Create special events for holidays or milestones
- **Competitive Events**: Host PvP tournaments and leaderboard competitions
- **Cooperative Events**: Organize raids and group challenges
- **Exploration Events**: Encourage discovery with exploration bonuses

By maintaining a consistent event calendar, you can:

- Increase player retention
- Foster community interaction
- Create memorable experiences
- Reward active participation
- Build long-term engagement
