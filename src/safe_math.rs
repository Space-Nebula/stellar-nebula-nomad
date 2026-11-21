//! Overflow-safe arithmetic primitives for economic calculations.
//!
//! The project policy (see `Cargo.toml` and `docs/YIELD_MATH.md`) is that
//! arithmetic must never silently wrap. `checked_*` helpers turn an overflow
//! into a catchable contract error, while `saturating_*` helpers clamp to the
//! representable range for values where aborting the whole transaction would
//! be disproportionate (e.g. a forecast or a display score).
//!
//! Every helper here is allocation-free and `no_std` compatible.
//!
//! Issue #239 / arithmetic-safety audit.

/// Multiply two signed values, then divide by `denom`, preserving the full
/// product's precision.
///
/// Returns `None` if the multiplication overflows, `denom` is zero, or the
/// final division overflows (only possible for `i128::MIN / -1`).
pub fn checked_mul_div_i128(value: i128, multiplier: i128, denom: i128) -> Option<i128> {
    if denom == 0 {
        return None;
    }
    value.checked_mul(multiplier)?.checked_div(denom)
}

/// Unsigned counterpart of [`checked_mul_div_i128`].
pub fn checked_mul_div_u64(value: u64, multiplier: u64, denom: u64) -> Option<u64> {
    if denom == 0 {
        return None;
    }
    value.checked_mul(multiplier)?.checked_div(denom)
}

/// Like [`checked_mul_div_i128`], but clamps instead of failing.
///
/// Intended for derived, non-state values such as yield forecasts and
/// confidence scores. Never use for balance or supply mutations.
pub fn saturating_mul_div_i128(value: i128, multiplier: i128, denom: i128) -> i128 {
    if denom == 0 {
        return 0;
    }
    value.saturating_mul(multiplier).checked_div(denom).unwrap_or(0)
}

/// Saturating addition for a value that must never wrap past its bounds
/// (e.g. a ledger sequence or a monotonic counter).
pub fn saturating_add_i128(a: i128, b: i128) -> i128 {
    a.saturating_add(b)
}

/// Saturating subtraction for reducing a balance-like value where clamping at
/// zero is preferable to aborting (callers should still validate inputs).
pub fn saturating_sub_i128(a: i128, b: i128) -> i128 {
    a.saturating_sub(b)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn checked_mul_div_detects_overflow() {
        assert_eq!(checked_mul_div_i128(i128::MAX, 2, 10), None);
    }

    #[test]
    fn checked_mul_div_rejects_zero_denominator() {
        assert_eq!(checked_mul_div_i128(100, 5, 0), None);
        assert_eq!(checked_mul_div_u64(100, 5, 0), None);
    }

    #[test]
    fn checked_mul_div_preserves_precision() {
        // Multiply before divide: 7 * 10 / 3 == 23, not (7/3)*10 == 20.
        assert_eq!(checked_mul_div_i128(7, 10, 3), Some(23));
        assert_eq!(checked_mul_div_u64(7, 10, 3), Some(23));
    }

    #[test]
    fn saturating_mul_div_clamps_instead_of_panicking() {
        let clamped = saturating_mul_div_i128(i128::MAX, 2, 10);
        assert!(clamped > 0);
        assert_eq!(saturating_mul_div_i128(100, 5, 0), 0);
    }

    #[test]
    fn saturating_add_and_sub_clamp_at_bounds() {
        assert_eq!(saturating_add_i128(i128::MAX, 1), i128::MAX);
        assert_eq!(saturating_sub_i128(i128::MIN, 1), i128::MIN);
    }
}
