# Dependency injection for contract modules

Contract modules used to reach straight into the Soroban SDK for storage,
ledger time and randomness. That made unit tests slow to write (register a
contract, wrap every call in `as_contract`, mutate the ledger to move time)
and impossible to run without a host. `src/traits/` puts one small trait in
front of each dependency so the logic can be tested with in-memory mocks.

## The traits

| Dependency | Trait | Production | Tests |
|------------|-------|------------|-------|
| storage    | `StorageProvider` (`get`, `set`, `has`, `remove`) | `RealStorageProvider::{persistent, instance, temporary}(env)` | `MockStorageProvider::new(&env)` |
| time       | `TimeProvider` (`current_timestamp`, `current_ledger`, `day_index`, `elapsed_since`) | `RealTimeProvider::new(env)` | `MockTimeProvider::new(ts, ledger)` |
| randomness | `RandomnessProvider` (`random_u64`, `random_seed`, `random_in_range`, `coin_flip`) | `RealRandomnessProvider::new(env)` | `MockRandomnessProvider::{new(seed), fixed(value)}` |

Keys and values for `StorageProvider` are anything the SDK converts to a host
`Val`, which is what `env.storage()` accepts, so a module's existing key
enums work unchanged.

## The pattern

Keep the public `Env`-taking entry point. Move the logic into a generic
`*_with` function that takes providers. The wrapper builds the real
providers and delegates:

```rust
pub fn apply_gathering(env: &Env, user: &Address, amount: u64) -> Result<u64, AntiWhaleError> {
    let effective = record_gathering_with(
        &RealStorageProvider::persistent(env),
        &RealTimeProvider::new(env),
        user,
        amount,
        tier_width,
    )?;
    // events, impact counters, ...
    Ok(effective)
}

pub fn record_gathering_with<S: StorageProvider, T: TimeProvider>(
    store: &S,
    time: &T,
    user: &Address,
    amount: u64,
    tier_width: u64,
) -> Result<u64, AntiWhaleError> {
    let key = AntiWhaleKey::DailyGathered(user.clone(), time.day_index());
    let before: u64 = store.get(&key).unwrap_or(0);
    store.set(&key, &before.checked_add(amount).ok_or(AntiWhaleError::ArithmeticOverflow)?);
    Ok(diminishing_returns(before, amount, tier_width))
}
```

Production behaviour is unchanged: callers still pass an `Env`. Tests call
the `*_with` function with mocks:

```rust
let env = Env::default();                       // only for Val conversions
let store = MockStorageProvider::new(&env);     // no contract registered
let clock = MockTimeProvider::new(0, 1);
let user = Address::generate(&env);

assert_eq!(record_gathering_with(&store, &clock, &user, 100, 100), Ok(100));
assert_eq!(record_gathering_with(&store, &clock, &user, 100, 100), Ok(80));
clock.advance_days(1);                          // new window, no ledger surgery
assert_eq!(record_gathering_with(&store, &clock, &user, 100, 100), Ok(100));
```

Generics are used instead of `&dyn Trait` so the provider calls
monomorphise away in the WASM build.

## What the mocks give you

- **No contract context.** No `env.register`, no `as_contract`, no
  `mock_all_auths` for logic that does not authorise.
- **Controllable time.** `advance_secs`, `advance_days`, `set_timestamp`.
- **Deterministic randomness.** `MockRandomnessProvider::fixed(v)` makes every
  roll return `v`, so both branches of a random outcome are two assertions.
  `pin(Some(v))` switches a seeded stream to fixed mid-test.
- **Failure injection.** `MockStorageProvider::fail_next_reads(n)` makes the
  next `n` reads report a missing entry, which is how "state vanished
  mid-flow" paths are exercised. `read_count`/`write_count` let a test assert
  a function touched storage exactly as often as intended.
- **Isolation.** Every mock is an owned value with interior mutability.
  Tests share no global state and run on all threads.

## Where it is used

| Module | Provider(s) | Entry points |
|--------|-------------|--------------|
| `economics::anti_whale` | storage, time | `record_gathering_with`, `record_operation_with`, `record_trade_volume_with`, `record_guild_contribution_with`, `add_activity_with` |
| `economics::apy_calculator` | time | `accrued_yield` |
| `yield_farming` | time | `resource_yield_with`, `nft_yield_with`, `il_compensation_with` |
| `staking` | (pure helpers over records) | `pending` reward math is provider-free |
| `clan_wars` | randomness | `resolve_battle_with` |
| `traits::randomness` | oracle | `RealRandomnessProvider::random_seed` wraps `randomness_oracle::request_random_seed` |

## Adding a provider to a module

1. Pick the smallest trait that covers the dependency. Do not add a method
   for one caller; compose two providers instead.
2. Add a `*_with` function taking `&impl Trait` (or a named generic) and move
   the logic into it. Keep the original signature as a thin wrapper that
   constructs the real provider.
3. Unit test the `*_with` function with mocks in the module's `tests`.
   Integration tests under `tests/` keep going through the contract client;
   they are the proof the wrapper wires the real provider correctly.
4. Events, audit logging and other host-only side effects stay in the
   wrapper, so the generic core remains free of `Env`.

## Limits

`MockStorageProvider` still needs an `Env` value to convert keys and values
to `Val`; it does not need a registered contract or a ledger. Tests that
assert on emitted events still go through the real `Env`.
