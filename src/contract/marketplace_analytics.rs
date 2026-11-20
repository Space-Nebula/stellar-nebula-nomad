    pub fn unstake_ship(env: Env, owner: Address, ship_id: u64) -> Result<i128, FarmError> {
        yield_farming::unstake_ship(&env, &owner, ship_id)
    }

    /// Read a ship stake.
    pub fn get_ship_stake(env: Env, ship_id: u64) -> Option<NftStake> {
        yield_farming::get_ship_stake(&env, ship_id)
    }

    /// Stake AMM LP units to earn a share of funded rewards with
    /// impermanent-loss protection.
    pub fn stake_lp(
        env: Env,
        provider: Address,
        pool_id: u64,
        amount: i128,
    ) -> Result<LpStake, FarmError> {
        yield_farming::stake_lp(&env, &provider, pool_id, amount)
    }

    /// Deposit rewards for a pool's LP stakers (80 % pro rata, 20 % to the
    /// impermanent-loss pot).
    pub fn fund_lp_rewards(
        env: Env,
        funder: Address,
        pool_id: u64,
        amount: u32,
    ) -> Result<(), FarmError> {
        yield_farming::fund_lp_rewards(&env, &funder, pool_id, amount)
    }

    /// Pay out pending LP rewards.
    pub fn claim_lp_rewards(env: Env, provider: Address, pool_id: u64) -> Result<i128, FarmError> {
        yield_farming::claim_lp_rewards(&env, &provider, pool_id)
    }

    /// Withdraw LP units; returns `(rewards, il_compensation)`.
    pub fn unstake_lp(
        env: Env,
        provider: Address,
        pool_id: u64,
        amount: i128,
    ) -> Result<(i128, i128), FarmError> {
        yield_farming::unstake_lp(&env, &provider, pool_id, amount)
    }

    /// Read an LP stake.
    pub fn get_lp_stake(env: Env, pool_id: u64, provider: Address) -> Option<LpStake> {
        yield_farming::get_lp_stake(&env, pool_id, &provider)
    }

    /// Read a pool's LP staking state.
    pub fn get_lp_pool_state(env: Env, pool_id: u64) -> Option<LpPoolState> {
        yield_farming::get_lp_pool_state(&env, pool_id)
    }

    /// Lock resources behind the caller's alliance for at least seven days.
    pub fn stake_to_guild(
        env: Env,
        member: Address,
        asset_id: Symbol,
        amount: u32,
    ) -> Result<GuildStake, StakingError> {
        staking::stake_to_guild(&env, &member, asset_id, amount)
    }

    /// Deposit rewards split pro rata across a guild's stakers.
    pub fn fund_guild_rewards(
        env: Env,
        funder: Address,
        alliance_id: u64,
        amount: u32,
    ) -> Result<(), StakingError> {
        staking::fund_guild_rewards(&env, &funder, alliance_id, amount)
    }

    /// Pay out the caller's share of guild rewards.
    pub fn claim_guild_rewards(env: Env, member: Address) -> Result<i128, StakingError> {
        staking::claim_guild_rewards(&env, &member)
    }

    /// Leave a guild stake after its lock; returns `(principal, rewards)`.
    pub fn unstake_from_guild(env: Env, member: Address) -> Result<(i128, i128), StakingError> {
        staking::unstake_from_guild(&env, &member)
    }

    /// Read a member's guild stake.
    pub fn get_guild_stake(env: Env, member: Address) -> Option<GuildStake> {
        staking::get_guild_stake(&env, &member)
    }

    /// Units staked into an alliance in total.
    pub fn get_guild_stake_total(env: Env, alliance_id: u64) -> i128 {
        staking::get_guild_stake_total(&env, alliance_id)
    }

    /// Initialise governance staking (admin, token, minimum stake, lock).
    pub fn init_staking(
        env: Env,
        admin: Address,
        token_address: Address,
        min_stake: i128,
        lock_duration_ledgers: u32,
    ) -> Result<(), StakingError> {
        staking::initialize(env, admin, token_address, min_stake, lock_duration_ledgers)
    }

    /// Stake for voting power under the default lock.
    pub fn stake_for_voting(env: Env, staker: Address, amount: i128) -> Result<(), StakingError> {
        staking::stake(env, staker, amount)
    }

    /// Stake for voting power under a named tier.
    pub fn stake_with_tier(
        env: Env,
        staker: Address,
        amount: i128,
        tier: StakingTier,
    ) -> Result<(), StakingError> {
        staking::stake_with_tier(env, staker, amount, tier)
    }

    /// Withdraw a governance stake after its lock.
    pub fn unstake_voting(env: Env, staker: Address) -> Result<i128, StakingError> {
        staking::unstake(env, staker)
    }

    /// Voting power of an address (0 while delegated or within the first
    /// ledger of the stake).
    pub fn get_voting_power(env: Env, address: Address) -> i128 {
        staking::get_voting_power(env, address)
    }

    /// Read a governance stake.
    pub fn get_stake_v2(env: Env, address: Address) -> Option<StakeRecord> {
        staking::get_stake_v2(env, address)
    }

    /// Slash part of a governance stake for malicious voting. Staking-admin
    /// only.
    pub fn slash_stake(
        env: Env,
        admin: Address,
        staker: Address,
        slash_bps: u32,
    ) -> Result<i128, StakingError> {
        staking::slash_stake(&env, &admin, &staker, slash_bps)
    }

    /// Lifetime units slashed from governance stakes.
    pub fn get_total_slashed(env: Env) -> i128 {
        staking::get_total_slashed(&env)
    }

    /// Aggregate governance staking statistics.
    pub fn get_global_staking_stats(env: Env) -> GlobalStakingStats {
        staking::get_global_staking_stats(&env)
    }

    // ─── Weather System (Issues #553, #630) ──────────────────────────────

    /// Current weather in a region.
    pub fn get_region_weather(env: Env, region_id: u64) -> weather_system::WeatherState {
        weather_system::get_current_weather(&env, region_id)
    }

    /// 24-hour forecast for a region.
    pub fn get_weather_forecast(env: Env, region_id: u64) -> Vec<weather_system::WeatherForecast> {
        weather_system::generate_forecast(&env, region_id)
    }

    /// Effective weather modifiers in a region right now.
    pub fn get_weather_effects(env: Env, region_id: u64) -> weather_system::WeatherImpact {
        weather_system::get_weather_impact(&env, region_id)
    }

    /// Set a region's climate weights (admin only).
    pub fn set_region_climate(
        env: Env,
        admin: Address,
        region_id: u64,
        weights: Vec<u32>,
    ) -> Result<(), weather_system::WeatherError> {
        weather_system::set_region_climate(&env, &admin, region_id, weights)
    }

    // ─── Player Housing (Issues #532, #629) ──────────────────────────────

    /// Create a player's house (Small).
    pub fn initialize_house(env: Env, owner: Address) -> Result<HouseInstance, HousingError> {
        player_housing::initialize_house(&env, owner)
    }

    /// House, layout, displays, banner and storage in one read.
    pub fn get_house_showcase(env: Env, owner: Address) -> Option<player_housing::HouseShowcase> {
        player_housing::get_house_showcase(&env, &owner)
    }

    /// Set wall, floor and lighting themes.
    pub fn customize_house(
        env: Env,
        owner: Address,
        wall_theme: u32,
        floor_theme: u32,
        lighting_theme: u32,
    ) -> Result<(), HousingError> {
        player_housing::customize_theme(&env, owner, wall_theme, floor_theme, lighting_theme)
    }

    /// Buy furniture into storage.
    pub fn buy_house_furniture(
        env: Env,
        owner: Address,
        furniture_id: u64,
    ) -> Result<(), HousingError> {
        player_housing::purchase_furniture(&env, owner, furniture_id)
    }

    /// Place owned furniture.
    pub fn place_house_furniture(
        env: Env,
        owner: Address,
        furniture_id: u64,
        x: u32,
        y: u32,
        rotation: u32,
        layer: u32,
    ) -> Result<(), HousingError> {
        player_housing::place_furniture(&env, owner, furniture_id, x, y, rotation, layer)
    }

    /// Move placed furniture back into storage.
    pub fn remove_house_furniture(
        env: Env,
        owner: Address,
        furniture_id: u64,
    ) -> Result<(), HousingError> {
        player_housing::remove_furniture(&env, owner, furniture_id)
    }

    /// Buy the next storage expansion.
    pub fn expand_house_storage(
        env: Env,
        owner: Address,
    ) -> Result<player_housing::StorageSummary, HousingError> {
        player_housing::expand_storage(&env, owner)
    }

    /// Show an unlocked achievement in a display case slot.
    pub fn display_house_achievement(
        env: Env,
        owner: Address,
        achievement_id: u64,
        slot: u32,
    ) -> Result<Vec<player_housing::AchievementDisplay>, HousingError> {
        player_housing::display_achievement(&env, owner, achievement_id, slot)
    }

    /// Hang the owner's guild banner.
    pub fn hang_guild_banner(
        env: Env,
        owner: Address,
    ) -> Result<player_housing::GuildBanner, HousingError> {
        player_housing::display_guild_banner(&env, owner)
    }

    /// Visit another player's house.
    pub fn visit_house(
        env: Env,
        visitor: Address,
        house_owner: Address,
    ) -> Result<(), HousingError> {
        player_housing::visit_house(&env, visitor, house_owner)
    }

    /// Rate a visited house (100-500).
    pub fn rate_house(
        env: Env,
        visitor: Address,
        house_owner: Address,
        rating: u32,
    ) -> Result<(), HousingError> {
        player_housing::rate_house(&env, visitor, house_owner, rating)
    }

    /// Add a guest to a friends-only house.
    pub fn add_house_guest(env: Env, owner: Address, guest: Address) -> Result<(), HousingError> {
        player_housing::add_guest(&env, owner, guest)
    }

    // ─── Battle Pass and Challenges (Issue #642) ─────────────────────────────

    /// Admin: seed the default free + premium reward tracks for a season.
    pub fn init_battle_pass_rewards(
        env: Env,
        admin: Address,
        season_id: u64,
    ) -> Result<(), battle_pass::BattlePassError> {
        battle_pass::init_default_season_rewards(&env, admin, season_id)
    }

    /// Read a player's battle pass progress for the active season.
    pub fn get_battle_pass_state(
        env: Env,
        profile_id: u64,
    ) -> Result<battle_pass::BattlePassState, battle_pass::BattlePassError> {
        battle_pass::get_battle_pass_state(&env, profile_id)
    }

    /// Award battle pass XP for scans and essence collected.
    pub fn add_battle_pass_xp(
        env: Env,
        profile_id: u64,
        scans: u32,
        essence: i128,
    ) -> Result<u64, battle_pass::BattlePassError> {
        battle_pass::add_xp(&env, profile_id, scans, essence)
    }

    /// Unlock the premium track for the active season.
    pub fn unlock_battle_pass_premium(
        env: Env,
        player: Address,
        profile_id: u64,
    ) -> Result<(), battle_pass::BattlePassError> {
        battle_pass::unlock_premium_pass(&env, player, profile_id)
    }

    /// Claim a free or premium reward at a tier.
    pub fn claim_battle_pass_reward(
        env: Env,
        player: Address,
        profile_id: u64,
        tier: u32,
        premium: bool,
    ) -> Result<i128, battle_pass::BattlePassError> {
        let track = if premium {
            battle_pass::PassTier::Premium
        } else {
            battle_pass::PassTier::Free
        };
        battle_pass::claim_reward_v2(&env, player, profile_id, tier, track)
    }

    /// Claim a completed challenge and credit its XP to the battle pass.
    pub fn claim_battle_pass_challenge(
        env: Env,
        player: Address,
        profile_id: u64,
        challenge_id: u64,
    ) -> Result<u64, battle_pass::BattlePassError> {
        battle_pass::claim_challenge_reward(&env, &player, profile_id, challenge_id)
    }

    /// Admin: create a daily, weekly, or seasonal challenge.
    pub fn create_bp_challenge(
        env: Env,
        admin: Address,
        frequency: challenge_system::ChallengeFrequency,
        name: String,
        description: String,
        target_value: u64,
        xp_reward: u64,
        duration_hours: u64,
    ) -> Result<u64, challenge_system::ChallengeError> {
        challenge_system::create_challenge(
            &env,
            &admin,
            frequency,
            name,
            description,
            target_value,
            xp_reward,
            duration_hours,
        )
    }

    /// Record progress on a challenge.
    pub fn update_bp_challenge_progress(
        env: Env,
        player: Address,
        profile_id: u64,
        challenge_id: u64,
        increment: u64,
    ) -> Result<challenge_system::ChallengeProgress, challenge_system::ChallengeError> {
        challenge_system::update_progress_for(
            &env,
            &player,
            profile_id,
            challenge_id,
            increment,
        )
    }

    /// List every challenge currently inside its active window.
    pub fn get_active_bp_challenges(
        env: Env,
        profile_id: u64,
    ) -> Vec<challenge_system::ChallengeWithProgress> {
        challenge_system::get_active_challenges(&env, profile_id)
    }

    /// List the challenges created for one frequency's current period.
    pub fn get_current_bp_challenges(
        env: Env,
        frequency: challenge_system::ChallengeFrequency,
    ) -> Vec<challenge_system::Challenge> {
        challenge_system::get_current_period_challenges(&env, frequency)
    }

    // ─── Skin Marketplace (Issue #643) ───────────────────────────────────────

    /// List an owned skin for sale at or above its rarity floor.
    pub fn list_skin_for_sale(
        env: Env,
        seller: Address,
        skin_id: u64,
        price: i128,
        duration_hours: u64,
    ) -> Result<u64, skin_marketplace::MarketplaceError> {
        skin_marketplace::list_skin(&env, &seller, skin_id, price, duration_hours)
    }

    /// Buy a listed skin.
    pub fn buy_skin_listing(
        env: Env,
        buyer: Address,
        listing_id: u64,
    ) -> Result<(), skin_marketplace::MarketplaceError> {
        skin_marketplace::buy_skin(&env, &buyer, listing_id)
    }

    /// Cancel an active skin listing.
    pub fn cancel_skin_listing(
        env: Env,
        seller: Address,
        listing_id: u64,
    ) -> Result<(), skin_marketplace::MarketplaceError> {
        skin_marketplace::cancel_listing(&env, &seller, listing_id)
    }

    /// Change a skin listing's asking price.
    pub fn update_skin_listing_price(
        env: Env,
        seller: Address,
        listing_id: u64,
        new_price: i128,
    ) -> Result<(), skin_marketplace::MarketplaceError> {
        skin_marketplace::update_listing_price(&env, &seller, listing_id, new_price)
    }

    /// Read a skin listing by ID.
    pub fn get_skin_listing(
        env: Env,
        listing_id: u64,
    ) -> Option<skin_marketplace::SkinListing> {
        skin_marketplace::get_listing(&env, listing_id)
    }

    /// Read the active listing for a skin.
    pub fn get_active_skin_listing(
        env: Env,
        skin_id: u64,
    ) -> Option<skin_marketplace::SkinListing> {
        skin_marketplace::get_active_listing(&env, skin_id)
    }

    /// List all listings created by a seller.
    pub fn get_seller_skin_listings(
        env: Env,
        seller: Address,
    ) -> Vec<skin_marketplace::SkinListing> {
        skin_marketplace::get_seller_listings(&env, &seller)
    }

    /// Search active skin listings by maximum price.
    pub fn search_skin_listings(
        env: Env,
        max_price: i128,
    ) -> Vec<skin_marketplace::SkinListing> {
        skin_marketplace::search_by_price(&env, max_price)
    }

    /// Search active skin listings by price range and rarity.
    pub fn search_skin_listings_by_rarity(
        env: Env,
        min_price: i128,
        max_price: i128,
        rarity: SkinRarity,
    ) -> Vec<skin_marketplace::SkinListing> {
        skin_marketplace::search_skins(&env, min_price, max_price, rarity)
    }

    /// Render a listing's skin preview.
    pub fn get_skin_listing_preview(
        env: Env,
        listing_id: u64,
    ) -> Option<skins::SkinPreview> {
        skin_marketplace::get_listing_preview(&env, listing_id)
    }

    /// Aggregate skin marketplace statistics.
    pub fn get_skin_marketplace_stats(
        env: Env,
    ) -> skin_marketplace::MarketplaceStats {
        skin_marketplace::get_marketplace_stats(&env)
    }

    /// Offer one skin for another player's skin.
    pub fn create_skin_trade_offer(
        env: Env,
        proposer: Address,
        offered_skin_id: u64,
        requested_skin_id: u64,
        duration_hours: u64,
    ) -> Result<u64, skin_marketplace::MarketplaceError> {
        skin_marketplace::create_trade_offer(
            &env,
            &proposer,
            offered_skin_id,
            requested_skin_id,
            duration_hours,
        )
    }

    /// Accept a pending skin-for-skin trade.
    pub fn accept_skin_trade_offer(
        env: Env,
        accepter: Address,
        offer_id: u64,
    ) -> Result<(), skin_marketplace::MarketplaceError> {
        skin_marketplace::accept_trade_offer(&env, &accepter, offer_id)
    }

    /// Cancel a pending skin trade offer.
    pub fn cancel_skin_trade_offer(
        env: Env,
        proposer: Address,
        offer_id: u64,
    ) -> Result<(), skin_marketplace::MarketplaceError> {
        skin_marketplace::cancel_trade_offer(&env, &proposer, offer_id)
    }

    /// Read a skin barter offer by ID.
    pub fn get_skin_trade_offer(
        env: Env,
        offer_id: u64,
    ) -> Option<skin_marketplace::SkinTradeOffer> {
        skin_marketplace::get_trade_offer(&env, offer_id)
    }

    // ─── NFT Marketplace (Issue #645) ────────────────────────────────────────

    /// Create a fixed-price NFT listing.
    pub fn create_nft_listing(
        env: Env,
        seller: Address,
        asset: nft_marketplace::Asset,
        price: i128,
        expires_at: Option<u64>,
    ) -> Result<u64, nft_marketplace::MarketplaceError> {
        nft_marketplace::create_listing(&env, &seller, asset, price, expires_at)
    }

    /// Buy a fixed-price NFT listing.
    pub fn buy_nft_listing(
        env: Env,
        buyer: Address,
        listing_id: u64,
    ) -> Result<(), nft_marketplace::MarketplaceError> {
        nft_marketplace::buy_listing(&env, &buyer, listing_id)
    }

    /// Cancel a fixed-price NFT listing.
    pub fn cancel_nft_listing(
        env: Env,
        seller: Address,
        listing_id: u64,
    ) -> Result<(), nft_marketplace::MarketplaceError> {
        nft_marketplace::cancel_listing(&env, &seller, listing_id)
    }

    /// Change a fixed-price NFT listing's asking price.
    pub fn update_nft_listing_price(
        env: Env,
        seller: Address,
        listing_id: u64,
        new_price: i128,
    ) -> Result<(), nft_marketplace::MarketplaceError> {
        nft_marketplace::update_listing_price(&env, &seller, listing_id, new_price)
    }

    /// Read an NFT listing by ID.
    pub fn get_nft_listing(
        env: Env,
        listing_id: u64,
    ) -> Option<nft_marketplace::FixedPriceListing> {
        nft_marketplace::get_listing(&env, listing_id)
    }

    /// List a seller's NFT listings.
    pub fn get_seller_nft_listings(
        env: Env,
        seller: Address,
    ) -> Vec<nft_marketplace::FixedPriceListing> {
        nft_marketplace::get_seller_listings(&env, &seller)
    }

    /// Sweep an expired NFT listing and release its escrow.
    pub fn expire_nft_listing(
        env: Env,
        listing_id: u64,
    ) -> Result<(), nft_marketplace::MarketplaceError> {
        nft_marketplace::expire_listing(&env, listing_id)
    }

    /// Create a ship NFT listing under the correct category.
    pub fn create_ship_listing(
        env: Env,
        seller: Address,
        ship_id: u64,
        rarity: nft_marketplace::Rarity,
        name: String,
        price: i128,
        expires_at: Option<u64>,
    ) -> Result<u64, nft_marketplace::MarketplaceError> {
        nft_marketplace::create_ship_listing(
            &env, &seller, ship_id, rarity, name, price, expires_at,
        )
    }

    /// Create a skin NFT listing under the correct category.
    pub fn create_skin_nft_listing(
        env: Env,
        seller: Address,
        skin_id: u64,
        rarity: nft_marketplace::Rarity,
        name: String,
        price: i128,
        expires_at: Option<u64>,
    ) -> Result<u64, nft_marketplace::MarketplaceError> {
        nft_marketplace::create_skin_listing(
            &env, &seller, skin_id, rarity, name, price, expires_at,
        )
    }

    /// Create a badge NFT listing under the correct category.
    pub fn create_badge_listing(
        env: Env,
        seller: Address,
        badge_id: u64,
        rarity: nft_marketplace::Rarity,
        name: String,
        price: i128,
        expires_at: Option<u64>,
    ) -> Result<u64, nft_marketplace::MarketplaceError> {
        nft_marketplace::create_badge_listing(
            &env, &seller, badge_id, rarity, name, price, expires_at,
        )
    }

    // ─── Typed Content Creation and Marketplace Browsing (Issue #644) ────────

    /// Create a community nebula definition.
    pub fn create_nebula(
        env: Env,
        creator: Address,
        name: String,
        description: String,
        data: Bytes,
        is_public: bool,
        tags: Vec<Symbol>,
    ) -> Result<u64, ContentToolsError> {
        content_tools::create_nebula(&env, &creator, name, description, data, is_public, tags)
    }

    /// Create a community mission definition.
    pub fn create_mission(
        env: Env,
        creator: Address,
        name: String,
        description: String,
        data: Bytes,
        is_public: bool,
        tags: Vec<Symbol>,
    ) -> Result<u64, ContentToolsError> {
        content_tools::create_mission(&env, &creator, name, description, data, is_public, tags)
    }

    /// Create a community event definition.
    pub fn create_event(
        env: Env,
        creator: Address,
        name: String,
        description: String,
        data: Bytes,
        is_public: bool,
        tags: Vec<Symbol>,
    ) -> Result<u64, ContentToolsError> {
        content_tools::create_event(&env, &creator, name, description, data, is_public, tags)
    }

    /// Browse every active content marketplace listing.
    pub fn get_active_content_listings(env: Env) -> Vec<MarketplaceListing> {
        content_tools::get_active_marketplace_listings(&env)
    }

    /// Search active content listings by maximum price.
    pub fn search_content_marketplace(
        env: Env,
        max_price: i128,
    ) -> Vec<MarketplaceListing> {
        content_tools::search_marketplace(&env, max_price)

    // ─── Revenue Attribution and Analytics (Issue #656) ───────────────────────

    /// Record a revenue event attributed to an acquisition channel.
    pub fn record_revenue(
        env: Env,
        channel: Symbol,
        amount: u64,
    ) -> Result<(), revenue_attribution::AttributionError> {
        revenue_attribution::record_revenue(&env, channel, amount)
    }

    /// Record marketing or acquisition spend for a channel.
    pub fn record_attribution_spend(env: Env, channel: Symbol, amount: u64) {
        revenue_attribution::record_spend(&env, channel, amount)
    }

    /// Record a user touchpoint with an acquisition channel.
    pub fn record_user_touchpoint(env: Env, user: Address, channel: Symbol) {
        user.require_auth();
        revenue_attribution::record_user_touchpoint(&env, user, channel)
    }

    /// Record a funnel step for a marketing channel.
    pub fn record_funnel_step(env: Env, channel: Symbol, step: revenue_attribution::FunnelStep) {
        revenue_attribution::record_funnel_step(&env, channel, step)
    }

    /// Record a conversion event and distribute attribution weights.
    pub fn record_conversion(
        env: Env,
        user: Address,
        amount: u64,
        source: revenue_attribution::RevenueSource,
        model: revenue_attribution::AttributionModel,
    ) -> Result<Vec<revenue_attribution::TouchpointWeight>, revenue_attribution::AttributionError> {
        user.require_auth();
        revenue_attribution::record_conversion(&env, user, amount, source, model)
    }

    /// Get attribution summary for a channel.
    pub fn get_channel_attribution(
        env: Env,
        channel: Symbol,
    ) -> revenue_attribution::ChannelAttribution {
        revenue_attribution::get_channel_attribution(&env, channel)
    }

    /// Get all channel attribution summaries.
    pub fn get_all_channel_attributions(
        env: Env,
    ) -> Vec<revenue_attribution::ChannelAttribution> {
        revenue_attribution::all_channel_attributions(&env)
    }

    /// Get full financial analysis and unit economics for a channel.
    pub fn get_channel_financials(
        env: Env,
        channel: Symbol,
    ) -> revenue_attribution::ChannelFinancialAnalysis {
        revenue_attribution::get_channel_financials(&env, channel)
    }

    /// Get macro-economic impact report.
    pub fn get_economic_impact(env: Env) -> revenue_attribution::EconomicImpactReport {
        revenue_attribution::get_economic_impact(&env)
    }

    /// Get consolidated revenue analytics dashboard snapshot.
    pub fn get_revenue_dashboard(
        env: Env,
    ) -> revenue_attribution::RevenueAnalyticsDashboard {
        revenue_attribution::get_revenue_dashboard(&env)
    }
