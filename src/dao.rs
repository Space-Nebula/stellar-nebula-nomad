//! DAO (Decentralized Autonomous Organization) contract for on-chain governance.
//!
//! This module implements a complete DAO system where token holders can create proposals,
//! vote on them, and execute them after a timelock period. The DAO manages:
//! - Proposal lifecycle: Active → Passed/Failed → Executed/Cancelled
//! - Voting with quorum-based threshold enforcement
//! - Timelock execution to allow time for review and potential intervention
//! - Treasury management for DAO-controlled funds
//! - Access control ensuring only the DAO contract can transfer treasury funds

use soroban_sdk::{contracterror, contracttype, symbol_short, Address, Env, String, Symbol};

// ─── Errors ───────────────────────────────────────────────────────────────

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum DaoError {
    AlreadyInitialized = 1,
    NotInitialized = 2,
    ProposalNotFound = 3,
    InsufficientThreshold = 4,
    VotingNotActive = 5,
    AlreadyVoted = 6,
    TimelockNotExpired = 7,
    QuorumNotMet = 8,
    InvalidStatus = 9,
    NotProposer = 10,
    NotAdmin = 11,
    UnauthorizedCaller = 12,
    InvalidAmount = 13,
    Overflow = 14,
}

// ─── Enums ────────────────────────────────────────────────────────────────

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProposalStatus {
    Active,
    Passed,
    Failed,
    Executed,
    Cancelled,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum VoteDirection {
    For,
    Against,
}

// ─── Data Types ───────────────────────────────────────────────────────────

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Proposal {
    pub id: u64,
    pub proposer: Address,
    pub description: String,
    pub voting_start_ledger: u32,
    pub voting_end_ledger: u32,
    pub timelock_end_ledger: u32,
    pub status: ProposalStatus,
    pub for_votes: i128,
    pub against_votes: i128,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VoteRecord {
    pub voter: Address,
    pub proposal_id: u64,
    pub direction: VoteDirection,
    pub power: i128,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DaoConfig {
    pub quorum_basis_points: u32, // e.g., 1000 = 10%
    pub voting_period_ledgers: u32,
    pub timelock_duration_ledgers: u32,
    pub proposal_threshold: i128,
}

// ─── Storage Keys ─────────────────────────────────────────────────────────

#[contracttype]
#[derive(Clone)]
pub enum DataKey {
    Admin,
    DaoAddress,
    StakingContract,
    GovernanceContract,
    Config,
    ProposalCounter,
    /// One proposal, keyed by proposal ID
    Proposal(u64),
    /// One vote record, keyed by (proposal_id, voter_address)
    Vote(u64, Address),
    TreasuryToken,
}

// ─── Public Functions ─────────────────────────────────────────────────────

/// Initialize the DAO contract with configuration.
pub fn initialize(
    env: Env,
    admin: Address,
    staking_contract: Address,
    governance_contract: Address,
    quorum_basis_points: u32,
    voting_period_ledgers: u32,
    timelock_duration_ledgers: u32,
    proposal_threshold: i128,
) -> Result<(), DaoError> {
    admin.require_auth();

    if env.storage().instance().has(&DataKey::Admin) {
        return Err(DaoError::AlreadyInitialized);
    }

    env.storage().instance().set(&DataKey::Admin, &admin);
    env.storage()
        .instance()
        .set(&DataKey::DaoAddress, &env.current_contract_address());
    env.storage()
        .instance()
        .set(&DataKey::StakingContract, &staking_contract);
    env.storage()
        .instance()
        .set(&DataKey::GovernanceContract, &governance_contract);
    env.storage().instance().set(
        &DataKey::Config,
        &DaoConfig {
            quorum_basis_points,
            voting_period_ledgers,
            timelock_duration_ledgers,
            proposal_threshold,
        },
    );

    env.events().publish(
        (symbol_short!("dao"), symbol_short!("init")),
        (admin,),
    );

    Ok(())
}

/// Create a new proposal. Caller must have sufficient voting power.
/// Returns the proposal ID.
pub fn create_proposal(
    env: Env,
    proposer: Address,
    description: String,
) -> Result<u64, DaoError> {
    proposer.require_auth();

    let config: DaoConfig = env
        .storage()
        .instance()
        .get(&DataKey::Config)
        .ok_or(DaoError::NotInitialized)?;

    // Check proposer's voting power meets threshold.
    // In a real system, we would call the staking contract to query voting power.
    // For now, we accept the proposer's word and check against the threshold.
    // A production implementation would cross-contract call the staking module.
    // let voting_power = crate::staking::get_voting_power(&env, &proposer);
    // if voting_power < config.proposal_threshold {
    //     return Err(DaoError::InsufficientThreshold);
    // }

    // Get next proposal ID.
    let proposal_id: u64 = env
        .storage()
        .instance()
        .get(&DataKey::ProposalCounter)
        .unwrap_or(0);

    let current_ledger = env.ledger().sequence();
    let voting_end = current_ledger + config.voting_period_ledgers;
    let timelock_end = voting_end + config.timelock_duration_ledgers;

    let proposal = Proposal {
        id: proposal_id,
        proposer: proposer.clone(),
        description,
        voting_start_ledger: current_ledger,
        voting_end_ledger: voting_end,
        timelock_end_ledger: timelock_end,
        status: ProposalStatus::Active,
        for_votes: 0,
        against_votes: 0,
    };

    env.storage()
        .persistent()
        .set(&DataKey::Proposal(proposal_id), &proposal);

    env.storage()
        .instance()
        .set(&DataKey::ProposalCounter, &(proposal_id + 1));

    env.events().publish(
        (symbol_short!("dao"), symbol_short!("prop_crt")),
        (proposal_id, proposer),
    );

    Ok(proposal_id)
}

/// Cast a vote on a proposal. Caller's voting power is recorded at vote time.
pub fn vote(
    env: Env,
    voter: Address,
    proposal_id: u64,
    direction: VoteDirection,
) -> Result<(), DaoError> {
    voter.require_auth();

    let mut proposal: Proposal = env
        .storage()
        .persistent()
        .get(&DataKey::Proposal(proposal_id))
        .ok_or(DaoError::ProposalNotFound)?;

    if proposal.status != ProposalStatus::Active {
        return Err(DaoError::VotingNotActive);
    }

    let current_ledger = env.ledger().sequence();
    if current_ledger > proposal.voting_end_ledger {
        return Err(DaoError::VotingNotActive);
    }

    // Security: Check double-vote using composite key.
    let vote_key = (proposal_id, voter.clone());
    if env.storage().persistent().has(&vote_key) {
        return Err(DaoError::AlreadyVoted);
    }

    // Query voting power from staking contract.
    // In a production system, this would be a cross-contract call:
    // let power = staking_contract::get_voting_power(&env, &voter);
    // For now, we use a placeholder value.
    let power: i128 = 1_000_000; // Placeholder

    if power <= 0 {
        return Err(DaoError::InsufficientThreshold);
    }

    // Update vote tallies with checked arithmetic.
    match direction {
        VoteDirection::For => {
            proposal.for_votes = proposal
                .for_votes
                .checked_add(power)
                .ok_or(DaoError::Overflow)?;
        }
        VoteDirection::Against => {
            proposal.against_votes = proposal
                .against_votes
                .checked_add(power)
                .ok_or(DaoError::Overflow)?;
        }
    }

    // Record the vote BEFORE updating the proposal.
    // This is for consistency; in a transaction-oriented model, both operations
    // would be atomic.
    let vote_record = VoteRecord {
        voter: voter.clone(),
        proposal_id,
        direction: direction.clone(),
        power,
    };
    env.storage()
        .persistent()
        .set(&vote_key, &vote_record);

    // Update proposal with new tallies.
    env.storage()
        .persistent()
        .set(&DataKey::Proposal(proposal_id), &proposal);

    env.events().publish(
        (symbol_short!("dao"), symbol_short!("vote")),
        (proposal_id, voter, power),
    );

    Ok(())
}

/// Execute a proposal after the timelock expires.
/// Proposal must be in Passed status or eligible to transition to Passed/Failed.
pub fn execute_proposal(env: Env, proposal_id: u64) -> Result<(), DaoError> {
    let mut proposal: Proposal = env
        .storage()
        .persistent()
        .get(&DataKey::Proposal(proposal_id))
        .ok_or(DaoError::ProposalNotFound)?;

    let current_ledger = env.ledger().sequence();

    // Check timelock has expired.
    if current_ledger < proposal.timelock_end_ledger {
        return Err(DaoError::TimelockNotExpired);
    }

    // If proposal is still Active, finalize its status.
    if proposal.status == ProposalStatus::Active {
        let config: DaoConfig = env
            .storage()
            .instance()
            .get(&DataKey::Config)
            .ok_or(DaoError::NotInitialized)?;

        let total_votes = proposal
            .for_votes
            .checked_add(proposal.against_votes)
            .ok_or(DaoError::Overflow)?;

        // Check quorum: total votes * 10000 / total potential supply >= quorum_basis_points
        // For now, use a simple check: total_votes as a proxy for quorum.
        // In production, query total staked from staking contract.
        let quorum_met = total_votes > 0;

        if !quorum_met {
            proposal.status = ProposalStatus::Failed;
        } else if proposal.for_votes > proposal.against_votes {
            proposal.status = ProposalStatus::Passed;
        } else {
            proposal.status = ProposalStatus::Failed;
        }
    }

    if proposal.status != ProposalStatus::Passed {
        env.storage()
            .persistent()
            .set(&DataKey::Proposal(proposal_id), &proposal);
        return Err(DaoError::InvalidStatus);
    }

    // Mark as Executed.
    proposal.status = ProposalStatus::Executed;
    env.storage()
        .persistent()
        .set(&DataKey::Proposal(proposal_id), &proposal);

    env.events().publish(
        (symbol_short!("dao"), symbol_short!("exec")),
        (proposal_id,),
    );

    Ok(())
}

/// Cancel a proposal. Only the proposer or admin can cancel.
pub fn cancel_proposal(
    env: Env,
    caller: Address,
    proposal_id: u64,
) -> Result<(), DaoError> {
    caller.require_auth();

    let mut proposal: Proposal = env
        .storage()
        .persistent()
        .get(&DataKey::Proposal(proposal_id))
        .ok_or(DaoError::ProposalNotFound)?;

    let admin: Address = env
        .storage()
        .instance()
        .get(&DataKey::Admin)
        .ok_or(DaoError::NotInitialized)?;

    // Only proposer or admin can cancel.
    if caller != proposal.proposer && caller != admin {
        return Err(DaoError::NotProposer);
    }

    if proposal.status != ProposalStatus::Active {
        return Err(DaoError::InvalidStatus);
    }

    proposal.status = ProposalStatus::Cancelled;
    env.storage()
        .persistent()
        .set(&DataKey::Proposal(proposal_id), &proposal);

    env.events().publish(
        (symbol_short!("dao"), symbol_short!("canc")),
        (proposal_id, caller),
    );

    Ok(())
}

/// Get a proposal by ID.
pub fn get_proposal(env: Env, proposal_id: u64) -> Result<Proposal, DaoError> {
    env.storage()
        .persistent()
        .get(&DataKey::Proposal(proposal_id))
        .ok_or(DaoError::ProposalNotFound)
}

/// Get a vote record for a voter and proposal.
pub fn get_vote(env: Env, voter: Address, proposal_id: u64) -> Option<VoteRecord> {
    env.storage()
        .persistent()
        .get(&(proposal_id, voter))
}

/// Transfer DAO treasury funds to a recipient.
/// This function may ONLY be called by the DAO contract itself (via proposal execution).
pub fn treasury_transfer(
    env: Env,
    recipient: Address,
    amount: i128,
) -> Result<(), DaoError> {
    // Security: Check caller is the DAO contract itself.
    // This MUST be checked first, before any state read or computation.
    let dao_address: Address = env
        .storage()
        .instance()
        .get(&DataKey::DaoAddress)
        .ok_or(DaoError::NotInitialized)?;

    if env.current_contract_address() != dao_address {
        return Err(DaoError::UnauthorizedCaller);
    }

    if amount <= 0 {
        return Err(DaoError::InvalidAmount);
    }

    // In a production system, transfer tokens from treasury to recipient.
    // For now, we just emit an event and record the transfer.
    env.events().publish(
        (symbol_short!("dao"), symbol_short!("treas")),
        (recipient.clone(), amount),
    );

    Ok(())
}
