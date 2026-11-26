use soroban_sdk::{contracterror, contracttype, symbol_short, Address, Env, Vec};

use crate::alliance_manager::{
    add_alliance_xp, credit_alliance_treasury, get_alliance, get_alliance_treasury, AllianceKey,
};
use crate::player_profile;

// ─── Constants ─────────────────────────────────────────────────────────────

pub const PROPOSAL_DURATION_SECS: u64 = 86_400 * 3; // 3 days

/// Highest guild tax rate a founder can set: 25% of each tribute.
pub const MAX_GUILD_TAX_BPS: u32 = 2_500;
/// 100% in basis points.
pub const GUILD_BPS_DENOMINATOR: u32 = 10_000;

// ─── Data Types ───────────────────────────────────────────────────────────────

#[derive(Clone, Debug, PartialEq)]
#[contracttype]
pub struct TreasuryProposal {
    pub proposal_id: u64,
    pub alliance_id: u64,
    pub proposer: Address,
    pub target_recipient: Address,
    pub amount: i128,
    pub votes_for: u32,
    pub votes_against: u32,
    pub expires_at: u64,
    pub executed: bool,
}

/// Exactly how a completed guild quest's essence is split. Calculated by the
/// quest module (so the bps math stays testable there) and recorded here before
/// any treasury or profile balances move.
#[derive(Clone, Debug, PartialEq)]
#[contracttype]
pub struct QuestRewardSplit {
    pub gross_essence: i128,
    pub tier_bps: u32,
    pub scaled_essence: i128,
    pub treasury_amount: i128,
    pub member_pool: i128,
    pub per_member: i128,
    /// Remainder from `member_pool / member_count`, credited to the first
    /// contributor so no essence is lost to integer division.
    pub remainder_to_first: i128,
    pub xp: u64,
}

/// Immutable record of one quest's payout. Members claim their share through
/// [`claim_quest_reward_share`]; the treasury portion is credited at recording
/// time and is spendable through the normal proposal flow.
#[derive(Clone, Debug, PartialEq)]
#[contracttype]
pub struct QuestRewardPayout {
    pub alliance_id: u64,
    pub quest_id: u64,
    pub gross_essence: i128,
    pub scaled_essence: i128,
    pub treasury_amount: i128,
    pub member_pool: i128,
    pub per_member: i128,
    pub remainder_to_first: i128,
    pub member_count: u32,
    pub claimed_count: u32,
    pub xp: u64,
    pub recorded_at: u64,
}

#[derive(Clone)]
#[contracttype]
pub enum GuildEconomyKey {
    Proposal(u64, u64),        // (alliance_id, proposal_id) -> TreasuryProposal
    ProposalCount(u64),        // alliance_id -> u64
    MemberVoted(u64, u64, Address), // (alliance_id, proposal_id, player) -> bool
    QuestPayout(u64, u64), // (alliance_id, quest_id) -> QuestRewardPayout
    QuestClaim(u64, u64, Address), // (alliance_id, quest_id, player) -> bool
    GuildTaxRate(u64),             // alliance_id -> tax rate in bps
}

#[contracterror]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum GuildEconomyError {
    ProposalNotFound = 1,
    AlreadyVoted = 2,
    ProposalExpired = 3,
    ProposalNotExpired = 4,
    InsufficientTreasuryFunds = 5,
    ProposalAlreadyExecuted = 6,
    NotAllianceMember = 7,
    ThresholdNotMet = 8,
    /// No payout was recorded for this quest.
    QuestPayoutNotFound = 9,
    /// The caller has already taken their share, or every share is claimed.
    QuestRewardAlreadyClaimed = 10,
    /// The claimant has no profile to credit essence to.
    QuestRewardProfileNotFound = 11,
    /// A quest payout had no contributors or invalid amounts.
    InvalidQuestReward = 12,
    /// Tax rate above `MAX_GUILD_TAX_BPS`, or a non-positive tribute.
    InvalidTax = 13,
    /// Only the alliance founder can change the tax rate.
    NotFounder = 14,
}

/// How one tribute payment was split between the guild treasury and the payer.
#[derive(Clone, Debug, PartialEq)]
#[contracttype]
pub struct TributeReceipt {
    pub alliance_id: u64,
    pub gross: i128,
    pub tax: i128,
    pub net: i128,
    pub tax_bps: u32,
    pub treasury_after: i128,
}

impl crate::error_standard::StandardContractError for GuildEconomyError {
    fn descriptor(self) -> crate::error_standard::ErrorDescriptor {
        crate::error_standard::ErrorDescriptor {
            module: "guild_economy",
            code: self as u32,
            kind: crate::error_standard::ErrorKind::Validation,
            retryable: false,
        }
    }
}


// ─── Functions ────────────────────────────────────────────────────────────────

/// Create a proposal to spend/withdraw essence from the alliance treasury.
pub fn create_treasury_proposal(
    env: &Env,
    proposer: Address,
    alliance_id: u64,
    target_recipient: Address,
    amount: i128,
) -> Result<u64, GuildEconomyError> {
    proposer.require_auth();

    // Verify proposer is in this alliance
    let alliance = get_alliance(env, alliance_id).map_err(|_| GuildEconomyError::NotAllianceMember)?;
    let mut is_member = false;
    for member in alliance.members.iter() {
        if member == proposer {
            is_member = true;
            break;
        }
    }
    if !is_member {
        return Err(GuildEconomyError::NotAllianceMember);
    }

    // Verify treasury has enough funds
    let treasury_bal = get_alliance_treasury(env, alliance_id);
    if treasury_bal < amount {
        return Err(GuildEconomyError::InsufficientTreasuryFunds);
    }

    // Generate proposal ID
    let count_key = GuildEconomyKey::ProposalCount(alliance_id);
    let proposal_id = env.storage().persistent().get(&count_key).unwrap_or(0u64) + 1;
    env.storage().persistent().set(&count_key, &proposal_id);

    let expires_at = env.ledger().timestamp() + PROPOSAL_DURATION_SECS;

    let proposal = TreasuryProposal {
        proposal_id,
        alliance_id,
        proposer: proposer.clone(),
        target_recipient,
        amount,
        votes_for: 1, // Proposer votes FOR automatically
        votes_against: 0,
        expires_at,
        executed: false,
    };

    let prop_key = GuildEconomyKey::Proposal(alliance_id, proposal_id);
    env.storage().persistent().set(&prop_key, &proposal);

    // Record proposer's vote
    let vote_key = GuildEconomyKey::MemberVoted(alliance_id, proposal_id, proposer.clone());
    env.storage().persistent().set(&vote_key, &true);

    env.events().publish(
        (symbol_short!("guild_ec"), symbol_short!("prop_new")),
        (alliance_id, proposal_id, proposer, amount),
    );

    Ok(proposal_id)
}

/// Cast a vote on a treasury spending proposal.
pub fn vote_on_proposal(
    env: &Env,
    voter: Address,
    alliance_id: u64,
    proposal_id: u64,
    approve: bool,
) -> Result<(), GuildEconomyError> {
    voter.require_auth();

    // Verify voter is in alliance
    let alliance = get_alliance(env, alliance_id).map_err(|_| GuildEconomyError::NotAllianceMember)?;
    let mut is_member = false;
    for member in alliance.members.iter() {
        if member == voter {
            is_member = true;
            break;
        }
    }
    if !is_member {
        return Err(GuildEconomyError::NotAllianceMember);
    }

    let prop_key = GuildEconomyKey::Proposal(alliance_id, proposal_id);
    let mut proposal: TreasuryProposal = env
        .storage()
        .persistent()
        .get(&prop_key)
        .ok_or(GuildEconomyError::ProposalNotFound)?;

    if proposal.executed {
        return Err(GuildEconomyError::ProposalAlreadyExecuted);
    }

    if env.ledger().timestamp() > proposal.expires_at {
        return Err(GuildEconomyError::ProposalExpired);
    }

    let vote_key = GuildEconomyKey::MemberVoted(alliance_id, proposal_id, voter.clone());
    if env.storage().persistent().has(&vote_key) {
        return Err(GuildEconomyError::AlreadyVoted);
    }

    env.storage().persistent().set(&vote_key, &true);

    if approve {
        proposal.votes_for += 1;
    } else {
        proposal.votes_against += 1;
    }

    env.storage().persistent().set(&prop_key, &proposal);

    env.events().publish(
        (symbol_short!("guild_ec"), symbol_short!("prop_vote")),
        (alliance_id, proposal_id, voter, approve),
    );

    Ok(())
}

/// Execute a passed proposal, transferring funds from treasury.
pub fn execute_proposal(
    env: &Env,
    caller: Address,
    alliance_id: u64,
    proposal_id: u64,
) -> Result<(), GuildEconomyError> {
    caller.require_auth();

    let prop_key = GuildEconomyKey::Proposal(alliance_id, proposal_id);
    let mut proposal: TreasuryProposal = env
        .storage()
        .persistent()
        .get(&prop_key)
        .ok_or(GuildEconomyError::ProposalNotFound)?;

    if proposal.executed {
        return Err(GuildEconomyError::ProposalAlreadyExecuted);
    }

    // Check voting threshold
    let alliance = get_alliance(env, alliance_id).map_err(|_| GuildEconomyError::NotAllianceMember)?;
    let total_members = alliance.members.len();

    // Threshold calculation: votes_for / total_members >= threshold %
    // votes_for * 100 >= total_members * threshold
    let threshold = alliance.voting_threshold as u32;
    if (proposal.votes_for * 100) < (total_members * threshold) {
        // If not expired, keep waiting. If expired, reject execution.
        if env.ledger().timestamp() <= proposal.expires_at {
            return Err(GuildEconomyError::ThresholdNotMet);
        } else {
            return Err(GuildEconomyError::ProposalExpired);
        }
    }

    // Perform treasury reduction
    let treasury_key = AllianceKey::AllianceTreasury(alliance_id);
    let current_treasury = env
        .storage()
        .persistent()
        .get::<AllianceKey, i128>(&treasury_key)
        .unwrap_or(0);

    if current_treasury < proposal.amount {
        return Err(GuildEconomyError::InsufficientTreasuryFunds);
    }

    let new_treasury = current_treasury.saturating_sub(proposal.amount);
    env.storage().persistent().set(&treasury_key, &new_treasury);

    proposal.executed = true;
    env.storage().persistent().set(&prop_key, &proposal);

    // Emit treasury payout / transfer event
    env.events().publish(
        (symbol_short!("guild_ec"), symbol_short!("prop_exec")),
        (alliance_id, proposal_id, proposal.target_recipient, proposal.amount),
    );

    Ok(())
}

/// Splits a tribute into `(tax, net)` at `tax_bps`, rounding the tax down.
pub fn split_guild_tribute(gross: i128, tax_bps: u32) -> Option<(i128, i128)> {
    if gross < 0 || tax_bps > GUILD_BPS_DENOMINATOR {
        return None;
    }
    let tax = gross.checked_mul(i128::from(tax_bps))? / i128::from(GUILD_BPS_DENOMINATOR);
    Some((tax, gross - tax))
}

/// Sets the guild tax charged on tributes. Founder only, capped at `MAX_GUILD_TAX_BPS`.
pub fn set_guild_tax_rate(
    env: &Env,
    founder: Address,
    alliance_id: u64,
    tax_bps: u32,
) -> Result<(), GuildEconomyError> {
    founder.require_auth();
    let alliance =
        get_alliance(env, alliance_id).map_err(|_| GuildEconomyError::NotAllianceMember)?;
    if alliance.founder != founder {
        return Err(GuildEconomyError::NotFounder);
    }
    if tax_bps > MAX_GUILD_TAX_BPS {
        return Err(GuildEconomyError::InvalidTax);
    }
    env.storage()
        .persistent()
        .set(&GuildEconomyKey::GuildTaxRate(alliance_id), &tax_bps);
    env.events().publish(
        (symbol_short!("guild_ec"), symbol_short!("tax_set")),
        (alliance_id, tax_bps),
    );
    Ok(())
}

/// Current guild tax rate in bps (0 until the founder sets one).
pub fn get_guild_tax_rate(env: &Env, alliance_id: u64) -> u32 {
    env.storage()
        .persistent()
        .get(&GuildEconomyKey::GuildTaxRate(alliance_id))
        .unwrap_or(0)
}

/// A member pays tribute on `gross` earnings: the guild tax goes to the
/// alliance treasury (spendable through treasury proposals) and the rest stays
/// with the member. Returns the split.
pub fn pay_guild_tribute(
    env: &Env,
    member: Address,
    alliance_id: u64,
    gross: i128,
) -> Result<TributeReceipt, GuildEconomyError> {
    member.require_auth();
    if gross <= 0 {
        return Err(GuildEconomyError::InvalidTax);
    }
    if !is_alliance_member(env, alliance_id, &member) {
        return Err(GuildEconomyError::NotAllianceMember);
    }
    let tax_bps = get_guild_tax_rate(env, alliance_id);
    let (tax, net) = split_guild_tribute(gross, tax_bps).ok_or(GuildEconomyError::InvalidTax)?;
    let treasury_after = credit_alliance_treasury(env, alliance_id, tax)
        .map_err(|_| GuildEconomyError::NotAllianceMember)?;
    env.events().publish(
        (symbol_short!("guild_ec"), symbol_short!("tribute")),
        (alliance_id, member, gross, tax),
    );
    Ok(TributeReceipt {
        alliance_id,
        gross,
        tax,
        net,
        tax_bps,
        treasury_after,
    })
}

/// Get details of a treasury spending proposal.
pub fn get_proposal(
    env: &Env,
    alliance_id: u64,
    proposal_id: u64,
) -> Option<TreasuryProposal> {
    let prop_key = GuildEconomyKey::Proposal(alliance_id, proposal_id);
    env.storage().persistent().get(&prop_key)
}

// ─── Guild quest reward integration (Issue #595) ─────────────────────────────

fn is_alliance_member(env: &Env, alliance_id: u64, player: &Address) -> bool {
    let alliance = match get_alliance(env, alliance_id) {
        Ok(alliance) => alliance,
        Err(_) => return false,
    };
    for member in alliance.members.iter() {
        if member == player.clone() {
            return true;
        }
    }
    false
}

/// Record and fund a completed guild quest's payout.
///
/// The treasury share is credited immediately (and is therefore spendable via
/// [`create_treasury_proposal`]); member shares are recorded as claimable so a
/// contributor without a profile can claim later. Calling this twice for the
/// same quest is a no-op — the stored payout is returned unchanged.
pub fn record_quest_rewards(
    env: &Env,
    alliance_id: u64,
    quest_id: u64,
    split: &QuestRewardSplit,
    contributors: &Vec<Address>,
) -> Result<QuestRewardPayout, GuildEconomyError> {
    let payout_key = GuildEconomyKey::QuestPayout(alliance_id, quest_id);
    if let Some(existing) = env.storage().persistent().get(&payout_key) {
        return Ok(existing);
    }

    let member_count = contributors.len();
    if member_count == 0 || split.scaled_essence < 0 {
        return Err(GuildEconomyError::InvalidQuestReward);
    }

    credit_alliance_treasury(env, alliance_id, split.treasury_amount)
        .map_err(|_| GuildEconomyError::InvalidQuestReward)?;
    // Alliance XP has no failure path; the level is derived from the stored XP.
    let _ = add_alliance_xp(env, alliance_id, split.xp);

    let payout = QuestRewardPayout {
        alliance_id,
        quest_id,
        gross_essence: split.gross_essence,
        scaled_essence: split.scaled_essence,
        treasury_amount: split.treasury_amount,
        member_pool: split.member_pool,
        per_member: split.per_member,
        remainder_to_first: split.remainder_to_first,
        member_count,
        claimed_count: 0,
        xp: split.xp,
        recorded_at: env.ledger().timestamp(),
    };
    env.storage().persistent().set(&payout_key, &payout);

    env.events().publish(
        (symbol_short!("guild_ec"), symbol_short!("questpay")),
        (
            alliance_id,
            quest_id,
            split.treasury_amount,
            split.member_pool,
        ),
    );

    Ok(payout)
}

/// Claim a contributor's share of a completed guild quest.
///
/// The first claimant receives the integer-division remainder so the sum of
/// all claims equals `member_pool` exactly.
pub fn claim_quest_reward_share(
    env: &Env,
    player: Address,
    alliance_id: u64,
    quest_id: u64,
) -> Result<i128, GuildEconomyError> {
    player.require_auth();

    let payout_key = GuildEconomyKey::QuestPayout(alliance_id, quest_id);
    let mut payout: QuestRewardPayout = env
        .storage()
        .persistent()
        .get(&payout_key)
        .ok_or(GuildEconomyError::QuestPayoutNotFound)?;

    let claim_key = GuildEconomyKey::QuestClaim(alliance_id, quest_id, player.clone());
    if env.storage().persistent().has(&claim_key)
        || payout.claimed_count >= payout.member_count
    {
        return Err(GuildEconomyError::QuestRewardAlreadyClaimed);
    }
    if !is_alliance_member(env, alliance_id, &player) {
        return Err(GuildEconomyError::NotAllianceMember);
    }

    let profile = player_profile::get_profile_by_owner(env, &player)
        .map_err(|_| GuildEconomyError::QuestRewardProfileNotFound)?;
    let is_first = payout.claimed_count == 0;
    let mut amount = payout.per_member;
    if is_first {
        amount = amount.saturating_add(payout.remainder_to_first);
    }
    player_profile::credit_essence(env, profile.id, amount)
        .map_err(|_| GuildEconomyError::QuestRewardProfileNotFound)?;

    env.storage().persistent().set(&claim_key, &true);
    payout.claimed_count = payout.claimed_count.saturating_add(1);
    env.storage().persistent().set(&payout_key, &payout);

    env.events().publish(
        (symbol_short!("guild_ec"), symbol_short!("qclaim")),
        (alliance_id, quest_id, player, amount),
    );

    Ok(amount)
}

/// Read the recorded payout for a guild quest.
pub fn get_quest_reward_payout(
    env: &Env,
    alliance_id: u64,
    quest_id: u64,
) -> Option<QuestRewardPayout> {
    env.storage()
        .persistent()
        .get(&GuildEconomyKey::QuestPayout(alliance_id, quest_id))
}
