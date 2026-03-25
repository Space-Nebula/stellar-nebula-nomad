#![no_std]

use soroban_sdk::{contract, contractimpl, Address, BytesN, Env, Symbol, Vec};

mod blueprint_factory;
mod nebula_explorer;
mod player_profile;
mod referral_system;
mod resource_minter;
mod session_manager;
mod ship_registry;

pub use nebula_explorer::{
    calculate_rarity_tier, compute_layout_hash, generate_nebula_layout, CellType, NebulaCell,
    NebulaLayout, Rarity, GRID_SIZE, TOTAL_CELLS,
};
pub use blueprint_factory::{Blueprint, BlueprintError, BlueprintRarity};
pub use referral_system::{Referral, ReferralError};
pub use player_profile::{PlayerProfile, ProfileError, ProgressUpdate};
pub use resource_minter::Resource;
pub use session_manager::{Session, SessionError};
pub use ship_registry::Ship;

#[contract]
pub struct NebulaNomadContract;

#[contractimpl]
impl NebulaNomadContract {
    /// Generate a 16×16 procedural nebula map using ledger-seeded PRNG.
    ///
    /// Combines the supplied `seed` with on-chain ledger sequence and
    /// timestamp. The player must authorize the call.
    pub fn generate_nebula_layout(
        env: Env,
        seed: BytesN<32>,
        player: Address,
    ) -> NebulaLayout {
        player.require_auth();
        nebula_explorer::generate_nebula_layout(&env, &seed, &player)
    }

    /// Calculate the rarity tier of a nebula layout using on-chain
    /// verifiable math (no off-chain RNG).
    pub fn calculate_rarity_tier(env: Env, layout: NebulaLayout) -> Rarity {
        nebula_explorer::calculate_rarity_tier(&env, &layout)
    }

    /// Full scan: generates layout, calculates rarity, and emits a
    /// `NebulaScanned` event containing the layout hash.
    pub fn scan_nebula(
        env: Env,
        seed: BytesN<32>,
        player: Address,
    ) -> (NebulaLayout, Rarity) {
        player.require_auth();

        let layout = nebula_explorer::generate_nebula_layout(&env, &seed, &player);
        let rarity = nebula_explorer::calculate_rarity_tier(&env, &layout);
        let layout_hash = nebula_explorer::compute_layout_hash(&env, &layout);

        nebula_explorer::emit_nebula_scanned(&env, &player, &layout_hash, &rarity);

        (layout, rarity)
    }

    // ─── Player Profile ───────────────────────────────────────────────────────

    /// Create a new on-chain player profile. Returns the assigned profile ID.
    pub fn initialize_profile(env: Env, owner: Address) -> Result<u64, ProfileError> {
        player_profile::initialize_profile(&env, owner)
    }

    /// Update scan count and essence earned after a harvest. Owner-only.
    pub fn update_progress(
        env: Env,
        caller: Address,
        profile_id: u64,
        scan_count: u32,
        essence: i128,
    ) -> Result<(), ProfileError> {
        player_profile::update_progress(&env, caller, profile_id, scan_count, essence)
    }

    /// Apply up to 5 stat updates in a single transaction for multi-scan runs.
    pub fn batch_update_progress(
        env: Env,
        caller: Address,
        updates: Vec<ProgressUpdate>,
    ) -> Result<(), ProfileError> {
        player_profile::batch_update_progress(&env, caller, updates)
    }

    /// Retrieve a player profile by ID.
    pub fn get_profile(env: Env, profile_id: u64) -> Result<PlayerProfile, ProfileError> {
        player_profile::get_profile(&env, profile_id)
    }

    // ─── Session Manager ──────────────────────────────────────────────────────

    /// Start a timed nebula exploration session for a ship. Max 3 per player.
    pub fn start_session(env: Env, owner: Address, ship_id: u64) -> Result<u64, SessionError> {
        session_manager::start_session(&env, owner, ship_id)
    }

    /// Close a session. Owner can force-close; anyone can clean up expired ones.
    pub fn expire_session(
        env: Env,
        caller: Address,
        session_id: u64,
    ) -> Result<(), SessionError> {
        session_manager::expire_session(&env, caller, session_id)
    }

    /// Retrieve session data by ID.
    pub fn get_session(env: Env, session_id: u64) -> Result<Session, SessionError> {
        session_manager::get_session(&env, session_id)
    }

    // ─── Blueprint Factory ────────────────────────────────────────────────────

    /// Mint a blueprint NFT from harvested resource components.
    pub fn craft_blueprint(
        env: Env,
        owner: Address,
        components: Vec<Symbol>,
    ) -> Result<u64, BlueprintError> {
        blueprint_factory::craft_blueprint(&env, owner, components)
    }

    /// Craft up to 2 blueprints in a single transaction.
    pub fn batch_craft_blueprints(
        env: Env,
        owner: Address,
        recipes: Vec<Vec<Symbol>>,
    ) -> Result<Vec<u64>, BlueprintError> {
        blueprint_factory::batch_craft_blueprints(&env, owner, recipes)
    }

    /// Consume a blueprint and permanently upgrade a ship.
    pub fn apply_blueprint_to_ship(
        env: Env,
        owner: Address,
        blueprint_id: u64,
        ship_id: u64,
    ) -> Result<(), BlueprintError> {
        blueprint_factory::apply_blueprint_to_ship(&env, owner, blueprint_id, ship_id)
    }

    /// Retrieve a blueprint by ID.
    pub fn get_blueprint(env: Env, blueprint_id: u64) -> Result<Blueprint, BlueprintError> {
        blueprint_factory::get_blueprint(&env, blueprint_id)
    }

    // ─── Referral System ──────────────────────────────────────────────────────

    /// Record an on-chain referral from `referrer` for `new_nomad`.
    pub fn register_referral(
        env: Env,
        referrer: Address,
        new_nomad: Address,
    ) -> Result<u64, ReferralError> {
        referral_system::register_referral(&env, referrer, new_nomad)
    }

    /// Mark that `nomad` has completed their first scan, unlocking the reward.
    pub fn mark_first_scan(env: Env, nomad: Address) -> Result<(), ReferralError> {
        referral_system::mark_first_scan(&env, nomad)
    }

    /// Claim the essence referral reward. One-time, capped at 10 claims/day.
    pub fn claim_referral_reward(
        env: Env,
        referrer: Address,
        new_nomad: Address,
    ) -> Result<i128, ReferralError> {
        referral_system::claim_referral_reward(&env, referrer, new_nomad)
    }

    /// Retrieve a referral record by the new nomad's address.
    pub fn get_referral(env: Env, new_nomad: Address) -> Result<Referral, ReferralError> {
        referral_system::get_referral(&env, new_nomad)
    }
}

