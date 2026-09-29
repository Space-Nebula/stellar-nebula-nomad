# Automated Bug Bounty Program & Responsible Disclosure Policy

Stellar Nebula Nomad operates an autonomous, smart-contract-governed bug bounty engine deployed on the Stellar network via Soroban. This document outlines the technical scope, reward structures, submission protocols, and responsible disclosure requirements for security researchers.

---

## 1. Reward Tiers & Severity Classification

Bounties are denominated in USD value and paid out programmatically in XLM or USDC directly from the on-chain bounty pool escrow. Rewards are classified according to impact:

| Severity | Bounty Reward | Technical Impact Scope |
| :--- | :--- | :--- |
| **Critical** | **$5,000 USD** | Direct, permanent drain or theft of escrow assets; consensus breaks; unauthorized state takeovers; bypassing smart contract authentication (`require_auth`). |
| **High** | **$2,000 USD** | Temporary locking of user/protocol assets; griefing vectors; unauthorized modification of game metadata or player inventory states. |
| **Medium** | **$500 USD** | Economic arbitrage resulting from rounding faults; DEX/AMM fee evasion; denial of service on non-critical game loops. |
| **Low** | **$100 USD** | Boundary edge cases; gas consumption inefficiencies without system halt; non-exploitable event log desyncs. |

Administrators and DAO governance reserve the right to dynamically adjust reward tiers via `set_reward_tier` or approve custom bounty payouts during consensus review.

---

## 2. On-Chain Cryptographic Duplicate Detection

To prevent bounty sniping, front-running, and duplicate payout drain, every submission is fingerprinted on-chain:
- **Hashing Primitive:** The contract computes a SHA-256 digest of the raw vulnerability description using `env.crypto().sha256(&Bytes::from(&description))`.
- **First-to-File Guarantee:** The SHA-256 digest is permanently registered in persistent contract storage alongside the originating report ID.
- **Automated Rejection:** Any subsequent submission yielding an identical hash is rejected with `BountyError::DuplicateReport`.

---

## 3. Responsible Disclosure & 90-Day Embargo

All vulnerability reports submitted to the Stellar Nebula Nomad contract are governed by a strict 90-day responsible disclosure embargo (`EMBARGO_PERIOD_SECONDS = 7_776_000`):

1. **Embargo Period:** The embargo window begins precisely at `submitted_at` block timestamp.
2. **Embargo Gating:** The smart contract function `can_disclose_report(report_id)` returns `false` during the 90 days. Calling `disclose_bug_report` prior to expiration aborts with `BountyError::EmbargoActive`.
3. **Public Disclosure:** Once the 90-day window has elapsed (`ledger_timestamp >= submitted_at + 7_776_000`), the researcher or administrator can execute `disclose_bug_report` to transition the status to `ReportStatus::Disclosed` and publish remediation findings.

---

## 4. Review Workflow & Payout Consensus

To ensure decentralized oversight and prevent unauthorized capital flight:
- **Multi-Approver Threshold:** Payouts require approval from a threshold of authorized security reviewers (`approval_threshold`).
- **Rejection with Feedback:** Reviewers can formally reject out-of-scope or informational submissions with `reject_bug_report(admin, report_id, feedback)`.
- **Severity Reclassification:** If a report's true impact differs from the researcher's initial filing, reviewers can update the classification via `reclassify_severity(admin, report_id, new_severity)`.
- **Timelock for High-Value Bounties:** Payouts equal to or exceeding `high_value_threshold` (e.g. $5,000) are subject to an on-chain timelock (`timelock_seconds`) calculated from `submitted_at` before the final approval and transfer can settle.

---

## 5. Researcher Hall of Fame & Cumulative Accounting

The contract maintains immutable statistics for all contributing security researchers:
- **Cumulative Metrics:** `get_total_bounties_paid()` provides real-time public accounting of all rewards disbursed.
- **Researcher Profiles:** `get_researcher_stats(address)` records `total_bounties` earned and `bugs_resolved` count.
- **Hall of Fame:** `get_hall_of_fame()` exposes a public roster of all researchers who have resolved vulnerabilities.

---

## 6. Governance & Emergency Controls

- **DAO Budget Allocation:** The community governance contract can directly fund the security bounty pool via `allocate_bounty_gov_budget(dao, amount)`.
- **Emergency Circuit Breaker:** In the event of active exploitation or migration, administrators can invoke `set_emergency_pause(admin, true)` to halt report ingestion and fund disbursement immediately.
