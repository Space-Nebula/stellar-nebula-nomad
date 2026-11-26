//! Comprehensive Achievement System with 200+ achievements across multiple categories.
//!
//! ## Overview
//!
//! This module provides a rich achievement catalog with:
//! - 200+ unique achievements across 5 categories
//! - 4 tier system: Bronze, Silver, Gold, Platinum
//! - 20+ secret achievements
//! - Meta-achievements for completing collections
//! - Achievement point system with leaderboard
//! - Progress tracking with notifications
//! - Integration with reputation system
//! - NFT badge minting via badges.rs

use soroban_sdk::{contracterror, contracttype, symbol_short, Address, Env, String, Symbol, Vec};

use crate::achievement_engine::{
    batch_unlock_achievements, unlock_achievement, AchievementError, AchievementKey,
    AchievementTemplate,
};
use crate::player_profile::get_profile_by_owner;
use crate::reputation::{source_points, ReputationSource};
use crate::ship_nft::get_ships_by_owner;

// ─── Constants ────────────────────────────────────────────────────────────────

pub const TOTAL_ACHIEVEMENTS: u64 = 250;
pub const SECRET_ACHIEVEMENT_START: u64 = 231;
pub const META_ACHIEVEMENT_START: u64 = 201;

// ─── Achievement Categories ───────────────────────────────────────────────────

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[contracttype]
#[repr(u32)]
pub enum AchievementCategory {
    Exploration = 0,
    Economic = 1,
    Social = 2,
    Progression = 3,
    Special = 4,
}

// ─── Achievement Tiers ────────────────────────────────────────────────────────

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
#[contracttype]
#[repr(u32)]
pub enum AchievementTier {
    Bronze = 0,
    Silver = 1,
    Gold = 2,
    Platinum = 3,
}

impl AchievementTier {
    pub fn points(&self) -> u32 {
        match self {
            AchievementTier::Bronze => 10,
            AchievementTier::Silver => 25,
            AchievementTier::Gold => 50,
            AchievementTier::Platinum => 100,
        }
    }
}

// ─── Achievement ID Enumeration ───────────────────────────────────────────────

/// Comprehensive achievement ID catalog (250 total achievements)
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[contracttype]
#[repr(u64)]
pub enum AchievementId {
    // ═══ EXPLORATION CATEGORY (1-80) ═══════════════════════════════════════════

    // Basic Exploration (1-20) - Bronze Tier
    FirstScan = 1,
    Surveyor10 = 2,
    Surveyor25 = 3,
    Explorer50 = 4,
    Navigator100 = 5,
    Pathfinder150 = 6,
    Voyager200 = 7,
    Traveler300 = 8,
    Pioneer400 = 9,
    Trailblazer500 = 10,
    Master750 = 11,
    Legend1000 = 12,
    CosmicWanderer1500 = 13,
    StellarNomad2000 = 14,
    GalacticExplorer3000 = 15,
    UniversalTraveler5000 = 16,
    FirstRegionDiscovery = 17,
    RegionMaster10 = 18,
    RegionConqueror25 = 19,
    AllRegionsExplored = 20,

    // Nebula Types (21-40) - Silver Tier
    CrimsonNebulaDiscovery = 21,
    SapphireNebulaDiscovery = 22,
    EmeraldNebulaDiscovery = 23,
    GoldenNebulaDiscovery = 24,
    VoidNebulaDiscovery = 25,
    NebulaTypeCollector = 26,
    RareNebulaHunter = 27,
    LegendaryNebula = 28,
    AncientNebulaDiscovery = 29,
    TemporalNebulaDiscovery = 30,
    QuantumNebulaDiscovery = 31,
    DarkMatterNebulaDiscovery = 32,
    NebulaPhenomenologist = 33,
    CosmicAnomalyFinder = 34,
    GravitationalWaveObserver = 35,
    BlackHoleProximity = 36,
    PulsarNavigator = 37,
    SupernovaWitness = 38,
    NeutronStarEncounter = 39,
    WhiteDwarfVisitor = 40,

    // Distance & Speed (41-60) - Gold Tier
    Distance1000LY = 41,
    Distance5000LY = 42,
    Distance10000LY = 43,
    Distance50000LY = 44,
    Distance100000LY = 45,
    IntergalacticVoyager = 46,
    SpeedDemon100 = 47,
    SpeedDemon500 = 48,
    WarpMaster = 49,
    HyperspaceJumper = 50,
    ConsecutiveScans10 = 51,
    ConsecutiveScans50 = 52,
    ConsecutiveScans100 = 53,
    MarathonExplorer24h = 54,
    WeekLongExpedition = 55,
    MonthlyMilestone = 56,
    YearlyDedication = 57,
    DailyStreak7 = 58,
    DailyStreak30 = 59,
    DailyStreak365 = 60,

    // Advanced Exploration (61-80) - Platinum Tier
    CompleteSystemScan = 61,
    BinaryStarSystem = 62,
    TripleStarSystem = 63,
    PlanetarySystemExplorer = 64,
    ExoplanetDiscoverer = 65,
    HabitableZoneFinder = 66,
    AlienArtifactSeeker = 67,
    AncientRelicCollector = 68,
    StellarCartographer = 69,
    CosmicMapmaker = 70,
    UnchartedTerritoryPioneer = 71,
    FrontierSettler = 72,
    DeepSpaceSurveyor = 73,
    DarkEnergyStudy = 74,
    CosmicMicrowaveMapping = 75,
    GalacticFilamentExplorer = 76,
    VoidRegionSurvivor = 77,
    InterstellarMediumAnalyst = 78,
    CosmicDustCollector = 79,
    StellarNurseryObserver = 80,

    // ═══ ECONOMIC CATEGORY (81-130) ════════════════════════════════════════════

    // Essence Milestones (81-100) - Bronze Tier
    Essence100 = 81,
    Essence500 = 82,
    Essence1000 = 83,
    Essence5000 = 84,
    Essence10000 = 85,
    Essence25000 = 86,
    Essence50000 = 87,
    Essence100000 = 88,
    Essence250000 = 89,
    Essence500000 = 90,
    Essence1M = 91,
    EssenceTycoon = 92,
    FirstEssenceEarned = 93,
    EssenceHoarder = 94,
    EssenceSpender = 95,
    EssenceInvestor = 96,
    DailyEssence1000 = 97,
    WeeklyEssence10000 = 98,
    MonthlyEssence100000 = 99,
    EssenceMogul = 100,

    // Resource Collection (101-120) - Silver Tier
    StellarDustCollector = 101,
    DarkMatterHarvester = 102,
    ExoticMatterGatherer = 103,
    QuantumParticleCollector = 104,
    AntiMatterAcquirer = 105,
    NeutroniumMiner = 106,
    StrangeQuarkProspector = 107,
    HiggsBosonHunter = 108,
    ResourceDiversifier = 109,
    ResourceMaster = 110,
    RareResourceCollector = 111,
    LegendaryResourceOwner = 112,
    Resource1000Units = 113,
    Resource10000Units = 114,
    Resource100000Units = 115,
    ResourceTradingNovice = 116,
    ResourceTradingExpert = 117,
    ResourceMarketMaster = 118,
    ArbitrageProfiteer = 119,
    MarketTimingGenius = 120,

    // Trading & Economy (121-130) - Gold Tier
    FirstTrade = 121,
    Trader10 = 122,
    Trader100 = 123,
    Trader1000 = 124,
    MerchantMagnate = 125,
    TradeProfit10000 = 126,
    TradeProfit100000 = 127,
    TradeProfit1M = 128,
    MarketManipulator = 129,
    EconomicKingpin = 130,

    // ═══ SOCIAL CATEGORY (131-170) ═════════════════════════════════════════════

    // Bonds & Relationships (131-150) - Bronze Tier
    FirstBondCreated = 131,
    BondMaster5 = 132,
    BondMaster10 = 133,
    BondMaster25 = 134,
    BondNetwork50 = 135,
    YieldDelegator = 136,
    YieldBenefactor = 137,
    MutualBonds = 138,
    BondDuration1Week = 139,
    BondDuration1Month = 140,
    BondDuration1Year = 141,
    LifelongBond = 142,
    YieldShared10000 = 143,
    YieldShared100000 = 144,
    YieldShared1M = 145,
    TrustedPartner = 146,
    BondingLegend = 147,
    CooperativeSpirit = 148,
    TeamPlayer = 149,
    SocialButterfly = 150,

    // Guild & Community (151-170) - Silver Tier
    GuildMember = 151,
    GuildOfficer = 152,
    GuildLeader = 153,
    GuildFounder = 154,
    GuildSize10 = 155,
    GuildSize50 = 156,
    GuildSize100 = 157,
    GuildContributor = 158,
    GuildBenefactor = 159,
    GuildChampion = 160,
    CommunityHelper = 161,
    Mentor10 = 162,
    Mentor50 = 163,
    MentorMaster = 164,
    ReferralPro5 = 165,
    ReferralPro25 = 166,
    ReferralLegend100 = 167,
    EventParticipant = 168,
    EventOrganizer = 169,
    CommunityPillar = 170,

    // ═══ PROGRESSION CATEGORY (171-200) ════════════════════════════════════════

    // Fleet Building (171-185) - Bronze/Silver Tier
    FirstShip = 171,
    Fleet3 = 172,
    Fleet5 = 173,
    Fleet10 = 174,
    Fleet20 = 175,
    Fleet50 = 176,
    Fleet100 = 177,
    Armada = 178,
    ShipUpgrade1 = 179,
    ShipUpgrade10 = 180,
    ShipUpgrade50 = 181,
    ShipMaxLevel = 182,
    FleetDiversity = 183,
    RareShipOwner = 184,
    LegendaryShipOwner = 185,

    // Player Progression (186-200) - Gold/Platinum Tier
    PlayerLevel10 = 186,
    PlayerLevel25 = 187,
    PlayerLevel50 = 188,
    PlayerLevel100 = 189,
    MaxPlayerLevel = 190,
    SkillMaster = 191,
    AllSkillsUnlocked = 192,
    TalentTreeComplete = 193,
    PrestigeLevel1 = 194,
    PrestigeLevel5 = 195,
    PrestigeLevel10 = 196,
    PrestigeMaster = 197,
    TotalPower10000 = 198,
    TotalPower100000 = 199,
    OmegaPowerLevel = 200,

    // ═══ META-ACHIEVEMENTS (201-230) ═══════════════════════════════════════════
    ExplorationMaster = 201,
    EconomicMogul = 202,
    SocialMaven = 203,
    ProgressionPerfectionist = 204,
    BronzeCollector = 205,
    SilverCollector = 206,
    GoldCollector = 207,
    PlatinumCollector = 208,
    CategoryMaster = 209,
    TierMaster = 210,
    Completionist50 = 211,
    Completionist100 = 212,
    Completionist150 = 213,
    Completionist200 = 214,
    UltimateCompletionist = 215,
    SecretSeeker10 = 216,
    SecretSeeker20 = 217,
    AllSecretsRevealed = 218,
    AchievementHunter = 219,
    LegendaryStatus = 220,
    PointsMilestone1000 = 221,
    PointsMilestone5000 = 222,
    PointsMilestone10000 = 223,
    LeaderboardTop100 = 224,
    LeaderboardTop10 = 225,
    LeaderboardKing = 226,
    FirstToAchieve = 227,
    PerfectWeek = 228,
    PerfectMonth = 229,
    PerfectYear = 230,

    // ═══ SECRET ACHIEVEMENTS (231-250) ═════════════════════════════════════════
    EasterEggFinder = 231,
    HiddenPathDiscoverer = 232,
    SecretNebula = 233,
    MythicalEncounter = 234,
    CosmicWhisperer = 235,
    ForbiddenKnowledge = 236,
    TimeTraveler = 237,
    ParallelExplorer = 238,
    DimensionalRift = 239,
    CosmicAnomaly = 240,
    UnknownSignal = 241,
    AlienContact = 242,
    AncientProphecy = 243,
    QuantumEntanglement = 244,
    SchrödingerNomad = 245,
    HeisenbergUncertain = 246,
    EinsteinBridge = 247,
    HawkingRadiation = 248,
    PlanckExplorer = 249,
    UnifiedTheory = 250,
}

// ─── Enhanced Achievement Definition ──────────────────────────────────────────

#[derive(Clone, Debug, PartialEq)]
#[contracttype]
pub struct AchievementDef {
    pub id: u64,
    pub title: String,
    pub description: String,
    pub category: AchievementCategory,
    pub tier: AchievementTier,
    pub is_secret: bool,
    pub is_meta: bool,
    pub points: u32,
    pub min_scans: u32,
    pub min_essence: i128,
    pub min_ships: u32,
    pub custom_requirements: String,
}

// ─── Progress Tracking ────────────────────────────────────────────────────────

#[derive(Clone, Debug, PartialEq)]
#[contracttype]
pub struct AchievementProgress {
    pub achievement_id: u64,
    pub title: String,
    pub description: String,
    pub category: AchievementCategory,
    pub tier: AchievementTier,
    pub unlocked: bool,
    pub eligible: bool,
    pub progress_pct: u32,
    pub points: u32,
    pub is_secret: bool,
    pub unlocked_at: u64,
}

// ─── Player Achievement Stats ─────────────────────────────────────────────────

#[derive(Clone, Debug, PartialEq)]
#[contracttype]
pub struct PlayerAchievementStats {
    pub player: Address,
    pub total_unlocked: u32,
    pub total_points: u32,
    pub bronze_count: u32,
    pub silver_count: u32,
    pub gold_count: u32,
    pub platinum_count: u32,
    pub secret_count: u32,
    pub meta_count: u32,
    pub completion_pct: u32,
    pub leaderboard_rank: u32,
}

// ─── Leaderboard ─────────────────────────────────────────────────────────────

#[derive(Clone)]
#[contracttype]
pub enum LeaderboardKey {
    PlayerScore(Address),
    PlayerPoints(Address),
    TopEntriesByCount,
    TopEntriesByPoints,
}

#[derive(Clone, Debug, PartialEq)]
#[contracttype]
pub struct AchievementLeaderboardEntry {
    pub player: Address,
    pub achievement_count: u32,
    pub achievement_points: u32,
    pub rank: u32,
}

// ─── Notifications ───────────────────────────────────────────────────────────

#[derive(Clone, Debug, PartialEq)]
#[contracttype]
pub struct AchievementNotification {
    pub player: Address,
    pub achievement_id: u64,
    pub title: String,
    pub points_earned: u32,
    pub timestamp: u64,
    pub is_secret: bool,
}

// ─── Storage Keys ────────────────────────────────────────────────────────────

#[derive(Clone)]
#[contracttype]
pub enum AchievementStorageKey {
    AchievementPlayerStats(Address),
    PlayerProgress(Address, u64),
    Notification(Address, u64),
    NotificationCount(Address),
    AchievementGlobalStats,
}

// ─── Errors ──────────────────────────────────────────────────────────────────

#[contracterror]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum AchievementsError {
    NotFound = 1,
    ProfileNotFound = 2,
    InvalidCategory = 3,
    InvalidTier = 4,
    InsufficientProgress = 5,
}

/// Bridge the catalog-layer error into the engine's error type so functions
/// like [`try_unlock`] can `?` through both layers.
impl From<AchievementsError> for AchievementError {
    fn from(e: AchievementsError) -> Self {
        match e {
            AchievementsError::NotFound => AchievementError::TemplateNotFound,
            AchievementsError::ProfileNotFound => AchievementError::ProfileNotFound,
            // The engine distinguishes "already unlocked" from "not eligible";
            // catalog lookups that fail on prerequisites land on the latter.
            AchievementsError::InsufficientProgress
            | AchievementsError::InvalidCategory
            | AchievementsError::InvalidTier => AchievementError::NotEligible,
        }
    }
}

// ─── Achievement Catalog ──────────────────────────────────────────────────────

/// Get achievement definition by ID
pub fn get_achievement_def(env: &Env, id: u64) -> Result<AchievementDef, AchievementsError> {
    if id == 0 || id > TOTAL_ACHIEVEMENTS {
        return Err(AchievementsError::NotFound);
    }

    let (title, desc, category, tier, scans, essence, ships, custom) = match id {
        // Exploration Category (1-80)
        1 => (
            "First Scan",
            "Complete your first nebula scan",
            AchievementCategory::Exploration,
            AchievementTier::Bronze,
            1,
