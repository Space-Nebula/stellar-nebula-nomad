//! # Anti-Cheat Detection for PvP Combat (Issue #593)
//!
//! Server-side validation and abuse detection for PvP combat. The contract is
//! the only authority for combat state: clients submit a move type and power,
//! and the contract derives the resulting HP/energy changes itself. This module
//! sits beside that engine and adds a defence-in-depth layer:
//!
//! - **Server-side validation**: every mutating PvP entry point checks that the
//!   caller is not banned, and move input/intent is validated before it is
//!   applied. Impossible deltas are neutralised rather than trusted.
//! - **Impossible state change detection**: the post-move combat snapshot is
//!   compared against the snapshot the engine was expected to produce. Any
//!   divergence is logged and overwritten with the expected values.
//! - **Timing / pattern monitoring**: a bounded per-player timestamp window is
//!   analysed for bursts and mechanically regular intervals.
//! - **Suspicion scoring**: deterministic, capped score with time decay and
//!   per-rule increments. Conservative defaults avoid punishing legit play.
//! - **Automatic penalties**: warnings, temporary bans and permanent bans are
//!   derived from score, severity and repeat-warning count.
//! - **Manual review + appeals**: high-signal detections enter an admin review
//!   queue; penalised players can appeal within a fixed window.
//! - **Detection logging**: every detection emits an `("ac", "detect")` event
//!   *and* a persistent [`DetectionRecord`] retrievable by id or by player.
//!
//! Timestamps are ledger seconds (same unit as `pvp_combat::COMBAT_TIMEOUT`).
//! All thresholds live in [`AntiCheatConfig`] and can only be changed by the
//! anti-cheat admin via `set_config`.

use soroban_sdk::{contracterror, contracttype, symbol_short, Address, Env, Symbol, Vec};

use crate::error_standard::{ErrorDescriptor, ErrorKind, StandardContractError};
use crate::pvp_combat::{CombatState, MAX_HP, MAX_MOVE_POWER};

// ── Errors ────────────────────────────────────────────────────────────────────

#[contracterror]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum AntiCheatError {
    /// Caller is not the anti-cheat admin.
    Unauthorized = 1,
    /// Admin-supplied configuration failed validation.
    InvalidConfig = 2,
    /// Detection record does not exist.
    DetectionNotFound = 3,
    /// Detection is not (or no longer) in the manual review queue.
    ReviewNotFound = 4,
    /// Appeal record does not exist.
    AppealNotFound = 5,
    /// Appeal was submitted after the appeal window closed.
    AppealWindowClosed = 6,
    /// An appeal already exists for this detection.
    AlreadyAppealed = 7,
    /// Player is currently banned from PvP actions.
    PlayerBanned = 8,
    /// Review or appeal has already been resolved.
    AlreadyResolved = 9,
    /// Invalid review decision for this operation.
    InvalidDecision = 10,
}

impl StandardContractError for AntiCheatError {
    fn descriptor(self) -> ErrorDescriptor {
        let (kind, retryable) = match self {
            Self::Unauthorized => (ErrorKind::Authorization, false),
            Self::InvalidConfig | Self::InvalidDecision => (ErrorKind::Validation, false),
            Self::DetectionNotFound | Self::ReviewNotFound | Self::AppealNotFound => {
                (ErrorKind::NotFound, false)
            }
            Self::AppealWindowClosed | Self::AlreadyAppealed | Self::AlreadyResolved => {
                (ErrorKind::Conflict, false)
            }
            Self::PlayerBanned => (ErrorKind::Authorization, false),
        };
        ErrorDescriptor {
            module: "anti_cheat",
            code: self as u32,
            kind,
            retryable,
        }
    }
}

// ── Storage Keys ──────────────────────────────────────────────────────────────

#[contracttype]
#[derive(Clone)]
pub enum AntiCheatKey {
    /// Anti-cheat administrator.
    Admin,
    /// Tunable thresholds ([`AntiCheatConfig`]).
    Config,
    /// Monotonic detection id counter.
    DetectionCounter,
    /// Persistent detection record by id.
    Detection(u64),
    /// Detection ids raised for a player (bounded).
    PlayerDetections(Address),
    /// Ids awaiting manual admin review (bounded).
    ReviewQueue,
    /// Per-player suspicion profile.
    Profile(Address),
    /// Currently active penalty for a player.
    Penalty(Address),
    /// Bounded per-player action timestamp window (ledger seconds).
    ActionWindow(Address),
    /// Monotonic appeal id counter.
    AppealCounter,
    /// Appeal record by id.
    Appeal(u64),
    /// Maps a detection id to its appeal id.
    AppealByDetection(u64),
    /// Appeal ids filed by a player (bounded).
    PlayerAppeals(Address),
}

// ── Constants ─────────────────────────────────────────────────────────────────

/// Upper bound of every suspicion score.
pub const MAX_SCORE: u32 = 100;
/// Conservative defaults. Admin can override with `set_config`.
pub const DEFAULT_REVIEW_THRESHOLD: u32 = 50;
pub const DEFAULT_WARNING_THRESHOLD: u32 = 40;
pub const DEFAULT_TEMP_BAN_THRESHOLD: u32 = 65;
pub const DEFAULT_PERMANENT_BAN_THRESHOLD: u32 = 90;
/// Two actions in the same second can be legitimate (batches, multi-combat),
/// so a single fast action is only a low-signal observation.
pub const MIN_ACTION_INTERVAL_SECS: u64 = 2;
pub const ACTION_WINDOW_SECS: u64 = 60;
pub const MAX_ACTIONS_PER_WINDOW: u32 = 8;
pub const MAX_ACTION_SAMPLES: u32 = 16;
pub const DETECTION_COOLDOWN_SECS: u64 = 30;
pub const TEMP_BAN_DURATION_SECS: u64 = 86_400;
pub const APPEAL_WINDOW_SECS: u64 = 604_800;
pub const MAX_DETECTIONS_PER_PLAYER: u32 = 50;
pub const MAX_REVIEW_QUEUE: u32 = 200;
pub const MAX_APPEALS_PER_PLAYER: u32 = 20;
/// Time for an untouched score to halve. Slow enough that a clean streak does
/// not erase a serious history overnight.
pub const SCORE_HALF_LIFE_SECS: u64 = 604_800;
pub const REGULAR_TOLERANCE_SECS: u64 = 1;
pub const REGULAR_MIN_SAMPLES: usize = 5;
pub const REGULAR_RATIO_PCT: u32 = 80;
pub const WARNINGS_BEFORE_TEMP_BAN: u32 = 3;

// ── Data Types ────────────────────────────────────────────────────────────────

#[contracttype]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DetectionType {
    /// Input that cannot be produced by a legitimate client.
    InvalidAction,
    /// Post-action combat state diverged from the server-derived expectation.
    ImpossibleState,
    /// Action timestamps imply superhuman reaction or clock manipulation.
    TimingAnomaly,
    /// Action timestamps are too regular to be human.
    PatternAnomaly,
    /// Too many actions inside a short window (scripted burst).
    BurstAnomaly,
}

#[contracttype]
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub enum Severity {
    Low,
    Medium,
    High,
    Critical,
}

#[contracttype]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PenaltyKind {
    None,
    Warning,
    TemporaryBan,
    PermanentBan,
}

#[contracttype]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReviewDecision {
    /// No admin decision yet (used on stored detection records).
    Pending,
    /// Detection was a false positive; score is restored and penalty lifted.
    Cleared,
    /// Detection stands; penalty and score remain.
    Upheld,
    /// Detection is fully reversed (score reset, warnings cleared).
    Reversed,
}

#[contracttype]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AppealStatus {
    Pending,
    Approved,
    Rejected,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AntiCheatConfig {
    pub review_threshold: u32,
    pub warning_threshold: u32,
    pub temp_ban_threshold: u32,
    pub permanent_ban_threshold: u32,
    pub min_action_interval_secs: u64,
    pub action_window_secs: u64,
    pub max_actions_per_window: u32,
    pub detection_cooldown_secs: u64,
    pub temp_ban_duration_secs: u64,
    pub appeal_window_secs: u64,
    pub max_detections_per_player: u32,
    pub max_review_queue: u32,
}

impl Default for AntiCheatConfig {
    fn default() -> Self {
        Self {
            review_threshold: DEFAULT_REVIEW_THRESHOLD,
            warning_threshold: DEFAULT_WARNING_THRESHOLD,
            temp_ban_threshold: DEFAULT_TEMP_BAN_THRESHOLD,
            permanent_ban_threshold: DEFAULT_PERMANENT_BAN_THRESHOLD,
            min_action_interval_secs: MIN_ACTION_INTERVAL_SECS,
            action_window_secs: ACTION_WINDOW_SECS,
            max_actions_per_window: MAX_ACTIONS_PER_WINDOW,
            detection_cooldown_secs: DETECTION_COOLDOWN_SECS,
            temp_ban_duration_secs: TEMP_BAN_DURATION_SECS,
            appeal_window_secs: APPEAL_WINDOW_SECS,
            max_detections_per_player: MAX_DETECTIONS_PER_PLAYER,
            max_review_queue: MAX_REVIEW_QUEUE,
        }
    }
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlayerRiskProfile {
    pub score: u32,
    pub detection_count: u32,
    pub warning_count: u32,
    pub last_detection_at: u64,
    pub penalty: PenaltyKind,
    pub banned_until: u64,
}

impl Default for PlayerRiskProfile {
    fn default() -> Self {
        Self {
            score: 0,
            detection_count: 0,
            warning_count: 0,
            last_detection_at: 0,
            penalty: PenaltyKind::None,
            banned_until: 0,
        }
    }
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PenaltyRecord {
    pub kind: PenaltyKind,
    pub issued_at: u64,
    /// `0` unless `kind == TemporaryBan`.
    pub expires_at: u64,
    pub detection_id: u64,
    pub active: bool,
}

impl Default for PenaltyRecord {
    fn default() -> Self {
        Self {
            kind: PenaltyKind::None,
            issued_at: 0,
            expires_at: 0,
            detection_id: 0,
            active: false,
        }
    }
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DetectionRecord {
    pub detection_id: u64,
    pub player: Address,
    pub detection_type: DetectionType,
    pub severity: Severity,
    pub score_delta: u32,
    pub score_after: u32,
    pub combat_id: Option<u64>,
    /// Action that triggered the detection (short symbol).
    pub action: Symbol,
    /// Stable evidence code, e.g. `power`, `state_mismatch`, `burst`.
    pub detail: Symbol,
    pub timestamp: u64,
    pub auto_penalty: PenaltyKind,
    pub reviewed: bool,
    /// `Pending` until an admin resolves the detection.
    pub review_decision: ReviewDecision,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AppealRecord {
    pub appeal_id: u64,
    pub detection_id: u64,
    pub player: Address,
    pub reason: Symbol,
    pub created_at: u64,
    pub deadline: u64,
    pub status: AppealStatus,
    pub reviewer: Option<Address>,
    pub resolved_at: u64,
}

/// Plain, env-free combat snapshot used for deterministic state comparison.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CombatSnapshot {
    pub player1_hp: u32,
    pub player2_hp: u32,
    pub player1_energy: u32,
    pub player2_energy: u32,
}

/// Server-derived expectation for one move. `expected` is the only state the
/// engine is allowed to produce for the supplied (validated) input.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MoveValidation {
    pub is_player1: bool,
    pub energy_cost: u32,
    pub damage: u32,
    pub heal: u32,
    pub expected: CombatSnapshot,
}

// ── Pure Logic (no Soroban env; unit-tested) ──────────────────────────────────

/// Clamp a score into `0..=MAX_SCORE` without wrapping.
pub fn clamp_score(score: u32) -> u32 {
    core::cmp::min(score, MAX_SCORE)
}

/// Add evidence to a score with saturating arithmetic.
pub fn add_score(score: u32, delta: u32) -> u32 {
    clamp_score(score.saturating_add(delta))
}

/// Halve the score for each complete half-life, capped at 16 periods to keep
/// execution deterministic even for very old timestamps.
pub fn decay_score(score: u32, elapsed_secs: u64, half_life_secs: u64) -> u32 {
    if score == 0 || half_life_secs == 0 || elapsed_secs < half_life_secs {
        return score;
    }
    let periods = core::cmp::min(elapsed_secs / half_life_secs, 16);
    let mut current = score;
    for _ in 0..periods {
        current = current.saturating_sub((current.saturating_add(1)) / 2);
        if current == 0 {
            break;
        }
    }
    current
}

/// Deterministic per-rule base score.
pub fn base_score(kind: DetectionType) -> u32 {
    match kind {
        DetectionType::InvalidAction => 10,
        DetectionType::TimingAnomaly => 10,
        DetectionType::PatternAnomaly => 12,
        DetectionType::BurstAnomaly => 12,
        DetectionType::ImpossibleState => 30,
    }
}

/// Severity multiplier. Critical evidence (impossible state) reaches the cap
/// in a single occurrence, which is intentional: it cannot happen legitimately.
pub fn score_for_detection(kind: DetectionType, severity: Severity) -> u32 {
    let multiplier = match severity {
        Severity::Low => 1,
        Severity::Medium => 2,
        Severity::High => 3,
        Severity::Critical => 4,
    };
    clamp_score(base_score(kind).saturating_mul(multiplier))
}

/// Score-derived penalty. Repeat warnings escalate to a temporary ban so that
/// a player cannot farm low-severity warnings indefinitely.
pub fn penalty_for_profile(
    score: u32,
    severity: Severity,
    warning_count: u32,
    config: &AntiCheatConfig,
) -> PenaltyKind {
    if config.permanent_ban_threshold <= MAX_SCORE && score >= config.permanent_ban_threshold {
        return PenaltyKind::PermanentBan;
    }
    if warning_count >= WARNINGS_BEFORE_TEMP_BAN
        || score >= config.temp_ban_threshold
        || severity == Severity::Critical
    {
        return PenaltyKind::TemporaryBan;
    }
    if score >= config.warning_threshold {
        return PenaltyKind::Warning;
    }
    PenaltyKind::None
}

/// `true` when a proposed attack/special deals more than the engine permits.
pub fn is_impossible_damage(
    power: u32,
    damage: u32,
    is_special: bool,
    max_power: u32,
) -> bool {
    let allowed = if is_special {
        max_power.saturating_mul(2)
    } else {
        max_power
    };
    damage > allowed || damage > power.saturating_mul(if is_special { 2 } else { 1 })
}

/// `true` when a heal exceeds the power-derived cap or the health bar.
pub fn is_impossible_heal(power: u32, heal: u32, max_hp: u32) -> bool {
    heal > power / 2 || heal > max_hp
}

/// `true` when the energy charge does not match `power / 2`.
pub fn is_impossible_energy(power: u32, energy_cost: u32, max_power: u32) -> bool {
    power > max_power || energy_cost != power / 2
}

/// `true` when a timestamp moves backwards (ledger clock manipulation).
pub fn is_timing_violation(now: u64, last: u64, min_interval_secs: u64) -> bool {
    now < last || (last > 0 && now.saturating_sub(last) < min_interval_secs)
}

/// Detect scripted bursts: more than `max_events` timestamps (plus the current
/// action) inside `window_secs`. Pure slice form for tests; the on-chain path
/// copies its bounded window into a fixed-size buffer before calling this.
pub fn is_burst(timestamps: &[u64], now: u64, window_secs: u64, max_events: u32) -> bool {
    let cutoff = now.saturating_sub(window_secs);
    let mut count = 1u32; // include the action being observed
    for &t in timestamps {
        if t >= cutoff && t <= now {
            count = count.saturating_add(1);
        }
    }
    count > max_events
}

/// Detect mechanically regular intervals: at least `min_samples` comparisons
/// and `ratio_pct` percent of them within `tolerance` of the previous gap.
pub fn is_regular_intervals(
    timestamps: &[u64],
    tolerance: u64,
    min_samples: usize,
    ratio_pct: u32,
) -> bool {
    if timestamps.len() < min_samples || min_samples < 3 {
        return false;
    }
    let mut compared = 0u32;
    let mut regular = 0u32;
    for i in 2..timestamps.len() {
        let prev_gap = timestamps[i - 1].saturating_sub(timestamps[i - 2]);
        let gap = timestamps[i].saturating_sub(timestamps[i - 1]);
        compared = compared.saturating_add(1);
        if prev_gap.abs_diff(gap) <= tolerance {
            regular = regular.saturating_add(1);
        }
    }
    compared > 0 && regular.saturating_mul(100) / compared >= ratio_pct
}

/// `true` when a score may produce another detection record (cooldown elapsed).
pub fn cooldown_elapsed(now: u64, last_detection_at: u64, cooldown_secs: u64) -> bool {
    last_detection_at == 0 || now.saturating_sub(last_detection_at) >= cooldown_secs
}

/// Compare two combat snapshots field by field.
pub fn snapshots_equal(a: &CombatSnapshot, b: &CombatSnapshot) -> bool {
    a.player1_hp == b.player1_hp
        && a.player2_hp == b.player2_hp
        && a.player1_energy == b.player1_energy
        && a.player2_energy == b.player2_energy
}

/// Admin configuration is only accepted when thresholds are strictly ordered
/// and all windows/durations are non-zero and bounded.
pub fn is_valid_config(config: &AntiCheatConfig) -> bool {
    config.warning_threshold > 0
        && config.warning_threshold < config.temp_ban_threshold
        && config.temp_ban_threshold < config.permanent_ban_threshold
        && config.permanent_ban_threshold <= MAX_SCORE
        && config.review_threshold > 0
        && config.review_threshold <= MAX_SCORE
        && config.min_action_interval_secs > 0
        && config.action_window_secs >= config.min_action_interval_secs
        && config.max_actions_per_window > 0
        && config.detection_cooldown_secs > 0
        && config.temp_ban_duration_secs > 0
        && config.appeal_window_secs > 0
        && config.max_detections_per_player > 0
        && config.max_review_queue > 0
}

// ── Admin / Config ────────────────────────────────────────────────────────────

/// Bootstrap or rotate the anti-cheat admin. The first caller becomes admin;
/// later callers must already be admin.
pub fn set_admin(env: &Env, caller: &Address, admin: &Address) -> Result<(), AntiCheatError> {
    caller.require_auth();
    if let Some(current) = get_admin(env) {
        if current != *caller {
            return Err(AntiCheatError::Unauthorized);
        }
    }
    env.storage().persistent().set(&AntiCheatKey::Admin, admin);
    env.events().publish(
        (symbol_short!("ac"), symbol_short!("admin")),
        (caller.clone(), admin.clone()),
    );
    Ok(())
}

pub fn get_admin(env: &Env) -> Option<Address> {
    env.storage().persistent().get(&AntiCheatKey::Admin)
}

fn require_admin(env: &Env, caller: &Address) -> Result<(), AntiCheatError> {
    caller.require_auth();
    match get_admin(env) {
        Some(admin) if admin == *caller => Ok(()),
        _ => Err(AntiCheatError::Unauthorized),
    }
}

pub fn get_config(env: &Env) -> AntiCheatConfig {
    env.storage()
        .persistent()
        .get(&AntiCheatKey::Config)
        .unwrap_or_default()
}

pub fn set_config(
    env: &Env,
    caller: &Address,
    config: AntiCheatConfig,
) -> Result<(), AntiCheatError> {
    require_admin(env, caller)?;
    if !is_valid_config(&config) {
        return Err(AntiCheatError::InvalidConfig);
    }
    env.storage()
        .persistent()
        .set(&AntiCheatKey::Config, &config);
    env.events().publish(
        (symbol_short!("ac"), symbol_short!("config")),
        (
            caller.clone(),
            config.warning_threshold,
            config.temp_ban_threshold,
            config.permanent_ban_threshold,
        ),
    );
    Ok(())
}

// ── Queries ───────────────────────────────────────────────────────────────────

pub fn get_profile(env: &Env, player: &Address) -> PlayerRiskProfile {
    env.storage()
        .persistent()
        .get(&AntiCheatKey::Profile(player.clone()))
        .unwrap_or_default()
}

fn save_profile(env: &Env, player: &Address, profile: &PlayerRiskProfile) {
    env.storage()
        .persistent()
        .set(&AntiCheatKey::Profile(player.clone()), profile);
}

pub fn get_penalty(env: &Env, player: &Address) -> PenaltyRecord {
    env.storage()
        .persistent()
        .get(&AntiCheatKey::Penalty(player.clone()))
        .unwrap_or_default()
}

/// True when the player has an active temporary or permanent ban.
pub fn is_banned(env: &Env, player: &Address) -> bool {
    let penalty = get_penalty(env, player);
    match penalty.kind {
        PenaltyKind::PermanentBan => penalty.active,
        PenaltyKind::TemporaryBan => {
            penalty.active && penalty.expires_at > env.ledger().timestamp()
        }
        PenaltyKind::Warning | PenaltyKind::None => false,
    }
}

/// Guard called at the top of every mutating PvP entry point. No-op for
/// legitimate players; banned callers are rejected before any state is written.
pub fn enforce_allowed(env: &Env, player: &Address) -> Result<(), AntiCheatError> {
    if is_banned(env, player) {
        return Err(AntiCheatError::PlayerBanned);
    }
    Ok(())
}

pub fn get_detection(env: &Env, detection_id: u64) -> Result<DetectionRecord, AntiCheatError> {
    env.storage()
        .persistent()
        .get(&AntiCheatKey::Detection(detection_id))
        .ok_or(AntiCheatError::DetectionNotFound)
}

pub fn get_player_detections(
    env: &Env,
    player: &Address,
    limit: u32,
) -> Vec<DetectionRecord> {
    let ids: Vec<u64> = env
        .storage()
        .persistent()
        .get(&AntiCheatKey::PlayerDetections(player.clone()))
        .unwrap_or_else(|| Vec::new(env));
    let mut out = Vec::new(env);
    let start = if ids.len() > limit {
        ids.len() - limit
    } else {
        0
    };
    for i in start..ids.len() {
        if let Some(id) = ids.get(i) {
            if let Ok(record) = get_detection(env, id) {
                out.push_back(record);
            }
        }
    }
    out
}

pub fn get_review_queue(env: &Env, limit: u32) -> Vec<DetectionRecord> {
    let ids: Vec<u64> = env
        .storage()
        .persistent()
        .get(&AntiCheatKey::ReviewQueue)
        .unwrap_or_else(|| Vec::new(env));
    let mut out = Vec::new(env);
    let start = if ids.len() > limit {
        ids.len() - limit
    } else {
        0
    };
    for i in start..ids.len() {
        if let Some(id) = ids.get(i) {
            if let Ok(record) = get_detection(env, id) {
                if !record.reviewed {
                    out.push_back(record);
                }
            }
        }
    }
    out
}

pub fn get_appeal(env: &Env, appeal_id: u64) -> Result<AppealRecord, AntiCheatError> {
    env.storage()
        .persistent()
        .get(&AntiCheatKey::Appeal(appeal_id))
        .ok_or(AntiCheatError::AppealNotFound)
}

// ── Detection Recording ───────────────────────────────────────────────────────

fn next_counter(env: &Env, key: AntiCheatKey) -> u64 {
    let current: u64 = env.storage().persistent().get(&key).unwrap_or(0);
    let next = current.saturating_add(1);
    env.storage().persistent().set(&key, &next);
    next
}

fn detection_symbol(env: &Env, kind: DetectionType) -> Symbol {
    // Longer codes use `Symbol::new`; short ones use `symbol_short!` (9-char
    // limit). All are stable event/indexer codes.
    match kind {
        DetectionType::InvalidAction => symbol_short!("invalid"),
        DetectionType::ImpossibleState => Symbol::new(env, "impossible"),
        DetectionType::TimingAnomaly => symbol_short!("timing"),
        DetectionType::PatternAnomaly => symbol_short!("pattern"),
        DetectionType::BurstAnomaly => symbol_short!("burst"),
    }
}

fn severity_symbol(severity: Severity) -> Symbol {
    match severity {
        Severity::Low => symbol_short!("low"),
        Severity::Medium => symbol_short!("medium"),
        Severity::High => symbol_short!("high"),
        Severity::Critical => symbol_short!("critical"),
    }
}

fn penalty_symbol(kind: PenaltyKind) -> Symbol {
    match kind {
        PenaltyKind::None => symbol_short!("none"),
        PenaltyKind::Warning => symbol_short!("warning"),
        PenaltyKind::TemporaryBan => symbol_short!("ban_temp"),
        PenaltyKind::PermanentBan => symbol_short!("ban_perm"),
    }
}

fn enqueue_bounded(env: &Env, key: AntiCheatKey, id: u64, max_len: u32) {
    let mut ids: Vec<u64> = env
        .storage()
        .persistent()
        .get(&key)
        .unwrap_or_else(|| Vec::new(env));
    if ids.len() >= max_len {
        ids.remove(0);
    }
    ids.push_back(id);
    env.storage().persistent().set(&key, &ids);
}

fn remove_from_review_queue(env: &Env, detection_id: u64) {
    let ids: Vec<u64> = env
        .storage()
        .persistent()
        .get(&AntiCheatKey::ReviewQueue)
        .unwrap_or_else(|| Vec::new(env));
    let mut kept = Vec::new(env);
    for i in 0..ids.len() {
        if let Some(id) = ids.get(i) {
            if id != detection_id {
                kept.push_back(id);
            }
        }
    }
    env.storage()
        .persistent()
        .set(&AntiCheatKey::ReviewQueue, &kept);
}

#[allow(clippy::too_many_arguments)]
fn record_detection(
    env: &Env,
    player: &Address,
    kind: DetectionType,
    severity: Severity,
    combat_id: Option<u64>,
    action: Symbol,
    detail: Symbol,
    now: u64,
) -> u64 {
    let config = get_config(env);
    let detection_id = next_counter(env, AntiCheatKey::DetectionCounter);

    let mut profile = get_profile(env, player);
    let delta = score_for_detection(kind, severity);
    profile.score = add_score(profile.score, delta);
    profile.detection_count = profile.detection_count.saturating_add(1);
    profile.last_detection_at = now;

    let penalty_kind =
        penalty_for_profile(profile.score, severity, profile.warning_count, &config);
    let existing = get_penalty(env, player);
    if penalty_kind != existing.kind
        || (penalty_kind == PenaltyKind::TemporaryBan && existing.expires_at <= now)
    {
        let expires_at = if penalty_kind == PenaltyKind::TemporaryBan {
            now.saturating_add(config.temp_ban_duration_secs)
        } else {
            0
        };
        let penalty = PenaltyRecord {
            kind: penalty_kind,
            issued_at: now,
            expires_at,
            detection_id,
            active: penalty_kind != PenaltyKind::None,
        };
        env.storage()
            .persistent()
            .set(&AntiCheatKey::Penalty(player.clone()), &penalty);
        profile.penalty = penalty_kind;
        profile.banned_until = expires_at;
        if penalty_kind == PenaltyKind::Warning {
            profile.warning_count = profile.warning_count.saturating_add(1);
        }
        env.events().publish(
            (symbol_short!("ac"), symbol_short!("penalty")),
            (player.clone(), penalty_symbol(penalty_kind), detection_id, expires_at),
        );
    }
    save_profile(env, player, &profile);

    let record = DetectionRecord {
        detection_id,
        player: player.clone(),
        detection_type: kind,
        severity,
        score_delta: delta,
        score_after: profile.score,
        combat_id,
        action,
        detail,
        timestamp: now,
        auto_penalty: penalty_kind,
        reviewed: false,
        review_decision: ReviewDecision::Pending,
    };
    env.storage()
        .persistent()
        .set(&AntiCheatKey::Detection(detection_id), &record);
    enqueue_bounded(
        env,
        AntiCheatKey::PlayerDetections(player.clone()),
        detection_id,
        config.max_detections_per_player,
    );

    if severity >= Severity::Medium || profile.score >= config.review_threshold {
        enqueue_bounded(
            env,
            AntiCheatKey::ReviewQueue,
            detection_id,
            config.max_review_queue,
        );
        env.events().publish(
            (symbol_short!("ac"), symbol_short!("review")),
            (player.clone(), detection_id),
        );
    }

    env.events().publish(
        (symbol_short!("ac"), symbol_short!("detect")),
        (
            player.clone(),
            detection_id,
            detection_symbol(env, kind),
            severity_symbol(severity),
            profile.score,
        ),
    );

    detection_id
}

/// Public entry used by the PvP engine when input cannot be legitimate.
#[allow(clippy::too_many_arguments)]
pub fn record_cheat_attempt(
    env: &Env,
    player: &Address,
    kind: DetectionType,
    severity: Severity,
    combat_id: Option<u64>,
    action: Symbol,
    detail: Symbol,
    now: u64,
) -> u64 {
    record_detection(env, player, kind, severity, combat_id, action, detail, now)
}

// ── PvP Validation Hooks ──────────────────────────────────────────────────────

/// Observe one PvP action for timing/pattern analysis. Called by every
/// mutating PvP entry point after its ban check. Never rejects legitimate
/// play: it only records evidence when conservative thresholds are crossed.
pub fn observe_action(
    env: &Env,
    player: &Address,
    action: Symbol,
    subject: Option<u64>,
    now: u64,
) {
    let config = get_config(env);
    let mut profile = get_profile(env, player);
    if profile.last_detection_at > 0 {
        profile.score = decay_score(
            profile.score,
            now.saturating_sub(profile.last_detection_at),
            SCORE_HALF_LIFE_SECS,
        );
    }

    let key = AntiCheatKey::ActionWindow(player.clone());
    let mut timestamps: Vec<u64> = env
        .storage()
        .persistent()
        .get(&key)
        .unwrap_or_else(|| Vec::new(env));

    // Copy the bounded Soroban Vec into a fixed buffer so the pure helpers can
    // operate on a plain slice without allocation.
    let mut buffer = [0u64; MAX_ACTION_SAMPLES as usize];
    let mut filled = 0usize;
    // Soroban `Vec::len()` is u32; the fixed buffer is usize-indexed.
    let start = if timestamps.len() > MAX_ACTION_SAMPLES {
        timestamps.len() - MAX_ACTION_SAMPLES
    } else {
        0
    };
    for i in start..timestamps.len() {
        if let Some(t) = timestamps.get(i) {
            if filled < buffer.len() {
                buffer[filled] = t;
                filled += 1;
            }
        }
    }
    let sample = &buffer[..filled];

    let can_record = cooldown_elapsed(now, profile.last_detection_at, config.detection_cooldown_secs);
    if can_record && is_burst(sample, now, config.action_window_secs, config.max_actions_per_window) {
        record_detection(
            env,
            player,
            DetectionType::BurstAnomaly,
            Severity::Medium,
            subject,
            action.clone(),
            symbol_short!("burst"),
            now,
        );
        profile = get_profile(env, player);
    } else if can_record
        && is_regular_intervals(
            sample,
            REGULAR_TOLERANCE_SECS,
            REGULAR_MIN_SAMPLES,
            REGULAR_RATIO_PCT,
        )
    {
        record_detection(
            env,
            player,
            DetectionType::PatternAnomaly,
            Severity::Medium,
            subject,
            action.clone(),
            symbol_short!("regular"),
            now,
        );
        profile = get_profile(env, player);
    }

    if filled > 0 {
        let last = sample[filled - 1];
        if cooldown_elapsed(now, profile.last_detection_at, config.detection_cooldown_secs)
            && is_timing_violation(now, last, config.min_action_interval_secs)
        {
            record_detection(
                env,
                player,
                DetectionType::TimingAnomaly,
                Severity::Low,
                subject,
                action.clone(),
                symbol_short!("interval"),
                now,
            );
            profile = get_profile(env, player);
        }
    }

    timestamps.push_back(now);
    if timestamps.len() > MAX_ACTION_SAMPLES {
        timestamps.remove(0);
    }
    env.storage().persistent().set(&key, &timestamps);
    save_profile(env, player, &profile);
}

/// Derive the exact combat state the engine may produce for a move. This is
/// called before the move is applied so the expectation is independent of the
/// mutated state.
pub fn validate_move(
    env: &Env,
    player: &Address,
    combat_id: u64,
    combat: &CombatState,
    move_type: Symbol,
    power: u32,
    now: u64,
) -> MoveValidation {
    let is_player1 = *player == combat.player1;
    let energy_cost = power / 2;
    let self_energy = if is_player1 {
        combat.player1_energy
    } else {
        combat.player2_energy
    };
    let self_hp = if is_player1 {
        combat.player1_hp
    } else {
        combat.player2_hp
    };
    let opponent_hp = if is_player1 {
        combat.player2_hp
    } else {
        combat.player1_hp
    };

    let is_special = move_type == symbol_short!("special");
    let mut damage = 0u32;
    let mut heal = 0u32;
    let mut energy_delta: i64 = -(energy_cost as i64);

    if move_type == symbol_short!("attack") {
        damage = power;
    } else if is_special {
        damage = power.saturating_mul(2);
    } else if move_type == symbol_short!("heal") {
        heal = power / 2;
    } else if move_type == symbol_short!("defend") {
        energy_delta = energy_delta.saturating_add(10);
    } else {
        record_cheat_attempt(
            env,
            player,
            DetectionType::InvalidAction,
            Severity::High,
            Some(combat_id),
            move_type.clone(),
            symbol_short!("move_type"),
            now,
        );
    }

    if is_impossible_damage(power, damage, is_special, MAX_MOVE_POWER) {
        record_cheat_attempt(
            env,
            player,
            DetectionType::ImpossibleState,
            Severity::High,
            Some(combat_id),
            move_type.clone(),
            symbol_short!("damage"),
            now,
        );
    }
    if is_impossible_heal(power, heal, MAX_HP) {
        record_cheat_attempt(
            env,
            player,
            DetectionType::ImpossibleState,
            Severity::High,
            Some(combat_id),
            move_type.clone(),
            symbol_short!("heal"),
            now,
        );
    }
    if is_impossible_energy(power, energy_cost, MAX_MOVE_POWER) {
        record_cheat_attempt(
            env,
            player,
            DetectionType::InvalidAction,
            Severity::High,
            Some(combat_id),
            move_type.clone(),
            symbol_short!("energy"),
            now,
        );
    }

    let self_energy_after = if energy_delta >= 0 {
        self_energy.saturating_add(energy_delta.unsigned_abs() as u32)
    } else {
        self_energy.saturating_sub(energy_delta.unsigned_abs() as u32)
    };

    let expected = if is_player1 {
        CombatSnapshot {
            player1_hp: self_hp.saturating_add(heal).min(MAX_HP),
            player2_hp: opponent_hp.saturating_sub(damage),
            player1_energy: self_energy_after,
            player2_energy: combat.player2_energy,
        }
    } else {
        CombatSnapshot {
            player1_hp: opponent_hp.saturating_sub(damage),
            player2_hp: self_hp.saturating_add(heal).min(MAX_HP),
            player1_energy: combat.player1_energy,
            player2_energy: self_energy_after,
        }
    };

    MoveValidation {
        is_player1,
        energy_cost,
        damage,
        heal,
        expected,
    }
}

/// Compare the applied state with the expectation. On mismatch, log a Critical
/// detection and restore the safe expected values so the exploit has no effect.
/// Must be called after the move is applied but before winner resolution.
pub fn verify_state_transition(
    env: &Env,
    player: &Address,
    combat_id: u64,
    expected: &MoveValidation,
    after: &mut CombatState,
    now: u64,
) {
    let actual = CombatSnapshot {
        player1_hp: after.player1_hp,
        player2_hp: after.player2_hp,
        player1_energy: after.player1_energy,
        player2_energy: after.player2_energy,
    };
    if !snapshots_equal(&actual, &expected.expected) {
        record_cheat_attempt(
            env,
            player,
            DetectionType::ImpossibleState,
            Severity::Critical,
            Some(combat_id),
            symbol_short!("move"),
            Symbol::new(env, "state_mismatch"),
            now,
        );
        after.player1_hp = expected.expected.player1_hp;
        after.player2_hp = expected.expected.player2_hp;
        after.player1_energy = expected.expected.player1_energy;
        after.player2_energy = expected.expected.player2_energy;
    }

    // Finished combats intentionally do not switch the turn.
    let combat_over = after.player1_hp == 0 || after.player2_hp == 0;
    if !combat_over {
        let expected_turn = if expected.is_player1 {
            after.player2.clone()
        } else {
            after.player1.clone()
        };
        if after.turn != expected_turn {
            record_cheat_attempt(
                env,
                player,
                DetectionType::ImpossibleState,
                Severity::High,
                Some(combat_id),
                symbol_short!("move"),
                symbol_short!("turn"),
                now,
            );
            after.turn = expected_turn;
        }
    }
}

// ── Manual Review ─────────────────────────────────────────────────────────────

fn clear_penalty_for(env: &Env, player: &Address, detection_id: u64) {
    let penalty = get_penalty(env, player);
    if penalty.detection_id == detection_id {
        env.storage()
            .persistent()
            .set(&AntiCheatKey::Penalty(player.clone()), &PenaltyRecord::default());
        let mut profile = get_profile(env, player);
        profile.penalty = PenaltyKind::None;
        profile.banned_until = 0;
        save_profile(env, player, &profile);
    }
}

/// Admin decision for a queued detection. `Cleared` restores the score and
/// lifts the penalty from this detection; `Reversed` additionally resets the
/// profile; `Upheld` leaves the penalty in place.
pub fn resolve_review(
    env: &Env,
    admin: &Address,
    detection_id: u64,
    decision: ReviewDecision,
    reason: Symbol,
) -> Result<(), AntiCheatError> {
    require_admin(env, admin)?;
    let mut record = get_detection(env, detection_id)?;
    if record.reviewed {
        return Err(AntiCheatError::AlreadyResolved);
    }

    match decision {
        ReviewDecision::Pending => return Err(AntiCheatError::InvalidDecision),
        ReviewDecision::Cleared | ReviewDecision::Reversed => {
            let mut profile = get_profile(env, &record.player);
            if decision == ReviewDecision::Reversed {
                profile.score = 0;
                profile.warning_count = 0;
            } else {
                profile.score = profile.score.saturating_sub(record.score_delta);
            }
            save_profile(env, &record.player, &profile);
            // Runs after save_profile so its own profile write (penalty cleared)
            // is not overwritten by the score update above.
            clear_penalty_for(env, &record.player, detection_id);
        }
        ReviewDecision::Upheld => {}
    }

    record.reviewed = true;
    record.review_decision = decision;
    env.storage()
        .persistent()
        .set(&AntiCheatKey::Detection(detection_id), &record);
    remove_from_review_queue(env, detection_id);
    env.events().publish(
        (symbol_short!("ac"), symbol_short!("reviewed")),
        (admin.clone(), detection_id, reason),
    );
    Ok(())
}

// ── Appeals ───────────────────────────────────────────────────────────────────

/// File an appeal for a detection that penalised the caller. One appeal per
/// detection, and only inside the configured appeal window.
pub fn submit_appeal(
    env: &Env,
    player: &Address,
    detection_id: u64,
    reason: Symbol,
) -> Result<u64, AntiCheatError> {
    player.require_auth();
    let record = get_detection(env, detection_id)?;
    if record.player != *player {
        return Err(AntiCheatError::Unauthorized);
    }
    let key = AntiCheatKey::AppealByDetection(detection_id);
    if env.storage().persistent().get::<_, u64>(&key).is_some() {
        return Err(AntiCheatError::AlreadyAppealed);
    }
    let config = get_config(env);
    let now = env.ledger().timestamp();
    let deadline = record.timestamp.saturating_add(config.appeal_window_secs);
    if now > deadline {
        return Err(AntiCheatError::AppealWindowClosed);
    }

    let appeal_id = next_counter(env, AntiCheatKey::AppealCounter);
    let appeal = AppealRecord {
        appeal_id,
        detection_id,
        player: player.clone(),
        reason,
        created_at: now,
        deadline,
        status: AppealStatus::Pending,
        reviewer: None,
        resolved_at: 0,
    };
    env.storage()
        .persistent()
        .set(&AntiCheatKey::Appeal(appeal_id), &appeal);
    env.storage().persistent().set(&key, &appeal_id);
    enqueue_bounded(
        env,
        AntiCheatKey::PlayerAppeals(player.clone()),
        appeal_id,
        MAX_APPEALS_PER_PLAYER,
    );
    env.events().publish(
        (symbol_short!("ac"), symbol_short!("appeal")),
        (player.clone(), appeal_id, detection_id),
    );
    Ok(appeal_id)
}

/// Admin resolution of an appeal. Approval reverses the underlying detection
/// and clears any penalty issued by it; rejection leaves it intact.
pub fn resolve_appeal(
    env: &Env,
    admin: &Address,
    appeal_id: u64,
    approve: bool,
    reason: Symbol,
) -> Result<(), AntiCheatError> {
    require_admin(env, admin)?;
    let mut appeal = get_appeal(env, appeal_id)?;
    if appeal.status != AppealStatus::Pending {
        return Err(AntiCheatError::AlreadyResolved);
    }
    appeal.status = if approve {
        AppealStatus::Approved
    } else {
        AppealStatus::Rejected
    };
    appeal.reviewer = Some(admin.clone());
    appeal.resolved_at = env.ledger().timestamp();
    env.storage()
        .persistent()
        .set(&AntiCheatKey::Appeal(appeal_id), &appeal);

    if approve {
        let mut detection = get_detection(env, appeal.detection_id)?;
        if !detection.reviewed {
            detection.reviewed = true;
            detection.review_decision = ReviewDecision::Reversed;
            env.storage().persistent().set(
                &AntiCheatKey::Detection(appeal.detection_id),
                &detection,
            );
            remove_from_review_queue(env, appeal.detection_id);
        }
        let mut profile = get_profile(env, &appeal.player);
        profile.score = 0;
        profile.warning_count = 0;
        save_profile(env, &appeal.player, &profile);
        clear_penalty_for(env, &appeal.player, appeal.detection_id);
    }

    env.events().publish(
        (symbol_short!("ac"), symbol_short!("appealed")),
        (admin.clone(), appeal_id, approve, reason),
    );
    Ok(())
}

// ── Tests (pure logic only — no Soroban env) ──────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exploit_impossible_damage_is_detected() {
        assert!(is_impossible_damage(10, 100, false, MAX_MOVE_POWER));
        assert!(is_impossible_damage(10, 30, false, MAX_MOVE_POWER));
        assert!(!is_impossible_damage(10, 10, false, MAX_MOVE_POWER));
        // Special doubles power but is still bounded by 2 * MAX_MOVE_POWER.
        assert!(!is_impossible_damage(10, 20, true, MAX_MOVE_POWER));
        assert!(is_impossible_damage(MAX_MOVE_POWER, 300, true, MAX_MOVE_POWER));
    }

    #[test]
    fn exploit_impossible_heal_and_energy_are_detected() {
        assert!(is_impossible_heal(10, 20, MAX_HP));
        assert!(!is_impossible_heal(10, 5, MAX_HP));
        assert!(is_impossible_energy(10, 9, MAX_MOVE_POWER));
        assert!(is_impossible_energy(MAX_MOVE_POWER + 1, 50, MAX_MOVE_POWER));
        assert!(!is_impossible_energy(10, 5, MAX_MOVE_POWER));
    }

    #[test]
    fn state_mismatch_detects_impossible_state_change() {
        let expected = CombatSnapshot {
            player1_hp: 90,
            player2_hp: 80,
            player1_energy: 95,
            player2_energy: 100,
        };
        let tampered = CombatSnapshot {
            player2_hp: 0, // exploit: one hit killed a full-health opponent
            ..expected
        };
        assert!(!snapshots_equal(&expected, &tampered));
        assert!(snapshots_equal(&expected, &expected));
    }

    #[test]
    fn scoring_is_bounded_and_decays() {
        assert_eq!(add_score(99, 10), MAX_SCORE);
        assert_eq!(score_for_detection(DetectionType::ImpossibleState, Severity::Critical), MAX_SCORE);
        assert!(score_for_detection(DetectionType::TimingAnomaly, Severity::Low) < score_for_detection(DetectionType::TimingAnomaly, Severity::High));
        assert_eq!(decay_score(80, 0, SCORE_HALF_LIFE_SECS), 80);
        assert_eq!(decay_score(80, SCORE_HALF_LIFE_SECS, SCORE_HALF_LIFE_SECS), 40);
        // Very old timestamps must not loop forever or underflow.
        assert_eq!(decay_score(80, u64::MAX, SCORE_HALF_LIFE_SECS), 0);
    }

    #[test]
    fn penalty_escalation_is_deterministic() {
        let config = AntiCheatConfig::default();
        assert_eq!(penalty_for_profile(10, Severity::Low, 0, &config), PenaltyKind::None);
        assert_eq!(penalty_for_profile(45, Severity::Low, 0, &config), PenaltyKind::Warning);
        assert_eq!(penalty_for_profile(70, Severity::Low, 0, &config), PenaltyKind::TemporaryBan);
        assert_eq!(penalty_for_profile(95, Severity::Low, 0, &config), PenaltyKind::PermanentBan);
        assert_eq!(penalty_for_profile(20, Severity::Critical, 0, &config), PenaltyKind::TemporaryBan);
        assert_eq!(
            penalty_for_profile(20, Severity::Low, WARNINGS_BEFORE_TEMP_BAN, &config),
            PenaltyKind::TemporaryBan
        );
    }

    #[test]
    fn timing_manipulation_is_caught() {
        assert!(is_timing_violation(100, 100, MIN_ACTION_INTERVAL_SECS));
        assert!(is_timing_violation(100, 105, MIN_ACTION_INTERVAL_SECS));
        assert!(is_timing_violation(99, 100, MIN_ACTION_INTERVAL_SECS)); // rollback
        assert!(!is_timing_violation(110, 100, MIN_ACTION_INTERVAL_SECS));
    }

    #[test]
    fn burst_and_regular_patterns_are_caught() {
        let burst = [100u64, 101, 102, 103, 104, 105, 106, 107];
        assert!(is_burst(&burst, 108, ACTION_WINDOW_SECS, MAX_ACTIONS_PER_WINDOW));
        assert!(!is_burst(&[100, 200, 300], 400, ACTION_WINDOW_SECS, MAX_ACTIONS_PER_WINDOW));

        let bot = [100u64, 110, 120, 130, 140, 150];
        assert!(is_regular_intervals(&bot, REGULAR_TOLERANCE_SECS, REGULAR_MIN_SAMPLES, REGULAR_RATIO_PCT));
        let human = [100u64, 111, 119, 137, 142, 168];
        assert!(!is_regular_intervals(&human, REGULAR_TOLERANCE_SECS, REGULAR_MIN_SAMPLES, REGULAR_RATIO_PCT));
    }

    #[test]
    fn cooldown_and_config_validation() {
        assert!(cooldown_elapsed(100, 0, DETECTION_COOLDOWN_SECS));
        assert!(!cooldown_elapsed(100, 90, DETECTION_COOLDOWN_SECS));
        assert!(cooldown_elapsed(130, 100, DETECTION_COOLDOWN_SECS));

        let config = AntiCheatConfig::default();
        assert!(is_valid_config(&config));
        let mut bad = config;
        bad.warning_threshold = bad.temp_ban_threshold;
        assert!(!is_valid_config(&bad));
    }
}
