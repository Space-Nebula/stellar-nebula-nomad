//! Ledger time behind a trait.
//!
//! Every daily window, lock period and yield accrual in the game reads "now"
//! through [`TimeProvider`] so tests can move the clock without touching the
//! ledger.

use core::cell::Cell;
use soroban_sdk::Env;

/// Seconds in one day; the granularity of every daily window in the game.
pub const SECONDS_PER_DAY: u64 = 86_400;

/// Nominal Stellar ledger close time, used by the mock to keep the ledger
/// sequence roughly consistent with the timestamp it advances.
pub const SECONDS_PER_LEDGER: u64 = 5;

/// Source of the current ledger time.
pub trait TimeProvider {
    /// Unix timestamp (seconds) of the current ledger.
    fn current_timestamp(&self) -> u64;

    /// Sequence number of the current ledger.
    fn current_ledger(&self) -> u32;

    /// Day index used by daily windows: `timestamp / 86_400`.
    fn day_index(&self) -> u64 {
        self.current_timestamp() / SECONDS_PER_DAY
    }

    /// Seconds elapsed since `since`, saturating at zero when `since` is in
    /// the future (a clock that appears to run backwards must never produce
    /// negative accrual).
    fn elapsed_since(&self, since: u64) -> u64 {
        self.current_timestamp().saturating_sub(since)
    }
}

/// [`TimeProvider`] backed by the Soroban ledger. The production default.
pub struct RealTimeProvider<'a> {
    env: &'a Env,
}

impl<'a> RealTimeProvider<'a> {
    /// Wrap the contract environment.
    pub fn new(env: &'a Env) -> Self {
        Self { env }
    }
}

impl TimeProvider for RealTimeProvider<'_> {
    fn current_timestamp(&self) -> u64 {
        self.env.ledger().timestamp()
    }

    fn current_ledger(&self) -> u32 {
        self.env.ledger().sequence()
    }
}

/// Controllable clock for tests.
///
/// Uses interior mutability so a shared `&MockTimeProvider` can be handed to
/// the code under test while the test keeps advancing it.
#[derive(Debug)]
pub struct MockTimeProvider {
    timestamp: Cell<u64>,
    ledger: Cell<u32>,
}

impl MockTimeProvider {
    /// Start the clock at the given timestamp and ledger sequence.
    pub fn new(timestamp: u64, ledger: u32) -> Self {
        Self {
            timestamp: Cell::new(timestamp),
            ledger: Cell::new(ledger),
        }
    }

    /// Jump the clock to an absolute timestamp.
    pub fn set_timestamp(&self, timestamp: u64) {
        self.timestamp.set(timestamp);
    }

    /// Jump the clock to an absolute ledger sequence.
    pub fn set_ledger(&self, ledger: u32) {
        self.ledger.set(ledger);
    }

    /// Move the clock forward by `secs`, advancing the ledger sequence at the
    /// nominal close rate so both views stay coherent.
    pub fn advance_secs(&self, secs: u64) {
        self.timestamp
            .set(self.timestamp.get().saturating_add(secs));
        let ledgers = u32::try_from(secs / SECONDS_PER_LEDGER).unwrap_or(u32::MAX);
        self.ledger.set(self.ledger.get().saturating_add(ledgers));
    }

    /// Move the clock forward by whole days.
    pub fn advance_days(&self, days: u64) {
        self.advance_secs(days.saturating_mul(SECONDS_PER_DAY));
    }

    /// Advance only the ledger sequence.
    pub fn advance_ledgers(&self, ledgers: u32) {
        self.ledger.set(self.ledger.get().saturating_add(ledgers));
    }
}

impl Default for MockTimeProvider {
    fn default() -> Self {
        Self::new(1_700_000_000, 100)
    }
}

impl TimeProvider for MockTimeProvider {
    fn current_timestamp(&self) -> u64 {
        self.timestamp.get()
    }

    fn current_ledger(&self) -> u32 {
        self.ledger.get()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use soroban_sdk::testutils::Ledger;

    #[test]
    fn mock_clock_advances_and_derives_day_index() {
        let clock = MockTimeProvider::new(0, 1);
        assert_eq!(clock.day_index(), 0);
        clock.advance_days(3);
        assert_eq!(clock.current_timestamp(), 3 * SECONDS_PER_DAY);
        assert_eq!(clock.day_index(), 3);
        assert_eq!(clock.current_ledger(), 1 + (3 * SECONDS_PER_DAY / 5) as u32);
    }

    #[test]
    fn elapsed_since_never_goes_negative() {
        let clock = MockTimeProvider::new(100, 1);
        assert_eq!(clock.elapsed_since(40), 60);
        assert_eq!(clock.elapsed_since(500), 0);
    }

    #[test]
    fn real_provider_reads_the_ledger() {
        let env = Env::default();
        env.ledger().with_mut(|li| {
            li.timestamp = 2 * SECONDS_PER_DAY + 7;
            li.sequence_number = 42;
        });
        let clock = RealTimeProvider::new(&env);
        assert_eq!(clock.current_timestamp(), 2 * SECONDS_PER_DAY + 7);
        assert_eq!(clock.current_ledger(), 42);
        assert_eq!(clock.day_index(), 2);
    }
}
