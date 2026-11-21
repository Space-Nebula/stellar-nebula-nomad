# Dead-code audit

The audit searches production Rust sources for explicit dead-code suppressions,
unreferenced public helpers, stale imports, and duplicate test utilities.

## Initial Audit Pass
- `dex_integration::get_offer`, which had no caller or root-contract entry point;
- `treasure_vault::set_min_lock_duration`, which had no caller and bypassed
  the authorization expected by its “admin function” comment.

## Comprehensive Dead-Code & Unused Feature Audit (#658)

Audited entire codebase for:
- **Unused functions & helpers**: Identified and resolved unreferenced internal functions.
- **Dead-code attributes**: Removed top-level `#![allow(dead_code)]` suppression from `src/gas_optimized_compute.rs`.
- **Commented-out code**: Cleaned obsolete commented code fragments across modules and modularized submodules.
- **Unreachable code paths**: Audited error handling matching branches to ensure every variant is reachable and mapped.
- **Test helper retention**: Verified that `src/test_helpers.rs` and related mocking utilities are properly guarded by `#[cfg(any(test, feature = "fuzz"))]` so they do not pollute WASM production builds.

Recommended repeatable checks:

```sh
cargo clippy --all-targets --all-features -- -D warnings
rg 'allow\(dead_code\)' src
cargo machete
```

`cargo machete` audits unused Cargo dependencies and should be run whenever
dependencies change.
