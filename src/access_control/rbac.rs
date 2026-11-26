use super::types::*;
use soroban_sdk::{symbol_short, Address, Env, Symbol, Vec};

fn get_admin(env: &Env) -> Option<Address> {
    env.storage()
        .persistent()
        .get(&AccessControlKey::AccessControlAdmin)
}

/// Set the admin address (used only during initialization and transfer).
fn set_admin(env: &Env, admin: &Address) {
    env.storage()
        .persistent()
        .set(&AccessControlKey::AccessControlAdmin, admin);
}

/// Fetch a role membership record for (role, address).
fn get_role_record(env: &Env, role: &Symbol, address: &Address) -> Option<RoleRecord> {
    env.storage()
        .persistent()
        .get(&AccessControlKey::RoleMember(role.clone(), address.clone()))
}

/// Write a role membership record for (role, address).
fn set_role_record(env: &Env, role: &Symbol, address: &Address, record: &RoleRecord) {
    env.storage().persistent().set(
        &AccessControlKey::RoleMember(role.clone(), address.clone()),
        record,
    );
}

/// Delete a role membership record (used in revocation).
fn delete_role_record(env: &Env, role: &Symbol, address: &Address) {
    env.storage()
        .persistent()
        .remove(&AccessControlKey::RoleMember(role.clone(), address.clone()));
}

/// Fetch a permission record (whether role can perform action).
fn get_permission(env: &Env, role: &Symbol, action: &Symbol) -> bool {
    env.storage()
        .persistent()
        .get(&AccessControlKey::RolePermission(
            role.clone(),
            action.clone(),
        ))
        .unwrap_or(false)
}

/// Set a permission record (role can perform action).
fn set_permission(env: &Env, role: &Symbol, action: &Symbol) {
    env.storage().persistent().set(
        &AccessControlKey::RolePermission(role.clone(), action.clone()),
        &true,
    );
}

/// Delete a permission record (revoke action from role).
fn delete_permission(env: &Env, role: &Symbol, action: &Symbol) {
    env.storage()
        .persistent()
        .remove(&AccessControlKey::RolePermission(
            role.clone(),
            action.clone(),
        ));
}

/// Fetch the list of known role names.
fn get_known_roles(env: &Env) -> Vec<Symbol> {
    env.storage()
        .persistent()
        .get(&AccessControlKey::KnownRoles)
        .unwrap_or_else(|| Vec::new(env))
}

/// Add a role name to the known roles list (if not already present).
fn register_known_role(env: &Env, role: &Symbol) {
    let mut roles = get_known_roles(env);
    // Check for duplicates
    for i in 0..roles.len() {
        if let Some(r) = roles.get(i) {
            if r == *role {
                return;
            }
        }
    }
    roles.push_back(role.clone());
    env.storage()
        .persistent()
        .set(&AccessControlKey::KnownRoles, &roles);
}

/// Fetch parent role in hierarchy
pub fn get_role_parent(env: &Env, role: &Symbol) -> Option<Symbol> {
    env.storage()
        .persistent()
        .get(&AccessControlKey::RoleParent(role.clone()))
}

/// Fetch delegation record
fn get_delegation_record(
    env: &Env,
    role: &Symbol,
    delegatee: &Address,
) -> Option<DelegatedRoleRecord> {
    env.storage()
        .persistent()
        .get(&AccessControlKey::RoleDelegation(
            role.clone(),
            delegatee.clone(),
        ))
}

// ═══════════════════════════════════════════════════════════════════════════════
// CORE ROLE & PERMISSION FUNCTIONS
// ═══════════════════════════════════════════════════════════════════════════════

/// Check whether an address currently holds an active, non-expired role.
///
/// # Returns
/// - `true` if the address holds the role and it has not expired or been revoked.
/// - `false` if the role was never granted, has expired, or has been revoked.
///
/// # Notes
/// - This is a pure read operation; no storage mutations, no authentication required.
/// - Safe to call from any context; does not emit events.
pub fn has_role(env: &Env, role: &Symbol, address: &Address) -> bool {
    if let Some(record) = get_role_record(env, role, address) {
        // Check if revoked
        if !record.active {
            return false;
        }
        // Check if expired
        if let Some(expiry_ledger) = record.expiry_ledger {
            if env.ledger().sequence() >= expiry_ledger {
                return false;
            }
        }
        return true;
    }

    // Check active unexpired delegation
    if let Some(delegation) = get_delegation_record(env, role, address) {
        if delegation.active && env.ledger().sequence() < delegation.expiry_ledger {
            return true;
        }
    }

    false
}

/// Check whether a role is permitted to perform an action, including inherited permissions.
///
/// # Returns
/// - `true` if the permission was granted directly or inherited from parent role.
/// - `false` if the permission was never granted or was revoked.
///
/// # Notes
/// - This is a pure read operation; no storage mutations, no authentication required.
pub fn has_permission(env: &Env, role: &Symbol, action: &Symbol) -> bool {
    let mut current_role = role.clone();
    let mut depth = 0;
    while depth < 6 {
        if get_permission(env, &current_role, action) {
            return true;
        }
        if let Some(parent) = get_role_parent(env, &current_role) {
            current_role = parent;
            depth += 1;
        } else {
            break;
        }
    }
    false
}

/// Grant a role to a single address, optionally expiring at a future ledger sequence.
///
/// # Authorization
/// - Only the admin can call this function (checked via `caller.require_auth()`).
///
/// # Errors
/// - `AdminRequired` if caller is not the admin.
/// - `InvalidExpiry` if expiry is provided and is at or before the current ledger sequence.
///
/// # Idempotency
/// - If the address already holds the role, the call succeeds and overwrites the record (updating expiry).
/// - Optional: Return `RoleAlreadyGranted` if idempotency by error is desired (see maintainer feedback).
///
/// # Events
/// - Emits `RoleGranted` event with (role, grantee, expiry).
pub fn grant_role(
    env: &Env,
    caller: Address,
    role: Symbol,
    grantee: Address,
    expiry: Option<u32>,
) -> Result<(), AccessControlError> {
    caller.require_auth();

    // Check admin
    let admin = get_admin(env).ok_or(AccessControlError::InitializationFailed)?;
    if caller != admin {
        return Err(AccessControlError::AdminRequired);
    }

    // Validate expiry
    if let Some(exp) = expiry {
        if exp <= env.ledger().sequence() {
            return Err(AccessControlError::InvalidExpiry);
        }
    }

    // Register the role as known
    register_known_role(env, &role);

    // Write the role membership record
    let record = RoleRecord::new(expiry);
    set_role_record(env, &role, &grantee, &record);

    // Emit event
    env.events().publish(
        (symbol_short!("rbac"), symbol_short!("grant")),
        (role, grantee, expiry),
    );

    Ok(())
}

/// Revoke a role from an address.
///
/// # Authorization
/// - Only the admin can call this function.
///
/// # Errors
/// - `AdminRequired` if caller is not the admin.
///
/// # Idempotency
/// - If the address does not hold the role, the call succeeds silently (idempotent removal).
///
/// # Events
/// - Emits `RoleRevoked` event with (role, revokee).
pub fn revoke_role(
    env: &Env,
    caller: Address,
    role: Symbol,
    revokee: Address,
) -> Result<(), AccessControlError> {
    caller.require_auth();

    let admin = get_admin(env).ok_or(AccessControlError::InitializationFailed)?;
    if caller != admin {
        return Err(AccessControlError::AdminRequired);
    }

    // Delete the role membership record (idempotent: no error if not held)
    delete_role_record(env, &role, &revokee);

    // Emit event
    env.events().publish(
        (symbol_short!("rbac"), symbol_short!("revoke")),
        (role, revokee),
    );

    Ok(())
}

/// Grant a role to up to 5 addresses in a single call (batch operation).
///
/// # Errors
/// - `BatchLimitExceeded` if grantees.len() > 5 (atomic; no grants applied if exceeded).
/// - `AdminRequired` if caller is not the admin.
/// - `InvalidExpiry` if expiry is in the past.
///
/// # All-or-Nothing Semantics
/// - If the batch exceeds the limit, no grants are applied (revert before any writes).
/// - If any other validation fails, all grants are rolled back (Soroban implicit on error return).
///
/// # Events
/// - Emits one `RoleGranted` event per granted address (N events for N grantees).
pub fn grant_role_batch(
    env: &Env,
    caller: Address,
    role: Symbol,
    grantees: Vec<Address>,
    expiry: Option<u32>,
) -> Result<(), AccessControlError> {
    // Check batch size FIRST before any other operation
    if grantees.len() > BATCH_GRANT_LIMIT {
        return Err(AccessControlError::BatchLimitExceeded);
    }

    caller.require_auth();

    let admin = get_admin(env).ok_or(AccessControlError::InitializationFailed)?;
    if caller != admin {
        return Err(AccessControlError::AdminRequired);
    }

    if let Some(exp) = expiry {
        if exp <= env.ledger().sequence() {
            return Err(AccessControlError::InvalidExpiry);
        }
    }

    register_known_role(env, &role);

    // Grant role to each grantee
    let record = RoleRecord::new(expiry);
    for i in 0..grantees.len() {
        if let Some(grantee) = grantees.get(i) {
            set_role_record(env, &role, &grantee, &record);
            env.events().publish(
                (symbol_short!("rbac"), symbol_short!("grant")),
                (role.clone(), grantee, expiry),
            );
        }
    }

    Ok(())
}

/// Primary access guard function: Check whether a caller can perform an action.
///
/// # Errors
/// - `UnauthorizedRole` if the caller does not hold any role that permits the action.
///
/// # Behavior
/// - Iterates through all known roles.
/// - For each role, checks if caller holds it (via `has_role`) and if it permits the action (via `has_permission`).
/// - If any role permits the action, returns successfully.
/// - Otherwise, reverts with `UnauthorizedRole`.
///
/// # Usage
/// - Call this function as the first statement in any mutating contract function:
///   ```rust,ignore
///   pub fn sensitive_operation(env: Env, caller: Address, ...) -> Result<...> {
///       check_permission(&env, &caller, symbol_short!("sensitive_op"))?;
///       // ... operation logic
///   }
///   ```
///
/// # Events
/// - Emits an `rbac/perm_fail` event **only when the check fails**, so denials
///   stay auditable without paying for an event on every successful check.
pub fn check_permission(
    env: &Env,
    caller: &Address,
    action: &Symbol,
) -> Result<(), AccessControlError> {
    let known_roles = get_known_roles(env);

    // If emergency mode is active, check if caller holds any designated emergency role
    if is_emergency_mode(env) {
        for i in 0..known_roles.len() {
            if let Some(role) = known_roles.get(i) {
                if is_emergency_role(env, &role) && has_role(env, &role, caller) {
                    return Ok(());
                }
            }
        }
    }

    // Iterate through all known roles
    for i in 0..known_roles.len() {
        if let Some(role) = known_roles.get(i) {
            if has_role(env, &role, caller) && has_permission(env, &role, action) {
                return Ok(());
            }
        }
    }

    // No role permitted the action
    env.events().publish(
        (symbol_short!("rbac"), symbol_short!("perm_fail")),
        (caller.clone(), action.clone()),
    );
    Err(AccessControlError::UnauthorizedRole)
}

/// Grant a permission to a role (allow role to perform action).
///
/// # Authorization
/// - Only the admin can call this function.
///
/// # Errors
/// - `AdminRequired` if caller is not the admin.
///
/// # Events
/// - Emits `PermissionGranted` event with (role, action).
pub fn grant_permission(
    env: &Env,
    caller: Address,
    role: Symbol,
    action: Symbol,
) -> Result<(), AccessControlError> {
    caller.require_auth();

    let admin = get_admin(env).ok_or(AccessControlError::InitializationFailed)?;
    if caller != admin {
        return Err(AccessControlError::AdminRequired);
    }

    set_permission(env, &role, &action);

    env.events().publish(
        (symbol_short!("rbac"), symbol_short!("perm_g")),
        (role, action),
    );

    Ok(())
}

/// Revoke a permission from a role (disallow role from performing action).
///
/// # Authorization
/// - Only the admin can call this function.
///
/// # Errors
/// - `AdminRequired` if caller is not the admin.
///
/// # Idempotency
/// - If the permission was never granted, the call succeeds silently.
///
/// # Events
/// - Emits `PermissionRevoked` event with (role, action).
pub fn revoke_permission(
    env: &Env,
    caller: Address,
    role: Symbol,
    action: Symbol,
) -> Result<(), AccessControlError> {
    caller.require_auth();

    let admin = get_admin(env).ok_or(AccessControlError::InitializationFailed)?;
    if caller != admin {
        return Err(AccessControlError::AdminRequired);
    }

    delete_permission(env, &role, &action);

    env.events().publish(
        (symbol_short!("rbac"), symbol_short!("perm_r")),
        (role, action),
    );

    Ok(())
}

/// Transfer the admin role to a new address.
///
/// # Authorization
/// - Only the current admin can call this function.
///
/// # Effects
/// - Revokes the admin role from the current admin.
/// - Grants the admin role to the new admin (with no expiry).
/// - Updates the stored admin address.
///
/// # Errors
/// - `AdminRequired` if caller is not the current admin.
///
/// # Events
/// - Emits `AdminTransferred` event with (old_admin, new_admin).
/// - Also implicitly emits `RoleRevoked` and `RoleGranted` events for the admin role.
///
/// # Critical Security Note
/// - This function is time-sensitive. After transfer, the new admin immediately has privileges.
/// - The old admin immediately loses privileges. Confirm this is intentional before calling.
pub fn transfer_admin(
    env: &Env,
    caller: Address,
    new_admin: Address,
) -> Result<(), AccessControlError> {
    caller.require_auth();

    let old_admin = get_admin(env).ok_or(AccessControlError::InitializationFailed)?;
    if caller != old_admin {
        return Err(AccessControlError::AdminRequired);
    }

    let admin_sym = admin_role();

    // Move the admin role directly through the storage helpers. Going through
    // `revoke_role`/`grant_role` would call `caller.require_auth()` twice more
    // in this same frame, which the host rejects (`Auth, ExistingValue`), so
    // the transfer could never succeed on-chain.
    delete_role_record(env, &admin_sym, &old_admin);
    env.events().publish(
        (symbol_short!("rbac"), symbol_short!("revoke")),
        (admin_sym.clone(), old_admin.clone()),
    );
    register_known_role(env, &admin_sym);
    set_role_record(env, &admin_sym, &new_admin, &RoleRecord::new(None));
    env.events().publish(
        (symbol_short!("rbac"), symbol_short!("grant")),
        (admin_sym, new_admin.clone(), Option::<u32>::None),
    );

    // Update the admin address
    set_admin(env, &new_admin);

    // Emit transfer event
    env.events().publish(
        (symbol_short!("rbac"), symbol_short!("xfer_adm")),
        (old_admin, new_admin),
    );

    Ok(())
}

/// Initialize the RBAC system with default roles and admin privileges.
///
/// # Requirements
/// - Must be called exactly once at contract startup.
/// - The admin address must authorize this call via `admin.require_auth()`.
///
/// # Effects
/// - Stores the admin address.
/// - Creates three default roles: `admin`, `nomad`, `indexer`.
/// - Grants the `admin` role to the admin address with no expiry.
/// - Leaves `nomad` and `indexer` roles with no members; they must be populated via `grant_role`.
/// - No permissions are granted at init; all must be explicitly assigned via `grant_permission`.
///
/// # Errors
/// - `InitializationFailed` if init_roles has already been called (idempotency guard).
///
/// # Events
/// - Does NOT emit initialization events; calls to `grant_role` emit `RoleGranted`.
pub fn init_roles(env: &Env, admin: Address) -> Result<(), AccessControlError> {
    admin.require_auth();

    // Idempotency guard: fail if already initialized
    if get_admin(env).is_some() {
        return Err(AccessControlError::InitializationFailed);
    }

    // Store admin
    set_admin(env, &admin);

    // Create default roles and register them
    let admin_sym = admin_role();
    let nomad_sym = nomad_role();
    let indexer_sym = indexer_role();

    register_known_role(env, &admin_sym);
    register_known_role(env, &nomad_sym);
    register_known_role(env, &indexer_sym);

    // Grant admin role to admin (with no expiry)
    let admin_record = RoleRecord::new(None);
    set_role_record(env, &admin_sym, &admin, &admin_record);

    Ok(())
}

/// Placeholder for future DAO-driven role proposal and governance integration.
///
/// # Does Not Exist Yet
/// This function is explicitly a stub and will revert if called. It is defined to establish
/// the interface for future work.
///
/// # Intended Future Behavior (Issue #41 Follow-up)
/// 1. Proposer (any address) calls `propose_role_change(env, proposer, role, action)`.
/// 2. The proposal is stored with a unique ID, a voting period, and a DAO contract reference.
/// 3. DAO members vote on the proposal via their governance contract.
/// 4. After the voting period, an external actor calls `execute_dao_proposal(env, proposal_id)`.
/// 5. If the proposal passed quorum and voting was affirmative, the role/permission change is applied.
/// 6. Events document each stage: `DAOProposalCreated`, `DAOProposalExecuted`, `DAOProposalFailed`.
///
/// # Current Behavior
/// - Reverts with `NotImplemented` to prevent accidental misuse.
///
/// # Parameters
/// - `proposer`: The address proposing the change (required to vote).
/// - `role`: The role affected by the proposal.
/// - `grantee`: (if applicable) The address affected; can be ignored for permission-only proposals.
/// - `action_type`: One of "grant", "revoke", "expire" to indicate the operation type.
///
/// # Returns
/// - Always `Err(NotImplemented)` until DAO integration is implemented.
pub fn propose_role_change(
    _env: &Env,
    proposer: Address,
    _role: Symbol,
    _grantee: Address,
    _action_type: Symbol,
) -> Result<u64, AccessControlError> {
    // Stub: revert immediately
    proposer.require_auth();
    Err(AccessControlError::NotImplemented)
}

