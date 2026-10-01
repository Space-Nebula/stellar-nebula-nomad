//! Randomness behind a trait.
//!
//! Battle resolution, loot rolls and seed mixing ask [`RandomnessProvider`]
//! for entropy. Production uses the host PRNG and the hybrid randomness
//! oracle; tests pin the outcome with [`MockRandomnessProvider`].

use core::cell::Cell;
use soroban_sdk::Env;

/// Source of random values.
pub trait RandomnessProvider {
    /// One uniformly distributed 64-bit value.
    fn random_u64(&self) -> u64;

    /// A 32-byte seed suitable for feeding a deterministic generator.
    fn random_seed(&self) -> [u8; 32];

    /// Uniform value in `lo..=hi`. Returns `lo` when the range is empty or
    /// inverted so callers never need to special-case degenerate bounds.
    fn random_in_range(&self, lo: u64, hi: u64) -> u64 {
        if hi <= lo {
            return lo;
        }
        let span = hi - lo + 1;
        lo + self.random_u64() % span
    }

    /// Fair coin flip.
    fn coin_flip(&self) -> bool {
        self.random_u64() & 1 == 1
    }
}

/// [`RandomnessProvider`] backed by the Soroban host PRNG and the
/// [`crate::randomness_oracle`] seed mixer. The production default.
pub struct RealRandomnessProvider<'a> {
    env: &'a Env,
}

impl<'a> RealRandomnessProvider<'a> {
    /// Wrap the contract environment.
    pub fn new(env: &'a Env) -> Self {
        Self { env }
    }
}

impl RandomnessProvider for RealRandomnessProvider<'_> {
    fn random_u64(&self) -> u64 {
        self.env.prng().gen::<u64>()
    }

    fn random_seed(&self) -> [u8; 32] {
        crate::randomness_oracle::request_random_seed(self.env).to_array()
    }
}

/// Deterministic generator for tests.
///
/// Either a seeded xorshift64* stream (`new`) or a pinned constant (`fixed`)
/// so a test can force the "won" and "lost" branches of a roll explicitly.
#[derive(Debug)]
pub struct MockRandomnessProvider {
    state: Cell<u64>,
    fixed: Cell<Option<u64>>,
}

impl MockRandomnessProvider {
    /// Seeded stream; the same seed always yields the same sequence.
    pub fn new(seed: u64) -> Self {
        Self {
            // xorshift must never start from zero.
            state: Cell::new(if seed == 0 {
                0x9E37_79B9_7F4A_7C15
            } else {
                seed
            }),
            fixed: Cell::new(None),
        }
    }

    /// Every call returns `value`.
    pub fn fixed(value: u64) -> Self {
        Self {
            state: Cell::new(1),
            fixed: Cell::new(Some(value)),
        }
    }

    /// Switch a stream provider into fixed mode (or back with `None`).
    pub fn pin(&self, value: Option<u64>) {
        self.fixed.set(value);
    }
}

impl RandomnessProvider for MockRandomnessProvider {
    fn random_u64(&self) -> u64 {
        if let Some(v) = self.fixed.get() {
            return v;
        }
        // xorshift64* (Vigna): full period, trivially reproducible.
        let mut x = self.state.get();
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.state.set(x);
        x.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    fn random_seed(&self) -> [u8; 32] {
        let mut out = [0u8; 32];
        for chunk in out.chunks_mut(8) {
            chunk.copy_from_slice(&self.random_u64().to_be_bytes());
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn seeded_stream_is_reproducible_and_not_constant() {
        let a = MockRandomnessProvider::new(7);
        let b = MockRandomnessProvider::new(7);
        let xs: [u64; 4] = core::array::from_fn(|_| a.random_u64());
        let ys: [u64; 4] = core::array::from_fn(|_| b.random_u64());
        assert_eq!(xs, ys);
        assert!(xs[0] != xs[1] || xs[1] != xs[2]);
    }

    #[test]
    fn fixed_provider_pins_every_roll() {
        let r = MockRandomnessProvider::fixed(3);
        assert_eq!(r.random_u64(), 3);
        assert_eq!(r.random_in_range(10, 20), 13);
        assert!(r.coin_flip());
        r.pin(Some(2));
        assert!(!r.coin_flip());
    }

    #[test]
    fn range_handles_degenerate_bounds() {
        let r = MockRandomnessProvider::new(1);
        assert_eq!(r.random_in_range(5, 5), 5);
        assert_eq!(r.random_in_range(9, 4), 9);
        for _ in 0..100 {
            let v = r.random_in_range(3, 6);
            assert!((3..=6).contains(&v));
        }
    }

    #[test]
    fn zero_seed_is_remapped() {
        let r = MockRandomnessProvider::new(0);
        assert_ne!(r.random_u64(), 0);
    }

    #[test]
    fn real_provider_produces_entropy_inside_a_contract_frame() {
        let env = Env::default();
        let id = env.register(crate::NebulaNomadContract, ());
        env.as_contract(&id, || {
            let r = RealRandomnessProvider::new(&env);
            let seed = r.random_seed();
            assert!(seed.iter().any(|b| *b != 0));
            let a = r.random_u64();
            let b = r.random_u64();
            assert!(a != b || r.random_u64() != a);
            let v = r.random_in_range(1, 6);
            assert!((1..=6).contains(&v));
        });
    }
}
