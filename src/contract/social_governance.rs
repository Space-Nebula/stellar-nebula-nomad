    pub fn transfer_share(
        env: Env,
        from: Address,
        to: Address,
        share_id: u64,
    ) -> Result<FractionalShare, FractionalError> {
        fractional_resources::transfer_share(&env, &from, &to, share_id)
    }

    /// Get a fractional share by ID.
    pub fn get_share(env: Env, share_id: u64) -> Option<FractionalShare> {
        fractional_resources::get_share(&env, share_id)
    }

    /// Get all share IDs owned by an address.
    pub fn get_owner_shares(env: Env, owner: Address) -> Vec<u64> {
        fractional_resources::get_owner_shares(&env, &owner)
    }

    /// Get the total number of shares created.
    pub fn get_total_shares(env: Env) -> u64 {
        fractional_resources::get_total_shares(&env)
    }

    /// Get original resource data.
    pub fn get_original_resource(env: Env, resource_type: Symbol) -> Option<OriginalResource> {
        fractional_resources::get_original_resource(&env, resource_type)
    }

    /// Check if an address owns a specific share.
    pub fn is_share_owner(env: Env, owner: Address, share_id: u64) -> bool {
        fractional_resources::is_share_owner(&env, &owner, share_id)
    }

    /// Update fractionalization config (admin only).
    pub fn update_fractional_config(
        env: Env,
        admin: Address,
        min_share_size: u32,
        max_fractions: u32,
    ) -> Result<FractionalConfig, FractionalError> {
        fractional_resources::update_config(&env, &admin, min_share_size, max_fractions)
    }

    // ─── Yield Forecasting API (Issue #90) ─────────────────────────────────

    /// Initialize the yield forecasting system.
    pub fn initialize_forecast(env: Env, admin: Address) -> Result<(), ForecastError> {
        yield_forecast::initialize(&env, &admin)
    }

    /// Generate a yield forecast for a player.
    pub fn generate_yield_forecast(
        env: Env,
        player: Address,
        days: u32,
    ) -> Result<YieldForecast, ForecastError> {
        player.require_auth();
        yield_forecast::generate_yield_forecast(&env, &player, days)
    }

    /// Update the forecast model with new data.
    pub fn update_forecast_model(
        env: Env,
        player: Address,
        data_point: YieldDataPoint,
    ) -> Result<ModelParams, ForecastError> {
        player.require_auth();
        yield_forecast::update_forecast_model(&env, &player, data_point)
    }

    /// Batch generate forecasts for multiple players.
    pub fn batch_generate_forecasts(
        env: Env,
        players: Vec<(Address, u32)>,
    ) -> Vec<Result<YieldForecast, ForecastError>> {
        yield_forecast::batch_generate_forecasts(&env, players)
    }

    /// Get cached forecast for a player.
    pub fn get_cached_forecast(env: Env, player: Address) -> Option<YieldForecast> {
        yield_forecast::get_cached_forecast(&env, &player)
    }

    /// Get historical data for a player.
    pub fn get_player_history(env: Env, player: Address) -> Vec<YieldDataPoint> {
        yield_forecast::get_player_history(&env, &player)
    }

    /// Get number of historical data points for a player.
    pub fn get_history_count(env: Env, player: Address) -> u32 {
        yield_forecast::get_history_count(&env, &player)
    }

    /// Get current model parameters.
    pub fn get_model_params(env: Env) -> Option<ModelParams> {
        yield_forecast::get_model_params(&env)
    }

    /// Get current model version.
    pub fn get_model_version(env: Env) -> u32 {
        yield_forecast::get_model_version(&env)
    }

    /// Update model parameters (admin only).
    pub fn update_model_params(
        env: Env,
        admin: Address,
        moving_average_window: u32,
        trend_weight: u32,
        volatility_adjustment: u32,
    ) -> Result<ModelParams, ForecastError> {
        yield_forecast::update_model_params(
            &env,
            &admin,
            moving_average_window,
            trend_weight,
            volatility_adjustment,
        )
    }

    // ─── Inter-Nebula Wormhole Travel System (Issue #77) ─────────────────────

    /// Open a new wormhole between two nebulae with verifiable travel link.
    pub fn open_wormhole(
        env: Env,
        creator: Address,
        origin_nebula: u64,
        destination: u64,
    ) -> Result<u64, WormholeError> {
        let result =
            wormhole_traveler::open_wormhole(&env, creator.clone(), origin_nebula, destination);
        if result.is_ok() {
            let mut details = [0u8; 128];
            details[0..8].copy_from_slice(&origin_nebula.to_be_bytes());
            details[8..16].copy_from_slice(&destination.to_be_bytes());
            let details_bytes = BytesN::from_array(&env, &details);
            let _ = audit_logger::log_audit_event(
                &env,
                Some(&creator),
                symbol_short!("ow"),
                details_bytes,
            );
        }
        result
    }

    /// Traverse an existing wormhole with energy cost validation and state sync.
    pub fn traverse_wormhole(
        env: Env,
        traveler: Address,
        ship_id: u64,
        wormhole_id: u64,
    ) -> Result<TravelRecord, WormholeError> {
        let result =
            wormhole_traveler::traverse_wormhole(&env, traveler.clone(), ship_id, wormhole_id);
        if result.is_ok() {
            let mut details = [0u8; 128];
            details[0..8].copy_from_slice(&ship_id.to_be_bytes());
            details[8..16].copy_from_slice(&wormhole_id.to_be_bytes());
            let details_bytes = BytesN::from_array(&env, &details);
            let _ = audit_logger::log_audit_event(
                &env,
                Some(&traveler),
                symbol_short!("tw"),
                details_bytes,
            );
        }
        result
    }

    /// Get wormhole details by ID.
    pub fn get_wormhole(env: Env, wormhole_id: u64) -> Option<Wormhole> {
        wormhole_traveler::get_wormhole(&env, wormhole_id)
    }

    /// Get all active wormholes.
    pub fn get_active_wormholes(env: Env) -> Vec<u64> {
        wormhole_traveler::get_active_wormholes(&env)
    }

    /// Get travel history for a ship.
    pub fn get_travel_history(env: Env, ship_id: u64) -> Vec<TravelRecord> {
        wormhole_traveler::get_travel_history(&env, ship_id)
    }

    /// Clean up expired wormholes (maintenance function).
    pub fn cleanup_expired_wormholes(env: Env) -> u32 {
        let cleaned = wormhole_traveler::cleanup_expired_wormholes(&env);
        let mut details = [0u8; 128];
        details[0..4].copy_from_slice(&cleaned.to_be_bytes());
        let details_bytes = BytesN::from_array(&env, &details);
        let _ = audit_logger::log_audit_event(&env, None, symbol_short!("cw"), details_bytes);
        cleaned
    }

    /// Calculate travel cost between two nebulae.
    pub fn calculate_travel_cost(_env: Env, origin_nebula: u64, destination: u64) -> u32 {
        wormhole_traveler::calculate_travel_cost(origin_nebula, destination)
    }

    /// Verify wormhole link integrity.
    pub fn verify_wormhole_link(env: Env, wormhole_id: u64, provided_link: BytesN<32>) -> bool {
        wormhole_traveler::verify_wormhole_link(&env, wormhole_id, provided_link)
    }

    /// Jump through a wormhole and get the full outcome (risk, roll, exit).
    pub fn jump_through_wormhole(
        env: Env,
        traveler: Address,
        ship_id: u64,
        wormhole_id: u64,
    ) -> Result<wormhole_traveler::TravelOutcome, WormholeError> {
        wormhole_traveler::jump_through_wormhole(&env, traveler, ship_id, wormhole_id)
    }

    /// Bail out back through the wormhole a ship just came through.
    pub fn wormhole_emergency_exit(
        env: Env,
        traveler: Address,
        ship_id: u64,
    ) -> Result<wormhole_traveler::TravelOutcome, WormholeError> {
        wormhole_traveler::emergency_exit(&env, traveler, ship_id)
    }

    /// Scan a nebula for wormholes; detected ones join the explorer's map.
    pub fn scan_for_wormholes(env: Env, explorer: Address, nebula_id: u64) -> Vec<u64> {
        wormhole_traveler::scan_for_wormholes(&env, explorer, nebula_id)
    }

    /// An explorer's wormhole map with live stability, risk and cost.
    pub fn get_wormhole_map(
        env: Env,
        explorer: Address,
    ) -> Vec<wormhole_traveler::WormholeMapEntry> {
        wormhole_traveler::get_wormhole_map(&env, &explorer)
    }

    /// Open wormhole routes leaving a nebula.
    pub fn get_wormhole_routes(
        env: Env,
        nebula_id: u64,
    ) -> Vec<wormhole_traveler::WormholeMapEntry> {
        wormhole_traveler::get_routes_from(&env, nebula_id)
    }

    // ─── Player Alliance and Faction System (Issue #79) ──────────────────

    /// Found a new alliance with initial treasury.
    pub fn found_alliance(env: Env, founder: Address, name: String) -> Result<u64, AllianceError> {
        let result = alliance_manager::found_alliance(&env, founder.clone(), name);
        if result.is_ok() {
            let mut details = [0u8; 128];
            if let Ok(alliance_id) = result {
                details[0..8].copy_from_slice(&alliance_id.to_be_bytes());
            }
            let details_bytes = BytesN::from_array(&env, &details);
            let _ = audit_logger::log_audit_event(
                &env,
                Some(&founder),
                symbol_short!("fa"),
                details_bytes,
            );
        }
        result
    }

    /// Join an existing alliance.
    pub fn join_alliance(
        env: Env,
        alliance_id: u64,
        player: Address,
    ) -> Result<MembershipRecord, AllianceError> {
        let result = alliance_manager::join_alliance(&env, alliance_id, player.clone());
        if result.is_ok() {
            let mut details = [0u8; 128];
            details[0..8].copy_from_slice(&alliance_id.to_be_bytes());
            let details_bytes = BytesN::from_array(&env, &details);
            let _ = audit_logger::log_audit_event(
                &env,
                Some(&player),
                symbol_short!("ja"),
                details_bytes,
            );
        }
        result
    }

    /// Leave an alliance.
    pub fn leave_alliance(env: Env, player: Address) -> Result<(), AllianceError> {
        alliance_manager::leave_alliance(&env, player)
    }

    /// Contribute resources to alliance treasury.
    pub fn contribute_to_treasury(
        env: Env,
        player: Address,
        amount: i128,
    ) -> Result<i128, AllianceError> {
        alliance_manager::contribute_to_treasury(&env, player, amount)
    }

    /// Get alliance details.
    pub fn get_alliance(env: Env, alliance_id: u64) -> Result<Alliance, AllianceError> {
        alliance_manager::get_alliance(&env, alliance_id)
    }

    /// Get alliance treasury balance.
    pub fn get_alliance_treasury(env: Env, alliance_id: u64) -> i128 {
        alliance_manager::get_alliance_treasury(&env, alliance_id)
    }

    /// Get member's contribution to alliance.
    pub fn get_member_contribution(env: Env, alliance_id: u64, member: Address) -> i128 {
        alliance_manager::get_member_contribution(&env, alliance_id, member)
    }

    /// Get player's current alliance ID.
    pub fn get_player_alliance(env: Env, player: Address) -> Option<u64> {
        alliance_manager::get_player_alliance(&env, player)
    }

    // ─── Cooperative Guild Quests (Issue #595) ────────────────────────────

    /// Create a one-off guild quest with the legacy 100% treasury payout.
    #[allow(clippy::too_many_arguments)]
    pub fn create_guild_quest(
        env: Env,
        admin: Address,
        alliance_id: u64,
        quest_type: Symbol,
        target_count: u64,
        reward_essence: i128,
        reward_xp: u64,
        duration_secs: u64,
    ) -> Result<u64, GuildQuestError> {
        guild_quests::create_guild_quest(
            &env,
            admin,
            alliance_id,
            quest_type,
            target_count,
            reward_essence,
            reward_xp,
            duration_secs,
        )
    }

    /// Create a guild quest with frequency, tier, chain and cooperative settings.
    #[allow(clippy::too_many_arguments)]
    pub fn create_guild_quest_with_config(
        env: Env,
        admin: Address,
        alliance_id: u64,
        quest_type: Symbol,
        target_count: u64,
        reward_essence: i128,
        reward_xp: u64,
        duration_secs: u64,
        config: GuildQuestConfig,
    ) -> Result<u64, GuildQuestError> {
        guild_quests::create_guild_quest_with_config(
            &env,
            admin,
            alliance_id,
            quest_type,
            target_count,
            reward_essence,
            reward_xp,
            duration_secs,
            config,
        )
    }

    /// Define an ordered chain of guild quests.
    pub fn create_guild_quest_chain(
        env: Env,
        admin: Address,
        alliance_id: u64,
        name: Symbol,
    ) -> Result<u64, GuildQuestError> {
        guild_quests::create_guild_quest_chain(&env, admin, alliance_id, name)
    }

    /// Restrict a quest to a roster of alliance members. An empty roster clears it.
    pub fn assign_guild_quest(
        env: Env,
        admin: Address,
        alliance_id: u64,
        quest_id: u64,
        assignees: Vec<Address>,
    ) -> Result<(), GuildQuestError> {
        guild_quests::assign_guild_quest(&env, admin, alliance_id, quest_id, assignees)
    }

    /// Make a quest open to every member again.
    pub fn clear_guild_quest_assignment(
        env: Env,
        admin: Address,
        alliance_id: u64,
        quest_id: u64,
    ) -> Result<(), GuildQuestError> {
        guild_quests::clear_guild_quest_assignment(&env, admin, alliance_id, quest_id)
    }

    /// Contribute to every matching active quest in the caller's alliance.
    pub fn contribute_guild_quest_progress(
        env: Env,
        player: Address,
        quest_type: Symbol,
        amount: u64,
    ) -> Result<(), GuildQuestError> {
        player.require_auth();
        guild_quests::contribute_quest_progress(&env, player, quest_type, amount)
    }

    /// Spawn this period's daily/weekly/seasonal quest instances from templates.
    pub fn rotate_guild_quests(
        env: Env,
        admin: Address,
        alliance_id: u64,
        template_ids: Vec<u64>,
    ) -> Result<u64, GuildQuestError> {
        guild_quests::rotate_guild_quests(&env, admin, alliance_id, template_ids)
    }

    /// Active guild quests for an alliance (rotation-aware).
    pub fn get_active_guild_quests(env: Env, alliance_id: u64) -> Vec<GuildQuest> {
        guild_quests::get_active_guild_quests(&env, alliance_id)
    }

    /// Status-rich quest list for the guild quest board UI.
    pub fn get_guild_quest_board(env: Env, alliance_id: u64) -> Vec<GuildQuestSummary> {
        guild_quests::get_guild_quest_board(&env, alliance_id)
    }

    /// Read one guild quest.
    pub fn get_guild_quest(env: Env, alliance_id: u64, quest_id: u64) -> Option<GuildQuest> {
        guild_quests::get_guild_quest(&env, alliance_id, quest_id)
    }

    /// Read a guild quest chain.
    pub fn get_guild_quest_chain(
        env: Env,
        alliance_id: u64,
        chain_id: u64,
    ) -> Option<GuildQuestChain> {
        guild_quests::get_guild_quest_chain(&env, alliance_id, chain_id)
    }

    /// Read a quest's assignment roster.
    pub fn get_guild_quest_assignment(
        env: Env,
        alliance_id: u64,
        quest_id: u64,
    ) -> Option<QuestAssignment> {
        guild_quests::get_guild_quest_assignment(&env, alliance_id, quest_id)
    }

    /// Read the members who contributed to a quest.
    pub fn get_guild_quest_contributors(
        env: Env,
        alliance_id: u64,
        quest_id: u64,
    ) -> Vec<Address> {
        guild_quests::get_guild_quest_contributors(&env, alliance_id, quest_id)
    }

    /// Read the guild economy payout recorded for a completed quest.
    pub fn get_guild_quest_payout(
        env: Env,
        alliance_id: u64,
        quest_id: u64,
    ) -> Option<QuestRewardPayout> {
        guild_economy::get_quest_reward_payout(&env, alliance_id, quest_id)
    }

    /// Claim a contributor's share of a completed guild quest.
    pub fn claim_guild_quest_reward_share(
        env: Env,
        player: Address,
        alliance_id: u64,
        quest_id: u64,
    ) -> Result<i128, GuildEconomyError> {
        guild_economy::claim_quest_reward_share(&env, player, alliance_id, quest_id)
    }

    // ─── Dynamic Resource Market Oracle Integration (Issue #78) ──────────

    /// Initialize the market oracle with admin and default sources.
    pub fn initialize_oracle(
        env: Env,
        admin: Address,
        sources: Vec<Address>,
    ) -> Result<(), MarketOracleError> {
        market_oracle::initialize_oracle(&env, admin, sources)
    }

    /// Update resource price with timestamp verification.
    pub fn update_resource_price(
        env: Env,
        admin: Address,
        resource: Symbol,
        new_price: i128,
    ) -> Result<PriceData, MarketOracleError> {
        market_oracle::update_resource_price(&env, admin, resource, new_price)
    }

    /// Batch update multiple resource prices.
    pub fn batch_update_prices(
        env: Env,
        admin: Address,
        resources: Vec<Symbol>,
        prices: Vec<i128>,
    ) -> Result<Vec<PriceData>, MarketOracleError> {
        market_oracle::batch_update_prices(&env, admin, resources, prices)
    }

    /// Get current market rate for a resource (pure view).
    pub fn get_current_market_rate(env: Env, resource: Symbol) -> Result<i128, MarketOracleError> {
        market_oracle::get_current_market_rate(&env, resource)
    }

    /// Get price data with metadata.
    pub fn get_price_data(env: Env, resource: Symbol) -> Result<PriceData, MarketOracleError> {
        market_oracle::get_price_data(&env, resource)
    }

    /// Get 24h price history for a resource.
    pub fn get_price_history(env: Env, resource: Symbol) -> Vec<PriceData> {
        market_oracle::get_price_history(&env, resource)
    }

    /// Add oracle source (admin only).
    pub fn add_oracle_source(
        env: Env,
        admin: Address,
        new_source: Address,
    ) -> Result<(), MarketOracleError> {
        market_oracle::add_oracle_source(&env, admin, new_source)
    }

    /// Remove a compromised oracle source (admin only, Issue #654).
    pub fn remove_oracle_source(
        env: Env,
        admin: Address,
        source: Address,
    ) -> Result<(), MarketOracleError> {
        market_oracle::remove_oracle_source(&env, admin, source)
    }

    /// List registered oracle sources.
    pub fn get_oracle_sources(env: Env) -> Vec<Address> {
        market_oracle::get_oracle_sources(&env)
    }

    /// Submit a signed price observation from a registered source (Issue #654).
    pub fn submit_source_price(
        env: Env,
        source: Address,
        resource: Symbol,
        price: i128,
    ) -> Result<SourcePrice, MarketOracleError> {
        market_oracle::submit_source_price(&env, source, resource, price)
    }

    /// Aggregate fresh source observations into a consensus price
    /// (permissionless keeper call, Issue #654).
    pub fn aggregate_resource_price(
        env: Env,
        resource: Symbol,
    ) -> Result<PriceData, MarketOracleError> {
        market_oracle::aggregate_resource_price(&env, resource)
    }

    // ─── Procedural Music and Sound Seed Generator (Issue #80) ───────────

    /// Initialize default instrument presets.
    pub fn initialize_presets(env: Env) {
        audio_seed_generator::initialize_presets(&env)
    }

    /// Generate deterministic music seed from nebula state.
    pub fn generate_music_seed(env: Env, nebula_id: u64) -> Result<MusicSeed, AudioError> {
        audio_seed_generator::generate_music_seed(&env, nebula_id)
    }

    /// Get instrument layer parameters for frontend rendering.
    pub fn get_instrument_layer(
        env: Env,
        seed: BytesN<32>,
        layer: u32,
    ) -> Result<InstrumentParams, AudioError> {
        audio_seed_generator::get_instrument_layer(&env, seed, layer)
    }

    /// Get all 8 layers for a nebula instantly.
    pub fn get_all_layers(env: Env, nebula_id: u64) -> Result<Vec<InstrumentParams>, AudioError> {
        audio_seed_generator::get_all_layers(&env, nebula_id)
    }

    /// Get stored seed for a nebula.
    pub fn get_nebula_seed(env: Env, nebula_id: u64) -> Option<BytesN<32>> {
        audio_seed_generator::get_nebula_seed(&env, nebula_id)
    }

    /// Get instrument preset by ID.
    pub fn get_preset(env: Env, preset_id: u32) -> Result<InstrumentParams, AudioError> {
        audio_seed_generator::get_preset(&env, preset_id)
    }

    // ─── Privacy-Preserving Player Stats (Issue #XX) ─────────────────────

    /// Opt in to privacy-preserving stat sharing.
    pub fn opt_in_privacy(env: Env, player: Address) -> Result<(), PrivacyError> {
        privacy_stats::opt_in_privacy(&env, player)
    }

    /// Check if a player has opted in to privacy features.
    pub fn is_opted_in_privacy(env: Env, player: Address) -> bool {
        privacy_stats::is_opted_in(&env, &player)
    }

    /// Commit a private stat without revealing the raw value.
    pub fn commit_private_stat(
        env: Env,
        player: Address,
        stat_type: Symbol,
        value: i128,
    ) -> Result<BytesN<32>, PrivacyError> {
        privacy_stats::commit_private_stat(&env, player, stat_type, value)
    }

    /// Verify a private stat commitment using a zero-knowledge proof.
    pub fn verify_private_stat(
        env: Env,
        commitment: BytesN<32>,
        proof: BytesN<64>,
    ) -> Result<bool, PrivacyError> {
        privacy_stats::verify_private_stat(&env, commitment, proof)
    }

    /// Get a commitment for a player and stat type.
    pub fn get_commitment(
        env: Env,
        player: Address,
        stat_type: Symbol,
    ) -> Result<StatCommitment, PrivacyError> {
        privacy_stats::get_commitment(&env, player, stat_type)
    }

    /// Get total number of commitments made across all players.
    pub fn get_commitment_count(env: Env) -> u64 {
        privacy_stats::get_commitment_count(&env)
    }

    /// Batch commit multiple stats in a single transaction (up to 10).
    pub fn batch_commit_stats(
        env: Env,
        player: Address,
        stat_types: Vec<Symbol>,
        values: Vec<i128>,
    ) -> Result<Vec<BytesN<32>>, PrivacyError> {
        privacy_stats::batch_commit_stats(&env, player, stat_types, values)
    }

    /// Reset privacy burst counter for a new transaction.
    pub fn reset_privacy_burst_counter(env: Env) {
        privacy_stats::reset_burst_counter(&env)
    }

    // ─── Nebula Navigation Route Planner (Issue #69) ──────────────────────────

    /// Initialise the nebula navigation graph with an admin address.
    pub fn initialize_nav_graph(env: Env, admin: Address) -> Result<(), NavError> {
        navigation_planner::initialize_nav_graph(&env, &admin)
    }

    /// Register a directed edge (connection) between two nebulae.
    pub fn add_nebula_connection(
        env: Env,
        admin: Address,
        from: u64,
        to: u64,
        fuel_cost: u32,
        hazard_level: u32,
    ) -> Result<(), NavError> {
        navigation_planner::add_nebula_connection(&env, &admin, from, to, fuel_cost, hazard_level)
    }

    /// Add up to MAX_CONNECTIONS_PER_BATCH edges in a single transaction.
    pub fn add_nebula_connections_batch(
        env: Env,
        admin: Address,
        edges: Vec<RouteEdge>,
    ) -> Result<u32, NavError> {
        navigation_planner::add_nebula_connections_batch(&env, &admin, edges)
    }

    /// Dijkstra shortest-fuel-cost route between two nebulae (≤ 12 hops).
    /// Emits RouteCalculated event on success.
    pub fn calculate_optimal_route(env: Env, start: u64, dest: u64) -> Result<NavPath, NavError> {
        navigation_planner::calculate_optimal_route(&env, start, dest)
    }

    /// Validate a caller-supplied route Vec and return its aggregate risk score.
    pub fn validate_route_safety(env: Env, route: Vec<u64>) -> Result<u32, NavError> {
        navigation_planner::validate_route_safety(&env, route)
    }

    /// Return the adjacency list (outgoing edges) for a nebula.
    pub fn get_neighbors(env: Env, nebula_id: u64) -> Vec<RouteEdge> {
        navigation_planner::get_neighbors(&env, nebula_id)
    }

    /// Return the single directed edge from `from` to `to`, if it exists.
    pub fn get_nav_connection(env: Env, from: u64, to: u64) -> Option<RouteEdge> {
        navigation_planner::get_connection(&env, from, to)
    }

    // ─── Standardized event framework ────────────────────────────────────

    /// Seed the default schemas and remember the framework admin.
    pub fn init_event_framework(env: Env, admin: Address) {
        event_framework::init_event_framework(&env, &admin)
    }

    /// Register (or re-version) the schema for one event type.
    pub fn register_event_schema(
        env: Env,
        admin: Address,
        event_type: Symbol,
        version: u32,
    ) -> Result<(), EventFrameworkError> {
        event_framework::register_event_schema(&env, &admin, &event_type, version)
    }

    /// Persist one schema-versioned event record and publish it.
    pub fn emit_standard_event(
        env: Env,
        caller: Address,
        event_type: Symbol,
        payload: BytesN<256>,
    ) -> Result<u64, EventFrameworkError> {
        event_framework::emit_standard_event(&env, &caller, event_type, payload)
    }

    /// Persist up to `MAX_BURST_EVENTS` records and publish a single batched
    /// event for the whole burst.
    pub fn emit_standard_event_burst(
        env: Env,
        caller: Address,
        event_type: Symbol,
        payloads: Vec<BytesN<256>>,
    ) -> Result<u32, EventFrameworkError> {
        event_framework::emit_standard_event_burst(&env, &caller, event_type, payloads)
    }

    /// Most recent records of `filter` (`all` for every type), newest first.
    pub fn query_recent_events(env: Env, filter: Symbol, limit: u32) -> Vec<StandardEvent> {
        event_framework::query_recent_events(&env, &filter, limit)
    }

    // ─── Automated Community Event Scheduler ──────────────────────────────

    /// Initialize the event scheduler with an admin address.
    pub fn initialize_scheduler(env: Env, admin: Address) {
        event_scheduler::initialize_scheduler(&env, &admin)
    }

    /// Schedule a new community event.
