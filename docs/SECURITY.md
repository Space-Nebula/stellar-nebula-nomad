# Security Policy

## Reporting a Vulnerability

**Do not open a public GitHub issue for security vulnerabilities.**

Report vulnerabilities to the maintainers via the [GitHub private security advisory](https://github.com/Space-Nebula/stellar-nebula-nomad/security/advisories/new) form. Include:

- A concise impact statement
- Reproduction steps
- The affected component and contract function
- Suggested remediation (optional)

You will receive an acknowledgement within 48 hours. We follow a **90-day responsible disclosure embargo** before public disclosure, unless maintainers approve earlier release. See [docs/BUG_BOUNTY.md](BUG_BOUNTY.md) for reward tiers (Critical: 5000, High: 2000, Medium: 500, Low: 100).

---

## Supported Versions

| Version | Status        |
|---------|--------------|
| `main`  | ✅ Supported  |
| `< 0.1` | ❌ Unsupported |

All security fixes target `main` and are released as patch versions.

---

## Security Architecture

### Smart Contract Isolation

Stellar Nebula Nomad runs entirely as Soroban WebAssembly contracts. The Soroban runtime enforces:

- **Memory isolation** — each contract invocation runs in a fresh WASM sandbox; there is no shared mutable memory between contracts.
- **Deterministic execution** — all arithmetic uses the host's checked operations; panic = abort (see `panic = "abort"` in `Cargo.toml`).
- **No unsafe code in production** — `#![forbid(unsafe_code)]` is enforced at compile time (see [docs/UNSAFE.md](UNSAFE.md)).
- **Overflow checks in release** — `overflow-checks = true` in the release profile causes arithmetic overflow to abort the WASM invocation rather than silently wrap.

### Authentication and Authorization

Every state-modifying contract function calls `require_auth()` or `require_auth_for_args()` before touching ledger state. The auth model is:

| Pattern | Used for |
|---------|----------|
| `env.require_auth(&caller)` | Standard player actions (scan, mint, transfer) |
| `env.require_auth(&admin)` | Admin-only operations (pause, config updates) |
| `require_auth_for_args` | Fine-grained argument binding to prevent substitution attacks |

Role separation is enforced via the RBAC pattern described in [docs/RBAC_PATTERN_GUIDE.md](RBAC_PATTERN_GUIDE.md). All roles are stored in Persistent storage and require a governance vote to change (see [docs/adr/006-access-control-model.md](adr/006-access-control-model.md)).

### Arithmetic Safety

All balance-modifying code uses one of:

- `checked_add` / `checked_sub` / `checked_mul` — returns `None` on overflow; callers return a contract error.
- `saturating_*` — used only for non-financial counters where clamping is acceptable.
- Native `overflow-checks = true` — catches any missed case at the WASM host level.

Basis-point (BPS) rate calculations always divide by the constant `10_000`, which is never zero.

See [docs/YIELD_MATH.md](YIELD_MATH.md) for the full derivation of yield, staking, and ELO formulas.

### Reentrancy

Soroban's execution model completes outbound cross-contract calls atomically before control returns, so the classic EVM-style reentry vector does not apply to calls *made by* this contract. The remaining vector is **inbound**: `require_auth` against a custom-account contract executes that account's `__check_auth` code while this transaction is still in flight. The explicit global mutex in `src/reentrancy_guard.rs` covers that path. See [docs/REENTRANCY_PROTECTION.md](REENTRANCY_PROTECTION.md) for the guarded entry points, the nesting rule, and the attack-test suite.

### Input Validation

All public contract entry points validate inputs before any storage access via `src/input_validation.rs`. Validation rules:

- String lengths: 1–64 bytes.
- Numeric ranges: checked against per-field constants in `src/constants/`.
- Address format: Soroban host enforces 32-byte Stellar account IDs.
- Enum discriminants: unknown variants return `InvalidInput` before dispatch.

### Storage Key Namespacing

All storage keys are prefixed with the contract's domain symbol (max 9 chars, enforced by `tests/misc/test_symbol_lengths.rs`) to prevent cross-contract key collisions. Key structure: `<domain>:<entity>:<id>:<field>`.

### Emergency Controls

The `src/emergency_controls.rs` module supports:

- **Pause** — halts all state-modifying functions; reads remain available.
- **Admin rotation** — transfers admin key via governance vote with timelock.
- **Rate limiting** — per-player request caps enforced by `src/rate_limiter.rs`.

See [docs/ROLLBACK_PROCEDURES.md](ROLLBACK_PROCEDURES.md) for the incident response runbook.

---

## Known Limitations

1. **Ledger-seeded randomness is predictable** — `scan_nebula` uses `ledger_sequence XOR region_id` as a seed. Validators can predict nebula outcomes before submitting a scan. This is by design (see [docs/adr/003-rng-algorithm-selection.md](adr/003-rng-algorithm-selection.md)) and mitigated by the fact that there is no financial advantage to prediction.

2. **No on-chain oracle price feed** — `market_oracle.rs` prices are set by an admin-controlled oracle; a compromised oracle key could manipulate prices. Mitigation: oracle key requires 2-of-3 multisig and time-locked updates.

3. **Temporary storage expiry** — entries in Soroban `Temporary` storage expire after their TTL. If a player does not interact for the TTL window, their session data is lost. This is documented behaviour and does not affect Persistent balances.

4. **WASM binary size constraint** — the contract is optimised for size (`opt-level = "z"`), which may prevent certain hardening techniques that increase binary size.

5. **Governance quorum bootstrapping** — during the initial period with few token holders, reaching the 10% quorum threshold may be difficult. An emergency multisig bypass is available for critical fixes only.

---

## Security Checklist

- [x] `#![forbid(unsafe_code)]` in `src/lib.rs`
- [x] `overflow-checks = true` in release profile
- [x] `panic = "abort"` in release profile
- [x] All public functions call `require_auth()` before state changes
- [x] All arithmetic uses `checked_*` or `saturating_*`
- [x] No `unwrap()` or `expect()` in production paths (uses `?` operator)
- [x] No `panic!` in production paths
- [x] Input validation on all entry points via `src/input_validation.rs`
- [x] Storage keys namespaced and length-checked
- [x] Reentrancy guard on cross-contract calls
- [x] Rate limiting on high-frequency operations
- [x] Emergency pause implemented
- [x] `cargo audit` runs in CI on every push (`.github/workflows/security.yml`)
- [x] Dependency licence compliance checked via `cargo-deny`
- [x] Secret scanning via Gitleaks on full history
- [x] SBOM generated on every merge to `main`
- [x] Fuzz tests cover all input validation paths (see `tests/fuzz/`)
- [x] Chaos tests verify resilience (see `tests/chaos/`)

---

## Dependency Vulnerability Management

`cargo audit` runs daily at 02:00 UTC (`.github/workflows/security.yml`). Advisory configuration is in `audit.toml`:

- Severity threshold: `low` (all advisories reported).
- Informational warnings: `unmaintained`, `notice`.
- Explicit ignores require a comment in `audit.toml` with the reason.

To run locally:

```bash
cargo audit
```

To check for new advisories without failing the build:

```bash
cargo audit --ignore-format-version
```

---

## Secret Management

- Never commit real secrets (private keys, API tokens, passwords) to the repository.
- `.env.example` files contain placeholders only; copy them to an untracked `.env` and fill in values locally.
- Test fixtures that contain key-like strings must be clearly marked as tests and allowlisted in `.gitleaks.toml` with a short justification.
- The CI **Secret Scan (Gitleaks)** job scans the full history with the default rule set plus the project allowlist. Keep that job green before merging.

### Handling a leaked secret

1. Revoke/rotate the credential at the provider immediately.
2. Remove it from the working tree and add an allowlist entry only if it is a genuine test vector.
3. If a real secret was committed, replace it everywhere and rewrite the affected history (the scan reads all commits, not just the latest one).
4. Record the rotation in the incident log and verify `gitleaks detect .` passes locally.

### Local verification

```bash
gitleaks detect --no-banner -v .
```

The command must exit `0` with no leaks before a pull request is opened.
