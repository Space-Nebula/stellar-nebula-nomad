//! Trait-based dependency injection for contract modules.
//!
//! Soroban modules traditionally reach straight into the SDK for the three
//! things that make them hard to unit test: ledger storage, ledger time and
//! randomness. This module defines one small trait for each of those
//! dependencies plus two implementations per trait:
//!
//! | Dependency | Trait                  | Production impl          | Test impl                |
//! |------------|------------------------|--------------------------|--------------------------|
//! | storage    | [`StorageProvider`]    | [`RealStorageProvider`]  | [`MockStorageProvider`]  |
//! | time       | [`TimeProvider`]       | [`RealTimeProvider`]     | [`MockTimeProvider`]     |
//! | randomness | [`RandomnessProvider`] | [`RealRandomnessProvider`] | [`MockRandomnessProvider`] |
//!
//! # The pattern
//!
//! A module keeps its public, `Env`-taking entry point for backward
//! compatibility and moves the logic into a generic `*_with` function that
//! receives providers instead of touching the SDK:
//!
//! ```rust,ignore
//! pub fn day_index(env: &Env) -> u64 {
//!     day_index_with(&RealTimeProvider::new(env))
//! }
//!
//! pub fn day_index_with<T: TimeProvider>(time: &T) -> u64 {
//!     time.day_index()
//! }
//! ```
//!
//! Production callers never notice the change: the `Env` wrapper builds the
//! real providers and delegates. Tests call the `*_with` variant directly
//! with mocks, which gives them:
//!
//! * **No contract context.** Mock providers do not need `env.register` or
//!   `as_contract`; storage lives in a plain map owned by the mock.
//! * **Controllable time.** `MockTimeProvider::advance_secs` replaces ledger
//!   surgery through `env.ledger().with_mut`.
//! * **Deterministic randomness.** `MockRandomnessProvider::fixed` pins the
//!   value every roll returns, so branch coverage of "won" and "lost" paths is
//!   a two-line test.
//! * **Failure injection.** `MockStorageProvider::fail_next_reads` makes the
//!   next `n` reads report a missing entry, which is how the "state vanished
//!   mid-flow" error paths get exercised.
//! * **Parallel tests.** Every mock is an owned value with interior
//!   mutability, so tests share no global state and run on all threads.
//!
//! Generics (`T: TimeProvider`) are used instead of `&dyn Trait` so the
//! provider calls monomorphise away in the WASM build; the trait objects would
//! cost a vtable per call site for no benefit inside a contract.
//!
//! # Adding a provider to a module
//!
//! 1. Pick the smallest trait that covers the dependency. Do not add methods
//!    to a trait for one caller; compose two providers instead.
//! 2. Add a `*_with` function taking `&impl Trait` (or a generic parameter)
//!    and move the logic there. Keep the original signature as a thin wrapper
//!    that constructs the real provider.
//! 3. Unit test the `*_with` function with the mock. Integration tests under
//!    `tests/` keep going through the contract client; they are the proof the
//!    wrapper still wires the real provider correctly.
//!
//! Modules using this pattern today: `economics::anti_whale`,
//! `economics::apy_calculator`, `staking`, `yield_farming`, `clan_wars`.

pub mod randomness;
pub mod storage;
pub mod time;

pub use randomness::{MockRandomnessProvider, RandomnessProvider, RealRandomnessProvider};
pub use storage::{MockStorageProvider, RealStorageProvider, StorageProvider, StorageTier};
pub use time::{MockTimeProvider, RealTimeProvider, TimeProvider};
