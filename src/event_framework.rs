use soroban_sdk::{contracterror, contracttype, symbol_short, Address, BytesN, Env, Symbol, Vec};

const MAX_BURST_EVENTS: u32 = 15;
const DEFAULT_SCHEMA_VERSION: u32 = 1;

#[derive(Clone)]
#[contracttype]
pub struct StandardEvent {
    pub id: u64,
    pub event_type: Symbol,
    pub payload: BytesN<256>,
    pub version: u32,
    pub caller: Address,
    pub timestamp: u64,
}

#[derive(Copy, Clone, Eq, PartialEq)]
#[repr(u32)]
#[contracterror]
pub enum EventFrameworkError {
    InvalidEventType = 1,
    LimitTooLarge = 2,
    Unauthorized = 3,
}

fn schema_key(event_type: &Symbol) -> (Symbol, Symbol) {
    (symbol_short!("ev_sch"), event_type.clone())
}

fn index_key() -> Symbol {
    symbol_short!("ev_idx")
}

fn event_key(index: u64) -> (Symbol, u64) {
    (symbol_short!("ev_rec"), index)
}

fn ts_seq_key(timestamp: u64) -> (Symbol, u64) {
    (symbol_short!("ev_tsq"), timestamp)
}

fn admin_key() -> Symbol {
    symbol_short!("ev_adm")
}

fn ensure_defaults(env: &Env) {
    if !env
        .storage()
        .persistent()
        .has(&schema_key(&symbol_short!("system")))
    {
        env.storage().persistent().set(
            &schema_key(&symbol_short!("system")),
            &DEFAULT_SCHEMA_VERSION,
        );
        env.storage().persistent().set(
            &schema_key(&symbol_short!("tutorial")),
            &DEFAULT_SCHEMA_VERSION,
        );
        env.storage().persistent().set(
            &schema_key(&symbol_short!("fleet")),
            &DEFAULT_SCHEMA_VERSION,
        );
        env.storage().persistent().set(
            &schema_key(&symbol_short!("bounty")),
            &DEFAULT_SCHEMA_VERSION,
        );
    }
}

pub fn init_event_framework(env: &Env, admin: &Address) {
    admin.require_auth();
    ensure_defaults(env);
    env.storage().persistent().set(&admin_key(), admin);
}

pub fn register_event_schema(
    env: &Env,
    admin: &Address,
    event_type: Symbol,
    version: u32,
) -> Result<(), EventFrameworkError> {
    admin.require_auth();

    let stored_admin = env
        .storage()
        .persistent()
        .get::<_, Address>(&admin_key())
        .ok_or(EventFrameworkError::Unauthorized)?;

    if stored_admin != *admin {
        return Err(EventFrameworkError::Unauthorized);
    }

    env.storage()
        .persistent()
        .set(&schema_key(&event_type), &version);
    Ok(())
}

pub fn emit_standard_event(
    env: &Env,
    caller: &Address,
    event_type: Symbol,
    payload: BytesN<256>,
) -> Result<u64, EventFrameworkError> {
    caller.require_auth();
    ensure_defaults(env);

    let version = env
        .storage()
        .persistent()
        .get::<_, u32>(&schema_key(&event_type))
        .ok_or(EventFrameworkError::InvalidEventType)?;

    let index = env
        .storage()
        .persistent()
        .get::<_, u64>(&index_key())
        .unwrap_or(0)
        + 1;
    env.storage().persistent().set(&index_key(), &index);

    let timestamp = env.ledger().timestamp();
    let seq_in_ts = env
        .storage()
        .persistent()
        .get::<_, u64>(&ts_seq_key(timestamp))
        .unwrap_or(0)
        + 1;
    env.storage()
        .persistent()
        .set(&ts_seq_key(timestamp), &seq_in_ts);

    let id = timestamp
        .saturating_mul(1_000_000)
        .saturating_add(seq_in_ts);

    let record = StandardEvent {
        id,
        event_type: event_type.clone(),
        payload: payload.clone(),
        version,
        caller: caller.clone(),
        timestamp,
    };

    env.storage().persistent().set(&event_key(index), &record);

    env.events().publish(
        (symbol_short!("std_evt"), event_type),
        (id, version, caller.clone(), payload),
    );

    Ok(id)
}

pub fn emit_standard_event_burst(
    env: &Env,
    caller: &Address,
    event_type: Symbol,
    payloads: Vec<BytesN<256>>,
) -> Result<u32, EventFrameworkError> {
    if payloads.len() > MAX_BURST_EVENTS {
        return Err(EventFrameworkError::LimitTooLarge);
    }

    let mut emitted: u32 = 0;
    for i in 0..payloads.len() {
        let payload = payloads.get(i).ok_or(EventFrameworkError::LimitTooLarge)?;
        emit_standard_event(env, caller, event_type.clone(), payload)?;
        emitted += 1;
    }
    Ok(emitted)
}

pub fn query_recent_events(env: &Env, filter: Symbol, limit: u32) -> Vec<StandardEvent> {
    let mut out = Vec::new(env);
    if limit == 0 {
        return out;
    }

    let max = if limit > 100 { 100 } else { limit };
    let index = env
        .storage()
        .persistent()
        .get::<_, u64>(&index_key())
        .unwrap_or(0);

    let mut scanned: u64 = 0;
    while scanned < index && out.len() < max {
        let current = index - scanned;
        if let Some(event) = env
            .storage()
            .persistent()
            .get::<_, StandardEvent>(&event_key(current))
        {
            if filter == symbol_short!("all") || event.event_type == filter {
                out.push_back(event);
            }
        }
        scanned += 1;
    }

    out
}
