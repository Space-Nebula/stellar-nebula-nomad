//! Ledger storage behind a trait.
//!
//! [`StorageProvider`] mirrors the four operations every module uses on
//! `env.storage()`. [`RealStorageProvider`] forwards to one storage tier;
//! [`MockStorageProvider`] keeps entries in an in-memory map so logic can be
//! tested without registering a contract, and can inject read failures.

use core::cell::{Cell, RefCell};
use soroban_sdk::{Env, IntoVal, Map, TryFromVal, Val};

/// Key/value access to contract state.
///
/// Keys and values are anything the SDK can convert to a host `Val`, which is
/// exactly what the concrete `env.storage()` tiers accept, so a module can be
/// switched to the trait without changing its key enums.
pub trait StorageProvider {
    /// Read `key`, or `None` if absent (or unreadable in a mock with injected
    /// failures).
    fn get<K: IntoVal<Env, Val>, V: TryFromVal<Env, Val>>(&self, key: &K) -> Option<V>;

    /// Write `value` under `key`, replacing any previous entry.
    fn set<K: IntoVal<Env, Val>, V: IntoVal<Env, Val>>(&self, key: &K, value: &V);

    /// Whether `key` is present.
    fn has<K: IntoVal<Env, Val>>(&self, key: &K) -> bool;

    /// Delete `key` if present.
    fn remove<K: IntoVal<Env, Val>>(&self, key: &K);
}

/// Which Soroban storage tier a [`RealStorageProvider`] addresses.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StorageTier {
    /// Contract-instance storage: small config shared by every call.
    Instance,
    /// Persistent storage: long-lived per-entity records.
    Persistent,
    /// Temporary storage: short-lived entries that may expire.
    Temporary,
}

/// [`StorageProvider`] backed by one tier of `env.storage()`. The production
/// default.
pub struct RealStorageProvider<'a> {
    env: &'a Env,
    tier: StorageTier,
}

impl<'a> RealStorageProvider<'a> {
    /// Address the given tier.
    pub fn new(env: &'a Env, tier: StorageTier) -> Self {
        Self { env, tier }
    }

    /// Persistent tier.
    pub fn persistent(env: &'a Env) -> Self {
        Self::new(env, StorageTier::Persistent)
    }

    /// Instance tier.
    pub fn instance(env: &'a Env) -> Self {
        Self::new(env, StorageTier::Instance)
    }

    /// Temporary tier.
    pub fn temporary(env: &'a Env) -> Self {
        Self::new(env, StorageTier::Temporary)
    }

    /// The tier this provider addresses.
    pub fn tier(&self) -> StorageTier {
        self.tier
    }
}

impl StorageProvider for RealStorageProvider<'_> {
    fn get<K: IntoVal<Env, Val>, V: TryFromVal<Env, Val>>(&self, key: &K) -> Option<V> {
        match self.tier {
            StorageTier::Instance => self.env.storage().instance().get(key),
            StorageTier::Persistent => self.env.storage().persistent().get(key),
            StorageTier::Temporary => self.env.storage().temporary().get(key),
        }
    }

    fn set<K: IntoVal<Env, Val>, V: IntoVal<Env, Val>>(&self, key: &K, value: &V) {
        match self.tier {
            StorageTier::Instance => self.env.storage().instance().set(key, value),
            StorageTier::Persistent => self.env.storage().persistent().set(key, value),
            StorageTier::Temporary => self.env.storage().temporary().set(key, value),
        }
    }

    fn has<K: IntoVal<Env, Val>>(&self, key: &K) -> bool {
        match self.tier {
            StorageTier::Instance => self.env.storage().instance().has(key),
            StorageTier::Persistent => self.env.storage().persistent().has(key),
            StorageTier::Temporary => self.env.storage().temporary().has(key),
        }
    }

    fn remove<K: IntoVal<Env, Val>>(&self, key: &K) {
        match self.tier {
            StorageTier::Instance => self.env.storage().instance().remove(key),
            StorageTier::Persistent => self.env.storage().persistent().remove(key),
            StorageTier::Temporary => self.env.storage().temporary().remove(key),
        }
    }
}

/// In-memory [`StorageProvider`] for tests.
///
/// Entries live in a host `Map<Val, Val>` owned by the mock, so no contract
/// needs to be registered and two mocks never share state. Reads and writes
/// are counted, and `fail_next_reads` makes the following `n` reads report a
/// missing entry so error paths that depend on vanished state can be driven
/// deterministically.
pub struct MockStorageProvider {
    env: Env,
    entries: RefCell<Map<Val, Val>>,
    read_failures: Cell<u32>,
    reads: Cell<u32>,
    writes: Cell<u32>,
}

impl MockStorageProvider {
    /// Empty store. The `Env` is only used for `Val` conversions; nothing is
    /// written to its ledger.
    pub fn new(env: &Env) -> Self {
        Self {
            env: env.clone(),
            entries: RefCell::new(Map::new(env)),
            read_failures: Cell::new(0),
            reads: Cell::new(0),
            writes: Cell::new(0),
        }
    }

    /// Make the next `n` reads (`get` and `has`) behave as if the key were
    /// absent.
    pub fn fail_next_reads(&self, n: u32) {
        self.read_failures.set(n);
    }

    /// Number of `get`/`has` calls so far.
    pub fn read_count(&self) -> u32 {
        self.reads.get()
    }

    /// Number of `set`/`remove` calls so far.
    pub fn write_count(&self) -> u32 {
        self.writes.get()
    }

    /// Number of stored entries.
    pub fn len(&self) -> u32 {
        self.entries.borrow().len()
    }

    /// Whether the store is empty.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    fn consume_read(&self) -> bool {
        self.reads.set(self.reads.get().saturating_add(1));
        let pending = self.read_failures.get();
        if pending > 0 {
            self.read_failures.set(pending - 1);
            return false;
        }
        true
    }
}

impl StorageProvider for MockStorageProvider {
    fn get<K: IntoVal<Env, Val>, V: TryFromVal<Env, Val>>(&self, key: &K) -> Option<V> {
        if !self.consume_read() {
            return None;
        }
        let k: Val = key.into_val(&self.env);
        let raw: Val = self.entries.borrow().get(k)?;
        V::try_from_val(&self.env, &raw).ok()
    }

    fn set<K: IntoVal<Env, Val>, V: IntoVal<Env, Val>>(&self, key: &K, value: &V) {
        self.writes.set(self.writes.get().saturating_add(1));
        let k: Val = key.into_val(&self.env);
        let v: Val = value.into_val(&self.env);
        self.entries.borrow_mut().set(k, v);
    }

    fn has<K: IntoVal<Env, Val>>(&self, key: &K) -> bool {
        if !self.consume_read() {
            return false;
        }
        let k: Val = key.into_val(&self.env);
        self.entries.borrow().contains_key(k)
    }

    fn remove<K: IntoVal<Env, Val>>(&self, key: &K) {
        self.writes.set(self.writes.get().saturating_add(1));
        let k: Val = key.into_val(&self.env);
        self.entries.borrow_mut().remove(k);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use soroban_sdk::{contracttype, symbol_short, Symbol};

    #[contracttype]
    #[derive(Clone)]
    enum Key {
        Counter(Symbol),
        Flag,
    }

    #[test]
    fn mock_round_trips_typed_values_without_a_contract() {
        let env = Env::default();
        let store = MockStorageProvider::new(&env);
        let key = Key::Counter(symbol_short!("scans"));

        assert!(!store.has(&key));
        assert_eq!(store.get::<_, u64>(&key), None);

        store.set(&key, &7u64);
        assert!(store.has(&key));
        assert_eq!(store.get::<_, u64>(&key), Some(7));

        store.set(&Key::Flag, &true);
        assert_eq!(store.get::<_, bool>(&Key::Flag), Some(true));
        assert_eq!(store.len(), 2);

        store.remove(&key);
        assert!(!store.has(&key));
        assert_eq!(store.len(), 1);
        assert_eq!(store.write_count(), 3);
    }

    #[test]
    fn mock_injects_read_failures_then_recovers() {
        let env = Env::default();
        let store = MockStorageProvider::new(&env);
        store.set(&Key::Flag, &1u32);

        store.fail_next_reads(2);
        assert_eq!(store.get::<_, u32>(&Key::Flag), None);
        assert!(!store.has(&Key::Flag));
        assert_eq!(store.get::<_, u32>(&Key::Flag), Some(1));
        assert_eq!(store.read_count(), 3);
    }

    #[test]
    fn two_mocks_share_nothing() {
        let env = Env::default();
        let a = MockStorageProvider::new(&env);
        let b = MockStorageProvider::new(&env);
        a.set(&Key::Flag, &5u32);
        assert_eq!(b.get::<_, u32>(&Key::Flag), None);
        assert!(b.is_empty());
    }

    #[test]
    fn real_provider_addresses_each_tier_independently() {
        let env = Env::default();
        let id = env.register(crate::NebulaNomadContract, ());
        env.as_contract(&id, || {
            let persistent = RealStorageProvider::persistent(&env);
            let instance = RealStorageProvider::instance(&env);
            let temporary = RealStorageProvider::temporary(&env);

            persistent.set(&Key::Flag, &1u32);
            assert_eq!(persistent.get::<_, u32>(&Key::Flag), Some(1));
            assert!(!instance.has(&Key::Flag));
            assert!(!temporary.has(&Key::Flag));

            instance.set(&Key::Flag, &2u32);
            temporary.set(&Key::Flag, &3u32);
            assert_eq!(env.storage().instance().get::<_, u32>(&Key::Flag), Some(2));
            assert_eq!(env.storage().temporary().get::<_, u32>(&Key::Flag), Some(3));

            persistent.remove(&Key::Flag);
            assert!(!env.storage().persistent().has(&Key::Flag));
            assert_eq!(persistent.tier(), StorageTier::Persistent);
        });
    }
}
