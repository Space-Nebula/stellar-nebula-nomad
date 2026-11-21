use soroban_sdk::{
    contracterror, contracttype, symbol_short, xdr::ToXdr, Address, BytesN, Env, Symbol, Vec,
};

use crate::rate_limiter;

/// Maximum number of commitments allowed per transaction.
pub const MAX_COMMITMENTS_PER_TX: u32 = 10;

// ─── Storage Keys ─────────────────────────────────────────────────────────────

#[derive(Clone)]
#[contracttype]
pub enum PrivacyKey {
    /// Commitment hash storage keyed by player and stat type.
    Commitment(Address, Symbol),
    /// Opt-in status for a player.
    OptIn(Address),
    /// Global commitment counter.
    CommitmentCount,
    /// Burst counter for rate limiting.
    PrivacyBurstCounter,
    /// Address allowed to configure anonymous aggregates.
    PrivacyAdmin,
    /// Aggregate configuration per stat type.
    PrivacyStatConfig(Symbol),
    /// Stat types that have an aggregate configuration.
    PrivacyConfiguredStats,
    /// Anonymous running total per (stat type, epoch).
    PrivacyAggregate(Symbol, u64),
    /// One-contribution-per-epoch marker, keyed by a hash instead of the address.
    PrivacyNullifier(BytesN<32>),
    /// Published differentially private report per (stat type, epoch).
    PrivacyReport(Symbol, u64),
    /// Number of distinct players that opted in.
    PrivacyOptInCount,
    /// Number of anonymous contributions accepted.
    PrivacyContributionCount,
    /// Number of reports published.
    PrivacyReportCount,
}

// ─── Data Types ───────────────────────────────────────────────────────────────

/// Privacy-preserving stat commitment record.
#[derive(Clone)]
#[contracttype]
pub struct StatCommitment {
    pub player: Address,
    pub stat_type: Symbol,
    pub commitment_hash: BytesN<32>,
    pub timestamp: u64,
    pub verified: bool,
}

// ─── Errors ───────────────────────────────────────────────────────────────────

#[contracterror]
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum PrivacyError {
    /// Player has not opted in to privacy features.
    NotOptedIn = 1,
    /// Invalid proof provided for verification.
    InvalidProof = 2,
    /// Commitment not found.
    CommitmentNotFound = 3,
    /// Burst limit exceeded (max 10 commitments per tx).
    BurstLimitExceeded = 4,
    /// Commitment already exists for this stat type.
    CommitmentExists = 5,
    /// Caller is not the privacy admin.
    NotAuthorized = 6,
    /// Privacy admin is already set.
    AlreadyInitialized = 7,
    /// No aggregate configuration exists for this stat type.
    StatNotConfigured = 8,
    /// Player already contributed to this stat in the current epoch.
    AlreadyContributed = 9,
    /// Reports can only be published for epochs that have ended.
    EpochNotClosed = 10,
    /// A report was already published for this stat and epoch.
    ReportAlreadyPublished = 11,
    /// No report has been published for this stat and epoch.
    ReportNotFound = 12,
    /// Aggregate configuration is out of bounds.
    InvalidConfig = 13,
}

impl crate::error_standard::StandardContractError for PrivacyError {
    fn descriptor(self) -> crate::error_standard::ErrorDescriptor {
        use crate::error_standard::ErrorKind;
        let (kind, retryable) = match self {
            Self::NotOptedIn => (ErrorKind::Authorization, false),
            Self::InvalidProof => (ErrorKind::Validation, false),
            Self::CommitmentNotFound => (ErrorKind::NotFound, false),
            Self::BurstLimitExceeded => (ErrorKind::ResourceLimit, true),
            Self::CommitmentExists => (ErrorKind::Conflict, false),
            Self::NotAuthorized => (ErrorKind::Authorization, false),
            Self::AlreadyInitialized => (ErrorKind::Conflict, false),
            Self::StatNotConfigured => (ErrorKind::NotFound, false),
            Self::AlreadyContributed => (ErrorKind::Conflict, true),
            Self::EpochNotClosed => (ErrorKind::Validation, true),
            Self::ReportAlreadyPublished => (ErrorKind::Conflict, false),
            Self::ReportNotFound => (ErrorKind::NotFound, false),
            Self::InvalidConfig => (ErrorKind::Validation, false),
        };
        crate::error_standard::ErrorDescriptor {
            module: "privacy_stats",
            code: self as u32,
            kind,
            retryable,
        }
    }
}

// ─── Helper Functions ─────────────────────────────────────────────────────────

/// Compute a simple commitment hash: hash(stat_type || value || player || timestamp).
/// In production, this would use a proper cryptographic commitment scheme.
fn compute_commitment_hash(
    env: &Env,
    stat_type: &Symbol,
    value: i128,
    _player: &Address,
    timestamp: u64,
) -> BytesN<32> {
    let mut data = soroban_sdk::Bytes::new(env);

    // Append stat_type as bytes (use a simple representation)
    // Convert symbol to a deterministic byte representation
    let stat_val = stat_type.to_val();
    let stat_u64 = stat_val.get_payload();
    let stat_bytes = stat_u64.to_be_bytes();
    for byte in stat_bytes.iter() {
        data.push_back(*byte);
    }

    // Append value bytes
    let value_bytes = value.to_be_bytes();
    for byte in value_bytes.iter() {
        data.push_back(*byte);
    }

    // Append timestamp bytes
    let ts_bytes = timestamp.to_be_bytes();
    for byte in ts_bytes.iter() {
        data.push_back(*byte);
    }

    // Use Soroban's built-in hash function and convert to BytesN
    BytesN::from_array(env, &env.crypto().sha256(&data).to_array())
}

/// Verify a proof against a commitment.
/// This is a simplified verification - in production, use proper ZK proofs.
fn verify_proof_internal(_env: &Env, commitment: &BytesN<32>, proof: &BytesN<64>) -> bool {
    // Simple verification: check if proof's first 32 bytes match commitment
    // In production, this would verify a proper zero-knowledge proof
    let mut matches = true;
    for i in 0..32 {
        if commitment.get(i).unwrap() != proof.get(i).unwrap() {
            matches = false;
            break;
        }
    }
    matches
}

/// Check and increment burst counter.
fn check_burst_limit(env: &Env) -> Result<(), PrivacyError> {
    let current: u32 = env
        .storage()
        .instance()
        .get(&PrivacyKey::PrivacyBurstCounter)
        .unwrap_or(0);

    if current >= MAX_COMMITMENTS_PER_TX {
        return Err(PrivacyError::BurstLimitExceeded);
    }

    env.storage()
        .instance()
        .set(&PrivacyKey::PrivacyBurstCounter, &(current + 1));

    Ok(())
}

/// Reset burst counter (called at start of new transaction).
pub fn reset_burst_counter(env: &Env) {
    env.storage()
        .instance()
        .set(&PrivacyKey::PrivacyBurstCounter, &0u32);
}

// ─── Public API ───────────────────────────────────────────────────────────────

/// Opt in to privacy-preserving stat sharing.
/// This is a one-time setup that enables privacy features for the player.
pub fn opt_in_privacy(env: &Env, player: Address) -> Result<(), PrivacyError> {
    player.require_auth();

    if !is_opted_in(env, &player) {
        increment_instance_counter(env, &PrivacyKey::PrivacyOptInCount);
    }
    env.storage()
        .persistent()
        .set(&PrivacyKey::OptIn(player.clone()), &true);

    env.events()
        .publish((symbol_short!("privacy"), symbol_short!("optin")), player);

    Ok(())
}

/// Check if a player has opted in to privacy features.
pub fn is_opted_in(env: &Env, player: &Address) -> bool {
    env.storage()
        .persistent()
        .get(&PrivacyKey::OptIn(player.clone()))
        .unwrap_or(false)
}

/// Commit a private stat without revealing the raw value.
/// Stores a cryptographic commitment that can later be verified.
///
/// # Arguments
/// * `player` - The player committing the stat
/// * `stat_type` - Type of stat (e.g., "score", "kills", "resources")
/// * `value` - The actual stat value (not stored, only used for commitment)
///
/// # Returns
/// The commitment hash that can be used for later verification.
pub fn commit_private_stat(
    env: &Env,
    player: Address,
    stat_type: Symbol,
    value: i128,
) -> Result<BytesN<32>, PrivacyError> {
    player.require_auth();

    rate_limiter::check_rate_limit(env, &player, rate_limiter::Operation::PrivacyCommit)
        .map_err(|_| PrivacyError::BurstLimitExceeded)?;

    // Check opt-in status
    if !is_opted_in(env, &player) {
        return Err(PrivacyError::NotOptedIn);
    }

    // Check burst limit
    check_burst_limit(env)?;

    // Check if commitment already exists
    let key = PrivacyKey::Commitment(player.clone(), stat_type.clone());
    if env.storage().persistent().has(&key) {
        return Err(PrivacyError::CommitmentExists);
    }

    let timestamp = env.ledger().timestamp();
    let commitment_hash = compute_commitment_hash(env, &stat_type, value, &player, timestamp);

    let commitment = StatCommitment {
        player: player.clone(),
        stat_type: stat_type.clone(),
        commitment_hash: commitment_hash.clone(),
        timestamp,
        verified: false,
    };

    env.storage().persistent().set(&key, &commitment);

    // Increment global counter
    let count: u64 = env
        .storage()
        .instance()
        .get(&PrivacyKey::CommitmentCount)
        .unwrap_or(0);
    env.storage()
        .instance()
        .set(&PrivacyKey::CommitmentCount, &(count + 1));

    // Emit PrivateStatCommitted event
    env.events().publish(
        (symbol_short!("privacy"), symbol_short!("commit")),
        (player, stat_type, commitment_hash.clone()),
    );

    Ok(commitment_hash)
}

/// Verify a private stat commitment using a zero-knowledge proof.
/// This allows validation without revealing the underlying data.
///
/// # Arguments
/// * `commitment` - The commitment hash to verify
/// * `proof` - A 64-byte zero-knowledge proof
///
/// # Returns
/// `true` if the proof is valid, otherwise returns `InvalidProof` error.
pub fn verify_private_stat(
    env: &Env,
    commitment: BytesN<32>,
    proof: BytesN<64>,
) -> Result<bool, PrivacyError> {
    // Pure verification function - no auth required

    if !verify_proof_internal(env, &commitment, &proof) {
        return Err(PrivacyError::InvalidProof);
    }

    // Emit verification event
    env.events().publish(
        (symbol_short!("privacy"), symbol_short!("verify")),
        (commitment, true),
    );

    Ok(true)
}

/// Get a commitment for a player and stat type.
pub fn get_commitment(
    env: &Env,
    player: Address,
    stat_type: Symbol,
) -> Result<StatCommitment, PrivacyError> {
    let key = PrivacyKey::Commitment(player, stat_type);
    env.storage()
        .persistent()
        .get(&key)
        .ok_or(PrivacyError::CommitmentNotFound)
}

/// Get total number of commitments made across all players.
pub fn get_commitment_count(env: &Env) -> u64 {
    env.storage()
        .instance()
        .get(&PrivacyKey::CommitmentCount)
        .unwrap_or(0)
}

/// Batch commit multiple stats in a single transaction (up to 10).
/// This is more gas-efficient for committing multiple stats at once.
pub fn batch_commit_stats(
    env: &Env,
    player: Address,
    stat_types: Vec<Symbol>,
    values: Vec<i128>,
) -> Result<Vec<BytesN<32>>, PrivacyError> {
    player.require_auth();

    if !is_opted_in(env, &player) {
        return Err(PrivacyError::NotOptedIn);
    }

    let count = stat_types.len();
    if count > MAX_COMMITMENTS_PER_TX {
        return Err(PrivacyError::BurstLimitExceeded);
    }

    if count != values.len() {
        return Err(PrivacyError::InvalidProof); // Reuse error for mismatched lengths
    }

    let mut commitments = Vec::new(env);

    for i in 0..count {
        let stat_type = stat_types.get(i).unwrap();
        let value = values.get(i).unwrap();

        // Check if commitment already exists
        let key = PrivacyKey::Commitment(player.clone(), stat_type.clone());
        if env.storage().persistent().has(&key) {
            return Err(PrivacyError::CommitmentExists);
        }

        let timestamp = env.ledger().timestamp();
        let commitment_hash = compute_commitment_hash(env, &stat_type, value, &player, timestamp);

        let commitment = StatCommitment {
            player: player.clone(),
            stat_type: stat_type.clone(),
            commitment_hash: commitment_hash.clone(),
            timestamp,
            verified: false,
        };

        env.storage().persistent().set(&key, &commitment);
        commitments.push_back(commitment_hash.clone());

        // Emit event for each commitment
        env.events().publish(
            (symbol_short!("privacy"), symbol_short!("commit")),
            (player.clone(), stat_type, commitment_hash),
        );
    }

    // Update global counter
    let current_count: u64 = env
        .storage()
        .instance()
        .get(&PrivacyKey::CommitmentCount)
        .unwrap_or(0);
    env.storage().instance().set(
        &PrivacyKey::CommitmentCount,
        &(current_count + count as u64),
    );

    Ok(commitments)
}

// ─── Balance Hiding (#277) ────────────────────────────────────────────────

/// Storage keys for balance hiding and selective disclosure.
#[derive(Clone)]
#[contracttype]
pub enum BalancePrivacyKey {
    /// Hidden balance commitment for a player: (player, token) → commitment.
    HiddenBalance(Address, Symbol),
    /// Selective disclosure grant: (owner, viewer, stat_type) → bool.
    DisclosureGrant(Address, Address, Symbol),
    /// Privacy level for a player (0=none, 1=balance-only, 2=full).
    PrivacyLevel(Address),
}

/// Commit a hidden balance. The raw balance is never stored on-chain —
/// only the commitment hash. The player can later reveal the balance
/// off-chain and prove it matches the commitment.
pub fn hide_balance(
    env: &Env,
    player: Address,
    token: Symbol,
    balance: i128,
) -> Result<BytesN<32>, PrivacyError> {
    player.require_auth();

    if !is_opted_in(env, &player) {
        return Err(PrivacyError::NotOptedIn);
    }

    let timestamp = env.ledger().timestamp();
    let commitment = compute_commitment_hash(env, &token, balance, &player, timestamp);

    env.storage().persistent().set(
        &BalancePrivacyKey::HiddenBalance(player.clone(), token.clone()),
        &commitment,
    );

    env.events().publish(
        (symbol_short!("privacy"), symbol_short!("bal_hide")),
        (player, token, commitment.clone()),
    );

    Ok(commitment)
}

/// Get the hidden balance commitment for a player.
pub fn get_hidden_balance(env: &Env, player: &Address, token: &Symbol) -> Option<BytesN<32>> {
    env.storage()
        .persistent()
        .get(&BalancePrivacyKey::HiddenBalance(
            player.clone(),
            token.clone(),
        ))
}

// ─── Selective Disclosure (#277) ──────────────────────────────────────────

/// Grant a viewer permission to see a specific stat type.
pub fn grant_disclosure(
    env: &Env,
    owner: Address,
    viewer: Address,
    stat_type: Symbol,
) -> Result<(), PrivacyError> {
    owner.require_auth();

    if !is_opted_in(env, &owner) {
        return Err(PrivacyError::NotOptedIn);
    }

    env.storage().persistent().set(
        &BalancePrivacyKey::DisclosureGrant(owner.clone(), viewer.clone(), stat_type.clone()),
        &true,
    );

    env.events().publish(
        (symbol_short!("privacy"), symbol_short!("disc_grnt")),
        (owner, viewer, stat_type),
    );

    Ok(())
}

/// Revoke a viewer's permission to see a specific stat type.
pub fn revoke_disclosure(
    env: &Env,
    owner: Address,
    viewer: Address,
    stat_type: Symbol,
) -> Result<(), PrivacyError> {
    owner.require_auth();

    env.storage()
        .persistent()
        .remove(&BalancePrivacyKey::DisclosureGrant(
            owner, viewer, stat_type,
        ));

    Ok(())
}

/// Check if a viewer has been granted access to a specific stat type.
pub fn has_disclosure(env: &Env, owner: &Address, viewer: &Address, stat_type: &Symbol) -> bool {
    env.storage()
        .persistent()
        .get::<_, bool>(&BalancePrivacyKey::DisclosureGrant(
            owner.clone(),
            viewer.clone(),
            stat_type.clone(),
        ))
        .unwrap_or(false)
}

/// Set the privacy level for a player.
/// - 0: No privacy (all stats visible)
/// - 1: Balance-only hidden
/// - 2: Full privacy (all stats require selective disclosure)
pub fn set_privacy_level(env: &Env, player: Address, level: u32) -> Result<(), PrivacyError> {
    player.require_auth();

    if level > 2 {
        return Err(PrivacyError::InvalidProof);
    }

    env.storage()
        .persistent()
        .set(&BalancePrivacyKey::PrivacyLevel(player), &level);

    Ok(())
}

/// Get the privacy level for a player.
pub fn get_privacy_level(env: &Env, player: &Address) -> u32 {
    env.storage()
        .persistent()
        .get(&BalancePrivacyKey::PrivacyLevel(player.clone()))
        .unwrap_or(0)
}

/// Verify a commitment without revealing the value. This is a read-only
/// proof verification that any party can perform.
pub fn verify_balance_commitment(
    env: &Env,
    player: &Address,
    token: &Symbol,
    proof: &BytesN<64>,
) -> Result<bool, PrivacyError> {
    let commitment =
        get_hidden_balance(env, player, token).ok_or(PrivacyError::CommitmentNotFound)?;

    Ok(verify_proof_internal(env, &commitment, proof))
}

// ─── Anonymous Aggregates & Differential Privacy (#709) ───────────────────
//
// Players add a bounded value to a per-epoch running total. No storage key or
// event links a value to an address. Once an epoch closes, anyone can publish
// one report for it; the report adds discrete Laplace noise so the presence or
// absence of any single contributor changes its distribution by at most e^ε.

/// Length of one aggregation window in seconds.
pub const PRIVACY_EPOCH_SECS: u64 = 86_400;
/// Reports whose noisy contributor count is below this hide the sum.
pub const MIN_REPORT_COHORT: u64 = 5;
/// Upper bound for `noise_multiplier` (ε per released value is at most 1/multiplier).
pub const MAX_NOISE_MULTIPLIER: u32 = 1_000;
/// Upper bound for `max_contribution`; keeps the fixed-point sampler precise.
pub const MAX_CONTRIBUTION_BOUND: i128 = 1_000_000_000;
/// Maximum number of stat types that can have an aggregate configuration.
pub const MAX_PRIVATE_STATS: u32 = 32;

/// How one stat type is aggregated and protected.
#[derive(Clone, Debug, PartialEq)]
#[contracttype]
pub struct PrivateAggregateConfig {
    /// Contributions are clamped to `[0, max_contribution]` (the sum's sensitivity).
    pub max_contribution: i128,
    /// Noise scale `b`: each released value is (1/b)-differentially private.
    pub noise_multiplier: u32,
}

/// Anonymous running total for one (stat type, epoch).
#[derive(Clone, Debug, PartialEq)]
#[contracttype]
pub struct PrivateAggregate {
    pub contributors: u64,
    pub sum: i128,
    /// Largest clamp bound used this epoch, so noise covers config changes mid-epoch.
    pub max_bound: i128,
}

/// A published, differentially private report.
#[derive(Clone, Debug, PartialEq)]
#[contracttype]
pub struct PrivacyReport {
    pub stat_type: Symbol,
    pub epoch: u64,
    pub noisy_contributors: i128,
    /// Zero when `suppressed`.
    pub noisy_sum: i128,
    /// True when the noisy cohort is smaller than [`MIN_REPORT_COHORT`].
    pub suppressed: bool,
    pub max_contribution: i128,
    pub noise_multiplier: u32,
    /// Upper bound on the total ε spent by this report, in thousandths.
    pub epsilon_milli: u32,
    pub published_at: u64,
}

/// Public summary of what the privacy system holds and how it protects it.
#[derive(Clone, Debug, PartialEq)]
#[contracttype]
pub struct PrivacyTransparencyReport {
    pub opted_in_players: u64,
    pub total_commitments: u64,
    pub total_contributions: u64,
    pub reports_published: u64,
    pub configured_stats: Vec<Symbol>,
    pub epoch_secs: u64,
    pub min_report_cohort: u64,
}

fn increment_instance_counter(env: &Env, key: &PrivacyKey) {
    let count: u64 = env.storage().instance().get(key).unwrap_or(0);
    env.storage().instance().set(key, &count.saturating_add(1));
}

fn instance_counter(env: &Env, key: &PrivacyKey) -> u64 {
    env.storage().instance().get(key).unwrap_or(0)
}

fn require_privacy_admin(env: &Env, admin: &Address) -> Result<(), PrivacyError> {
    admin.require_auth();
    let stored: Address = env
        .storage()
        .instance()
        .get(&PrivacyKey::PrivacyAdmin)
        .ok_or(PrivacyError::NotAuthorized)?;
    if stored != *admin {
        return Err(PrivacyError::NotAuthorized);
    }
    Ok(())
}

/// Hash of (player, stat, epoch): blocks double contributions without
/// storing the address in a key.
fn contribution_nullifier(
    env: &Env,
    player: &Address,
    stat_type: &Symbol,
    epoch: u64,
) -> BytesN<32> {
    let mut data = player.clone().to_xdr(env);
    data.append(&stat_type.clone().to_xdr(env));
    data.append(&epoch.to_xdr(env));
    env.crypto().sha256(&data).into()
}

const FP_ONE: u128 = 1 << 64;

/// `base^exp` in 64.64 fixed point; `base < FP_ONE` keeps every product in u128.
fn fp_pow(mut base: u128, mut exp: u128) -> u128 {
    let mut acc = FP_ONE;
    while exp > 0 {
        if exp & 1 == 1 {
            acc = (acc * base) >> 64;
        }
        base = (base * base) >> 64;
        exp >>= 1;
    }
    acc
}

/// Geometric sample with P(X >= k) = q^k, q = scale / (scale + 1).
/// Inverse-CDF by binary search, so cost is O(log^2(scale)) and float-free.
fn sample_geometric(env: &Env, scale: u128) -> u128 {
    let q = (scale << 64) / (scale + 1);
    let u = u128::from(env.prng().gen::<u64>());
    // P(X > 64 * scale) < e^-64: truncating there is unobservable.
    let (mut lo, mut hi) = (0u128, scale * 64);
    while lo < hi {
        let mid = lo + (hi - lo).div_ceil(2);
        if fp_pow(q, mid) > u {
            lo = mid;
        } else {
            hi = mid - 1;
        }
    }
    lo
}

/// Discrete Laplace noise with P(x) ∝ q^|x|: the difference of two geometrics.
#[allow(clippy::cast_possible_wrap)] // samples are <= 64 * scale < 2^56
fn discrete_laplace(env: &Env, scale: u128) -> i128 {
    sample_geometric(env, scale) as i128 - sample_geometric(env, scale) as i128
}

/// Index of the aggregation window containing the current ledger time.
pub fn current_privacy_epoch(env: &Env) -> u64 {
    env.ledger().timestamp() / PRIVACY_EPOCH_SECS
}

/// Set the address allowed to configure anonymous aggregates (once).
pub fn initialize_privacy_stats(env: &Env, admin: Address) -> Result<(), PrivacyError> {
    admin.require_auth();
    if env.storage().instance().has(&PrivacyKey::PrivacyAdmin) {
        return Err(PrivacyError::AlreadyInitialized);
    }
    env.storage()
        .instance()
        .set(&PrivacyKey::PrivacyAdmin, &admin);
    Ok(())
}

/// Enable (or retune) anonymous aggregation for a stat type. Admin only.
pub fn configure_private_aggregate(
    env: &Env,
    admin: Address,
    stat_type: Symbol,
    max_contribution: i128,
    noise_multiplier: u32,
) -> Result<(), PrivacyError> {
    require_privacy_admin(env, &admin)?;
    if !(1..=MAX_CONTRIBUTION_BOUND).contains(&max_contribution)
        || !(1..=MAX_NOISE_MULTIPLIER).contains(&noise_multiplier)
    {
        return Err(PrivacyError::InvalidConfig);
    }

    let mut stats: Vec<Symbol> = env
        .storage()
        .persistent()
        .get(&PrivacyKey::PrivacyConfiguredStats)
        .unwrap_or_else(|| Vec::new(env));
    if !stats.contains(&stat_type) {
        if stats.len() >= MAX_PRIVATE_STATS {
            return Err(PrivacyError::InvalidConfig);
        }
        stats.push_back(stat_type.clone());
        env.storage()
            .persistent()
            .set(&PrivacyKey::PrivacyConfiguredStats, &stats);
    }

    let config = PrivateAggregateConfig {
        max_contribution,
        noise_multiplier,
    };
    env.storage()
        .persistent()
        .set(&PrivacyKey::PrivacyStatConfig(stat_type.clone()), &config);
    env.events().publish(
        (symbol_short!("privacy"), symbol_short!("config")),
        (stat_type, max_contribution, noise_multiplier),
    );
    Ok(())
}

/// Aggregate configuration for a stat type, if any.
pub fn get_private_aggregate_config(
    env: &Env,
    stat_type: &Symbol,
) -> Option<PrivateAggregateConfig> {
    env.storage()
        .persistent()
        .get(&PrivacyKey::PrivacyStatConfig(stat_type.clone()))
}

/// Add a value to the current epoch's anonymous total. The value is clamped
/// to `[0, max_contribution]`; only the total and contributor count are kept.
/// Returns the epoch the value was counted in.
pub fn contribute_anonymous_stat(
    env: &Env,
    player: Address,
    stat_type: Symbol,
    value: i128,
) -> Result<u64, PrivacyError> {
    player.require_auth();

    rate_limiter::check_rate_limit(env, &player, rate_limiter::Operation::PrivacyCommit)
        .map_err(|_| PrivacyError::BurstLimitExceeded)?;

    if !is_opted_in(env, &player) {
        return Err(PrivacyError::NotOptedIn);
    }
    let config =
        get_private_aggregate_config(env, &stat_type).ok_or(PrivacyError::StatNotConfigured)?;

    let epoch = current_privacy_epoch(env);
    let nullifier_key =
        PrivacyKey::PrivacyNullifier(contribution_nullifier(env, &player, &stat_type, epoch));
    if env.storage().persistent().has(&nullifier_key) {
        return Err(PrivacyError::AlreadyContributed);
    }
    env.storage().persistent().set(&nullifier_key, &true);

    let aggregate_key = PrivacyKey::PrivacyAggregate(stat_type.clone(), epoch);
    let mut aggregate: PrivateAggregate =
        env.storage()
            .persistent()
            .get(&aggregate_key)
            .unwrap_or(PrivateAggregate {
                contributors: 0,
                sum: 0,
                max_bound: 0,
            });
    aggregate.contributors = aggregate.contributors.saturating_add(1);
    aggregate.sum = aggregate
        .sum
        .saturating_add(value.clamp(0, config.max_contribution));
    aggregate.max_bound = aggregate.max_bound.max(config.max_contribution);
    env.storage().persistent().set(&aggregate_key, &aggregate);

    increment_instance_counter(env, &PrivacyKey::PrivacyContributionCount);

    // Deliberately no player or value in the event.
    env.events().publish(
        (symbol_short!("privacy"), symbol_short!("contrib")),
        (stat_type, epoch),
    );
    Ok(epoch)
}

/// Publish the differentially private report for a closed epoch.
/// Permissionless and one-shot: the noisy result is stored so repeated reads
/// cannot average the noise away.
pub fn publish_privacy_report(
    env: &Env,
    stat_type: Symbol,
    epoch: u64,
) -> Result<PrivacyReport, PrivacyError> {
    if epoch >= current_privacy_epoch(env) {
        return Err(PrivacyError::EpochNotClosed);
    }
    let report_key = PrivacyKey::PrivacyReport(stat_type.clone(), epoch);
    if env.storage().persistent().has(&report_key) {
        return Err(PrivacyError::ReportAlreadyPublished);
    }
    let config =
        get_private_aggregate_config(env, &stat_type).ok_or(PrivacyError::StatNotConfigured)?;
    let aggregate: Option<PrivateAggregate> = env
        .storage()
        .persistent()
        .get(&PrivacyKey::PrivacyAggregate(stat_type.clone(), epoch));
    let (contributors, sum, bound) = match aggregate {
        Some(a) => (
            a.contributors,
            a.sum,
            a.max_bound.max(config.max_contribution),
        ),
        None => (0, 0, config.max_contribution),
    };

    let b = u128::from(config.noise_multiplier);
    // Count has sensitivity 1, sum has sensitivity `bound`: scale noise to match.
    let noisy_contributors = (i128::from(contributors) + discrete_laplace(env, b)).max(0);
    let noisy_sum = (sum + discrete_laplace(env, b * bound.unsigned_abs())).max(0);
    let suppressed = noisy_contributors < i128::from(MIN_REPORT_COHORT);

    let report = PrivacyReport {
        stat_type: stat_type.clone(),
        epoch,
        noisy_contributors,
        noisy_sum: if suppressed { 0 } else { noisy_sum },
        suppressed,
        max_contribution: bound,
        noise_multiplier: config.noise_multiplier,
        // Two releases, each ε <= 1/b (ln(1 + 1/s) <= 1/s), rounded up.
        epsilon_milli: 2000_u32.div_ceil(config.noise_multiplier),
        published_at: env.ledger().timestamp(),
    };
    env.storage().persistent().set(&report_key, &report);
    increment_instance_counter(env, &PrivacyKey::PrivacyReportCount);

    env.events().publish(
        (symbol_short!("privacy"), symbol_short!("report")),
        (
            stat_type,
            epoch,
            report.noisy_contributors,
            report.noisy_sum,
            suppressed,
        ),
    );
    Ok(report)
}

/// Read a published report.
pub fn get_privacy_report(
    env: &Env,
    stat_type: Symbol,
    epoch: u64,
) -> Result<PrivacyReport, PrivacyError> {
    env.storage()
        .persistent()
        .get(&PrivacyKey::PrivacyReport(stat_type, epoch))
        .ok_or(PrivacyError::ReportNotFound)
}

/// Public transparency summary: counts only, never per-player data.
pub fn get_privacy_transparency_report(env: &Env) -> PrivacyTransparencyReport {
    PrivacyTransparencyReport {
        opted_in_players: instance_counter(env, &PrivacyKey::PrivacyOptInCount),
        total_commitments: get_commitment_count(env),
        total_contributions: instance_counter(env, &PrivacyKey::PrivacyContributionCount),
        reports_published: instance_counter(env, &PrivacyKey::PrivacyReportCount),
        configured_stats: env
            .storage()
            .persistent()
            .get(&PrivacyKey::PrivacyConfiguredStats)
            .unwrap_or_else(|| Vec::new(env)),
        epoch_secs: PRIVACY_EPOCH_SECS,
        min_report_cohort: MIN_REPORT_COHORT,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use soroban_sdk::testutils::{Address as _, Events as _, Ledger as _};
    use soroban_sdk::{contract, contractimpl, xdr, IntoVal, TryFromVal};

    #[contract]
    struct Stub;
    #[contractimpl]
    impl Stub {}

    struct Setup {
        env: Env,
        id: Address,
        admin: Address,
    }

    fn setup(noise_multiplier: u32) -> Setup {
        let env = Env::default();
        env.mock_all_auths();
        let id = env.register(Stub, ());
        let admin = Address::generate(&env);
        // One auth-checked call per frame: the test host rejects a second
        // require_auth for the same address within one invocation.
        env.as_contract(&id, || {
            initialize_privacy_stats(&env, admin.clone()).unwrap();
        });
        env.as_contract(&id, || {
            configure_private_aggregate(
                &env,
                admin.clone(),
                symbol_short!("score"),
                100,
                noise_multiplier,
            )
            .unwrap();
        });
        Setup { env, id, admin }
    }

    fn opted_in_player(s: &Setup) -> Address {
        let player = Address::generate(&s.env);
        s.env
            .as_contract(&s.id, || opt_in_privacy(&s.env, player.clone()).unwrap());
        player
    }

    fn raw_aggregate(s: &Setup, epoch: u64) -> PrivateAggregate {
        s.env.as_contract(&s.id, || {
            s.env
                .storage()
                .persistent()
                .get(&PrivacyKey::PrivacyAggregate(symbol_short!("score"), epoch))
                .unwrap()
        })
    }

    #[test]
    fn aggregates_clamped_values_without_storing_players() {
        let s = setup(1);
        let values = [40i128, 250, -7];
        for v in values {
            let p = opted_in_player(&s);
            s.env.as_contract(&s.id, || {
                contribute_anonymous_stat(&s.env, p, symbol_short!("score"), v).unwrap()
            });
        }
        // 250 clamps to 100, -7 clamps to 0.
        let agg = raw_aggregate(&s, 0);
        assert_eq!((agg.contributors, agg.sum, agg.max_bound), (3, 140, 100));
    }

    #[test]
    fn contribution_event_carries_no_player_or_value() {
        let s = setup(1);
        let p = opted_in_player(&s);
        s.env.as_contract(&s.id, || {
            contribute_anonymous_stat(&s.env, p, symbol_short!("score"), 42).unwrap()
        });
        let events = s.env.events().all();
        let last = events.events().last().unwrap();
        let xdr::ContractEventBody::V0(body) = &last.body;
        let data: soroban_sdk::Val = (symbol_short!("score"), 0u64).into_val(&s.env);
        let expected = xdr::ScVal::try_from_val(&s.env, &data).unwrap();
        assert_eq!(body.data, expected);
    }

    #[test]
    fn one_contribution_per_player_per_epoch() {
        let s = setup(1);
        let p = opted_in_player(&s);
        let contribute = |v| {
            s.env.as_contract(&s.id, || {
                contribute_anonymous_stat(&s.env, p.clone(), symbol_short!("score"), v)
            })
        };
        assert_eq!(contribute(1), Ok(0));
        assert_eq!(contribute(1), Err(PrivacyError::AlreadyContributed));
        s.env.ledger().set_timestamp(PRIVACY_EPOCH_SECS);
        assert_eq!(contribute(1), Ok(1));
    }

    #[test]
    fn contribution_requires_opt_in_and_config() {
        let s = setup(1);
        let stranger = Address::generate(&s.env);
        s.env.as_contract(&s.id, || {
            assert_eq!(
                contribute_anonymous_stat(&s.env, stranger.clone(), symbol_short!("score"), 1),
                Err(PrivacyError::NotOptedIn)
            );
        });
        let p = opted_in_player(&s);
        s.env.as_contract(&s.id, || {
            assert_eq!(
                contribute_anonymous_stat(&s.env, p, symbol_short!("kills"), 1),
                Err(PrivacyError::StatNotConfigured)
            );
        });
    }

    #[test]
    fn configuration_is_admin_only_and_bounded() {
        let s = setup(1);
        let other = Address::generate(&s.env);
        s.env.as_contract(&s.id, || {
            assert_eq!(
                initialize_privacy_stats(&s.env, other.clone()),
                Err(PrivacyError::AlreadyInitialized)
            );
        });
        s.env.as_contract(&s.id, || {
            assert_eq!(
                configure_private_aggregate(&s.env, other, symbol_short!("kills"), 10, 1),
                Err(PrivacyError::NotAuthorized)
            );
        });
        for (max, mult) in [
            (0, 1),
            (MAX_CONTRIBUTION_BOUND + 1, 1),
            (10, 0),
            (10, MAX_NOISE_MULTIPLIER + 1),
        ] {
            s.env.as_contract(&s.id, || {
                assert_eq!(
                    configure_private_aggregate(
                        &s.env,
                        s.admin.clone(),
                        symbol_short!("kills"),
                        max,
                        mult
                    ),
                    Err(PrivacyError::InvalidConfig)
                );
            });
        }
    }

    #[test]
    fn report_only_after_epoch_closes_and_only_once() {
        let s = setup(1);
        s.env.as_contract(&s.id, || {
            assert_eq!(
                publish_privacy_report(&s.env, symbol_short!("score"), 0),
                Err(PrivacyError::EpochNotClosed)
            );
        });
        s.env.ledger().set_timestamp(PRIVACY_EPOCH_SECS);
        s.env.as_contract(&s.id, || {
            let report = publish_privacy_report(&s.env, symbol_short!("score"), 0).unwrap();
            assert_eq!(report.epsilon_milli, 2000);
            assert_eq!(
                get_privacy_report(&s.env, symbol_short!("score"), 0),
                Ok(report)
            );
            assert_eq!(
                publish_privacy_report(&s.env, symbol_short!("score"), 0),
                Err(PrivacyError::ReportAlreadyPublished)
            );
            assert_eq!(
                get_privacy_report(&s.env, symbol_short!("score"), 7),
                Err(PrivacyError::ReportNotFound)
            );
        });
    }

    #[test]
    fn small_cohorts_hide_the_sum() {
        let s = setup(1);
        let p = opted_in_player(&s);
        let mut suppressed = 0;
        for epoch in 0..20u64 {
            s.env.ledger().set_timestamp(epoch * PRIVACY_EPOCH_SECS);
            s.env.as_contract(&s.id, || {
                contribute_anonymous_stat(&s.env, p.clone(), symbol_short!("score"), 90).unwrap();
            });
            s.env
                .ledger()
                .set_timestamp((epoch + 1) * PRIVACY_EPOCH_SECS);
            let report = s.env.as_contract(&s.id, || {
                publish_privacy_report(&s.env, symbol_short!("score"), epoch).unwrap()
            });
            assert_eq!(
                report.suppressed,
                report.noisy_contributors < i128::from(MIN_REPORT_COHORT)
            );
            if report.suppressed {
                suppressed += 1;
                assert_eq!(report.noisy_sum, 0);
            }
        }
        // One real contributor with b = 1 is almost always under the cohort floor.
        assert!(suppressed >= 15, "only {suppressed} of 20 suppressed");
    }

    #[test]
    fn geometric_sampler_matches_its_mean() {
        let s = setup(1);
        s.env.as_contract(&s.id, || {
            for scale in [1u128, 10, 1_000] {
                let n = 2_000u128;
                let total: u128 = (0..n).map(|_| sample_geometric(&s.env, scale)).sum();
                // E[X] = scale, Var[X] = scale * (scale + 1): allow 5 std of the mean.
                let mean_x100 = total * 100 / n;
                let tol_x100 = 5 * (10_000 * scale * (scale + 1) / n).isqrt() + 1;
                assert!(
                    mean_x100.abs_diff(scale * 100) <= tol_x100,
                    "scale {scale}: mean*100 = {mean_x100}"
                );
            }
        });
    }

    #[test]
    fn laplace_noise_is_centred() {
        let s = setup(1);
        s.env.as_contract(&s.id, || {
            let n = 2_000i128;
            let total: i128 = (0..n).map(|_| discrete_laplace(&s.env, 5)).sum();
            // Variance 2*5*6 = 60, so the std of the mean is ~0.17.
            assert!(
                (total * 100 / n).abs() <= 60,
                "mean*100 = {}",
                total * 100 / n
            );
        });
    }

    #[test]
    fn transparency_report_counts_without_double_counting_opt_ins() {
        let s = setup(1);
        let p = opted_in_player(&s);
        s.env
            .as_contract(&s.id, || opt_in_privacy(&s.env, p.clone()).unwrap());
        s.env.as_contract(&s.id, || {
            contribute_anonymous_stat(&s.env, p, symbol_short!("score"), 5).unwrap();
        });
        s.env.ledger().set_timestamp(PRIVACY_EPOCH_SECS);
        let t = s.env.as_contract(&s.id, || {
            publish_privacy_report(&s.env, symbol_short!("score"), 0).unwrap();
            get_privacy_transparency_report(&s.env)
        });
        assert_eq!(t.opted_in_players, 1);
        assert_eq!(t.total_contributions, 1);
        assert_eq!(t.reports_published, 1);
        assert_eq!(t.configured_stats.len(), 1);
        assert_eq!(
            (t.epoch_secs, t.min_report_cohort),
            (PRIVACY_EPOCH_SECS, MIN_REPORT_COHORT)
        );
    }
}
