    pub fn set_rate_limit_config(
        env: Env,
        admin: Address,
        op: rate_limiter::Operation,
        config: rate_limiter::RateLimitConfig,
    ) -> Result<(), rate_limiter::RateLimitError> {
        rate_limiter::set_rate_limit_config(&env, &admin, op, config)
    }

    // === Gas Recovery API ===

    pub fn initialize_refund(env: Env, admin: Address) {
        gas_recovery::initialize_refund(&env, &admin);
    }

    pub fn set_refund_percentage(env: Env, admin: Address, bps: u32) {
        gas_recovery::set_refund_percentage(&env, &admin, bps).unwrap();
    }

    pub fn request_refund(
        env: Env,
        caller: Address,
        tx_hash: BytesN<32>,
        gas_used: u64,
    ) -> RefundRequest {
        gas_recovery::request_refund(&env, &caller, tx_hash, gas_used).unwrap()
    }

    pub fn process_refund_batch(env: Env, admin: Address, tx_hashes: Vec<BytesN<32>>) -> u64 {
        gas_recovery::process_refund_batch(&env, &admin, tx_hashes).unwrap()
    }

    // === Bounty Board API ===

    pub fn initialize_bounty_board(env: Env, admin: Address) {
        bounty_board::initialize_bounty_board(&env, &admin);
    }

    pub fn post_bounty(env: Env, poster: Address, description: String, reward: i128) -> Bounty {
        let result = bounty_board::post_bounty(&env, &poster, description, reward).unwrap();
        let _ = audit_logger::log_audit_event(
            &env,
            Some(&poster),
            symbol_short!("pb"),
            BytesN::from_array(&env, &[0u8; 128]),
        );
        result
    }

    pub fn claim_bounty(env: Env, claimer: Address, bounty_id: u64, proof: BytesN<32>) -> Bounty {
        let result = bounty_board::claim_bounty(&env, &claimer, bounty_id, proof).unwrap();
        let mut b = [0u8; 128];
        b[0..8].copy_from_slice(&bounty_id.to_be_bytes());
        let _ = audit_logger::log_audit_event(
            &env,
            Some(&claimer),
            symbol_short!("cb"),
            BytesN::from_array(&env, &b),
        );
        result
    }

    // === Recycling/Crafting API ===

    pub fn initialize_recycling(env: Env) {
        recycling_crafter::initialize_recycling(&env);
    }

    pub fn recycle_resource(
        env: Env,
        caller: Address,
        resource: Symbol,
        amount: u32,
    ) -> Vec<(Symbol, u32)> {
        let result = recycling_crafter::recycle_resource(&env, &caller, resource, amount).unwrap();
        let _ = audit_logger::log_audit_event(
            &env,
            Some(&caller),
            symbol_short!("rr"),
            BytesN::from_array(&env, &[0u8; 128]),
        );
        result
    }

    pub fn craft_new_item(
        env: Env,
        caller: Address,
        recipe_id: u64,
        inputs: Vec<Symbol>,
        quantities: Vec<u32>,
    ) -> CraftingResult {
        let result =
            recycling_crafter::craft_new_item(&env, &caller, recipe_id, inputs, quantities)
                .unwrap();
        let mut b = [0u8; 128];
        b[0..8].copy_from_slice(&recipe_id.to_be_bytes());
        let _ = audit_logger::log_audit_event(
            &env,
            Some(&caller),
            symbol_short!("cn"),
            BytesN::from_array(&env, &b),
        );
        result
    }

    pub fn get_recipe(env: Env, recipe_id: u64) -> Recipe {
        recycling_crafter::get_recipe(&env, recipe_id).unwrap()
    }

    /// Mint a new ship NFT for `owner` with initial stats derived from
    /// `ship_type` and optional free-form `metadata`.
    pub fn mint_ship(
        env: Env,
        owner: Address,
        ship_type: Symbol,
        metadata: Bytes,
    ) -> Result<ShipNft, ShipError> {
        // `ship_nft::mint_ship` authorises the owner; a failed auth reverts
        // this counter along with everything else.
        if let Err(e) = economics::anti_whale::check_operation(&env, &owner, OpKind::Mint, 1) {
            soroban_sdk::panic_with_error!(&env, e);
        }
        let result = ship_nft::mint_ship(&env, &owner, &ship_type, &metadata);
        if result.is_ok() {
            let details = BytesN::from_array(&env, &[0u8; 128]);
            let _ = audit_logger::log_audit_event(&env, Some(&owner), symbol_short!("ms"), details);
        }
        result
    }

    /// Batch-mint up to 3 ship NFTs.
    pub fn batch_mint_ships(
        env: Env,
        owner: Address,
        ship_types: Vec<Symbol>,
        metadata: Bytes,
    ) -> Result<Vec<ShipNft>, ShipError> {
        let count = ship_types.len().max(1);
        if let Err(e) = economics::anti_whale::check_operation(&env, &owner, OpKind::Mint, count) {
            soroban_sdk::panic_with_error!(&env, e);
        }
        let result = ship_nft::batch_mint_ships(&env, &owner, &ship_types, &metadata);
        if result.is_ok() {
            let details = BytesN::from_array(&env, &[0u8; 128]);
            let _ =
                audit_logger::log_audit_event(&env, Some(&owner), symbol_short!("bms"), details);
        }
        result
    }

    /// Transfer ship ownership.
    pub fn transfer_ownership(
        env: Env,
        ship_id: u64,
        new_owner: Address,
    ) -> Result<ShipNft, ShipError> {
        let result = ship_nft::transfer_ownership(&env, ship_id, &new_owner);
        if result.is_ok() {
            let mut b = [0u8; 128];
            b[0..8].copy_from_slice(&ship_id.to_be_bytes());
            let details = BytesN::from_array(&env, &b);
            let _ =
                audit_logger::log_audit_event(&env, Some(&new_owner), symbol_short!("to"), details);
        }
        result
    }

    /// Transfer a ship NFT from `from` to `to`.
    pub fn transfer_ship(
        env: Env,
        ship_id: u64,
        from: Address,
        to: Address,
    ) -> Result<ShipNft, ShipError> {
        let result = ship_nft::transfer_ship(&env, ship_id, &from, &to);
        if result.is_ok() {
            let mut b = [0u8; 128];
            b[0..8].copy_from_slice(&ship_id.to_be_bytes());
            let details = BytesN::from_array(&env, &b);
            let _ = audit_logger::log_audit_event(&env, Some(&from), symbol_short!("ts"), details);
        }
        result
    }

    /// Read a ship by ID.
    pub fn get_ship(env: Env, ship_id: u64) -> Result<ShipNft, ShipError> {
        ship_nft::get_ship(&env, ship_id)
    }

    /// Read all ship IDs owned by `owner`.
    pub fn get_ships_by_owner(env: Env, owner: Address) -> Vec<u64> {
        ship_nft::get_ships_by_owner(&env, &owner)
    }

    /// Set a ship NFT metadata URI for external marketplace compatibility.
    pub fn set_metadata(
        env: Env,
        owner: Address,
        ship_id: u64,
        metadata_uri: Bytes,
    ) -> Result<ShipNft, ShipError> {
        ship_nft::set_metadata(&env, &owner, ship_id, &metadata_uri)
    }

    /// Read a ship NFT metadata URI for marketplace listings.
    pub fn get_metadata(env: Env, ship_id: u64) -> Result<Bytes, ShipError> {
        ship_nft::get_metadata(&env, ship_id)
    }

    /// Gas-optimized harvest of a ship's scanned nebula layout into per-cell
    /// resource balances credited to the ship's current owner.
    pub fn harvest_resources(
        env: Env,
        ship_id: u64,
        layout: NebulaLayout,
    ) -> Result<resource_minter::HarvestResult, resource_minter::HarvestError> {
        resource_minter::harvest_resources(&env, ship_id, &layout)
    }

    /// Read a player's harvested balance for one resource symbol.
    pub fn get_resource_balance(env: Env, owner: Address, resource: Symbol) -> u32 {
        resource_minter::resource_balance(&env, &owner, &resource)
    }

    /// Escrow a player's entire harvested balance of `resource` into a DEX offer.
    pub fn auto_list_on_dex(
        env: Env,
        player: Address,
        resource: Symbol,
        min_price: i128,
    ) -> Result<resource_minter::DexOffer, resource_minter::HarvestError> {
        resource_minter::auto_list_on_dex(&env, &player, &resource, min_price)
    }

    // ─── DEX Integration (Issue #9) ──────────────────────────────────────

    /// Harvest resources and immediately list on DEX.
    pub fn harvest_and_list(
        env: Env,
        player: Address,
        ship_id: u64,
        layout: NebulaLayout,
        resource: Symbol,
        min_price: i128,
    ) -> Result<
        (dex_integration::HarvestResult, dex_integration::DexOffer),
        dex_integration::HarvestError,
    > {
        dex_integration::harvest_and_list(&env, &player, ship_id, &layout, &resource, min_price)
    }

    /// Cancel an active DEX listing.
    pub fn cancel_listing(
        env: Env,
        owner: Address,
        offer_id: u64,
    ) -> Result<dex_integration::DexOffer, dex_integration::HarvestError> {
        dex_integration::cancel_listing(&env, &owner, offer_id)
    }

    /// List `amount` units of an already-held resource on the DEX.
    pub fn list_resource(
        env: Env,
        seller: Address,
        resource: Symbol,
        amount: u32,
        min_price: i128,
    ) -> Result<dex_integration::DexOffer, dex_integration::HarvestError> {
        dex_integration::list_resource(&env, &seller, &resource, amount, min_price)
    }

    /// Buy `amount` units from a DEX offer, paying at most `max_price` per unit.
    pub fn buy_offer(
        env: Env,
        buyer: Address,
        offer_id: u64,
        amount: u32,
        max_price: i128,
    ) -> Result<dex_integration::DexFill, dex_integration::HarvestError> {
        dex_integration::buy_offer(&env, &buyer, offer_id, amount, max_price)
    }

    /// Sell `amount` units into an open buy limit order, receiving at least
    /// `min_price` per unit.
    pub fn sell_to_order(
        env: Env,
        seller: Address,
        order_id: u64,
        resource: Symbol,
        amount: u32,
        min_price: i128,
    ) -> Result<dex_integration::DexFill, dex_integration::HarvestError> {
        dex_integration::sell_to_order(&env, &seller, order_id, &resource, amount, min_price)
    }

    /// Read a DEX offer by ID.
    pub fn get_dex_offer(env: Env, offer_id: u64) -> Option<dex_integration::DexOffer> {
        dex_integration::get_offer(&env, offer_id)
    }

    /// Page through active DEX offers, optionally filtered by resource.
    pub fn get_open_offers(
        env: Env,
        resource: Option<Symbol>,
        start_after: u64,
        limit: u32,
    ) -> dex_integration::OfferPage {
        dex_integration::get_open_offers(&env, resource.as_ref(), start_after, limit)
    }

    // ─── Treasure Vault ───────────────────────────────────────────────────

    /// Deposit resources into a time-locked treasure vault.
    pub fn deposit_treasure(
        env: Env,
        owner: Address,
        ship_id: u64,
        amount: u64,
    ) -> Result<TreasureVault, VaultError> {
        let result = treasure_vault::deposit_treasure(&env, &owner, ship_id, amount);
        if result.is_ok() {
            let mut b = [0u8; 128];
            b[0..8].copy_from_slice(&ship_id.to_be_bytes());
            let _ = audit_logger::log_audit_event(
                &env,
                Some(&owner),
                symbol_short!("dt"),
                BytesN::from_array(&env, &b),
            );
        }
        result
    }

    /// Claim a treasure vault after the lock period expires.
    pub fn claim_treasure(env: Env, owner: Address, vault_id: u64) -> Result<u64, VaultError> {
        let result = treasure_vault::claim_treasure(&env, &owner, vault_id);
        if result.is_ok() {
            let mut b = [0u8; 128];
            b[0..8].copy_from_slice(&vault_id.to_be_bytes());
            let _ = audit_logger::log_audit_event(
                &env,
                Some(&owner),
                symbol_short!("ct"),
                BytesN::from_array(&env, &b),
            );
        }
        result
    }

    /// Read a vault by ID.
    pub fn get_vault(env: Env, vault_id: u64) -> Option<TreasureVault> {
        treasure_vault::get_vault(&env, vault_id)
    }

    /// Discover an unlocked vault and roll its loot (Issue #655).
    pub fn discover_vault(
        env: Env,
        owner: Address,
        vault_id: u64,
    ) -> Result<VaultDiscovery, VaultError> {
        treasure_vault::discover_vault(&env, &owner, vault_id)
    }

    /// Spend a vault key to claim a discovered vault's loot (Issue #655).
    pub fn claim_vault_loot(
        env: Env,
        owner: Address,
        vault_id: u64,
    ) -> Result<LootReward, VaultError> {
        treasure_vault::claim_vault_loot(&env, &owner, vault_id)
    }

    /// Read a vault's discovery (rolled loot), if any.
    pub fn get_vault_discovery(env: Env, vault_id: u64) -> Option<VaultDiscovery> {
        treasure_vault::get_vault_discovery(&env, vault_id)
    }

    /// Number of unspent vault keys an owner holds.
    pub fn get_vault_key_balance(env: Env, owner: Address) -> u32 {
        treasure_vault::get_key_balance(&env, &owner)
    }

    // ─── Difficulty Scaling ───────────────────────────────────────────────

    /// Calculate difficulty scaling for a player level.
    pub fn calculate_difficulty(
        env: Env,
        player_level: u32,
    ) -> Result<DifficultyResult, DifficultyError> {
        difficulty_scaler::calculate_difficulty(&env, player_level)
    }

    /// Apply difficulty scaling to a layout's anomaly count.
    pub fn apply_scaling_to_layout(
        env: Env,
        base_anomaly_count: u32,
        player_level: u32,
    ) -> Result<u32, DifficultyError> {
        difficulty_scaler::apply_scaling_to_layout(&env, base_anomaly_count, player_level)
    }

    // ─── Randomness Oracle ────────────────────────────────────────────────

    /// Request a ledger-mixed random seed.
    pub fn request_random_seed(env: Env) -> BytesN<32> {
        randomness_oracle::request_random_seed(&env)
    }

    /// Validate a seed or fall back to previous block hash.
    pub fn verify_and_fallback(env: Env, seed: BytesN<32>) -> Result<BytesN<32>, OracleError> {
        randomness_oracle::verify_and_fallback(&env, &seed)
    }

    /// Get the current entropy pool.
    pub fn get_entropy_pool(env: Env) -> Vec<BytesN<32>> {
        randomness_oracle::get_entropy_pool(&env)
    }

    // ─── Player Profile ───────────────────────────────────────────────────────
    // ─── Player Profile ───────────────────────────────────────────────────

    /// Create a new on-chain player profile.
    pub fn initialize_profile(env: Env, owner: Address) -> Result<u64, ProfileError> {
        player_profile::initialize_profile(&env, owner)
    }

    /// Update scan count and essence earned after a harvest.
    pub fn update_progress(
        env: Env,
        caller: Address,
        profile_id: u64,
        scan_count: u32,
        essence: i128,
    ) -> Result<(), ProfileError> {
        player_profile::update_progress(&env, caller, profile_id, scan_count, essence)
    }

    /// Apply up to 5 stat updates in a single transaction.
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

    /// Load a single profile section so callers only pay for what they read.
    pub fn load_profile_section(
        env: Env,
        profile_id: u64,
        section: ProfileSection,
    ) -> Result<ProfileSectionData, ProfileError> {
        player_profile::load_profile_section(&env, profile_id, section)
    }

    // ─── Onboarding Tutorial (Issue #139) ────────────────────────────────

    /// Initialize the onboarding system with an admin.
    pub fn init_onboarding(env: Env, admin: Address) -> Result<(), OnboardingError> {
        onboarding_tutorial::init_onboarding(&env, &admin)
    }

    /// Create a player profile and start the tutorial.
    pub fn create_profile_onboarding(env: Env, player: Address) -> Result<(), OnboardingError> {
        onboarding_tutorial::create_profile(&env, &player)
    }

    /// Start the tutorial for an existing profile.
    pub fn start_tutorial(env: Env, player: Address) -> Result<(), OnboardingError> {
        onboarding_tutorial::start_tutorial(&env, &player)
    }

    /// Complete a tutorial step and earn rewards.
    pub fn complete_tutorial_step(
        env: Env,
        player: Address,
        step_id: u32,
    ) -> Result<i128, OnboardingError> {
        onboarding_tutorial::complete_tutorial_step(&env, &player, step_id)
    }

    /// Get tutorial progress for a player.
    pub fn get_tutorial_progress(env: Env, player: Address) -> Option<TutorialProgress> {
        onboarding_tutorial::get_tutorial_progress(&env, &player)
    }

    /// Get starter resources earned from tutorial.
    pub fn get_starter_resources(env: Env, player: Address) -> i128 {
        onboarding_tutorial::get_starter_resources(&env, &player)
    }

    /// Admin function to set a custom tutorial path.
    pub fn set_tutorial_path(
        env: Env,
        admin: Address,
        path: Vec<u32>,
    ) -> Result<(), OnboardingError> {
        onboarding_tutorial::set_tutorial_path(&env, &admin, path)
    }

    // ─── Session Manager ──────────────────────────────────────────────────

    /// Start a timed nebula exploration session for a ship.
    pub fn start_session(env: Env, owner: Address, ship_id: u64) -> Result<u64, SessionError> {
        session_manager::start_session(&env, owner, ship_id)
    }

    /// Close a session.
    pub fn expire_session(env: Env, caller: Address, session_id: u64) -> Result<(), SessionError> {
        session_manager::expire_session(&env, caller, session_id)
    }

    /// Retrieve session data by ID.
    pub fn get_session(env: Env, session_id: u64) -> Result<Session, SessionError> {
        session_manager::get_session(&env, session_id)
    }

    // ─── Blueprint Factory ────────────────────────────────────────────────

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

    // ─── Referral System ──────────────────────────────────────────────────

    /// Record an on-chain referral.
    pub fn register_referral(
        env: Env,
        referrer: Address,
        new_nomad: Address,
    ) -> Result<u64, ReferralError> {
        referral_system::register_referral(&env, referrer, new_nomad)
    }

    /// Mark first scan completed, unlocking referral reward.
    pub fn mark_first_scan(env: Env, nomad: Address) -> Result<(), ReferralError> {
        referral_system::mark_first_scan(&env, nomad)
    }

    /// Claim the essence referral reward.
    pub fn claim_referral_reward(
        env: Env,
        referrer: Address,
        new_nomad: Address,
    ) -> Result<i128, ReferralError> {
        referral_system::claim_referral_reward(&env, referrer, new_nomad)
    }

    /// Retrieve a referral record.
    pub fn get_referral(env: Env, new_nomad: Address) -> Result<Referral, ReferralError> {
        referral_system::get_referral(&env, new_nomad)
    }

    // ─── Ship Upgrade (Issue #7) ─────────────────────────────────────────

    /// Initialise the upgrade config with a blueprint map. Admin-only, once.
    pub fn init_upgrade_config(
        env: Env,
        admin: Address,
        blueprints: soroban_sdk::Map<Symbol, UpgradeBlueprint>,
    ) -> Result<(), ShipUpgradeError> {
        ship_upgrade::init_upgrade_config(&env, &admin, blueprints)
    }

    /// Apply a single component upgrade to a ship, burning the required
    /// resource from the player's harvested balance.
    pub fn apply_upgrade(
        env: Env,
        player: Address,
        ship_id: u64,
        component: Symbol,
    ) -> Result<ShipState, ShipUpgradeError> {
        ship_upgrade::apply_upgrade(&env, &player, ship_id, component)
    }

    /// Apply up to 2 upgrades in a single transaction.
    pub fn batch_upgrade(
        env: Env,
        player: Address,
        ship_id: u64,
        components: Vec<Symbol>,
    ) -> Result<Vec<ShipState>, ShipUpgradeError> {
        ship_upgrade::batch_upgrade(&env, &player, ship_id, components)
    }

    /// Read the current upgrade state of a ship.
    pub fn get_ship_state(env: Env, ship_id: u64) -> Option<ShipState> {
        ship_upgrade::get_ship_state(&env, ship_id)
    }

    /// Apply a regeneration upgrade effect (called after a regen upgrade).
    pub fn apply_regen_upgrade(env: Env, ship_id: u64, bonus: u32) -> Result<(), ShipUpgradeError> {
        ship_upgrade::apply_regen_upgrade(&env, ship_id, bonus)
    }

    // ─── Upgrade cost curve (Issue #454) ─────────────────────────────────

    /// Quote what installing `component` on `ship_id` costs right now, under
    /// the active exponential cost curve. Pure view — does not charge the
    /// player.
    pub fn quote_upgrade_cost(
        env: Env,
        ship_id: u64,
        component: Symbol,
    ) -> Result<u32, ShipUpgradeError> {
        ship_upgrade::quote_upgrade_cost(&env, ship_id, component)
    }

    /// Read the active upgrade cost curve. Returns the rebalanced default when
    /// the admin has never overridden it.
    pub fn get_upgrade_economy(env: Env) -> UpgradeEconomy {
        ship_upgrade::get_upgrade_economy(&env)
    }

    /// Override the upgrade cost curve. Upgrade-admin only. `growth_bps = 0`
    /// restores the legacy flat schedule.
    pub fn set_upgrade_economy(
        env: Env,
        admin: Address,
        economy: UpgradeEconomy,
    ) -> Result<(), ShipUpgradeError> {
        ship_upgrade::set_upgrade_economy(&env, &admin, economy)
    }

    /// Total resource units burned by upgrades across all ships.
    pub fn get_total_upgrade_spend(env: Env) -> u32 {
        ship_upgrade::get_total_upgrade_spend(&env)
    }

    // ─── Paid ship repair — resource sink (Issue #453) ───────────────────

    /// Price a repair without mutating state. `emergency = true` prices the
    /// field-patch special action, which repairs more per call at a surcharge.
    pub fn quote_repair(
        env: Env,
        owner: Address,
        ship_id: u64,
        asset_id: Symbol,
        emergency: bool,
    ) -> Result<RepairQuote, ShipRepairError> {
        ship_repair::quote_repair(&env, &owner, ship_id, asset_id, emergency)
    }

    /// Repair a ship, permanently burning resources in proportion to the
    /// durability restored. `emergency = true` performs the surcharged
    /// field-patch special action.
    pub fn repair_ship(
        env: Env,
        player: Address,
        ship_id: u64,
        asset_id: Symbol,
        emergency: bool,
    ) -> Result<RepairReceipt, ShipRepairError> {
        ship_repair::repair_ship(&env, &player, ship_id, asset_id, emergency)
    }

    /// Cumulative resource units destroyed by repairs across all ships.
    pub fn get_total_repair_burn(env: Env) -> u32 {
        ship_repair::get_total_repair_burn(&env)
    }

    /// Read the active repair pricing. Falls back to the balanced default.
    pub fn get_repair_config(env: Env) -> RepairConfig {
        ship_repair::get_repair_config(&env)
    }

    /// Seed the repair sink admin so pricing can be retuned later. Once only,
    /// and optional — the sink runs on defaults without it.
    pub fn init_repair_config(
        env: Env,
        admin: Address,
        config: RepairConfig,
    ) -> Result<(), ShipRepairError> {
        ship_repair::init_repair_config(&env, &admin, config)
    }

    /// Retune repair pricing. Admin-only.
    pub fn set_repair_config(
        env: Env,
        admin: Address,
        config: RepairConfig,
    ) -> Result<(), ShipRepairError> {
        ship_repair::set_repair_config(&env, &admin, config)
    }

    // ─── Dynamic Resource Pricing (#452) ──────────────────────────────────

    /// Seed the dynamic pricing admin and guards. Admin-only, once.
    pub fn init_pricing_config(
        env: Env,
        admin: Address,
        config: PricingConfig,
    ) -> Result<(), PricingError> {
        dynamic_pricing::init_pricing_config(&env, &admin, config)
    }

    /// Retune the dynamic pricing guards. Admin-only.
    pub fn set_pricing_config(
        env: Env,
        admin: Address,
        config: PricingConfig,
    ) -> Result<(), PricingError> {
        dynamic_pricing::set_pricing_config(&env, &admin, config)
    }

    /// The active dynamic pricing configuration.
    pub fn get_pricing_config(env: Env) -> PricingConfig {
        dynamic_pricing::get_pricing_config(&env)
    }

    /// Feed one raw price print into the EMA engine. Admin-only.
    ///
    /// Returns [`PricingError::DeviationTooLarge`] when the print sits outside
    /// `max_deviation_bps` of the current average, in which case the average is
    /// left untouched.
    pub fn observe_price(
        env: Env,
        source: Address,
        resource: Symbol,
        raw_price: i128,
    ) -> Result<PriceState, PricingError> {
        dynamic_pricing::observe_price(&env, &source, resource, raw_price)
    }

    /// Feed the market oracle's fresh consensus price into the pricing
    /// engine (permissionless keeper call, Issue #654).
    pub fn observe_oracle_price(env: Env, resource: Symbol) -> Result<PriceState, PricingError> {
        dynamic_pricing::observe_oracle_price(&env, resource)
    }

    /// Record the supply/demand balance for a resource. Admin-only.
    pub fn observe_supply_demand(
        env: Env,
        source: Address,
        resource: Symbol,
        supply: i128,
        demand: i128,
    ) -> Result<i128, PricingError> {
        dynamic_pricing::observe_supply_demand(&env, &source, resource, supply, demand)
    }

    /// The EMA-smoothed, volatility-clamped price the economy should use.
    pub fn get_dynamic_price(env: Env, resource: Symbol) -> Result<DynamicPrice, PricingError> {
        dynamic_pricing::dynamic_price(&env, resource)
    }

    /// Rolling state for a resource, including realized volatility.
    pub fn get_price_state(env: Env, resource: Symbol) -> Result<PriceState, PricingError> {
        dynamic_pricing::get_price_state(&env, resource)
    }

    /// Rolling price history for a resource, oldest first.
    pub fn get_pricing_history(env: Env, resource: Symbol) -> Vec<PricePoint> {
        dynamic_pricing::get_pricing_history(&env, resource)
    }

    /// Harvest a resource and list it at the current dynamic market price,
    /// instead of a caller-supplied price (#452).
    pub fn list_at_market(
        env: Env,
        player: Address,
        ship_id: u64,
        layout: NebulaLayout,
        resource: Symbol,
    ) -> Result<
        (
            crate::resource_minter::HarvestResult,
            crate::resource_minter::DexOffer,
            i128,
        ),
        DynamicListError,
    > {
        dex_integration::list_at_market(&env, &player, ship_id, &layout, &resource)
    }

    // ─── Cross-Player Resource Gifting (#27) ──────────────────────────────

    /// Send a resource gift to another player.
    pub fn send_gift(
        env: Env,
        sender: Address,
        receiver: Address,
        resource: AssetId,
        amount: i128,
    ) -> Result<u64, GiftError> {
        gifting_system::send_gift(&env, sender, receiver, resource, amount)
    }

    /// Accept a pending gift and claim the resources.
    pub fn accept_gift(env: Env, receiver: Address, gift_id: u64) -> Result<(), GiftError> {
        gifting_system::accept_gift(&env, receiver, gift_id)
    }

    /// Read a gift by ID.
    pub fn get_gift(env: Env, gift_id: u64) -> Option<Gift> {
        gifting_system::get_gift(&env, gift_id)
    }

