# Governance Process

Governance supports four proposal classes: parameter changes, feature toggles, treasury spends, and contract upgrades.

## Lifecycle

1. A proposer with the configured minimum voting power creates a proposal.
2. Token holders vote during the active window.
3. Delegated power is added to direct voting power at vote time.
4. Proposals require 20% quorum by default.
5. Passing proposals enter a 48 hour timelock.
6. Simple parameter and feature proposals can execute automatically.
7. Treasury spends require multisig approval for large amounts.
8. Contract upgrades require manual execution and release checklist review.

## Emergency Veto

Emergency veto power is reserved for the admin multisig and should be used only for active exploits, governance capture, or irreversible treasury loss. Veto use must be documented in an incident report.

## Metrics

The DAO tracks proposal count, passed count, vote count, delegated votes, and participation basis points. These metrics feed the frontend governance dashboard.
