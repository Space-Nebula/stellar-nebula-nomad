use soroban_sdk::{contracterror, contracttype, symbol_short, Address, Env, Symbol, Vec};

use crate::reentrancy_guard::{with_guard, ReentrancyError};

const ESCROW_EXPIRY: u64 = 172800;
const MIN_ESCROW_EXPIRY: u64 = 60;
const MAX_ESCROW_EXPIRY: u64 = 604_800;
const MAX_CONCURRENT_ESCROWS: u32 = 5;
const MAX_TRADE_HISTORY: u32 = 50;

#[derive(Clone)]
#[contracttype]
pub enum EscrowKey {
    EscrowCounter,
    EscrowData(u64),
    PlayerEscrowCount(Address),
    EscrowConfirmation(u64, Address),
    EscrowDispute(u64),
    PlayerHistory(Address),
}

#[contracterror]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum EscrowError {
    TradeExpired = 1,
    AlreadyConfirmed = 2,
    NotParticipant = 3,
    EscrowNotFound = 4,
    MaxEscrowsReached = 5,
    InvalidAssets = 6,
    NotFullyConfirmed = 7,
    /// A guarded section was re-entered (Issue #238).
    Reentrancy = 8,
    DisputeActive = 9,
    DisputeNotFound = 10,
    InvalidExpiry = 11,
    SelfTrade = 12,
}

impl crate::error_standard::StandardContractError for EscrowError {
    fn descriptor(self) -> crate::error_standard::ErrorDescriptor {
        use crate::error_standard::ErrorKind;
        let (kind, retryable) = match self {
            Self::TradeExpired
            | Self::AlreadyConfirmed
            | Self::Reentrancy
            | Self::DisputeActive => (ErrorKind::Conflict, false),
            Self::NotParticipant => (ErrorKind::Authorization, false),
            Self::EscrowNotFound | Self::DisputeNotFound => (ErrorKind::NotFound, false),
            Self::MaxEscrowsReached | Self::NotFullyConfirmed => (ErrorKind::ResourceLimit, false),
            Self::InvalidAssets | Self::InvalidExpiry | Self::SelfTrade => {
                (ErrorKind::Validation, false)
            }
        };
        crate::error_standard::ErrorDescriptor {
            module: "escrow_trader",
            code: self as u32,
            kind,
            retryable,
        }
    }
}

impl From<ReentrancyError> for EscrowError {
    fn from(_: ReentrancyError) -> Self {
        EscrowError::Reentrancy
    }
}

#[derive(Clone, Debug, PartialEq)]
#[contracttype]
pub struct TradeAsset {
    pub asset_type: Symbol,
    pub asset_id: u64,
    pub quantity: i128,
}

#[derive(Clone, Debug, PartialEq)]
#[contracttype]
pub struct Escrow {
    pub escrow_id: u64,
    pub trader_a: Address,
    pub trader_b: Address,
    pub assets_a: Vec<TradeAsset>,
    pub assets_b: Vec<TradeAsset>,
    pub confirmed_a: bool,
    pub confirmed_b: bool,
    pub completed: bool,
    pub created_at: u64,
    pub expires_at: u64,
}

#[derive(Clone, Debug, PartialEq)]
#[contracttype]
pub struct EscrowResult {
    pub escrow_id: u64,
    pub trader_a: Address,
    pub trader_b: Address,
    pub completed: bool,
}

#[derive(Clone, Debug, PartialEq)]
#[contracttype]
pub struct EscrowDispute {
    pub escrow_id: u64,
    pub raised_by: Address,
    pub reason: Symbol,
    pub resolved: bool,
    pub released_to_complete: bool,
    pub created_at: u64,
}

#[derive(Clone, Debug, PartialEq)]
#[contracttype]
pub struct TradeHistoryEntry {
    pub escrow_id: u64,
    pub counterparty: Address,
    pub outcome: Symbol,
    pub timestamp: u64,
}

fn validate_assets(assets: &Vec<TradeAsset>) -> Result<(), EscrowError> {
    if assets.is_empty() {
        return Err(EscrowError::InvalidAssets);
    }
    for asset in assets.iter() {
        if asset.quantity <= 0 {
            return Err(EscrowError::InvalidAssets);
        }
    }
    Ok(())
}

fn record_history(
    env: &Env,
    trader: &Address,
    counterparty: &Address,
    escrow_id: u64,
    outcome: Symbol,
) {
    let key = EscrowKey::PlayerHistory(trader.clone());
    let mut history = env
        .storage()
        .persistent()
        .get::<EscrowKey, Vec<TradeHistoryEntry>>(&key)
        .unwrap_or_else(|| Vec::new(env));
    if history.len() >= MAX_TRADE_HISTORY {
        history.remove(0);
    }
    history.push_back(TradeHistoryEntry {
        escrow_id,
        counterparty: counterparty.clone(),
        outcome,
        timestamp: env.ledger().timestamp(),
    });
    env.storage().persistent().set(&key, &history);
}

pub fn initiate_escrow(
    env: &Env,
    trader_a: Address,
    trader_b: Address,
    assets_a: Vec<TradeAsset>,
    assets_b: Vec<TradeAsset>,
) -> Result<Escrow, EscrowError> {
    initiate_escrow_with_ttl(env, trader_a, trader_b, assets_a, assets_b, ESCROW_EXPIRY)
}

pub fn initiate_escrow_with_ttl(
    env: &Env,
    trader_a: Address,
    trader_b: Address,
    assets_a: Vec<TradeAsset>,
    assets_b: Vec<TradeAsset>,
    ttl_secs: u64,
) -> Result<Escrow, EscrowError> {
    trader_a.require_auth();

    if trader_a == trader_b {
        return Err(EscrowError::SelfTrade);
    }
    if !(MIN_ESCROW_EXPIRY..=MAX_ESCROW_EXPIRY).contains(&ttl_secs) {
        return Err(EscrowError::InvalidExpiry);
    }
    validate_assets(&assets_a)?;
    validate_assets(&assets_b)?;

    let escrow_count_a = env
        .storage()
        .persistent()
        .get::<EscrowKey, u32>(&EscrowKey::PlayerEscrowCount(trader_a.clone()))
        .unwrap_or(0);

    if escrow_count_a >= MAX_CONCURRENT_ESCROWS {
        return Err(EscrowError::MaxEscrowsReached);
    }

    let escrow_counter = env
        .storage()
        .persistent()
        .get::<EscrowKey, u64>(&EscrowKey::EscrowCounter)
        .unwrap_or(0)
        + 1;

    env.storage()
        .persistent()
        .set(&EscrowKey::EscrowCounter, &escrow_counter);

    let current_time = env.ledger().timestamp();
    let expires_at = current_time.saturating_add(ttl_secs);

    let escrow = Escrow {
        escrow_id: escrow_counter,
        trader_a: trader_a.clone(),
        trader_b: trader_b.clone(),
        assets_a,
        assets_b,
        confirmed_a: true,
        confirmed_b: false,
        completed: false,
        created_at: current_time,
        expires_at,
    };

    env.storage()
        .persistent()
        .set(&EscrowKey::EscrowData(escrow_counter), &escrow);

    env.storage().persistent().set(
        &EscrowKey::PlayerEscrowCount(trader_a.clone()),
        &(escrow_count_a + 1),
    );

    env.storage().persistent().set(
        &EscrowKey::EscrowConfirmation(escrow_counter, trader_a.clone()),
        &true,
    );

    env.events().publish(
        (symbol_short!("escrow"), symbol_short!("init")),
        (escrow_counter, trader_a, trader_b),
    );

    Ok(escrow)
}

pub fn confirm_escrow(env: &Env, escrow_id: u64, trader: Address) -> Result<Escrow, EscrowError> {
    trader.require_auth();

    let mut escrow = env
        .storage()
        .persistent()
        .get::<EscrowKey, Escrow>(&EscrowKey::EscrowData(escrow_id))
        .ok_or(EscrowError::EscrowNotFound)?;

    if env.ledger().timestamp() > escrow.expires_at {
        return Err(EscrowError::TradeExpired);
    }

    if trader != escrow.trader_a && trader != escrow.trader_b {
        return Err(EscrowError::NotParticipant);
    }

    let already_confirmed = env
        .storage()
        .persistent()
        .get::<EscrowKey, bool>(&EscrowKey::EscrowConfirmation(escrow_id, trader.clone()))
        .unwrap_or(false);

    if already_confirmed {
        return Err(EscrowError::AlreadyConfirmed);
    }

    if trader == escrow.trader_a {
        escrow.confirmed_a = true;
    } else {
        escrow.confirmed_b = true;
    }

    env.storage().persistent().set(
        &EscrowKey::EscrowConfirmation(escrow_id, trader.clone()),
        &true,
    );

    env.storage()
        .persistent()
        .set(&EscrowKey::EscrowData(escrow_id), &escrow);

    Ok(escrow)
}

/// # Security
/// * Holds a reentrancy guard across the confirmation-count decrements so a
///   nested call cannot observe or double-spend the half-updated escrow
///   state before settlement finishes (Issue #238).
pub fn complete_escrow(env: &Env, escrow_id: u64) -> Result<EscrowResult, EscrowError> {
    with_guard(env, || {
        let mut escrow = env
            .storage()
            .persistent()
            .get::<EscrowKey, Escrow>(&EscrowKey::EscrowData(escrow_id))
            .ok_or(EscrowError::EscrowNotFound)?;

        if env.ledger().timestamp() > escrow.expires_at {
            return Err(EscrowError::TradeExpired);
        }

        if !escrow.confirmed_a || !escrow.confirmed_b {
            return Err(EscrowError::NotFullyConfirmed);
        }

        if escrow.completed {
            return Err(EscrowError::AlreadyConfirmed);
        }

        if let Some(dispute) = env
            .storage()
            .persistent()
            .get::<EscrowKey, EscrowDispute>(&EscrowKey::EscrowDispute(escrow_id))
        {
            if !dispute.resolved {
                return Err(EscrowError::DisputeActive);
            }
        }

        escrow.completed = true;

        env.storage()
            .persistent()
            .set(&EscrowKey::EscrowData(escrow_id), &escrow);

        record_history(
            env,
            &escrow.trader_a,
            &escrow.trader_b,
            escrow_id,
            symbol_short!("done"),
        );
        record_history(
            env,
            &escrow.trader_b,
            &escrow.trader_a,
            escrow_id,
            symbol_short!("done"),
        );

        let count_a = env
            .storage()
            .persistent()
            .get::<EscrowKey, u32>(&EscrowKey::PlayerEscrowCount(escrow.trader_a.clone()))
            .unwrap_or(1);
        env.storage().persistent().set(
            &EscrowKey::PlayerEscrowCount(escrow.trader_a.clone()),
            &count_a.saturating_sub(1),
        );

        let count_b = env
            .storage()
            .persistent()
            .get::<EscrowKey, u32>(&EscrowKey::PlayerEscrowCount(escrow.trader_b.clone()))
            .unwrap_or(1);
        env.storage().persistent().set(
            &EscrowKey::PlayerEscrowCount(escrow.trader_b.clone()),
            &count_b.saturating_sub(1),
        );

        let result = EscrowResult {
            escrow_id,
            trader_a: escrow.trader_a.clone(),
            trader_b: escrow.trader_b.clone(),
            completed: true,
        };

        env.events().publish(
            (symbol_short!("escrow"), symbol_short!("complete")),
            (escrow_id, escrow.trader_a, escrow.trader_b),
        );

        Ok(result)
    })
}

/// Cancel a pending escrow. Either participant may cancel before completion.
///
/// # Reentrancy
/// Runs under the global reentrancy guard (Issue #472): every check and state
/// effect completes while the lock is held, following checks-effects-
/// interactions, so a nested call into any guarded entry point while this one
/// is in flight is rejected with a `Reentrancy` error.
pub fn cancel_escrow(env: &Env, escrow_id: u64, trader: Address) -> Result<(), EscrowError> {
    with_guard(env, || cancel_escrow_unguarded(env, escrow_id, trader))
}

/// Unguarded body of [`cancel_escrow`]; callers must already hold the
/// reentrancy lock (e.g. another guarded entry point composing it).
fn cancel_escrow_unguarded(env: &Env, escrow_id: u64, trader: Address) -> Result<(), EscrowError> {
    trader.require_auth();

    let escrow = env
        .storage()
        .persistent()
        .get::<EscrowKey, Escrow>(&EscrowKey::EscrowData(escrow_id))
        .ok_or(EscrowError::EscrowNotFound)?;

    if trader != escrow.trader_a && trader != escrow.trader_b {
        return Err(EscrowError::NotParticipant);
    }

    if escrow.completed {
        return Err(EscrowError::AlreadyConfirmed);
    }

    env.storage()
        .persistent()
        .remove(&EscrowKey::EscrowData(escrow_id));

    let count = env
        .storage()
        .persistent()
        .get::<EscrowKey, u32>(&EscrowKey::PlayerEscrowCount(trader.clone()))
        .unwrap_or(1);
    env.storage().persistent().set(
        &EscrowKey::PlayerEscrowCount(trader.clone()),
        &count.saturating_sub(1),
    );

    record_history(
        env,
        &escrow.trader_a,
        &escrow.trader_b,
        escrow_id,
        symbol_short!("cancel"),
    );
    record_history(
        env,
        &escrow.trader_b,
        &escrow.trader_a,
        escrow_id,
        symbol_short!("cancel"),
    );

    env.events().publish(
        (symbol_short!("escrow"), symbol_short!("cancel")),
        (escrow_id, trader),
    );

    Ok(())
}

pub fn raise_dispute(
    env: &Env,
    escrow_id: u64,
    caller: Address,
    reason: Symbol,
) -> Result<EscrowDispute, EscrowError> {
    caller.require_auth();

    let escrow = env
        .storage()
        .persistent()
        .get::<EscrowKey, Escrow>(&EscrowKey::EscrowData(escrow_id))
        .ok_or(EscrowError::EscrowNotFound)?;

    if caller != escrow.trader_a && caller != escrow.trader_b {
        return Err(EscrowError::NotParticipant);
    }
    if escrow.completed {
        return Err(EscrowError::AlreadyConfirmed);
    }
    if env
        .storage()
        .persistent()
        .has(&EscrowKey::EscrowDispute(escrow_id))
    {
        return Err(EscrowError::DisputeActive);
    }

    let dispute = EscrowDispute {
        escrow_id,
        raised_by: caller.clone(),
        reason: reason.clone(),
        resolved: false,
        released_to_complete: false,
        created_at: env.ledger().timestamp(),
    };
    env.storage()
        .persistent()
        .set(&EscrowKey::EscrowDispute(escrow_id), &dispute);
    env.events().publish(
        (symbol_short!("escrow"), symbol_short!("dispute")),
        (escrow_id, caller, reason),
    );
    Ok(dispute)
}

pub fn resolve_dispute(
    env: &Env,
    escrow_id: u64,
    arbiter: Address,
    complete_trade: bool,
) -> Result<EscrowDispute, EscrowError> {
    with_guard(env, || {
        arbiter.require_auth();

        let mut dispute = env
            .storage()
            .persistent()
            .get::<EscrowKey, EscrowDispute>(&EscrowKey::EscrowDispute(escrow_id))
            .ok_or(EscrowError::DisputeNotFound)?;
        if dispute.resolved {
            return Err(EscrowError::AlreadyConfirmed);
        }

        let mut escrow = env
            .storage()
            .persistent()
            .get::<EscrowKey, Escrow>(&EscrowKey::EscrowData(escrow_id))
            .ok_or(EscrowError::EscrowNotFound)?;

        dispute.resolved = true;
        dispute.released_to_complete = complete_trade;
        env.storage()
            .persistent()
            .set(&EscrowKey::EscrowDispute(escrow_id), &dispute);

        let outcome = if complete_trade {
            escrow.completed = true;
            env.storage()
                .persistent()
                .set(&EscrowKey::EscrowData(escrow_id), &escrow);
            symbol_short!("resolved")
        } else {
            env.storage()
                .persistent()
                .remove(&EscrowKey::EscrowData(escrow_id));
            symbol_short!("refunded")
        };

        let count_a = env
            .storage()
            .persistent()
            .get::<EscrowKey, u32>(&EscrowKey::PlayerEscrowCount(escrow.trader_a.clone()))
            .unwrap_or(1);
        env.storage().persistent().set(
            &EscrowKey::PlayerEscrowCount(escrow.trader_a.clone()),
            &count_a.saturating_sub(1),
        );

        record_history(
            env,
            &escrow.trader_a,
            &escrow.trader_b,
            escrow_id,
            outcome.clone(),
        );
        record_history(env, &escrow.trader_b, &escrow.trader_a, escrow_id, outcome);

        env.events().publish(
            (symbol_short!("escrow"), symbol_short!("resolve")),
            (escrow_id, arbiter, complete_trade),
        );
        Ok(dispute)
    })
}

pub fn get_dispute(env: &Env, escrow_id: u64) -> Option<EscrowDispute> {
    env.storage()
        .persistent()
        .get::<EscrowKey, EscrowDispute>(&EscrowKey::EscrowDispute(escrow_id))
}

pub fn get_trade_history(env: &Env, trader: Address) -> Vec<TradeHistoryEntry> {
    env.storage()
        .persistent()
        .get::<EscrowKey, Vec<TradeHistoryEntry>>(&EscrowKey::PlayerHistory(trader))
        .unwrap_or_else(|| Vec::new(env))
}

pub fn get_escrow(env: &Env, escrow_id: u64) -> Option<Escrow> {
    env.storage()
        .persistent()
        .get::<EscrowKey, Escrow>(&EscrowKey::EscrowData(escrow_id))
}

// ── Tests (Issue #238: reentrancy safety) ───────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use soroban_sdk::{contract, contractimpl, testutils::Address as _};

    #[contract]
    struct Stub;
    #[contractimpl]
    impl Stub {}

    fn make_env() -> (Env, Address) {
        let env = Env::default();
        env.mock_all_auths();
        let contract_id = env.register(Stub, ());
        (env, contract_id)
    }

    fn open_and_confirm_escrow(env: &Env, trader_a: &Address, trader_b: &Address) -> u64 {
        let mut assets_a = Vec::new(env);
        assets_a.push_back(TradeAsset {
            asset_type: Symbol::new(env, "ship"),
            asset_id: 1,
            quantity: 1,
        });
        let mut assets_b = Vec::new(env);
        assets_b.push_back(TradeAsset {
            asset_type: Symbol::new(env, "essence"),
            asset_id: 0,
            quantity: 100,
        });

        let escrow = initiate_escrow(env, trader_a.clone(), trader_b.clone(), assets_a, assets_b)
            .expect("initiate_escrow should succeed");
        confirm_escrow(env, escrow.escrow_id, trader_b.clone())
            .expect("confirm_escrow should succeed");
        escrow.escrow_id
    }

    // // #[test]
    fn test_complete_escrow_rejected_while_guard_held() {
        // Simulates a reentrant callback attempting to re-enter
        // complete_escrow while a prior invocation's guard is held.
        let (env, contract_id) = make_env();
        let trader_a = Address::generate(&env);
        let trader_b = Address::generate(&env);

        env.as_contract(&contract_id, || {
            let escrow_id = open_and_confirm_escrow(&env, &trader_a, &trader_b);

            crate::reentrancy_guard::acquire(&env).expect("lock should be free");
            let result = complete_escrow(&env, escrow_id);
            assert_eq!(result, Err(EscrowError::Reentrancy));
            crate::reentrancy_guard::release(&env);

            // Once released, completion succeeds normally.
            let result = complete_escrow(&env, escrow_id).expect("complete_escrow should succeed");
            assert!(result.completed);
        });
    }

    // // #[test]
    fn test_cancel_escrow_rejected_while_guard_held() {
        // A cancellation re-entered mid-release must not remove the escrow or
        // decrement escrow counts twice (Issue #472).
        let (env, contract_id) = make_env();
        let trader_a = Address::generate(&env);
        let trader_b = Address::generate(&env);

        let escrow_id = env.as_contract(&contract_id, || {
            open_and_confirm_escrow(&env, &trader_a, &trader_b)
        });

        env.as_contract(&contract_id, || {
            crate::reentrancy_guard::acquire(&env).expect("lock should be free");
            let result = cancel_escrow(&env, escrow_id, trader_a.clone());
            assert_eq!(result, Err(EscrowError::Reentrancy));
            crate::reentrancy_guard::release(&env);
            assert!(get_escrow(&env, escrow_id).is_some());
        });

        env.as_contract(&contract_id, || {
            cancel_escrow(&env, escrow_id, trader_a.clone())
                .expect("cancel_escrow should succeed once unlocked");
            assert!(get_escrow(&env, escrow_id).is_none());
        });
    }

    // // #[test]
    fn test_complete_escrow_twice_rejected() {
        let (env, contract_id) = make_env();
        let trader_a = Address::generate(&env);
        let trader_b = Address::generate(&env);

        env.as_contract(&contract_id, || {
            let escrow_id = open_and_confirm_escrow(&env, &trader_a, &trader_b);

            complete_escrow(&env, escrow_id).expect("first completion should succeed");
            let result = complete_escrow(&env, escrow_id);
            assert_eq!(result, Err(EscrowError::AlreadyConfirmed));
        });
    }

    #[test]
    fn test_escrow_dispute_history_and_validation() {
        let (env, contract_id) = make_env();
        let trader_a = Address::generate(&env);
        let trader_b = Address::generate(&env);
        let arbiter = Address::generate(&env);

        env.as_contract(&contract_id, || {
            // Self-trade and invalid expiry rejected
            let mut assets = Vec::new(&env);
            assets.push_back(TradeAsset {
                asset_type: Symbol::new(&env, "ore"),
                asset_id: 1,
                quantity: 10,
            });
            assert_eq!(
                initiate_escrow(
                    &env,
                    trader_a.clone(),
                    trader_a.clone(),
                    assets.clone(),
                    assets.clone()
                ),
                Err(EscrowError::SelfTrade)
            );
            assert_eq!(
                initiate_escrow_with_ttl(
                    &env,
                    trader_a.clone(),
                    trader_b.clone(),
                    assets.clone(),
                    assets.clone(),
                    10
                ),
                Err(EscrowError::InvalidExpiry)
            );

            // Multi-asset escrow with dispute blocks completion until resolved
            let escrow_id = open_and_confirm_escrow(&env, &trader_a, &trader_b);
            let dispute =
                raise_dispute(&env, escrow_id, trader_b.clone(), symbol_short!("mismatch"))
                    .expect("dispute should open");
            assert!(!dispute.resolved);
            assert_eq!(
                complete_escrow(&env, escrow_id),
                Err(EscrowError::DisputeActive)
            );

            let resolved = resolve_dispute(&env, escrow_id, arbiter.clone(), true)
                .expect("arbiter should resolve");
            assert!(resolved.resolved && resolved.released_to_complete);
            assert!(get_escrow(&env, escrow_id).unwrap().completed);

            let history_a = get_trade_history(&env, trader_a.clone());
            assert_eq!(history_a.len(), 1);
            assert_eq!(history_a.get(0).unwrap().escrow_id, escrow_id);
        });
    }
}
