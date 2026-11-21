//! # Bot Detection and Prevention (#278, #704)
//!
//! Behavioural bot detection for Stellar Nebula Nomad.
//!
//! - **Behaviour analysis** — every game action calls [`record_action`], which
//!   keeps a rolling history of `(timestamp, action)` per player.
//! - **Pattern detection** — bursts inside one ledger second, rate-limit
//!   violations, metronome-regular timing, long runs of one repeated action,
//!   and (via device fingerprints) multi-accounting and account farms.
//! - **Scoring** — each signal adds a weighted amount to a 0–100 suspicion
//!   score. Trusted players get half weight, untrusted players 1.5×. The score
//!   decays by [`DECAY_PER_HOUR`] for every hour without new signals.
//! - **Automated responses** — the score maps to a [`BotResponse`]:
//!   allow → monitor → CAPTCHA → temporary block. Bans are only ever applied
//!   by a human through the review queue.
//! - **Review queue** — players crossing the CAPTCHA threshold get a
//!   [`ReviewCase`]; admins clear or ban them with [`resolve_case`].
//!
//! ## Timestamps
//!
//! Soroban ledger timestamps are in **seconds** and ledgers close every few
//! seconds, so sub-second timing cannot be observed on-chain. "Too fast" means
//! more than [`MAX_ACTIONS_PER_SECOND`] actions sharing one ledger timestamp.
//!
//! ## Rollback semantics
//!
//! A Soroban call that returns `Err` rolls back all of its writes. Detection
//! therefore never fails the call: [`record_action`] returns
//! `Ok(ActionAssessment { allowed: false, .. })` for a rejected action so the
//! penalty is persisted. Gates that only read state (ban, block, CAPTCHA)
//! return `Err`. Integrators should treat `allowed == false` as a soft
//! rejection rather than converting it into an error of the outer call.

use soroban_sdk::{contracterror, contracttype, symbol_short, Address, BytesN, Env, Symbol, Vec};

// ─── Storage Keys ─────────────────────────────────────────────────────────

#[derive(Clone)]
#[contracttype]
pub enum BotKey {
    /// Module admin.
    Admin,
    /// Per-player [`BotProfile`].
    Profile(Address),
    /// Rolling action history (temporary storage).
    History(Address),
    /// CAPTCHA challenge: (player, challenge_id) → challenge data.
    CaptchaChallenge(Address, u64),
    /// Global challenge counter.
    BotChallengeCounter,
    /// Device fingerprint → linked accounts.
    Fingerprint(u64),
    /// Review case counter.
    CaseCounter,
    /// Review case by ID.
    Case(u64),
    /// Open case ID per player.
    OpenCase(Address),
    /// IDs of open cases, oldest first.
    ReviewQueue,
}

// ─── Constants ────────────────────────────────────────────────────────────

/// Actions remembered per player.
pub const HISTORY_SIZE: u32 = 20;
/// Rate-limit window length in seconds.
pub const WINDOW_SECS: u64 = 60;
/// Actions allowed per [`WINDOW_SECS`].
pub const MAX_ACTIONS_PER_WINDOW: u32 = 15;
/// Actions allowed with the same ledger timestamp.
pub const MAX_ACTIONS_PER_SECOND: u32 = 3;
/// Samples needed before timing regularity is judged.
pub const REGULAR_MIN_SAMPLES: u32 = 10;
/// Share of identical intervals (percent) that marks timing as scripted.
pub const REGULAR_RATIO_PCT: u32 = 90;
/// Consecutive identical actions that mark a repetitive loop.
pub const REPEAT_RUN: u32 = 15;
/// Accounts allowed per device before it counts as multi-accounting.
pub const MULTI_ACCOUNT_LIMIT: u32 = 3;
/// Accounts per device that indicate an account farm.
pub const FARM_LIMIT: u32 = 6;
/// Accounts tracked per fingerprint.
pub const MAX_LINKED_ACCOUNTS: u32 = 50;

/// Score lost per hour without new signals.
pub const DECAY_PER_HOUR: u32 = 10;
/// Maximum suspicion score.
pub const MAX_SUSPICION: u32 = 100;
/// Score at which the player is monitored (events only).
pub const MONITOR_THRESHOLD: u32 = 30;
/// Score at which a CAPTCHA is required and a review case opens.
pub const CAPTCHA_THRESHOLD: u32 = 60;
/// Score at which the player is temporarily blocked.
pub const BLOCK_THRESHOLD: u32 = 85;
/// Length of an automatic block.
pub const BLOCK_SECS: u64 = 3_600;
/// Failed CAPTCHAs that trigger an automatic block.
pub const MAX_CAPTCHA_FAILURES: u32 = 3;
/// Score removed by solving a CAPTCHA.
pub const CAPTCHA_SOLVE_RELIEF: u32 = 30;
/// Largest review-queue page.
pub const MAX_QUEUE_PAGE: u32 = 50;

// Signal bits (also used in events and review cases).
pub const SIG_BURST: u32 = 1 << 0;
pub const SIG_RATE_LIMIT: u32 = 1 << 1;
pub const SIG_REGULAR_TIMING: u32 = 1 << 2;
pub const SIG_REPETITIVE: u32 = 1 << 3;
pub const SIG_MULTI_ACCOUNT: u32 = 1 << 4;
pub const SIG_FARMING: u32 = 1 << 5;
pub const SIG_CAPTCHA_FAIL: u32 = 1 << 6;
pub const SIG_MANUAL: u32 = 1 << 7;

/// Base score weight of a single signal bit.
pub fn signal_weight(signal: u32) -> u32 {
    match signal {
        SIG_BURST => 20,
        SIG_RATE_LIMIT => 10,
        SIG_REGULAR_TIMING => 15,
        SIG_REPETITIVE => 10,
        SIG_MULTI_ACCOUNT => 25,
        SIG_FARMING => 40,
        SIG_CAPTCHA_FAIL => 15,
        _ => 0,
    }
}

// ─── Data Types ───────────────────────────────────────────────────────────

/// Trust levels.
pub const TRUST_UNTRUSTED: u32 = 0;
pub const TRUST_NORMAL: u32 = 1;
pub const TRUST_TRUSTED: u32 = 2;

/// Per-player detection state.
#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct BotProfile {
    /// Suspicion score, 0–100.
    pub score: u32,
    /// Every signal ever raised (bitmask of `SIG_*`).
    pub signals: u32,
    /// Number of signal events.
    pub violations: u32,
    /// 0 untrusted, 1 normal, 2 trusted.
    pub trust: u32,
    pub captcha_required: bool,
    pub captcha_failures: u32,
    /// Blocked until this timestamp (0 = not blocked).
    pub blocked_until: u64,
    /// Banned by an admin.
    pub banned: bool,
    /// Timestamp decay was last applied.
    pub last_scored_at: u64,
}

/// One remembered action.
#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct ActionRecord {
    pub at: u64,
    pub action: Symbol,
}

/// Automated response for a score band.
#[contracttype]
#[derive(Clone, Copy, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum BotResponse {
    Allow = 0,
    /// Allowed, but an event is emitted for off-chain monitoring.
    Monitor = 1,
    /// Next action requires a solved CAPTCHA.
    Captcha = 2,
    /// Temporarily blocked for [`BLOCK_SECS`].
    Block = 3,
    /// Banned by an admin.
    Ban = 4,
}

/// Result of [`record_action`].
#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct ActionAssessment {
    /// False when the action itself was rejected (burst / rate limit).
    pub allowed: bool,
    /// Signals raised by this action.
    pub signals: u32,
    /// Score after this action.
    pub score: u32,
    pub response: BotResponse,
}

/// A CAPTCHA challenge issued to a suspicious player.
#[derive(Clone)]
#[contracttype]
pub struct CaptchaChallengeData {
    pub challenge_id: u64,
    pub player: Address,
    /// Hash of the expected answer.
    pub answer_hash: BytesN<32>,
    pub issued_at: u64,
    pub expires_at: u64,
    pub solved: bool,
}

/// Review case state.
#[contracttype]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum CaseStatus {
    Open = 0,
    Cleared = 1,
    Banned = 2,
}

/// Admin decision on a case.
#[contracttype]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum CaseVerdict {
    /// False positive: reset the player's score and restrictions.
    Clear = 0,
    /// Confirmed bot: ban the player.
    Ban = 1,
}

/// An entry in the admin review queue.
#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct ReviewCase {
    pub id: u64,
    pub player: Address,
    /// Highest score seen while the case was open.
    pub peak_score: u32,
    /// Signals seen while the case was open.
    pub signals: u32,
    pub opened_at: u64,
    pub updated_at: u64,
    pub status: CaseStatus,
    pub resolved_at: u64,
    /// Admin who resolved the case (`None` while open).
    pub resolved_by: Option<Address>,
}

// ─── Errors ───────────────────────────────────────────────────────────────

#[contracterror]
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum BotError {
    /// Player is flagged as suspicious and must solve a CAPTCHA.
    CaptchaRequired = 1,
    /// CAPTCHA challenge not found.
    ChallengeNotFound = 2,
    /// CAPTCHA challenge expired.
    ChallengeExpired = 3,
    /// CAPTCHA answer incorrect (kept for ABI compatibility; wrong answers are
    /// now reported as `Ok(false)` so the failure is persisted).
    IncorrectAnswer = 4,
    /// Action too fast — potential automation detected.
    ActionTooFast = 5,
    /// Player is temporarily blocked.
    PlayerBlocked = 6,
    /// Player is banned.
    PlayerBanned = 7,
    /// Caller is not the module admin.
    Unauthorized = 8,
    /// Admin already set.
    AlreadyInitialized = 9,
    /// Review case not found.
    CaseNotFound = 10,
    /// Review case already resolved.
    CaseClosed = 11,
    /// Trust level out of range.
    InvalidTrustLevel = 12,
    /// Challenge already solved.
    ChallengeSolved = 13,
}

impl crate::error_standard::StandardContractError for BotError {
    fn descriptor(self) -> crate::error_standard::ErrorDescriptor {
        use crate::error_standard::ErrorKind;
        let (kind, retryable) = match self {
            Self::CaptchaRequired | Self::PlayerBlocked => (ErrorKind::Authorization, true),
            Self::PlayerBanned | Self::Unauthorized => (ErrorKind::Authorization, false),
            Self::ChallengeNotFound | Self::CaseNotFound => (ErrorKind::NotFound, false),
            Self::ActionTooFast => (ErrorKind::ResourceLimit, true),
            Self::AlreadyInitialized | Self::CaseClosed | Self::ChallengeSolved => {
                (ErrorKind::Conflict, false)
            }
            Self::ChallengeExpired | Self::IncorrectAnswer | Self::InvalidTrustLevel => {
                (ErrorKind::Validation, false)
            }
        };
        crate::error_standard::ErrorDescriptor {
            module: "bot_detection",
            code: self as u32,
            kind,
            retryable,
        }
    }
}

// ─── Admin ────────────────────────────────────────────────────────────────

/// Set the module admin. One-shot.
pub fn init_bot_detection(env: &Env, admin: Address) -> Result<(), BotError> {
    admin.require_auth();
    if env.storage().instance().has(&BotKey::Admin) {
        return Err(BotError::AlreadyInitialized);
    }
    env.storage().instance().set(&BotKey::Admin, &admin);
    Ok(())
}

fn require_admin(env: &Env, admin: &Address) -> Result<(), BotError> {
    admin.require_auth();
    match env.storage().instance().get::<_, Address>(&BotKey::Admin) {
        Some(stored) if stored == *admin => Ok(()),
        _ => Err(BotError::Unauthorized),
    }
}

// ─── Profile helpers ──────────────────────────────────────────────────────

fn default_profile(env: &Env) -> BotProfile {
    BotProfile {
        score: 0,
        signals: 0,
        violations: 0,
        trust: TRUST_NORMAL,
        captcha_required: false,
        captcha_failures: 0,
        blocked_until: 0,
        banned: false,
        last_scored_at: env.ledger().timestamp(),
    }
}

/// A player's detection state (defaults for unknown players).
pub fn get_bot_profile(env: &Env, player: &Address) -> BotProfile {
    env.storage()
        .persistent()
        .get(&BotKey::Profile(player.clone()))
        .unwrap_or_else(|| default_profile(env))
}

fn store_profile(env: &Env, player: &Address, profile: &BotProfile) {
    env.storage()
        .persistent()
        .set(&BotKey::Profile(player.clone()), profile);
}

/// Apply hourly decay since `last_scored_at`. Lifts a CAPTCHA requirement
/// once the score falls back below the threshold.
fn apply_decay(env: &Env, profile: &mut BotProfile) {
    let now = env.ledger().timestamp();
    let hours = now.saturating_sub(profile.last_scored_at) / 3_600;
    if hours > 0 {
        let drop = (hours.min(u32::MAX as u64) as u32).saturating_mul(DECAY_PER_HOUR);
        profile.score = profile.score.saturating_sub(drop);
        profile.last_scored_at = profile.last_scored_at + hours * 3_600;
        if profile.score < CAPTCHA_THRESHOLD {
            profile.captcha_required = false;
        }
    }
}

/// Score added for `signals`, scaled by trust level.
pub fn weighted_score(signals: u32, trust: u32) -> u32 {
    let mut total = 0u32;
    let mut bit = 1u32;
    while bit <= SIG_MANUAL {
        if signals & bit != 0 {
            total = total.saturating_add(signal_weight(bit));
        }
        bit <<= 1;
    }
    match trust {
        TRUST_TRUSTED => total / 2,
        TRUST_UNTRUSTED => total.saturating_mul(3) / 2,
        _ => total,
    }
}

/// Automated response for a score.
pub fn response_for_score(score: u32) -> BotResponse {
    if score >= BLOCK_THRESHOLD {
        BotResponse::Block
    } else if score >= CAPTCHA_THRESHOLD {
        BotResponse::Captcha
    } else if score >= MONITOR_THRESHOLD {
        BotResponse::Monitor
    } else {
        BotResponse::Allow
    }
}

/// Add `signals` to `player`'s profile, apply the automated response and open
/// or update a review case. Returns the response.
fn raise(env: &Env, player: &Address, profile: &mut BotProfile, signals: u32) -> BotResponse {
    if signals != 0 {
        profile.score = profile
            .score
            .saturating_add(weighted_score(signals, profile.trust))
            .min(MAX_SUSPICION);
        profile.signals |= signals;
        profile.violations = profile.violations.saturating_add(1);
        profile.last_scored_at = env.ledger().timestamp();
    }

    let response = if profile.banned {
        BotResponse::Ban
    } else {
        response_for_score(profile.score)
    };
    match response {
        BotResponse::Block => {
            profile.captcha_required = true;
            profile.blocked_until = env.ledger().timestamp() + BLOCK_SECS;
        }
        BotResponse::Captcha => profile.captcha_required = true,
        _ => {}
    }

    if signals != 0 {
        env.events().publish(
            (symbol_short!("bot"), symbol_short!("signal")),
            (player.clone(), signals, profile.score, response),
        );
        if profile.score >= CAPTCHA_THRESHOLD {
            open_or_update_case(env, player, profile.score, signals);
        }
    }
    response
}

// ─── Behavioural Analysis ─────────────────────────────────────────────────

/// Read-only gate: `Err` if the player is banned, blocked or CAPTCHA-gated.
pub fn check_access(env: &Env, player: &Address) -> Result<(), BotError> {
    let profile = get_bot_profile(env, player);
    if profile.banned {
        return Err(BotError::PlayerBanned);
    }
    if profile.blocked_until > env.ledger().timestamp() {
        return Err(BotError::PlayerBlocked);
    }
    if profile.captcha_required {
        // Decay may already have lifted the requirement.
        let mut decayed = profile.clone();
        apply_decay(env, &mut decayed);
        if decayed.captcha_required {
            return Err(BotError::CaptchaRequired);
        }
    }
    Ok(())
}

/// Record and assess one game action. See the module docs for semantics.
pub fn record_action(
    env: &Env,
    player: &Address,
    action: &Symbol,
) -> Result<ActionAssessment, BotError> {
    check_access(env, player)?;

    let now = env.ledger().timestamp();
    let mut profile = get_bot_profile(env, player);
    apply_decay(env, &mut profile);

    let key = BotKey::History(player.clone());
    let mut history: Vec<ActionRecord> = env
        .storage()
        .temporary()
        .get(&key)
        .unwrap_or_else(|| Vec::new(env));

    let mut same_second = 0u32;
    let mut in_window = 0u32;
    for rec in history.iter() {
        if rec.at == now {
            same_second += 1;
        }
        if rec.at + WINDOW_SECS > now {
            in_window += 1;
        }
    }

    let mut signals = 0u32;
    let mut allowed = true;
    if same_second >= MAX_ACTIONS_PER_SECOND {
        signals |= SIG_BURST;
        allowed = false;
    } else if in_window >= MAX_ACTIONS_PER_WINDOW {
        signals |= SIG_RATE_LIMIT;
        allowed = false;
    }

    if allowed {
        history.push_back(ActionRecord {
            at: now,
            action: action.clone(),
        });
        while history.len() > HISTORY_SIZE {
            history.pop_front();
        }
        env.storage().temporary().set(&key, &history);
        signals |= analyze_patterns(&history);
    }

    let response = raise(env, player, &mut profile, signals);
    store_profile(env, player, &profile);

    Ok(ActionAssessment {
        allowed,
        signals,
        score: profile.score,
        response,
    })
}

/// Detect scripted timing and repetitive loops in an action history.
pub fn analyze_patterns(history: &Vec<ActionRecord>) -> u32 {
    let len = history.len();
    let mut signals = 0u32;

    // Metronome timing: almost every interval identical (and non-zero, since
    // same-second bursts are handled separately).
    if len >= REGULAR_MIN_SAMPLES {
        let first = history.get(1).unwrap().at - history.get(0).unwrap().at;
        let mut same = 0u32;
        let mut prev_at = history.get(0).unwrap().at;
        for i in 1..len {
            let at = history.get(i).unwrap().at;
            if first > 0 && at - prev_at == first {
                same += 1;
            }
            prev_at = at;
        }
        if same * 100 >= (len - 1) * REGULAR_RATIO_PCT {
            signals |= SIG_REGULAR_TIMING;
        }
    }

    // Same action over and over, ending with the latest one.
    if len >= REPEAT_RUN {
        let last = history.get(len - 1).unwrap().action;
        let mut run = 0u32;
        for i in (0..len).rev() {
            if history.get(i).unwrap().action == last {
                run += 1;
            } else {
                break;
            }
        }
        if run >= REPEAT_RUN {
            signals |= SIG_REPETITIVE;
        }
    }

    signals
}

/// Persist hourly decay for `player` and return the new score.
pub fn decay_suspicion(env: &Env, player: &Address) -> u32 {
    let mut profile = get_bot_profile(env, player);
    apply_decay(env, &mut profile);
    store_profile(env, player, &profile);
    profile.score
}

// ─── Multi-accounting / farming ───────────────────────────────────────────

/// Link `player` to a device `fingerprint` (from the trusted backend). Admin
/// only. When more than [`MULTI_ACCOUNT_LIMIT`] accounts share the device all
/// of them get `SIG_MULTI_ACCOUNT`; from [`FARM_LIMIT`] they also get
/// `SIG_FARMING`. Returns the number of accounts on the device.
pub fn link_fingerprint(
    env: &Env,
    admin: Address,
    player: Address,
    fingerprint: u64,
) -> Result<u32, BotError> {
    require_admin(env, &admin)?;

    let key = BotKey::Fingerprint(fingerprint);
    let mut accounts: Vec<Address> = env
        .storage()
        .persistent()
        .get(&key)
        .unwrap_or_else(|| Vec::new(env));
    if accounts.contains(&player) {
        return Ok(accounts.len());
    }
    if accounts.len() < MAX_LINKED_ACCOUNTS {
        accounts.push_back(player.clone());
        env.storage().persistent().set(&key, &accounts);
    }
    let size = accounts.len();

    let mut signals = 0u32;
    if size > MULTI_ACCOUNT_LIMIT {
        signals |= SIG_MULTI_ACCOUNT;
    }
    if size >= FARM_LIMIT {
        signals |= SIG_FARMING;
    }
    if signals != 0 {
        for account in accounts.iter() {
            let mut profile = get_bot_profile(env, &account);
            apply_decay(env, &mut profile);
            // Accounts already carrying these signals were penalised when
            // they were raised; only new signals add score.
            let fresh = signals & !profile.signals;
            if fresh != 0 {
                raise(env, &account, &mut profile, fresh);
                store_profile(env, &account, &profile);
            }
        }
        env.events().publish(
            (symbol_short!("bot"), symbol_short!("cluster")),
            (fingerprint, size, signals),
        );
    }
    Ok(size)
}

/// Accounts linked to a device fingerprint.
pub fn get_fingerprint_accounts(env: &Env, fingerprint: u64) -> Vec<Address> {
    env.storage()
        .persistent()
        .get(&BotKey::Fingerprint(fingerprint))
        .unwrap_or_else(|| Vec::new(env))
}

// ─── CAPTCHA Integration ──────────────────────────────────────────────────

/// Issue a CAPTCHA challenge to a player. Admin (backend) only.
pub fn issue_captcha(
    env: &Env,
    admin: Address,
    player: &Address,
    answer_hash: BytesN<32>,
    ttl_seconds: u64,
) -> Result<u64, BotError> {
    require_admin(env, &admin)?;

    let challenge_id: u64 = env
        .storage()
        .instance()
        .get(&BotKey::BotChallengeCounter)
        .unwrap_or(0)
        + 1;
    env.storage()
        .instance()
        .set(&BotKey::BotChallengeCounter, &challenge_id);

    let now = env.ledger().timestamp();
    let challenge = CaptchaChallengeData {
        challenge_id,
        player: player.clone(),
        answer_hash,
        issued_at: now,
        expires_at: now + ttl_seconds,
        solved: false,
    };

    env.storage().persistent().set(
        &BotKey::CaptchaChallenge(player.clone(), challenge_id),
        &challenge,
    );

    env.events().publish(
        (symbol_short!("bot"), symbol_short!("captcha")),
        (player.clone(), challenge_id),
    );

    Ok(challenge_id)
}

/// Answer a CAPTCHA challenge. Returns `Ok(true)` when solved (the gate is
/// lifted and the score drops by [`CAPTCHA_SOLVE_RELIEF`]) and `Ok(false)` for a
/// wrong answer, which is recorded as a `SIG_CAPTCHA_FAIL` signal; after
/// [`MAX_CAPTCHA_FAILURES`] wrong answers the player is blocked.
pub fn solve_captcha(
    env: &Env,
    player: Address,
    challenge_id: u64,
    answer_hash: BytesN<32>,
) -> Result<bool, BotError> {
    player.require_auth();

    let key = BotKey::CaptchaChallenge(player.clone(), challenge_id);
    let mut challenge: CaptchaChallengeData = env
        .storage()
        .persistent()
        .get(&key)
        .ok_or(BotError::ChallengeNotFound)?;
    if challenge.solved {
        return Err(BotError::ChallengeSolved);
    }
    if env.ledger().timestamp() > challenge.expires_at {
        return Err(BotError::ChallengeExpired);
    }

    let mut profile = get_bot_profile(env, &player);
    apply_decay(env, &mut profile);

    if challenge.answer_hash != answer_hash {
        profile.captcha_failures = profile.captcha_failures.saturating_add(1);
        raise(env, &player, &mut profile, SIG_CAPTCHA_FAIL);
        if profile.captcha_failures >= MAX_CAPTCHA_FAILURES {
            profile.blocked_until = env.ledger().timestamp() + BLOCK_SECS;
        }
        store_profile(env, &player, &profile);
        return Ok(false);
    }

    challenge.solved = true;
    env.storage().persistent().set(&key, &challenge);

    profile.captcha_required = false;
    profile.captcha_failures = 0;
    profile.score = profile.score.saturating_sub(CAPTCHA_SOLVE_RELIEF);
    store_profile(env, &player, &profile);

    env.events().publish(
        (symbol_short!("bot"), symbol_short!("solved")),
        (player, challenge_id),
    );

    Ok(true)
}

// ─── Review queue ─────────────────────────────────────────────────────────

fn review_queue(env: &Env) -> Vec<u64> {
    env.storage()
        .persistent()
        .get(&BotKey::ReviewQueue)
        .unwrap_or_else(|| Vec::new(env))
}

fn open_or_update_case(env: &Env, player: &Address, score: u32, signals: u32) -> u64 {
    let now = env.ledger().timestamp();
    let persistent = env.storage().persistent();
    if let Some(id) = persistent.get::<_, u64>(&BotKey::OpenCase(player.clone())) {
        if let Some(mut case) = persistent.get::<_, ReviewCase>(&BotKey::Case(id)) {
            case.peak_score = case.peak_score.max(score);
            case.signals |= signals;
            case.updated_at = now;
            persistent.set(&BotKey::Case(id), &case);
        }
        return id;
    }

    let id: u64 = env
        .storage()
        .instance()
        .get(&BotKey::CaseCounter)
        .unwrap_or(0u64)
        + 1;
    env.storage().instance().set(&BotKey::CaseCounter, &id);

    let case = ReviewCase {
        id,
        player: player.clone(),
        peak_score: score,
        signals,
        opened_at: now,
        updated_at: now,
        status: CaseStatus::Open,
        resolved_at: 0,
        resolved_by: None,
    };
    persistent.set(&BotKey::Case(id), &case);
    persistent.set(&BotKey::OpenCase(player.clone()), &id);
    let mut queue = review_queue(env);
    queue.push_back(id);
    persistent.set(&BotKey::ReviewQueue, &queue);

    env.events().publish(
        (symbol_short!("bot"), symbol_short!("review")),
        (player.clone(), id, score),
    );
    id
}

/// Manually put a player in the review queue. Admin only. Returns the case ID.
pub fn flag_for_review(env: &Env, admin: Address, player: Address) -> Result<u64, BotError> {
    require_admin(env, &admin)?;
    let mut profile = get_bot_profile(env, &player);
    profile.signals |= SIG_MANUAL;
    store_profile(env, &player, &profile);
    Ok(open_or_update_case(env, &player, profile.score, SIG_MANUAL))
}

/// Resolve a case. `Clear` resets the player's score and restrictions; `Ban`
/// bans them. Admin only.
pub fn resolve_case(
    env: &Env,
    admin: Address,
    case_id: u64,
    verdict: CaseVerdict,
) -> Result<ReviewCase, BotError> {
    require_admin(env, &admin)?;
    let persistent = env.storage().persistent();
    let mut case: ReviewCase = persistent
        .get(&BotKey::Case(case_id))
        .ok_or(BotError::CaseNotFound)?;
    if case.status != CaseStatus::Open {
        return Err(BotError::CaseClosed);
    }

    let now = env.ledger().timestamp();
    let mut profile = get_bot_profile(env, &case.player);
    match verdict {
        CaseVerdict::Clear => {
            profile.score = 0;
            profile.captcha_required = false;
            profile.captcha_failures = 0;
            profile.blocked_until = 0;
            profile.last_scored_at = now;
            case.status = CaseStatus::Cleared;
        }
        CaseVerdict::Ban => {
            profile.banned = true;
            case.status = CaseStatus::Banned;
        }
    }
    store_profile(env, &case.player, &profile);

    case.resolved_at = now;
    case.resolved_by = Some(admin.clone());
    case.updated_at = now;
    persistent.set(&BotKey::Case(case_id), &case);
    persistent.remove(&BotKey::OpenCase(case.player.clone()));

    let mut queue = review_queue(env);
    if let Some(pos) = queue.first_index_of(case_id) {
        queue.remove(pos);
        persistent.set(&BotKey::ReviewQueue, &queue);
    }

    env.events().publish(
        (symbol_short!("bot"), symbol_short!("resolved")),
        (case.player.clone(), case_id, verdict),
    );
    Ok(case)
}

/// Lift a ban. Admin only.
pub fn admin_unban(env: &Env, admin: Address, player: Address) -> Result<(), BotError> {
    require_admin(env, &admin)?;
    let mut profile = get_bot_profile(env, &player);
    profile.banned = false;
    profile.score = 0;
    profile.captcha_required = false;
    profile.blocked_until = 0;
    profile.last_scored_at = env.ledger().timestamp();
    store_profile(env, &player, &profile);
    Ok(())
}

/// A case by ID.
pub fn get_case(env: &Env, case_id: u64) -> Option<ReviewCase> {
    env.storage().persistent().get(&BotKey::Case(case_id))
}

/// Number of open cases.
pub fn review_queue_len(env: &Env) -> u32 {
    review_queue(env).len()
}

/// Page through open cases, oldest first.
pub fn get_review_queue(env: &Env, start: u32, limit: u32) -> Vec<ReviewCase> {
    let queue = review_queue(env);
    let end = start
        .saturating_add(limit.min(MAX_QUEUE_PAGE))
        .min(queue.len());
    let mut out = Vec::new(env);
    for i in start..end {
        if let Some(case) = get_case(env, queue.get(i).unwrap()) {
            out.push_back(case);
        }
    }
    out
}

// ─── Query Functions ──────────────────────────────────────────────────────

/// Get a player's current suspicion score.
pub fn get_suspicion_score(env: &Env, player: &Address) -> u32 {
    get_bot_profile(env, player).score
}

/// Check if a player is currently CAPTCHA-gated.
pub fn is_captcha_required(env: &Env, player: &Address) -> bool {
    get_bot_profile(env, player).captcha_required
}

/// Get a player's trust level.
pub fn get_trust_level(env: &Env, player: &Address) -> u32 {
    get_bot_profile(env, player).trust
}

/// Admin override: set a player's trust level (0–2). Raising a player to
/// trusted also clears their score and CAPTCHA gate.
pub fn admin_set_trust(
    env: &Env,
    admin: Address,
    player: Address,
    level: u32,
) -> Result<(), BotError> {
    require_admin(env, &admin)?;
    if level > TRUST_TRUSTED {
        return Err(BotError::InvalidTrustLevel);
    }

    let mut profile = get_bot_profile(env, &player);
    profile.trust = level;
    if level == TRUST_TRUSTED {
        profile.score = 0;
        profile.captcha_required = false;
    }
    store_profile(env, &player, &profile);
    Ok(())
}

// ─── Tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use soroban_sdk::testutils::{Address as _, Ledger};
    use soroban_sdk::{contract, contractimpl, Bytes};

    #[contract]
    struct Stub;
    #[contractimpl]
    impl Stub {}

    struct H {
        env: Env,
        id: Address,
        admin: Address,
        player: Address,
    }

    impl H {
        fn new() -> Self {
            let env = Env::default();
            env.mock_all_auths();
            env.ledger().with_mut(|l| l.timestamp = 1_700_000_000);
            let id = env.register(Stub, ());
            let admin = Address::generate(&env);
            let player = Address::generate(&env);
            let h = H {
                env,
                id,
                admin,
                player,
            };
            h.run(|| init_bot_detection(&h.env, h.admin.clone()).unwrap());
            h
        }
        fn run<T>(&self, f: impl FnOnce() -> T) -> T {
            self.env.as_contract(&self.id, f)
        }
        fn advance(&self, secs: u64) {
            self.env.ledger().with_mut(|l| l.timestamp += secs);
        }
        fn act(&self, action: &str) -> Result<ActionAssessment, BotError> {
            self.run(|| record_action(&self.env, &self.player, &Symbol::new(&self.env, action)))
        }
        fn profile(&self) -> BotProfile {
            self.run(|| get_bot_profile(&self.env, &self.player))
        }
    }

    /// A human-ish sequence: varied gaps and actions.
    const HUMAN_GAPS: [u64; 12] = [7, 13, 5, 22, 9, 31, 6, 18, 11, 8, 27, 14];
    const HUMAN_ACTIONS: [&str; 4] = ["scan", "trade", "mine", "chat"];

    #[test]
    fn human_play_is_allowed_with_zero_score() {
        let h = H::new();
        for (i, gap) in HUMAN_GAPS.iter().enumerate() {
            h.advance(*gap);
            let a = h.act(HUMAN_ACTIONS[i % 4]).unwrap();
            assert!(a.allowed);
            assert_eq!(a.signals, 0);
            assert_eq!(a.response, BotResponse::Allow);
        }
        assert_eq!(h.profile().score, 0);
    }

    #[test]
    fn same_second_burst_is_rejected_and_scored() {
        let h = H::new();
        for _ in 0..MAX_ACTIONS_PER_SECOND {
            assert!(h.act("scan").unwrap().allowed);
        }
        let a = h.act("scan").unwrap();
        assert!(!a.allowed);
        assert_eq!(a.signals, SIG_BURST);
        assert_eq!(a.score, signal_weight(SIG_BURST));
    }

    #[test]
    fn rate_limit_violation_is_detected() {
        let h = H::new();
        for i in 0..MAX_ACTIONS_PER_WINDOW {
            h.advance(if i % 2 == 0 { 2 } else { 3 });
            assert!(h.act(HUMAN_ACTIONS[(i % 4) as usize]).unwrap().allowed);
        }
        h.advance(1);
        let a = h.act("scan").unwrap();
        assert!(!a.allowed);
        assert_eq!(a.signals & SIG_RATE_LIMIT, SIG_RATE_LIMIT);
        // After the window passes, actions are allowed again.
        h.advance(WINDOW_SECS);
        assert!(h.act("scan").unwrap().allowed);
    }

    #[test]
    fn metronome_timing_is_flagged() {
        let h = H::new();
        let mut flagged = false;
        for i in 0..REGULAR_MIN_SAMPLES {
            h.advance(5);
            let a = h.act(HUMAN_ACTIONS[(i % 4) as usize]).unwrap();
            flagged |= a.signals & SIG_REGULAR_TIMING != 0;
        }
        assert!(flagged);
        assert!(h.profile().signals & SIG_REGULAR_TIMING != 0);
    }

    #[test]
    fn repetitive_loop_is_flagged() {
        let h = H::new();
        let mut flagged = false;
        for i in 0..REPEAT_RUN {
            // Irregular gaps so only the repetition signal fires.
            h.advance(HUMAN_GAPS[(i % 12) as usize]);
            flagged |= h.act("mine").unwrap().signals & SIG_REPETITIVE != 0;
        }
        assert!(flagged);
    }

    #[test]
    fn analyze_patterns_ignores_short_histories() {
        let env = Env::default();
        let mut history = Vec::new(&env);
        for i in 0..5u64 {
            history.push_back(ActionRecord {
                at: i * 5,
                action: symbol_short!("mine"),
            });
        }
        assert_eq!(analyze_patterns(&history), 0);
    }

    #[test]
    fn scoring_respects_trust_levels() {
        let all = SIG_BURST | SIG_REGULAR_TIMING;
        assert_eq!(weighted_score(all, TRUST_NORMAL), 35);
        assert_eq!(weighted_score(all, TRUST_TRUSTED), 17);
        assert_eq!(weighted_score(all, TRUST_UNTRUSTED), 52);
        assert_eq!(weighted_score(0, TRUST_NORMAL), 0);
    }

    #[test]
    fn responses_follow_score_bands() {
        assert_eq!(response_for_score(0), BotResponse::Allow);
        assert_eq!(response_for_score(MONITOR_THRESHOLD), BotResponse::Monitor);
        assert_eq!(response_for_score(CAPTCHA_THRESHOLD), BotResponse::Captcha);
        assert_eq!(response_for_score(BLOCK_THRESHOLD), BotResponse::Block);
        assert_eq!(response_for_score(MAX_SUSPICION), BotResponse::Block);
    }

    /// Drive the player to the CAPTCHA band with bursts.
    fn burst_until_gated(h: &H) {
        for _ in 0..10 {
            h.advance(1);
            for _ in 0..=MAX_ACTIONS_PER_SECOND {
                match h.act("scan") {
                    Ok(_) => {}
                    Err(_) => return,
                }
            }
        }
        panic!("player never gated");
    }

    #[test]
    fn crossing_threshold_gates_and_opens_one_review_case() {
        let h = H::new();
        burst_until_gated(&h);
        let p = h.profile();
        assert!(p.score >= CAPTCHA_THRESHOLD);
        assert!(p.captcha_required);
        assert!(matches!(
            h.act("scan"),
            Err(BotError::CaptchaRequired) | Err(BotError::PlayerBlocked)
        ));

        h.run(|| {
            assert_eq!(review_queue_len(&h.env), 1);
            let case = get_review_queue(&h.env, 0, 10).get(0).unwrap();
            assert_eq!(case.player, h.player);
            assert_eq!(case.status, CaseStatus::Open);
            assert!(case.signals & SIG_BURST != 0);
        });
    }

    #[test]
    fn very_high_score_blocks_temporarily() {
        let h = H::new();
        h.run(|| {
            admin_set_trust(&h.env, h.admin.clone(), h.player.clone(), TRUST_UNTRUSTED).unwrap()
        });
        // Untrusted bursts score 30 each: 30, 60 (captcha) …
        burst_until_gated(&h);
        // Push further: farming cluster on the player's device.
        for _ in 0..FARM_LIMIT {
            let other = Address::generate(&h.env);
            h.run(|| link_fingerprint(&h.env, h.admin.clone(), other, 7).unwrap());
        }
        h.run(|| link_fingerprint(&h.env, h.admin.clone(), h.player.clone(), 7).unwrap());
        let p = h.profile();
        assert!(p.score >= BLOCK_THRESHOLD, "score {}", p.score);
        assert!(p.blocked_until > h.env.ledger().timestamp());
        assert_eq!(h.act("scan"), Err(BotError::PlayerBlocked));
    }

    #[test]
    fn score_decays_hourly_and_lifts_gate() {
        let h = H::new();
        burst_until_gated(&h);
        let before = h.profile().score;
        h.advance(BLOCK_SECS + 6 * 3_600);
        let after = h.run(|| decay_suspicion(&h.env, &h.player));
        assert!(after < before);
        assert!(after < CAPTCHA_THRESHOLD);
        assert!(!h.profile().captcha_required);
        assert!(h.act("scan").unwrap().allowed);
    }

    #[test]
    fn multi_accounting_and_farming_are_detected() {
        let h = H::new();
        let mut accounts = std::vec::Vec::new();
        for _ in 0..FARM_LIMIT {
            accounts.push(Address::generate(&h.env));
        }
        for (i, a) in accounts.iter().enumerate() {
            let size = h.run(|| link_fingerprint(&h.env, h.admin.clone(), a.clone(), 99).unwrap());
            assert_eq!(size as usize, i + 1);
        }
        h.run(|| {
            for a in accounts.iter() {
                let p = get_bot_profile(&h.env, a);
                assert!(p.signals & SIG_MULTI_ACCOUNT != 0);
                assert!(p.signals & SIG_FARMING != 0);
                // Each signal counted once despite repeated cluster growth.
                assert_eq!(
                    p.score,
                    signal_weight(SIG_MULTI_ACCOUNT) + signal_weight(SIG_FARMING)
                );
            }
            assert_eq!(get_fingerprint_accounts(&h.env, 99).len(), FARM_LIMIT);
        });
        // Re-linking is idempotent.
        let size =
            h.run(|| link_fingerprint(&h.env, h.admin.clone(), accounts[0].clone(), 99).unwrap());
        assert_eq!(size, FARM_LIMIT);
    }

    #[test]
    fn small_households_are_not_flagged() {
        let h = H::new();
        for _ in 0..MULTI_ACCOUNT_LIMIT {
            let a = Address::generate(&h.env);
            h.run(|| link_fingerprint(&h.env, h.admin.clone(), a.clone(), 5).unwrap());
            h.run(|| assert_eq!(get_suspicion_score(&h.env, &a), 0));
        }
    }

    fn hash(env: &Env, answer: u8) -> BytesN<32> {
        env.crypto()
            .sha256(&Bytes::from_slice(env, &[answer]))
            .into()
    }

    #[test]
    fn captcha_solve_clears_gate() {
        let h = H::new();
        burst_until_gated(&h);
        h.advance(BLOCK_SECS + 1);
        let score = h.profile().score;
        let id = h.run(|| {
            issue_captcha(&h.env, h.admin.clone(), &h.player, hash(&h.env, 42), 300).unwrap()
        });
        assert!(h.run(|| solve_captcha(&h.env, h.player.clone(), id, hash(&h.env, 42)).unwrap()));
        let p = h.profile();
        assert!(!p.captcha_required);
        assert!(p.score <= score.saturating_sub(CAPTCHA_SOLVE_RELIEF));
        assert_eq!(
            h.run(|| solve_captcha(&h.env, h.player.clone(), id, hash(&h.env, 42))),
            Err(BotError::ChallengeSolved)
        );
    }

    #[test]
    fn wrong_captcha_answers_are_persisted_and_block() {
        let h = H::new();
        let id = h.run(|| {
            issue_captcha(&h.env, h.admin.clone(), &h.player, hash(&h.env, 1), 300).unwrap()
        });
        for _ in 0..MAX_CAPTCHA_FAILURES {
            assert!(
                !h.run(|| solve_captcha(&h.env, h.player.clone(), id, hash(&h.env, 2)).unwrap())
            );
        }
        let p = h.profile();
        assert_eq!(p.captcha_failures, MAX_CAPTCHA_FAILURES);
        assert!(p.signals & SIG_CAPTCHA_FAIL != 0);
        assert!(p.blocked_until > 0);
    }

    #[test]
    fn expired_captcha_is_rejected() {
        let h = H::new();
        let id = h.run(|| {
            issue_captcha(&h.env, h.admin.clone(), &h.player, hash(&h.env, 1), 10).unwrap()
        });
        h.advance(11);
        assert_eq!(
            h.run(|| solve_captcha(&h.env, h.player.clone(), id, hash(&h.env, 1))),
            Err(BotError::ChallengeExpired)
        );
    }

    #[test]
    fn review_clear_resets_player() {
        let h = H::new();
        burst_until_gated(&h);
        let case_id = h.run(|| get_review_queue(&h.env, 0, 1).get(0).unwrap().id);
        let case =
            h.run(|| resolve_case(&h.env, h.admin.clone(), case_id, CaseVerdict::Clear).unwrap());
        assert_eq!(case.status, CaseStatus::Cleared);
        assert_eq!(case.resolved_by, Some(h.admin.clone()));
        let p = h.profile();
        assert_eq!(p.score, 0);
        assert!(!p.captcha_required);
        assert_eq!(p.blocked_until, 0);
        h.run(|| assert_eq!(review_queue_len(&h.env), 0));
        h.advance(1);
        assert!(h.act("scan").unwrap().allowed);
        assert_eq!(
            h.run(|| resolve_case(&h.env, h.admin.clone(), case_id, CaseVerdict::Ban)),
            Err(BotError::CaseClosed)
        );
    }

    #[test]
    fn review_ban_blocks_until_unbanned() {
        let h = H::new();
        let case_id = h.run(|| flag_for_review(&h.env, h.admin.clone(), h.player.clone()).unwrap());
        h.run(|| resolve_case(&h.env, h.admin.clone(), case_id, CaseVerdict::Ban).unwrap());
        assert_eq!(h.act("scan"), Err(BotError::PlayerBanned));
        h.run(|| admin_unban(&h.env, h.admin.clone(), h.player.clone()).unwrap());
        assert!(h.act("scan").unwrap().allowed);
    }

    #[test]
    fn manual_flag_reuses_open_case() {
        let h = H::new();
        let a = h.run(|| flag_for_review(&h.env, h.admin.clone(), h.player.clone()).unwrap());
        let b = h.run(|| flag_for_review(&h.env, h.admin.clone(), h.player.clone()).unwrap());
        assert_eq!(a, b);
        h.run(|| assert_eq!(review_queue_len(&h.env), 1));
    }

    #[test]
    fn admin_functions_reject_non_admins() {
        let h = H::new();
        let mallory = Address::generate(&h.env);
        h.run(|| {
            assert_eq!(
                admin_set_trust(&h.env, mallory.clone(), h.player.clone(), TRUST_TRUSTED),
                Err(BotError::Unauthorized)
            )
        });
        h.run(|| {
            assert_eq!(
                link_fingerprint(&h.env, mallory.clone(), h.player.clone(), 1),
                Err(BotError::Unauthorized)
            )
        });
        h.run(|| {
            assert_eq!(
                issue_captcha(&h.env, mallory.clone(), &h.player, hash(&h.env, 1), 10),
                Err(BotError::Unauthorized)
            )
        });
        h.run(|| {
            assert_eq!(
                flag_for_review(&h.env, mallory.clone(), h.player.clone()),
                Err(BotError::Unauthorized)
            )
        });
        h.run(|| {
            assert_eq!(
                init_bot_detection(&h.env, mallory.clone()),
                Err(BotError::AlreadyInitialized)
            )
        });
        h.run(|| {
            assert_eq!(
                admin_set_trust(&h.env, h.admin.clone(), h.player.clone(), 3),
                Err(BotError::InvalidTrustLevel)
            )
        });
    }

    #[test]
    fn trust_level_default_and_trusted_halves_penalties() {
        let h = H::new();
        assert_eq!(h.run(|| get_trust_level(&h.env, &h.player)), TRUST_NORMAL);
        h.run(|| {
            admin_set_trust(&h.env, h.admin.clone(), h.player.clone(), TRUST_TRUSTED).unwrap()
        });
        for _ in 0..MAX_ACTIONS_PER_SECOND {
            h.act("scan").unwrap();
        }
        assert_eq!(h.act("scan").unwrap().score, signal_weight(SIG_BURST) / 2);
    }
}
