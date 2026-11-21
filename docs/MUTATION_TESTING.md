# Mutation Testing with `cargo-mutants`

## Overview

Mutation testing evaluates test suite quality by intentionally mutating program source code (replacing binary operations, replacing return values with defaults, omitting statements) and verifying whether automated tests catch and fail on the mutation ("kill the mutant").

Surviving mutants indicate gaps in boundary checks, error conditions, or unasserted return states.

---

## Priority Modules

Mutation analysis focuses on core smart contract systems:
1. `src/access_control.rs`: Role hierarchy, emergency privileges, delegations, and time-lock mechanisms.
2. `src/batch_processor.rs`: Bounded batch execution, gas calculation, and atomicity invariants.
3. `src/nebula_gen.rs`: Procedural determinism, coordinate bounds, and anomaly generations.
4. `src/ship_registry.rs`: Ship minting, stats calculation, and registry lookups.
5. `src/nomad_bonding.rs`: Curve pricing, bonding collateral, and slippage protections.

---

## Running Locally

To run mutation testing on specific modules:

```bash
cargo install cargo-mutants
chmod +x ./scripts/mutation-test.sh
./scripts/mutation-test.sh
```

Or targeting a single module:

```bash
cargo mutants --file src/batch_processor.rs --output mutants.out
```

---

## CI Integration

Mutation testing runs in GitHub Actions via:
- `.github/workflows/mutation.yml` (on PR and push to main)
- `.github/workflows/mutation-testing.yml` (on contract/test changes)

Results are published to `$GITHUB_STEP_SUMMARY` and uploaded as an artifact (`mutants-report`).

---

## Target Mutation Score

- **Target Threshold**: `> 80%` mutants killed across prioritized modules.
- **Justified Surviving Mutants**:
  - Non-essential debug assertions and log format strings.
  - Redundant saturating math operations on values strictly bounded by earlier checks.
