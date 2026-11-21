# Security Testing Guide

## Overview

This document describes the comprehensive security testing framework for Stellar Nebula Nomad. The framework includes penetration testing, access control verification, input validation, and security vulnerability detection.

## Security Testing Layers

### Layer 1: Static Analysis

Automated security checks run on every commit:

```bash
cargo audit        # Dependency vulnerability scanning
cargo deny check   # License compliance
gitleaks           # Secret scanning
```

### Layer 2: Access Control Testing

Verifies authorization patterns:

```bash
cargo test --test security_access_control
```

Tests:
- Signature requirements on all state-changing operations
- Admin-only operation enforcement
- Ownership verification for NFTs and resources
- Guild membership validation
- Role-based access control (RBAC)

### Layer 3: Input Validation Testing

Ensures all inputs are properly validated:

```bash
cargo test --test security_input_validation
```

Tests:
- Seed parameter validation for nebula generation
- Ship type validation
- String length limits for alliance names
- Numeric boundary checks
- Array/Vec size limits

### Layer 4: Rate Limiting Testing

Prevents abuse through rate limiting:

```bash
cargo test --test security_rate_limiting
```

Tests:
- Request frequency limiting
- Operation throttling
- Quota enforcement
- Reset timing

### Layer 5: Storage Security Testing

Verifies storage key safety:

```bash
cargo test --test security_storage_keys
```

Tests:
- Storage key collision prevention
- Data isolation between accounts
- Unauthorized state access prevention

### Layer 6: Penetration Testing

Comprehensive attack vector simulation:

```bash
cargo test --test security_penetration_tests
```

Tests:
- Reentrancy attacks
- Integer overflow/underflow
- Authorization bypass attempts
- Data leak prevention
- Resource exhaustion scenarios
- Cryptographic security

## Running Security Tests

### Quick Check

```bash
# All security tests
cargo test --test security_

# Specific test suite
cargo test --test security_access_control
cargo test --test security_penetration_tests
```

### Complete Security Audit

```bash
# Run all security checks
cargo audit
cargo deny check
cargo test --test security_
gitleaks --source . --max-depth 3
```

### In CI/CD

The `.github/workflows/security.yml` workflow:

1. **Cargo Audit** - CVE scanning on every push
2. **Dependency Review** - New dependency security on PRs
3. **Secret Scan** - Gitleaks scanning
4. **License Compliance** - Cargo deny checks
5. **SBOM Generation** - Software Bill of Materials

## Security Test Suites

### Access Control Tests (`tests/security/access_control_tests.rs`)

**Purpose**: Verify authorization enforcement

**Coverage**:
- Signature requirement verification
- Admin role enforcement
- Ownership checks for ships and NFTs
- Alliance membership validation
- PvP operation authorization
- Tournament organizer verification

**Example**:

```rust
#[test]
fn test_admin_only_pause_requires_auth() {
    let (env, id) = setup();
    env.mock_auths(&[]);  // Clear mocks - require real auth
    
    let admin = Address::generate(&env);
    at(&env, &id, || {
        client.pause_contract(&admin);  // Must fail without auth
    });
}
```

### Input Validation Tests (`tests/security/input_validation_tests.rs`)

**Purpose**: Verify input sanitization

**Coverage**:
- Seed validation (cryptographic requirements)
- String length limits
- Numeric bounds checking
- Array size limits
- Type validation

**Example**:

```rust
#[test]
fn test_string_length_limits() {
    let name = String::from_str(&env, &"x".repeat(10000));  // Should reject
    // Expected: validation error
}
```

### Rate Limiting Tests (`tests/security/rate_limiting_tests.rs`)

**Purpose**: Verify request throttling

**Coverage**:
- Operation frequency limits
- Account-level quotas
- Reset timing verification
- Bypass prevention

### Storage Security Tests (`tests/security/storage_key_collisions.rs`)

**Purpose**: Verify storage key isolation

**Coverage**:
- Key collision detection
- Data isolation verification
- Unauthorized access prevention
- Storage entropy validation

### Penetration Tests (`tests/security/penetration_tests.rs`)

**Purpose**: Simulate real attacks

**Coverage**:

#### Reentrancy Attacks
```rust
#[test]
fn test_no_reentrancy_on_harvest() {
    // Verify harvest can't be reentered during execution
}
```

#### Integer Overflow/Underflow
```rust
#[test]
fn test_resource_balance_no_overflow() {
    // Verify math operations don't overflow
}
```

#### Authorization Bypass
```rust
#[test]
fn test_non_owner_cannot_operate_ship() {
    // Verify ownership checks work
}
```

#### Data Leaks
```rust
#[test]
fn test_private_data_not_exposed() {
    // Verify sensitive data access is controlled
}
```

#### Resource Exhaustion
```rust
#[test]
fn test_rapid_mint_no_bypass() {
    // Verify rate limiting prevents abuse
}
```

## Common Vulnerabilities Tested

### OWASP Top 10 (Smart Contracts)

| Vulnerability | Test Location | Mitigation |
|---------------|---------------|-----------|
| Reentrancy | `penetration_tests.rs` | Checks-effects-interactions pattern |
| Integer Overflow | `penetration_tests.rs` | Checked arithmetic |
| Unchecked Call | `penetration_tests.rs` | Error propagation |
| Delegatecall | N/A (Soroban) | Soroban doesn't have delegatecall |
| Front-running | N/A (Soroban) | Soroban atomic transactions |
| Time Dependency | `penetration_tests.rs` | Deterministic operations |
| Short Address | N/A (Soroban) | Type-safe |
| Unhandled Exception | `penetration_tests.rs` | Proper error handling |
| Bad Randomness | `penetration_tests.rs` | Oracle-based randomness |
| Access Control | `access_control_tests.rs` | RBAC enforcement |

## Writing New Security Tests

### Template

```rust
#[test]
fn test_security_vulnerability_name() {
    // Setup
    let (env, client, attacker) = setup();
    
    // Execute attack
    let result = env.as_contract(&contract_id, || {
        client.operation(&attacker, &malicious_param)
    });
    
    // Assert protection
    assert!(matches!(result, Err(SecurityError)));
}
```

### Guidelines

1. **Clear naming**: Test name should describe the vulnerability
2. **Setup isolation**: Each test independent from others
3. **Realistic attack**: Simulate actual exploit technique
4. **Correct assertions**: Verify defense worked
5. **Documentation**: Comment on why this attack matters

## CI/CD Security Workflow

### On Every PR

```
Submission
  ├─ cargo audit (dependency vulnerabilities)
  ├─ cargo deny (license/source checks)
  ├─ gitleaks (secrets scanning)
  └─ [Block if failures]
```

### On Every Commit

```
Push to any branch
  ├─ Cargo audit (daily schedule also runs)
  ├─ Artifact upload (audit report)
  └─ Email on failure
```

### Manual Security Audit

For major releases:

```bash
# Run penetration tests
cargo test --test security_penetration_tests -- --nocapture

# Check for new CVEs
cargo audit

# Scan for secrets
gitleaks

# Review generated SBOM
cat sbom.json | jq '.components[] | .name, .version'
```

## Security Best Practices

### Code Review

- ✓ Review authorization logic carefully
- ✓ Check all external calls for error handling
- ✓ Verify input validation
- ✓ Check for arithmetic overflow
- ✓ Verify state consistency after operations

### Testing

- ✓ Always test both success and failure paths
- ✓ Test with invalid inputs
- ✓ Test with unauthorized callers
- ✓ Test boundary conditions
- ✓ Test state transitions

### Deployment

- ✓ Run full security audit before deployment
- ✓ Check SBOM for known vulnerabilities
- ✓ Review all recent code changes
- ✓ Monitor contract post-deployment
- ✓ Have emergency pause procedure ready

## Responding to Security Issues

### Discovery

If you discover a security vulnerability:

1. **Do NOT** commit to main branch
2. Create a private security advisory
3. Email security@stellar-nebula-nomad.dev
4. Provide:
   - Vulnerability description
   - Proof of concept
   - Affected components
   - Suggested fix

### Remediation

1. Create fix on private branch
2. Write test case verifying protection
3. Have security team review
4. Deploy to testnet
5. Monitor testnet for 1 week
6. Deploy to mainnet
7. Post security advisory

## Security Advisories

Previous advisories and fixes:

- [CVE-XXXX-XXXXX](./SECURITY_ADVISORIES.md) - Issue title

## Tools & Resources

### Security Tools

- `cargo audit` - Dependency vulnerability scanner
- `gitleaks` - Secret scanning
- `cargo deny` - License and dependency checker
- `axe` - Accessibility testing

### Documentation

- [Soroban Security Best Practices](https://docs.stellar.org/soroban/learn/security)
- [Smart Contract Security](./docs/SMART_CONTRACT_SECURITY.md)
- [OWASP Smart Contracts](https://owasp.org/www-community/attacks/)

## Related Commands

```bash
# Run security audit
cargo audit

# Fix known vulnerabilities
cargo audit --fix

# Check licenses
cargo deny check licenses

# Scan for secrets
gitleaks detect --source .

# View detailed vulnerability report
cargo audit json | jq .
```

## Support

For security questions or concerns:
- Email: security@stellar-nebula-nomad.dev
- Private advisory form: [link to form]
- PGP key: [link to key]
