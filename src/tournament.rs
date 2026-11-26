//! Tournament system (Issues #284, #537, #587).
//!
//! Competitive PvP events in four formats: single elimination, double
//! elimination, round robin and Swiss. They support entry-fee registration,
//! seeded brackets with byes, match scheduling with deadlines, check-ins and
//! automatic forfeits, multi-pool prize distribution, an organizer/admin
//! control panel and a dispute workflow.
//!
//! * **Brackets** come from [`crate::bracket_generator`], which is pure.
//!   This module stores the matches, tracks who fills each slot and moves
//!   results forward.
//! * **Matches are real PvP combats.** A ready match starts a
//!   `pvp_combat::CombatState` (automatically, or via [`start_match`]), so
//!   spectators can follow it through `pvp_combat::add_spectator`, and
//!   [`report_result`] pulls the winner from the finished combat.
//! * **Money never leaves the resource ledger.** Entry fees and sponsor prizes
//!   move into the contract's own balance with `resource_minter::move_balance`
//!   and are paid out the same way, so total supply never changes.
//!
//! See `docs/TOURNAMENT_GUIDE.md` for the full lifecycle.
//!
//! ## Lifecycle
//!
//! ```text
//! create ──► "reg" ──start──► "active" ──last match──► "final" ──finalize──► "done"
//!               │                │                        │ (dispute window)
//!               └──── cancel ────┴────────► "cancelled" ◄─┘
//! ```
//!
//! With `dispute_window_secs == 0` (the legacy [`create_tournament`] default)
//! a tournament goes straight from "active" to "done".

use soroban_sdk::{contracterror, contracttype, symbol_short, Address, Env, Symbol, Vec};

use crate::access_control;
use crate::bracket_generator::{
    self as bg, MatchSpec, NO_MATCH, SIDE_GRAND_FINAL, SIDE_GRAND_FINAL_RESET, SIDE_POOL,
    SIDE_WINNERS,
};
pub use crate::bracket_generator::{SlotSource, TournamentFormat};
use crate::pvp_combat;
use crate::resource_minter::{self, ResourceType};

// ── Error ───────────────────────────────────────────────────────────────────

#[contracterror]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum TournamentError {
    /// `max_players` is outside the range the chosen format accepts (see
    /// [`create_tournament_with_config`]).
    InvalidPlayerCount = 1,
    /// Prize distribution basis points don't sum to <= 10_000, or the list
    /// is longer than the number of possible placements.
    InvalidPrizeDistribution = 2,
    /// Tournament not found.
    TournamentNotFound = 3,
    /// Not authorized (not the organizer/admin).
    Unauthorized = 4,
    /// Registration window has closed.
    RegistrationClosed = 5,
    /// Player already registered for this tournament.
    AlreadyRegistered = 6,
    /// Tournament already at max capacity.
    TournamentFull = 7,
    /// Fewer than 2 players registered: no bracket can be built.
    NotEnoughRegistrants = 8,
    /// Tournament isn't in the registration phase.
    NotInRegistration = 9,
    /// Tournament isn't currently active (bracket in progress).
    TournamentNotActive = 10,
    /// Round/match index out of range for this tournament's bracket.
    InvalidMatch = 11,
    /// The match can't take this action yet: its players aren't known, or
    /// its combat hasn't finished.
    MatchNotReady = 12,
    /// This match already has a recorded winner.
    MatchAlreadyResolved = 13,
    /// Player isn't part of this specific match.
    PlayerNotInMatch = 14,
    /// Tournament already completed or cancelled.
    TournamentAlreadyDone = 15,
    /// Caller exceeded the rate limit for this operation (Issue #610).
    RateLimitExceeded = 16,
    /// Registration window is outside the allowed bounds (Issue #611).
    InvalidRegistrationWindow = 17,
    /// Player (or sponsor) cannot cover the amount (Issue #611).
    InsufficientFunds = 18,
    /// A scheduling, dispute or time-zone parameter is out of range.
    InvalidConfig = 19,
    /// The match deadline (or dispute window) has not passed yet.
    DeadlineNotReached = 20,
    /// Forfeit can't be claimed: both players checked in, or a combat is
    /// already running.
    ForfeitNotAllowed = 21,
    /// Check-in is only open from shortly before kick-off until the deadline.
    CheckInClosed = 22,
    /// Disputes are disabled for this tournament or the window has passed.
    DisputeWindowClosed = 23,
    /// This match already has an open dispute.
    DisputeAlreadyOpen = 24,
    /// No open dispute for this match.
    DisputeNotFound = 25,
    /// A match feeding this one is under dispute; wait for the ruling.
    MatchDisputed = 26,
    /// Open disputes block finalization.
    DisputesOpen = 27,
    /// The tournament isn't waiting to be finalized.
    NotFinalizable = 28,
    /// The named winner isn't one of the match's players.
    InvalidWinner = 29,
    /// Address isn't registered in this tournament.
    NotRegistered = 30,
    /// The match isn't scheduled to start yet.
    TooEarly = 31,
    /// A prize or refund transfer failed (ledger inconsistency).
    PrizeTransferFailed = 32,
    /// Amount must be greater than zero.
    InvalidAmount = 33,
    /// The result already fed a decided match and can no longer change.
    ResultLocked = 34,
}

impl crate::error_standard::StandardContractError for TournamentError {
    fn descriptor(self) -> crate::error_standard::ErrorDescriptor {
        crate::error_standard::ErrorDescriptor {
            module: "tournament",
            code: self as u32,
            kind: crate::error_standard::ErrorKind::Validation,
            retryable: false,
        }
    }
}


impl From<crate::rate_limiter::RateLimitError> for TournamentError {
    fn from(_: crate::rate_limiter::RateLimitError) -> Self {
        TournamentError::RateLimitExceeded
    }
}

// ── Storage Keys ────────────────────────────────────────────────────────────

#[contracttype]
#[derive(Clone)]
pub enum TournamentDataKey {
    Tournament(u64),
    TournamentCounter,
    /// Registrants in registration order.
    Registrants(u64),
    /// Registrants re-ordered by seed (ELO, descending) at start.
    TournSeeds(u64),
    /// First match id of each stored stage (a stage = one round of one side).
    TournStageStarts(u64),
    /// One stage's matches: (tournament_id, stage) -> Vec<TournamentMatch>.
    TournStage(u64, u32),
    /// Dispute on one match: (tournament_id, match_id) -> MatchDispute.
    TournDispute(u64, u32),
    /// Sponsor prize contributions.
    TournSponsors(u64),
    /// Players removed by an admin.
    TournDisqualified(u64),
    /// Final placements, best first (written at completion).
    TournStandings(u64),
    /// Championships won by a player, across all tournaments.
    TournTitles(Address),
}

// ── Data Types ──────────────────────────────────────────────────────────────

#[contracttype]
#[derive(Clone, Debug)]
pub struct Tournament {
    pub tournament_id: u64,
    pub organizer: Address,
    /// Resource the entry fee (and the entry-fee prize pool) is paid in.
    pub resource_type: ResourceType,
    pub entry_fee: u64,
    /// Registration cap.
    pub max_players: u32,
    /// "reg" | "active" | "final" | "done" | "cancelled"
    pub status: Symbol,
    pub created_at: u64,
    pub registration_deadline: u64,
    /// Highest scheduling slot with a ready match (Swiss: current round).
    pub current_round: u32,
    /// Total scheduling slots in the bracket (excluding an unplayed reset).
    pub total_rounds: u32,
    /// Entry fees collected (paid out by `prize_distribution_bps`).
    pub prize_pool: u64,
    /// Basis-point share of every prize pool per placement, best-first
    /// (index 0 = champion). Must sum to <= 10_000; any remainder goes back
    /// to whoever funded that pool (organizer for entry fees, the sponsor
    /// for sponsor pools).
    pub prize_distribution_bps: Vec<u32>,
    // ── Issue #587 ──
    pub format: TournamentFormat,
    /// Requested first-round kick-off (UTC, 0 = whenever the organizer starts).
    pub first_round_at: u64,
    /// Actual first-round kick-off (UTC), set by `start_tournament`.
    pub schedule_base: u64,
    /// Gap between consecutive scheduling slots.
    pub round_interval_secs: u64,
    /// Time a ready match has to be played before forfeits can be claimed.
    pub match_window_secs: u64,
    /// How long after a result a player may dispute it (0 = no disputes).
    pub dispute_window_secs: u64,
    /// Organizer's display time zone. All stored times are UTC.
    pub timezone_offset_minutes: i32,
    /// Start a PvP combat as soon as a match has both players.
    pub auto_start_combats: bool,
    pub match_count: u32,
    /// Matches not yet decided (or skipped).
    pub pending_matches: u32,
    pub open_disputes: u32,
    /// When the last match was decided (start of the dispute window).
    pub completed_at: u64,
    pub champion: Option<Address>,
}

/// Parameters for [`create_tournament_with_config`].
#[contracttype]
#[derive(Clone, Debug)]
pub struct TournamentConfig {
    pub format: TournamentFormat,
    pub resource_type: ResourceType,
    pub entry_fee: u64,
    pub max_players: u32,
    pub registration_window_secs: u64,
    pub prize_distribution_bps: Vec<u32>,
    /// UTC kick-off for round 1; 0 = when started. Must not be before the
    /// registration deadline.
    pub first_round_at: u64,
    pub round_interval_secs: u64,
    pub match_window_secs: u64,
    pub dispute_window_secs: u64,
    pub timezone_offset_minutes: i32,
    pub auto_start_combats: bool,
}

/// Match lifecycle: waiting for its inputs.
pub const MATCH_PENDING: u32 = 0;
/// Both players known; can be checked into, started, forfeited.
pub const MATCH_READY: u32 = 1;
/// A PvP combat is running.
pub const MATCH_LIVE: u32 = 2;
/// Decided (including byes).
pub const MATCH_DONE: u32 = 3;
/// Not needed (an unplayed grand-final reset).
pub const MATCH_SKIPPED: u32 = 4;

/// How a match was decided.
pub const OUTCOME_NONE: u32 = 0;
pub const OUTCOME_COMBAT: u32 = 1;
pub const OUTCOME_FORFEIT: u32 = 2;
pub const OUTCOME_BYE: u32 = 3;
pub const OUTCOME_ADMIN: u32 = 4;
pub const OUTCOME_DISQUALIFIED: u32 = 5;

#[contracttype]
#[derive(Clone, Debug)]
pub struct TournamentMatch {
    pub match_id: u32,
    pub round: u32,
    /// `bracket_generator::SIDE_*`.
    pub side: u32,
    /// Scheduling slot (1-based).
    pub sched: u32,
    pub slot1: SlotSource,
    pub slot2: SlotSource,
    pub player1: Option<Address>,
    pub player2: Option<Address>,
    /// Whether each slot's source has been decided (a decided slot can still
    /// be empty: that's a bye).
    pub settled1: bool,
    pub settled2: bool,
    pub winner: Option<Address>,
    pub loser: Option<Address>,
    pub status: u32,
    pub outcome: u32,
    pub combat_id: Option<u64>,
    /// Planned kick-off (UTC).
    pub scheduled_at: u64,
    /// When both players became known.
    pub ready_at: u64,
    /// After this, forfeits can be claimed.
    pub deadline: u64,
    pub checked_in1: bool,
    pub checked_in2: bool,
    pub resolved_at: u64,
    pub winner_to: u32,
    pub winner_slot: u32,
    pub loser_to: u32,
    pub loser_slot: u32,
}

/// Legacy single-elimination view of one match (see [`get_bracket_round`]).
#[contracttype]
#[derive(Clone, Debug)]
pub struct BracketMatch {
    /// `None` on either side means a bye.
    pub player1: Option<Address>,
    pub player2: Option<Address>,
    pub winner: Option<Address>,
    pub combat_id: Option<u64>,
}

#[contracttype]
#[derive(Clone, Debug)]
pub struct MatchDispute {
    pub match_id: u32,
    pub raised_by: Address,
    pub reason: Symbol,
    pub raised_at: u64,
    pub open: bool,
    pub overturned: bool,
}

/// A sponsor-funded prize pool. Several may exist, in any resource type.
#[contracttype]
#[derive(Clone, Debug)]
pub struct SponsorContribution {
    pub sponsor: Address,
    pub resource_type: ResourceType,
    pub amount: u64,
}

/// A player's live record in a tournament.
#[contracttype]
#[derive(Clone, Debug)]
pub struct Standing {
    pub player: Address,
    pub seed: u32,
    pub wins: u32,
    pub losses: u32,
    /// Swiss: Buchholz (sum of opponents' wins). Round robin:
    /// Sonneborn–Berger (sum of beaten opponents' wins). Elimination: 0.
    pub tiebreak: u32,
    pub disqualified: bool,
}

/// One-call summary for the organizer dashboard.
#[contracttype]
#[derive(Clone, Debug)]
pub struct TournamentOverview {
    pub tournament_id: u64,
    pub status: Symbol,
    pub format: TournamentFormat,
    pub registrants: u32,
    pub disqualified: u32,
    pub matches_total: u32,
    pub matches_decided: u32,
    pub matches_live: u32,
    pub matches_overdue: u32,
    pub open_disputes: u32,
    pub current_round: u32,
    pub total_rounds: u32,
    pub prize_pool: u64,
    pub sponsor_pools: u32,
}

// ── Constants ───────────────────────────────────────────────────────────────

pub const MIN_TOURNAMENT_PLAYERS: u32 = 4;
pub const MAX_TOURNAMENT_PLAYERS: u32 = 64;
/// Shortest and longest accepted registration window.
pub const MIN_REGISTRATION_WINDOW_SECS: u64 = 60;
pub const MAX_REGISTRATION_WINDOW_SECS: u64 = 30 * 24 * 60 * 60;
/// Match window bounds.
pub const MIN_MATCH_WINDOW_SECS: u64 = 5 * 60;
pub const MAX_MATCH_WINDOW_SECS: u64 = 7 * 24 * 60 * 60;
/// Window given to matches by the legacy [`create_tournament`].
pub const DEFAULT_MATCH_WINDOW_SECS: u64 = 24 * 60 * 60;
pub const MAX_ROUND_INTERVAL_SECS: u64 = 7 * 24 * 60 * 60;
pub const MAX_DISPUTE_WINDOW_SECS: u64 = 7 * 24 * 60 * 60;
/// Players may check in this long before kick-off.
pub const CHECK_IN_LEAD_SECS: u64 = 15 * 60;
/// UTC-12:00 .. UTC+14:00.
pub const MIN_TZ_OFFSET_MINUTES: i32 = -12 * 60;
pub const MAX_TZ_OFFSET_MINUTES: i32 = 14 * 60;

fn st_reg() -> Symbol {
    symbol_short!("reg")
}
fn st_active() -> Symbol {
    symbol_short!("active")
}
fn st_final() -> Symbol {
    symbol_short!("final")
}
fn st_done() -> Symbol {
    symbol_short!("done")
}
fn st_cancelled() -> Symbol {
    symbol_short!("cancelled")
}

// ── Creation ────────────────────────────────────────────────────────────────

/// Create a single-elimination tournament (legacy entry point, Issue #284).
///
/// Combats start automatically, matches get [`DEFAULT_MATCH_WINDOW_SECS`],
/// and there is no dispute window, so the tournament finishes and pays out as
/// soon as the final is decided. `max_players` must be a power of two in
/// `[4, 64]`.
pub fn create_tournament(
    env: &Env,
    organizer: &Address,
    resource_type: ResourceType,
    entry_fee: u64,
    max_players: u32,
    registration_window_secs: u64,
    prize_distribution_bps: Vec<u32>,
) -> Result<u64, TournamentError> {
    create_tournament_with_config(
        env,
        organizer,
        TournamentConfig {
            format: TournamentFormat::SingleElimination,
            resource_type,
            entry_fee,
            max_players,
            registration_window_secs,
            prize_distribution_bps,
            first_round_at: 0,
            round_interval_secs: 0,
            match_window_secs: DEFAULT_MATCH_WINDOW_SECS,
            dispute_window_secs: 0,
            timezone_offset_minutes: 0,
            auto_start_combats: true,
        },
    )
}

fn valid_player_cap(format: TournamentFormat, max_players: u32) -> bool {
    match format {
        TournamentFormat::SingleElimination | TournamentFormat::DoubleElimination => {
            bg::is_power_of_two(max_players)
                && (MIN_TOURNAMENT_PLAYERS..=MAX_TOURNAMENT_PLAYERS).contains(&max_players)
        }
        TournamentFormat::RoundRobin => {
            (MIN_TOURNAMENT_PLAYERS..=bg::MAX_ROUND_ROBIN_PLAYERS).contains(&max_players)
        }
        TournamentFormat::Swiss => {
            (MIN_TOURNAMENT_PLAYERS..=MAX_TOURNAMENT_PLAYERS).contains(&max_players)
        }
    }
}

/// Create a tournament in any format (Issue #587).
///
/// Player caps: elimination formats take a power of two in `[4, 64]` (fields
/// that don't fill it get byes); round robin takes `4..=16`; Swiss `4..=64`.
pub fn create_tournament_with_config(
    env: &Env,
    organizer: &Address,
    config: TournamentConfig,
) -> Result<u64, TournamentError> {
    organizer.require_auth();

    if !valid_player_cap(config.format, config.max_players) {
        return Err(TournamentError::InvalidPlayerCount);
    }
    crate::input_validation::check_bps_distribution(
        &config.prize_distribution_bps,
        config.max_players,
    )
    .map_err(|_| TournamentError::InvalidPrizeDistribution)?;

    let now = env.ledger().timestamp();
    let registration_deadline = crate::input_validation::checked_deadline(
        now,
        config.registration_window_secs,
        MIN_REGISTRATION_WINDOW_SECS,
        MAX_REGISTRATION_WINDOW_SECS,
    )
    .map_err(|_| TournamentError::InvalidRegistrationWindow)?;

    if !(MIN_MATCH_WINDOW_SECS..=MAX_MATCH_WINDOW_SECS).contains(&config.match_window_secs)
        || config.round_interval_secs > MAX_ROUND_INTERVAL_SECS
        || config.dispute_window_secs > MAX_DISPUTE_WINDOW_SECS
        || !(MIN_TZ_OFFSET_MINUTES..=MAX_TZ_OFFSET_MINUTES)
            .contains(&config.timezone_offset_minutes)
    {
        return Err(TournamentError::InvalidConfig);
    }
    if config.first_round_at != 0
        && (config.first_round_at < registration_deadline
            || config.first_round_at - registration_deadline > MAX_REGISTRATION_WINDOW_SECS)
    {
        return Err(TournamentError::InvalidConfig);
    }

    let counter: u64 = env
        .storage()
        .persistent()
        .get(&TournamentDataKey::TournamentCounter)
        .unwrap_or(0);
    let tournament_id = counter + 1;
    env.storage()
        .persistent()
        .set(&TournamentDataKey::TournamentCounter, &tournament_id);

    let tournament = Tournament {
        tournament_id,
        organizer: organizer.clone(),
        resource_type: config.resource_type,
        entry_fee: config.entry_fee,
        max_players: config.max_players,
        status: st_reg(),
        created_at: now,
        registration_deadline,
        current_round: 0,
        total_rounds: 0,
        prize_pool: 0,
        prize_distribution_bps: config.prize_distribution_bps,
        format: config.format,
        first_round_at: config.first_round_at,
        schedule_base: 0,
        round_interval_secs: config.round_interval_secs,
        match_window_secs: config.match_window_secs,
        dispute_window_secs: config.dispute_window_secs,
        timezone_offset_minutes: config.timezone_offset_minutes,
        auto_start_combats: config.auto_start_combats,
        match_count: 0,
        pending_matches: 0,
        open_disputes: 0,
        completed_at: 0,
        champion: None,
    };

    save_tournament(env, &tournament);
    env.storage().persistent().set(
        &TournamentDataKey::Registrants(tournament_id),
        &Vec::<Address>::new(env),
    );

    env.events().publish(
        (symbol_short!("tourn"), symbol_short!("created")),
        (tournament_id, organizer.clone(), config.max_players),
    );

    Ok(tournament_id)
}

// ── Storage helpers ─────────────────────────────────────────────────────────

pub fn get_tournament(env: &Env, tournament_id: u64) -> Result<Tournament, TournamentError> {
    env.storage()
        .persistent()
        .get(&TournamentDataKey::Tournament(tournament_id))
        .ok_or(TournamentError::TournamentNotFound)
}

fn save_tournament(env: &Env, t: &Tournament) {
    env.storage()
        .persistent()
        .set(&TournamentDataKey::Tournament(t.tournament_id), t);
}

pub fn get_registrants(env: &Env, tournament_id: u64) -> Vec<Address> {
    env.storage()
        .persistent()
        .get(&TournamentDataKey::Registrants(tournament_id))
        .unwrap_or(Vec::new(env))
}

/// Registrants in seed order (empty before the tournament starts).
pub fn get_seeds(env: &Env, tournament_id: u64) -> Vec<Address> {
    env.storage()
        .persistent()
        .get(&TournamentDataKey::TournSeeds(tournament_id))
        .unwrap_or(Vec::new(env))
}

pub fn get_disqualified(env: &Env, tournament_id: u64) -> Vec<Address> {
    env.storage()
        .persistent()
        .get(&TournamentDataKey::TournDisqualified(tournament_id))
        .unwrap_or(Vec::new(env))
}

pub fn get_sponsors(env: &Env, tournament_id: u64) -> Vec<SponsorContribution> {
    env.storage()
        .persistent()
        .get(&TournamentDataKey::TournSponsors(tournament_id))
        .unwrap_or(Vec::new(env))
}

pub fn get_dispute(env: &Env, tournament_id: u64, match_id: u32) -> Option<MatchDispute> {
    env.storage()
        .persistent()
        .get(&TournamentDataKey::TournDispute(tournament_id, match_id))
}

/// Final placements, best first. Empty until the last match is decided;
/// recomputed at finalization if a dispute changed a result.
pub fn get_final_standings(env: &Env, tournament_id: u64) -> Vec<Address> {
    env.storage()
        .persistent()
        .get(&TournamentDataKey::TournStandings(tournament_id))
        .unwrap_or(Vec::new(env))
}

/// Number of tournaments `player` has won (the "title" prize).
pub fn get_player_titles(env: &Env, player: &Address) -> u32 {
    env.storage()
        .persistent()
        .get(&TournamentDataKey::TournTitles(player.clone()))
        .unwrap_or(0)
}

fn stage_starts(env: &Env, tid: u64) -> Vec<u32> {
    env.storage()
        .persistent()
        .get(&TournamentDataKey::TournStageStarts(tid))
        .unwrap_or(Vec::new(env))
}

fn load_stage(env: &Env, tid: u64, stage: u32) -> Vec<TournamentMatch> {
    env.storage()
        .persistent()
        .get(&TournamentDataKey::TournStage(tid, stage))
        .unwrap_or(Vec::new(env))
}

/// (stage, index within stage) of a match id.
fn locate(env: &Env, tid: u64, match_id: u32) -> Option<(u32, u32)> {
    let starts = stage_starts(env, tid);
    let mut found: Option<(u32, u32)> = None;
    for (stage, start) in starts.iter().enumerate() {
        if start <= match_id {
            found = Some((stage as u32, match_id - start));
        } else {
            break;
        }
    }
    found
}

pub fn get_match(
    env: &Env,
    tournament_id: u64,
    match_id: u32,
) -> Result<TournamentMatch, TournamentError> {
    let (stage, idx) = locate(env, tournament_id, match_id).ok_or(TournamentError::InvalidMatch)?;
    load_stage(env, tournament_id, stage)
        .get(idx)
        .ok_or(TournamentError::InvalidMatch)
}

fn save_match(env: &Env, tid: u64, m: &TournamentMatch) {
    if let Some((stage, idx)) = locate(env, tid, m.match_id) {
        let mut matches = load_stage(env, tid, stage);
        if idx < matches.len() {
            matches.set(idx, m.clone());
            env.storage()
                .persistent()
                .set(&TournamentDataKey::TournStage(tid, stage), &matches);
        }
    }
}

fn append_stage(env: &Env, tid: u64, matches: &Vec<TournamentMatch>) {
    let Some(first) = matches.get(0) else {
        return;
    };
    let mut starts = stage_starts(env, tid);
    let stage = starts.len();
    starts.push_back(first.match_id);
    env.storage()
        .persistent()
        .set(&TournamentDataKey::TournStageStarts(tid), &starts);
    env.storage()
        .persistent()
        .set(&TournamentDataKey::TournStage(tid, stage), matches);
}

/// Every match of a tournament, in id order.
pub fn get_matches(env: &Env, tournament_id: u64) -> Vec<TournamentMatch> {
    let mut all: Vec<TournamentMatch> = Vec::new(env);
    for stage in 0..stage_starts(env, tournament_id).len() {
        for m in load_stage(env, tournament_id, stage).iter() {
            all.push_back(m);
        }
    }
    all
}

fn contains(list: &Vec<Address>, who: &Address) -> bool {
    list.iter().any(|a| a == *who)
}

fn seed_of(seeds: &Vec<Address>, who: &Address) -> u32 {
    seeds.first_index_of(who.clone()).unwrap_or(u32::MAX)
}

fn escrow(env: &Env) -> Address {
    env.current_contract_address()
}

/// Caller must be the organizer or hold the RBAC `admin` role.
fn require_admin(env: &Env, caller: &Address, t: &Tournament) -> Result<(), TournamentError> {
    caller.require_auth();
    if *caller == t.organizer
        || access_control::has_role(env, &access_control::admin_role(), caller)
    {
        Ok(())
    } else {
        Err(TournamentError::Unauthorized)
    }
}

// ── Registration & sponsorship ──────────────────────────────────────────────

/// Register for a tournament, paying the entry fee into its prize pool.
/// Returns the registrant's 0-based registration order.
pub fn register_for_tournament(
    env: &Env,
    player: &Address,
    tournament_id: u64,
) -> Result<u32, TournamentError> {
    player.require_auth();
    crate::rate_limiter::check_rate_limit(
        env,
        player,
        crate::rate_limiter::Operation::TournamentRegistration,
    )?;

    let mut tournament = get_tournament(env, tournament_id)?;
    if tournament.status != st_reg() {
        return Err(TournamentError::NotInRegistration);
    }
    if env.ledger().timestamp() > tournament.registration_deadline {
        return Err(TournamentError::RegistrationClosed);
    }

    let mut registrants = get_registrants(env, tournament_id);
    if registrants.len() >= tournament.max_players {
        return Err(TournamentError::TournamentFull);
    }
    if contains(&registrants, player) {
        return Err(TournamentError::AlreadyRegistered);
    }

    if tournament.entry_fee > 0 {
        resource_minter::move_balance(
            env,
            player,
            &escrow(env),
            &tournament.resource_type,
            tournament.entry_fee,
        )
        .map_err(|_| TournamentError::InsufficientFunds)?;
        tournament.prize_pool = tournament
            .prize_pool
            .checked_add(tournament.entry_fee)
            .ok_or(TournamentError::InsufficientFunds)?;
    }

    let position = registrants.len();
    registrants.push_back(player.clone());
    env.storage()
        .persistent()
        .set(&TournamentDataKey::Registrants(tournament_id), &registrants);
    save_tournament(env, &tournament);

    env.events().publish(
        (symbol_short!("tourn"), symbol_short!("register")),
        (tournament_id, player.clone(), position),
    );

    Ok(position)
}

/// Add a sponsor prize pool in any resource type. Paid out with the same
/// `prize_distribution_bps` as the entry-fee pool; any undistributed share
/// goes back to the sponsor. Refunded in full if the tournament is cancelled.
pub fn sponsor_prize(
    env: &Env,
    sponsor: &Address,
    tournament_id: u64,
    resource_type: ResourceType,
    amount: u64,
) -> Result<(), TournamentError> {
    sponsor.require_auth();
    if amount == 0 {
        return Err(TournamentError::InvalidAmount);
    }
    let t = get_tournament(env, tournament_id)?;
    if t.status != st_reg() && t.status != st_active() {
        return Err(TournamentError::TournamentAlreadyDone);
    }
    resource_minter::move_balance(env, sponsor, &escrow(env), &resource_type, amount)
        .map_err(|_| TournamentError::InsufficientFunds)?;

    let mut sponsors = get_sponsors(env, tournament_id);
    sponsors.push_back(SponsorContribution {
        sponsor: sponsor.clone(),
        resource_type: resource_type.clone(),
        amount,
    });
    env.storage()
        .persistent()
        .set(&TournamentDataKey::TournSponsors(tournament_id), &sponsors);

    env.events().publish(
        (symbol_short!("tourn"), symbol_short!("sponsor")),
        (tournament_id, sponsor.clone(), resource_type, amount),
    );
    Ok(())
}

// ── Bracket generation ──────────────────────────────────────────────────────

/// Close registration, seed players by ELO and build the bracket.
///
/// Seeding is by current ELO, descending (stable for ties), so byes go to the
/// strongest players and top seeds meet as late as possible.
pub fn start_tournament(
    env: &Env,
    caller: &Address,
    tournament_id: u64,
) -> Result<(), TournamentError> {
    let mut t = get_tournament(env, tournament_id)?;
    require_admin(env, caller, &t)?;
    if t.status != st_reg() {
        return Err(TournamentError::NotInRegistration);
    }

    let registrants = get_registrants(env, tournament_id);
    if registrants.len() < bg::MIN_PLAYERS {
        return Err(TournamentError::NotEnoughRegistrants);
    }

    // Seed by ELO descending (stable insertion sort; at most 64 players).
    let mut seeded: Vec<Address> = Vec::new(env);
    let mut elos: Vec<u32> = Vec::new(env);
    for p in registrants.iter() {
        let p_elo = pvp_combat::get_elo_rating(env, &p);
        let mut insert_at = seeded.len();
        for j in 0..elos.len() {
            if p_elo > elos.get(j).unwrap_or(0) {
                insert_at = j;
                break;
            }
        }
        seeded.insert(insert_at, p);
        elos.insert(insert_at, p_elo);
    }
    env.storage()
        .persistent()
        .set(&TournamentDataKey::TournSeeds(tournament_id), &seeded);

    let n = seeded.len();
    let now = env.ledger().timestamp();
    t.schedule_base = now.max(t.first_round_at);
    t.status = st_active();
    t.total_rounds = bg::total_rounds(env, t.format, n);

    env.events().publish(
        (symbol_short!("tourn"), symbol_short!("start")),
        (tournament_id, n, t.total_rounds),
    );

    match t.format {
        TournamentFormat::SingleElimination => {
            install_bracket(env, &mut t, &seeded, &bg::single_elimination(env, n))?;
        }
        TournamentFormat::DoubleElimination => {
            install_bracket(env, &mut t, &seeded, &bg::double_elimination(env, n))?;
        }
        TournamentFormat::RoundRobin => {
            install_bracket(env, &mut t, &seeded, &bg::round_robin(env, n))?;
        }
        TournamentFormat::Swiss => {
            // Rounds are paired one at a time by `after_progress`.
        }
    }

    after_progress(env, &mut t)?;
    save_tournament(env, &t);
    Ok(())
}

fn scheduled_at(t: &Tournament, sched: u32) -> u64 {
    t.schedule_base.saturating_add(
        t.round_interval_secs
            .saturating_mul(u64::from(sched.saturating_sub(1))),
    )
}

fn seat(seeded: &Vec<Address>, src: &SlotSource) -> (Option<Address>, bool) {
    match src {
        SlotSource::Bye => (None, true),
        SlotSource::Seed(s) => (seeded.get(*s), true),
        SlotSource::WinnerOf(_) | SlotSource::LoserOf(_) => (None, false),
    }
}

/// Store a pre-generated bracket and settle every match whose players are
/// known up front (seeds and byes).
fn install_bracket(
    env: &Env,
    t: &mut Tournament,
    seeded: &Vec<Address>,
    specs: &Vec<MatchSpec>,
) -> Result<(), TournamentError> {
    let tid = t.tournament_id;
    let links = bg::forward_links(env, specs);
    let base_id = t.match_count;

    let mut stage: Vec<TournamentMatch> = Vec::new(env);
    let mut stage_key: Option<(u32, u32)> = None;
    let mut to_settle: Vec<u32> = Vec::new(env);

    for spec in specs.iter() {
        let key = (spec.side, spec.round);
        if stage_key.is_some() && stage_key != Some(key) {
            append_stage(env, tid, &stage);
            stage = Vec::new(env);
        }
        stage_key = Some(key);

        let (wt, ws, lt, ls) = links
            .get(spec.match_id)
            .unwrap_or((NO_MATCH, 0, NO_MATCH, 0));
        let shift = |id: u32| {
            if id == NO_MATCH {
                NO_MATCH
            } else {
                id + base_id
            }
        };
        let (p1, s1) = seat(seeded, &spec.slot1);
        let (p2, s2) = seat(seeded, &spec.slot2);
        if s1 && s2 {
            to_settle.push_back(spec.match_id + base_id);
        }
        stage.push_back(TournamentMatch {
            match_id: spec.match_id + base_id,
            round: spec.round,
            side: spec.side,
            sched: spec.sched,
            slot1: spec.slot1.clone(),
            slot2: spec.slot2.clone(),
            player1: p1,
            player2: p2,
            settled1: s1,
            settled2: s2,
            winner: None,
            loser: None,
            status: MATCH_PENDING,
            outcome: OUTCOME_NONE,
            combat_id: None,
            scheduled_at: scheduled_at(t, spec.sched),
            ready_at: 0,
            deadline: 0,
            checked_in1: false,
            checked_in2: false,
            resolved_at: 0,
            winner_to: shift(wt),
            winner_slot: ws,
            loser_to: shift(lt),
            loser_slot: ls,
        });
    }
    append_stage(env, tid, &stage);
    t.match_count += specs.len();
    t.pending_matches += specs.len();

    for id in to_settle.iter() {
        let m = get_match(env, tid, id)?;
        settle(env, t, m)?;
    }
    Ok(())
}

/// Both slots of `m` are decided: make it ready, or auto-resolve byes and
/// disqualifications.
fn settle(env: &Env, t: &mut Tournament, mut m: TournamentMatch) -> Result<(), TournamentError> {
    let dq = get_disqualified(env, t.tournament_id);
    match (m.player1.clone(), m.player2.clone()) {
        (Some(a), Some(b)) => {
            let (dq_a, dq_b) = (contains(&dq, &a), contains(&dq, &b));
            if dq_a || dq_b {
                let winner = match (dq_a, dq_b) {
                    (true, true) => None,
                    (true, false) => Some(b),
                    _ => Some(a),
                };
                return finish(env, t, m, winner, OUTCOME_DISQUALIFIED);
            }
            let now = env.ledger().timestamp();
            m.status = MATCH_READY;
            m.ready_at = now;
            m.deadline = now.max(m.scheduled_at).saturating_add(t.match_window_secs);
            m.checked_in1 = false;
            m.checked_in2 = false;
            m.combat_id = None;
            t.current_round = t.current_round.max(m.sched);
            if t.auto_start_combats {
                begin_combat(env, t, &mut m)?;
            }
            save_match(env, t.tournament_id, &m);
            Ok(())
        }
        (Some(a), None) => finish(env, t, m, Some(a), OUTCOME_BYE),
        (None, Some(b)) => finish(env, t, m, Some(b), OUTCOME_BYE),
        (None, None) => finish(env, t, m, None, OUTCOME_BYE),
    }
}

fn begin_combat(env: &Env, t: &Tournament, m: &mut TournamentMatch) -> Result<(), TournamentError> {
    let (Some(a), Some(b)) = (m.player1.clone(), m.player2.clone()) else {
        return Err(TournamentError::MatchNotReady);
    };
    // The tournament id is passed as the challenge id so combats are easy to
    // trace in `pvp_combat` event logs.
    let combat_id = pvp_combat::start_combat(env, &a, &b, t.tournament_id)
        .map_err(|_| TournamentError::MatchNotReady)?;
    m.combat_id = Some(combat_id);
    m.status = MATCH_LIVE;
    Ok(())
}

/// Put `player` into one slot of a downstream match.
fn fill_slot(
    env: &Env,
    t: &mut Tournament,
    match_id: u32,
    slot: u32,
    player: Option<Address>,
) -> Result<(), TournamentError> {
    let mut m = get_match(env, t.tournament_id, match_id)?;
    if slot == 1 {
        m.player1 = player;
        m.settled1 = true;
    } else {
        m.player2 = player;
        m.settled2 = true;
    }
    if m.settled1 && m.settled2 {
        settle(env, t, m)
    } else {
        save_match(env, t.tournament_id, &m);
        Ok(())
    }
}

/// Record a result and move both players on.
fn finish(
    env: &Env,
    t: &mut Tournament,
    mut m: TournamentMatch,
    winner: Option<Address>,
    outcome: u32,
) -> Result<(), TournamentError> {
    let loser = match &winner {
        Some(w) if m.player1.as_ref() == Some(w) => m.player2.clone(),
        Some(_) => m.player1.clone(),
        None => None,
    };
    m.winner = winner.clone();
    m.loser = loser.clone();
    m.status = MATCH_DONE;
    m.outcome = outcome;
    m.resolved_at = env.ledger().timestamp();
    save_match(env, t.tournament_id, &m);
    t.pending_matches = t.pending_matches.saturating_sub(1);

    env.events().publish(
        (symbol_short!("tourn"), symbol_short!("match")),
        (t.tournament_id, m.match_id, winner.clone(), outcome),
    );

    if m.side == SIDE_GRAND_FINAL {
        return route_grand_final(env, t, &m);
    }
    if m.winner_to != NO_MATCH {
        fill_slot(env, t, m.winner_to, m.winner_slot, winner)?;
    }
    if m.loser_to != NO_MATCH {
        fill_slot(env, t, m.loser_to, m.loser_slot, loser)?;
    }
    Ok(())
}

/// Double elimination: the reset match is only played if the losers-bracket
/// champion (slot 2) wins the first grand final.
fn route_grand_final(
    env: &Env,
    t: &mut Tournament,
    gf: &TournamentMatch,
) -> Result<(), TournamentError> {
    let reset_id = gf.winner_to;
    if reset_id == NO_MATCH {
        return Ok(());
    }
    let mut reset = get_match(env, t.tournament_id, reset_id)?;
    let wb_champion_won = gf.player2.is_none() || gf.winner == gf.player1;
    if wb_champion_won {
        if reset.status != MATCH_SKIPPED {
            reset.status = MATCH_SKIPPED;
            reset.settled1 = true;
            reset.settled2 = true;
            reset.player1 = None;
            reset.player2 = None;
            save_match(env, t.tournament_id, &reset);
            t.pending_matches = t.pending_matches.saturating_sub(1);
        }
        return Ok(());
    }
    if reset.status == MATCH_SKIPPED {
        // A dispute flipped the grand final: the reset is back on.
        reset.status = MATCH_PENDING;
        t.pending_matches += 1;
    }
    reset.player1 = gf.winner.clone();
    reset.player2 = gf.loser.clone();
    reset.settled1 = true;
    reset.settled2 = true;
    settle(env, t, reset)
}

/// True if a match feeding `m` has an open dispute.
fn inputs_disputed(env: &Env, tid: u64, m: &TournamentMatch) -> bool {
    [&m.slot1, &m.slot2].iter().any(|src| match src {
        SlotSource::WinnerOf(x) | SlotSource::LoserOf(x) => {
            get_dispute(env, tid, *x).is_some_and(|d| d.open)
        }
        SlotSource::Bye | SlotSource::Seed(_) => false,
    })
}

// ── Progress, Swiss rounds and completion ───────────────────────────────────

/// After any result: pair the next Swiss round or complete the tournament
/// once nothing is pending and no dispute is open.
fn after_progress(env: &Env, t: &mut Tournament) -> Result<(), TournamentError> {
    loop {
        if t.status != st_active() || t.pending_matches > 0 || t.open_disputes > 0 {
            return Ok(());
        }
        if t.format == TournamentFormat::Swiss && t.current_round < t.total_rounds {
            if pair_swiss_round(env, t)? {
                continue;
            }
        }
        return complete(env, t);
    }
}

/// Pair and store the next Swiss round. Returns false if fewer than two
/// active players remain.
fn pair_swiss_round(env: &Env, t: &mut Tournament) -> Result<bool, TournamentError> {
    let tid = t.tournament_id;
    let seeds = get_seeds(env, tid);
    let standings = compute_standings(env, t);
    let mut order: Vec<u32> = Vec::new(env);
    for s in standings.iter() {
        if !s.disqualified {
            order.push_back(s.seed);
        }
    }
    if order.len() < 2 {
        return Ok(false);
    }

    let mut played: Vec<u64> = Vec::new(env);
    let mut had_bye: Vec<u32> = Vec::new(env);
    for m in get_matches(env, tid).iter() {
        match (&m.player1, &m.player2) {
            (Some(a), Some(b)) => {
                played.push_back(bg::pair_key(seed_of(&seeds, a), seed_of(&seeds, b)));
            }
            (Some(a), None) if m.outcome == OUTCOME_BYE => had_bye.push_back(seed_of(&seeds, a)),
            _ => {}
        }
    }
    let (pairs, bye) = bg::swiss_pairings(env, &order, &played, &had_bye);

    let round = t.current_round + 1;
    let kick_off = scheduled_at(t, round);
    let mut stage: Vec<TournamentMatch> = Vec::new(env);
    let mut next_id = t.match_count;
    let mut make = |s1: SlotSource, s2: SlotSource, id: u32| -> TournamentMatch {
        let (p1, _) = seat(&seeds, &s1);
        let (p2, _) = seat(&seeds, &s2);
        TournamentMatch {
            match_id: id,
            round,
            side: SIDE_POOL,
            sched: round,
            slot1: s1,
            slot2: s2,
            player1: p1,
            player2: p2,
            settled1: true,
            settled2: true,
            winner: None,
            loser: None,
            status: MATCH_PENDING,
            outcome: OUTCOME_NONE,
            combat_id: None,
            scheduled_at: kick_off,
            ready_at: 0,
            deadline: 0,
            checked_in1: false,
            checked_in2: false,
            resolved_at: 0,
            winner_to: NO_MATCH,
            winner_slot: 0,
            loser_to: NO_MATCH,
            loser_slot: 0,
        }
    };
    for (a, b) in pairs.iter() {
        stage.push_back(make(SlotSource::Seed(a), SlotSource::Seed(b), next_id));
        next_id += 1;
    }
    if let Some(b) = bye {
        stage.push_back(make(SlotSource::Seed(b), SlotSource::Bye, next_id));
        next_id += 1;
    }

    let first_new = t.match_count;
    let count = stage.len();
    append_stage(env, tid, &stage);
    t.match_count += count;
    t.pending_matches += count;
    t.current_round = round;

    env.events().publish(
        (symbol_short!("tourn"), symbol_short!("round")),
        (tid, round, count),
    );

    for id in first_new..first_new + count {
        let m = get_match(env, tid, id)?;
        settle(env, t, m)?;
    }
    Ok(true)
}

fn champion_of(env: &Env, t: &Tournament, placements: &Vec<Address>) -> Option<Address> {
    match t.format {
        TournamentFormat::RoundRobin | TournamentFormat::Swiss => placements.get(0),
        TournamentFormat::SingleElimination | TournamentFormat::DoubleElimination => {
            let all = get_matches(env, t.tournament_id);
            let mut champ: Option<Address> = None;
            for m in all.iter() {
                if m.status == MATCH_DONE
                    && (m.winner_to == NO_MATCH
                        || m.side == SIDE_GRAND_FINAL
                        || m.side == SIDE_GRAND_FINAL_RESET)
                {
                    champ = m.winner.clone();
                }
            }
            champ
        }
    }
}

/// Placements, best first.
///
/// * Elimination: champion, then each match's loser in reverse match order
///   (later rounds rank higher; ties keep bracket order).
/// * Round robin / Swiss: wins, then tiebreak, then seed.
///
/// Anyone not placed by those rules (e.g. two disqualified players who never
/// lost a match) is appended in seed order.
fn compute_placements(env: &Env, t: &Tournament) -> Vec<Address> {
    let mut placements: Vec<Address> = Vec::new(env);
    match t.format {
        TournamentFormat::RoundRobin | TournamentFormat::Swiss => {
            for s in compute_standings(env, t).iter() {
                placements.push_back(s.player);
            }
        }
        TournamentFormat::SingleElimination | TournamentFormat::DoubleElimination => {
            if let Some(c) = champion_of(env, t, &placements) {
                placements.push_back(c);
            }
            let all = get_matches(env, t.tournament_id);
            let mut i = all.len();
            while i > 0 {
                i -= 1;
                let m = all.get(i).unwrap();
                if m.status != MATCH_DONE {
                    continue;
                }
                if let Some(l) = m.loser {
                    if !contains(&placements, &l) {
                        placements.push_back(l);
                    }
                }
            }
        }
    }
    for p in get_seeds(env, t.tournament_id).iter() {
        if !contains(&placements, &p) {
            placements.push_back(p);
        }
    }
    placements
}

/// Live win/loss table, sorted best first.
pub fn get_standings(env: &Env, tournament_id: u64) -> Result<Vec<Standing>, TournamentError> {
    let t = get_tournament(env, tournament_id)?;
    Ok(compute_standings(env, &t))
}

fn compute_standings(env: &Env, t: &Tournament) -> Vec<Standing> {
    let seeds = get_seeds(env, t.tournament_id);
    let dq = get_disqualified(env, t.tournament_id);
    let n = seeds.len();
    let mut wins: Vec<u32> = Vec::new(env);
    let mut losses: Vec<u32> = Vec::new(env);
    for _ in 0..n {
        wins.push_back(0);
        losses.push_back(0);
    }
    let matches = get_matches(env, t.tournament_id);
    for m in matches.iter() {
        if m.status != MATCH_DONE {
            continue;
        }
        if let Some(w) = &m.winner {
            let i = seed_of(&seeds, w);
            if i < n {
                wins.set(i, wins.get(i).unwrap_or(0) + 1);
            }
        }
        if let Some(l) = &m.loser {
            let i = seed_of(&seeds, l);
            if i < n {
                losses.set(i, losses.get(i).unwrap_or(0) + 1);
            }
        }
    }

    let mut table: Vec<Standing> = Vec::new(env);
    for (i, player) in seeds.iter().enumerate() {
        let i = i as u32;
        let mut tiebreak = 0u32;
        for m in matches.iter() {
            if m.status != MATCH_DONE {
                continue;
            }
            let (Some(a), Some(b)) = (&m.player1, &m.player2) else {
                continue;
            };
            let opp = if *a == player {
                b
            } else if *b == player {
                a
            } else {
                continue;
            };
            let opp_wins = wins.get(seed_of(&seeds, opp)).unwrap_or(0);
            match t.format {
                TournamentFormat::Swiss => tiebreak += opp_wins,
                TournamentFormat::RoundRobin => {
                    if m.winner.as_ref() == Some(&player) {
                        tiebreak += opp_wins;
                    }
                }
                _ => {}
            }
        }
        let row = Standing {
            disqualified: contains(&dq, &player),
            player,
            seed: i,
            wins: wins.get(i).unwrap_or(0),
            losses: losses.get(i).unwrap_or(0),
            tiebreak,
        };
        // Insertion sort: wins desc, tiebreak desc, seed asc.
        let mut at = table.len();
        for j in 0..table.len() {
            let other = table.get(j).unwrap();
            let better = (row.wins, row.tiebreak) > (other.wins, other.tiebreak);
            if better {
                at = j;
                break;
            }
        }
        table.insert(at, row);
    }
    table
}

fn complete(env: &Env, t: &mut Tournament) -> Result<(), TournamentError> {
    let placements = compute_placements(env, t);
    env.storage().persistent().set(
        &TournamentDataKey::TournStandings(t.tournament_id),
        &placements,
    );
    t.champion = placements.get(0);
    t.status = st_final();
    t.completed_at = env.ledger().timestamp();

    env.events().publish(
        (symbol_short!("tourn"), symbol_short!("final")),
        (t.tournament_id, t.champion.clone()),
    );

    if t.dispute_window_secs == 0 {
        finalize_inner(env, t)?;
    }
    Ok(())
}

/// Pay out one pool by `bps`, returning undistributed funds to `refund_to`.
fn pay_pool(
    env: &Env,
    t: &Tournament,
    placements: &Vec<Address>,
    resource_type: &ResourceType,
    amount: u64,
    refund_to: &Address,
) -> Result<(), TournamentError> {
    if amount == 0 {
        return Ok(());
    }
    let mut paid = 0u64;
    for (i, bps) in t.prize_distribution_bps.iter().enumerate() {
        let Some(who) = placements.get(i as u32) else {
            break;
        };
        let payout = (u128::from(amount) * u128::from(bps) / 10_000u128) as u64;
        if payout == 0 {
            continue;
        }
        resource_minter::move_balance(env, &escrow(env), &who, resource_type, payout)
            .map_err(|_| TournamentError::PrizeTransferFailed)?;
        paid += payout;
        env.events().publish(
            (symbol_short!("tourn"), symbol_short!("prize")),
            (t.tournament_id, who, resource_type.clone(), payout),
        );
    }
    let rest = amount - paid;
    if rest > 0 {
        resource_minter::move_balance(env, &escrow(env), refund_to, resource_type, rest)
            .map_err(|_| TournamentError::PrizeTransferFailed)?;
    }
    Ok(())
}

fn finalize_inner(env: &Env, t: &mut Tournament) -> Result<(), TournamentError> {
    // Recompute: a dispute may have changed results after completion.
    let placements = compute_placements(env, t);
    env.storage().persistent().set(
        &TournamentDataKey::TournStandings(t.tournament_id),
        &placements,
    );
    t.champion = placements.get(0);

    pay_pool(
        env,
        t,
        &placements,
        &t.resource_type,
        t.prize_pool,
        &t.organizer,
    )?;
    for s in get_sponsors(env, t.tournament_id).iter() {
        pay_pool(env, t, &placements, &s.resource_type, s.amount, &s.sponsor)?;
    }

    if let Some(c) = &t.champion {
        let titles = get_player_titles(env, c) + 1;
        env.storage()
            .persistent()
            .set(&TournamentDataKey::TournTitles(c.clone()), &titles);
    }
    t.status = st_done();

    env.events().publish(
        (symbol_short!("tourn"), symbol_short!("done")),
        (t.tournament_id, t.champion.clone()),
    );
    Ok(())
}

/// Pay out a tournament whose dispute window has elapsed. Callable by anyone.
pub fn finalize_tournament(env: &Env, tournament_id: u64) -> Result<(), TournamentError> {
    let mut t = get_tournament(env, tournament_id)?;
    if t.status != st_final() {
        return Err(TournamentError::NotFinalizable);
    }
    if t.open_disputes > 0 {
        return Err(TournamentError::DisputesOpen);
    }
    if env.ledger().timestamp() < t.completed_at.saturating_add(t.dispute_window_secs) {
        return Err(TournamentError::DeadlineNotReached);
    }
    finalize_inner(env, &mut t)?;
    save_tournament(env, &t);
    Ok(())
}

// ── Playing matches ─────────────────────────────────────────────────────────

fn active_match(
    env: &Env,
    tournament_id: u64,
    match_id: u32,
) -> Result<(Tournament, TournamentMatch), TournamentError> {
    let t = get_tournament(env, tournament_id)?;
    if t.status != st_active() {
        return Err(TournamentError::TournamentNotActive);
    }
    let m = get_match(env, tournament_id, match_id)?;
    if m.status == MATCH_DONE || m.status == MATCH_SKIPPED {
        return Err(TournamentError::MatchAlreadyResolved);
    }
    Ok((t, m))
}

/// Check in for a match: open from [`CHECK_IN_LEAD_SECS`] before kick-off
/// until the deadline. A player who checks in when the opponent doesn't can
/// claim a forfeit after the deadline.
pub fn check_in(
    env: &Env,
    player: &Address,
    tournament_id: u64,
    match_id: u32,
) -> Result<(), TournamentError> {
    player.require_auth();
    let (_, mut m) = active_match(env, tournament_id, match_id)?;
    if m.status != MATCH_READY {
        return Err(TournamentError::MatchNotReady);
    }
    let now = env.ledger().timestamp();
    if now < m.scheduled_at.saturating_sub(CHECK_IN_LEAD_SECS) || now > m.deadline {
        return Err(TournamentError::CheckInClosed);
    }
    if m.player1.as_ref() == Some(player) {
        m.checked_in1 = true;
    } else if m.player2.as_ref() == Some(player) {
        m.checked_in2 = true;
    } else {
        return Err(TournamentError::PlayerNotInMatch);
    }
    save_match(env, tournament_id, &m);
    env.events().publish(
        (symbol_short!("tourn"), symbol_short!("checkin")),
        (tournament_id, match_id, player.clone()),
    );
    Ok(())
}

/// Start the PvP combat for a ready match (for tournaments without
/// `auto_start_combats`). Either player or an admin may call it from
/// kick-off onwards.
pub fn start_match(
    env: &Env,
    caller: &Address,
    tournament_id: u64,
    match_id: u32,
) -> Result<u64, TournamentError> {
    let (t, mut m) = active_match(env, tournament_id, match_id)?;
    if m.player1.as_ref() == Some(caller) || m.player2.as_ref() == Some(caller) {
        caller.require_auth();
    } else {
        require_admin(env, caller, &t)?;
    }
    if m.status != MATCH_READY {
        return Err(TournamentError::MatchNotReady);
    }
    if env.ledger().timestamp() < m.scheduled_at {
        return Err(TournamentError::TooEarly);
    }
    if inputs_disputed(env, tournament_id, &m) {
        return Err(TournamentError::MatchDisputed);
    }
    begin_combat(env, &t, &mut m)?;
    save_match(env, tournament_id, &m);
    Ok(m.combat_id.unwrap_or(0))
}

/// Pull the result of a finished PvP combat into its match. Permissionless:
/// the combat itself is the source of truth.
pub fn report_result(env: &Env, tournament_id: u64, match_id: u32) -> Result<(), TournamentError> {
    let (mut t, m) = active_match(env, tournament_id, match_id)?;
    let combat_id = m.combat_id.ok_or(TournamentError::MatchNotReady)?;
    let combat =
        pvp_combat::get_combat(env, combat_id).map_err(|_| TournamentError::InvalidMatch)?;
    let winner = combat.winner.ok_or(TournamentError::MatchNotReady)?;
    if m.player1.as_ref() != Some(&winner) && m.player2.as_ref() != Some(&winner) {
        return Err(TournamentError::InvalidWinner);
    }
    if inputs_disputed(env, tournament_id, &m) {
        return Err(TournamentError::MatchDisputed);
    }
    finish(env, &mut t, m, Some(winner), OUTCOME_COMBAT)?;
    after_progress(env, &mut t)?;
    save_tournament(env, &t);
    Ok(())
}

/// Legacy addressing: `match_index`-th winners-bracket (or pool) match of
/// `round`. Fails for byes, which have no combat.
pub fn report_match_result(
    env: &Env,
    tournament_id: u64,
    round: u32,
    match_index: u32,
) -> Result<(), TournamentError> {
    let t = get_tournament(env, tournament_id)?;
    if t.status != st_active() {
        return Err(TournamentError::TournamentNotActive);
    }
    let mut seen = 0u32;
    for m in get_matches(env, tournament_id).iter() {
        if m.round != round || (m.side != SIDE_WINNERS && m.side != SIDE_POOL) {
            continue;
        }
        if seen == match_index {
            if m.status == MATCH_DONE {
                return Err(TournamentError::MatchAlreadyResolved);
            }
            if m.combat_id.is_none() {
                return Err(TournamentError::InvalidMatch);
            }
            return report_result(env, tournament_id, m.match_id);
        }
        seen += 1;
    }
    Err(TournamentError::InvalidMatch)
}

/// Legacy view: the winners-bracket (or pool) matches of one round.
pub fn get_bracket_round(env: &Env, tournament_id: u64, round: u32) -> Vec<BracketMatch> {
    let mut out: Vec<BracketMatch> = Vec::new(env);
    for m in get_matches(env, tournament_id).iter() {
        if m.round == round && (m.side == SIDE_WINNERS || m.side == SIDE_POOL) {
            out.push_back(BracketMatch {
                player1: m.player1,
                player2: m.player2,
                winner: m.winner,
                combat_id: m.combat_id,
            });
        }
    }
    out
}

/// After a ready match's deadline, award it to the player who checked in.
///
/// * one player checked in → they win;
/// * nobody checked in → the higher seed advances (double no-show);
/// * both checked in, or a combat is running → [`TournamentError::ForfeitNotAllowed`]
///   (play it out, or ask an admin).
///
/// Callable by anyone, so keepers can keep brackets moving.
pub fn claim_forfeit(
    env: &Env,
    caller: &Address,
    tournament_id: u64,
    match_id: u32,
) -> Result<Address, TournamentError> {
    caller.require_auth();
    let (mut t, m) = active_match(env, tournament_id, match_id)?;
    if m.status == MATCH_LIVE {
        return Err(TournamentError::ForfeitNotAllowed);
    }
    if m.status != MATCH_READY {
        return Err(TournamentError::MatchNotReady);
    }
    if env.ledger().timestamp() <= m.deadline {
        return Err(TournamentError::DeadlineNotReached);
    }
    if inputs_disputed(env, tournament_id, &m) {
        return Err(TournamentError::MatchDisputed);
    }
    let (Some(a), Some(b)) = (m.player1.clone(), m.player2.clone()) else {
        return Err(TournamentError::MatchNotReady);
    };
    let winner = match (m.checked_in1, m.checked_in2) {
        (true, false) => a,
        (false, true) => b,
        (false, false) => {
            let seeds = get_seeds(env, tournament_id);
            if seed_of(&seeds, &a) <= seed_of(&seeds, &b) {
                a
            } else {
                b
            }
        }
        (true, true) => return Err(TournamentError::ForfeitNotAllowed),
    };
    env.events().publish(
        (symbol_short!("tourn"), symbol_short!("forfeit")),
        (tournament_id, match_id, winner.clone()),
    );
    finish(env, &mut t, m, Some(winner.clone()), OUTCOME_FORFEIT)?;
    after_progress(env, &mut t)?;
    save_tournament(env, &t);
    Ok(winner)
}

// ── Admin panel ─────────────────────────────────────────────────────────────

/// Referee override: decide a ready or live match.
pub fn admin_set_result(
    env: &Env,
    caller: &Address,
    tournament_id: u64,
    match_id: u32,
    winner: &Address,
) -> Result<(), TournamentError> {
    let (mut t, m) = active_match(env, tournament_id, match_id)?;
    require_admin(env, caller, &t)?;
    if m.status != MATCH_READY && m.status != MATCH_LIVE {
        return Err(TournamentError::MatchNotReady);
    }
    if m.player1.as_ref() != Some(winner) && m.player2.as_ref() != Some(winner) {
        return Err(TournamentError::InvalidWinner);
    }
    if inputs_disputed(env, tournament_id, &m) {
        return Err(TournamentError::MatchDisputed);
    }
    finish(env, &mut t, m, Some(winner.clone()), OUTCOME_ADMIN)?;
    after_progress(env, &mut t)?;
    save_tournament(env, &t);
    Ok(())
}

/// Give a ready or live match more time before forfeits can be claimed.
pub fn extend_match_deadline(
    env: &Env,
    caller: &Address,
    tournament_id: u64,
    match_id: u32,
    extra_secs: u64,
) -> Result<u64, TournamentError> {
    let (t, mut m) = active_match(env, tournament_id, match_id)?;
    require_admin(env, caller, &t)?;
    if m.status != MATCH_READY && m.status != MATCH_LIVE {
        return Err(TournamentError::MatchNotReady);
    }
    if extra_secs == 0 || extra_secs > MAX_MATCH_WINDOW_SECS {
        return Err(TournamentError::InvalidConfig);
    }
    m.deadline = m.deadline.saturating_add(extra_secs);
    save_match(env, tournament_id, &m);
    Ok(m.deadline)
}

/// Remove a player.
///
/// * During registration: they are dropped and refunded.
/// * While active: every ready/live match they are in is awarded to the
///   opponent, and any later match they would reach is forfeited the same
///   way. Swiss stops pairing them.
pub fn disqualify_player(
    env: &Env,
    caller: &Address,
    tournament_id: u64,
    player: &Address,
) -> Result<(), TournamentError> {
    let mut t = get_tournament(env, tournament_id)?;
    require_admin(env, caller, &t)?;

    if t.status == st_reg() {
        let mut registrants = get_registrants(env, tournament_id);
        let idx = registrants
            .first_index_of(player.clone())
            .ok_or(TournamentError::NotRegistered)?;
        registrants.remove(idx);
        env.storage()
            .persistent()
            .set(&TournamentDataKey::Registrants(tournament_id), &registrants);
        if t.entry_fee > 0 {
            resource_minter::move_balance(env, &escrow(env), player, &t.resource_type, t.entry_fee)
                .map_err(|_| TournamentError::PrizeTransferFailed)?;
            t.prize_pool = t.prize_pool.saturating_sub(t.entry_fee);
        }
        save_tournament(env, &t);
        env.events().publish(
            (symbol_short!("tourn"), symbol_short!("dq")),
            (tournament_id, player.clone()),
        );
        return Ok(());
    }
    if t.status != st_active() {
        return Err(TournamentError::TournamentNotActive);
    }
    if !contains(&get_seeds(env, tournament_id), player) {
        return Err(TournamentError::NotRegistered);
    }
    let mut dq = get_disqualified(env, tournament_id);
    if contains(&dq, player) {
        return Ok(());
    }
    dq.push_back(player.clone());
    env.storage()
        .persistent()
        .set(&TournamentDataKey::TournDisqualified(tournament_id), &dq);
    env.events().publish(
        (symbol_short!("tourn"), symbol_short!("dq")),
        (tournament_id, player.clone()),
    );

    for m in get_matches(env, tournament_id).iter() {
        // Re-read: an earlier forfeit in this loop may have changed it.
        let m = get_match(env, tournament_id, m.match_id)?;
        if m.status != MATCH_READY && m.status != MATCH_LIVE {
            continue;
        }
        let opponent = if m.player1.as_ref() == Some(player) {
            m.player2.clone()
        } else if m.player2.as_ref() == Some(player) {
            m.player1.clone()
        } else {
            continue;
        };
        let winner = opponent.filter(|o| !contains(&dq, o));
        finish(env, &mut t, m, winner, OUTCOME_DISQUALIFIED)?;
    }
    after_progress(env, &mut t)?;
    save_tournament(env, &t);
    Ok(())
}

/// Cancel a tournament that hasn't paid out: refunds every entry fee and
/// every sponsor pool.
pub fn cancel_tournament(
    env: &Env,
    caller: &Address,
    tournament_id: u64,
) -> Result<(), TournamentError> {
    let mut t = get_tournament(env, tournament_id)?;
    require_admin(env, caller, &t)?;
    if t.status == st_done() || t.status == st_cancelled() {
        return Err(TournamentError::TournamentAlreadyDone);
    }
    if t.entry_fee > 0 {
        for p in get_registrants(env, tournament_id).iter() {
            resource_minter::move_balance(env, &escrow(env), &p, &t.resource_type, t.entry_fee)
                .map_err(|_| TournamentError::PrizeTransferFailed)?;
        }
    }
    for s in get_sponsors(env, tournament_id).iter() {
        resource_minter::move_balance(env, &escrow(env), &s.sponsor, &s.resource_type, s.amount)
            .map_err(|_| TournamentError::PrizeTransferFailed)?;
    }
    env.storage().persistent().set(
        &TournamentDataKey::TournSponsors(tournament_id),
        &Vec::<SponsorContribution>::new(env),
    );
    t.prize_pool = 0;
    t.status = st_cancelled();
    save_tournament(env, &t);
    env.events().publish(
        (symbol_short!("tourn"), symbol_short!("cancel")),
        (tournament_id, caller.clone()),
    );
    Ok(())
}

/// Dashboard summary for organizers.
pub fn get_admin_overview(
    env: &Env,
    tournament_id: u64,
) -> Result<TournamentOverview, TournamentError> {
    let t = get_tournament(env, tournament_id)?;
    let now = env.ledger().timestamp();
    let (mut decided, mut live, mut overdue) = (0u32, 0u32, 0u32);
    for m in get_matches(env, tournament_id).iter() {
        match m.status {
            MATCH_DONE | MATCH_SKIPPED => decided += 1,
            MATCH_LIVE => live += 1,
            _ => {}
        }
        if (m.status == MATCH_READY || m.status == MATCH_LIVE) && now > m.deadline {
            overdue += 1;
        }
    }
    Ok(TournamentOverview {
        tournament_id,
        status: t.status.clone(),
        format: t.format,
        registrants: get_registrants(env, tournament_id).len(),
        disqualified: get_disqualified(env, tournament_id).len(),
        matches_total: t.match_count,
        matches_decided: decided,
        matches_live: live,
        matches_overdue: overdue,
        open_disputes: t.open_disputes,
        current_round: t.current_round,
        total_rounds: t.total_rounds,
        prize_pool: t.prize_pool,
        sponsor_pools: get_sponsors(env, tournament_id).len(),
    })
}

/// A match's kick-off in the organizer's display time zone (seconds since
/// the epoch, shifted by `timezone_offset_minutes`).
pub fn local_kickoff(env: &Env, tournament_id: u64, match_id: u32) -> Result<u64, TournamentError> {
    let t = get_tournament(env, tournament_id)?;
    let m = get_match(env, tournament_id, match_id)?;
    bg::to_local_timestamp(m.scheduled_at, t.timezone_offset_minutes)
        .ok_or(TournamentError::InvalidConfig)
}

// ── Disputes ────────────────────────────────────────────────────────────────

/// Dispute a decided match. Only its players may do so, within
/// `dispute_window_secs` of the result, and only while every match it fed
/// is still undecided (results that already moved the bracket on are locked).
/// While open, matches fed by it can't be decided and the tournament can't
/// be finalized.
pub fn raise_dispute(
    env: &Env,
    player: &Address,
    tournament_id: u64,
    match_id: u32,
    reason: Symbol,
) -> Result<(), TournamentError> {
    player.require_auth();
    let mut t = get_tournament(env, tournament_id)?;
    if t.status != st_active() && t.status != st_final() {
        return Err(TournamentError::TournamentNotActive);
    }
    if t.dispute_window_secs == 0 {
        return Err(TournamentError::DisputeWindowClosed);
    }
    let m = get_match(env, tournament_id, match_id)?;
    if m.status != MATCH_DONE || m.outcome == OUTCOME_BYE {
        return Err(TournamentError::MatchNotReady);
    }
    if m.player1.as_ref() != Some(player) && m.player2.as_ref() != Some(player) {
        return Err(TournamentError::PlayerNotInMatch);
    }
    let now = env.ledger().timestamp();
    if now > m.resolved_at.saturating_add(t.dispute_window_secs) {
        return Err(TournamentError::DisputeWindowClosed);
    }
    if get_dispute(env, tournament_id, match_id).is_some_and(|d| d.open) {
        return Err(TournamentError::DisputeAlreadyOpen);
    }
    for dest in [m.winner_to, m.loser_to] {
        if dest != NO_MATCH {
            let d = get_match(env, tournament_id, dest)?;
            if d.status == MATCH_DONE {
                return Err(TournamentError::ResultLocked);
            }
        }
    }

    env.storage().persistent().set(
        &TournamentDataKey::TournDispute(tournament_id, match_id),
        &MatchDispute {
            match_id,
            raised_by: player.clone(),
            reason: reason.clone(),
            raised_at: now,
            open: true,
            overturned: false,
        },
    );
    t.open_disputes += 1;
    save_tournament(env, &t);
    env.events().publish(
        (symbol_short!("tourn"), symbol_short!("dispute")),
        (tournament_id, match_id, player.clone(), reason),
    );
    Ok(())
}

/// Swap the occupant of one slot of a downstream, still-undecided match.
fn replace_slot(
    env: &Env,
    t: &mut Tournament,
    match_id: u32,
    slot: u32,
    player: Option<Address>,
) -> Result<(), TournamentError> {
    let mut m = get_match(env, t.tournament_id, match_id)?;
    if m.status == MATCH_DONE {
        return Err(TournamentError::ResultLocked);
    }
    if m.status == MATCH_SKIPPED {
        return Ok(());
    }
    if slot == 1 {
        m.player1 = player;
    } else {
        m.player2 = player;
    }
    if m.settled1 && m.settled2 {
        // Was ready (or live with the wrong players): re-evaluate from scratch.
        m.status = MATCH_PENDING;
        settle(env, t, m)
    } else {
        save_match(env, t.tournament_id, &m);
        Ok(())
    }
}

/// Rule on a dispute (organizer or RBAC admin). Overturning swaps winner and
/// loser and re-routes both into the matches they feed.
pub fn resolve_dispute(
    env: &Env,
    caller: &Address,
    tournament_id: u64,
    match_id: u32,
    overturn: bool,
) -> Result<(), TournamentError> {
    let mut t = get_tournament(env, tournament_id)?;
    require_admin(env, caller, &t)?;
    let mut dispute =
        get_dispute(env, tournament_id, match_id).ok_or(TournamentError::DisputeNotFound)?;
    if !dispute.open {
        return Err(TournamentError::DisputeNotFound);
    }

    if overturn {
        let mut m = get_match(env, tournament_id, match_id)?;
        let new_winner = m.loser.clone().ok_or(TournamentError::InvalidWinner)?;
        let new_loser = m.winner.clone();
        m.winner = Some(new_winner.clone());
        m.loser = new_loser.clone();
        m.outcome = OUTCOME_ADMIN;
        save_match(env, tournament_id, &m);

        if m.side == SIDE_GRAND_FINAL {
            route_grand_final(env, &mut t, &m)?;
        } else {
            if m.winner_to != NO_MATCH {
                replace_slot(env, &mut t, m.winner_to, m.winner_slot, Some(new_winner))?;
            }
            if m.loser_to != NO_MATCH {
                replace_slot(env, &mut t, m.loser_to, m.loser_slot, new_loser)?;
            }
        }
    }

    dispute.open = false;
    dispute.overturned = overturn;
    env.storage().persistent().set(
        &TournamentDataKey::TournDispute(tournament_id, match_id),
        &dispute,
    );
    t.open_disputes = t.open_disputes.saturating_sub(1);

    // An overturned grand final can bring a reset match back into play.
    if t.status == st_final() && t.pending_matches > 0 {
        t.status = st_active();
        t.completed_at = 0;
    }

    env.events().publish(
        (symbol_short!("tourn"), symbol_short!("disp_res")),
        (tournament_id, match_id, overturn),
    );
    after_progress(env, &mut t)?;
    save_tournament(env, &t);
    Ok(())
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::test_helpers::{contract_env, in_contract};
    use soroban_sdk::testutils::{Address as _, Ledger as _};
    use soroban_sdk::vec;
    extern crate std;

    const DUST: ResourceType = ResourceType::StellarDust;

    fn env() -> Env {
        let env = contract_env();
        env.mock_all_auths();
        env.cost_estimate().budget().reset_unlimited();
        env
    }

    fn advance(env: &Env, secs: u64) {
        let now = env.ledger().timestamp();
        env.ledger().set_timestamp(now + secs);
    }

    fn players(env: &Env, n: u32) -> std::vec::Vec<Address> {
        (0..n).map(|_| Address::generate(env)).collect()
    }

    fn bal(env: &Env, who: &Address, rt: &ResourceType) -> u64 {
        in_contract(env, || resource_minter::balance_of(env, who, rt))
    }

    fn fund(env: &Env, who: &Address, rt: &ResourceType, amount: u64) {
        in_contract(env, || {
            resource_minter::credit_balance(env, who, rt, amount)
        })
        .unwrap();
    }

    fn cfg(env: &Env, format: TournamentFormat, max: u32, fee: u64) -> TournamentConfig {
        TournamentConfig {
            format,
            resource_type: DUST,
            entry_fee: fee,
            max_players: max,
            registration_window_secs: 3_600,
            prize_distribution_bps: vec![env, 5_000u32, 3_000, 2_000],
            first_round_at: 0,
            round_interval_secs: 0,
            match_window_secs: 3_600,
            dispute_window_secs: 0,
            timezone_offset_minutes: 0,
            auto_start_combats: false,
        }
    }

    /// Create, register `ps` (funding fees) and start.
    fn setup(env: &Env, org: &Address, c: TournamentConfig, ps: &[Address]) -> u64 {
        let fee = c.entry_fee;
        let t = in_contract(env, || create_tournament_with_config(env, org, c)).unwrap();
        for p in ps {
            if fee > 0 {
                fund(env, p, &DUST, fee);
            }
            in_contract(env, || register_for_tournament(env, p, t)).unwrap();
        }
        in_contract(env, || start_tournament(env, org, t)).unwrap();
        t
    }

    fn open_matches(env: &Env, t: u64) -> std::vec::Vec<TournamentMatch> {
        in_contract(env, || get_matches(env, t))
            .iter()
            .filter(|m| m.status == MATCH_READY || m.status == MATCH_LIVE)
            .collect()
    }

    /// Referee every match until the tournament leaves "active". Returns the
    /// number of real (two-player) matches decided.
    fn play_out(
        env: &Env,
        org: &Address,
        t: u64,
        mut pick: impl FnMut(&TournamentMatch) -> Address,
    ) -> u32 {
        let mut played = 0;
        loop {
            let open = open_matches(env, t);
            if open.is_empty() {
                return played;
            }
            for m in open {
                let w = pick(&m);
                in_contract(env, || admin_set_result(env, org, t, m.match_id, &w)).unwrap();
                played += 1;
            }
        }
    }

    fn lower_seed(env: &Env, t: u64) -> impl Fn(&TournamentMatch) -> Address {
        let seeds = in_contract(env, || get_seeds(env, t));
        move |m: &TournamentMatch| {
            let (a, b) = (m.player1.clone().unwrap(), m.player2.clone().unwrap());
            if seed_of(&seeds, &a) < seed_of(&seeds, &b) {
                a
            } else {
                b
            }
        }
    }

    // ── Legacy API (Issue #284) ──

    #[test]
    fn legacy_create_validates_inputs() {
        let env = env();
        let org = Address::generate(&env);
        let mk = |max: u32, bps: Vec<u32>| {
            in_contract(&env, || {
                create_tournament(&env, &org, DUST, 0, max, 3_600, bps)
            })
        };
        assert_eq!(
            mk(5, Vec::new(&env)),
            Err(TournamentError::InvalidPlayerCount)
        );
        assert_eq!(
            mk(4, vec![&env, 6_000u32, 5_000]),
            Err(TournamentError::InvalidPrizeDistribution)
        );
        assert!(mk(4, vec![&env, 10_000u32]).is_ok());
    }

    #[test]
    fn legacy_bracket_runs_on_real_combats_and_pays_prizes() {
        use crate::pvp_combat::{execute_move, get_combat};
        let env = env();
        let org = Address::generate(&env);
        let ps = players(&env, 4);
        let t = in_contract(&env, || {
            create_tournament(&env, &org, DUST, 100, 4, 3_600, vec![&env, 7_000u32, 3_000])
        })
        .unwrap();
        for p in &ps {
            fund(&env, p, &DUST, 100);
            in_contract(&env, || register_for_tournament(&env, p, t)).unwrap();
        }
        in_contract(&env, || start_tournament(&env, &org, t)).unwrap();

        for round in 1..=2u32 {
            let matches = in_contract(&env, || get_bracket_round(&env, t, round));
            for i in 0..matches.len() {
                let cid = matches.get(i).unwrap().combat_id.unwrap();
                for _ in 0..8 {
                    let c = in_contract(&env, || get_combat(&env, cid)).unwrap();
                    if c.status == symbol_short!("finished") {
                        break;
                    }
                    in_contract(&env, || {
                        execute_move(&env, &c.turn, cid, symbol_short!("attack"), 100)
                    })
                    .ok();
                }
                in_contract(&env, || report_match_result(&env, t, round, i)).unwrap();
            }
        }
        let tour = in_contract(&env, || get_tournament(&env, t)).unwrap();
        assert_eq!(tour.status, symbol_short!("done"));
        let standings = in_contract(&env, || get_final_standings(&env, t));
        assert_eq!(bal(&env, &standings.get(0).unwrap(), &DUST), 280);
        assert_eq!(bal(&env, &standings.get(1).unwrap(), &DUST), 120);
        assert_eq!(
            bal(&env, &env.current_contract_address_for_test(), &DUST),
            0
        );
    }

    trait EscrowAddr {
        fn current_contract_address_for_test(&self) -> Address;
    }
    impl EscrowAddr for Env {
        fn current_contract_address_for_test(&self) -> Address {
            crate::test_helpers::unit_contract_id(self)
        }
    }

    #[test]
    fn byes_go_to_top_seeds() {
        let env = env();
        let org = Address::generate(&env);
        let ps = players(&env, 5);
        let t = setup(
            &env,
            &org,
            cfg(&env, TournamentFormat::SingleElimination, 8, 0),
            &ps,
        );
        let round1 = in_contract(&env, || get_bracket_round(&env, t, 1));
        let byes: std::vec::Vec<_> = round1
            .iter()
            .filter(|m| m.player1.is_none() || m.player2.is_none())
            .collect();
        assert_eq!(byes.len(), 3);
        for b in byes {
            let w = b.winner.unwrap();
            assert!(ps[..3].contains(&w), "bye went to a low seed");
        }
    }

    // ── Formats ──

    #[test]
    fn double_elimination_plays_reset_when_losers_champion_wins() {
        let env = env();
        let org = Address::generate(&env);
        let ps = players(&env, 4);
        let t = setup(
            &env,
            &org,
            cfg(&env, TournamentFormat::DoubleElimination, 4, 0),
            &ps,
        );
        let fav = lower_seed(&env, t);
        let played = play_out(&env, &org, t, |m| {
            if m.side == SIDE_GRAND_FINAL {
                m.player2.clone().unwrap() // losers-bracket champion wins GF 1
            } else {
                fav(m)
            }
        });
        // 6 bracket matches + reset.
        assert_eq!(played, 7);
        let tour = in_contract(&env, || get_tournament(&env, t)).unwrap();
        assert_eq!(tour.status, symbol_short!("done"));
        assert_eq!(tour.champion, Some(ps[0].clone()));
    }

    #[test]
    fn round_robin_ranks_by_wins() {
        let env = env();
        let org = Address::generate(&env);
        let ps = players(&env, 5);
        let t = setup(
            &env,
            &org,
            cfg(&env, TournamentFormat::RoundRobin, 8, 0),
            &ps,
        );
        let fav = lower_seed(&env, t);
        assert_eq!(play_out(&env, &org, t, fav), 10);
        let table = in_contract(&env, || get_standings(&env, t)).unwrap();
        for (i, row) in table.iter().enumerate() {
            assert_eq!(row.player, ps[i]);
            assert_eq!(row.wins, 4 - i as u32);
        }
    }

    #[test]
    fn swiss_avoids_rematches_and_rotates_byes() {
        let env = env();
        let org = Address::generate(&env);
        let ps = players(&env, 7);
        let t = setup(&env, &org, cfg(&env, TournamentFormat::Swiss, 8, 0), &ps);
        let fav = lower_seed(&env, t);
        assert_eq!(play_out(&env, &org, t, fav), 9); // 3 rounds * 3 pairs
        let mut pairs = std::collections::BTreeSet::new();
        let mut byes = std::collections::BTreeSet::new();
        for m in in_contract(&env, || get_matches(&env, t)).iter() {
            match (m.player1, m.player2) {
                (Some(a), Some(b)) => {
                    let k = if a < b { (a, b) } else { (b, a) };
                    assert!(pairs.insert(k), "rematch");
                }
                (Some(a), None) => assert!(byes.insert(a), "second bye"),
                _ => {}
            }
        }
        assert_eq!(byes.len(), 3);
    }

    // ── Scheduling, check-in and forfeits ──

    #[test]
    fn schedule_check_in_and_forfeits() {
        let env = env();
        let org = Address::generate(&env);
        let ps = players(&env, 8);
        let mut c = cfg(&env, TournamentFormat::SingleElimination, 8, 0);
        c.first_round_at = env.ledger().timestamp() + 7_200;
        c.round_interval_secs = 86_400;
        c.timezone_offset_minutes = 60;
        let t = setup(&env, &org, c.clone(), &ps);
        let m0 = in_contract(&env, || get_match(&env, t, 0)).unwrap();
        assert_eq!(m0.scheduled_at, c.first_round_at);
        assert_eq!(m0.deadline, c.first_round_at + 3_600);
        assert_eq!(
            in_contract(&env, || local_kickoff(&env, t, 0)),
            Ok(c.first_round_at + 3_600)
        );
        let final_match = in_contract(&env, || get_match(&env, t, 6)).unwrap();
        assert_eq!(final_match.scheduled_at, c.first_round_at + 2 * 86_400);

        let p = |m: &TournamentMatch| (m.player1.clone().unwrap(), m.player2.clone().unwrap());
        let (a0, _) = p(&m0);
        assert_eq!(
            in_contract(&env, || check_in(&env, &a0, t, 0)),
            Err(TournamentError::CheckInClosed)
        );
        assert_eq!(
            in_contract(&env, || start_match(&env, &a0, t, 0)),
            Err(TournamentError::TooEarly)
        );

        advance(&env, 7_200);
        in_contract(&env, || check_in(&env, &a0, t, 0)).unwrap();
        let m2 = in_contract(&env, || get_match(&env, t, 2)).unwrap();
        let (a2, b2) = p(&m2);
        in_contract(&env, || check_in(&env, &a2, t, 2)).unwrap();
        in_contract(&env, || check_in(&env, &b2, t, 2)).unwrap();
        assert_eq!(
            in_contract(&env, || claim_forfeit(&env, &org, t, 0)),
            Err(TournamentError::DeadlineNotReached)
        );

        advance(&env, 3_601);
        // One check-in: that player wins.
        assert_eq!(
            in_contract(&env, || claim_forfeit(&env, &org, t, 0)),
            Ok(a0)
        );
        // No check-ins: higher seed advances.
        let m1 = in_contract(&env, || get_match(&env, t, 1)).unwrap();
        let w1 = in_contract(&env, || claim_forfeit(&env, &org, t, 1)).unwrap();
        assert_eq!(w1, lower_seed(&env, t)(&m1));
        // Both checked in: must be played.
        assert_eq!(
            in_contract(&env, || claim_forfeit(&env, &org, t, 2)),
            Err(TournamentError::ForfeitNotAllowed)
        );
    }

    // ── Admin panel, prizes ──

    #[test]
    fn admin_controls_and_multi_pool_prizes() {
        let env = env();
        let org = Address::generate(&env);
        let sponsor = Address::generate(&env);
        let mallory = Address::generate(&env);
        let ps = players(&env, 5);
        let dm = ResourceType::DarkMatter;
        let mut c = cfg(&env, TournamentFormat::SingleElimination, 8, 100);
        c.prize_distribution_bps = vec![&env, 6_000u32, 3_000];
        let t = in_contract(&env, || create_tournament_with_config(&env, &org, c)).unwrap();
        for p in &ps {
            fund(&env, p, &DUST, 100);
            in_contract(&env, || register_for_tournament(&env, p, t)).unwrap();
        }
        fund(&env, &sponsor, &dm, 1_000);
        in_contract(&env, || sponsor_prize(&env, &sponsor, t, dm.clone(), 1_000)).unwrap();

        // DQ during registration refunds.
        assert_eq!(
            in_contract(&env, || disqualify_player(&env, &mallory, t, &ps[4])),
            Err(TournamentError::Unauthorized)
        );
        in_contract(&env, || disqualify_player(&env, &org, t, &ps[4])).unwrap();
        assert_eq!(bal(&env, &ps[4], &DUST), 100);

        in_contract(&env, || start_tournament(&env, &org, t)).unwrap();
        let m = open_matches(&env, t)[0].clone();
        assert_eq!(
            in_contract(&env, || admin_set_result(
                &env,
                &mallory,
                t,
                m.match_id,
                &m.player1.clone().unwrap()
            )),
            Err(TournamentError::Unauthorized)
        );
        assert_eq!(
            in_contract(&env, || admin_set_result(
                &env, &org, t, m.match_id, &mallory
            )),
            Err(TournamentError::InvalidWinner)
        );
        let d = in_contract(&env, || {
            extend_match_deadline(&env, &org, t, m.match_id, 600)
        })
        .unwrap();
        assert_eq!(d, m.deadline + 600);

        let fav = lower_seed(&env, t);
        play_out(&env, &org, t, fav);
        let overview = in_contract(&env, || get_admin_overview(&env, t)).unwrap();
        assert_eq!(overview.status, symbol_short!("done"));
        assert_eq!(overview.matches_decided, overview.matches_total);

        // Pool 400 DUST + 1000 DM split 60/30; remainders to their funders.
        assert_eq!(bal(&env, &ps[0], &DUST), 240);
        assert_eq!(bal(&env, &ps[0], &dm), 600);
        assert_eq!(bal(&env, &ps[1], &DUST), 120);
        assert_eq!(bal(&env, &ps[1], &dm), 300);
        assert_eq!(bal(&env, &org, &DUST), 40);
        assert_eq!(bal(&env, &sponsor, &dm), 100);
        assert_eq!(in_contract(&env, || get_player_titles(&env, &ps[0])), 1);
    }

    #[test]
    fn cancel_refunds_everyone() {
        let env = env();
        let org = Address::generate(&env);
        let ps = players(&env, 4);
        let t = setup(&env, &org, cfg(&env, TournamentFormat::Swiss, 4, 50), &ps);
        fund(&env, &org, &DUST, 70);
        in_contract(&env, || sponsor_prize(&env, &org, t, DUST, 70)).unwrap();
        in_contract(&env, || cancel_tournament(&env, &org, t)).unwrap();
        for p in &ps {
            assert_eq!(bal(&env, p, &DUST), 50);
        }
        assert_eq!(bal(&env, &org, &DUST), 70);
        assert_eq!(
            in_contract(&env, || cancel_tournament(&env, &org, t)),
            Err(TournamentError::TournamentAlreadyDone)
        );
    }

    #[test]
    fn disqualification_mid_event_forfeits_matches() {
        let env = env();
        let org = Address::generate(&env);
        let ps = players(&env, 4);
        let t = setup(
            &env,
            &org,
            cfg(&env, TournamentFormat::SingleElimination, 4, 0),
            &ps,
        );
        in_contract(&env, || disqualify_player(&env, &org, t, &ps[0])).unwrap();
        let m = in_contract(&env, || get_match(&env, t, 0)).unwrap();
        assert_eq!(m.outcome, OUTCOME_DISQUALIFIED);
        assert_eq!(m.winner, Some(ps[3].clone()));
    }

    // ── Disputes ──

    #[test]
    fn dispute_blocks_progress_and_overturn_reroutes() {
        let env = env();
        let org = Address::generate(&env);
        let outsider = Address::generate(&env);
        let ps = players(&env, 4);
        let mut c = cfg(&env, TournamentFormat::SingleElimination, 4, 0);
        c.dispute_window_secs = 600;
        let t = setup(&env, &org, c, &ps);

        // Match 0: seed 0 vs seed 3. Referee gives it to seed 0.
        in_contract(&env, || admin_set_result(&env, &org, t, 0, &ps[0])).unwrap();
        assert_eq!(
            in_contract(&env, || raise_dispute(
                &env,
                &outsider,
                t,
                0,
                symbol_short!("cheat")
            )),
            Err(TournamentError::PlayerNotInMatch)
        );
        in_contract(&env, || {
            raise_dispute(&env, &ps[3], t, 0, symbol_short!("cheat"))
        })
        .unwrap();
        assert_eq!(
            in_contract(&env, || raise_dispute(
                &env,
                &ps[3],
                t,
                0,
                symbol_short!("cheat")
            )),
            Err(TournamentError::DisputeAlreadyOpen)
        );
        in_contract(&env, || admin_set_result(&env, &org, t, 1, &ps[1])).unwrap();
        // Final is fed by the disputed match.
        assert_eq!(
            in_contract(&env, || admin_set_result(&env, &org, t, 2, &ps[1])),
            Err(TournamentError::MatchDisputed)
        );
        in_contract(&env, || resolve_dispute(&env, &org, t, 0, true)).unwrap();
        let fin = in_contract(&env, || get_match(&env, t, 2)).unwrap();
        assert!(fin.player1 == Some(ps[3].clone()) || fin.player2 == Some(ps[3].clone()));

        in_contract(&env, || admin_set_result(&env, &org, t, 2, &ps[3])).unwrap();
        let tour = in_contract(&env, || get_tournament(&env, t)).unwrap();
        assert_eq!(tour.status, symbol_short!("final"));
        assert_eq!(
            in_contract(&env, || finalize_tournament(&env, t)),
            Err(TournamentError::DeadlineNotReached)
        );
        advance(&env, 601);
        assert_eq!(
            in_contract(&env, || raise_dispute(
                &env,
                &ps[1],
                t,
                2,
                symbol_short!("late")
            )),
            Err(TournamentError::DisputeWindowClosed)
        );
        in_contract(&env, || finalize_tournament(&env, t)).unwrap();
        assert_eq!(
            in_contract(&env, || get_tournament(&env, t))
                .unwrap()
                .champion,
            Some(ps[3].clone())
        );
    }

    #[test]
    fn overturned_grand_final_brings_back_the_reset() {
        let env = env();
        let org = Address::generate(&env);
        let ps = players(&env, 2);
        let mut c = cfg(&env, TournamentFormat::DoubleElimination, 4, 0);
        c.dispute_window_secs = 600;
        let t = setup(&env, &org, c, &ps);
        in_contract(&env, || admin_set_result(&env, &org, t, 0, &ps[0])).unwrap(); // WB final
        in_contract(&env, || admin_set_result(&env, &org, t, 1, &ps[0])).unwrap(); // GF
        assert_eq!(
            in_contract(&env, || get_tournament(&env, t))
                .unwrap()
                .status,
            symbol_short!("final")
        );
        in_contract(&env, || {
            raise_dispute(&env, &ps[1], t, 1, symbol_short!("lag"))
        })
        .unwrap();
        in_contract(&env, || resolve_dispute(&env, &org, t, 1, true)).unwrap();
        assert_eq!(
            in_contract(&env, || get_tournament(&env, t))
                .unwrap()
                .status,
            symbol_short!("active")
        );
        let reset = in_contract(&env, || get_match(&env, t, 2)).unwrap();
        assert_eq!(reset.status, MATCH_READY);
        in_contract(&env, || admin_set_result(&env, &org, t, 2, &ps[1])).unwrap();
        advance(&env, 601);
        in_contract(&env, || finalize_tournament(&env, t)).unwrap();
        assert_eq!(
            in_contract(&env, || get_tournament(&env, t))
                .unwrap()
                .champion,
            Some(ps[1].clone())
        );
    }

    // ── 10,000+ simulated matches ──

    /// Fast CI version of the simulation (~300 matches; keeps the coverage
    /// job well inside its 120 s per-test timeout).
    #[test]
    fn simulate_quick() {
        assert!(simulate(300) >= 300);
    }

    /// Full acceptance run for Issue #587. Too slow for a debug/coverage CI
    /// run, so it is opt-in:
    /// `cargo test --release --lib simulate_ten_thousand -- --ignored`
    #[test]
    #[ignore = "slow: run with --release -- --ignored"]
    fn simulate_ten_thousand_matches() {
        assert!(simulate(10_000) >= 10_000);
    }

    /// Plays tournaments of every format and size with pseudo-random results
    /// and checks, for each: it finishes, every player is placed exactly once,
    /// the real match count matches the format, and the prize pool is paid
    /// out exactly (escrow ends empty). Returns the matches played.
    fn simulate(target: u32) -> u32 {
        let mut total = 0u32;
        let mut rng = 0x2545_f491_u64;
        let formats = [
            (TournamentFormat::SingleElimination, 64u32),
            (TournamentFormat::DoubleElimination, 64),
            (TournamentFormat::RoundRobin, 16),
            (TournamentFormat::Swiss, 64),
        ];
        let mut i = 0u32;
        while total < target {
            let env = env();
            let org = Address::generate(&env);
            let (format, cap) = formats[(i % 4) as usize];
            let n = 2 + (i * 7 + 5) % (cap - 1); // 2..=cap, varied
            let ps = players(&env, n);
            let t = setup(&env, &org, cfg(&env, format, cap, 10), &ps);
            let played = play_out(&env, &org, t, |m| {
                rng ^= rng << 13;
                rng ^= rng >> 7;
                rng ^= rng << 17;
                if rng % 2 == 0 {
                    m.player1.clone().unwrap()
                } else {
                    m.player2.clone().unwrap()
                }
            });
            let tour = in_contract(&env, || get_tournament(&env, t)).unwrap();
            assert_eq!(tour.status, symbol_short!("done"), "{format:?} n={n}");
            let placed = in_contract(&env, || get_final_standings(&env, t));
            assert_eq!(placed.len(), n);
            let uniq: std::collections::BTreeSet<_> = placed.iter().collect();
            assert_eq!(uniq.len() as u32, n);
            let expected_ok = match format {
                TournamentFormat::SingleElimination => played == n - 1,
                TournamentFormat::DoubleElimination => played == 2 * n - 2 || played == 2 * n - 1,
                TournamentFormat::RoundRobin => played == n * (n - 1) / 2,
                TournamentFormat::Swiss => played == bg::swiss_round_count(n) * (n / 2),
            };
            assert!(expected_ok, "{format:?} n={n} played={played}");
            let escrow = crate::test_helpers::unit_contract_id(&env);
            assert_eq!(bal(&env, &escrow, &DUST), 0);
            let paid: u64 =
                ps.iter().map(|p| bal(&env, p, &DUST)).sum::<u64>() + bal(&env, &org, &DUST);
            assert_eq!(paid, u64::from(n) * 10);
            total += played;
            i += 1;
        }
        total
    }
}
