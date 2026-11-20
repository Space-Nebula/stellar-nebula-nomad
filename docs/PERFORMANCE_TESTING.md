# Performance Testing Guide

## Overview

This document describes the performance regression testing framework for Stellar Nebula Nomad. Performance testing ensures that contract operations remain within CPU and memory budgets and that optimizations maintain their efficiency gains.

## Running Performance Tests

### Quick Start

Run all performance benchmarks:

```bash
cargo bench --all
```

Run a specific benchmark:

```bash
cargo bench --bench performance_regression
cargo bench --bench performance_metrics
cargo bench --bench gas_benchmarks
```

### Continuous Integration

Performance tests run automatically on:
- **Pull requests** to `main`
- **Pushes** to `main`
- **Daily schedule** (via size-regression.yml)

## Performance Metrics

### CPU Cost Baselines

| Operation | CPU Limit | Purpose |
|-----------|-----------|---------|
| `mint_ship` | 3,000,000 | Ship NFT creation |
| `generate_nebula_layout` | 6,000,000 | Procedural nebula generation |
| `scan_nebula` | 6,000,000 | Nebula region discovery |
| `harvest_resources` | 6,000,000 | Resource gathering from layouts |
| `update_progress` | 2,000,000 | Player progress updates |
| `found_alliance` | 4,000,000 | Alliance creation |
| `consume_energy` | 2,000,000 | Ship energy depletion |
| `recharge_energy` | 2,000,000 | Ship energy recovery |
| `pause_contract` | 2,000,000 | Emergency pause |

### Memory Limits

- **Max Memory per Operation**: 6,000,000 bytes
- Used to detect memory leaks and storage inefficiencies

## Benchmark Files

### `benches/performance_regression.rs`

Core performance gate that checks all critical paths:
- Establishes fixed ceilings for each operation
- Tests batch efficiency (batch mint vs single mints)
- Runs as part of `cargo bench --all`

### `benches/performance_metrics.rs`

Detailed metrics collection and reporting:
- Collects CPU and memory costs for all operations
- Generates formatted performance report
- Verifies all operations stay within limits
- Includes percentage-of-limit visualization

### `benches/gas_benchmarks.rs`

Gas cost analysis:
- Measures gas consumption per operation
- Tracks improvements/regressions in gas efficiency
- Used for testnet fee estimation

### `benches/nebula_storage_benchmarks.rs`

Storage-specific performance tests:
- Benchmarks storage access patterns
- Compares before/after optimization changes
- Tracks storage access latency

## Interpreting Results

### Success Output

```
performance_regression: no regressions
```

All operations completed within their CPU ceilings.

### Failure Output

```
REGRESSION: operation_name CPU 7000000 exceeds baseline 6000000
```

The operation exceeded its CPU limit. Investigate:
1. Recent code changes in that operation's path
2. Whether algorithmic changes are needed
3. If the baseline needs updating (requires justification)

## Optimization Guidelines

### When to Update Baselines

Only update baselines when:
1. ✅ Intentional algorithmic improvements reduce cost
2. ✅ Hardware/compiler changes increase baseline uniformly
3. ✅ Thorough testing confirms no behavioral change

Do NOT update baselines for:
1. ❌ Performance regressions from new features
2. ❌ Temporary workarounds (fix the root cause instead)
3. ❌ Unverified changes

### Optimization Strategies

1. **Algorithmic**: Reduce algorithm complexity (e.g., from O(n²) to O(n log n))
2. **Caching**: Memoize expensive computations
3. **Batching**: Group operations to reduce overhead
4. **Storage**: Optimize storage access patterns
5. **Vectorization**: Use SIMD where applicable

## Batch Efficiency

The framework includes a batch efficiency test:

```rust
fn regression_batch_efficiency() {
    // 3 ships individually vs 1 batch call
    // batch_cpu * 10 must be <= single_cpu * 3 * 11
}
```

This ensures batch operations maintain at least 10% efficiency gain over individual calls.

## CI/CD Integration

### Size Regression Workflow

Located in `.github/workflows/size-regression.yml`:
- Runs on PR and push to main
- Compares WASM binary size before/after
- Fails if binary grows unexpectedly

### Performance Thresholds

| Threshold | Action |
|-----------|--------|
| CPU within limit | ✅ Pass |
| CPU exceeds limit by <5% | ⚠️ Warning (investigate) |
| CPU exceeds limit by >5% | ❌ Fail (block merge) |
| Memory > 6MB | ❌ Fail (memory leak suspected) |

## Adding New Performance Tests

1. Create a new test function in `benches/performance_regression.rs`:

```rust
fn regression_new_operation() {
    let (env, client, player) = setup();
    let (cpu, mem) = measure(&env, || {
        let _ = client.new_operation(&player);
    });
    gate("new_operation", cpu, mem, MAX_CPU_NEW_OP);
}
```

2. Add operation ceiling constant:

```rust
const MAX_CPU_NEW_OP: u64 = 2_000_000;  // Adjust based on complexity
```

3. Call the test from `main()`:

```rust
fn main() {
    // ... existing tests ...
    regression_new_operation();
}
```

4. Update this doc with the new baseline

## Tools & Commands

### Local Analysis

Compare two branches:

```bash
# Branch A: main
cargo bench --bench performance_regression -- --baseline main

# Branch B: feature-branch
git checkout feature-branch
cargo bench --bench performance_regression
```

### Profiling

Identify hot spots with Rust profiling:

```bash
# Flamegraph (if installed)
cargo flamegraph --bench performance_regression
```

## Troubleshooting

### "REGRESSION: operation_name CPU exceeds baseline"

**Cause**: Operation took more CPU than expected

**Diagnosis**:
1. Check git diff for recent changes to that operation
2. Run locally to confirm
3. Profile the operation to find the hot spot

**Solutions**:
1. Optimize the code
2. If intentional, justify and update baseline
3. Consider algorithmic changes

### Inconsistent Results

**Cause**: Performance varies between runs (variability normal ±5%)

**Solutions**:
1. Run multiple times and average
2. Close other applications
3. Run on consistent hardware
4. Check for thermal throttling

## Related Documentation

- [Gas Optimization Guide](./GAS_OPTIMIZATION.md)
- [Storage Optimization](./STORAGE.md)
- [Benchmarking Best Practices](./BENCHMARKING.md)
