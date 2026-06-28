use soroban_sdk::{contracterror, contracttype, symbol_short, Address, Bytes, Env, Vec};

/// Maximum number of tokens in a single batch resolve call.
///
/// This is the documented **maximum safe batch size**: at
/// [`GAS_PER_METADATA_RESOLVE`] gas per item, ten resolutions stay within the
/// [`DEFAULT_METADATA_GAS_BUDGET`]. Callers that pass their own (smaller) gas
/// budget should use [`max_batch_for_budget`] to derive a safe size.
pub const MAX_METADATA_BATCH: u32 = 10;

/// Estimated gas (abstract units) consumed resolving a single token's
/// metadata: one persistent storage read plus gateway concatenation. Chosen
/// conservatively so the estimate over-approximates real consumption.
pub const GAS_PER_METADATA_RESOLVE: u64 = 5_000;

/// Default gas budget for a single `batch_resolve_metadata` call.
/// Sized so the maximum safe batch ([`MAX_METADATA_BATCH`]) fits exactly:
/// `MAX_METADATA_BATCH * GAS_PER_METADATA_RESOLVE`.
pub const DEFAULT_METADATA_GAS_BUDGET: u64 = 50_000;

/// Default IPFS gateway prefix (encoded as UTF-8 bytes).
/// "https://ipfs.io/ipfs/"
pub const DEFAULT_GATEWAY: &[u8] = b"https://ipfs.io/ipfs/";

// ─── Storage Keys ─────────────────────────────────────────────────────────

#[derive(Clone)]
#[contracttype]
pub enum MetadataKey {
    /// IPFS CID for a token: `TokenUri(token_id)`.
    TokenUri(u64),
    /// Configurable IPFS gateway bytes.
    Gateway,
}

// ─── Errors ───────────────────────────────────────────────────────────────

#[contracterror]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum MetadataError {
    /// CID bytes are empty or invalid.
    InvalidCID = 1,
    /// No metadata stored for the given token ID.
    TokenNotFound = 2,
    /// Metadata has already been set and is immutable after first set.
    AlreadySet = 3,
    /// Batch size exceeds the maximum of 10.
    BatchLimitExceeded = 4,
    /// Estimated gas for the batch exceeds the caller's gas budget.
    GasBudgetExceeded = 5,
}

// ─── Data Types ───────────────────────────────────────────────────────────

/// Resolved metadata for a single token.
#[derive(Clone)]
#[contracttype]
pub struct TokenMetadata {
    /// The token ID this metadata belongs to.
    pub token_id: u64,
    /// The raw IPFS CID bytes (e.g. "QmXxx..." or "bafy...").
    pub cid: Bytes,
    /// The IPFS gateway prefix bytes used for resolution.
    pub gateway: Bytes,
}

// ─── Internal Helpers ────────────────────────────────────────────────────

fn get_gateway(env: &Env) -> Bytes {
    env.storage()
        .instance()
        .get(&MetadataKey::Gateway)
        .unwrap_or_else(|| Bytes::from_slice(env, DEFAULT_GATEWAY))
}

fn validate_cid(cid: &Bytes) -> bool {
    cid.len() > 0
}

// ─── Public API ──────────────────────────────────────────────────────────

/// Set the IPFS CID for a token. Immutable after the first call.
///
/// `caller` must authorize. CID must be non-empty. Once set, the
/// mapping cannot be changed — ship metadata is permanent on-chain.
///
/// Emits a `MetadataUpdated` event on success.
pub fn set_metadata_uri(
    env: &Env,
    caller: &Address,
    token_id: u64,
    cid: Bytes,
) -> Result<(), MetadataError> {
    caller.require_auth();

    if !validate_cid(&cid) {
        return Err(MetadataError::InvalidCID);
    }

    if env
        .storage()
        .persistent()
        .has(&MetadataKey::TokenUri(token_id))
    {
        return Err(MetadataError::AlreadySet);
    }

    env.storage()
        .persistent()
        .set(&MetadataKey::TokenUri(token_id), &cid);

    env.events().publish(
        (symbol_short!("meta"), symbol_short!("updated")),
        (token_id, cid, caller.clone()),
    );

    Ok(())
}

/// Resolve full metadata for a token using the configured IPFS gateway.
///
/// Returns `TokenMetadata` containing the token ID, raw CID bytes, and
/// the gateway prefix. Callers concatenate gateway + CID to form the URL.
pub fn resolve_metadata(env: &Env, token_id: u64) -> Result<TokenMetadata, MetadataError> {
    let cid: Bytes = env
        .storage()
        .persistent()
        .get(&MetadataKey::TokenUri(token_id))
        .ok_or(MetadataError::TokenNotFound)?;

    Ok(TokenMetadata {
        token_id,
        cid,
        gateway: get_gateway(env),
    })
}

// ─── Gas Estimation & Budgeting ───────────────────────────────────────────

/// Estimate the gas required to resolve `count` token metadata entries.
///
/// Uses saturating multiplication so an oversized `count` reports the maximum
/// gas rather than overflowing.
pub fn estimate_batch_gas(count: u32) -> u64 {
    (count as u64).saturating_mul(GAS_PER_METADATA_RESOLVE)
}

/// Largest batch size that fits within `gas_budget`, capped at
/// [`MAX_METADATA_BATCH`]. Returns `0` when the budget cannot afford a single
/// resolution.
pub fn max_batch_for_budget(gas_budget: u64) -> u32 {
    // GAS_PER_METADATA_RESOLVE is a non-zero constant, so this never divides by zero.
    let affordable = gas_budget / GAS_PER_METADATA_RESOLVE;
    affordable.min(MAX_METADATA_BATCH as u64) as u32
}

/// Trim `token_ids` down to the largest prefix that can be safely resolved
/// within `gas_budget`. Lets callers process what fits now and re-submit the
/// remainder, instead of having the whole call fail.
pub fn adjust_batch_to_budget(env: &Env, token_ids: &Vec<u64>, gas_budget: u64) -> Vec<u64> {
    let take = token_ids.len().min(max_batch_for_budget(gas_budget));
    let mut out = Vec::new(env);
    for i in 0..take {
        out.push_back(token_ids.get(i).unwrap());
    }
    out
}

/// Batch resolve metadata for up to 10 tokens in a single call.
///
/// Reduces round-trips for fleet/grid display. Estimates gas up-front against
/// [`DEFAULT_METADATA_GAS_BUDGET`] and rejects oversized batches before doing
/// any work. Returns an error if any token ID is not found.
pub fn batch_resolve_metadata(
    env: &Env,
    token_ids: Vec<u64>,
) -> Result<Vec<TokenMetadata>, MetadataError> {
    batch_resolve_metadata_within_budget(env, token_ids, DEFAULT_METADATA_GAS_BUDGET)
}

/// Batch resolve metadata with an explicit `gas_budget`.
///
/// Estimates gas before processing and rejects the call with
/// [`MetadataError::GasBudgetExceeded`] if the estimate exceeds the budget,
/// preventing mid-batch transaction failures and wasted fees. Callers wanting
/// best-effort behaviour should pre-trim with [`adjust_batch_to_budget`].
pub fn batch_resolve_metadata_within_budget(
    env: &Env,
    token_ids: Vec<u64>,
    gas_budget: u64,
) -> Result<Vec<TokenMetadata>, MetadataError> {
    if token_ids.len() > MAX_METADATA_BATCH {
        return Err(MetadataError::BatchLimitExceeded);
    }

    if estimate_batch_gas(token_ids.len()) > gas_budget {
        return Err(MetadataError::GasBudgetExceeded);
    }

    let mut results = Vec::new(env);
    for i in 0..token_ids.len() {
        let token_id = token_ids.get(i).unwrap();
        let metadata = resolve_metadata(env, token_id)?;
        results.push_back(metadata);
    }

    Ok(results)
}

/// Update the IPFS gateway prefix. Admin-only.
///
/// Allows switching between gateways (e.g. Cloudflare, Pinata, Arweave)
/// without redeploying the contract — future-proof for Arweave fallback.
pub fn set_gateway(env: &Env, admin: &Address, gateway: Bytes) {
    admin.require_auth();
    env.storage()
        .instance()
        .set(&MetadataKey::Gateway, &gateway);
}

/// Return the currently configured IPFS gateway prefix bytes.
pub fn get_current_gateway(env: &Env) -> Bytes {
    get_gateway(env)
}

// ─── Tests ─────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;
    use soroban_sdk::{Env, Vec};

    proptest! {
        /// Gas estimation is monotonic and matches the per-item cost.
        #[test]
        fn estimate_matches_per_item_cost(count in 0u32..=MAX_METADATA_BATCH) {
            prop_assert_eq!(
                estimate_batch_gas(count),
                count as u64 * GAS_PER_METADATA_RESOLVE
            );
        }

        /// The derived max batch never exceeds the hard cap and always fits
        /// within the supplied budget.
        #[test]
        fn max_batch_respects_cap_and_budget(gas_budget in 0u64..=1_000_000u64) {
            let n = max_batch_for_budget(gas_budget);
            prop_assert!(n <= MAX_METADATA_BATCH);
            prop_assert!(estimate_batch_gas(n) <= gas_budget);
        }
    }

    #[test]
    fn default_budget_affords_max_batch() {
        assert_eq!(max_batch_for_budget(DEFAULT_METADATA_GAS_BUDGET), MAX_METADATA_BATCH);
        assert_eq!(
            estimate_batch_gas(MAX_METADATA_BATCH),
            DEFAULT_METADATA_GAS_BUDGET
        );
    }

    #[test]
    fn tiny_budget_affords_nothing() {
        assert_eq!(max_batch_for_budget(GAS_PER_METADATA_RESOLVE - 1), 0);
    }

    #[test]
    fn adjust_batch_trims_to_budget() {
        let env = Env::default();
        let mut ids = Vec::new(&env);
        for i in 0..MAX_METADATA_BATCH as u64 {
            ids.push_back(i);
        }
        // Budget for only 3 items.
        let trimmed = adjust_batch_to_budget(&env, &ids, GAS_PER_METADATA_RESOLVE * 3);
        assert_eq!(trimmed.len(), 3);
    }
}
