//! Reentrancy attack scenarios against the global guard.
//!
//! Soroban completes cross-contract calls atomically, but a `require_auth`
//! call to a *custom-account* contract can invoke arbitrary code before the
//! caller resumes. These tests model that callback: a hostile account tries to
//! re-enter a guarded critical section while the first invocation is still in
//! progress, and must be rejected.

#![cfg(test)]

use soroban_sdk::{contract, contracterror, contractimpl, symbol_short, Address, Env, Symbol};
use stellar_nebula_nomad::reentrancy_guard as guard;
use stellar_nebula_nomad::ReentrancyError;

#[contracterror]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u32)]
enum AttackError {
    Reentered = 1,
}

impl From<ReentrancyError> for AttackError {
    fn from(_: ReentrancyError) -> Self {
        AttackError::Reentered
    }
}

#[contract]
struct Vault;

#[contractimpl]
impl Vault {
    /// A value-moving operation that holds the lock across its whole body.
    /// `nested` selects whether the body tries to re-enter itself — this is
    /// what a malicious `require_auth` callback would do.
    pub fn withdraw(env: Env, nested: bool) -> Result<u32, AttackError> {
        guard::with_guard(&env, || {
            if nested {
                // Re-enter through a fresh guarded entry point. This models a
                // callback re-entering another function of the same contract.
                guard::acquire(&env)?;
            }
            Ok(1)
        })
    }

    /// Directly check the lock flag from inside the contract frame.
    pub fn locked(env: Env) -> bool {
        guard::is_locked(&env)
    }

    /// Two guarded calls in the same transaction: the second must succeed
    /// because the first released the lock.
    pub fn withdraw_twice(env: Env) -> Result<u32, AttackError> {
        Self::withdraw(env.clone(), false)?;
        Self::withdraw(env, false)
    }

    /// Cross-function reentrancy: calling another guarded function from inside
    /// a guarded section must be rejected.
    pub fn nested_function(env: Env) -> Result<u32, AttackError> {
        guard::with_guard(&env, || Self::withdraw(env.clone(), false))
    }

    /// Ensure the lock is released even when the guarded body returns an error.
    pub fn failing_call(env: Env) -> Result<u32, AttackError> {
        guard::with_guard(&env, || Err(AttackError::Reentered))
    }

    /// A guarded call that must succeed immediately after a failing one.
    pub fn recover(env: Env) -> Result<u32, AttackError> {
        let _: Result<u32, AttackError> = Self::failing_call(env.clone());
        Self::withdraw(env, false)
    }

    /// Symbol passthrough used by the event-trace scenario.
    pub fn trace(env: Env, tag: Symbol) -> Result<Symbol, AttackError> {
        guard::with_guard(&env, || Ok(tag))
    }
}

fn setup() -> (Env, Address) {
    let env = Env::default();
    env.mock_all_auths();
    let id = env.register(Vault, ());
    (env, id)
}

#[test]
fn attack_01_reentrant_callback_is_blocked() {
    let (env, id) = setup();
    let client = VaultClient::new(&env, &id);
    assert_eq!(client.try_withdraw(&true), Err(Ok(AttackError::Reentered)));
    assert!(!client.locked(), "failed nested call must not leave the lock held");
}

#[test]
fn attack_02_honest_call_succeeds_and_releases_lock() {
    let (env, id) = setup();
    let client = VaultClient::new(&env, &id);
    assert_eq!(client.withdraw(&false), 1);
    assert!(!client.locked());
    assert_eq!(client.withdraw(&false), 1);
}

#[test]
fn attack_03_cross_function_reentrancy_is_blocked() {
    let (env, id) = setup();
    let client = VaultClient::new(&env, &id);
    assert_eq!(client.try_nested_function(), Err(Ok(AttackError::Reentered)));
    assert!(!client.locked());
    assert_eq!(client.withdraw(&false), 1);
}

#[test]
fn attack_04_error_path_releases_lock() {
    let (env, id) = setup();
    let client = VaultClient::new(&env, &id);
    assert!(client.try_failing_call().is_err());
    assert!(!client.locked());
    assert_eq!(client.recover(), 1);
}

#[test]
fn attack_05_sequential_guarded_calls_in_one_frame() {
    let (env, id) = setup();
    let client = VaultClient::new(&env, &id);
    assert_eq!(client.withdraw_twice(), 1);
    assert!(!client.locked());
}

#[test]
fn attack_06_guard_does_not_leak_across_calls() {
    let (env, id) = setup();
    let client = VaultClient::new(&env, &id);
    let tag = symbol_short!("ok");
    assert_eq!(client.trace(&tag), tag);
    assert!(!client.locked());
    // A fresh contract frame starts unguarded.
    let (env2, id2) = setup();
    let client2 = VaultClient::new(&env2, &id2);
    assert_eq!(client2.withdraw(&false), 1);
}
