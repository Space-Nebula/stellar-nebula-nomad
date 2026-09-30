#![cfg(test)]
//! The dependency-injection providers from the crate's public surface:
//! module logic driven by mocks with no contract registered, and the real
//! providers wired through a contract frame.

use soroban_sdk::testutils::{Address as _, Ledger};
use soroban_sdk::{contracttype, symbol_short, Address, Env};
use stellar_nebula_nomad::economics::anti_whale::{
    record_gathering_with, record_operation_with, AntiWhaleError, OpKind,
};
use stellar_nebula_nomad::economics::apy_calculator::{accrued_yield, LockTier};
use stellar_nebula_nomad::traits::{
    MockRandomnessProvider, MockStorageProvider, MockTimeProvider, RandomnessProvider,
    RealRandomnessProvider, RealStorageProvider, RealTimeProvider, StorageProvider, StorageTier,
    TimeProvider,
};
use stellar_nebula_nomad::NebulaNomadContract;

#[contracttype]
#[derive(Clone)]
enum Key {
    Counter(u32),
}

#[test]
fn module_logic_runs_on_mocks_without_a_contract() {
    let env = Env::default();
    let store = MockStorageProvider::new(&env);
    let clock = MockTimeProvider::new(0, 1);
    let user = Address::generate(&env);

    // Diminishing returns across a mocked day boundary.
    assert_eq!(
        record_gathering_with(&store, &clock, &user, 100, 100),
        Ok(100)
    );
    assert_eq!(
        record_gathering_with(&store, &clock, &user, 100, 100),
        Ok(80)
    );
    clock.advance_days(1);
    assert_eq!(
        record_gathering_with(&store, &clock, &user, 100, 100),
        Ok(100)
    );

    // Operation caps with injected read failure: a vanished counter starts
    // over instead of trapping.
    assert_eq!(
        record_operation_with(&store, &clock, &user, OpKind::Scan, 3, 3),
        Ok(3)
    );
    assert_eq!(
        record_operation_with(&store, &clock, &user, OpKind::Scan, 1, 3),
        Err(AntiWhaleError::OperationCapExceeded)
    );
    store.fail_next_reads(1);
    assert_eq!(
        record_operation_with(&store, &clock, &user, OpKind::Scan, 1, 3),
        Ok(1)
    );
    assert!(store.read_count() > 0 && store.write_count() > 0);
}

#[test]
fn yield_accrual_follows_the_mock_clock() {
    let clock = MockTimeProvider::new(1_000, 1);
    let tier = LockTier::Days30;
    clock.advance_days(15);
    let half = accrued_yield(&clock, 100_000, tier.apy_bps(), 1_000, tier.duration_secs());
    clock.advance_days(15);
    let full = accrued_yield(&clock, 100_000, tier.apy_bps(), 1_000, tier.duration_secs());
    clock.advance_days(300);
    let capped = accrued_yield(&clock, 100_000, tier.apy_bps(), 1_000, tier.duration_secs());
    assert_eq!(half, Some(616));
    assert_eq!(full, Some(1_232));
    assert_eq!(capped, full);
}

#[test]
fn mocks_are_isolated_and_deterministic() {
    let env = Env::default();
    let a = MockStorageProvider::new(&env);
    let b = MockStorageProvider::new(&env);
    a.set(&Key::Counter(1), &7u32);
    assert_eq!(a.get::<_, u32>(&Key::Counter(1)), Some(7));
    assert!(!b.has(&Key::Counter(1)));

    let r1 = MockRandomnessProvider::new(99);
    let r2 = MockRandomnessProvider::new(99);
    assert_eq!(r1.random_u64(), r2.random_u64());
    assert_eq!(r1.random_seed(), r2.random_seed());
    let pinned = MockRandomnessProvider::fixed(4);
    assert_eq!(pinned.random_in_range(10, 20), 14);
    assert!(!pinned.coin_flip());
}

#[test]
fn real_providers_read_the_ledger_inside_a_contract_frame() {
    let env = Env::default();
    env.ledger().with_mut(|li| {
        li.timestamp = 3 * 86_400 + 5;
        li.sequence_number = 77;
    });
    let id = env.register(NebulaNomadContract, ());
    env.as_contract(&id, || {
        let clock = RealTimeProvider::new(&env);
        assert_eq!(clock.day_index(), 3);
        assert_eq!(clock.current_ledger(), 77);

        let store = RealStorageProvider::new(&env, StorageTier::Persistent);
        store.set(&Key::Counter(2), &5u64);
        assert_eq!(
            env.storage().persistent().get::<_, u64>(&Key::Counter(2)),
            Some(5)
        );
        store.remove(&Key::Counter(2));
        assert!(!store.has(&Key::Counter(2)));

        let rng = RealRandomnessProvider::new(&env);
        let v = rng.random_in_range(1, 3);
        assert!((1..=3).contains(&v));
        assert_ne!(rng.random_seed(), [0u8; 32]);
    });
    let _ = symbol_short!("traits");
}
