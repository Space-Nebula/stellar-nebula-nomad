use super::types::*;
use super::rbac::*;
use soroban_sdk::{Address, Env, Symbol, Vec};

pub fn init_multisig(
    env: &Env,
    admin: Address,
    signers: Vec<Address>,
    required_approvals: u32,
    timelock_period: u64,
) -> Result<(), AccessControlError> {
    admin.require_auth();

    let stored_admin = get_admin(env).ok_or(AccessControlError::InitializationFailed)?;
    if admin != stored_admin {
        return Err(AccessControlError::AdminRequired);
    }

    if signers.len() == 0 || required_approvals == 0 || required_approvals > signers.len() {
        return Err(AccessControlError::InvalidSignerConfig);
    }

    let config = MultiSigConfig {
        required_approvals,
        total_signers: signers.len(),
        timelock_period,
        enabled: true,
    };

    env.storage()
        .persistent()
        .set(&AccessControlKey::MultiSigConfig, &config);

    env.storage()
        .persistent()
        .set(&AccessControlKey::AdminSigners, &signers);

    env.events().publish(
        (symbol_short!("multisig"), symbol_short!("init")),
        (required_approvals, signers.len(), timelock_period),
    );

    Ok(())
}

/// Create a proposal for a multi-sig admin operation.
///
/// # Authorization
/// - Caller must be one of the configured signers.
///
/// # Returns
/// - Proposal ID if successful.
pub fn create_proposal(
    env: &Env,
    proposer: Address,
    operation: ProposalOperation,
) -> Result<u64, AccessControlError> {
    proposer.require_auth();

    let config: MultiSigConfig = env
        .storage()
        .persistent()
        .get(&AccessControlKey::MultiSigConfig)
        .ok_or(AccessControlError::MultiSigNotConfigured)?;

    if !config.enabled {
        return Err(AccessControlError::MultiSigNotConfigured);
    }

    // Verify proposer is a signer
    let signers: Vec<Address> = env
        .storage()
        .persistent()
        .get(&AccessControlKey::AdminSigners)
        .ok_or(AccessControlError::MultiSigNotConfigured)?;

    let mut is_signer = false;
    for i in 0..signers.len() {
        if let Some(signer) = signers.get(i) {
            if signer == proposer {
                is_signer = true;
                break;
            }
        }
    }

    if !is_signer {
        return Err(AccessControlError::AdminRequired);
    }

    let proposal_id = next_proposal_id(env);
    let now = env.ledger().timestamp();
    let execution_time = now + config.timelock_period;

    let proposal = Proposal {
        proposal_id,
        proposer: proposer.clone(),
        operation,
        created_at: now,
        execution_time,
        executed: false,
        approval_count: 1, // Proposer auto-approves
    };

    env.storage().persistent().set(
        &AccessControlKey::AccessControlProposal(proposal_id),
        &proposal,
    );

    // Record proposer's approval
    env.storage().persistent().set(
        &AccessControlKey::ProposalApproval(proposal_id, proposer.clone()),
        &true,
    );

    env.events().publish(
        (symbol_short!("multisig"), symbol_short!("propose")),
        (proposal_id, proposer, execution_time),
    );

    Ok(proposal_id)
}

/// Approve a pending proposal.
///
/// # Authorization
/// - Caller must be one of the configured signers.
///
/// # Errors
/// - `ProposalNotFound` if proposal doesn't exist.
/// - `ProposalAlreadyExecuted` if already executed.
/// - `AlreadyApproved` if caller already approved.
pub fn approve_proposal(
    env: &Env,
    approver: Address,
    proposal_id: u64,
) -> Result<(), AccessControlError> {
    approver.require_auth();

    let _config: MultiSigConfig = env
        .storage()
        .persistent()
        .get(&AccessControlKey::MultiSigConfig)
        .ok_or(AccessControlError::MultiSigNotConfigured)?;
    let signers: Vec<Address> = env
        .storage()
        .persistent()
        .get(&AccessControlKey::AdminSigners)
        .ok_or(AccessControlError::MultiSigNotConfigured)?;

    let mut is_signer = false;
    for i in 0..signers.len() {
        if let Some(signer) = signers.get(i) {
            if signer == approver {
                is_signer = true;
                break;
            }
        }
    }

    if !is_signer {
        return Err(AccessControlError::AdminRequired);
    }

    let mut proposal: Proposal = env
        .storage()
        .persistent()
        .get(&AccessControlKey::AccessControlProposal(proposal_id))
        .ok_or(AccessControlError::ProposalNotFound)?;

    if proposal.executed {
        return Err(AccessControlError::ProposalAlreadyExecuted);
    }

    // Check if already approved
    let already_approved: bool = env
        .storage()
        .persistent()
        .get(&AccessControlKey::ProposalApproval(
            proposal_id,
            approver.clone(),
        ))
        .unwrap_or(false);

    if already_approved {
        return Err(AccessControlError::AlreadyApproved);
    }

    // Record approval
    env.storage().persistent().set(
        &AccessControlKey::ProposalApproval(proposal_id, approver.clone()),
        &true,
    );

    proposal.approval_count += 1;

    env.storage().persistent().set(
        &AccessControlKey::AccessControlProposal(proposal_id),
        &proposal,
    );

    env.events().publish(
        (symbol_short!("multisig"), symbol_short!("approve")),
        (proposal_id, approver, proposal.approval_count),
    );

    Ok(())
}

/// Execute an approved proposal.
///
/// # Errors
/// - `ProposalNotFound` if proposal doesn't exist.
/// - `InsufficientApprovals` if not enough approvals.
/// - `TimelockNotElapsed` if timelock period hasn't passed.
/// - `ProposalAlreadyExecuted` if already executed.
pub fn execute_proposal(
    env: &Env,
    executor: Address,
    proposal_id: u64,
) -> Result<(), AccessControlError> {
    executor.require_auth();

    let config: MultiSigConfig = env
        .storage()
        .persistent()
        .get(&AccessControlKey::MultiSigConfig)
        .ok_or(AccessControlError::MultiSigNotConfigured)?;

    let mut proposal: Proposal = env
        .storage()
        .persistent()
        .get(&AccessControlKey::AccessControlProposal(proposal_id))
        .ok_or(AccessControlError::ProposalNotFound)?;

    if proposal.executed {
        return Err(AccessControlError::ProposalAlreadyExecuted);
    }

    if proposal.approval_count < config.required_approvals {
        return Err(AccessControlError::InsufficientApprovals);
    }

    let now = env.ledger().timestamp();
    if now < proposal.execution_time {
        return Err(AccessControlError::TimelockNotElapsed);
    }

    // Execute the operation
    match &proposal.operation {
        ProposalOperation::GrantRole(role, grantee, expiry) => {
            let record = RoleRecord::new(*expiry);
            set_role_record(env, role, grantee, &record);
            register_known_role(env, role);
        }
        ProposalOperation::RevokeRole(role, revokee) => {
            delete_role_record(env, role, revokee);
        }
        ProposalOperation::GrantPermission(role, action) => {
            set_permission(env, role, action);
        }
        ProposalOperation::RevokePermission(role, action) => {
            delete_permission(env, role, action);
        }
        ProposalOperation::TransferAdmin(new_admin) => {
            set_admin(env, new_admin);
            let admin_sym = admin_role();
            let old_admin = proposal.proposer.clone();
            delete_role_record(env, &admin_sym, &old_admin);
            let record = RoleRecord::new(None);
            set_role_record(env, &admin_sym, new_admin, &record);
        }
        ProposalOperation::UpdateMultiSig(new_config) => {
            env.storage()
                .persistent()
                .set(&AccessControlKey::MultiSigConfig, new_config);
        }
    }

    proposal.executed = true;
    env.storage().persistent().set(
        &AccessControlKey::AccessControlProposal(proposal_id),
        &proposal,
    );

    env.events().publish(
        (symbol_short!("multisig"), symbol_short!("execute")),
        (proposal_id, executor, now),
    );

    Ok(())
}

/// Get proposal details.
pub fn get_proposal(env: &Env, proposal_id: u64) -> Result<Proposal, AccessControlError> {
    env.storage()
        .persistent()
        .get(&AccessControlKey::AccessControlProposal(proposal_id))
        .ok_or(AccessControlError::ProposalNotFound)
}

/// Get multi-sig configuration.
pub fn get_multisig_config(env: &Env) -> Result<MultiSigConfig, AccessControlError> {
    env.storage()
        .persistent()
        .get(&AccessControlKey::MultiSigConfig)
        .ok_or(AccessControlError::MultiSigNotConfigured)
}

/// Get next proposal ID and increment counter.
fn next_proposal_id(env: &Env) -> u64 {
    let current: u64 = env
        .storage()
        .instance()
        .get(&AccessControlKey::AccessControlProposalCounter)
        .unwrap_or(0);
    let next = current + 1;
    env.storage()
        .instance()
        .set(&AccessControlKey::AccessControlProposalCounter, &next);
    next
}

// ═══════════════════════════════════════════════════════════════════════════════
// HIERARCHY, EMERGENCY ROLES, DELEGATION & TEMPLATES
// ═══════════════════════════════════════════════════════════════════════════════
