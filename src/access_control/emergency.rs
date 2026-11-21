use super::types::*;
use super::rbac::*;
use soroban_sdk::{symbol_short, Address, Env, Symbol};


/// Set parent role for role hierarchy inheritance.
pub fn set_role_parent(
    env: &Env,
    caller: Address,
    role: Symbol,
    parent: Symbol,
) -> Result<(), AccessControlError> {
    caller.require_auth();
    let admin = get_admin(env).ok_or(AccessControlError::InitializationFailed)?;
    if caller != admin {
        return Err(AccessControlError::AdminRequired);
    }
    if role == parent {
        return Err(AccessControlError::InvalidRoleHierarchy);
    }
    register_known_role(env, &role);
    register_known_role(env, &parent);

    env.storage()
        .persistent()
        .set(&AccessControlKey::RoleParent(role.clone()), &parent);

    env.events().publish(
        (symbol_short!("rbac"), symbol_short!("parent")),
        (role, parent),
    );
    Ok(())
}

/// Set emergency mode globally.
pub fn set_emergency_mode(
    env: &Env,
    caller: Address,
    enabled: bool,
) -> Result<(), AccessControlError> {
    caller.require_auth();
    let admin = get_admin(env).ok_or(AccessControlError::InitializationFailed)?;
    if caller != admin {
        return Err(AccessControlError::AdminRequired);
    }
    env.storage()
        .persistent()
        .set(&AccessControlKey::EmergencyMode, &enabled);

    env.events()
        .publish((symbol_short!("rbac"), symbol_short!("emrg_mode")), enabled);
    Ok(())
}

/// Check whether emergency mode is active.
pub fn is_emergency_mode(env: &Env) -> bool {
    env.storage()
        .persistent()
        .get(&AccessControlKey::EmergencyMode)
        .unwrap_or(false)
}

/// Designate or undesignate an emergency role.
pub fn set_emergency_role(
    env: &Env,
    caller: Address,
    role: Symbol,
    is_emergency: bool,
) -> Result<(), AccessControlError> {
    caller.require_auth();
    let admin = get_admin(env).ok_or(AccessControlError::InitializationFailed)?;
    if caller != admin {
        return Err(AccessControlError::AdminRequired);
    }
    register_known_role(env, &role);
    env.storage()
        .persistent()
        .set(&AccessControlKey::EmergencyRole(role), &is_emergency);

    Ok(())
}

/// Check whether a role is an emergency role.
pub fn is_emergency_role(env: &Env, role: &Symbol) -> bool {
    env.storage()
        .persistent()
        .get(&AccessControlKey::EmergencyRole(role.clone()))
        .unwrap_or(false)
}

/// Delegate role permissions to another address until expiry_ledger.
pub fn delegate_role(
    env: &Env,
    delegator: Address,
    delegatee: Address,
    role: Symbol,
    expiry_ledger: u32,
) -> Result<(), AccessControlError> {
    delegator.require_auth();
    if !has_role(env, &role, &delegator) {
        return Err(AccessControlError::UnauthorizedRole);
    }
    if expiry_ledger <= env.ledger().sequence() {
        return Err(AccessControlError::InvalidExpiry);
    }

    let record = DelegatedRoleRecord {
        delegator: delegator.clone(),
        delegatee: delegatee.clone(),
        role: role.clone(),
        expiry_ledger,
        active: true,
    };

    env.storage().persistent().set(
        &AccessControlKey::RoleDelegation(role.clone(), delegatee.clone()),
        &record,
    );

    env.events().publish(
        (symbol_short!("rbac"), symbol_short!("delegate")),
        (delegator, delegatee, role, expiry_ledger),
    );
    Ok(())
}

/// Revoke a role delegation.
pub fn revoke_delegation(
    env: &Env,
    delegator: Address,
    delegatee: Address,
    role: Symbol,
) -> Result<(), AccessControlError> {
    delegator.require_auth();
    env.storage()
        .persistent()
        .remove(&AccessControlKey::RoleDelegation(
            role.clone(),
            delegatee.clone(),
        ));

    env.events().publish(
        (symbol_short!("rbac"), symbol_short!("rev_del")),
        (delegator, delegatee, role),
    );
    Ok(())
}

/// Initialize role templates (admin -> operator -> readonly).
pub fn init_role_templates(env: &Env, admin: Address) -> Result<(), AccessControlError> {
    admin.require_auth();
    let current_admin = get_admin(env).ok_or(AccessControlError::InitializationFailed)?;
    if admin != current_admin {
        return Err(AccessControlError::AdminRequired);
    }

    let readonly = symbol_short!("readonly");
    let operator = symbol_short!("operator");
    let admin_r = admin_role();

    register_known_role(env, &readonly);
    register_known_role(env, &operator);
    register_known_role(env, &admin_r);

    // Readonly permissions
    set_permission(env, &readonly, &symbol_short!("read"));
    set_permission(env, &readonly, &symbol_short!("query"));

    // Operator permissions
    set_permission(env, &operator, &symbol_short!("operate"));
    set_permission(env, &operator, &symbol_short!("pause"));

    // Admin permissions
    set_permission(env, &admin_r, &symbol_short!("admin"));
    set_permission(env, &admin_r, &symbol_short!("manage"));

    // Hierarchy: Admin -> Operator -> Readonly
    env.storage()
        .persistent()
        .set(&AccessControlKey::RoleParent(admin_r), &operator);
    env.storage()
        .persistent()
        .set(&AccessControlKey::RoleParent(operator), &readonly);

    Ok(())
}
