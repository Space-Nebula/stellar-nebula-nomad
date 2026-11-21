    pub fn generate_nebula_layout(env: Env, seed: BytesN<32>, player: Address) -> NebulaLayout {
        player.require_auth();
        nebula_explorer::generate_nebula_layout(&env, &seed, &player)
    }

    /// Calculate the rarity tier of a nebula layout.
    pub fn calculate_rarity_tier(env: Env, layout: NebulaLayout) -> Rarity {
        nebula_explorer::calculate_rarity_tier(&env, &layout)
    }

    /// Full scan: generates layout, calculates rarity, and emits a
    /// `NebulaScanned` event containing the layout hash.
    ///
    /// Full scan: generates layout, calculates rarity, emits NebulaScanned event.
    /// Also updates the on-chain analytics counters (total_scans,
    /// total_essence_accrued) and registers the player for the leaderboard.
    pub fn scan_nebula(env: Env, seed: BytesN<32>, player: Address) -> (NebulaLayout, Rarity) {
        player.require_auth();
        // Daily scan cap (anti-whale). The signature has no error channel, so
        // a capped player gets the contract error code (303) via a trap.
        if let Err(e) = economics::anti_whale::check_operation(&env, &player, OpKind::Scan, 1) {
            soroban_sdk::panic_with_error!(&env, e);
        }
        let layout = nebula_explorer::generate_nebula_layout(&env, &seed, &player);
        let rarity = nebula_explorer::calculate_rarity_tier(&env, &layout);
        let layout_hash = nebula_explorer::compute_layout_hash(&env, &layout);
        nebula_explorer::emit_nebula_scanned(&env, &player, &layout_hash, &rarity);

        // Record analytics: use total_energy as the essence earned this scan.
        analytics::record_scan(&env, &player, layout.total_energy as u64);

        (layout, rarity)
    }

    /// Return aggregate global statistics (total scans, ships minted, etc.).
    ///
    /// Pure view — no ledger writes, zero gas cost beyond the read.
    pub fn get_global_stats(env: Env) -> GlobalStats {
        analytics::get_global_stats(&env)
    }

    /// Return the top-`top_n` explorers sorted by cumulative cosmic essence.
    ///
    /// Emits a `LeaderboardSnapshot` event so frontends can subscribe via
    /// Stellar event streams.  Returns `Err(InvalidTopN)` when `top_n` is 0
    /// or exceeds 50.
    pub fn snapshot_leaderboard(
        env: Env,
        top_n: u32,
    ) -> Result<Vec<analytics::LeaderboardEntry>, AnalyticsError> {
        analytics::snapshot_leaderboard(&env, top_n)
    }

    // === Enhanced Leaderboards API (Issue #159) ===

    /// Update a player's score in a specific leaderboard category and time period.
    pub fn update_leaderboard_score(
        env: Env,
        player: Address,
        category: Symbol,
        time_period: Symbol,
        score: u64,
    ) -> Result<(), LeaderboardError> {
        leaderboards::update_score(&env, &player, category, time_period, score)?;
        // Mirror the score onto the player's power-tier board so whales rank
        // against whales, not against newcomers.
        economics::anti_whale::record_tier_score(&env, &player, score);
        Ok(())
    }

    /// Get leaderboard entries for a category and time period.
    pub fn get_leaderboard(
        env: Env,
        category: Symbol,
        time_period: Symbol,
        limit: u32,
    ) -> Result<Vec<EnhancedLeaderboardEntry>, LeaderboardError> {
        leaderboards::get_leaderboard(&env, category, time_period, limit)
    }

    /// Read one page of a category leaderboard (bounded read for large boards).
    pub fn get_leaderboard_page(
        env: Env,
        category: Symbol,
        time_period: Symbol,
        page: u32,
        page_size: u32,
    ) -> Result<(Vec<EnhancedLeaderboardEntry>, PageMeta), LeaderboardError> {
        leaderboards::get_leaderboard_page(&env, category, time_period, page, page_size)
    }

    /// Read one page of the guild leaderboard.
    pub fn get_guild_leaderboard_page(
        env: Env,
        page: u32,
        page_size: u32,
    ) -> Result<(Vec<leaderboards::GuildEntry>, PageMeta), LeaderboardError> {
        leaderboards::get_guild_leaderboard_page(&env, page, page_size)
    }

    /// Read one page of a regional leaderboard.
    pub fn get_regional_leaderboard_page(
        env: Env,
        region: Symbol,
        page: u32,
        page_size: u32,
    ) -> Result<(Vec<leaderboards::RegionalEntry>, PageMeta), LeaderboardError> {
        leaderboards::get_regional_leaderboard_page(&env, region, page, page_size)
    }

    /// Read one page of the achievement leaderboard.
    pub fn get_achievement_leaderboard_page(
        env: Env,
        page: u32,
        page_size: u32,
    ) -> Result<(Vec<leaderboards::AchievementEntry>, PageMeta), LeaderboardError> {
        leaderboards::get_achievement_leaderboard_page(&env, page, page_size)
    }

    /// Read one page of an archived (season) leaderboard.
    pub fn get_archived_leaderboard_page(
        env: Env,
        category: Symbol,
        time_period: Symbol,
        season: u32,
        page: u32,
        page_size: u32,
    ) -> Result<(Vec<EnhancedLeaderboardEntry>, PageMeta), LeaderboardError> {
        leaderboards::get_archived_leaderboard_page(
            &env,
            category,
            time_period,
            season,
            page,
            page_size,
        )
    }

    /// Set a player's guild affiliation.
    pub fn set_player_guild(
        env: Env,
        player: Address,
        guild_name: String,
    ) -> Result<(), LeaderboardError> {
        leaderboards::set_player_guild(&env, &player, guild_name)
    }

    /// Get a player's guild.
    pub fn get_player_guild(env: Env, player: Address) -> Option<String> {
        leaderboards::get_player_guild(&env, &player)
    }

    /// Update guild leaderboard score (admin only).
    pub fn update_guild_score(
        env: Env,
        caller: Address,
        guild_name: String,
        score: u64,
        member_count: u32,
    ) -> Result<(), LeaderboardError> {
        leaderboards::update_guild_score(&env, &caller, guild_name, score, member_count)
    }

    /// Get guild leaderboard.
    pub fn get_guild_leaderboard(env: Env, limit: u32) -> Vec<leaderboards::GuildEntry> {
        leaderboards::get_guild_leaderboard(&env, limit)
    }

    /// Update regional leaderboard score.
    pub fn update_regional_score(
        env: Env,
        player: Address,
        region: Symbol,
        score: u64,
    ) -> Result<(), LeaderboardError> {
        leaderboards::update_regional_score(&env, &player, region, score)
    }

    /// Get regional leaderboard.
    pub fn get_regional_leaderboard(
        env: Env,
        region: Symbol,
        limit: u32,
    ) -> Result<Vec<leaderboards::RegionalEntry>, LeaderboardError> {
        leaderboards::get_regional_leaderboard(&env, region, limit)
    }

    /// Update achievement leaderboard.
    pub fn update_achievement_score(
        env: Env,
        player: Address,
        achievement_count: u32,
        points: u64,
    ) -> Result<(), LeaderboardError> {
        leaderboards::update_achievement_score(&env, &player, achievement_count, points)
    }

    /// Get achievement leaderboard.
    pub fn get_achievement_leaderboard(
        env: Env,
        limit: u32,
    ) -> Vec<leaderboards::AchievementEntry> {
        leaderboards::get_achievement_leaderboard(&env, limit)
    }

    /// Distribute rewards to top leaderboard players (admin only).
    pub fn distribute_leaderboard_rewards(
        env: Env,
        caller: Address,
        category: Symbol,
        time_period: Symbol,
        rewards: LeaderboardRewards,
    ) -> Result<(), LeaderboardError> {
        leaderboards::distribute_rewards(&env, &caller, category, time_period, rewards)
    }

    /// Set leaderboard admin (admin only).
    pub fn set_leaderboard_admin(env: Env, admin: Address) {
        let _ = leaderboards::set_admin(&env, &admin);
    }

    // === Content Creation Tools API (Issue #158) ===

    /// Create new community content (nebula, mission, or event).
    pub fn create_content(
        env: Env,
        creator: Address,
        name: String,
        description: String,
        content_type: Symbol,
        data: Bytes,
        is_public: bool,
        tags: Vec<Symbol>,
    ) -> Result<u64, ContentToolsError> {
        content_tools::create_content(
            &env,
            &creator,
            name,
            description,
            content_type,
            data,
            is_public,
            tags,
        )
    }

    /// Update existing content.
    pub fn update_content(
        env: Env,
        creator: Address,
        content_id: u64,
        name: Option<String>,
        description: Option<String>,
        data: Option<Bytes>,
        is_public: Option<bool>,
        tags: Option<Vec<Symbol>>,
    ) -> Result<(), ContentToolsError> {
        content_tools::update_content(
            &env,
            &creator,
            content_id,
            name,
            description,
            data,
            is_public,
            tags,
        )
    }

    /// Get content by ID.
    pub fn get_content(env: Env, content_id: u64) -> Result<CreatedContent, ContentToolsError> {
        content_tools::get_content(&env, content_id)
    }

    /// Get all content IDs created by an address.
    pub fn get_creator_content(env: Env, creator: Address) -> Vec<u64> {
        content_tools::get_creator_content(&env, &creator)
    }

    /// Delete content (creator only).
    pub fn delete_content(
        env: Env,
        creator: Address,
        content_id: u64,
    ) -> Result<(), ContentToolsError> {
        content_tools::delete_content(&env, &creator, content_id)
    }

    /// Approve content (admin only).
    pub fn approve_content(
        env: Env,
        caller: Address,
        content_id: u64,
    ) -> Result<(), ContentToolsError> {
        content_tools::approve_content(&env, &caller, content_id)
    }

    /// Reject content (admin only).
    pub fn reject_content(
        env: Env,
        caller: Address,
        content_id: u64,
    ) -> Result<(), ContentToolsError> {
        content_tools::reject_content(&env, &caller, content_id)
    }

    /// Get content status.
    pub fn get_content_status(env: Env, content_id: u64) -> Symbol {
        content_tools::get_content_status(&env, content_id)
    }

    /// Vote/rate content (1-5 stars).
    pub fn vote_content(
        env: Env,
        voter: Address,
        content_id: u64,
        rating: u32,
    ) -> Result<(), ContentToolsError> {
        content_tools::vote_content(&env, &voter, content_id, rating)
    }

    /// Get vote result for content.
    pub fn get_vote_result(env: Env, content_id: u64) -> VoteResult {
        content_tools::get_vote_result(&env, content_id)
    }

    /// Increment play count for content.
    pub fn increment_play_count(env: Env, content_id: u64) -> Result<(), ContentToolsError> {
        content_tools::increment_play_count(&env, content_id)
    }

    /// List content on marketplace.
    pub fn list_on_marketplace(
        env: Env,
        seller: Address,
        content_id: u64,
        price: i128,
    ) -> Result<u64, ContentToolsError> {
        content_tools::list_on_marketplace(&env, &seller, content_id, price)
    }

    /// Unlist content from marketplace.
    pub fn unlist_from_marketplace(
        env: Env,
        seller: Address,
        listing_id: u64,
    ) -> Result<(), ContentToolsError> {
        content_tools::unlist_from_marketplace(&env, &seller, listing_id)
    }

    /// Get marketplace listing.
    pub fn get_marketplace_listing(
        env: Env,
        listing_id: u64,
    ) -> Result<MarketplaceListing, ContentToolsError> {
        content_tools::get_marketplace_listing(&env, listing_id)
    }

    /// Set content tools admin (admin only).
    pub fn set_content_admin(env: Env, admin: Address) {
        let _ = content_tools::set_admin(&env, &admin);
    }

    // ─── Content Revenue Sharing (Issue #192) ────────────────────────────────

    /// Set the price of a content item (creator only).
    pub fn set_content_price(
        env: Env,
        creator: Address,
        content_id: u64,
        price: i128,
    ) -> Result<(), ContentToolsError> {
        content_tools::set_content_price(&env, &creator, content_id, price)
    }

    /// Purchase content. Revenue split between creator and platform.
    pub fn purchase_content(
        env: Env,
        buyer: Address,
        content_id: u64,
    ) -> Result<PurchaseResult, ContentToolsError> {
        content_tools::purchase_content(&env, &buyer, content_id)
    }

    /// Get the revenue split configuration.
    pub fn get_revenue_split_config(env: Env) -> RevenueSplitConfig {
        content_tools::get_revenue_split_config(&env)
    }

    /// Set the revenue split configuration (admin only).
    pub fn set_revenue_split_config(
        env: Env,
        caller: Address,
        config: RevenueSplitConfig,
    ) -> Result<(), ContentToolsError> {
        content_tools::set_revenue_split_config(&env, &caller, config)
    }

    /// Get accumulated revenue for a creator.
    pub fn get_creator_revenue(env: Env, creator: Address) -> i128 {
        content_tools::get_creator_revenue(&env, &creator)
    }

    /// Get accumulated platform revenue.
    pub fn get_platform_revenue(env: Env) -> i128 {
        content_tools::get_platform_revenue(&env)
    }

    /// Withdraw creator revenue (creator only).
    pub fn withdraw_creator_revenue(
        env: Env,
        creator: Address,
        amount: i128,
    ) -> Result<i128, ContentToolsError> {
        content_tools::withdraw_creator_revenue(&env, &creator, amount)
    }

    // === PvP Combat API (Issue #152) ===

    /// Get combat stats for a player.
    pub fn get_combat_stats(env: Env, player: Address) -> pvp_combat::CombatStats {
        pvp_combat::get_combat_stats(&env, &player)
    }

    /// Get ELO rating for a player.
    pub fn get_elo_rating(env: Env, player: Address) -> u32 {
        pvp_combat::get_elo_rating(&env, &player)
    }

    /// Create a PvP challenge.
    pub fn create_challenge(
        env: Env,
        challenger: Address,
        opponent: Address,
        stake: i128,
    ) -> Result<u64, PvPError> {
        pvp_combat::create_challenge(&env, &challenger, &opponent, stake)
    }

    /// Accept a PvP challenge.
    pub fn accept_challenge(env: Env, caller: Address, challenge_id: u64) -> Result<u64, PvPError> {
        pvp_combat::accept_challenge(&env, &caller, challenge_id)
    }

    /// Decline a PvP challenge.
    pub fn decline_challenge(env: Env, caller: Address, challenge_id: u64) -> Result<(), PvPError> {
        pvp_combat::decline_challenge(&env, &caller, challenge_id)
    }

    /// Get a challenge by ID.
    pub fn get_challenge(env: Env, challenge_id: u64) -> Result<Challenge, PvPError> {
        pvp_combat::get_challenge(&env, challenge_id)
    }

    /// Execute a combat move.
    pub fn execute_combat_move(
        env: Env,
        player: Address,
        combat_id: u64,
        move_type: Symbol,
        power: u32,
    ) -> Result<(), PvPError> {
        pvp_combat::execute_move(&env, &player, combat_id, move_type, power)
    }

    /// Get combat state.
    pub fn get_combat(env: Env, combat_id: u64) -> Result<CombatState, PvPError> {
        pvp_combat::get_combat(&env, combat_id)
    }

    /// Get player's combat history.
    pub fn get_player_combat_history(env: Env, player: Address, limit: u32) -> Vec<CombatHistory> {
        pvp_combat::get_player_combat_history(&env, &player, limit)
    }

    /// Join matchmaking queue.
    pub fn join_matchmaking(
        env: Env,
        player: Address,
        preferred_stake: i128,
    ) -> Result<(), PvPError> {
        pvp_combat::join_matchmaking(&env, &player, preferred_stake)
    }

    /// Leave matchmaking queue.
    pub fn leave_matchmaking(env: Env, player: Address) -> Result<(), PvPError> {
        pvp_combat::leave_matchmaking(&env, &player)
    }

    /// Process matchmaking (admin/system).
    pub fn process_matchmaking(env: Env) -> Result<Option<(Address, Address)>, PvPError> {
        pvp_combat::process_matchmaking(&env)
    }

    /// Get matchmaking queue size.
    pub fn get_matchmaking_queue_size(env: Env) -> u32 {
        pvp_combat::get_matchmaking_queue_size(&env)
    }

    // ─── ELO Decay (Issue #191) ─────────────────────────────────────────────

    /// Apply ELO decay for a player based on inactivity.
    pub fn apply_elo_decay(env: Env, player: Address) -> u32 {
        pvp_combat::apply_elo_decay(&env, &player)
    }

    /// Get the last active timestamp for a player.
    pub fn get_last_active(env: Env, player: Address) -> u64 {
        pvp_combat::get_last_active(&env, &player)
    }

    /// Set ELO decay configuration (admin only).
    pub fn set_elo_decay_config(
        env: Env,
        caller: Address,
        config: EloDecayConfig,
    ) -> Result<(), PvPError> {
        pvp_combat::set_elo_decay_config(&env, &caller, config)
    }

    /// Get ELO decay configuration.
    pub fn get_elo_decay_config(env: Env) -> EloDecayConfig {
        pvp_combat::get_elo_decay_config(&env)
    }

    /// Apply batch ELO decay (admin only).
    pub fn apply_batch_elo_decay(
        env: Env,
        caller: Address,
        players: Vec<Address>,
    ) -> Result<u32, PvPError> {
        pvp_combat::apply_batch_elo_decay(&env, &caller, players)
    }

    /// Add spectator to combat.
    pub fn add_spectator(env: Env, spectator: Address, combat_id: u64) -> Result<(), PvPError> {
        pvp_combat::add_spectator(&env, &spectator, combat_id)
    }

    /// Remove spectator from combat.
    pub fn remove_spectator(env: Env, spectator: Address, combat_id: u64) -> Result<(), PvPError> {
        pvp_combat::remove_spectator(&env, &spectator, combat_id)
    }

    /// Get combat spectators.
    pub fn get_spectators(env: Env, combat_id: u64) -> Vec<Address> {
        pvp_combat::get_spectators(&env, combat_id)
    }

    /// Set combat rewards config (admin only).
    pub fn set_combat_rewards_config(
        env: Env,
        caller: Address,
        config: RewardsConfig,
    ) -> Result<(), PvPError> {
        pvp_combat::set_rewards_config(&env, &caller, config)
    }

    /// Get combat rewards config.
    pub fn get_combat_rewards_config(env: Env) -> RewardsConfig {
        pvp_combat::get_rewards_config(&env)
    }

    /// Set PvP combat admin (admin only).
    pub fn set_pvp_admin(env: Env, admin: Address) {
        let _ = pvp_combat::set_admin(&env, &admin);
    }

    // ─── MMR Matchmaking — Issue #591 ─────────────────────────────────────

    /// Enqueue a player for MMR-based matchmaking in the given mode.
    pub fn mm_enqueue(env: Env, player: Address, mode: GameMode) -> Result<(), PvPError> {
        matchmaking::enqueue(&env, &player, mode)
    }

    /// Remove a player from the matchmaking queue.
    pub fn mm_dequeue(env: Env, player: Address, mode: GameMode) -> Result<(), PvPError> {
        matchmaking::dequeue(&env, &player, mode)
    }

    /// Process the matchmaking queue for a game mode — returns matched pair
    /// or None. Prunes timed-out entries automatically.
    pub fn mm_process(env: Env, mode: GameMode) -> Result<Option<MatchmakingResult>, PvPError> {
        matchmaking::process_queue(&env, mode)
    }

    /// Get current queue depth for a game mode.
    pub fn mm_queue_size(env: Env, mode: GameMode) -> u32 {
        matchmaking::queue_size(&env, mode)
    }

    /// Check if a player is currently in queue for a mode.
    pub fn mm_is_queued(env: Env, player: Address, mode: GameMode) -> bool {
        matchmaking::is_queued(&env, &player, mode)
    }

    /// Get a player's current queue entry (position + wait info).
    pub fn mm_get_status(env: Env, player: Address, mode: GameMode) -> Option<QueueEntry> {
        matchmaking::get_queue_status(&env, &player, mode)
    }

    // ─── ELO Rating Tiers — Issue #590 ───────────────────────────────────

    /// Get the full rating entry (tier, games played, season) for a player.
    pub fn get_rating_entry(env: Env, player: Address) -> RatingEntry {
        elo_rating::get_rating_entry(&env, &player)
    }

    /// Apply a match result: update ELO ratings with dynamic K-factor and
    /// emit tier promotion/demotion events.
    pub fn apply_match_result(env: Env, winner: Address, loser: Address) {
        elo_rating::apply_match_result(&env, &winner, &loser)
    }

    /// Apply seasonal soft-reset to a batch of players.
    pub fn apply_seasonal_reset(env: Env, players: Vec<Address>) {
        elo_rating::apply_seasonal_reset(&env, players)
    }

    /// Advance the season counter.
    pub fn advance_season(env: Env) -> u32 {
        elo_rating::advance_season(&env)
    }

    // ─── Ship Abilities — Issue #589 ──────────────────────────────────────

    /// Get the 5 abilities available for a ship type.
    pub fn get_ship_abilities(env: Env, ship_type: Symbol) -> Vec<Ability> {
        ship_abilities::get_abilities_for_type(&env, &ship_type)
    }

    /// Initialise ability states for a player entering combat.
    pub fn init_combat_abilities(env: Env, combat_id: u64, player: Address, ship_type: Symbol) {
        player.require_auth();
        ship_abilities::init_combat_abilities(&env, combat_id, &player, &ship_type)
    }

    /// Use a ship ability during combat.
    pub fn use_ability(
        env: Env,
        combat_id: u64,
        player: Address,
        ship_type: Symbol,
        ability_id: Symbol,
    ) -> Result<(), PvPError> {
        player.require_auth();
        ship_abilities::use_ability(&env, combat_id, &player, &ship_type, &ability_id)
    }

    /// Get current cooldown states for a player's abilities in a combat.
    pub fn get_ability_states(env: Env, combat_id: u64, player: Address) -> Vec<AbilityState> {
        ship_abilities::get_ability_states(&env, combat_id, &player)
    }

    /// Tick all ability cooldowns down by 1 (call at start of player's turn).
    pub fn tick_ability_cooldowns(env: Env, combat_id: u64, player: Address) {
        player.require_auth();
        ship_abilities::tick_cooldowns(&env, combat_id, &player)
    }

    // === Contract Versioning API ===

    pub fn initialize_version(env: Env) {
        contract_versioning::initialize_version(&env);
    }

    pub fn get_version(env: Env) -> u32 {
        contract_versioning::get_version(&env)
    }

    pub fn check_compatibility(env: Env, version: u32) {
        contract_versioning::check_compatibility(&env, version).unwrap();
    }

    pub fn set_auto_migrate(env: Env, caller: Address, enabled: bool) {
        contract_versioning::set_auto_migrate(&env, &caller, enabled);
    }

    pub fn migrate_data(
        env: Env,
        caller: Address,
        old_version: u32,
        new_version: u32,
        batch: Vec<Bytes>,
    ) -> MigrationRecord {
        contract_versioning::migrate_data(&env, &caller, old_version, new_version, batch).unwrap()
    }

    // === Role-Based Access Control (RBAC) API ===

    /// Initialize the RBAC system with default roles (admin, nomad, indexer).
    /// Must be called exactly once before any role-gated operations.
    pub fn init_rbac(env: Env, admin: Address) -> Result<(), AccessControlError> {
        access_control::init_roles(&env, admin)
    }

    /// Grant a role to a single address with optional expiry.
    pub fn grant_role(
        env: Env,
        caller: Address,
        role: Symbol,
        grantee: Address,
        expiry: Option<u32>,
    ) -> Result<(), AccessControlError> {
        access_control::grant_role(&env, caller, role, grantee, expiry)
    }

    /// Grant a role to up to 5 addresses in a single batch operation.
    pub fn grant_role_batch(
        env: Env,
        caller: Address,
        role: Symbol,
        grantees: Vec<Address>,
        expiry: Option<u32>,
    ) -> Result<(), AccessControlError> {
        access_control::grant_role_batch(&env, caller, role, grantees, expiry)
    }

    /// Revoke a role from an address.
    pub fn revoke_role(
        env: Env,
        caller: Address,
        role: Symbol,
        revokee: Address,
    ) -> Result<(), AccessControlError> {
        access_control::revoke_role(&env, caller, role, revokee)
    }

    /// Grant a permission to a role (allow role to perform action).
    pub fn grant_permission(
        env: Env,
        caller: Address,
        role: Symbol,
        action: Symbol,
    ) -> Result<(), AccessControlError> {
        access_control::grant_permission(&env, caller, role, action)
    }

    /// Revoke a permission from a role.
    pub fn revoke_permission(
        env: Env,
        caller: Address,
        role: Symbol,
        action: Symbol,
    ) -> Result<(), AccessControlError> {
        access_control::revoke_permission(&env, caller, role, action)
    }

    /// Transfer admin privileges to a new address.
    pub fn transfer_admin(
        env: Env,
        caller: Address,
        new_admin: Address,
    ) -> Result<(), AccessControlError> {
        access_control::transfer_admin(&env, caller, new_admin)
    }

    /// Check whether an address holds a specific role.
    pub fn has_role(env: Env, role: Symbol, address: Address) -> bool {
        access_control::has_role(&env, &role, &address)
    }

    /// Check whether a role is permitted to perform an action.
    pub fn has_permission(env: Env, role: Symbol, action: Symbol) -> bool {
        access_control::has_permission(&env, &role, &action)
    }

    // === Rate Limiting API ===

    /// Set the rate limit for an operation type (RBAC admin only).
