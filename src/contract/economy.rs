    // ─── Yield Farming (Issue #36) ───────────────────────────────────────────

    /// Stake resources for boosted yields.
    pub fn deposit_to_pool(
        env: Env,
        owner: Address,
        amount: i128,
        lock_period: u32,
    ) -> Result<u64, yield_farming::FarmError> {
        yield_farming::deposit_to_pool(env, owner, amount, lock_period)
    }

    /// Claim accumulated cosmic rewards.
    pub fn harvest_farm_rewards(
        env: Env,
        owner: Address,
        pool_id: u64,
    ) -> Result<i128, yield_farming::FarmError> {
        yield_farming::harvest_farm_rewards(env, owner, pool_id)
    }

    // ─── Community Governance (Issue #38) ────────────────────────────────────

    /// Submit a proposed config change.
    pub fn create_proposal(
        env: Env,
        creator: Address,
        description: String,
        param_change: BytesN<128>,
    ) -> Result<u64, governance::GovError> {
        governance::create_proposal(env, creator, description, param_change)
    }

    /// Record a vote weighted by essence held.
    pub fn cast_vote(
        env: Env,
        voter: Address,
        proposal_id: u64,
        support: bool,
        weight: i128,
    ) -> Result<(), governance::GovError> {
        governance::cast_vote(env, voter, proposal_id, support, weight)
    }

    // ─── Theme Customizer (Issue #37) ────────────────────────────────────────

    /// Set ship color palette and particle style.
    pub fn apply_theme(
        env: Env,
        owner: Address,
        ship_id: u64,
        theme_id: Symbol,
    ) -> Result<(), theme_customizer::ThemeError> {
        theme_customizer::apply_theme(env, owner, ship_id, theme_id)
    }

    /// Returns theme preview metadata.
    pub fn generate_theme_preview(
        env: Env,
        theme_id: Symbol,
    ) -> Result<theme_customizer::ThemePreview, theme_customizer::ThemeError> {
        theme_customizer::generate_theme_preview(env, theme_id)
    }

    // ─── Indexer Callbacks (Issue #35) ───────────────────────────────────────

    /// Subscribes an external service to events.
    pub fn register_indexer_callback(
        env: Env,
        caller: Address,
        callback_id: Symbol,
    ) -> Result<(), indexer_callbacks::IndexerError> {
        indexer_callbacks::register_indexer_callback(env, caller, callback_id)
    }

    /// Broadcasts rich data for external dashboards.
    pub fn trigger_indexer_event(
        env: Env,
        event_type: Symbol,
        payload: BytesN<256>,
    ) -> Result<(), indexer_callbacks::IndexerError> {
        indexer_callbacks::trigger_indexer_event(env, event_type, payload)
    }

    // ─── Energy Management ────────────────────────────────────────────────

    /// Consume energy for ship operations.
    pub fn consume_energy(
        env: Env,
        ship_id: u64,
        amount: u32,
    ) -> Result<u32, energy_manager::EnergyError> {
        energy_manager::consume_energy(&env, ship_id, amount)
    }

    /// Recharge ship energy using resources.
    pub fn recharge_energy(
        env: Env,
        ship_id: u64,
        resource_amount: i128,
    ) -> Result<energy_manager::RechargeResult, energy_manager::EnergyError> {
        energy_manager::recharge_energy(&env, ship_id, resource_amount)
    }

    /// Get ship energy balance.
    pub fn get_energy_balance(
        env: Env,
        ship_id: u64,
    ) -> Result<energy_manager::EnergyBalance, energy_manager::EnergyError> {
        energy_manager::get_energy_balance(&env, ship_id)
    }

    // ─── Passive Energy Regeneration (Issue #190) ─────────────────────────

    /// Apply passive energy recovery for a ship.
    pub fn apply_passive_regen(
        env: Env,
        ship_id: u64,
    ) -> Result<energy_manager::PassiveRegenResult, energy_manager::EnergyError> {
        energy_manager::apply_passive_regen(&env, ship_id)
    }

    /// Set the passive regeneration rate (admin / upgrade system).
    pub fn set_regen_rate(env: Env, caller: Address, rate: u32) {
        energy_manager::set_regen_rate(&env, &caller, rate)
    }

    /// Get the current passive regeneration rate.
    pub fn get_regen_rate(env: Env) -> u32 {
        energy_manager::get_regen_rate(&env)
    }

    // ─── Environmental Simulation ─────────────────────────────────────────

    /// Simulate environmental conditions for a nebula.
    pub fn simulate_conditions(
        env: Env,
        nebula_id: u64,
    ) -> Result<environment_simulator::EnvironmentCondition, environment_simulator::EnvironmentError>
    {
        environment_simulator::simulate_conditions(&env, nebula_id)
    }

    /// Apply environmental modifiers to harvest yields.
    pub fn apply_environmental_modifier(
        env: Env,
        ship_id: u64,
        nebula_id: u64,
        base_yield: i32,
    ) -> Result<environment_simulator::ModifierResult, environment_simulator::EnvironmentError>
    {
        environment_simulator::apply_environmental_modifier(&env, ship_id, nebula_id, base_yield)
    }

    /// Get current nebula environmental condition.
    pub fn get_nebula_condition(
        env: Env,
        nebula_id: u64,
    ) -> Option<environment_simulator::EnvironmentCondition> {
        environment_simulator::get_nebula_condition(&env, nebula_id)
    }

    // ─── Mission System ───────────────────────────────────────────────────

    /// Generate a new daily mission for player.
    pub fn generate_daily_mission(
        env: Env,
        player: Address,
    ) -> Result<mission_generator::Mission, mission_generator::MissionError> {
        mission_generator::generate_daily_mission(&env, player)
    }

    /// Complete a mission and claim rewards.
    pub fn complete_mission(
        env: Env,
        player: Address,
        mission_id: u64,
    ) -> Result<mission_generator::MissionReward, mission_generator::MissionError> {
        mission_generator::complete_mission(&env, player, mission_id)
    }

    /// Update mission progress.
    pub fn update_mission_progress(
        env: Env,
        mission_id: u64,
        progress: u32,
    ) -> Result<mission_generator::Mission, mission_generator::MissionError> {
        mission_generator::update_mission_progress(&env, mission_id, progress)
    }

    /// Get all missions for a player.
    pub fn get_player_missions(env: Env, player: Address) -> Vec<mission_generator::Mission> {
        mission_generator::get_player_missions(&env, player)
    }

    // ─── Escrow Trading ───────────────────────────────────────────────────

    /// Initiate a peer-to-peer escrow trade.
    pub fn initiate_escrow(
        env: Env,
        trader_a: Address,
        trader_b: Address,
        assets_a: Vec<escrow_trader::TradeAsset>,
        assets_b: Vec<escrow_trader::TradeAsset>,
    ) -> Result<escrow_trader::Escrow, escrow_trader::EscrowError> {
        escrow_trader::initiate_escrow(&env, trader_a, trader_b, assets_a, assets_b)
    }

    /// Confirm participation in an escrow trade.
    pub fn confirm_escrow(
        env: Env,
        escrow_id: u64,
        trader: Address,
    ) -> Result<escrow_trader::Escrow, escrow_trader::EscrowError> {
        escrow_trader::confirm_escrow(&env, escrow_id, trader)
    }

    /// Complete an escrow trade atomically.
    pub fn complete_escrow(
        env: Env,
        escrow_id: u64,
    ) -> Result<escrow_trader::EscrowResult, escrow_trader::EscrowError> {
        escrow_trader::complete_escrow(&env, escrow_id)
    }

    /// Cancel an escrow trade.
    pub fn cancel_escrow(
        env: Env,
        escrow_id: u64,
        trader: Address,
    ) -> Result<(), escrow_trader::EscrowError> {
        escrow_trader::cancel_escrow(&env, escrow_id, trader)
    }

    /// Get escrow details by ID.
    pub fn get_escrow(env: Env, escrow_id: u64) -> Option<escrow_trader::Escrow> {
        escrow_trader::get_escrow(&env, escrow_id)
    }

    // ─── Emergency Controls (Issue #29) ──────────────────────────────────

    /// Initialize the multi-sig admin set at deployment. One-time call.
    pub fn initialize_admins(env: Env, admins: Vec<Address>) -> Result<(), EmergencyError> {
        emergency_controls::initialize_admins(&env, admins)
    }

    /// Instantly freeze all mutating contract functions. Admin-only.
    pub fn pause_contract(env: Env, admin: Address) -> Result<(), EmergencyError> {
        emergency_controls::pause_contract(&env, &admin)
    }

    /// Schedule a time-delayed unpause. Admin-only.
    pub fn schedule_unpause(env: Env, admin: Address) -> Result<u64, EmergencyError> {
        emergency_controls::schedule_unpause(&env, &admin)
    }

    /// Execute the unpause after the delay has elapsed. Admin-only.
    pub fn execute_unpause(env: Env, admin: Address) -> Result<(), EmergencyError> {
        emergency_controls::execute_unpause(&env, &admin)
    }

    /// Admin-only emergency recovery of stuck resources.
    pub fn emergency_withdraw(
        env: Env,
        admin: Address,
        resource: Symbol,
    ) -> Result<(), EmergencyError> {
        emergency_controls::emergency_withdraw(&env, &admin, resource)
    }

    /// Returns true if the contract is currently paused.
    pub fn is_paused(env: Env) -> bool {
        emergency_controls::is_paused(&env)
    }

    /// Returns the current admin list.
    pub fn get_admins(env: Env) -> Vec<Address> {
        emergency_controls::get_admins(&env)
    }

    // ─── Metadata URI Resolver (Issue #30) ───────────────────────────────

    /// Set the IPFS CID for a token. Immutable after first set.
    pub fn set_metadata_uri(
        env: Env,
        caller: Address,
        token_id: u64,
        cid: Bytes,
    ) -> Result<(), MetadataError> {
        metadata_resolver::set_metadata_uri(&env, &caller, token_id, cid)
    }

    /// Resolve full metadata for a token using the configured gateway.
    pub fn resolve_metadata(env: Env, token_id: u64) -> Result<TokenMetadata, MetadataError> {
        metadata_resolver::resolve_metadata(&env, token_id)
    }

    /// Batch resolve metadata for up to 10 tokens.
    pub fn batch_resolve_metadata(
        env: Env,
        token_ids: Vec<u64>,
    ) -> Result<Vec<TokenMetadata>, MetadataError> {
        metadata_resolver::batch_resolve_metadata(&env, token_ids)
    }

    /// Update the IPFS gateway prefix. Admin-only.
    pub fn set_gateway(env: Env, admin: Address, gateway: Bytes) {
        metadata_resolver::set_gateway(&env, &admin, gateway)
    }

    /// Return the currently configured IPFS gateway prefix.
    pub fn get_current_gateway(env: Env) -> Bytes {
        metadata_resolver::get_current_gateway(&env)
    }

    // ─── Batch Ship Operations (Issue #31) ───────────────────────────────

    /// Stage up to 8 ship operations into the player's batch queue.
    pub fn queue_batch_operation(
        env: Env,
        player: Address,
        operations: Vec<BatchOp>,
    ) -> Result<u32, BatchError> {
        batch_processor::queue_batch_operation(&env, &player, operations)
    }

    /// Execute all queued operations atomically for the provided ship IDs.
    pub fn execute_batch(
        env: Env,
        player: Address,
        ship_ids: Vec<u64>,
    ) -> Result<BatchResult, BatchError> {
        batch_processor::execute_batch(&env, &player, ship_ids)
    }

    /// Return the player's currently queued batch.
    pub fn get_player_batch(env: Env, player: Address) -> Option<Vec<BatchOp>> {
        batch_processor::get_player_batch(&env, &player)
    }

    /// Clear the player's pending batch queue.
    pub fn clear_batch(env: Env, player: Address) {
        batch_processor::clear_batch(&env, &player)
    }

    /// Execute batch minting operations atomically (Issue #678).
    pub fn execute_batch_mint(
        env: Env,
        caller: Address,
        mint_ops: Vec<(u64, u32, Symbol, u64)>,
    ) -> Result<BatchMintResult, BatchError> {
        batch_processor::execute_batch_mint(&env, &caller, mint_ops)
    }

    /// Execute batch trade operations atomically (Issue #678).
    pub fn execute_batch_trade(
        env: Env,
        caller: Address,
        trades: Vec<(Symbol, Symbol, u64)>,
    ) -> Result<BatchTradeResult, BatchError> {
        batch_processor::execute_batch_trade(&env, &caller, trades)
    }

    /// Execute batch transfer operations atomically (Issue #678).
    pub fn execute_batch_transfer(
        env: Env,
        caller: Address,
        transfers: Vec<BatchTransferItem>,
    ) -> Result<BatchTransferResult, BatchError> {
        batch_processor::execute_batch_transfer(&env, &caller, transfers)
    }

    /// Execute batch update operations atomically (Issue #678).
    pub fn execute_batch_update(
        env: Env,
        caller: Address,
        updates: Vec<BatchUpdateItem>,
    ) -> Result<BatchUpdateResult, BatchError> {
        batch_processor::execute_batch_update(&env, &caller, updates)
    }

    // ─── On-chain Audit Logging (Issue #64) ───────────────────────────────

    pub fn log_audit_event(
        env: Env,
        actor: Option<Address>,
        action: Symbol,
        details: BytesN<128>,
    ) -> Result<AuditEntry, AuditLoggerError> {
        audit_logger::log_audit_event(&env, actor.as_ref(), action, details)
    }

    pub fn query_audit_logs(
        env: Env,
        filter: Symbol,
        limit: u32,
    ) -> Result<Vec<AuditEntry>, AuditLoggerError> {
        audit_logger::query_audit_logs(&env, filter, limit)
    }

    pub fn get_audit_count(env: Env) -> u64 {
        audit_logger::get_audit_count(&env)
    }

    /// Number of audit entries currently stored (after pruning).
    pub fn get_retained_audit_count(env: Env) -> u64 {
        audit_logger::get_retained_audit_count(&env)
    }

    /// Active audit-log retention policy.
    pub fn get_audit_retention(env: Env) -> RetentionPolicy {
        audit_logger::get_audit_retention(&env)
    }

    /// Replace the audit-log retention policy. Admin role required.
    pub fn set_audit_retention(
        env: Env,
        admin: Address,
        policy: RetentionPolicy,
    ) -> Result<(), AuditLoggerError> {
        audit_logger::set_audit_retention(&env, &admin, &policy)
    }

    /// Delete expired cache entries in `namespaces` and audit entries outside
    /// the retention policy. Permissionless and bounded per call.
    pub fn prune_expired_data(
        env: Env,
        namespaces: Vec<Symbol>,
    ) -> Result<PruneReport, StorageError> {
        storage_optim::prune_expired_data(&env, &namespaces)
    }

    // ─── Sustainability and Carbon Tracking (Issue #68) ──────────────────

    pub fn record_transaction_footprint(
        env: Env,
        player: Address,
        gas_used: u64,
    ) -> FootprintRecord {
        let record =
            sustainability_metrics::record_transaction_footprint(&env, &player, gas_used).unwrap();
        let mut details_bytes = [0u8; 128];
        details_bytes[0..8].copy_from_slice(&record.gas_used.to_be_bytes());
        let details = BytesN::from_array(&env, &details_bytes);
        let _ = audit_logger::log_audit_event(&env, Some(&player), symbol_short!("ec"), details);
        record
    }

    pub fn claim_sustainability_reward(env: Env, player: Address) -> i128 {
        let reward = sustainability_metrics::claim_sustainability_reward(&env, &player).unwrap();
        let mut details_bytes = [0u8; 128];
        details_bytes[0..8].copy_from_slice(&(reward as i64).to_be_bytes());
        let details = BytesN::from_array(&env, &details_bytes);
        let _ = audit_logger::log_audit_event(&env, Some(&player), symbol_short!("er"), details);
        reward
    }

    pub fn get_footprint(env: Env, player: Address) -> FootprintRecord {
        sustainability_metrics::get_footprint(&env, &player)
    }

    // ─── Cosmic Anomaly Classification Engine (Issue #70) ────────────────

    pub fn classify_anomaly(env: Env, anomaly_id: u64, features: Vec<u32>) -> ClassificationRecord {
        anomaly_classifier::classify_anomaly(&env, anomaly_id, features).unwrap()
    }

    pub fn refine_classification(
        env: Env,
        anomaly_id: u64,
        new_data: Vec<u32>,
    ) -> ClassificationRecord {
        anomaly_classifier::refine_classification(&env, anomaly_id, new_data).unwrap()
    }

    pub fn classify_batch(env: Env, items: Vec<(u64, Vec<u32>)>) -> Vec<ClassificationRecord> {
        anomaly_classifier::classify_batch(&env, items)
    }

    pub fn get_classification(env: Env, anomaly_id: u64) -> Option<ClassificationRecord> {
        anomaly_classifier::get_classification(&env, anomaly_id)
    }

    // ─── Shared Reusability Library (Issue #67) ──────────────────────────

    pub fn validate_address(env: Env, auth: Address) -> Result<(), SharedError> {
        shared_lib::validate_address(&env, auth)
    }

    pub fn calculate_yield(_env: Env, base: i128, multiplier: u32) -> Result<i128, SharedError> {
        shared_lib::calculate_yield(base, multiplier)
    }

    // ─── Storage Optimization & Re-Entrancy Guards (Issue #10) ────────────

    /// Initialize the bump storage configuration. Admin-only.
    pub fn initialize_bump_config(env: Env, admin: Address) {
        storage_optim::initialize_bump_config(&env, &admin)
    }

    /// Store data with optimized persistent bump TTL.
    pub fn store_with_bump(
        env: Env,
        key: Symbol,
        value: BytesN<64>,
    ) -> Result<OptimResult, StorageError> {
        storage_optim::store_with_bump(&env, key, value)
    }

    /// Retrieve an optimized storage entry.
    pub fn get_optimized_entry(env: Env, key: Symbol) -> Result<OptimizedEntry, StorageError> {
        storage_optim::get_optimized_entry(&env, key)
    }

    /// Batch-store multiple entries with a single re-entrancy guard.
    pub fn batch_store_with_bump(
        env: Env,
        keys: Vec<Symbol>,
        values: Vec<BytesN<64>>,
    ) -> Result<Vec<OptimResult>, StorageError> {
        storage_optim::batch_store_with_bump(&env, keys, values)
    }

    /// Store composite ship-nebula data in a single slot.
    pub fn store_ship_nebula(
        env: Env,
        ship_id: u64,
        nebula_id: u64,
        scan_count: u32,
        resource_cache: u64,
    ) -> Result<(), StorageError> {
        storage_optim::store_ship_nebula(&env, ship_id, nebula_id, scan_count, resource_cache)
    }

    /// Retrieve composite ship-nebula data.
    pub fn get_ship_nebula(
        env: Env,
        ship_id: u64,
        nebula_id: u64,
    ) -> Result<ShipNebulaData, StorageError> {
        storage_optim::get_ship_nebula(&env, ship_id, nebula_id)
    }

    /// Update bump TTL configuration. Admin-only.
    pub fn update_bump_config(
        env: Env,
        admin: Address,
        default_ttl: u32,
        max_ttl: u32,
    ) -> Result<(), StorageError> {
        storage_optim::update_bump_config(&env, &admin, default_ttl, max_ttl)
    }

    /// Set the proxy upgrade target address. Admin-only.
    pub fn set_upgrade_target(
        env: Env,
        admin: Address,
        target: Address,
    ) -> Result<(), StorageError> {
        storage_optim::set_upgrade_target(&env, &admin, target)
    }

    /// Get the current upgrade target if set.
    pub fn get_upgrade_target(env: Env) -> Option<Address> {
        storage_optim::get_upgrade_target(&env)
    }

    /// Reset the burst-read counter for a new invocation.
    pub fn reset_burst_counter(env: Env) {
        storage_optim::reset_burst_counter(&env)
    }

    // ─── On-Chain Game State Snapshots (Issue #58) ───────────────────────

    /// Take a snapshot of the current ship and resource state.
    pub fn take_snapshot(
        env: Env,
        caller: Address,
        ship_id: u64,
    ) -> Result<StateSnapshot, SnapshotError> {
        state_snapshot::take_snapshot(&env, &caller, ship_id)
    }

    /// Restore ship state from a previously taken snapshot.
    pub fn restore_from_snapshot(
        env: Env,
        caller: Address,
        snapshot_id: u64,
    ) -> Result<RestoreResult, SnapshotError> {
        state_snapshot::restore_from_snapshot(&env, &caller, snapshot_id)
    }

    /// Get a snapshot by ID.
    pub fn get_snapshot(env: Env, snapshot_id: u64) -> Result<StateSnapshot, SnapshotError> {
        state_snapshot::get_snapshot(&env, snapshot_id)
    }

    /// Get all snapshot IDs for a ship.
    pub fn get_ship_snapshots(env: Env, ship_id: u64) -> Vec<u64> {
        state_snapshot::get_ship_snapshots(&env, ship_id)
    }

    /// Trigger an automatic daily snapshot if the interval has elapsed.
    pub fn auto_snapshot(
        env: Env,
        caller: Address,
        ship_id: u64,
    ) -> Result<StateSnapshot, SnapshotError> {
        state_snapshot::auto_snapshot(&env, &caller, ship_id)
    }

    /// Reset snapshot session counter for a ship.
    pub fn reset_session_count(env: Env, ship_id: u64) {
        state_snapshot::reset_session_count(&env, ship_id)
    }

    // ─── Prize Distributor (Issue #62) ───────────────────────────────────

    /// Initialize the weekly prize distributor. One-time setup.
    pub fn initialize_prize_distributor(env: Env, admin: Address) {
        prize_distributor::initialize_prize_distributor(&env, &admin)
    }

    /// Add funds to the prize pool (sponsor-funded pools supported).
    pub fn fund_prize_pool(env: Env, funder: Address, amount: i128) -> Result<i128, PrizeError> {
        prize_distributor::fund_prize_pool(&env, &funder, amount)
    }

    /// Admin: record the current leaderboard snapshot for payout.
    pub fn submit_leaderboard_snapshot(
        env: Env,
        admin: Address,
        winners: Vec<Address>,
    ) -> Result<u32, PrizeError> {
        prize_distributor::submit_leaderboard_snapshot(&env, &admin, &winners)
    }

    /// Distribute weekly prizes to the top N positions (max 50 per tx).
    pub fn distribute_weekly_prizes(
        env: Env,
        caller: Address,
        top_n: u32,
    ) -> Result<Vec<PrizeRecord>, PrizeError> {
        prize_distributor::distribute_weekly_prizes(&env, &caller, top_n)
    }

    /// Return current prize pool balance.
    pub fn get_prize_pool(env: Env) -> i128 {
        prize_distributor::get_prize_pool(&env)
    }

    /// Return total prizes distributed all-time.
    pub fn get_total_distributed(env: Env) -> i128 {
        prize_distributor::get_total_distributed(&env)
    }

    // ─── Portal Registry (Issue #71) ─────────────────────────────────────

    /// Initialize the inter-nebula portal registry.
    pub fn initialize_portal_registry(env: Env, admin: Address) {
        portal_registry::initialize_portal_registry(&env, &admin)
    }

    /// Register a new portal between two nebulae.
    pub fn register_portal(
        env: Env,
        owner: Address,
        source_nebula: u64,
        target_nebula: u64,
    ) -> Result<u64, PortalError> {
        portal_registry::register_portal(&env, &owner, source_nebula, target_nebula)
    }

    /// Query stability percentage and travel cost for a portal.
    pub fn query_portal_status(env: Env, portal_id: u64) -> Result<(u32, i128), PortalError> {
        portal_registry::query_portal_status(&env, portal_id)
    }

    /// Refresh a portal's stability back to 100%.
    pub fn refresh_portal(env: Env, owner: Address, portal_id: u64) -> Result<(), PortalError> {
        portal_registry::refresh_portal(&env, &owner, portal_id)
    }

    /// Attempt travel through a portal.
    pub fn travel_through_portal(env: Env, portal_id: u64) -> Result<i128, PortalError> {
        portal_registry::travel_through_portal(&env, portal_id)
    }

    // ─── Constellation Mapper (Issue #72) ────────────────────────────────

    /// Record a new star constellation pattern on-chain.
    pub fn record_constellation(
        env: Env,
        recorder: Address,
        stars: Vec<BytesN<32>>,
    ) -> Result<u64, ConstellationError> {
        constellation_mapper::record_constellation(&env, &recorder, &stars)
    }

    /// Find the best matching known constellation for an observed pattern.
    pub fn match_constellation(
        env: Env,
        observed: Vec<BytesN<32>>,
    ) -> Result<ConstellationMatchResult, ConstellationError> {
        constellation_mapper::match_constellation(&env, &observed)
    }

    /// Return total recorded constellations.
    pub fn get_constellation_count(env: Env) -> u64 {
        constellation_mapper::get_constellation_count(&env)
    }

    /// Name a constellation, anchor it to a region and attach lore (recorder, once).
    pub fn annotate_constellation(
        env: Env,
        recorder: Address,
        constellation_id: u64,
        name: String,
        anchor_region: u64,
        lore: String,
    ) -> Result<constellation_mapper::ConstellationMeta, ConstellationError> {
        constellation_mapper::annotate_constellation(
            &env,
            &recorder,
            constellation_id,
            name,
            anchor_region,
            lore,
        )
    }

    /// Recognize a partial or noisy star observation.
    pub fn recognize_constellation(
        env: Env,
        observed: Vec<BytesN<32>>,
    ) -> Result<constellation_mapper::Recognition, ConstellationError> {
        constellation_mapper::recognize_pattern(&env, &observed)
    }

    /// Recognize an observation and point toward its anchor region.
    pub fn get_navigation_aid(
        env: Env,
        observed: Vec<BytesN<32>>,
        current_region: u64,
    ) -> Result<constellation_mapper::NavigationAid, ConstellationError> {
        constellation_mapper::navigation_aid(&env, &observed, current_region)
    }

    /// Sight a constellation for discovery bonuses and collection progress.
    pub fn sight_constellation(
        env: Env,
        observer: Address,
        observed: Vec<BytesN<32>>,
        region_id: u64,
    ) -> Result<constellation_mapper::SightingResult, ConstellationError> {
        constellation_mapper::sight_constellation(&env, &observer, &observed, region_id)
    }

    /// A player's constellation collection progress.
    pub fn get_constellation_collection(
        env: Env,
        player: Address,
    ) -> constellation_mapper::CollectionProgress {
        constellation_mapper::get_collection(&env, &player)
    }

    // ─── Quantum Entanglement Comms (Issue #73) ───────────────────────────

    /// Establish an entanglement pair between two ships.
    pub fn create_entanglement_pair(
        env: Env,
        owner_a: Address,
        ship_a: u64,
        owner_b: Address,
        ship_b: u64,
    ) -> Result<u64, EntanglementError> {
        entanglement_comms::create_entanglement_pair(&env, &owner_a, ship_a, &owner_b, ship_b)
    }

    /// Send a single encrypted message over an active pair.
    pub fn send_entangled_message(
        env: Env,
        caller: Address,
        pair_id: u64,
        message: BytesN<64>,
    ) -> Result<u64, EntanglementError> {
        entanglement_comms::send_entangled_message(&env, &caller, pair_id, &message)
    }

    /// Send up to 20 messages in one transaction.
    pub fn send_entangled_message_batch(
        env: Env,
        caller: Address,
        pair_id: u64,
        messages: Vec<BytesN<64>>,
    ) -> Result<u64, EntanglementError> {
        entanglement_comms::send_entangled_message_batch(&env, &caller, pair_id, &messages)
    }

    /// Dissolve an entanglement pair.
    pub fn dissolve_pair(env: Env, caller: Address, pair_id: u64) -> Result<(), EntanglementError> {
        entanglement_comms::dissolve_pair(&env, &caller, pair_id)
    }

    /// Return total messages sent over a pair.
    pub fn get_message_count(env: Env, pair_id: u64) -> u64 {
        entanglement_comms::get_message_count(&env, pair_id)
    }

    // ─── Fractional Resource Ownership API (Issue #89) ────────────────────

    /// Initialize the fractional resource system.
    pub fn initialize_fractional(env: Env, admin: Address) -> Result<(), FractionalError> {
        fractional_resources::initialize(&env, &admin)
    }

    /// Fractionalize a resource into divisible shares.
    pub fn fractionalize_resource(
        env: Env,
        owner: Address,
        resource_type: Symbol,
        total_amount: u32,
        shares: u32,
    ) -> Result<Vec<u64>, FractionalError> {
        fractional_resources::fractionalize_resource(
            &env,
            &owner,
            resource_type,
            total_amount,
            shares,
        )
    }

    /// Merge fractional shares back into a whole resource.
    pub fn merge_fractions(
        env: Env,
        owner: Address,
        share_ids: Vec<u64>,
    ) -> Result<u32, FractionalError> {
        fractional_resources::merge_fractions(&env, &owner, share_ids)
    }

    /// Transfer a fractional share to another owner.
