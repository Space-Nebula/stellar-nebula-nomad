use crate::ship_nft::{DataKey as ShipDataKey, ShipNft};
use soroban_sdk::{contracterror, contracttype, symbol_short, Env};

// ─── Storage Keys ─────────────────────────────────────────────────────────

#[derive(Clone)]
#[contracttype]
pub enum EnergyKey {
    /// Per-ship energy balance: `EnergyBalance(ship_id)`.
    EnergyBalance(u64),
    /// Global base recharge efficiency rate (u32, percentage).
    BaseRechargeRate,
    /// Per-ship blueprint-derived efficiency bonus: `EfficiencyBonus(ship_id)`.
    EfficiencyBonus(u64),
}

// ─── Custom Errors ────────────────────────────────────────────────────────

#[contracterror]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum EnergyError {
    InsufficientEnergy = 1,
    ShipNotFound = 2,
    InvalidAmount = 3,
    Overflow = 4,
}

// ─── Constants ────────────────────────────────────────────────────────────

/// Default recharge efficiency: 50%.
const DEFAULT_RECHARGE_RATE: u32 = 50;

/// Maximum energy cap to prevent overflow.
const MAX_ENERGY: u32 = u32::MAX;

// ─── Internal Helpers ─────────────────────────────────────────────────────

/// Verify that a ship exists in persistent storage.
fn require_ship_exists(env: &Env, ship_id: u64) -> Result<(), EnergyError> {
    let _ship: ShipNft = env
        .storage()
        .persistent()
        .get(&ShipDataKey::Ship(ship_id))
        .ok_or(EnergyError::ShipNotFound)?;
    Ok(())
}

// ─── Public API (called by NebulaNomadContract in lib.rs) ─────────────────

/// Initialize energy for a ship with a given starting balance.
///
/// The ship must already exist (minted via `ship_nft`). Sets the initial
/// energy balance and emits an initialization event.
pub fn initialize_energy(env: &Env, ship_id: u64, initial_energy: u32) -> Result<(), EnergyError> {
    require_ship_exists(env, ship_id)?;

    env.storage()
        .persistent()
        .set(&EnergyKey::EnergyBalance(ship_id), &initial_energy);

    env.events().publish(
        (symbol_short!("energy"), symbol_short!("init")),
        (ship_id, initial_energy),
    );

    Ok(())
}

/// Consume energy from a ship's balance for actions like scans and harvests.
///
/// Validates that the amount is non-zero and that the ship has sufficient
/// energy. Returns the remaining balance after deduction.
pub fn consume_energy(env: &Env, ship_id: u64, amount: u32) -> Result<u32, EnergyError> {
    if amount == 0 {
        return Err(EnergyError::InvalidAmount);
    }

    require_ship_exists(env, ship_id)?;

    let balance: u32 = env
        .storage()
        .persistent()
        .get(&EnergyKey::EnergyBalance(ship_id))
        .unwrap_or(0);

    if balance < amount {
        return Err(EnergyError::InsufficientEnergy);
    }

    // Safe: underflow impossible due to the check above.
    let new_balance = balance - amount;

    env.storage()
        .persistent()
        .set(&EnergyKey::EnergyBalance(ship_id), &new_balance);

    env.events().publish(
        (symbol_short!("energy"), symbol_short!("consumed")),
        (ship_id, amount, new_balance),
    );

    Ok(new_balance)
}

/// Convert resources into ship energy using the effective recharge rate.
///
/// The effective rate is `base_rate + per-ship bonus`, capped at 100%.
/// Energy gained is computed in i128 space to prevent intermediate overflow,
/// then clamped to u32::MAX via saturating_add.
pub fn recharge_energy(
    env: &Env,
    ship_id: u64,
    resource_amount: i128,
) -> Result<u32, EnergyError> {
    if resource_amount <= 0 {
        return Err(EnergyError::InvalidAmount);
    }

    require_ship_exists(env, ship_id)?;

    let base_rate: u32 = env
        .storage()
        .instance()
        .get(&EnergyKey::BaseRechargeRate)
        .unwrap_or(DEFAULT_RECHARGE_RATE);

    let bonus: u32 = env
        .storage()
        .persistent()
        .get(&EnergyKey::EfficiencyBonus(ship_id))
        .unwrap_or(0);

    // Cap effective rate at 100%.
    let effective_rate: u32 = {
        let sum = base_rate.saturating_add(bonus);
        if sum > 100 { 100 } else { sum }
    };

    // Compute in i128 to avoid intermediate overflow.
    let gained_i128 = resource_amount * (effective_rate as i128) / 100;

    // Clamp to u32 range.
    let energy_gained: u32 = if gained_i128 > MAX_ENERGY as i128 {
        MAX_ENERGY
    } else {
        gained_i128 as u32
    };

    let balance: u32 = env
        .storage()
        .persistent()
        .get(&EnergyKey::EnergyBalance(ship_id))
        .unwrap_or(0);

    let new_balance = balance.saturating_add(energy_gained);

    env.storage()
        .persistent()
        .set(&EnergyKey::EnergyBalance(ship_id), &new_balance);

    env.events().publish(
        (symbol_short!("energy"), symbol_short!("rechargd")),
        (ship_id, energy_gained, new_balance),
    );

    Ok(new_balance)
}

/// Read the current energy balance for a ship (view function).
pub fn get_energy(env: &Env, ship_id: u64) -> u32 {
    env.storage()
        .persistent()
        .get(&EnergyKey::EnergyBalance(ship_id))
        .unwrap_or(0)
}

/// Set the global base recharge efficiency rate (admin function).
pub fn set_base_recharge_rate(env: &Env, rate: u32) {
    env.storage()
        .instance()
        .set(&EnergyKey::BaseRechargeRate, &rate);
}

/// Apply a blueprint-derived efficiency bonus to a specific ship.
///
/// This enables dynamic upgrade paths: craft a blueprint, then apply it
/// to boost a ship's recharge efficiency permanently.
pub fn apply_efficiency_bonus(env: &Env, ship_id: u64, bonus: u32) -> Result<(), EnergyError> {
    require_ship_exists(env, ship_id)?;

    env.storage()
        .persistent()
        .set(&EnergyKey::EfficiencyBonus(ship_id), &bonus);

    env.events().publish(
        (symbol_short!("energy"), symbol_short!("upgrade")),
        (ship_id, bonus),
    );

    Ok(())
}

/// Read the current base recharge rate (view function).
pub fn get_recharge_rate(env: &Env) -> u32 {
    env.storage()
        .instance()
        .get(&EnergyKey::BaseRechargeRate)
        .unwrap_or(DEFAULT_RECHARGE_RATE)
}
