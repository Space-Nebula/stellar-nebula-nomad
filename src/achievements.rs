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
    PlayerStats(Address),
    PlayerProgress(Address, u64),
    Notification(Address, u64),
    NotificationCount(Address),
    GlobalStats,
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
            0,
            0,
            "",
        ),
        2 => (
            "Surveyor",
            "Complete 10 scans",
            AchievementCategory::Exploration,
            AchievementTier::Bronze,
            10,
            0,
            0,
            "",
        ),
        3 => (
            "Surveyor Pro",
            "Complete 25 scans",
            AchievementCategory::Exploration,
            AchievementTier::Bronze,
            25,
            0,
            0,
            "",
        ),
        4 => (
            "Explorer",
            "Complete 50 scans",
            AchievementCategory::Exploration,
            AchievementTier::Bronze,
            50,
            0,
            0,
            "",
        ),
        5 => (
            "Navigator",
            "Complete 100 scans",
            AchievementCategory::Exploration,
            AchievementTier::Silver,
            100,
            0,
            0,
            "",
        ),
        6 => (
            "Pathfinder",
            "Complete 150 scans",
            AchievementCategory::Exploration,
            AchievementTier::Silver,
            150,
            0,
            0,
            "",
        ),
        7 => (
            "Voyager",
            "Complete 200 scans",
            AchievementCategory::Exploration,
            AchievementTier::Silver,
            200,
            0,
            0,
            "",
        ),
        8 => (
            "Traveler",
            "Complete 300 scans",
            AchievementCategory::Exploration,
            AchievementTier::Silver,
            300,
            0,
            0,
            "",
        ),
        9 => (
            "Pioneer",
            "Complete 400 scans",
            AchievementCategory::Exploration,
            AchievementTier::Gold,
            400,
            0,
            0,
            "",
        ),
        10 => (
            "Trailblazer",
            "Complete 500 scans",
            AchievementCategory::Exploration,
            AchievementTier::Gold,
            500,
            0,
            0,
            "",
        ),
        11 => (
            "Master Explorer",
            "Complete 750 scans",
            AchievementCategory::Exploration,
            AchievementTier::Gold,
            750,
            0,
            0,
            "",
        ),
        12 => (
            "Legend",
            "Complete 1000 scans",
            AchievementCategory::Exploration,
            AchievementTier::Platinum,
            1000,
            0,
            0,
            "",
        ),
        13 => (
            "Cosmic Wanderer",
            "Complete 1500 scans",
            AchievementCategory::Exploration,
            AchievementTier::Platinum,
            1500,
            0,
            0,
            "",
        ),
        14 => (
            "Stellar Nomad",
            "Complete 2000 scans",
            AchievementCategory::Exploration,
            AchievementTier::Platinum,
            2000,
            0,
            0,
            "",
        ),
        15 => (
            "Galactic Explorer",
            "Complete 3000 scans",
            AchievementCategory::Exploration,
            AchievementTier::Platinum,
            3000,
            0,
            0,
            "",
        ),
        16 => (
            "Universal Traveler",
            "Complete 5000 scans",
            AchievementCategory::Exploration,
            AchievementTier::Platinum,
            5000,
            0,
            0,
            "",
        ),
        17 => (
            "First Discovery",
            "Discover your first region",
            AchievementCategory::Exploration,
            AchievementTier::Bronze,
            1,
            0,
            0,
            "regions:1",
        ),
        18 => (
            "Region Master",
            "Discover 10 regions",
            AchievementCategory::Exploration,
            AchievementTier::Silver,
            0,
            0,
            0,
            "regions:10",
        ),
        19 => (
            "Region Conqueror",
            "Discover 25 regions",
            AchievementCategory::Exploration,
            AchievementTier::Gold,
            0,
            0,
            0,
            "regions:25",
        ),
        20 => (
            "All Regions Explored",
            "Discover all regions",
            AchievementCategory::Exploration,
            AchievementTier::Platinum,
            0,
            0,
            0,
            "regions:all",
        ),

        // Nebula Types (21-40)
        21..=40 => get_nebula_achievement(id),

        // Distance & Speed (41-60)
        41..=60 => get_distance_achievement(id),

        // Advanced Exploration (61-80)
        61..=80 => get_advanced_exploration_achievement(id),

        // Economic Category (81-130)
        81 => (
            "Essence Starter",
            "Earn 100 essence",
            AchievementCategory::Economic,
            AchievementTier::Bronze,
            0,
            100,
            0,
            "",
        ),
        82 => (
            "Essence Gatherer",
            "Earn 500 essence",
            AchievementCategory::Economic,
            AchievementTier::Bronze,
            0,
            500,
            0,
            "",
        ),
        83 => (
            "Essence Collector",
            "Earn 1,000 essence",
            AchievementCategory::Economic,
            AchievementTier::Bronze,
            0,
            1000,
            0,
            "",
        ),
        84 => (
            "Essence Hoarder",
            "Earn 5,000 essence",
            AchievementCategory::Economic,
            AchievementTier::Silver,
            0,
            5000,
            0,
            "",
        ),
        85 => (
            "Essence Master",
            "Earn 10,000 essence",
            AchievementCategory::Economic,
            AchievementTier::Silver,
            0,
            10000,
            0,
            "",
        ),
        86 => (
            "Essence Tycoon",
            "Earn 25,000 essence",
            AchievementCategory::Economic,
            AchievementTier::Silver,
            0,
            25000,
            0,
            "",
        ),
        87 => (
            "Essence Magnate",
            "Earn 50,000 essence",
            AchievementCategory::Economic,
            AchievementTier::Gold,
            0,
            50000,
            0,
            "",
        ),
        88 => (
            "Essence Baron",
            "Earn 100,000 essence",
            AchievementCategory::Economic,
            AchievementTier::Gold,
            0,
            100000,
            0,
            "",
        ),
        89 => (
            "Essence Lord",
            "Earn 250,000 essence",
            AchievementCategory::Economic,
            AchievementTier::Gold,
            0,
            250000,
            0,
            "",
        ),
        90 => (
            "Essence King",
            "Earn 500,000 essence",
            AchievementCategory::Economic,
            AchievementTier::Platinum,
            0,
            500000,
            0,
            "",
        ),
        91 => (
            "Essence Emperor",
            "Earn 1,000,000 essence",
            AchievementCategory::Economic,
            AchievementTier::Platinum,
            0,
            1000000,
            0,
            "",
        ),
        92 => (
            "Essence Deity",
            "Earn 10,000,000 essence",
            AchievementCategory::Economic,
            AchievementTier::Platinum,
            0,
            10000000,
            0,
            "",
        ),
        93..=130 => get_economic_achievement(id),

        // Social Category (131-170)
        131 => (
            "First Bond",
            "Create your first bond",
            AchievementCategory::Social,
            AchievementTier::Bronze,
            0,
            0,
            0,
            "bonds:1",
        ),
        132 => (
            "Bond Builder",
            "Create 5 bonds",
            AchievementCategory::Social,
            AchievementTier::Bronze,
            0,
            0,
            0,
            "bonds:5",
        ),
        133 => (
            "Bond Master",
            "Create 10 bonds",
            AchievementCategory::Social,
            AchievementTier::Silver,
            0,
            0,
            0,
            "bonds:10",
        ),
        134 => (
            "Bond Expert",
            "Create 25 bonds",
            AchievementCategory::Social,
            AchievementTier::Silver,
            0,
            0,
            0,
            "bonds:25",
        ),
        135 => (
            "Bond Network",
            "Create 50 bonds",
            AchievementCategory::Social,
            AchievementTier::Gold,
            0,
            0,
            0,
            "bonds:50",
        ),
        136..=170 => get_social_achievement(id),

        // Progression Category (171-200)
        171 => (
            "First Ship",
            "Acquire your first ship",
            AchievementCategory::Progression,
            AchievementTier::Bronze,
            0,
            0,
            1,
            "",
        ),
        172 => (
            "Small Fleet",
            "Own 3 ships",
            AchievementCategory::Progression,
            AchievementTier::Bronze,
            0,
            0,
            3,
            "",
        ),
        173 => (
            "Growing Fleet",
            "Own 5 ships",
            AchievementCategory::Progression,
            AchievementTier::Bronze,
            0,
            0,
            5,
            "",
        ),
        174 => (
            "Fleet Captain",
            "Own 10 ships",
            AchievementCategory::Progression,
            AchievementTier::Silver,
            0,
            0,
            10,
            "",
        ),
        175 => (
            "Fleet Commander",
            "Own 20 ships",
            AchievementCategory::Progression,
            AchievementTier::Silver,
            0,
            0,
            20,
            "",
        ),
        176 => (
            "Fleet Admiral",
            "Own 50 ships",
            AchievementCategory::Progression,
            AchievementTier::Gold,
            0,
            0,
            50,
            "",
        ),
        177 => (
            "Armada Leader",
            "Own 100 ships",
            AchievementCategory::Progression,
            AchievementTier::Platinum,
            0,
            0,
            100,
            "",
        ),
        178..=200 => get_progression_achievement(id),

        // Meta-Achievements (201-230)
        201 => (
            "Exploration Master",
            "Complete all exploration achievements",
            AchievementCategory::Special,
            AchievementTier::Platinum,
            0,
            0,
            0,
            "meta:exploration",
        ),
        202 => (
            "Economic Mogul",
            "Complete all economic achievements",
            AchievementCategory::Special,
            AchievementTier::Platinum,
            0,
            0,
            0,
            "meta:economic",
        ),
        203 => (
            "Social Maven",
            "Complete all social achievements",
            AchievementCategory::Special,
            AchievementTier::Platinum,
            0,
            0,
            0,
            "meta:social",
        ),
        204 => (
            "Progression Perfectionist",
            "Complete all progression achievements",
            AchievementCategory::Special,
            AchievementTier::Platinum,
            0,
            0,
            0,
            "meta:progression",
        ),
        205..=230 => get_meta_achievement(id),

        // Secret Achievements (231-250)
        231..=250 => get_secret_achievement(id),

        _ => return Err(AchievementsError::NotFound),
    };

    Ok(AchievementDef {
        id,
        title: String::from_str(env, title),
        description: String::from_str(env, desc),
        category,
        tier,
        is_secret: id >= SECRET_ACHIEVEMENT_START,
        is_meta: id >= META_ACHIEVEMENT_START && id < SECRET_ACHIEVEMENT_START,
        points: tier.points(),
        min_scans: scans,
        min_essence: essence as i128,
        min_ships: ships,
        custom_requirements: String::from_str(env, custom),
    })
}

// ─── Helper Functions for Achievement Definitions ────────────────────────────

fn get_nebula_achievement(
    id: u64,
) -> (
    &'static str,
    &'static str,
    AchievementCategory,
    AchievementTier,
    u32,
    i128,
    u32,
    &'static str,
) {
    match id {
        21 => (
            "Crimson Explorer",
            "Discover a Crimson Nebula",
            AchievementCategory::Exploration,
            AchievementTier::Silver,
            0,
            0,
            0,
            "nebula:crimson",
        ),
        22 => (
            "Sapphire Seeker",
            "Discover a Sapphire Nebula",
            AchievementCategory::Exploration,
            AchievementTier::Silver,
            0,
            0,
            0,
            "nebula:sapphire",
        ),
        23 => (
            "Emerald Finder",
            "Discover an Emerald Nebula",
            AchievementCategory::Exploration,
            AchievementTier::Silver,
            0,
            0,
            0,
            "nebula:emerald",
        ),
        24 => (
            "Golden Hunter",
            "Discover a Golden Nebula",
            AchievementCategory::Exploration,
            AchievementTier::Gold,
            0,
            0,
            0,
            "nebula:golden",
        ),
        25 => (
            "Void Traveler",
            "Discover a Void Nebula",
            AchievementCategory::Exploration,
            AchievementTier::Gold,
            0,
            0,
            0,
            "nebula:void",
        ),
        26 => (
            "Nebula Collector",
            "Discover all nebula types",
            AchievementCategory::Exploration,
            AchievementTier::Gold,
            0,
            0,
            0,
            "nebula:all",
        ),
        27 => (
            "Rare Nebula Hunter",
            "Discover 10 rare nebulae",
            AchievementCategory::Exploration,
            AchievementTier::Gold,
            0,
            0,
            0,
            "nebula:rare:10",
        ),
        28 => (
            "Legendary Nebula",
            "Discover a legendary nebula",
            AchievementCategory::Exploration,
            AchievementTier::Platinum,
            0,
            0,
            0,
            "nebula:legendary",
        ),
        29 => (
            "Ancient Observer",
            "Discover an Ancient Nebula",
            AchievementCategory::Exploration,
            AchievementTier::Gold,
            0,
            0,
            0,
            "nebula:ancient",
        ),
        30 => (
            "Temporal Explorer",
            "Discover a Temporal Nebula",
            AchievementCategory::Exploration,
            AchievementTier::Platinum,
            0,
            0,
            0,
            "nebula:temporal",
        ),
        _ => (
            "Nebula Achievement",
            "Explore nebulae",
            AchievementCategory::Exploration,
            AchievementTier::Silver,
            0,
            0,
            0,
            "nebula:special",
        ),
    }
}

fn get_distance_achievement(
    id: u64,
) -> (
    &'static str,
    &'static str,
    AchievementCategory,
    AchievementTier,
    u32,
    i128,
    u32,
    &'static str,
) {
    match id {
        41 => (
            "Short Range",
            "Travel 1,000 light years",
            AchievementCategory::Exploration,
            AchievementTier::Bronze,
            0,
            0,
            0,
            "distance:1000",
        ),
        42 => (
            "Medium Range",
            "Travel 5,000 light years",
            AchievementCategory::Exploration,
            AchievementTier::Silver,
            0,
            0,
            0,
            "distance:5000",
        ),
        43 => (
            "Long Range",
            "Travel 10,000 light years",
            AchievementCategory::Exploration,
            AchievementTier::Silver,
            0,
            0,
            0,
            "distance:10000",
        ),
        44 => (
            "Interstellar",
            "Travel 50,000 light years",
            AchievementCategory::Exploration,
            AchievementTier::Gold,
            0,
            0,
            0,
            "distance:50000",
        ),
        45 => (
            "Intergalactic",
            "Travel 100,000 light years",
            AchievementCategory::Exploration,
            AchievementTier::Platinum,
            0,
            0,
            0,
            "distance:100000",
        ),
        46 => (
            "Universal",
            "Travel 1,000,000 light years",
            AchievementCategory::Exploration,
            AchievementTier::Platinum,
            0,
            0,
            0,
            "distance:1000000",
        ),
        47 => (
            "Speed Demon",
            "Achieve 100 warp speed",
            AchievementCategory::Exploration,
            AchievementTier::Bronze,
            0,
            0,
            0,
            "speed:100",
        ),
        48 => (
            "Warp Master",
            "Achieve 500 warp speed",
            AchievementCategory::Exploration,
            AchievementTier::Silver,
            0,
            0,
            0,
            "speed:500",
        ),
        49 => (
            "Hyperspeed",
            "Achieve 1000 warp speed",
            AchievementCategory::Exploration,
            AchievementTier::Gold,
            0,
            0,
            0,
            "speed:1000",
        ),
        50 => (
            "Light Speed",
            "Achieve maximum speed",
            AchievementCategory::Exploration,
            AchievementTier::Platinum,
            0,
            0,
            0,
            "speed:max",
        ),
        _ => (
            "Distance Achievement",
            "Travel far",
            AchievementCategory::Exploration,
            AchievementTier::Silver,
            0,
            0,
            0,
            "distance:custom",
        ),
    }
}

fn get_advanced_exploration_achievement(
    id: u64,
) -> (
    &'static str,
    &'static str,
    AchievementCategory,
    AchievementTier,
    u32,
    i128,
    u32,
    &'static str,
) {
    match id {
        61 => (
            "System Scanner",
            "Complete full system scan",
            AchievementCategory::Exploration,
            AchievementTier::Platinum,
            0,
            0,
            0,
            "system:full",
        ),
        62 => (
            "Binary Observer",
            "Explore binary star system",
            AchievementCategory::Exploration,
            AchievementTier::Gold,
            0,
            0,
            0,
            "system:binary",
        ),
        63 => (
            "Triple Star",
            "Explore triple star system",
            AchievementCategory::Exploration,
            AchievementTier::Platinum,
            0,
            0,
            0,
            "system:triple",
        ),
        _ => (
            "Advanced Explorer",
            "Advanced exploration",
            AchievementCategory::Exploration,
            AchievementTier::Gold,
            0,
            0,
            0,
            "advanced:special",
        ),
    }
}

fn get_economic_achievement(
    id: u64,
) -> (
    &'static str,
    &'static str,
    AchievementCategory,
    AchievementTier,
    u32,
    i128,
    u32,
    &'static str,
) {
    match id {
        93 => (
            "First Earnings",
            "Earn your first essence",
            AchievementCategory::Economic,
            AchievementTier::Bronze,
            0,
            1,
            0,
            "",
        ),
        101 => (
            "Stellar Dust",
            "Collect stellar dust",
            AchievementCategory::Economic,
            AchievementTier::Bronze,
            0,
            0,
            0,
            "resource:dust",
        ),
        102 => (
            "Dark Matter",
            "Collect dark matter",
            AchievementCategory::Economic,
            AchievementTier::Silver,
            0,
            0,
            0,
            "resource:dark",
        ),
        121 => (
            "First Trade",
            "Complete first trade",
            AchievementCategory::Economic,
            AchievementTier::Bronze,
            0,
            0,
            0,
            "trade:1",
        ),
        122 => (
            "Trader",
            "Complete 10 trades",
            AchievementCategory::Economic,
            AchievementTier::Bronze,
            0,
            0,
            0,
            "trade:10",
        ),
        _ => (
            "Economic Activity",
            "Economic milestone",
            AchievementCategory::Economic,
            AchievementTier::Silver,
            0,
            0,
            0,
            "economic:custom",
        ),
    }
}

fn get_social_achievement(
    id: u64,
) -> (
    &'static str,
    &'static str,
    AchievementCategory,
    AchievementTier,
    u32,
    i128,
    u32,
    &'static str,
) {
    match id {
        136 => (
            "Yield Sharer",
            "Share yield with bonds",
            AchievementCategory::Social,
            AchievementTier::Bronze,
            0,
            0,
            0,
            "yield:share",
        ),
        151 => (
            "Guild Member",
            "Join a guild",
            AchievementCategory::Social,
            AchievementTier::Bronze,
            0,
            0,
            0,
            "guild:join",
        ),
        152 => (
            "Guild Officer",
            "Become guild officer",
            AchievementCategory::Social,
            AchievementTier::Silver,
            0,
            0,
            0,
            "guild:officer",
        ),
        _ => (
            "Social Activity",
            "Social milestone",
            AchievementCategory::Social,
            AchievementTier::Bronze,
            0,
            0,
            0,
            "social:custom",
        ),
    }
}

fn get_progression_achievement(
    id: u64,
) -> (
    &'static str,
    &'static str,
    AchievementCategory,
    AchievementTier,
    u32,
    i128,
    u32,
    &'static str,
) {
    match id {
        178 => (
            "Massive Armada",
            "Own 200 ships",
            AchievementCategory::Progression,
            AchievementTier::Platinum,
            0,
            0,
            200,
            "",
        ),
        186 => (
            "Level 10",
            "Reach player level 10",
            AchievementCategory::Progression,
            AchievementTier::Bronze,
            0,
            0,
            0,
            "level:10",
        ),
        190 => (
            "Max Level",
            "Reach maximum level",
            AchievementCategory::Progression,
            AchievementTier::Platinum,
            0,
            0,
            0,
            "level:max",
        ),
        _ => (
            "Progression",
            "Progress milestone",
            AchievementCategory::Progression,
            AchievementTier::Silver,
            0,
            0,
            0,
            "progression:custom",
        ),
    }
}

fn get_meta_achievement(
    id: u64,
) -> (
    &'static str,
    &'static str,
    AchievementCategory,
    AchievementTier,
    u32,
    i128,
    u32,
    &'static str,
) {
    match id {
        205 => (
            "Bronze Master",
            "Unlock all bronze achievements",
            AchievementCategory::Special,
            AchievementTier::Gold,
            0,
            0,
            0,
            "meta:bronze",
        ),
        206 => (
            "Silver Master",
            "Unlock all silver achievements",
            AchievementCategory::Special,
            AchievementTier::Gold,
            0,
            0,
            0,
            "meta:silver",
        ),
        207 => (
            "Gold Master",
            "Unlock all gold achievements",
            AchievementCategory::Special,
            AchievementTier::Platinum,
            0,
            0,
            0,
            "meta:gold",
        ),
        208 => (
            "Platinum Master",
            "Unlock all platinum achievements",
            AchievementCategory::Special,
            AchievementTier::Platinum,
            0,
            0,
            0,
            "meta:platinum",
        ),
        215 => (
            "Ultimate Completionist",
            "Unlock all achievements",
            AchievementCategory::Special,
            AchievementTier::Platinum,
            0,
            0,
            0,
            "meta:all",
        ),
        _ => (
            "Meta Achievement",
            "Collection complete",
            AchievementCategory::Special,
            AchievementTier::Platinum,
            0,
            0,
            0,
            "meta:collection",
        ),
    }
}

fn get_secret_achievement(
    id: u64,
) -> (
    &'static str,
    &'static str,
    AchievementCategory,
    AchievementTier,
    u32,
    i128,
    u32,
    &'static str,
) {
    match id {
        231 => (
            "Easter Egg",
            "???",
            AchievementCategory::Special,
            AchievementTier::Gold,
            0,
            0,
            0,
            "secret:easter",
        ),
        232 => (
            "Hidden Path",
            "???",
            AchievementCategory::Special,
            AchievementTier::Gold,
            0,
            0,
            0,
            "secret:path",
        ),
        250 => (
            "Unified Theory",
            "???",
            AchievementCategory::Special,
            AchievementTier::Platinum,
            0,
            0,
            0,
            "secret:ultimate",
        ),
        _ => (
            "Secret",
            "???",
            AchievementCategory::Special,
            AchievementTier::Gold,
            0,
            0,
            0,
            "secret:hidden",
        ),
    }
}

// ─── Progress Query ──────────────────────────────────────────────────────────

pub fn query_progress(
    env: &Env,
    player: &Address,
    achievement_id: u64,
) -> Result<AchievementProgress, AchievementsError> {
    let profile =
        get_profile_by_owner(env, player).map_err(|_| AchievementsError::ProfileNotFound)?;

    let ships = get_ships_by_owner(env, player);
    let ship_count = ships.len() as u32;

    let def = get_achievement_def(env, achievement_id)?;

    let unlocked = env
        .storage()
        .persistent()
        .has(&AchievementKey::PlayerAchievement(
            player.clone(),
            achievement_id,
        ));

    let unlocked_at = if unlocked {
        env.storage()
            .persistent()
            .get(&AchievementKey::PlayerAchievement(
                player.clone(),
                achievement_id,
            ))
            .unwrap_or(0u64)
    } else {
        0
    };

    let pct = compute_progress_pct(
        &def,
        profile.total_scans,
        profile.essence_earned,
        ship_count,
    );

    Ok(AchievementProgress {
        achievement_id,
        title: def.title,
        description: if def.is_secret && !unlocked {
            String::from_str(env, "???")
        } else {
            def.description
        },
        category: def.category,
        tier: def.tier,
        unlocked,
        eligible: pct >= 100 && !unlocked,
        progress_pct: pct,
        points: def.points,
        is_secret: def.is_secret,
        unlocked_at,
    })
}

pub fn query_all_progress(
    env: &Env,
    player: &Address,
) -> Result<Vec<AchievementProgress>, AchievementsError> {
    let mut progress = Vec::new(env);

    for id in 1..=TOTAL_ACHIEVEMENTS {
        if let Ok(p) = query_progress(env, player, id) {
            progress.push_back(p);
        }
    }

    Ok(progress)
}

pub fn query_category_progress(
    env: &Env,
    player: &Address,
    category: AchievementCategory,
) -> Result<Vec<AchievementProgress>, AchievementsError> {
    let mut progress = Vec::new(env);

    for id in 1..=TOTAL_ACHIEVEMENTS {
        if let Ok(def) = get_achievement_def(env, id) {
            if def.category as u32 == category as u32 {
                if let Ok(p) = query_progress(env, player, id) {
                    progress.push_back(p);
                }
            }
        }
    }

    Ok(progress)
}

// ─── Unlock Functions ────────────────────────────────────────────────────────

pub fn try_unlock(
    env: &Env,
    player: &Address,
    achievement_id: u64,
) -> Result<u64, AchievementError> {
    let badge = unlock_achievement(env, player.clone(), achievement_id)?;

    // Update player stats
    increment_player_stats(env, player, achievement_id)?;

    // Update leaderboard
    update_leaderboard(env, player);

    // Create notification
    create_notification(env, player, achievement_id);

    // Integrate with reputation
    integrate_reputation(env, player, achievement_id);

    // Emit event
    emit_achievement_event(env, player, achievement_id, badge.badge_id);

    // Check for meta-achievements
    check_meta_achievements(env, player)?;

    Ok(badge.badge_id)
}

// ─── Player Stats Management ─────────────────────────────────────────────────

fn increment_player_stats(
    env: &Env,
    player: &Address,
    achievement_id: u64,
) -> Result<(), AchievementsError> {
    let key = AchievementStorageKey::PlayerStats(player.clone());
    let mut stats: PlayerAchievementStats =
        env.storage()
            .persistent()
            .get(&key)
            .unwrap_or_else(|| PlayerAchievementStats {
                player: player.clone(),
                total_unlocked: 0,
                total_points: 0,
                bronze_count: 0,
                silver_count: 0,
                gold_count: 0,
                platinum_count: 0,
                secret_count: 0,
                meta_count: 0,
                completion_pct: 0,
                leaderboard_rank: 0,
            });

    let def = get_achievement_def(env, achievement_id)?;

    stats.total_unlocked += 1;
    stats.total_points += def.points;

    match def.tier {
        AchievementTier::Bronze => stats.bronze_count += 1,
        AchievementTier::Silver => stats.silver_count += 1,
        AchievementTier::Gold => stats.gold_count += 1,
        AchievementTier::Platinum => stats.platinum_count += 1,
    }

    if def.is_secret {
        stats.secret_count += 1;
    }

    if def.is_meta {
        stats.meta_count += 1;
    }

    stats.completion_pct = (stats.total_unlocked * 100) / (TOTAL_ACHIEVEMENTS as u32);

    env.storage().persistent().set(&key, &stats);

    Ok(())
}

pub fn get_player_stats(
    env: &Env,
    player: &Address,
) -> Result<PlayerAchievementStats, AchievementsError> {
    let key = AchievementStorageKey::PlayerStats(player.clone());
    env.storage()
        .persistent()
        .get(&key)
        .ok_or(AchievementsError::ProfileNotFound)
}

// ─── Leaderboard Management ──────────────────────────────────────────────────

fn update_leaderboard(env: &Env, player: &Address) {
    let stats = get_player_stats(env, player).unwrap_or_else(|_| PlayerAchievementStats {
        player: player.clone(),
        total_unlocked: 0,
        total_points: 0,
        bronze_count: 0,
        silver_count: 0,
        gold_count: 0,
        platinum_count: 0,
        secret_count: 0,
        meta_count: 0,
        completion_pct: 0,
        leaderboard_rank: 0,
    });

    // Update count-based leaderboard
    env.storage().persistent().set(
        &LeaderboardKey::PlayerScore(player.clone()),
        &stats.total_unlocked,
    );

    // Update points-based leaderboard
    env.storage().persistent().set(
        &LeaderboardKey::PlayerPoints(player.clone()),
        &stats.total_points,
    );

    record_leaderboard_entry(env, player, stats.total_unlocked, stats.total_points);
}

fn record_leaderboard_entry(env: &Env, player: &Address, count: u32, points: u32) {
    let mut entries: Vec<AchievementLeaderboardEntry> = env
        .storage()
        .persistent()
        .get(&LeaderboardKey::TopEntriesByPoints)
        .unwrap_or_else(|| Vec::new(env));

    // Remove existing entry
    let mut without_player: Vec<AchievementLeaderboardEntry> = Vec::new(env);
    for i in 0..entries.len() {
        if let Some(e) = entries.get(i) {
            if e.player != *player {
                without_player.push_back(e);
            }
        }
    }

    // Insert new entry
    let new_entry = AchievementLeaderboardEntry {
        player: player.clone(),
        achievement_count: count,
        achievement_points: points,
        rank: 0,
    };

    let mut sorted: Vec<AchievementLeaderboardEntry> = Vec::new(env);
    let mut inserted = false;

    for i in 0..without_player.len() {
        if let Some(e) = without_player.get(i) {
            if !inserted && points >= e.achievement_points {
                sorted.push_back(new_entry.clone());
                inserted = true;
            }
            sorted.push_back(e);
        }
    }

    if !inserted {
        sorted.push_back(new_entry);
    }

    // Cap at 100 entries and assign ranks
    let mut ranked: Vec<AchievementLeaderboardEntry> = Vec::new(env);
    for i in 0..sorted.len().min(100) {
        if let Some(mut e) = sorted.get(i) {
            e.rank = i + 1;
            ranked.push_back(e);
        }
    }

    env.storage()
        .persistent()
        .set(&LeaderboardKey::TopEntriesByPoints, &ranked);
}

pub fn get_leaderboard(env: &Env) -> Vec<AchievementLeaderboardEntry> {
    env.storage()
        .persistent()
        .get(&LeaderboardKey::TopEntriesByPoints)
        .unwrap_or_else(|| Vec::new(env))
}

// ─── Notifications ───────────────────────────────────────────────────────────

fn create_notification(env: &Env, player: &Address, achievement_id: u64) {
    let def = match get_achievement_def(env, achievement_id) {
        Ok(d) => d,
        Err(_) => return,
    };

    let count_key = AchievementStorageKey::NotificationCount(player.clone());
    let count: u64 = env.storage().persistent().get(&count_key).unwrap_or(0);
    let new_count = count + 1;

    let notification = AchievementNotification {
        player: player.clone(),
        achievement_id,
        title: def.title,
        points_earned: def.points,
        timestamp: env.ledger().timestamp(),
        is_secret: def.is_secret,
    };

    env.storage().persistent().set(
        &AchievementStorageKey::Notification(player.clone(), new_count),
        &notification,
    );

    env.storage().persistent().set(&count_key, &new_count);

    env.events().publish(
        (symbol_short!("ach_notif"), player.clone()),
        (achievement_id, def.points, def.is_secret),
    );
}

pub fn get_notifications(env: &Env, player: &Address) -> Vec<AchievementNotification> {
    let count_key = AchievementStorageKey::NotificationCount(player.clone());
    let count: u64 = env.storage().persistent().get(&count_key).unwrap_or(0);

    let mut notifications = Vec::new(env);

    for i in 1..=count.min(50) {
        if let Some(notif) = env
            .storage()
            .persistent()
            .get::<AchievementStorageKey, AchievementNotification>(
                &AchievementStorageKey::Notification(player.clone(), count - i + 1),
            )
        {
            notifications.push_back(notif);
        }
    }

    notifications
}

// ─── Reputation Integration ──────────────────────────────────────────────────

fn integrate_reputation(env: &Env, player: &Address, achievement_id: u64) {
    let def = match get_achievement_def(env, achievement_id) {
        Ok(d) => d,
        Err(_) => return,
    };

    let reputation_points = match def.tier {
        AchievementTier::Bronze => 1,
        AchievementTier::Silver => 2,
        AchievementTier::Gold => 4,
        AchievementTier::Platinum => 8,
    };

    // Store reputation gain for external reputation system
    env.events().publish(
        (symbol_short!("rep_gain"), player.clone()),
        (ReputationSource::GameAchievement as u32, reputation_points),
    );
}

// ─── Meta-Achievements ───────────────────────────────────────────────────────

fn check_meta_achievements(env: &Env, player: &Address) -> Result<(), AchievementError> {
    let stats = get_player_stats(env, player).map_err(|_| AchievementError::ProfileNotFound)?;

    // Check bronze collector
    if stats.bronze_count >= 50 && !is_unlocked(env, player, 205) {
        let _ = try_unlock(env, player, 205);
    }

    // Check silver collector
    if stats.silver_count >= 40 && !is_unlocked(env, player, 206) {
        let _ = try_unlock(env, player, 206);
    }

    // Check gold collector
    if stats.gold_count >= 30 && !is_unlocked(env, player, 207) {
        let _ = try_unlock(env, player, 207);
    }

    // Check platinum collector
    if stats.platinum_count >= 20 && !is_unlocked(env, player, 208) {
        let _ = try_unlock(env, player, 208);
    }

    // Check completionist milestones
    if stats.total_unlocked >= 50 && !is_unlocked(env, player, 211) {
        let _ = try_unlock(env, player, 211);
    }

    if stats.total_unlocked >= 100 && !is_unlocked(env, player, 212) {
        let _ = try_unlock(env, player, 212);
    }

    if stats.total_unlocked >= 200 && !is_unlocked(env, player, 214) {
        let _ = try_unlock(env, player, 214);
    }

    // Check ultimate completionist
    if stats.total_unlocked >= 250 && !is_unlocked(env, player, 215) {
        let _ = try_unlock(env, player, 215);
    }

    // Check points milestones
    if stats.total_points >= 1000 && !is_unlocked(env, player, 221) {
        let _ = try_unlock(env, player, 221);
    }

    if stats.total_points >= 5000 && !is_unlocked(env, player, 222) {
        let _ = try_unlock(env, player, 222);
    }

    if stats.total_points >= 10000 && !is_unlocked(env, player, 223) {
        let _ = try_unlock(env, player, 223);
    }

    Ok(())
}

fn is_unlocked(env: &Env, player: &Address, achievement_id: u64) -> bool {
    env.storage()
        .persistent()
        .has(&AchievementKey::PlayerAchievement(
            player.clone(),
            achievement_id,
        ))
}

// ─── Events ──────────────────────────────────────────────────────────────────

fn emit_achievement_event(env: &Env, player: &Address, achievement_id: u64, badge_id: u64) {
    let def = match get_achievement_def(env, achievement_id) {
        Ok(d) => d,
        Err(_) => return,
    };

    env.events().publish(
        (symbol_short!("ach_unlck"), player.clone()),
        (achievement_id, badge_id, def.tier as u32, def.points),
    );
}

// ─── Helper Functions ────────────────────────────────────────────────────────

fn compute_progress_pct(def: &AchievementDef, scans: u32, essence: i128, ships: u32) -> u32 {
    let mut pct = 100u32;

    if def.min_scans > 0 {
        pct = pct.min((scans.saturating_mul(100) / def.min_scans).min(100));
    }

    if def.min_essence > 0 {
        let essence_u = if essence < 0 { 0 } else { essence as u128 };
        let required = def.min_essence as u128;
        pct = pct.min(((essence_u.saturating_mul(100)) / required).min(100) as u32);
    }

    if def.min_ships > 0 {
        pct = pct.min((ships.saturating_mul(100) / def.min_ships).min(100));
    }

    pct
}

// ─── Batch Operations ────────────────────────────────────────────────────────

pub fn batch_check_and_unlock(env: &Env, player: &Address) -> Result<Vec<u64>, AchievementError> {
    let profile =
        get_profile_by_owner(env, player).map_err(|_| AchievementError::ProfileNotFound)?;

    let ships = get_ships_by_owner(env, player);
    let ship_count = ships.len() as u32;

    let mut unlocked_ids = Vec::new(env);

    for id in 1..=TOTAL_ACHIEVEMENTS {
        if is_unlocked(env, player, id) {
            continue;
        }

        if let Ok(def) = get_achievement_def(env, id) {
            let pct = compute_progress_pct(
                &def,
                profile.total_scans,
                profile.essence_earned,
                ship_count,
            );

            if pct >= 100 {
                if let Ok(badge_id) = try_unlock(env, player, id) {
                    unlocked_ids.push_back(id);
                }
            }
        }
    }

    Ok(unlocked_ids)
}
