# Testing Overview

## Quick Reference

| Test Type | Command | Purpose | Coverage |
|-----------|---------|---------|----------|
| **Unit Tests** | `cargo test --lib` | Individual module functions | 85%+ |
| **Integration Tests** | `cargo test --test test_` | Cross-module scenarios | End-to-end flows |
| **Benchmarks** | `cargo bench` | Performance regression detection | Critical paths |
| **Performance Metrics** | `cargo bench --bench performance_metrics` | Detailed metrics collection | All operations |
| **Security Tests** | `cargo test --test security_` | Vulnerability and attack detection | OWASP Top 10 |
| **Upgrade Tests** | `cargo test --test upgrade_` | Contract upgrade safety | Migration & rollback |
| **Visual Regression** | `npm run test:visual` | UI change detection | Frontend components |
| **Property-Based** | `cargo test --test *_properties` | Invariant verification | Edge cases |
| **Load Tests** | See `tests/load/` | Performance under load | Stress testing |
| **Fuzzing** | `cargo test --test fuzz` | Crash detection | Unexpected inputs |

## Test Pyramid

```
              👁️ Visual Regression (1%)
           🔒 Security Tests (5%)
         🚀 Performance Benchmarks (5%)
      🔄 Property-Based Tests (10%)
   📋 Integration Tests (20%)
 ✅ Unit Tests (60%)
```

## Full Testing Guide by Category

### Smart Contract Testing

#### Unit Tests
- **Location**: `src/*/mod.rs` (`#[cfg(test)]` sections)
- **Command**: `cargo test --lib`
- **Purpose**: Test individual functions and modules
- **Coverage**: Function logic, error paths, edge cases

**Example**:
```rust
#[cfg(test)]
mod tests {
    #[test]
    fn test_ship_level_increases() {
        // Test logic
    }
}
```

#### Integration Tests
- **Location**: `tests/*/` directory
- **Command**: `cargo test --test <name>`
- **Purpose**: Test cross-module flows through contract client
- **Documentation**: [Test Organization](../tests/README.md)

**Organization**:
- `tests/economy/` - Staking, yield, DEX, gas sponsorship
- `tests/gameplay/` - Ships, seasons, exploration
- `tests/governance/` - DAO, RBAC, emergency controls
- `tests/infra/` - Storage, snapshots, disaster recovery
- `tests/integration/` - E2E cross-contract flows
- `tests/property/` - Property-based invariant testing
- `tests/chaos/` - Chaos engineering scenarios
- `tests/fuzz/` - Fuzz testing
- `tests/security/` - Security vulnerability testing
- `tests/upgrades/` - Contract upgrade testing

### Performance Testing

**Guide**: [Performance Testing Guide](./PERFORMANCE_TESTING.md)

#### Benchmarks
```bash
# All benchmarks
cargo bench --all

# Specific benchmark
cargo bench --bench performance_regression
cargo bench --bench gas_benchmarks
```

#### Performance Metrics Collection
```bash
cargo bench --bench performance_metrics
```

Provides detailed report of:
- CPU costs for each operation
- Memory usage
- Percentage of limit used
- Performance status (pass/fail)

#### Size Regression
- **Workflow**: `.github/workflows/size-regression.yml`
- **Trigger**: Every PR and push to main
- **Action**: Detects unexpected WASM binary growth

### Security Testing

**Guide**: [Security Testing Guide](./SECURITY_TESTING.md)

#### Automated Security Checks

```bash
# Dependency vulnerabilities
cargo audit

# License compliance
cargo deny check

# Secret scanning
gitleaks

# All security tests
cargo test --test security_
```

#### Security Test Suites

1. **Access Control Tests** (`tests/security/access_control_tests.rs`)
   - Authorization verification
   - Admin role enforcement
   - Ownership checks
   - RBAC validation

2. **Input Validation Tests** (`tests/security/input_validation_tests.rs`)
   - String length limits
   - Numeric bounds
   - Type validation
   - Array size limits

3. **Rate Limiting Tests** (`tests/security/rate_limiting_tests.rs`)
   - Operation frequency limits
   - Quota enforcement
   - Bypass prevention

4. **Storage Security Tests** (`tests/security/storage_key_collisions.rs`)
   - Key collision prevention
   - Data isolation
   - Unauthorized access prevention

5. **Penetration Tests** (`tests/security/penetration_tests.rs`)
   - Reentrancy attacks
   - Integer overflow/underflow
   - Authorization bypass attempts
   - Data leak prevention
   - Resource exhaustion

#### CI/CD Security

- **Workflow**: `.github/workflows/security.yml`
- **Triggers**: Every PR, push to main, daily schedule
- **Checks**: Audit, dependency review, secrets, licenses, SBOM

### Contract Upgrade Testing

**Guide**: [Contract Upgrade Testing Guide](./CONTRACT_UPGRADE_TESTING.md)

#### Upgrade Test Suites

```bash
# All upgrade tests
cargo test --test upgrade_

# Specific suites
cargo test --test upgrade_migration
cargo test --test upgrade_state_preservation
cargo test --test upgrade_backward_compatibility
cargo test --test upgrade_rollback
```

1. **Migration Tests** - Data migration scenarios
2. **State Preservation Tests** - Critical invariant preservation
3. **Backward Compatibility Tests** - Old API still works
4. **Rollback Tests** - Emergency downgrade safety

### Visual Regression Testing

**Guide**: [Visual Regression Testing Guide](./VISUAL_REGRESSION_TESTING.md)

#### Local Testing

```bash
cd frontend
npm install
npm run build-storybook
npm run test:visual
```

#### Baseline Management

```bash
# Update baselines (after intentional UI changes)
npm run test:visual:baseline

# Review what changed
git diff frontend/visual-regression-baselines/
```

#### CI/CD Integration

- **Workflow**: `.github/workflows/visual-regression.yml`
- **Triggers**: Changes to `frontend/src/**` or Storybook config
- **Actions**: Builds Storybook, captures screenshots, compares with baseline
- **Viewports**: Desktop (1280x720), Tablet (768x1024), Mobile (375x812)

### Property-Based Testing

**Location**: `tests/property/`

```bash
cargo test --test *_properties
```

Uses `proptest` to verify invariants hold for all inputs:

- **nebula_gen_properties** - Nebula generation bounds and anomaly placement
- **resource_minter_properties** - Ledger conservation laws
- **economic_properties** - Price caps and economic bounds

### Load Testing

**Location**: `tests/load/`

K6 load test scripts for performance under stress:

```bash
k6 run tests/load/baseline.js
```

### Fuzzing

**Location**: `tests/fuzz/`

```bash
cargo test --test fuzz --features fuzz
```

Finds crashes with randomly generated inputs.

## Running Tests

### Local Development

```bash
# Quick check (all tests)
cargo test

# Fast subset (unit tests only)
cargo test --lib

# Verbose output
cargo test -- --nocapture

# Single-threaded (better debugging)
cargo test -- --test-threads=1

# Specific test
cargo test test_name

# Pattern matching
cargo test security_
```

### Full Local Audit

```bash
# Run everything
./scripts/test-all.sh

# Or manually:
cargo test --all                    # Unit + integration
cargo bench --all                   # Benchmarks
cargo test --test security_         # Security
cargo test --test upgrade_          # Upgrades
cargo audit                         # Dependencies
cargo deny check                    # Licenses
gitleaks                            # Secrets
cd frontend && npm run test:visual  # Visual tests
```

### CI/CD Runs

**On Pull Requests**:
1. Build (native + WASM)
2. Lint (clippy, rustfmt)
3. Unit tests
4. Integration tests
5. Security checks
6. Size regression
7. Visual regression (frontend)

**On Push to Main**:
1. All PR checks
2. Benchmarks (CPU/memory)
3. Performance metrics collection
4. Flamegraph generation
5. Artifact storage

**Daily Schedule**:
1. Cargo audit
2. Dependency review
3. SBOM generation

## Test Result Interpretation

### ✅ Success

```
test result: ok. 547 passed; 0 failed; 2 ignored
```

All tests passed. Ready for merge.

### ⚠️ Warning

```
test result: ok. 547 passed; 2 ignored
```

Some tests ignored. Check if intentional.

### ❌ Failure - Logic Error

```
thread 'test_name' panicked at 'assertion failed: 5 == 10'
```

Test failed due to assertion. Check test expectations.

### ❌ Failure - Regression

```
REGRESSION: mint_ship CPU 7000000 exceeds baseline 6000000
```

Performance regression detected. Optimize code or update baseline.

### ❌ Failure - Security Issue

```
✗ Visual regression detected
  2 differences found in Components/Button
```

UI changes detected. Review and approve or revert CSS changes.

## Best Practices

### Writing Tests

- ✓ Test behavior, not implementation
- ✓ Use descriptive test names
- ✓ Test happy path AND error cases
- ✓ Isolate tests (no dependencies between them)
- ✓ Use fixtures for common setup
- ✓ Comment why, not what
- ✓ Keep tests focused and small

### Running Tests

- ✓ Run locally before pushing
- ✓ Run full suite before PR
- ✓ Check CI results before merging
- ✓ Run upgrade tests before deployment
- ✓ Monitor performance metrics trend

### Maintaining Tests

- ✓ Update tests when requirements change
- ✓ Fix flaky tests immediately
- ✓ Remove obsolete tests
- ✓ Review test coverage regularly
- ✓ Refactor duplicate test code

## Coverage Goals

| Category | Target | Current |
|----------|--------|---------|
| Smart Contract Code | 85%+ | [Check coverage] |
| Critical Paths | 100% | [Check coverage] |
| Error Paths | 95%+ | [Check coverage] |
| Security Tests | 100% | [Check coverage] |
| Integration Flows | 100% | [Check coverage] |

## Test Naming Conventions

```
test_{feature}_{scenario}_{expected_result}
test_ship_repair_with_zero_health_fails
test_resource_mint_preserves_total_supply
test_alliance_treasury_audit_log_created
```

## Related Documentation

- [Performance Testing](./PERFORMANCE_TESTING.md)
- [Security Testing](./SECURITY_TESTING.md)
- [Contract Upgrade Testing](./CONTRACT_UPGRADE_TESTING.md)
- [Visual Regression Testing](./VISUAL_REGRESSION_TESTING.md)
- [Test Organization](../tests/README.md)

## Tools

- **Rust Testing**: Built-in `cargo test`
- **Benchmarking**: `cargo bench` with custom harness
- **Property Testing**: `proptest` crate
- **Load Testing**: `k6` framework
- **Fuzzing**: `cargo fuzz` / `honggfuzz`
- **Visual Testing**: Playwright + Storybook
- **Security Scanning**: `cargo-audit`, `gitleaks`, `cargo-deny`

## Performance Optimization

For tests running slow:

```bash
# Parallel execution (default)
cargo test

# Single-threaded (slower but deterministic)
cargo test -- --test-threads=1

# Skip expensive tests locally
cargo test --lib  # Only unit tests (fast)

# Skip in CI
# (Add #[ignore] to test, include in CI)
#[test]
#[ignore]
fn expensive_test() { }
```

## Troubleshooting

### Test Hangs

```bash
# Timeout setting (seconds)
timeout 300 cargo test test_name
```

### Flaky Tests

```bash
# Run multiple times
for i in {1..10}; do cargo test test_name || break; done
```

### Memory Issues

```bash
# Limit parallel tests
cargo test -- --test-threads=1
```

### Coverage Reports

```bash
# Generate LCOV report (requires setup)
cargo tarpaulin --out Html
```

## Contributing Tests

1. Write test alongside code (TDD preferred)
2. Add documentation for complex tests
3. Run full suite locally: `cargo test --all`
4. Submit PR with test included
5. CI runs all checks automatically
6. Address any test failures

---

**Last Updated**: 2026-11-20  
**Test Count**: 547 unit + 89 integration + 24 security + 20 upgrade + 15 property + 5 benchmark suites
