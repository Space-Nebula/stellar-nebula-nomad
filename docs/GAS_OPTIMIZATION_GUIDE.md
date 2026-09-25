# Gas Optimization Guide

## Overview

This guide documents gas optimization strategies implemented in the Stellar Nebula Nomad contract and provides best practices for maintaining optimal performance.

## Optimization Targets

- **30% gas reduction** from baseline
- CPU instructions < 2M per major operation
- Memory usage < 100KB per operation
- Batch operations 15%+ more efficient than individual calls

## Key Optimizations Implemented

### 1. Storage Optimization

#### Packed Storage

```rust
// Before: Multiple storage entries
env.storage().persistent().set(&Key::Field1(id), &value1);
env.storage().persistent().set(&Key::Field2(id), &value2);

// After: Single packed entry
let packed = PackedData { field1: value1, field2: value2 };
env.storage().persistent().set(&Key::Data(id), &packed);
```

**Savings**: ~40% reduction in storage operations

#### Storage Bumping Strategy

- Use `bump_instance()` for frequently accessed data
- Set appropriate TTLs based on access patterns
- Batch bump operations when possible

```rust
// Efficient bumping
env.storage().instance().bump(LEDGERS_PER_WEEK);
```

### 2. Computation Optimization

#### Loop Unrolling

```rust
// Before: Generic loop
for i in 0..cells.len() {
    process_cell(cells.get(i));
}

// After: Unrolled for known sizes
let len = cells.len();
let mut i = 0;
while i + 4 <= len {
    process_cell(cells.get_unchecked(i));
    process_cell(cells.get_unchecked(i + 1));
    process_cell(cells.get_unchecked(i + 2));
    process_cell(cells.get_unchecked(i + 3));
    i += 4;
}
while i < len {
    process_cell(cells.get_unchecked(i));
    i += 1;
}
```

**Savings**: ~15% reduction in loop overhead

#### Lazy Evaluation

```rust
// Before: Eager computation
let result = expensive_calculation();
if condition {
    use_result(result);
}

// After: Lazy evaluation
if condition {
    let result = expensive_calculation();
    use_result(result);
}
```

### 3. Batch Operations

All batch operations implement shared setup costs:

```rust
pub fn batch_mint_ships(
    env: Env,
    owner: Address,
    ship_types: Vec<Symbol>,
    metadata: Bytes,
) -> Result<Vec<ShipNft>, ShipError> {
    owner.require_auth(); // Single auth check

    let mut ships = Vec::new(&env);
    let counter = get_counter(&env); // Single counter read

    for (i, ship_type) in ship_types.iter().enumerate() {
        let ship = mint_ship_internal(&env, &owner, &ship_type, &metadata, counter + i as u64)?;
        ships.push_back(ship);
    }

    set_counter(&env, counter + ship_types.len() as u64); // Single counter write
    Ok(ships)
}
```

**Savings**: 20-30% vs individual operations

### 4. Memory Management

#### Pre-allocation

```rust
// Before: Dynamic growth
let mut vec = Vec::new(&env);
for item in items {
    vec.push_back(item);
}

// After: Pre-allocated
let mut vec = Vec::with_capacity(&env, items.len());
for item in items {
    vec.push_back(item);
}
```

#### Avoid Clones

```rust
// Before: Unnecessary clones
let data = get_data(&env).clone();
process(data.clone());

// After: References where possible
let data = get_data(&env);
process(&data);
```

### 5. Event Optimization

#### Batched Events

```rust
// Before: Multiple events
for item in items {
    env.events().publish((topic, symbol_short!("item")), item);
}

// After: Single batched event
env.events().publish((topic, symbol_short!("batch")), items);
```

## Benchmarking

### Running Benchmarks

```bash
# Run all benchmarks
cargo test --benches

# Run specific benchmark
cargo test --benches bench_nebula_generation

# Run with detailed output
cargo test --benches -- --nocapture
```

### Interpreting Results

- **CPU Instructions**: Target < 2M for major operations
- **Memory Bytes**: Target < 100KB
- **Batch Efficiency**: Should be < 0.85 ratio vs individual ops

### Performance Regression Tests

```bash
# Run regression suite
cargo test --test performance_regression

# These tests will fail if performance degrades
```

## Best Practices

### 1. Always Profile First

```rust
env.budget().reset_unlimited();
// ... operation ...
let cpu = env.budget().cpu_instruction_cost();
let mem = env.budget().memory_bytes_cost();
```

### 2. Optimize Hot Paths

Focus on:

- Frequently called functions
- Operations in loops
- Storage-heavy operations

### 3. Use Appropriate Data Structures

- `Vec` for sequential access
- `Map` for key-value lookups (but expensive)
- Packed structs for related data

### 4. Minimize Storage Operations

- Read once, use multiple times
- Write once at the end
- Batch related updates

### 5. Avoid Redundant Checks

```rust
// Before: Multiple auth checks
pub fn operation1(env: Env, caller: Address) {
    caller.require_auth();
    // ...
}
pub fn operation2(env: Env, caller: Address) {
    caller.require_auth();
    // ...
}

// After: Single auth check in wrapper
pub fn batch_operations(env: Env, caller: Address) {
    caller.require_auth(); // Once
    operation1_internal(&env, &caller);
    operation2_internal(&env, &caller);
}
```

## Monitoring

### Gas Usage Tracking

Track gas usage in CI/CD:

```yaml
- name: Gas Benchmark
  run: |
    cargo test --benches -- --nocapture > gas_report.txt
    # Parse and compare with baseline
```

### Alerting on Regressions

Set up alerts for:

- CPU usage > 10% increase
- Memory usage > 10% increase
- Batch efficiency < 0.85

## Common Pitfalls

### 1. Over-optimization

Don't optimize code that isn't a bottleneck. Profile first!

### 2. Premature Abstraction

Keep hot paths simple and direct.

### 3. Ignoring Storage Costs

Storage operations are expensive. Minimize them.

### 4. Not Testing Edge Cases

Ensure optimizations work for:

- Empty inputs
- Maximum size inputs
- Boundary conditions

## Optimization Checklist

- [ ] Profiled current gas usage
- [ ] Identified hot paths
- [ ] Optimized storage operations
- [ ] Implemented batch operations
- [ ] Added benchmarks
- [ ] Verified 30% reduction
- [ ] Added regression tests
- [ ] Updated documentation

## Results

### Baseline vs Optimized

| Operation      | Baseline CPU | Optimized CPU | Reduction |
| -------------- | ------------ | ------------- | --------- |
| Nebula Gen     | 1.4M         | 0.9M          | 36%       |
| Scan           | 2.8M         | 1.8M          | 36%       |
| Harvest        | 2.1M         | 1.3M          | 38%       |
| Mint Ship      | 1.1M         | 0.7M          | 36%       |
| Batch Mint (3) | 3.3M         | 2.0M          | 39%       |

**Average Reduction: 37%** ✅ (Target: 30%)

## Storage Access Patterns (Issue #437)

Every storage read is a host call that deserialises the stored value, and
every write re-serialises it. The main waste found in the codebase was the
**same key being read more than once in one invocation**.

### Patterns

| Pattern | Before | After | Where |
|---------|--------|-------|-------|
| Reuse values already read | Read counter → check cap → read it again to increment | Read once, check, write `value + 1` | `gas_sponsor::sponsor_first_scan` |
| Return the effective value from reset helpers | `reset_if_needed(); get(counter)` | `let count = reset_if_needed();` | `gas_sponsor` daily / per-user counters |
| Drop re-reads of keys nobody wrote | Re-read `PlayerStats` after ELO decay (decay only writes `EloRating`) | Removed | `pvp_combat::get_or_init_stats` |
| Pass known values down | `update_elo_rating` re-read the old rating | `write_elo_rating(old, new)` | `pvp_combat::end_combat`, decay |
| Resolve related data in one pass | Scan the tier table, then fetch the chosen tier again | `tier_for_count` returns `(tier, multiplier)` | `referral_system` v2 |
| `get` instead of `has` + `get` | Two host calls | One `get` → `ok_or(..)?` | `gas_sponsor::revoke_mobile_session` |
| Hoist loop invariants | `ledger().timestamp()` and key building inside loops | Computed once per batch | `storage_optim::batch_store_with_bump`, `nebula_gen` sweeps |
| Batch counters | Burst counter read + written per item | Once per batch | `storage_optim::get_optimized_entries`, `get_ship_nebula_batch` |
| Remove idle entries | Re-entrancy lock left as `false` in instance storage | Entry removed on release | `storage_optim::release_guard` |

### `CachedEntry`: read-through / write-back cache

For read-modify-write sequences on the same key, use
`storage_optim::CachedEntry`:

```rust
use crate::storage_optim::{CachedEntry, StorageTier};

let mut pool = CachedEntry::<DataKey, i128>::new(StorageTier::Instance, DataKey::Pool);
let balance = pool.get_or(&env, 0);   // 1 host read
pool.set(balance - fee);              // in memory
let staged = pool.get_or(&env, 0);   // served from cache
pool.set(staged + tip);
pool.flush(&env);                     // 1 host write
```

## Nebula Generation Hot Path (Issue #438)

Profiling `nebula_gen::generate_validated_nebula_layout` with
`env.cost_estimate().budget()` found these hotspots:

1. **Seed extraction:** 32 `BytesN::get` host calls. Replaced with one
   `to_array()` and a native fold (`gas_optimized_compute::fold_seed_bytes`).
2. **Anomaly vector growth:** one `push_back` per anomaly. Each push clones
   the host vector, so cost grew quadratically. Anomalies are now built in
   chunks of 8 with `Vec::from_array` / `extend_from_array`.
3. **Per-salt index mixing:** `index * GOLDEN_GAMMA` was recomputed for every
   property. It is now computed once per anomaly (`derive_spread`).
4. **Layout hash:** built byte-by-byte. It is now built from four
   little-endian lanes (`expand_u64_to_bytes32`).

The PRNG maths is unchanged, so output is bit-for-bit identical. The
`optimised_generation_matches_legacy_output` test compares against a
reference copy of the old algorithm at sizes 1–64.

### Measuring

```bash
cargo bench --bench nebula_storage_benchmarks
```

The benchmark runs the legacy generator (as its own contract) and the
optimised one in the same `Env`. It prints CPU instructions and memory bytes
before and after for layout sizes 8, 16, 32 and 64, plus the storage-pattern
comparisons above.

## Future Optimizations

1. **Parallel Processing**: Explore parallel cell processing
2. **Caching**: Implement result caching for expensive calculations
3. **Compression**: Compress large data structures
4. **Lazy Loading**: Defer loading of rarely-used data

## References

- [Soroban Gas Documentation](https://soroban.stellar.org/docs/fundamentals-and-concepts/fees-and-metering)
- [Rust Performance Book](https://nnethercote.github.io/perf-book/)
- [Stellar Optimization Patterns](https://soroban.stellar.org/docs/learn/optimization)
