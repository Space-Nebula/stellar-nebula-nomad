#![no_std]

use soroban_sdk::{contract, contractimpl, Address, Env};

mod nebula_explorer;
pub mod nomad_bonding;
mod resource_minter;
mod ship_registry;

pub use nomad_bonding::{BondStatus, DataKey, NomadBond, YieldDelegation};
pub use resource_minter::Resource;
pub use ship_registry::Ship;

#[contract]
pub struct NebulaNomadContract;

#[contractimpl]
impl NebulaNomadContract {
    // ── Bonding System ────────────────────────────────────────────────

    /// Create a multi-sig bond between the caller and a partner,
    /// linked to the given ship.
    pub fn create_bond(env: Env, initiator: Address, ship_id: u64, partner: Address) -> NomadBond {
        nomad_bonding::create_bond(&env, &initiator, ship_id, &partner)
    }

    /// The designated partner accepts a pending bond.
    pub fn accept_bond(env: Env, partner: Address, bond_id: u64) -> NomadBond {
        nomad_bonding::accept_bond(&env, &partner, bond_id)
    }

    /// Set up passive yield delegation on an active bond.
    /// `percentage` is 1–100.
    pub fn delegate_yield(
        env: Env,
        delegator: Address,
        bond_id: u64,
        percentage: u32,
    ) -> YieldDelegation {
        nomad_bonding::delegate_yield(&env, &delegator, bond_id, percentage)
    }

    /// Award cosmic essence to a player (game-logic entry point).
    pub fn accrue_essence(env: Env, player: Address, amount: u64) {
        nomad_bonding::accrue_essence(&env, &player, amount);
    }

    /// Beneficiary claims their delegated yield share.
    pub fn claim_yield(env: Env, claimer: Address, bond_id: u64) -> u64 {
        nomad_bonding::claim_yield(&env, &claimer, bond_id)
    }

    /// Either bonded party dissolves the bond.
    pub fn dissolve_bond(env: Env, caller: Address, bond_id: u64) -> NomadBond {
        nomad_bonding::dissolve_bond(&env, &caller, bond_id)
    }

    // ── Read-only views ───────────────────────────────────────────────

    /// View bond details.
    pub fn get_bond(env: Env, bond_id: u64) -> NomadBond {
        nomad_bonding::get_bond(&env, bond_id)
    }

    /// View yield delegation for a bond.
    pub fn get_yield_delegation(env: Env, bond_id: u64) -> YieldDelegation {
        nomad_bonding::get_yield_delegation(&env, bond_id)
    }

    /// View a player's cosmic essence balance.
    pub fn get_essence_balance(env: Env, player: Address) -> u64 {
        nomad_bonding::get_essence_balance(&env, &player)
    }
}

