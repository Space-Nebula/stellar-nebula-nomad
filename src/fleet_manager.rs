use soroban_sdk::{contracterror, contracttype, symbol_short, xdr::ToXdr, Address, Bytes, Env, Vec};

use crate::Ship;

const MAX_SHIPS_PER_FLEET: u32 = 10;

#[derive(Clone)]
#[contracttype]
pub struct Fleet {
    pub id: u64,
    pub owner: Address,
    pub ship_ids: Vec<u64>,
    pub template_id: u32,
    pub created_at: u64,
    pub immutable_membership: bool,
}

#[derive(Clone)]
#[contracttype]
pub struct FleetStatus {
    pub fleet_id: u64,
    pub total_level: u32,
    pub average_scan_range: u32,
    pub vessel_count: u32,
    pub synced_at: u64,
}

#[derive(Clone)]
#[contracttype]
pub struct FleetTemplate {
    pub id: u32,
    pub name: soroban_sdk::Symbol,
    pub bonus_scan_range: u32,
}

#[derive(Copy, Clone, Eq, PartialEq)]
#[repr(u32)]
#[contracterror]
pub enum FleetError {
    FleetLimitExceeded = 1,
    EmptyFleet = 2,
    Unauthorized = 3,
    ShipNotFound = 4,
    ShipOwnershipMismatch = 5,
    FleetNotFound = 6,
    AlreadyInitialized = 7,
}

fn template_key(id: u32) -> (soroban_sdk::Symbol, u32) {
    (symbol_short!("flt_tpl"), id)
}

fn fleet_key(id: u64) -> (soroban_sdk::Symbol, u64) {
    (symbol_short!("fleet"), id)
}

fn owner_fleet_key(owner: &Address) -> (soroban_sdk::Symbol, Address) {
    (symbol_short!("flt_own"), owner.clone())
}

fn status_key(id: u64) -> (soroban_sdk::Symbol, u64) {
    (symbol_short!("flt_sts"), id)
}

fn ship_key(ship_id: u64) -> (soroban_sdk::Symbol, u64) {
    (symbol_short!("ship"), ship_id)
}

fn init_key() -> soroban_sdk::Symbol {
    symbol_short!("flt_ini")
}

fn hash_fleet_id(env: &Env, owner: &Address) -> u64 {
    let mut input = Bytes::new(env);
    input.append(&owner.clone().to_xdr(env));
    input.append(&Bytes::from_slice(
        env,
        &env.ledger().timestamp().to_be_bytes(),
    ));
    input.append(&Bytes::from_slice(
        env,
        &env.ledger().sequence().to_be_bytes(),
    ));

    let hash = env.crypto().sha256(&input);
    let hash_bytes: Bytes = hash.into();

    let mut out: u64 = 0;
    for i in 0..8u32 {
        out = (out << 8) | hash_bytes.get(i).unwrap_or(0) as u64;
    }
    out
}

pub fn init_fleet_templates(env: &Env) -> Result<(), FleetError> {
    if env.storage().persistent().has(&init_key()) {
        return Err(FleetError::AlreadyInitialized);
    }

    let scout = FleetTemplate {
        id: 1,
        name: symbol_short!("scout"),
        bonus_scan_range: 3,
    };
    let miner = FleetTemplate {
        id: 2,
        name: symbol_short!("miner"),
        bonus_scan_range: 2,
    };
    let balanced = FleetTemplate {
        id: 3,
        name: symbol_short!("hybrid"),
        bonus_scan_range: 1,
    };

    env.storage().persistent().set(&template_key(1), &scout);
    env.storage().persistent().set(&template_key(2), &miner);
    env.storage().persistent().set(&template_key(3), &balanced);
    env.storage().persistent().set(&init_key(), &true);

    Ok(())
}

pub fn register_ship_for_owner(env: &Env, owner: &Address, ship: Ship) -> Result<(), FleetError> {
    owner.require_auth();
    if ship.owner != *owner {
        return Err(FleetError::ShipOwnershipMismatch);
    }
    env.storage().persistent().set(&ship_key(ship.id), &ship);
    Ok(())
}

pub fn register_fleet(
    env: &Env,
    owner: &Address,
    ship_ids: Vec<u64>,
    template_id: u32,
) -> Result<Fleet, FleetError> {
    owner.require_auth();

    if ship_ids.is_empty() {
        return Err(FleetError::EmptyFleet);
    }

    if ship_ids.len() > MAX_SHIPS_PER_FLEET {
        return Err(FleetError::FleetLimitExceeded);
    }

    for i in 0..ship_ids.len() {
        let ship_id = ship_ids.get(i).ok_or(FleetError::ShipNotFound)?;
        let ship = env
            .storage()
            .persistent()
            .get::<_, Ship>(&ship_key(ship_id))
            .ok_or(FleetError::ShipNotFound)?;
        if ship.owner != *owner {
            return Err(FleetError::ShipOwnershipMismatch);
        }
    }

    let fleet_id = hash_fleet_id(env, owner);

    let fleet = Fleet {
        id: fleet_id,
        owner: owner.clone(),
        ship_ids,
        template_id,
        created_at: env.ledger().timestamp(),
        immutable_membership: true,
    };

    env.storage().persistent().set(&fleet_key(fleet_id), &fleet);
    env.storage()
        .persistent()
        .set(&owner_fleet_key(owner), &fleet_id);

    env.events().publish(
        (symbol_short!("fleet"), symbol_short!("reg")),
        (fleet_id, owner.clone()),
    );

    Ok(fleet)
}

pub fn sync_fleet_status(env: &Env, fleet_id: u64) -> Result<FleetStatus, FleetError> {
    let fleet = env
        .storage()
        .persistent()
        .get::<_, Fleet>(&fleet_key(fleet_id))
        .ok_or(FleetError::FleetNotFound)?;

    let template = env
        .storage()
        .persistent()
        .get::<_, FleetTemplate>(&template_key(fleet.template_id));

    let mut total_level: u32 = 0;
    let mut total_scan_range: u32 = 0;

    for i in 0..fleet.ship_ids.len() {
        let ship_id = fleet.ship_ids.get(i).ok_or(FleetError::ShipNotFound)?;
        let ship = env
            .storage()
            .persistent()
            .get::<_, Ship>(&ship_key(ship_id))
            .ok_or(FleetError::ShipNotFound)?;

        total_level += ship.level;
        total_scan_range += ship.scan_range;
    }

    if let Some(t) = template {
        total_scan_range += t.bonus_scan_range.saturating_mul(fleet.ship_ids.len());
    }

    let vessel_count = fleet.ship_ids.len();
    let average_scan_range = if vessel_count == 0 {
        0
    } else {
        total_scan_range / vessel_count
    };

    let status = FleetStatus {
        fleet_id,
        total_level,
        average_scan_range,
        vessel_count,
        synced_at: env.ledger().timestamp(),
    };

    env.storage()
        .persistent()
        .set(&status_key(fleet_id), &status);
    env.events().publish(
        (symbol_short!("fleet"), symbol_short!("sync")),
        (fleet_id, total_level),
    );

    Ok(status)
}

pub fn get_fleet(env: &Env, fleet_id: u64) -> Option<Fleet> {
    env.storage().persistent().get(&fleet_key(fleet_id))
}

pub fn get_fleet_status(env: &Env, fleet_id: u64) -> Option<FleetStatus> {
    env.storage().persistent().get(&status_key(fleet_id))
}
