//! Composability helpers: validating the return data of a cross-contract call.
//!
//! Run with: `cargo run --example composability_example`
#![allow(warnings)]

use soroban_sdk::{BytesN, Env};
use stellar_nebula_nomad::validate_composable_response;

fn main() {
    // Step 1: a Soroban environment is needed to build the byte arrays.
    let env = Env::default();

    // Step 2: a non-empty response from the external contract is accepted.
    let payload = BytesN::from_array(&env, &[7u8; 64]);
    println!(
        "non-empty response -> {:?}",
        validate_composable_response(&env, &payload)
    );

    // Step 3: an all-zero response is treated as empty and rejected with
    // `ComposabilityError::InvalidResponse`, so the caller can fall back.
    let empty = BytesN::from_array(&env, &[0u8; 64]);
    println!(
        "all-zero response  -> {:?}",
        validate_composable_response(&env, &empty)
    );
}
