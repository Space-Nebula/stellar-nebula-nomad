//! # Role-Based Access Control (RBAC) System
//!
//! This module implements a fine-grained, production-grade Role-Based Access Control (RBAC) system
//! for the Stellar Nebula Nomad smart contract suite. It provides:
//!
//! - **Role Management**: Grant, revoke, and manage roles (admin, nomad, indexer, custom).
//! - **Time-Bound Roles**: Roles can expire at a specified ledger sequence number.
//! - **Permissions**: Link actions (Symbols) to roles, enabling fine-grained access control.
//! - **Event Emission**: Every role and permission change emits an event for on-chain auditing.
//! - **Idempotent Operations**: Grant and revoke operations are safe to retry.
//! - **Admin Transfer**: Transfer admin privileges to a new address.
//! - **DAO Extensibility**: Stub for future DAO-driven role proposals and governance.
//!
//! ## Architecture
//!
//! The RBAC system is built around three core concepts:
//!
//! 1. **Roles**: Named groups (admin, nomad, indexer) that can perform actions.
//! 2. **Permissions**: Mappings from (role, action) pairs to booleans, defining what actions a role can perform.
//! 3. **Role Membership**: Individual (address, role) pairs with optional expiration, defining who holds what role.
//!
//! ## Security Invariants
//!
//! ✓ Admin-only mutating operations are non-bypassable (grant_role, revoke_role, transfer_admin).
//! ✓ Expired roles are treated as absent, identical to never-granted roles.
//! ✓ Revoked roles are indistinguishable from never-granted roles.
//! ✓ Batch grants are atomic (all-or-nothing with B=5 limit per call).
//! ✓ Storage key collision is impossible (DataKey enum variants are structurally distinct).
//! ✓ Initialization is idempotent and revert-safe (called only once; subsequent calls fail).
//! ✓ Failed operations leave state exactly as before (no partial writes).
//! ✓ Time-bound expiry uses ledger.sequence(), which is manipulation-resistant in Soroban.
//!
//! ## Security Audit Checklist
//!
//! - [ ] Admin Key Protection: Only the stored admin address can call grant_role, revoke_role, etc.
//! - [ ] Role Expiry Enforcement: Expired roles return false from has_role() and fail check_permission().
//! - [ ] Batch Limit Enforcement: Batch grants with N > 5 revert without granting any role.
//! - [ ] Revocation Completeness: Revoked roles behave identically to never-granted roles.
//! - [ ] Event Emission Completeness: All role/permission changes emit events; events carry correct data.
//! - [ ] Storage Key Collision Safety: (role, address) keys are distinct from (role, action) keys.
//!
//! ## Reusable Pattern Guide
//!
//! ### Adding a New Role
//!
//! 1. Choose a role name (max 32 chars, ideally <15 to align with symbol_short!() usage).
//! 2. Call `grant_role(env, admin, symbol_short!("new_role"), address, None)` to give the role to an address.
//! 3. Call `grant_permission(env, admin, symbol_short!("new_role"), symbol_short!("action_name"))` to allow the role to perform an action.
//!
//! ### Adding a New Guarded Action
//!
//! 1. Define an action Symbol constant in the calling module or lib.rs:
//!    ```rust,ignore
//!    const TRANSFER_ACTION: Symbol = symbol_short!("transfer");
//!    ```
//! 2. In the function performing that action, add an early-exit guard as the first statement:
//!    ```rust,ignore
//!    pub fn transfer(..., caller: Address, ...) -> Result<...> {
//!        caller.require_auth();
//!        access_control::check_permission(&env, &caller, symbol_short!("transfer"))?;
//!        // ... rest of function
//!    }
//!    ```
//! 3. Ensure the admin grants the necessary roles permission to the action via `grant_permission`.
//!
//! ### Extending for DAO Proposals
//!
//! 1. Implement a DAO module that stores proposals and voting records.
//! 2. Create an `execute_dao_proposal(env, proposal_id)` function that:
//!    - Retrieves the proposal from DAO storage.
//!    - Checks that voting is closed and the proposal passed quorum.
//!    - Calls the appropriate RBAC function (e.g., `grant_role_internal`).
//! 3. Leave `propose_role_change` as a stub or integrate with DAO's public interface.

use crate::error_standard::{ErrorDescriptor, ErrorKind, StandardContractError};
use soroban_sdk::{contracterror, contracttype, symbol_short, Address, Env, Symbol, Vec};

// ═══════════════════════════════════════════════════════════════════════════════
// ERRORS
// ═══════════════════════════════════════════════════════════════════════════════

/// Errors arising from RBAC operations.
///
/// Public and re-exported at the crate root (`stellar_nebula_nomad::AccessControlError`,
/// Issue #586), so contract entry points in `lib.rs` such as `init_rbac`,
/// `grant_role` and `check_permission` can return
/// `Result<_, AccessControlError>` and SDK clients can match on it.
///
/// Discriminants are part of the on-chain ABI (`#[contracterror]`) and must
/// never be renumbered; append new variants with fresh values only. See
/// `docs/ERROR_CODES.md`.
#[contracterror]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum AccessControlError {
    /// Admin required: Caller is not the current admin.
    AdminRequired = 1,
    /// Role not found: The specified role does not exist or was never defined.
    RoleNotFound = 2,
    /// Unauthorized role: Caller does not hold the required role, or role is expired/revoked.
    UnauthorizedRole = 3,
    /// Role already granted: The address already holds the role (idempotency policy).
    RoleAlreadyGranted = 4,
    /// Batch limit exceeded: Batch grant attempted with more than 5 addresses.
    BatchLimitExceeded = 5,
    /// Invalid expiry: Expiry value is in the past or otherwise invalid.
    InvalidExpiry = 6,
    /// Permission not found: The (role, action) permission was never defined.
    PermissionNotFound = 7,
    /// Initialization failed: init_roles was already called or another init error occurred.
    InitializationFailed = 8,
    /// Not implemented: This function is a placeholder for future DAO integration.
    NotImplemented = 9,
    /// Multi-sig not configured.
    MultiSigNotConfigured = 10,
    /// Proposal not found.
    ProposalNotFound = 11,
    /// Proposal already executed.
    ProposalAlreadyExecuted = 12,
    /// Proposal expired.
    ProposalExpired = 13,
    /// Insufficient approvals.
    InsufficientApprovals = 14,
    /// Already approved.
    AlreadyApproved = 15,
    /// Timelock not elapsed.
    TimelockNotElapsed = 16,
    /// Invalid signer configuration.
    InvalidSignerConfig = 17,
    /// Invalid role hierarchy (e.g. cycle or self-parenting).
    InvalidRoleHierarchy = 18,
    /// Operation not permitted outside emergency mode.
    NotInEmergencyMode = 19,
    /// Delegation expired or invalid.
    InvalidDelegation = 20,
}

impl StandardContractError for AccessControlError {
    fn descriptor(self) -> ErrorDescriptor {
        use AccessControlError::*;
        let (kind, retryable) = match self {
            AdminRequired | UnauthorizedRole | InsufficientApprovals | NotInEmergencyMode => {
                (ErrorKind::Authorization, false)
            }
            RoleNotFound | PermissionNotFound | MultiSigNotConfigured | ProposalNotFound => {
                (ErrorKind::NotFound, false)
            }
            RoleAlreadyGranted | ProposalAlreadyExecuted | AlreadyApproved => {
                (ErrorKind::Conflict, false)
            }
            BatchLimitExceeded => (ErrorKind::ResourceLimit, false),
            InvalidExpiry | ProposalExpired | InvalidSignerConfig | InvalidRoleHierarchy
            | InvalidDelegation => (ErrorKind::Validation, false),
            TimelockNotElapsed => (ErrorKind::Conflict, true),
            InitializationFailed | NotImplemented => (ErrorKind::Internal, false),
        };
        ErrorDescriptor {
            module: "access_control",
            code: self as u32,
            kind,
            retryable,
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// STORAGE KEYS & TYPES
// ═══════════════════════════════════════════════════════════════════════════════

/// Storage key variants for RBAC state.
///
/// - `Admin`: Singleton key storing the current admin address (Address).
/// - `RoleMember(role, address)`: Stores a RoleRecord indicating membership of address in role.
/// - `RolePermission(role, action)`: Stores a boolean (or unit ()) indicating the role can perform action.
/// - `KnownRoles`: Stores a Vec of all defined role names to enable role enumeration.
/// - `MultiSigConfig`: Configuration for multi-signature admin operations.
/// - `Proposal(proposal_id)`: Stores proposal data for multi-sig operations.
/// - `ProposalApproval(proposal_id, approver)`: Records approvals for proposals.
/// - `ProposalCounter`: Global counter for proposal IDs.
#[derive(Clone)]
#[contracttype]
pub enum AccessControlKey {
    /// Singleton: current admin address.
    AccessControlAdmin,
    /// Role membership: (role Symbol, member Address) -> RoleRecord.
    RoleMember(Symbol, Address),
    /// Permission grant: (role Symbol, action Symbol) -> bool.
    RolePermission(Symbol, Symbol),
    /// Enumeration: All known roles (for check_permission iteration).
    KnownRoles,
    /// Multi-signature configuration.
    MultiSigConfig,
    /// Proposal data keyed by proposal ID.
    AccessControlProposal(u64),
    /// Approval record: (proposal_id, approver) -> bool.
    ProposalApproval(u64, Address),
    /// Global proposal counter.
    AccessControlProposalCounter,
    /// List of all admin signers.
    AdminSigners,
    /// Role inheritance: role -> parent role Symbol.
    RoleParent(Symbol),
    /// Emergency mode toggle.
    EmergencyMode,
    /// Emergency role flag: role Symbol -> bool.
    EmergencyRole(Symbol),
    /// Role delegation: (role Symbol, delegatee Address) -> DelegatedRoleRecord.
    RoleDelegation(Symbol, Address),
}

/// Record of a delegated role assignment.
#[derive(Clone, Debug, PartialEq)]
#[contracttype]
pub struct DelegatedRoleRecord {
    pub delegator: Address,
    pub delegatee: Address,
    pub role: Symbol,
    pub expiry_ledger: u32,
    pub active: bool,
}

/// Record of a role membership, including expiry and revocation state.
#[derive(Clone, Debug, PartialEq)]
#[contracttype]
pub struct RoleRecord {
    /// Whether the role assignment is currently active (true) or revoked (false).
    pub active: bool,
    /// Optional ledger sequence at which the role expires. If Some(seq), the role is invalid after ledger.sequence() >= seq.
    pub expiry_ledger: Option<u32>,
}

impl RoleRecord {
    /// Create a new active role assignment, optionally expiring at a future ledger sequence.
    pub fn new(expiry_ledger: Option<u32>) -> Self {
        Self {
            active: true,
            expiry_ledger,
        }
    }
}

/// Multi-signature configuration for admin operations.
#[derive(Clone, Debug, PartialEq)]
#[contracttype]
pub struct MultiSigConfig {
    /// Number of required approvals (M in M-of-N).
    pub required_approvals: u32,
    /// Total number of signers (N in M-of-N).
    pub total_signers: u32,
    /// Timelock period in seconds for critical operations.
    pub timelock_period: u64,
    /// Whether multi-sig is enabled.
    pub enabled: bool,
}

/// Proposal for multi-sig operations.
#[derive(Clone, Debug, PartialEq)]
#[contracttype]
pub struct Proposal {
    pub proposal_id: u64,
    pub proposer: Address,
    pub operation: ProposalOperation,
    pub created_at: u64,
    pub execution_time: u64, // When it can be executed (after timelock)
    pub executed: bool,
    pub approval_count: u32,
}

/// Type of operation in a proposal.
#[derive(Clone, Debug, PartialEq)]
#[contracttype]
pub enum ProposalOperation {
    /// Grant a role to an address.
    GrantRole(Symbol, Address, Option<u32>),
    /// Revoke a role from an address.
    RevokeRole(Symbol, Address),
    /// Grant permission to a role.
    GrantPermission(Symbol, Symbol),
    /// Revoke permission from a role.
    RevokePermission(Symbol, Symbol),
    /// Transfer admin to a new address.
    TransferAdmin(Address),
    /// Update multi-sig configuration.
    UpdateMultiSig(MultiSigConfig),
}

// ═══════════════════════════════════════════════════════════════════════════════
// CONSTANTS
// ═══════════════════════════════════════════════════════════════════════════════

/// Maximum number of addresses that can receive a role in a single batch grant call.
pub const BATCH_GRANT_LIMIT: u32 = 5;

// Default role names as Symbols. Using symbol_short!() for brevity.
pub(crate) fn admin_role() -> Symbol {
    symbol_short!("admin")
}

fn nomad_role() -> Symbol {
    symbol_short!("nomad")
}

fn indexer_role() -> Symbol {
    symbol_short!("indexer")
}

