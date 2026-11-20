# Threat Model — Stellar Nebula Nomad

**Version:** 0.1.0  
**Date:** 2026-11-19  
**Methodology:** STRIDE (Spoofing, Tampering, Repudiation, Information Disclosure, Denial of Service, Elevation of Privilege)

---

## 1. System Overview

Stellar Nebula Nomad is a set of Soroban smart contracts deployed on Stellar. All game state lives on-chain; there is no off-chain database with write authority. Players interact via signed Stellar transactions submitted to a Soroban RPC node. The backend services (analytics API, WebSocket server) are read-only observers.

### Trust Boundaries

```
[Player Wallet]  ──signs──▶  [Stellar RPC]  ──invokes──▶  [Soroban Contracts]
                                                                    │
[Admin Multisig] ──signs──▶  [Stellar RPC]  ──invokes──▶  [Admin Functions]
                                                                    │
[Oracle Key]     ──signs──▶  [Stellar RPC]  ──sets────▶  [Price / RNG Data]

[Analytics API]  ◀──read────  [Horizon API]  (no write authority)
[WebSocket Srv]  ◀──read────  [Horizon API]  (no write authority)
```

The Soroban runtime is the innermost trust boundary. Everything outside it — RPC nodes, frontend, backend services — is untrusted.

---

## 2. Assets

| Asset | Value | Location |
|-------|-------|----------|
| Player token balances | High | Persistent storage |
| Ship NFT ownership | High | Persistent storage |
| Staking positions | High | Persistent storage |
| Escrow funds | High | Persistent storage (escrow_trader) |
| Admin / governance keys | Critical | Off-chain multisig wallets |
| Oracle price feed key | High | Off-chain signing key |
| Contract WASM bytecode | High | Soroban ledger (hash-locked) |
| Player scores / leaderboard | Low | Persistent storage |
| Nebula layout cache | Low | Temporary storage (TTL-limited) |

---

## 3. Threat Analysis (STRIDE)

### 3.1 Spoofing

**S-01: Player impersonation**  
*Threat:* Attacker submits a transaction claiming to be another player.  
*Mitigation:* Soroban enforces `require_auth(&caller)` — any function that modifies state requires a valid Ed25519 signature from the caller's key. The runtime rejects the transaction if auth is not provided.  
*Residual risk:* None, assuming the player's private key is not compromised.

**S-02: Admin key compromise**  
*Threat:* Attacker obtains the admin private key and calls admin-only functions.  
*Mitigation:* Admin operations require a 2-of-3 multisig. Admin key rotation is enforced via governance vote with a 48-hour timelock (`emergency_controls.rs`).  
*Residual risk:* Low. Collusion of two signers or a supply-chain attack on the signing hardware.

**S-03: Oracle key spoofing**  
*Threat:* Attacker replaces the oracle price feed with manipulated data.  
*Mitigation:* Oracle address is stored in Persistent storage and can only be changed by admin multisig. Price updates include a staleness check (max 5 ledger sequences old).  
*Residual risk:* Medium. A compromised oracle key can manipulate prices until the timelock expires.

---

### 3.2 Tampering

**T-01: Direct ledger state manipulation**  
*Threat:* Attacker modifies contract storage without going through contract functions.  
*Mitigation:* Soroban ledger storage is append-only via the host interface; arbitrary writes are impossible without a valid contract invocation that passes auth checks.  
*Residual risk:* None at the protocol level.

**T-02: Integer overflow / underflow**  
*Threat:* Attacker crafts input values that overflow balance arithmetic, allowing fund creation or drain.  
*Mitigation:* All arithmetic uses `checked_*` operators. Release profile sets `overflow-checks = true`, which aborts on overflow instead of wrapping. Fuzz tests cover all arithmetic paths (`tests/fuzz/`).  
*Residual risk:* Very low. Would require a missed `checked_*` call; CI and fuzz testing guard against this.

**T-03: Reentrancy / cross-contract callback**  
*Threat:* A malicious contract called by Nebula Nomad re-enters a Nebula Nomad function before it completes, exploiting partially-applied state.  
*Mitigation:* Soroban completes **outbound** cross-contract calls atomically before returning. For the **inbound** vector — a custom-account `require_auth` callback re-entering a value-moving entry point — an explicit global mutex (`src/reentrancy_guard.rs`) rejects nested calls; 20 production entry points are guarded and six end-to-end attack scenarios are in `tests/security/reentrancy_attack_tests.rs`. Details: `docs/REENTRANCY_PROTECTION.md`.  
*Residual risk:* Low. Several `require_auth` entry points outside the guarded set rely on checks-effects-interactions ordering; they are not proven exploitable but should be re-audited before adding new composability paths.

**T-04: WASM bytecode replacement**  
*Threat:* Attacker replaces the deployed contract with a malicious version.  
*Mitigation:* Contract upgrades require admin multisig and a governance vote. The deployed WASM hash is recorded on-chain. See [docs/CONTRACT_UPGRADES.md](CONTRACT_UPGRADES.md).  
*Residual risk:* Low (requires admin compromise, see S-02).

---

### 3.3 Repudiation

**R-01: Player denies a transaction**  
*Threat:* Player claims they did not submit a transaction (e.g. to avoid a loss).  
*Mitigation:* Every transaction is signed by the player's key and recorded immutably on the Stellar ledger. The `audit_logger.rs` module emits structured events for every state change.  
*Residual risk:* None. On-chain signatures are non-repudiable.

**R-02: Admin denies a configuration change**  
*Threat:* Admin denies making a configuration change (e.g. pausing the contract).  
*Mitigation:* All admin operations emit audit events. Multisig requires multiple signers; the signing record is on-chain.  
*Residual risk:* None.

---

### 3.4 Information Disclosure

**I-01: Nebula outcome prediction**  
*Threat:* A player or validator predicts the outcome of `scan_nebula` before submitting the transaction, gaining an unfair advantage.  
*Mitigation:* The seed is `region_id XOR ledger_sequence`. Validators can predict outcomes. This is a **known, accepted limitation** (see [docs/adr/003-rng-algorithm-selection.md](adr/003-rng-algorithm-selection.md)) because there is no economic advantage to prediction in the current game design.  
*Residual risk:* Low for current design; must be re-evaluated if high-value resources are added.

**I-02: Player balance enumeration**  
*Threat:* Anyone can read all ledger storage, including player balances.  
*Mitigation:* Soroban storage is public by design. Sensitive aggregated statistics use the differential privacy approach in `src/privacy_stats.rs` to prevent individual identification from analytics.  
*Residual risk:* Low. Balances are public on all blockchains; players accept this.

**I-03: Private key leakage in logs / environment**  
*Threat:* Private keys are accidentally committed to the repository or logged.  
*Mitigation:* Gitleaks scans the full git history on every push. `.env.example` contains only placeholder values. CI uses GitHub Secrets for all sensitive values.  
*Residual risk:* Low.

---

### 3.5 Denial of Service

**D-01: Spam transactions exhaust rate limits**  
*Threat:* Attacker floods the contract with valid but cheap transactions, consuming Soroban instruction budget and degrading service for legitimate players.  
*Mitigation:* `src/rate_limiter.rs` enforces per-player caps (e.g. 5 scans per minute). Stellar network-level fees also make spam economically costly.  
*Residual risk:* Medium. A well-funded attacker can sustain spam despite fees.

**D-02: Leaderboard read cost grows with player count**  
*Threat:* As the player count grows, `leaderboard` reads sort the full set on each call, eventually exceeding the instruction limit.  
*Mitigation:* The leaderboard module uses a fixed-size top-N cache refreshed periodically, capping the per-call instruction cost. See known bottleneck in `tests/load/README.md`.  
*Residual risk:* Medium at very large scale (>100k players); requires a storage tier redesign.

**D-03: Oversized batch transactions**  
*Threat:* Attacker submits a batch of 10,000 items to exhaust gas.  
*Mitigation:* `src/batch_processor.rs` enforces a hard cap of 100 items per batch. The `src/constants/limits.rs` module centralises all batch and size limits.  
*Residual risk:* None — rejected before execution.

---

### 3.6 Elevation of Privilege

**E-01: Player calls admin function**  
*Threat:* Player submits a transaction calling `pause_contract` or `update_config`.  
*Mitigation:* Every admin function checks `env.require_auth(&self.admin)` where `self.admin` is the stored multisig address. A non-admin caller fails auth immediately.  
*Residual risk:* None.

**E-02: Governance vote manipulation**  
*Threat:* A whale accumulates enough tokens to unilaterally pass governance proposals, including elevating themselves to admin.  
*Mitigation:* Anti-whale caps (`src/economics/anti_whale.rs`) limit per-address voting power. Quorum requires 10% of total supply. Admin changes have a 48-hour timelock.  
*Residual risk:* Medium. A coalition of whales could collude. Mitigated by timelock giving the community time to react.

**E-03: Crafted error triggers privileged fallback path**  
*Threat:* Attacker triggers a specific error code that the contract handles by falling back to a less-restricted code path.  
*Mitigation:* Error handling uses explicit match on `#[contracterror]` enum variants. There are no generic `catch-all` fallbacks that grant elevated access. See [docs/ERROR_HANDLING.md](ERROR_HANDLING.md).  
*Residual risk:* Very low.

---

## 4. Attack Surface Summary

| Surface | Entry Points | Auth Required | Notes |
|---------|-------------|---------------|-------|
| Player contract functions | ~80 public fns | Yes (`require_auth`) | Rate-limited |
| Admin contract functions | ~15 public fns | Yes (multisig) | Timelocked |
| Oracle update functions | 2 public fns | Yes (oracle key) | Staleness checked |
| Soroban RPC endpoint | Public | No (read), Yes (write) | Network-level |
| Analytics API | Public (read) | No | Read-only Horizon |
| WebSocket server | Public (read) | No | Read-only events |

---

## 5. Risk Register

| ID | Description | Likelihood | Impact | Severity | Mitigation Status |
|----|-------------|------------|--------|----------|------------------|
| S-01 | Player impersonation | Very Low | High | Low | ✅ Mitigated |
| S-02 | Admin key compromise | Low | Critical | High | ✅ Mitigated (multisig) |
| S-03 | Oracle manipulation | Low | High | Medium | ⚠️ Partial (timelock) |
| T-01 | Direct state tamper | Very Low | Critical | Low | ✅ Mitigated (Soroban) |
| T-02 | Arithmetic overflow | Very Low | Critical | Low | ✅ Mitigated (checked) |
| T-03 | Reentrancy | Very Low | High | Low | ✅ Mitigated |
| T-04 | WASM replacement | Low | Critical | Medium | ✅ Mitigated (governance) |
| R-01 | Transaction repudiation | None | Medium | None | ✅ N/A (on-chain) |
| I-01 | RNG prediction | Medium | Low | Low | ✅ Accepted |
| I-02 | Balance enumeration | High | Low | Low | ✅ Accepted |
| D-01 | Transaction spam | Medium | Medium | Medium | ⚠️ Rate-limited |
| D-02 | Leaderboard scaling | Low | Medium | Low | ⚠️ Cache mitigates |
| D-03 | Oversized batch | Very Low | Medium | Very Low | ✅ Mitigated (cap) |
| E-01 | Privilege escalation | Very Low | Critical | Low | ✅ Mitigated |
| E-02 | Governance whale attack | Low | High | Medium | ⚠️ Anti-whale + timelock |
| E-03 | Error fallback exploit | Very Low | Medium | Very Low | ✅ Mitigated |

---

## 6. Security Assumptions

1. The Stellar/Soroban protocol itself is not compromised.
2. The admin multisig hardware wallets are stored securely and no two signers share a device.
3. The Stellar RPC nodes used in production are operated by trusted infrastructure providers.
4. Players understand that on-chain data (including balances) is publicly visible.
5. The Gitleaks and dependency audit tooling correctly identifies secrets and vulnerabilities.
6. The `overflow-checks = true` compiler flag is not stripped by any downstream build step.

---

## 7. Out of Scope

- Attacks on the Stellar protocol or Soroban runtime itself.
- Social engineering attacks on admin key holders.
- Physical attacks on signing hardware.
- Attacks on the player's local machine or wallet software.
- Frontend/mobile app security (separate scope).
