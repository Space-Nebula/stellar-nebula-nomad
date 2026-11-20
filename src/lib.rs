//! Stellar Nebula Nomad — a decentralized space-exploration simulation on Soroban.
//!
//! The crate is a suite of contract modules covering procedural nebula
//! generation, NFT ships, resource economics, guilds, governance, and the
//! operational safety rails (rate limiting, emergency controls, reentrancy
//! guards) around them. Modules are re-exported flatly from the crate root so
//! callers can name types without walking the module tree; see
//! `docs/API_REFERENCE.md` for the curated public surface.

#![no_std]
// Soroban SDK 28 deprecates `events().publish` (in favour of `#[contractevent]`
// types) and `Env::register_contract` (in favour of `register`). Migrating the
// hundreds of emit sites is tracked separately, so keep the deprecation noise
// out of `clippy -- -D warnings` until that migration lands.
#![allow(deprecated)]
#![allow(warnings)]
// Soroban's contract macros emit hundreds of undocumented public helpers, so a
// crate-wide allow is required for `RUSTDOCFLAGS="-D missing_docs" cargo doc`
// to succeed without deleting public API. Hand-written API docs live in docs/.
#![allow(missing_docs)]

// Unit tests (proptest in particular) need std's `format!`/`vec!` macros.
#[cfg(test)]
#[macro_use]
extern crate std;

use soroban_sdk::{
    contract, contractimpl, symbol_short, Address, Bytes, BytesN, Env, String, Symbol, Vec,
};

// `nebula_explorer` is a private module, so re-export the whole scan surface to
// give downstream callers (and integration tests) a nameable path.
pub use crate::input_validation::{
    checked_region_offset, validate_region_id, validate_seed, ValidationError, MAX_REGION_ID,
};
pub use crate::nebula_explorer::{
    validate_scan_inputs, CellType, NebulaCell, NebulaLayout, Rarity, GRID_SIZE, TOTAL_CELLS,
};

pub mod access_control;
mod analytics;
mod blueprint_factory;
pub mod constants;
mod content_tools;
pub mod error_standard;
/// Overflow-safe arithmetic primitives shared by the economic modules.
pub mod safe_math;
pub mod traits;

mod elo_rating;
mod gifting_system;
mod leaderboards;
mod matchmaking;
mod nebula_explorer;
mod onboarding_tutorial;
mod player_profile;
mod player_segmentation;
mod pvp_combat;
mod referral_system;
/// Rate-limited resource minting and the player balance ledger.
pub mod resource_minter;
mod session_manager;
mod ship_abilities;
mod ship_nft;
mod ship_registry;

mod achievement_engine;
// `achievements` (catalog + progress + leaderboard) and `badges` (NFT badge
// minting/transfer) are the user-facing layer over `achievement_engine`.
// They were present on disk but never declared here, so the whole achievement
// system was dead code; player housing's achievement display cases need them.
mod achievements;
mod badges;
mod batch_processor;
mod data_exporter;
mod dex_integration;
mod difficulty_curve;
mod difficulty_scaler;
pub use difficulty_scaler::{DifficultyError, DifficultyResult, RarityWeights};
pub mod dynamic_pricing;
pub mod emergency_controls;
pub mod event_framework;
#[cfg(test)]
mod event_gas_tests;
/// Shared on-chain event emission helpers and severity levels.
pub mod events;
mod health_monitor;
mod metadata_resolver;
/// Procedural nebula layout generation with TTL-based lifecycle management.
pub mod nebula_gen;
mod randomness_oracle;
/// Per-address rate limiting for sensitive contract operations.
pub mod rate_limiter;
pub mod ship_repair;
/// Ship module upgrade costs and application of upgrade purchases.
pub mod ship_upgrade;
#[cfg(any(test, feature = "fuzz"))]
pub mod test_helpers;
mod treasure_vault;
// Issue #293: staking is exercised by `tests/economy/test_staking.rs`.
pub mod staking;

/// DAO proposal creation, voting, and execution flows.
pub mod governance;
mod indexer_callbacks;
mod theme_customizer;
mod yield_farming;

mod bounty_board;
mod contract_versioning;
mod gas_recovery;
mod recycling_crafter;

mod anomaly_classifier;
mod audit_logger;
mod energy_manager;
mod environment_simulator;
mod escrow_trader;
mod exploration_heatmap;
mod fractional_resources;
mod fraud_detection;
mod mission_generator;
mod revenue_attribution;
mod shared_lib;
mod smart_alerts;
mod sustainability_metrics;
mod yield_forecast;

mod gas_sponsor;

pub mod cache_ttl_manager;
pub mod cache_layers;
mod metrics_exporter;
mod migration_framework;
mod state_snapshot;
mod storage_optim;

pub mod alliance_manager;
mod audio_seed_generator;
pub mod clan_wars;
mod constellation_mapper;
mod entanglement_comms;
/// Scheduled in-game event registration and lifecycle management.
pub mod event_scheduler;
mod market_oracle;
mod navigation_planner;
mod portal_registry;
/// Commitment-based privacy-preserving player statistics.
pub mod privacy_stats;
mod prize_distributor;
mod wormhole_traveler;

pub mod battle_pass;
pub mod nft_marketplace;
mod rewards;
/// Seasonal content windows, leaderboards, and reward tracks.
pub mod seasons;
pub mod trading;

/// Pure bracket / pairing algorithms behind the tournament formats (Issue #587).
pub mod bracket_generator;
pub mod crafting;
/// Crafting recipe registry and recipe validation.
pub mod recipes;
mod ship_customization;
pub mod skins;
/// Tournaments: four formats, registration, scheduling, forfeits, prizes,
/// admin controls and dispute resolution (Issues #284, #537, #587).
pub mod tournament;
/// Player notification dispatch: threshold alerts and push delivery.
pub mod notifications {
    /// Threshold checks that raise player notifications (low resources, etc.).
    pub mod alerts;
    /// Notification records and delivery emission to player inboxes.
    pub mod push_service;
}

/// Compact aggregated state views served to mobile clients.
pub mod mobile_views;

/// Adapters for external Stellar services (Horizon, Stellar.expert).
#[path = "../integrations/mod.rs"]
pub mod integrations;

/// Economic modeling: pricing, sinks, progression curves, anti-whale guards.
pub mod economics;

// Gas optimization modules
mod gas_optimized_compute;
mod gas_optimized_storage;

// Cross-contract safety: reentrancy guard + composable call helpers.
mod ai_mission_engine;
mod composability_examples;
/// Cooperative guild quest tracking and reward distribution.
/// Alliance treasury governance and guild quest reward payouts.
pub mod guild_economy;
pub mod guild_quests;
pub mod input_validation;
mod nomad_bonding;
mod player_housing;
mod quest_system;
pub mod reentrancy_guard;
mod reputation;
mod validation_macros;

pub use reentrancy_guard::ReentrancyError;
pub use reputation::{
    BehaviorRecord, BehaviorType, DisputeReport, ReportStatus, ReputationError, ReputationScore,
};

/// RBAC error type returned by the `init_rbac` / `grant_role` / `check_permission`
/// family of contract entry points (Issue #586).
pub use access_control::AccessControlError;
pub use analytics::{AnalyticsError, GlobalStats};
pub use blueprint_factory::{Blueprint, BlueprintError, BlueprintRarity};
pub use content_tools::{
    ContentMetadata, ContentToolsError, CreatedContent, MarketplaceListing, PurchaseResult,
    RevenueSplitConfig, VoteResult, CONTENT_TYPE_EVENT, CONTENT_TYPE_MISSION, CONTENT_TYPE_NEBULA,
    DEFAULT_CREATOR_SHARE_BPS, DEFAULT_PLATFORM_SHARE_BPS, MAX_CONTENT_PER_CREATOR,
    MAX_CONTENT_PRICE, MAX_MARKETPLACE_LISTINGS,
};
// `LeaderboardEntry` names three types in this crate. The crate root exports
// `leaderboards::LeaderboardEntry` (player score rankings). The analytics
// snapshot type is `analytics::LeaderboardEntry`, and the referral type is
// `rewards::LeaderboardEntry`. Both are reached by module path and are not
// re-exported here. See the doc comment on `leaderboards::LeaderboardEntry`.
pub use leaderboards::{
    AchievementEntry, GuildEntry, LeaderboardEntry, LeaderboardEntry as EnhancedLeaderboardEntry,
    LeaderboardError, LeaderboardRewards, PageMeta, RegionalEntry, CATEGORY_ACHIEVEMENTS,
    CATEGORY_BOUNTIES, CATEGORY_CRAFTS, CATEGORY_ESSENCE, CATEGORY_GUILD_CONTRIBUTION,
    CATEGORY_MISSIONS, CATEGORY_NEBULAE_EXPLORED, CATEGORY_PVP_RATING, CATEGORY_PVP_WINS,
    CATEGORY_SCANS, CATEGORY_SHIPS_MINTED, CATEGORY_TRADES, DEFAULT_PAGE_SIZE,
    MAX_LEADERBOARD_ENTRIES, MAX_PAGE_SIZE, PERIOD_ALL_TIME, PERIOD_DAILY, PERIOD_MONTHLY,
    PERIOD_WEEKLY,
};
pub use nebula_gen::{
    Anomaly, AnomalyType, NebulaError as NebulaGenError, NebulaGenLayout, ResourceClass,
};
pub use onboarding_tutorial::{
    OnboardingError, PlayerProfile as OnboardingPlayerProfile, TutorialProgress, STEP_REWARDS,
    TOTAL_STEPS,
};
pub use player_profile::{
    PlayerProfile, ProfileError, ProfileSection, ProfileSectionData, ProgressUpdate,
};
pub use player_segmentation::{
    PlayerEngagementMetrics, PlayerSegment, SegmentMetrics, SegmentationError,
};
pub use pvp_combat::{
    Challenge, CombatHistory, CombatMove, CombatState, CombatStats, EloDecayConfig,
    MatchmakingEntry, PvPError, RewardsConfig, SpectatorInfo, COMBAT_TIMEOUT,
    ELO_DECAY_BASE_POINTS, ELO_DECAY_FLOOR, ELO_DECAY_INACTIVITY_SECS, INITIAL_ELO, K_FACTOR,
    MAX_ENERGY, MAX_HP, MAX_QUEUE_SIZE, MAX_SPECTATORS,
};
// Issue #590 — ELO rating system with 8 tiers, K-factor, seasonal adjustments.
pub use elo_rating::{
    advance_season, apply_match_result, apply_seasonal_reset, calculate_updated_ratings,
    check_demotion, check_promotion, get_games_played, get_rating_entry, get_season_config,
    increment_games_played, k_factor_for, tier_for_rating, tier_symbol, EloKey, RankTier,
    RatingEntry, SeasonConfig, DEMOTION_BUFFER, K_FACTOR_HIGH, K_FACTOR_MID, K_FACTOR_NEW,
    SEASONAL_DECAY_BPS, SEASONAL_FLOOR, TIER_BRONZE_MIN, TIER_CHALLENGER_MIN, TIER_DIAMOND_MIN,
    TIER_GOLD_MIN, TIER_GRANDMASTER_MIN, TIER_MASTER_MIN, TIER_PLATINUM_MIN, TIER_SILVER_MIN,
};
// Issue #591 — MMR-based matchmaking with expanding range and abuse prevention.
pub use matchmaking::{
    dequeue, enqueue, get_queue_status, is_queued, process_queue, queue_size, GameMode,
    MatchResult as MatchmakingResult, MatchmakingKey, QueueEntry, EXPAND_INTERVAL_SECS,
    MAX_QUEUE_WAIT_SECS, MAX_TEAM_SIZE, MMR_EXPAND_STEP, MMR_INITIAL_RANGE, REQUEUE_COOLDOWN_SECS,
};
// Issue #589 — Ship ability system with 5 abilities per ship type.
pub use referral_system::{Referral, ReferralError};
pub use resource_minter::{
    balance_of, circulating_supply, credit_balance, debit_balance, move_balance, reduce_supply,
    resource_type_to_symbol, total_minted, AssetId, MinterError, MinterKey, ResourceKey,
    ResourceMinterContract, ResourceRecord, ResourceType,
};
pub use session_manager::{Session, SessionError};
pub use ship_abilities::{
    get_abilities_for_type, get_ability_states, init_combat_abilities, is_ability_ready,
    tick_cooldowns, use_ability, Ability, AbilityEffect, AbilityKey, AbilityState,
    ShipAbilityLoadout, ABILITY_MAX_ENERGY, MAX_ABILITIES_PER_SHIP,
};
pub use ship_nft::{DataKey as ShipDataKey, ShipError, ShipNft};
pub use ship_registry::Ship;

pub use batch_processor::{
    clear_batch, execute_batch, get_player_batch, queue_batch_operation, BatchError, BatchOp,
    BatchOpType, BatchResult, MAX_BATCH_SIZE,
};
pub use bounty_board::{
    approve_bounty, cancel_bounty, claim_bounty, dispute_bounty, get_bounty,
    initialize_bounty_board, post_bounty, set_bounty_expiry, Bounty, BountyError,
    DEFAULT_BOUNTY_EXPIRY, MAX_ACTIVE_BOUNTIES,
};
pub use contract_versioning::{
    check_compatibility, get_migration_record, get_version, initialize_version,
    is_auto_migrate_enabled, migrate_data, set_auto_migrate, MigrationRecord, VersioningError,
    CURRENT_VERSION, MIGRATION_BATCH_SIZE,
};
pub use dex_integration::{
    buy_offer, cancel_listing, get_open_offers, harvest_and_list, list_at_market, list_resource,
    sell_to_order, DexFill, DynamicListError, OfferPage, MAX_OFFER_PAGE, MAX_OFFER_SCAN,
};
pub use dynamic_pricing::{
    deviation_bps, dynamic_price, ema_step, get_price_state, get_pricing_config,
    get_pricing_history, get_sma, get_volatility_bps, init_pricing_config, listing_price,
    observe_oracle_price, observe_price, observe_supply_demand, price_from_state,
    set_pricing_config, DynamicPrice, PricePoint, PriceState, PricingConfig, PricingError,
    DEFAULT_BASE_VOLATILITY_BAND_BPS,
    DEFAULT_MAX_DEVIATION_BPS, DEFAULT_MIN_COOLDOWN_SECS, DEFAULT_SMOOTHING_BPS,
    DEFAULT_SUPPLY_DEMAND_ADJ_BPS, MAX_HISTORY_ENTRIES, MAX_SMOOTHING_BPS, MAX_VOLATILITY_BAND_BPS,
};
pub use emergency_controls::{
    emergency_withdraw, execute_unpause, get_admins, initialize_admins, is_paused, pause_contract,
    require_not_paused, schedule_unpause, EmergencyError, UNPAUSE_DELAY,
};
pub use gas_recovery::{
    get_refund_request, initialize_refund, process_refund_batch, request_refund,
    set_refund_percentage, verify_refund_eligibility, RefundError, RefundRequest,
    DEFAULT_REFUND_BPS, REFUND_BATCH_SIZE,
};
pub use gifting_system::{Gift, GiftError};
pub use metadata_resolver::{
    batch_resolve_metadata, get_current_gateway, resolve_metadata, set_gateway, set_metadata_uri,
    MetadataError, TokenMetadata, MAX_METADATA_BATCH,
};
pub use randomness_oracle::{
    get_entropy_pool, request_random_seed, verify_and_fallback, OracleError,
};
pub use recycling_crafter::{
    craft_new_item, get_recipe, initialize_recycling, recycle_resource, CraftingResult, Recipe,
    RecyclingError, RECYCLE_CRAFT_BATCH_SIZE,
};
pub use ship_upgrade::{
    apply_regen_upgrade, get_total_upgrade_spend, get_upgrade_economy, quote_upgrade_cost,
    scaled_upgrade_cost, set_upgrade_economy, ShipState, ShipUpgradeError, UpgradeBlueprint,
    UpgradeEconomy, DEFAULT_GROWTH_BPS, DEFAULT_MAX_COST,
};
pub use treasure_vault::{
    claim_treasure, claim_vault_loot, deposit_treasure, discover_vault, get_key_balance,
    get_pity_counter, get_vault, get_vault_discovery, select_loot_tier, LootRarity, LootReward,
    LootTier, SpecialReward, TreasureVault, VaultDiscovery, VaultError,
    DEFAULT_MIN_LOCK_DURATION, LOOT_TABLE, PITY_THRESHOLD,
};

pub use anomaly_classifier::{
    classify_anomaly, classify_batch, get_classification, refine_classification, AnomalyError,
    ClassificationRecord,
};
pub use audit_logger::{
    get_audit_count, get_audit_retention, get_retained_audit_count, log_audit_event,
    oldest_audit_id, prune_audit_logs, query_audit_logs, set_audit_retention, AuditEntry,
    AuditLoggerError, RetentionPolicy, DEFAULT_AUDIT_RETENTION_SECS, DEFAULT_MAX_AUDIT_ENTRIES,
    MAX_QUERY_LIMIT,
};
pub use energy_manager::{
    apply_passive_regen, consume_energy, get_energy_balance, get_regen_rate, recharge_energy,
    set_regen_rate, EnergyBalance, EnergyError, PassiveRegenResult, RechargeResult,
};
pub use environment_simulator::{
    apply_environmental_modifier, get_nebula_condition, simulate_conditions, EnvironmentCondition,
    EnvironmentError, ModifierResult,
};
pub use escrow_trader::{
    cancel_escrow, complete_escrow, confirm_escrow, get_escrow, initiate_escrow, Escrow,
    EscrowError, EscrowResult, TradeAsset,
};
pub use gas_sponsor::{
    claim_sponsorship_fund, get_admin, get_config, get_daily_count, get_fund_balance,
    get_remaining_daily_slots, has_been_sponsored, initialize as initialize_sponsorship,
    mark_profile_verified, sponsor_first_scan, update_config, SponsorConfig, SponsorError,
    MAX_DAILY_SPONSORSHIPS,
};
pub use mission_generator::{
    complete_mission, generate_daily_mission, get_player_missions, update_mission_progress,
    Mission, MissionError, MissionReward,
};
pub use shared_lib::{calculate_yield, validate_address, SharedError};
pub use sustainability_metrics::{
    claim_sustainability_reward, get_footprint, record_transaction_footprint, FootprintRecord,
    SustainabilityError,
};

pub use fractional_resources::{
    buy_listed_share, cancel_share_listing, fractionalize_resource, get_last_share_price,
    get_original_resource, get_owner_shares, get_share, get_share_listing, get_total_shares,
    initialize as initialize_fractional, is_share_owner, list_share_for_sale, merge_fractions,
    transfer_share, ShareListing,
    update_config as update_fractional_config, FractionalConfig, FractionalError, FractionalShare,
    OriginalResource, MAX_FRACTIONS_PER_TX, MIN_SHARE_SIZE,
};
pub use yield_forecast::{
    batch_generate_forecasts, clear_stale_forecasts, generate_yield_forecast, get_cached_forecast,
    analyze_yield_risk, get_history_count, get_model_params, get_model_version,
    get_player_history, initialize as initialize_forecast, update_forecast_model,
    update_model_params, ForecastError, ModelParams, YieldDataPoint, YieldForecast,
    YieldRiskLevel, YieldRiskReport, MAX_FORECAST_BURST, MAX_FORECAST_DAYS,
    MAX_HISTORY_POINTS,
};

pub use alliance_manager::{
    contribute_to_treasury, found_alliance, get_alliance, get_alliance_treasury,
    get_member_contribution, get_player_alliance, join_alliance, leave_alliance, Alliance,
    AllianceError, MembershipRecord, MAX_MEMBERS_PER_ALLIANCE,
};
pub use guild_economy::{
    claim_quest_reward_share, create_treasury_proposal, execute_proposal, get_guild_tax_rate,
    get_proposal, get_quest_reward_payout, pay_guild_tribute, record_quest_rewards,
    set_guild_tax_rate, split_guild_tribute, vote_on_proposal, GuildEconomyError,
    GuildEconomyKey, QuestRewardPayout, QuestRewardSplit, TreasuryProposal, TributeReceipt,
    MAX_GUILD_TAX_BPS,
};
pub use guild_quests::{
    apply_reward_bps, assign_guild_quest, calculate_reward_split, clear_guild_quest_assignment,
    contribute_quest_progress, create_guild_quest, create_guild_quest_chain,
    create_guild_quest_with_config, get_active_guild_quests, get_guild_quest,
    get_guild_quest_assignment, get_guild_quest_board, get_guild_quest_chain,
    get_guild_quest_contributors, get_guild_quest_meta, is_chain_unlocked, rotate_guild_quests,
    rotation_expires_at, rotation_period, rotation_period_secs, tier_reward_bps, GuildQuest,
    GuildQuestChain, GuildQuestConfig, GuildQuestError, GuildQuestKey, GuildQuestMeta,
    GuildQuestStatus, GuildQuestSummary, QuestAssignment, QuestFrequency,
    GUILD_QUEST_BPS_DENOMINATOR, GUILD_QUEST_CHAIN_TERMINUS, GUILD_QUEST_DAY_SECS,
    GUILD_QUEST_DEFAULT_TREASURY_BPS, GUILD_QUEST_LEGACY_TREASURY_BPS,
    GUILD_QUEST_MAX_CHAIN_LENGTH, GUILD_QUEST_SEASON_SECS, GUILD_QUEST_WEEK_SECS,
};
pub use audio_seed_generator::{
    generate_music_seed, get_all_layers, get_instrument_layer, get_nebula_seed, get_preset,
    initialize_presets, AudioError, InstrumentParams, MusicSeed, INSTRUMENT_PRESETS,
    MAX_LAYERS_PER_NEBULA,
};
pub use constellation_mapper::{
    get_constellation, get_constellation_count, match_constellation, match_constellations_batch,
    record_constellation, Constellation, ConstellationError, MatchResult as ConstellationMatchResult,
    MAX_MATCH_BURST, MIN_STARS,
};
pub use entanglement_comms::{
    create_entanglement_pair, dissolve_pair, get_entanglement_pair, get_message_count,
    send_entangled_message, send_entangled_message_batch, EntangledMessage, EntanglementError,
    EntanglementPair, MAX_MESSAGE_BURST, PAIR_LIFETIME_SECS,
};
pub use event_framework::{EventFrameworkError, StandardEvent};
pub use event_scheduler::{
    cancel_event, get_active_events, get_event, get_event_count, initialize_scheduler,
    reset_burst_counter as reset_event_burst, schedule_event, schedule_weekly_festival,
    trigger_scheduled_event, update_participants, EventError, EventResult, ScheduledEvent,
    SeasonalChallengeSpec, SeasonalEvent, SeasonalEventConfig, SeasonalEventEntry,
    SeasonalEventStatus, MAX_ACTIVE_EVENTS, MAX_CHALLENGES_PER_EVENT, MAX_PENDING_SEASONAL_EVENTS,
    MAX_SEASONAL_EVENT_COOLDOWN, MAX_SEASONAL_EVENT_DURATION, MIN_SEASONAL_EVENT_DURATION,
    SEASONAL_REWARD_CLAIM_WINDOW, WEEKLY_FESTIVAL_INTERVAL,
};
pub use market_oracle::{
    add_oracle_source, aggregate_resource_price, batch_update_prices, compute_consensus,
    get_current_market_rate, get_oracle_sources, get_price_data, get_price_history,
    get_source_price, initialize_oracle, remove_oracle_source, submit_source_price,
    update_resource_price, validate_price, OracleError as MarketOracleError, PriceData,
    SourcePrice, MAX_BATCH_UPDATE, MAX_OBSERVATION_AGE_SECS, MAX_ORACLE_SOURCES,
    MAX_PRICE_JUMP_BPS, MAX_SOURCE_DEVIATION_BPS, MIN_ORACLE_SOURCES,
};
pub use navigation_planner::{
    add_nebula_connection, add_nebula_connections_batch, calculate_optimal_route, get_connection,
    get_neighbors, initialize_nav_graph, validate_route_safety, NavConfig, NavError, NavPath,
    RouteEdge, MAX_CONNECTIONS_PER_BATCH, MAX_ROUTE_HOPS,
};
pub use portal_registry::{
    get_portal, initialize_portal_registry, query_portal_status, refresh_portal, register_portal,
    register_portal_batch, travel_through_portal, Portal, PortalError, BASE_TRAVEL_COST,
    MAX_PORTALS_PER_TX, MIN_STABLE_PCT,
};
pub use privacy_stats::{
    batch_commit_stats, commit_private_stat, get_commitment, get_commitment_count, is_opted_in,
    opt_in_privacy, reset_burst_counter as reset_privacy_burst, verify_private_stat, PrivacyError,
    StatCommitment, MAX_COMMITMENTS_PER_TX,
};
pub use prize_distributor::{
    distribute_weekly_prizes, fund_prize_pool, get_last_reset, get_prize_pool,
    get_total_distributed, initialize_prize_distributor, submit_leaderboard_snapshot, PrizeError,
    PrizeRecord, MAX_PAYOUT_POSITIONS, WEEK_SECONDS,
};
pub use seasons::{
    seasonal_event_reward_pool, Season, SeasonError, SeasonTheme, EXCLUSIVE_EVENT_BONUS_BPS,
};
pub use state_snapshot::{
    auto_snapshot, get_ship_snapshots, get_snapshot, reset_session_count, restore_from_snapshot,
    take_snapshot, RestoreResult, SnapshotError, StateSnapshot, AUTO_SNAPSHOT_INTERVAL,
    MAX_SNAPSHOTS_PER_SESSION, SNAPSHOT_TTL,
};
pub use storage_optim::{
    batch_store_with_bump, bloom_insert, bloom_may_contain, get_bump_config, get_optimized_entries,
    get_optimized_entry, get_ship_nebula, get_ship_nebula_batch, get_upgrade_target,
    guard_reentrancy, initialize_bump_config, pack_u32x3, pack_u64x2, prune_expired_data,
    release_guard, reset_burst_counter, set_upgrade_target, store_ship_nebula, store_with_bump,
    unpack_u32x3, unpack_u64x2, update_bump_config, BumpConfig, CachedEntry, OptimResult,
    OptimizedEntry, PruneReport, ShipNebulaData, StorageError, StorageTier, DEFAULT_BUMP_TTL,
    MAX_BUMP_TTL, MAX_BURST_READS, MAX_PRUNE_NAMESPACES,
};
pub use wormhole_traveler::{
    calculate_travel_cost, cleanup_expired_wormholes, get_active_wormholes, get_travel_history,
    get_wormhole, open_wormhole, traverse_wormhole, verify_wormhole_link, TravelRecord, Wormhole,
    WormholeError, MAX_SIMULTANEOUS_WORMHOLES, WORMHOLE_LIFETIME_SECS,
};

pub use ship_customization::{
    apply_skin, get_owner_skins, get_ship_skin, mint_skin, transfer_skin, ShipSkin, SkinError,
    SkinRarity,
};
pub use skins::{get_skin_templates, SkinTemplate};

pub use economics::anti_whale::{
    apply_gathering, calculate_diminishing_returns, calculate_progressive_fee,
    charge_progressive_fee, check_guild_contribution, check_operation, diminishing_returns,
    effective_config as get_anti_whale_config, get_daily_cap, get_day_index, get_impact_stats,
    get_player_activity, get_player_tier, get_tier_leaderboard, get_user_daily_volume, is_exempt,
    process_anti_whale_action, progressive_trade_fee, record_tier_score, tier_for_activity,
    AntiWhaleConfig, AntiWhaleError, AntiWhaleKey, ImpactStats, OpKind, PlayerTier, TierEntry,
    DAILY_WINDOW_SECONDS, DEFAULT_DAILY_CAP, DEFAULT_GUILD_DAILY_CAP, DEFAULT_MAX_MINTS_PER_DAY,
    DEFAULT_MAX_SCANS_PER_DAY, DEFAULT_MAX_TRADES_PER_DAY, DEFAULT_TIER_WIDTH,
    PROGRESSIVE_FEE_BANDS, PROGRESSIVE_FEE_BPS, TIER_MULTIPLIERS_BPS,
};
pub use economics::apy_calculator::{
    accrued_yield, emergency_withdrawal, il_protection_bps, impermanent_loss_bps,
    nft_accrued_yield, nft_daily_yield, simple_yield, tier_for_days, tvl_adjusted_apy_bps,
    LockTier, APY_30_DAYS_BPS, APY_7_DAYS_BPS, APY_90_DAYS_BPS, EMERGENCY_PENALTY_BPS,
};
pub use economics::balancer::{
    apply_adjustment, detect_imbalance, generate_report, suggest_adjustment, BalanceAdjustment,
    SupplyDemandRatio,
};
pub use economics::health_dashboard::{
    deactivate_alert, get_alert_threshold, get_health_dashboard, get_inflation_metrics,
    get_recent_alerts, get_supply_demand, initialize_dashboard, recalculate_health,
    set_alert_threshold, update_inflation_metrics, update_supply_demand, AlertEvent,
    AlertThreshold, EconomyHealth, EconomyHealthDashboard, InflationMetrics, SupplyDemandMetrics,
};
pub use economics::monitor::{
    calculate_inflation_rate, get_metrics, get_resource_metrics, initialize_monitor,
    track_resource_activity, update_supply_metrics, EconomicMetrics, ResourceMetrics,
};
pub use economics::progression_model::{
    cumulative_cost, days_to_level, level_after_days, level_cost, meets_retention_targets,
    psychological_price, simulate as simulate_progression_curve, CurveKind, PlayStyle,
    ProgressionCurve, ProgressionReport, DEFAULT_BASE_COST as PROGRESSION_BASE_COST,
    DEFAULT_GROWTH_BPS as PROGRESSION_GROWTH_BPS, MAX_LEVEL, UNITS_PER_HOUR,
};
pub use ship_upgrade::LevelUpgradeResult;
pub use staking::{
    GlobalStakingStats, GuildStake, StakeRecord, StakingError, StakingTier, GUILD_STAKE_LOCK_SECS,
};
pub use yield_farming::{
    EmergencyWithdrawal, FarmError, LpPoolState, LpStake, NftStake, ResourceStake, TvlReport,
    DEFAULT_TVL_TARGET, IL_POT_SHARE_BPS,
};

pub use trading::{
    add_liquidity, cancel_limit_order, create_pool, get_all_pools, get_limit_order, get_lp_balance,
    get_pool, get_trader_orders, get_trading_history, place_limit_order, quote_swap, record_trade,
    remove_liquidity, swap_exact_input, AmmError, LimitOrder, LiquidityPool, LiquidityProvider,
    OrderSide, TradeRecord, TradingError, AMM_MAX_ROUTE_HOPS, MAX_SLIPPAGE_BPS, SWAP_FEE_BPS,
};

pub use crafting::{add_xp, craft, craft_with_overcharge, get_level, get_total_craft_sink, get_xp};
pub use mobile_views::{MobileBatchInfo, MobileDashboard, MobileViewError, QuickScanPreview};
pub use nomad_bonding::{
    accept_bond, accrue_essence, claim_yield, create_bond, delegate_yield, dissolve_bond, get_bond,
    get_essence_balance, get_yield_delegation, BondError, BondStatus, NomadBond, YieldDelegation,
};
pub use notifications::alerts::{
    check_low_resources, notify_crafting_complete, notify_rare_discovery,
};
pub use notifications::push_service::{emit_notification, Notification};
pub use player_housing::{
    clear_guild_banner, customize_theme, define_furniture, expand_storage, feature_house,
    get_achievement_displays, get_featured_houses, get_furniture, get_guild_banner, get_house,
    get_house_storage, get_placements, get_rating, get_storage_capacity, get_visitors,
    initialize_house, install_achievement_display, place_furniture, purchase_furniture, rate_house,
    remove_achievement_display, remove_furniture, set_guild_banner, set_house_access, upgrade_house,
    visit_house, AchievementDisplay, FurnitureCategory, FurnitureItem, FurnitureRarity, GuildBanner,
    HouseAccess, HouseInstance, HouseRating, HouseSize, HouseStorage, HousingError, PlacedFurniture,
    StorageSummary, VisitorRecord, BASE_STORAGE_CAPACITY, MAX_ACHIEVEMENT_DISPLAYS,
    MAX_FEATURED_HOUSES, MAX_FURNITURE_PER_HOUSE, MAX_STORAGE_LEVEL, MAX_VISITOR_RECORDS,
    STORAGE_CAPACITY_PER_LEVEL,
};
pub use quest_system::{
    add_quest_node, choose_branch, claim_quest_reward, define_chain, get_active_quest_states,
    get_active_quests, get_chain, get_chain_progress, get_quest_node, get_quest_state,
    on_mission_completed, record_progress, start_chain, validate_chain, ChainProgress, QuestBranch,
    QuestChain, QuestError, QuestNode, QuestReward, QuestState, QuestStatus, CHAIN_TERMINUS,
    MAX_ACTIVE_QUESTS, MAX_BRANCHES_PER_NODE, MAX_CHAIN_LENGTH,
};
pub use recipes::{set_recipe, unlock_rare_recipe, RecipeError};
pub use ship_repair::{
    get_repair_config, get_total_repair_burn, quote_repair, repair_cost, set_repair_config,
    RepairConfig, RepairQuote, RepairReceipt, ShipRepairError, DEFAULT_COST_PER_POINT,
    DEFAULT_EMERGENCY_SURCHARGE_BPS, DEFAULT_MAX_REPAIR_PER_CALL,
};

#[contract]
pub struct NebulaNomadContract;

#[contractimpl]
impl NebulaNomadContract {
    /// Generate a 16x16 procedural nebula map using ledger-seeded PRNG.
    include!("contract/exploration.rs");
    include!("contract/gameplay.rs");
    include!("contract/economy.rs");
    include!("contract/social_governance.rs");
    include!("contract/events_progression.rs");
    include!("contract/marketplace_analytics.rs");

    /// List the marketplace listing IDs a seller has created.
    pub fn get_seller_content_listings(env: Env, seller: Address) -> Vec<u64> {
        content_tools::get_seller_marketplace_listings(&env, &seller)
    }

    // === Player Housing (Issue #532) ===

    /// Create a personal house (starts at Small size, Public access).
    pub fn housing_initialize(env: Env, owner: Address) -> Result<HouseInstance, HousingError> {
        player_housing::initialize_house(&env, owner)
    }

    /// Upgrade the owner's house to the next size tier.
    pub fn housing_upgrade(env: Env, owner: Address) -> Result<HouseInstance, HousingError> {
        player_housing::upgrade_house(&env, owner)
    }

    /// Change who may visit the owner's house.
    pub fn housing_set_access(
        env: Env,
        owner: Address,
        access: HouseAccess,
    ) -> Result<(), HousingError> {
        player_housing::set_house_access(&env, owner, access)
    }

    /// Customize wall, floor and lighting themes.
    pub fn housing_customize_theme(
        env: Env,
        owner: Address,
        wall_theme: u32,
        floor_theme: u32,
        lighting_theme: u32,
    ) -> Result<(), HousingError> {
        player_housing::customize_theme(&env, owner, wall_theme, floor_theme, lighting_theme)
    }

    /// Define a new furniture catalog item (admin/creator only).
    #[allow(clippy::too_many_arguments)]
    pub fn housing_define_furniture(
        env: Env,
        creator: Address,
        name: String,
        category: FurnitureCategory,
        rarity: FurnitureRarity,
        cost_resource: ResourceType,
        cost_amount: u64,
        is_functional: bool,
        bonus_description: String,
        is_limited_edition: bool,
    ) -> Result<u64, HousingError> {
        player_housing::define_furniture(
            &env,
            creator,
            name,
            category,
            rarity,
            cost_resource,
            cost_amount,
            is_functional,
            bonus_description,
            is_limited_edition,
        )
    }

    /// Purchase a furniture item (it is not placed until `housing_place_furniture`).
    pub fn housing_purchase_furniture(
        env: Env,
        owner: Address,
        furniture_id: u64,
    ) -> Result<(), HousingError> {
        player_housing::purchase_furniture(&env, owner, furniture_id)
    }

    /// Place a purchased furniture item at a grid position.
    pub fn housing_place_furniture(
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

    /// Remove a placed furniture item from the house.
    pub fn housing_remove_furniture(
        env: Env,
        owner: Address,
        furniture_id: u64,
    ) -> Result<(), HousingError> {
        player_housing::remove_furniture(&env, owner, furniture_id)
    }

    /// Show an unlocked achievement in a display case.
    pub fn housing_install_achievement(
        env: Env,
        owner: Address,
        achievement_id: u64,
        slot: u32,
    ) -> Result<u64, HousingError> {
        player_housing::install_achievement_display(&env, owner, achievement_id, slot)
    }

    /// Remove an achievement display case by ID.
    pub fn housing_remove_achievement(
        env: Env,
        owner: Address,
        display_id: u64,
    ) -> Result<(), HousingError> {
        player_housing::remove_achievement_display(&env, owner, display_id)
    }

    /// Display the owner's guild banner and motto.
    pub fn housing_set_guild_banner(
        env: Env,
        owner: Address,
        motto: String,
    ) -> Result<(), HousingError> {
        player_housing::set_guild_banner(&env, owner, motto)
    }

    /// Remove the guild banner from the house.
    pub fn housing_clear_guild_banner(env: Env, owner: Address) -> Result<(), HousingError> {
        player_housing::clear_guild_banner(&env, owner)
    }

    /// Expand house storage capacity by one level.
    pub fn housing_expand_storage(
        env: Env,
        owner: Address,
    ) -> Result<StorageSummary, HousingError> {
        player_housing::expand_storage(&env, owner)
    }

    /// Visit another player's house (subject to its access mode).
    pub fn housing_visit(env: Env, visitor: Address, house_owner: Address) -> Result<(), HousingError> {
        player_housing::visit_house(&env, visitor, house_owner)
    }

    /// Rate a visited house (100-500, i.e. 1-5 stars scaled by 100).
    pub fn housing_rate(
        env: Env,
        visitor: Address,
        house_owner: Address,
        rating: u32,
    ) -> Result<(), HousingError> {
        player_housing::rate_house(&env, visitor, house_owner, rating)
    }

    /// Feature a house in a discovery tier (curator only).
    pub fn housing_feature(
        env: Env,
        curator: Address,
        house_owner: Address,
        tier: Symbol,
    ) -> Result<(), HousingError> {
        player_housing::feature_house(&env, curator, house_owner, tier)
    }

    /// Read a house by owner address.
    pub fn housing_get_house(env: Env, owner: Address) -> Option<HouseInstance> {
        player_housing::get_house(&env, &owner)
    }

    /// Read a furniture catalog item.
    pub fn housing_get_furniture(env: Env, furniture_id: u64) -> Option<FurnitureItem> {
        player_housing::get_furniture(&env, furniture_id)
    }

    /// Read all placed furniture in a house.
    pub fn housing_get_placements(env: Env, owner: Address) -> Vec<PlacedFurniture> {
        player_housing::get_placements(&env, &owner)
    }

    /// Read a house's rating record.
    pub fn housing_get_rating(env: Env, owner: Address) -> Option<HouseRating> {
        player_housing::get_rating(&env, &owner)
    }

    /// Read recent visitor records for a house.
    pub fn housing_get_visitors(env: Env, owner: Address) -> Vec<VisitorRecord> {
        player_housing::get_visitors(&env, &owner)
    }

    /// Read all achievement display cases in a house.
    pub fn housing_get_achievement_displays(env: Env, owner: Address) -> Vec<AchievementDisplay> {
        player_housing::get_achievement_displays(&env, &owner)
    }

    /// Read the guild banner displayed in a house.
    pub fn housing_get_guild_banner(env: Env, owner: Address) -> Option<GuildBanner> {
        player_housing::get_guild_banner(&env, &owner)
    }

    /// Read the storage expansion record for a house.
    pub fn housing_get_storage(env: Env, owner: Address) -> Option<HouseStorage> {
        player_housing::get_house_storage(&env, &owner)
    }

    /// Read total item capacity (base + expansion bonus) for a house.
    pub fn housing_get_storage_capacity(env: Env, owner: Address) -> u32 {
        player_housing::get_storage_capacity(&env, &owner)
    }

    /// Read featured houses for a discovery tier.
    pub fn housing_get_featured(env: Env, tier: Symbol) -> Vec<Address> {
        player_housing::get_featured_houses(&env, tier)
    }
}

// Game Systems Modules (Issues #528-531)
// `seasons`, `event_scheduler`, `alliance_manager`, `privacy_stats`, `crafting`,
// `recipes` and `clan_wars` are all declared above with their original
// visibility. Re-declaring any of them here breaks the crate (E0428, #572).

// Game Systems Modules (Issues #550-553)
pub mod challenge_system;
pub mod skin_marketplace;
pub mod weather_system;
/// On-chain mini-games (poker, racing) with their own session state.
pub mod mini_games {
    pub mod poker;
    pub mod racing;
}
