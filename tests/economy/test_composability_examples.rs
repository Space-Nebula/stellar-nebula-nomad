#![cfg(test)]
//! Composability helpers (Issue #86): response validation rejects empty
//! return data and accepts any non-empty payload.

use soroban_sdk::{BytesN, Env};
use stellar_nebula_nomad::{validate_composable_response, ComposabilityError};

#[test]
fn all_zero_response_is_rejected() {
    let env = Env::default();
    let empty = BytesN::from_array(&env, &[0u8; 64]);

    assert_eq!(
        validate_composable_response(&env, &empty),
        Err(ComposabilityError::InvalidResponse)
    );
}

#[test]
fn non_empty_response_is_accepted() {
    let env = Env::default();
    let payload = BytesN::from_array(&env, &[7u8; 64]);

    assert_eq!(validate_composable_response(&env, &payload), Ok(true));
}

#[test]
fn single_non_zero_byte_is_enough() {
    let env = Env::default();
    let mut bytes = [0u8; 64];
    bytes[63] = 1;
    let payload = BytesN::from_array(&env, &bytes);

    assert_eq!(validate_composable_response(&env, &payload), Ok(true));
}
