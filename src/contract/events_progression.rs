    pub fn schedule_event(
        env: Env,
        admin: Address,
        event_type: Symbol,
        start_time: u64,
        reward_pool: i128,
    ) -> Result<u64, EventError> {
        event_scheduler::schedule_event(&env, &admin, event_type, start_time, reward_pool)
    }

    /// Trigger a scheduled event when its time arrives.
    pub fn trigger_scheduled_event(env: Env, event_id: u64) -> Result<EventResult, EventError> {
        event_scheduler::trigger_scheduled_event(&env, event_id)
    }

    /// Get event details by ID.
    pub fn get_event(env: Env, event_id: u64) -> Result<ScheduledEvent, EventError> {
        event_scheduler::get_event(&env, event_id)
    }

    /// Get all active event IDs.
    pub fn get_active_events(env: Env) -> Vec<u64> {
        event_scheduler::get_active_events(&env)
    }

    /// Schedule a weekly nebula festival (template).
    pub fn schedule_weekly_festival(
        env: Env,
        admin: Address,
        reward_pool: i128,
    ) -> Result<u64, EventError> {
        event_scheduler::schedule_weekly_festival(&env, &admin, reward_pool)
    }

    /// Cancel a scheduled event (admin only).
    pub fn cancel_event(env: Env, admin: Address, event_id: u64) -> Result<(), EventError> {
        event_scheduler::cancel_event(&env, &admin, event_id)
    }

    /// Update event participant count.
    pub fn update_event_participants(
        env: Env,
        event_id: u64,
        participant_count: u32,
    ) -> Result<(), EventError> {
        event_scheduler::update_participants(&env, event_id, participant_count)
    }

    /// Get total number of events scheduled.
    pub fn get_event_count(env: Env) -> u64 {
        event_scheduler::get_event_count(&env)
    }

    /// Reset event burst counter.
    pub fn reset_event_burst_counter(env: Env) {
        event_scheduler::reset_burst_counter(&env)
    }

    // ─── Seasons ──────────────────────────────────────────────────────────

    /// Admin: start the first season.
    pub fn init_season(env: Env, admin: Address, title: String) -> Result<u64, SeasonError> {
        seasons::initialize_season(&env, &admin, title)
    }

    /// Get the current season.
    pub fn get_current_season(env: Env) -> Result<Season, SeasonError> {
        seasons::get_current_season(&env)
    }

    // ─── Time-Limited Seasonal Events ─────────────────────────────────────

    /// Admin: schedule a time-limited seasonal event in the current season.
    pub fn schedule_seasonal_event(
        env: Env,
        admin: Address,
        config: SeasonalEventConfig,
    ) -> Result<u64, EventError> {
        event_scheduler::schedule_seasonal_event(&env, &admin, config)
    }

    /// Activate a seasonal event once its window opens.
    pub fn activate_seasonal_event(env: Env, event_id: u64) -> Result<SeasonalEvent, EventError> {
        event_scheduler::activate_seasonal_event(&env, event_id)
    }

    /// End a seasonal event once its window closes and start its cooldown.
    pub fn end_seasonal_event(env: Env, event_id: u64) -> Result<SeasonalEvent, EventError> {
        event_scheduler::end_seasonal_event(&env, event_id)
    }

    /// Admin: cancel a scheduled or active seasonal event.
    pub fn cancel_seasonal_event(
        env: Env,
        admin: Address,
        event_id: u64,
    ) -> Result<(), EventError> {
        event_scheduler::cancel_seasonal_event(&env, &admin, event_id)
    }

    /// Admin: attach an event-specific challenge to a seasonal event.
    pub fn add_seasonal_event_challenge(
        env: Env,
        admin: Address,
        event_id: u64,
        spec: SeasonalChallengeSpec,
    ) -> Result<u64, EventError> {
        event_scheduler::add_seasonal_event_challenge(&env, &admin, event_id, spec)
    }

    /// Record points a player earned during a live seasonal event.
    pub fn record_seasonal_event_points(
        env: Env,
        event_id: u64,
        profile_id: u64,
        points: u64,
    ) -> Result<u64, EventError> {
        event_scheduler::record_seasonal_event_points(&env, event_id, profile_id, points)
    }

    /// Claim a player's share of an ended seasonal event's reward pool.
    pub fn claim_seasonal_event_reward(
        env: Env,
        player: Address,
        event_id: u64,
        profile_id: u64,
    ) -> Result<i128, EventError> {
        event_scheduler::claim_seasonal_event_reward(&env, &player, event_id, profile_id)
    }

    /// Get a seasonal event by ID.
    pub fn get_seasonal_event(env: Env, event_id: u64) -> Result<SeasonalEvent, EventError> {
        event_scheduler::get_seasonal_event(&env, event_id)
    }

    /// IDs of seasonal events that are scheduled or active.
    pub fn get_pending_seasonal_events(env: Env) -> Vec<u64> {
        event_scheduler::get_pending_seasonal_events(&env)
    }

    /// A player's participation in a seasonal event.
    pub fn get_seasonal_event_entry(
        env: Env,
        event_id: u64,
        profile_id: u64,
    ) -> Option<SeasonalEventEntry> {
        event_scheduler::get_seasonal_event_entry(&env, event_id, profile_id)
    }

    /// Timestamp before which no new event of `category` may start.
    pub fn get_event_category_cooldown(env: Env, category: Symbol) -> u64 {
        event_scheduler::get_category_cooldown(&env, category)
    }

    /// Record progress on a time-limited (or event-specific) challenge.
    pub fn record_challenge_progress(
        env: Env,
        profile_id: u64,
        challenge_id: u64,
        metric_delta: u64,
    ) -> Result<u64, EventError> {
        event_scheduler::record_challenge_progress(&env, profile_id, challenge_id, metric_delta)
    }

    // ─── Ship Customization & Skins ───────────────────────────────────────

    pub fn mint_skin(
        env: Env,
        owner: Address,
        name: Symbol,
        rarity: SkinRarity,
        color_primary: u32,
        color_secondary: u32,
        metadata: Bytes,
    ) -> Result<ShipSkin, SkinError> {
        ship_customization::mint_skin(
            &env,
            &owner,
            name,
            rarity,
            color_primary,
            color_secondary,
            metadata,
        )
    }

    pub fn apply_skin(
        env: Env,
        owner: Address,
        ship_id: u64,
        skin_id: u64,
    ) -> Result<(), SkinError> {
        ship_customization::apply_skin(&env, &owner, ship_id, skin_id)
    }

    pub fn get_ship_skin(env: Env, ship_id: u64) -> Option<u64> {
        ship_customization::get_ship_skin(&env, ship_id)
    }

    pub fn get_owner_skins(env: Env, owner: Address) -> Vec<u64> {
        ship_customization::get_owner_skins(&env, &owner)
    }

    pub fn transfer_skin(
        env: Env,
        skin_id: u64,
        new_owner: Address,
    ) -> Result<ShipSkin, SkinError> {
        ship_customization::transfer_skin(&env, skin_id, &new_owner)
    }

    pub fn get_skin_templates(env: Env) -> Vec<SkinTemplate> {
        skins::get_skin_templates(&env)
    }

    // ─── Economic Monitoring & Balancing ──────────────────────────────────

    pub fn initialize_economic_monitor(env: Env, admin: Address) {
        economics::monitor::initialize_monitor(&env, &admin)
    }

    pub fn update_supply_metrics(
        env: Env,
        admin: Address,
        total_supply: i128,
        circulating_supply: i128,
        staked_supply: i128,
    ) {
        economics::monitor::update_supply_metrics(
            &env,
            &admin,
            total_supply,
            circulating_supply,
            staked_supply,
        )
    }

    pub fn track_resource_activity(
        env: Env,
        resource_type: Symbol,
        minted: i128,
        burned: i128,
        avg_price: i128,
    ) {
        economics::monitor::track_resource_activity(&env, resource_type, minted, burned, avg_price)
    }

    pub fn get_economic_metrics(env: Env) -> EconomicMetrics {
        economics::monitor::get_metrics(&env)
    }

    pub fn get_resource_metrics(env: Env, resource_type: Symbol) -> ResourceMetrics {
        economics::monitor::get_resource_metrics(&env, resource_type)
    }

    pub fn calculate_inflation_rate(env: Env, old_supply: i128, new_supply: i128) -> u32 {
        economics::monitor::calculate_inflation_rate(&env, old_supply, new_supply)
    }

    pub fn detect_economic_imbalance(
        env: Env,
        resource_type: Symbol,
        supply: i128,
        demand: i128,
    ) -> SupplyDemandRatio {
        economics::balancer::detect_imbalance(&env, resource_type, supply, demand)
    }

    pub fn suggest_balance_adjustment(
        env: Env,
        resource_type: Symbol,
    ) -> Option<BalanceAdjustment> {
        economics::balancer::suggest_adjustment(&env, resource_type)
    }

    pub fn apply_balance_adjustment(
        env: Env,
        admin: Address,
        parameter: Symbol,
        new_value: i128,
        reason: Symbol,
    ) {
        economics::balancer::apply_adjustment(&env, &admin, parameter, new_value, reason)
    }

    pub fn generate_economic_report(env: Env) -> (i128, i128, i128) {
        economics::balancer::generate_report(&env)
    }

    // ── Anti-Whale Economy Mechanics (Issue #455) ──────────────────────

    pub fn process_anti_whale_action(
        env: Env,
        user: Address,
        amount: u64,
    ) -> Result<(u64, u64), AntiWhaleError> {
        user.require_auth();
        economics::anti_whale::process_anti_whale_action(&env, &user, amount)
    }

    /// Register the anti-whale admin (one-time). Only the admin can retune
    /// limits, set the legacy cap or manage exemptions.
    pub fn init_anti_whale_admin(env: Env, admin: Address) -> Result<(), AntiWhaleError> {
        economics::anti_whale::init_admin(&env, &admin)
    }

    /// Set the legacy daily volume cap. Admin only.
    pub fn set_anti_whale_cap(env: Env, admin: Address, cap: u64) -> Result<(), AntiWhaleError> {
        economics::anti_whale::set_daily_cap(&env, &admin, cap)
    }

    pub fn get_anti_whale_cap(env: Env) -> u64 {
        economics::anti_whale::get_daily_cap(&env)
    }

    /// Exempt (or un-exempt) an account from every anti-whale mechanism.
    /// Admin only.
    pub fn set_anti_whale_exempt(
        env: Env,
        admin: Address,
        user: Address,
        exempt: bool,
    ) -> Result<(), AntiWhaleError> {
        economics::anti_whale::set_exempt(&env, &admin, &user, exempt)
    }

    pub fn is_anti_whale_exempt(env: Env, user: Address) -> bool {
        economics::anti_whale::is_exempt(&env, &user)
    }

    pub fn get_user_daily_volume(env: Env, user: Address) -> u64 {
        economics::anti_whale::get_user_daily_volume(&env, &user)
    }

    /// Replace the admin-set anti-whale limits. Admin only; governance
    /// parameters (`aw_*`) still override individual fields.
    pub fn set_anti_whale_config(
        env: Env,
        admin: Address,
        config: AntiWhaleConfig,
    ) -> Result<(), AntiWhaleError> {
        economics::anti_whale::set_config(&env, &admin, config)
    }

    /// The anti-whale limits currently in force (admin config plus
    /// governance overrides).
    pub fn get_anti_whale_config(env: Env) -> AntiWhaleConfig {
        economics::anti_whale::effective_config(&env)
    }

    /// Units gathered by `user` today before diminishing returns.
    pub fn get_user_daily_gathered(env: Env, user: Address) -> u64 {
        economics::anti_whale::get_user_daily_gathered(&env, &user)
    }

    /// Operations of `kind` performed by `user` today.
    pub fn get_user_daily_ops(env: Env, user: Address, kind: OpKind) -> u32 {
        economics::anti_whale::get_user_daily_ops(&env, &user, kind)
    }

    /// Trade volume of `user` today.
    pub fn get_user_daily_trade_volume(env: Env, user: Address) -> u64 {
        economics::anti_whale::get_user_daily_trade_volume(&env, &user)
    }

    /// Guild treasury contributions of `user` today.
    pub fn get_daily_guild_contribution(env: Env, user: Address) -> i128 {
        economics::anti_whale::get_user_daily_guild_contribution(&env, &user)
    }

    /// Lifetime activity points of `user`.
    pub fn get_player_activity(env: Env, user: Address) -> u64 {
        economics::anti_whale::get_player_activity(&env, &user)
    }

    /// Current power tier of `user` (casual / dedicated / hardcore).
    pub fn get_player_tier(env: Env, user: Address) -> PlayerTier {
        economics::anti_whale::get_player_tier(&env, &user)
    }

    /// Top `limit` rows of the leaderboard for one player tier.
    pub fn get_tier_leaderboard(env: Env, tier: PlayerTier, limit: u32) -> Vec<TierEntry> {
        economics::anti_whale::get_tier_leaderboard(&env, tier, limit)
    }

    /// Lifetime counters of how often each anti-whale mechanism fired.
    pub fn get_anti_whale_impact(env: Env) -> ImpactStats {
        economics::anti_whale::get_impact_stats(&env)
    }

    // ─── Trading System ───────────────────────────────────────────────────

    pub fn place_limit_order(
        env: Env,
        trader: Address,
        order: LimitOrder,
    ) -> Result<u64, TradingError> {
        trading::place_limit_order(&env, &trader, order)
    }

    pub fn cancel_limit_order(
        env: Env,
        trader: Address,
        order_id: u64,
    ) -> Result<(), TradingError> {
        trading::cancel_limit_order(&env, &trader, order_id)
    }

    pub fn get_limit_order(env: Env, order_id: u64) -> Option<LimitOrder> {
        trading::get_limit_order(&env, order_id)
    }

    pub fn get_trader_orders(env: Env, trader: Address) -> Vec<LimitOrder> {
        trading::get_trader_orders(&env, &trader)
    }

    pub fn record_trade(env: Env, caller: Address, trade: TradeRecord) -> Result<(), TradingError> {
        trading::record_trade(&env, &caller, trade)
    }

    pub fn get_trading_history(env: Env) -> Vec<TradeRecord> {
        trading::get_trading_history(&env)
    }

    // ─── AMM — Automated Market Maker (Issue #189) ───────────────────────────

    /// Create a new liquidity pool.
    pub fn create_pool(
        env: Env,
        creator: Address,
        resource_a: Symbol,
        resource_b: Symbol,
    ) -> Result<u64, AmmError> {
        trading::create_pool(&env, &creator, resource_a, resource_b)
    }

    /// Add liquidity to a pool and receive LP tokens.
    pub fn add_liquidity(
        env: Env,
        provider: Address,
        pool_id: u64,
        amount_a: i128,
        amount_b: i128,
    ) -> Result<(i128, LiquidityPool), AmmError> {
        trading::add_liquidity(&env, &provider, pool_id, amount_a, amount_b)
    }

    /// Remove liquidity by burning LP tokens.
    pub fn remove_liquidity(
        env: Env,
        provider: Address,
        pool_id: u64,
        lp_amount: i128,
    ) -> Result<(i128, i128), AmmError> {
        trading::remove_liquidity(&env, &provider, pool_id, lp_amount)
    }

    /// Swap exact input for output via a pool route.
    pub fn swap_exact_input(
        env: Env,
        trader: Address,
        resource_in: Symbol,
        amount_in: i128,
        min_amount_out: i128,
        route: Vec<u64>,
    ) -> Result<i128, AmmError> {
        trading::swap_exact_input(&env, &trader, resource_in, amount_in, min_amount_out, route)
    }

    /// Get pool details by ID.
    pub fn get_pool(env: Env, pool_id: u64) -> Option<LiquidityPool> {
        trading::get_pool(&env, pool_id)
    }

    /// Get LP balance for a provider in a pool.
    pub fn get_lp_balance(env: Env, pool_id: u64, provider: Address) -> i128 {
        trading::get_lp_balance(&env, pool_id, &provider)
    }

    /// Get all pool IDs.
    pub fn get_all_pools(env: Env) -> Vec<u64> {
        trading::get_all_pools(&env)
    }

    /// Quote the output amount for a swap without executing.
    pub fn quote_swap(
        env: Env,
        pool_id: u64,
        resource_in: Symbol,
        amount_in: i128,
    ) -> Result<i128, AmmError> {
        trading::quote_swap(&env, pool_id, resource_in, amount_in)
    }

    // ── Mobile interface (Issue #199) ─────────────────────────────────────────

    /// Return a compact dashboard summary for the given player.
    pub fn get_mobile_dashboard(env: Env, player: Address) -> MobileDashboard {
        mobile_views::get_mobile_dashboard(&env, &player)
    }

    /// Return a lightweight scan-result estimate for a specific ship.
    pub fn get_quick_scan_preview(
        env: Env,
        ship_id: u64,
    ) -> Result<QuickScanPreview, MobileViewError> {
        mobile_views::get_quick_scan_preview(&env, ship_id)
    }

    /// Batch dashboard + primary-ship scan preview into a single RPC call.
    pub fn batch_get_mobile_info(env: Env, player: Address) -> MobileBatchInfo {
        mobile_views::batch_get_mobile_info(&env, &player)
    }

    /// Emit a mobile-event subscription marker for off-chain indexers.
    pub fn subscribe_mobile_events(env: Env, player: Address) {
        player.require_auth();
        mobile_views::subscribe_mobile_events(&env, &player);
    }

    // ── Reputation System (Issue #261) ────────────────────────────────────────

    pub fn initialize_reputation_system(env: Env, admin: Address) -> Result<(), ReputationError> {
        reputation::initialize_reputation(&env, &admin)
    }

    pub fn create_player_reputation(env: Env, player: Address) -> Result<(), ReputationError> {
        player.require_auth();
        reputation::create_player_reputation(&env, &player)
    }

    pub fn get_player_reputation_score(env: Env, player: Address) -> Result<u32, ReputationError> {
        reputation::get_reputation_score(&env, &player)
    }

    pub fn get_player_reputation_details(
        env: Env,
        player: Address,
    ) -> Result<ReputationScore, ReputationError> {
        reputation::get_reputation_details(&env, &player)
    }

    pub fn record_player_behavior(
        env: Env,
        player: Address,
        behavior_type: BehaviorType,
        description: String,
        points: i32,
        reporter: Address,
    ) -> Result<(), ReputationError> {
        reputation::record_behavior(&env, &player, behavior_type, description, points, reporter)
    }

    pub fn submit_dispute_report(
        env: Env,
        reporter: Address,
        accused: Address,
        reason: String,
        evidence: String,
    ) -> Result<u64, ReputationError> {
        reputation::submit_report(&env, &reporter, &accused, reason, evidence)
    }

    pub fn resolve_dispute_report(
        env: Env,
        admin: Address,
        report_id: u64,
        resolved: bool,
    ) -> Result<(), ReputationError> {
        reputation::resolve_report(&env, &admin, report_id, resolved)
    }

    pub fn ban_player_account(
        env: Env,
        admin: Address,
        player: Address,
    ) -> Result<(), ReputationError> {
        reputation::ban_player(&env, &admin, &player)
    }

    pub fn check_player_ban_status(env: Env, player: Address) -> bool {
        reputation::is_player_banned(&env, &player)
    }

    pub fn get_player_behavior_history(env: Env, player: Address) -> Vec<BehaviorRecord> {
        reputation::get_player_history(&env, &player)
    }

    pub fn get_player_report_count(env: Env, player: Address) -> u32 {
        reputation::get_player_report_count(&env, &player)
    }

    pub fn get_all_dispute_reports(env: Env) -> Vec<DisputeReport> {
        reputation::get_all_reports(&env)
    }

    pub fn claim_reputation_reward(env: Env, player: Address) -> Result<i128, ReputationError> {
        reputation::claim_reputation_reward(&env, &player)
    }

    // ─── Ship level ladder (progression rebalance) ────────────────────────

    /// Progression level of a ship (0 = never levelled).
    pub fn get_ship_level(env: Env, ship_id: u64) -> u32 {
        ship_upgrade::get_ship_level(&env, ship_id)
    }

    /// Active level cost curve (the rebalanced default unless overridden).
    pub fn get_progression_curve(env: Env) -> ProgressionCurve {
        ship_upgrade::get_progression_curve(&env)
    }

    /// Override the level cost curve. Upgrade-admin only.
    pub fn set_progression_curve(
        env: Env,
        admin: Address,
        curve: ProgressionCurve,
    ) -> Result<(), ShipUpgradeError> {
        ship_upgrade::set_progression_curve(&env, &admin, curve)
    }

    /// Price of a ship's next level. Pure view.
    pub fn level_upgrade_cost(env: Env, ship_id: u64) -> Result<u64, ShipUpgradeError> {
        ship_upgrade::level_upgrade_cost(&env, ship_id)
    }

    /// Price of reaching `level` under the active curve.
    pub fn level_cost_at(env: Env, level: u32) -> u64 {
        ship_upgrade::level_cost_at(&env, level)
    }

    /// Days-to-level for casual, regular and hardcore play under the active
    /// curve.
    pub fn simulate_progression(env: Env) -> ProgressionReport {
        ship_upgrade::simulate_progression(&env)
    }

    /// Cumulative resource units burned by level upgrades.
    pub fn get_total_level_spend(env: Env) -> u64 {
        ship_upgrade::get_total_level_spend(&env)
    }

    /// Take a ship to its next level, burning the level cost in `asset_id`.
    pub fn upgrade_ship_level(
        env: Env,
        player: Address,
        ship_id: u64,
        asset_id: Symbol,
    ) -> Result<LevelUpgradeResult, ShipUpgradeError> {
        ship_upgrade::upgrade_ship_level(&env, &player, ship_id, asset_id)
    }

    // ─── Staking: resources, ships, liquidity, guilds, governance ─────────

    /// Lock `amount` of `asset_id` for a 7 / 30 / 90-day tier. Returns the
    /// stake id.
    pub fn stake_resource(
        env: Env,
        owner: Address,
        asset_id: Symbol,
        amount: u32,
        tier: LockTier,
    ) -> Result<u64, FarmError> {
        yield_farming::stake_resource(&env, &owner, asset_id, amount, tier)
    }

    /// Pay out yield accrued on a resource stake.
    pub fn claim_resource_yield(
        env: Env,
        owner: Address,
        stake_id: u64,
    ) -> Result<i128, FarmError> {
        yield_farming::claim_resource_yield(&env, &owner, stake_id)
    }

    /// Return principal plus unclaimed yield after the lock.
    pub fn unstake_resource(env: Env, owner: Address, stake_id: u64) -> Result<i128, FarmError> {
        yield_farming::unstake_resource(&env, &owner, stake_id)
    }

    /// Leave a resource stake early: 50 % of principal and all yield are
    /// forfeited to the yield reserve.
    pub fn emergency_unstake_resource(
        env: Env,
        owner: Address,
        stake_id: u64,
    ) -> Result<EmergencyWithdrawal, FarmError> {
        yield_farming::emergency_unstake_resource(&env, &owner, stake_id)
    }

    /// Read a resource stake.
    pub fn get_resource_stake(env: Env, stake_id: u64) -> Option<ResourceStake> {
        yield_farming::get_resource_stake(&env, stake_id)
    }

    /// Move `amount` of `asset_id` from the caller into the yield reserve.
    pub fn fund_yield_reserve(
        env: Env,
        funder: Address,
        asset_id: Symbol,
        amount: u32,
    ) -> Result<i128, FarmError> {
        yield_farming::fund_yield_reserve(&env, &funder, asset_id, amount)
    }

    /// Units of `asset_id` available to pay yield.
    pub fn get_yield_reserve(env: Env, asset_id: Symbol) -> i128 {
        yield_farming::get_yield_reserve(&env, asset_id)
    }

    /// Units of `asset_id` locked in resource stakes.
    pub fn get_staking_tvl(env: Env, asset_id: Symbol) -> i128 {
        yield_farming::get_tvl(&env, asset_id)
    }

    /// Locked value across every staking product.
    pub fn get_tvl_report(env: Env) -> TvlReport {
        yield_farming::get_tvl_report(&env)
    }

    /// Set the per-asset TVL above which APYs are dampened. Staking-admin
    /// only.
    pub fn set_tvl_target(env: Env, admin: Address, target: i128) -> Result<(), FarmError> {
        yield_farming::require_staking_admin(&env, &admin)?;
        yield_farming::set_tvl_target(&env, target);
        Ok(())
    }

    /// Set the asset staked ships pay out in. Staking-admin only.
    pub fn set_nft_yield_asset(
        env: Env,
        admin: Address,
        asset_id: Symbol,
    ) -> Result<(), FarmError> {
        yield_farming::require_staking_admin(&env, &admin)?;
        yield_farming::set_nft_yield_asset(&env, asset_id);
        Ok(())
    }

    /// Asset staked ships pay out in.
    pub fn get_nft_yield_asset(env: Env) -> Symbol {
        yield_farming::get_nft_yield_asset(&env)
    }

    /// Stake a ship; it cannot be transferred until unstaked.
    pub fn stake_ship(env: Env, owner: Address, ship_id: u64) -> Result<NftStake, FarmError> {
        yield_farming::stake_ship(&env, &owner, ship_id)
    }

    /// Pay out yield a staked ship has earned.
    pub fn claim_ship_yield(env: Env, owner: Address, ship_id: u64) -> Result<i128, FarmError> {
        yield_farming::claim_ship_yield(&env, &owner, ship_id)
    }

    /// Unstake a ship, paying unclaimed yield.
