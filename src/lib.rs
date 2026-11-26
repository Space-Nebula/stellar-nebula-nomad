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

use crate::bridge::stellar_ethereum::{
    self, BridgeConfig, BridgeError, BridgeRequest, WrappedAsset,
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
pub mod config;
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
/// Server-side anti-cheat detection, scoring, review and appeals for PvP (Issue #593).
pub mod anti_cheat;
mod referral_system;
/// Referral codes, tracking, rewards and anti-abuse (#701).
pub mod referral_system;
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
/// Behavioural bot detection, scoring and the admin review queue (#704).
pub mod bot_detection;
/// Daily login rewards, streaks and special reward events (#703).
pub mod daily_rewards;
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
/// Ship themes and per-player theme preferences (#700).
pub mod theme_customizer;
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
/// Exclusive rotating content, season-pass tracks and seasonal challenges.
pub mod seasonal_rewards;
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

/// Ethereum <-> Stellar asset bridge and cross-chain messaging (#713).
pub mod bridge;

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
// Issue #593 — anti-cheat detection, suspicion scoring, review and appeals.
pub use anti_cheat::{
    AntiCheatConfig, AntiCheatError, AppealRecord as AntiCheatAppeal,
    AppealStatus as AntiCheatAppealStatus, DetectionRecord as AntiCheatDetection,
    DetectionType as AntiCheatDetectionType, PenaltyKind as AntiCheatPenalty,
    PlayerRiskProfile as AntiCheatProfile, ReviewDecision as AntiCheatReviewDecision,
    Severity as AntiCheatSeverity, APPEAL_WINDOW_SECS as DEFAULT_APPEAL_WINDOW_SECS,
    DEFAULT_PERMANENT_BAN_THRESHOLD, DEFAULT_REVIEW_THRESHOLD, DEFAULT_TEMP_BAN_THRESHOLD,
    DEFAULT_WARNING_THRESHOLD, MAX_SCORE as ANTI_CHEAT_MAX_SCORE,
// Issue #592 — combat replay storage, playback and sharing.
pub use combat_replay::{
    deserialize_replay, get_replay, get_replay_bundle, get_replay_by_share, get_replay_frame,
    get_replay_frames, get_replay_id_for_combat, get_replay_id_for_share, get_player_replays,
    playback_state, seek_to_turn, serialize_replay, share_replay, CombatReplay, PlaybackState,
    ReplayBundle, ReplayError, ReplayExport, ReplayFrameView, ReplayKey, ReplayShare,
    FrameState, ReplayError as CombatReplayError, FRAME_CANCEL, FRAME_FINISH, FRAME_MOVE,
    FRAME_START, FRAMES_PER_CHUNK, MAX_FRAME_PAGE, MAX_REPLAY_FRAMES, MAX_REPLAYS_PER_OWNER,
    REPLAY_FORMAT_VERSION, REPLAY_FRAME_BYTES, SPEED_DOUBLE_BPS, SPEED_HALF_BPS,
    SPEED_NORMAL_BPS,
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
    clear_batch, execute_batch, execute_batch_mint, execute_batch_trade, execute_batch_transfer,
    execute_batch_update, get_player_batch, queue_batch_operation, BatchError, BatchMintResult,
    BatchOp, BatchOpType, BatchResult, BatchTradeResult, BatchTransferItem, BatchTransferResult,
    BatchUpdateItem, BatchUpdateResult, MAX_BATCH_SIZE,
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
    calculate_risk_reward, classify_anomaly, classify_batch, classify_nebula_anomaly,
    get_anomaly_effect, get_classification, refine_classification, AnomalyCategory, AnomalyEffect,
    AnomalyError, ClassificationRecord,
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
    complete_mission, generate_ai_mission, generate_daily_mission, get_player_missions,
    update_mission_progress, Mission, MissionError, MissionReward,
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
    contribute_objective, contribute_to_treasury, create_objective, found_alliance, get_alliance,
    get_alliance_benefits, get_alliance_treasury, get_member_contribution, get_member_role,
    get_objective, get_objective_count, get_player_alliance, get_relation, get_relation_proposal,
    join_alliance, kick_member, leave_alliance, propose_relation, set_member_role,
    transfer_leadership, Alliance, AllianceBenefits, AllianceError, AllianceObjective,
    AllianceRole, DiplomaticStatus, MembershipRecord, MAX_MEMBERS_PER_ALLIANCE, MAX_MEMBER_CAP,
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
    create_entanglement_pair, create_guild_channel, dissolve_pair, get_entangled_messages,
    get_entanglement_pair, get_guild_messages, get_message_count, get_pair_encryption_key,
    get_pair_remaining_uses, send_entangled_message, send_entangled_message_batch,
    send_guild_message, set_pair_encryption_key, set_pair_use_limit, EntangledMessage,
    EntanglementError, EntanglementPair, GuildCommsChannel, GuildMessage, DEFAULT_PAIR_USE_LIMIT,
    MAX_MESSAGE_BURST, PAIR_LIFETIME_SECS,
};
pub use achievements::{
    batch_check_and_unlock, get_achievement_def, get_notifications, get_player_stats,
    query_progress, try_unlock, AchievementCategory, AchievementDef, AchievementNotification,
    AchievementProgress as CatalogAchievementProgress, AchievementTier, AchievementsError,
    PlayerAchievementStats,
};
pub use badges::{
    get_badge_metadata, list_badges_by_owner, mint_badge, transfer_badge, BadgeError,
    BadgeMetadata, RarityTier,
};
pub use event_framework::{EventFrameworkError, StandardEvent};
pub use event_scheduler::{
    cancel_event, get_active_events, get_event, get_event_count, initialize_scheduler,
    reset_burst_counter as reset_event_burst, schedule_event, schedule_weekly_festival,
    trigger_scheduled_event, update_participants, EventError, EventResult, ScheduledEvent,
    SeasonalChallengeSpec, SeasonalEvent, SeasonalEventConfig, SeasonalEventEntry,
    SeasonalEventStatus, TimeLimitedChallenge, MAX_ACTIVE_EVENTS, MAX_CHALLENGES_PER_EVENT,
    MAX_PENDING_SEASONAL_EVENTS,
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
    can_travel, discover_portals, get_portal, get_portal_access, get_portal_count,
    get_portal_metadata, get_portal_topology, initialize_portal_registry, is_portal_open,
    query_portal_status, refresh_portal, register_portal, register_portal_batch,
    set_portal_access, set_portal_allowed, set_portal_metadata, set_portal_open,
    travel_through_portal, travel_through_portal_as, Portal, PortalAccessMode, PortalError,
    PortalLink, PortalMetadata, BASE_TRAVEL_COST, MAX_PORTALS_PER_TX, MAX_TOPOLOGY_PAGE,
    MIN_STABLE_PCT,
};
pub use privacy_stats::{
    batch_commit_stats, commit_private_stat, configure_private_aggregate,
    contribute_anonymous_stat, current_privacy_epoch, get_commitment, get_commitment_count,
    get_privacy_report, get_privacy_transparency_report, get_private_aggregate_config,
    initialize_privacy_stats, is_opted_in, opt_in_privacy, publish_privacy_report,
    reset_burst_counter as reset_privacy_burst, verify_private_stat, PrivacyError, PrivacyReport,
    PrivacyTransparencyReport, PrivateAggregateConfig, StatCommitment, MAX_COMMITMENTS_PER_TX,
    MIN_REPORT_COHORT, PRIVACY_EPOCH_SECS,
};
pub use prize_distributor::{
    distribute_weekly_prizes, fund_prize_pool, get_last_reset, get_prize_pool,
    get_total_distributed, initialize_prize_distributor, submit_leaderboard_snapshot, PrizeError,
    PrizeRecord, MAX_PAYOUT_POSITIONS, WEEK_SECONDS,
};
pub use battle_pass::PassTier;
pub use seasons::{
    get_last_transition, get_season_phase, is_transition_due, season_id_at, season_window,
    seasonal_event_reward_pool, Season, SeasonArchive, SeasonConfig as SeasonContentConfig,
    SeasonError, SeasonNebulaType, SeasonPhase, SeasonTheme, SeasonTransition,
    CHAPTER_DURATION_SECS, CHAPTERS_PER_SEASON, EXCLUSIVE_EVENT_BONUS_BPS, SEASON_DURATION_SECS,
    SEASON_MONTHS,
};
// Issue #594 — seasonal rewards, pass tiers, challenges and rotation.
pub use seasonal_rewards::{
    claim_season_pass_reward, claim_seasonal_challenge_reward, create_seasonal_challenge,
    ensure_season_transition, exclusive_content_for, get_challenge_status, get_exclusive_content,
    get_season_dashboard, get_season_pass_summary, highest_unlocked_tier, init_season_pass,
    next_unlocked_tier, pro_rata_reward, record_nebula_discovery, record_seasonal_activity,
    record_seasonal_challenge_progress, rotate_exclusive_content, seasonal_challenges_for_season,
    theme_symbol, tier_xp_required, transition_season, unlock_premium_pass, ChallengeStatus,
    ExclusiveContent, SeasonDashboard, SeasonPassSummary, SeasonTransitionView,
    SeasonalRewardsError, SeasonalRewardsKey,
    FREE_TIER_XP_BASE, FREE_TIER_XP_STEP, PREMIUM_TIER_XP_BASE, PREMIUM_TIER_XP_STEP,
    SEASONAL_CHALLENGE_WINDOW_SECS,
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
pub use notifications::push_service::{
    clear_inbox, emit_notification, get_inbox, get_notification_preferences, notify,
    set_notification_preferences, Notification, NotificationError, NotificationPreferences,
    NotificationPriority, ALL_CHANNELS, CHANNEL_EMAIL, CHANNEL_IN_APP, CHANNEL_PUSH,
};
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

    /// Generate an AI-tailored procedural mission for a player (Issue #639).
    pub fn generate_ai_mission(
        env: Env,
        player: Address,
    ) -> Result<mission_generator::Mission, mission_generator::MissionError> {
        mission_generator::generate_ai_mission(&env, player)
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

    pub fn get_anomaly_effect(
        _env: Env,
        anomaly_type: Symbol,
        confidence: u32,
    ) -> anomaly_classifier::AnomalyEffect {
        anomaly_classifier::get_anomaly_effect(&anomaly_type, confidence)
    }

    pub fn calculate_anomaly_risk_reward(
        env: Env,
        anomaly_id: u64,
    ) -> Result<anomaly_classifier::AnomalyEffect, anomaly_classifier::AnomalyError> {
        anomaly_classifier::calculate_risk_reward(&env, anomaly_id)
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
    ) -> Result<constellation_mapper::MatchResult, ConstellationError> {
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

    /// Set a custom message use limit on an entanglement pair.
    pub fn set_pair_use_limit(
        env: Env,
        caller: Address,
        pair_id: u64,
        limit: u64,
    ) -> Result<(), EntanglementError> {
        entanglement_comms::set_pair_use_limit(&env, &caller, pair_id, limit)
    }

    /// Return remaining message uses for an entanglement pair.
    pub fn get_pair_remaining_uses(env: Env, pair_id: u64) -> Result<u64, EntanglementError> {
        entanglement_comms::get_pair_remaining_uses(&env, pair_id)
    }

    /// Set the shared encryption key commitment hash for an entanglement pair.
    pub fn set_pair_encryption_key(
        env: Env,
        caller: Address,
        pair_id: u64,
        key_hash: BytesN<32>,
    ) -> Result<(), EntanglementError> {
        entanglement_comms::set_pair_encryption_key(&env, &caller, pair_id, key_hash)
    }

    /// Read the shared encryption key commitment hash for a pair.
    pub fn get_pair_encryption_key(
        env: Env,
        pair_id: u64,
    ) -> Result<BytesN<32>, EntanglementError> {
        entanglement_comms::get_pair_encryption_key(&env, pair_id)
    }

    /// Fetch encrypted message history for a pair.
    pub fn get_entangled_messages(
        env: Env,
        pair_id: u64,
        offset: u64,
        limit: u32,
    ) -> Vec<EntangledMessage> {
        entanglement_comms::get_entangled_messages(&env, pair_id, offset, limit)
    }

    /// Create a guild coordination channel backed by alliance_manager.
    pub fn create_guild_channel(
        env: Env,
        caller: Address,
        guild_id: u64,
        encrypted_topic: BytesN<32>,
    ) -> Result<entanglement_comms::GuildCommsChannel, EntanglementError> {
        entanglement_comms::create_guild_channel(&env, &caller, guild_id, encrypted_topic)
    }

    /// Broadcast an encrypted coordination message to a guild channel.
    pub fn send_guild_message(
        env: Env,
        caller: Address,
        guild_id: u64,
        sender_region: u64,
        payload: BytesN<64>,
    ) -> Result<u64, EntanglementError> {
        entanglement_comms::send_guild_message(&env, &caller, guild_id, sender_region, payload)
    }

    /// Fetch guild coordination messages.
    pub fn get_guild_messages(
        env: Env,
        guild_id: u64,
        offset: u64,
        limit: u32,
    ) -> Vec<entanglement_comms::GuildMessage> {
        entanglement_comms::get_guild_messages(&env, guild_id, offset, limit)
    }

    // ─── Achievements & Badges Integration (Issue #641) ───────────────────

    pub fn get_achievement_def(
        env: Env,
        id: u64,
    ) -> Result<achievements::AchievementDef, achievements::AchievementsError> {
        achievements::get_achievement_def(&env, id)
    }

    pub fn unlock_catalog_achievement(
        env: Env,
        player: Address,
        achievement_id: u64,
    ) -> Result<u64, achievement_engine::AchievementError> {
        achievements::try_unlock(&env, &player, achievement_id)
    }

    pub fn query_achievement_progress(
        env: Env,
        player: Address,
        achievement_id: u64,
    ) -> Result<achievements::AchievementProgress, achievements::AchievementsError> {
        achievements::query_progress(&env, &player, achievement_id)
    }

    pub fn get_achievement_stats(
        env: Env,
        player: Address,
    ) -> Result<achievements::PlayerAchievementStats, achievements::AchievementsError> {
        achievements::get_player_stats(&env, &player)
    }

    pub fn get_achievement_notifications(
        env: Env,
        player: Address,
    ) -> Vec<achievements::AchievementNotification> {
        achievements::get_notifications(&env, &player)
    }

    pub fn batch_unlock_catalog_achvs(
        env: Env,
        player: Address,
    ) -> Result<Vec<u64>, achievement_engine::AchievementError> {
        achievements::batch_check_and_unlock(&env, &player)
    }

    pub fn mint_badge_nft(
        env: Env,
        badge_id: u64,
        description: String,
        image_uri: String,
    ) -> Result<badges::BadgeMetadata, badges::BadgeError> {
        badges::mint_badge(&env, badge_id, description, image_uri)
    }

    pub fn get_badge_metadata(
        env: Env,
        badge_id: u64,
    ) -> Result<badges::BadgeMetadata, badges::BadgeError> {
        badges::get_badge_metadata(&env, badge_id)
    }

    pub fn transfer_badge(
        env: Env,
        from: Address,
        to: Address,
        badge_id: u64,
    ) -> Result<(), badges::BadgeError> {
        badges::transfer_badge(&env, from, to, badge_id)
    }

    pub fn list_badges_by_owner(env: Env, player: Address) -> Vec<u64> {
        badges::list_badges_by_owner(&env, &player)
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

    /// Whether the current 90-day season is active or awaiting rollover.
    pub fn get_season_phase(env: Env) -> Result<SeasonPhase, SeasonError> {
        seasons::get_season_phase(&env)
    }

    /// Seconds remaining in the current season (0 once due for transition).
    pub fn get_season_time_remaining(env: Env) -> Result<u64, SeasonError> {
        seasons::get_season_time_remaining(&env)
    }

    /// Whether the 90-day window has closed and a rollover is due.
    pub fn is_transition_due(env: Env) -> Result<bool, SeasonError> {
        seasons::is_transition_due(&env)
    }

    /// Most recent atomic transition audit record, if one exists.
    pub fn get_last_transition(env: Env) -> Option<SeasonTransition> {
        seasons::get_last_transition(&env)
    }

    /// Current chapter (1–3) derived from elapsed season time.
    pub fn get_current_chapter(env: Env) -> Result<u32, SeasonError> {
        seasons::get_current_chapter(&env)
    }

    /// Admin: persist the chapter derived from elapsed time.
    pub fn advance_chapter(env: Env, admin: Address) -> Result<u32, SeasonError> {
        seasons::advance_chapter(&env, &admin)
    }

    /// Record a player's seasonal scan/essence progress and award pass XP.
    pub fn record_seasonal_participation(
        env: Env,
        profile_id: u64,
        scans: u32,
        essence: i128,
    ) -> Result<(), SeasonalRewardsError> {
        seasons::record_participation(&env, profile_id, scans, essence)?;
        seasonal_rewards::record_seasonal_activity(&env, profile_id, scans, essence)?;
        Ok(())
    }

    /// Record discovery of the season's exclusive nebula and award pass XP.
    pub fn record_seasonal_nebula_discovery(
        env: Env,
        profile_id: u64,
    ) -> Result<(), SeasonalRewardsError> {
        seasonal_rewards::record_nebula_discovery(&env, profile_id)
    }

    /// Admin: end the current season and start the next one.
    pub fn rollover_season(
        env: Env,
        admin: Address,
        new_title: String,
        participant_ids: Vec<u64>,
    ) -> Result<u64, SeasonError> {
        seasons::rollover_season(&env, &admin, new_title, &participant_ids)
    }

    /// Admin/keeper: full season transition including ELO ranking reset and
    /// provisioning of the next season's pass and rotating content.
    pub fn transition_season(
        env: Env,
        admin: Address,
        new_title: String,
        participant_ids: Vec<u64>,
        ranked_players: Vec<Address>,
    ) -> Result<SeasonTransitionView, SeasonalRewardsError> {
        seasonal_rewards::transition_season(
            &env,
            admin,
            new_title,
            participant_ids,
            ranked_players,
        )
    }

    /// Idempotent keeper hook: transitions only when the season has expired.
    pub fn ensure_season_transition(
        env: Env,
        admin: Address,
        new_title: String,
        participant_ids: Vec<u64>,
        ranked_players: Vec<Address>,
    ) -> Result<Option<SeasonTransitionView>, SeasonalRewardsError> {
        seasonal_rewards::ensure_season_transition(
            &env,
            admin,
            new_title,
            participant_ids,
            ranked_players,
        )
    }

    /// One-read seasonal dashboard payload for the frontend.
    pub fn get_season_dashboard(
        env: Env,
        profile_id: Option<u64>,
    ) -> Result<SeasonDashboard, SeasonalRewardsError> {
        seasonal_rewards::get_season_dashboard(&env, profile_id)
    }

    /// Admin: install a season's free/premium reward tables and exclusive content.
    pub fn init_season_pass(
        env: Env,
        admin: Address,
        season_id: u64,
    ) -> Result<bool, SeasonalRewardsError> {
        seasonal_rewards::init_season_pass(&env, admin, season_id)
    }

    /// Exclusive cosmetics/nebula active for a season.
    pub fn get_exclusive_content(env: Env, season_id: u64) -> Option<ExclusiveContent> {
        seasonal_rewards::get_exclusive_content(&env, season_id)
    }

    /// Admin: refresh a season's rotated content bundle after a theme hotfix.
    pub fn rotate_exclusive_content(
        env: Env,
        admin: Address,
        season_id: u64,
    ) -> Result<ExclusiveContent, SeasonalRewardsError> {
        seasonal_rewards::rotate_exclusive_content(&env, admin, season_id)
    }

    /// Player's season-pass tier progress for the active season.
    pub fn get_season_pass_summary(
        env: Env,
        profile_id: u64,
    ) -> Result<SeasonPassSummary, SeasonalRewardsError> {
        seasonal_rewards::get_season_pass_summary(&env, profile_id)
    }

    /// Claim a season-pass tier reward from the free or premium track.
    pub fn claim_season_pass_reward(
        env: Env,
        player: Address,
        profile_id: u64,
        tier: u32,
        pass_tier: PassTier,
    ) -> Result<i128, SeasonalRewardsError> {
        seasonal_rewards::claim_season_pass_reward(&env, player, profile_id, tier, pass_tier)
    }

    /// Unlock the cosmetic-only premium pass track for the active season.
    pub fn unlock_premium_pass(
        env: Env,
        player: Address,
        profile_id: u64,
    ) -> Result<(), SeasonalRewardsError> {
        seasonal_rewards::unlock_premium_pass(&env, player, profile_id)
    }

    /// Admin: create a challenge bounded by the active season window.
    pub fn create_seasonal_challenge(
        env: Env,
        admin: Address,
        spec: SeasonalChallengeSpec,
        duration_secs: u64,
    ) -> Result<u64, SeasonalRewardsError> {
        seasonal_rewards::create_seasonal_challenge(&env, &admin, spec, duration_secs)
    }

    /// Record progress toward a challenge in the active season.
    pub fn record_seasonal_challenge_progress(
        env: Env,
        profile_id: u64,
        challenge_id: u64,
        metric_delta: u64,
    ) -> Result<u64, SeasonalRewardsError> {
        seasonal_rewards::record_seasonal_challenge_progress(
            &env,
            profile_id,
            challenge_id,
            metric_delta,
        )
    }

    /// Claim a completed seasonal challenge (premium rate for pass holders).
    pub fn claim_seasonal_challenge_reward(
        env: Env,
        player: Address,
        profile_id: u64,
        challenge_id: u64,
    ) -> Result<i128, SeasonalRewardsError> {
        seasonal_rewards::claim_seasonal_challenge_reward(
            &env,
            player,
            profile_id,
            challenge_id,
        )
    }

    /// Open challenges belonging to a season.
    pub fn get_seasonal_challenges(
        env: Env,
        season_id: u64,
    ) -> Vec<TimeLimitedChallenge> {
        seasonal_rewards::seasonal_challenges_for_season(&env, season_id)
    }

    /// Player-facing progress for one seasonal challenge.
    pub fn get_seasonal_challenge_status(
        env: Env,
        profile_id: u64,
        challenge_id: u64,
    ) -> Result<ChallengeStatus, SeasonalRewardsError> {
        seasonal_rewards::get_challenge_status(&env, profile_id, challenge_id)
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

    /// Read featured houses for a discovery tier.
    pub fn housing_get_featured(env: Env, tier: Symbol) -> Vec<Address> {
        player_housing::get_featured_houses(&env, tier)
    }

    // === Ethereum Bridge (Issue #713) ===

    /// Initialize the Ethereum bridge with its admin (relayer). Runs once.
    pub fn bridge_initialize(env: Env, admin: Address) -> Result<(), BridgeError> {
        stellar_ethereum::initialize_bridge(&env, &admin)
    }

    /// Pause or unpause the bridge (admin only).
    pub fn bridge_set_paused(env: Env, admin: Address, paused: bool) -> Result<(), BridgeError> {
        stellar_ethereum::set_bridge_paused(&env, &admin, paused)
    }

    /// Update fee, confirmations and per-transfer limit (admin only).
    pub fn bridge_update_config(
        env: Env,
        admin: Address,
        config: BridgeConfig,
    ) -> Result<(), BridgeError> {
        stellar_ethereum::update_bridge_config(&env, &admin, config)
    }

    /// Read the bridge configuration.
    pub fn bridge_get_config(env: Env) -> Result<BridgeConfig, BridgeError> {
        stellar_ethereum::get_bridge_config(&env)
    }

    /// Register a Stellar token for bridging to an Ethereum token (admin only).
    pub fn bridge_register_asset(
        env: Env,
        admin: Address,
        symbol: Symbol,
        token: Address,
        eth_contract: BytesN<20>,
        chain_id: u32,
    ) -> Result<u64, BridgeError> {
        stellar_ethereum::register_wrapped_asset(&env, &admin, symbol, token, eth_contract, chain_id)
    }

    /// Enable or disable bridging for one asset (admin only).
    pub fn bridge_set_asset_enabled(
        env: Env,
        admin: Address,
        symbol: Symbol,
        chain_id: u32,
        enabled: bool,
    ) -> Result<(), BridgeError> {
        stellar_ethereum::set_wrapped_asset_enabled(&env, &admin, symbol, chain_id, enabled)
    }

    /// Read a bridged asset, including locked liquidity and collected fees.
    pub fn bridge_get_asset(
        env: Env,
        symbol: Symbol,
        chain_id: u32,
    ) -> Result<WrappedAsset, BridgeError> {
        stellar_ethereum::get_wrapped_asset(&env, symbol, chain_id)
    }

    /// Lock tokens and request their release on Ethereum. Returns the request ID.
    pub fn bridge_initiate(
        env: Env,
        player: Address,
        asset: Symbol,
        chain_id: u32,
        amount: i128,
        eth_recipient: BytesN<20>,
    ) -> Result<u64, BridgeError> {
        stellar_ethereum::initiate_bridge(&env, player, asset, chain_id, amount, eth_recipient)
    }

    /// Confirm the Ethereum mint for a pending request (admin only).
    pub fn bridge_confirm(
        env: Env,
        admin: Address,
        request_id: u64,
        dest_tx_hash: BytesN<32>,
        confirmations: u32,
    ) -> Result<(), BridgeError> {
        stellar_ethereum::confirm_bridge(&env, admin, request_id, dest_tx_hash, confirmations)
    }

    /// Complete a confirmed request (admin only).
    pub fn bridge_complete(env: Env, admin: Address, request_id: u64) -> Result<(), BridgeError> {
        stellar_ethereum::complete_bridge(&env, admin, request_id)
    }

    /// Cancel a pending request and refund it (player only).
    pub fn bridge_cancel(env: Env, player: Address, request_id: u64) -> Result<(), BridgeError> {
        stellar_ethereum::cancel_bridge(&env, player, request_id)
    }

    /// Reject a pending request and refund it (admin only).
    pub fn bridge_fail(env: Env, admin: Address, request_id: u64) -> Result<(), BridgeError> {
        stellar_ethereum::fail_bridge(&env, admin, request_id)
    }

    /// Release tokens for a burn on Ethereum, once per burn transaction (admin only).
    #[allow(clippy::too_many_arguments)]
    pub fn bridge_release(
        env: Env,
        admin: Address,
        eth_tx_hash: BytesN<32>,
        eth_sender: BytesN<20>,
        asset: Symbol,
        chain_id: u32,
        recipient: Address,
        amount: i128,
        confirmations: u32,
    ) -> Result<u64, BridgeError> {
        stellar_ethereum::release_from_ethereum(
            &env,
            admin,
            eth_tx_hash,
            eth_sender,
            asset,
            chain_id,
            recipient,
            amount,
            confirmations,
        )
    }

    /// Whether an Ethereum burn transaction has already been released.
    pub fn bridge_is_eth_tx_processed(env: Env, eth_tx_hash: BytesN<32>) -> bool {
        stellar_ethereum::is_eth_tx_processed(&env, eth_tx_hash)
    }

    /// Withdraw the fees collected for an asset to `to` (admin only).
    pub fn bridge_withdraw_fees(
        env: Env,
        admin: Address,
        asset: Symbol,
        chain_id: u32,
        to: Address,
    ) -> Result<i128, BridgeError> {
        stellar_ethereum::withdraw_bridge_fees(&env, admin, asset, chain_id, to)
    }

    /// Send a message to an Ethereum contract. Returns its nonce.
    pub fn bridge_send_message(
        env: Env,
        sender: Address,
        chain_id: u32,
        eth_target: BytesN<20>,
        payload: Bytes,
    ) -> Result<u64, BridgeError> {
        stellar_ethereum::send_message(&env, sender, chain_id, eth_target, payload)
    }

    /// Record a message from Ethereum, once per (chain, nonce) (admin only).
    pub fn bridge_receive_message(
        env: Env,
        admin: Address,
        chain_id: u32,
        nonce: u64,
        eth_sender: BytesN<20>,
        recipient: Address,
        payload: Bytes,
    ) -> Result<(), BridgeError> {
        stellar_ethereum::receive_message(&env, admin, chain_id, nonce, eth_sender, recipient, payload)
    }

    /// Whether the inbound message (chain, nonce) has been received.
    pub fn bridge_is_message_processed(env: Env, chain_id: u32, nonce: u64) -> bool {
        stellar_ethereum::is_message_processed(&env, chain_id, nonce)
    }

    /// Read a bridge request.
    pub fn bridge_get_request(env: Env, request_id: u64) -> Result<BridgeRequest, BridgeError> {
        stellar_ethereum::get_bridge_request(&env, request_id)
    }

    /// Total amount an address has bridged to Ethereum.
    pub fn bridge_get_bridged_total(env: Env, player: Address) -> i128 {
        stellar_ethereum::get_bridged_total(&env, &player)
    }

    // === Anti-Cheat Detection & Review (Issue #593) ===

    /// Bootstrap or rotate the anti-cheat administrator.
    pub fn ac_set_admin(env: Env, caller: Address, admin: Address) -> Result<(), AntiCheatError> {
        anti_cheat::set_admin(&env, &caller, &admin)
    }

    /// Update anti-cheat thresholds (admin only).
    pub fn ac_set_config(
        env: Env,
        caller: Address,
        config: AntiCheatConfig,
    ) -> Result<(), AntiCheatError> {
        anti_cheat::set_config(&env, &caller, config)
    }

    /// Current anti-cheat thresholds.
    pub fn ac_get_config(env: Env) -> AntiCheatConfig {
        anti_cheat::get_config(&env)
    }

    /// Current suspicion profile for a player.
    pub fn ac_get_profile(env: Env, player: Address) -> AntiCheatProfile {
        anti_cheat::get_profile(&env, &player)
    }

    /// Whether a player is currently banned from PvP actions.
    pub fn ac_is_banned(env: Env, player: Address) -> bool {
        anti_cheat::is_banned(&env, &player)
    }

    /// Fetch a single detection record by id.
    pub fn ac_get_detection(
        env: Env,
        detection_id: u64,
    ) -> Result<AntiCheatDetection, AntiCheatError> {
        anti_cheat::get_detection(&env, detection_id)
    }

    /// Most recent detections for a player (bounded by `limit`).
    pub fn ac_get_player_detections(
        env: Env,
        player: Address,
        limit: u32,
    ) -> Vec<AntiCheatDetection> {
        anti_cheat::get_player_detections(&env, &player, limit)
    }

    /// Detections awaiting manual admin review (bounded by `limit`).
    pub fn ac_get_review_queue(env: Env, limit: u32) -> Vec<AntiCheatDetection> {
        anti_cheat::get_review_queue(&env, limit)
    }

    /// Resolve a queued detection (admin only).
    pub fn ac_resolve_review(
        env: Env,
        admin: Address,
        detection_id: u64,
        decision: AntiCheatReviewDecision,
        reason: Symbol,
    ) -> Result<(), AntiCheatError> {
        anti_cheat::resolve_review(&env, &admin, detection_id, decision, reason)
    }

    /// File an appeal for a detection (player only, within appeal window).
    pub fn ac_submit_appeal(
        env: Env,
        player: Address,
        detection_id: u64,
        reason: Symbol,
    ) -> Result<u64, AntiCheatError> {
        anti_cheat::submit_appeal(&env, &player, detection_id, reason)
    }

    /// Fetch an appeal by id.
    pub fn ac_get_appeal(env: Env, appeal_id: u64) -> Result<AntiCheatAppeal, AntiCheatError> {
        anti_cheat::get_appeal(&env, appeal_id)
    }

    /// Approve or reject an appeal (admin only).
    pub fn ac_resolve_appeal(
        env: Env,
        admin: Address,
        appeal_id: u64,
        approve: bool,
        reason: Symbol,
    ) -> Result<(), AntiCheatError> {
        anti_cheat::resolve_appeal(&env, &admin, appeal_id, approve, reason)
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
