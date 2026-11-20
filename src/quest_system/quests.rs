use super::types::*;
use crate::player_profile;
use crate::reputation;
use crate::rewards;
use soroban_sdk::{symbol_short, Address, Env, Symbol, Vec};

///
/// Nodes are added afterwards with [`add_quest_node`]; the first node added
/// becomes the chain's root.
pub fn define_chain(env: &Env, creator: Address, name: Symbol) -> Result<u64, QuestError> {
    creator.require_auth();

    let chain_id: u64 = env
        .storage()
        .persistent()
        .get::<QuestKey, u64>(&QuestKey::ChainCounter)
        .unwrap_or(0)
        .saturating_add(1);
    env.storage()
        .persistent()
        .set(&QuestKey::ChainCounter, &chain_id);

    let chain = QuestChain {
        chain_id,
        name: name.clone(),
        creator: creator.clone(),
        root_quest_id: CHAIN_TERMINUS,
        node_count: 0,
    };
    env.storage()
        .persistent()
        .set(&QuestKey::Chain(chain_id), &chain);

    env.events().publish(
        (symbol_short!("quest"), symbol_short!("chaindef")),
        (creator, chain_id, name),
    );

    Ok(chain_id)
}

/// Append a node to `chain_id` and return its quest ID.
///
/// `branches` may reference quest IDs that do not exist yet — chains are
/// authored root-first, so forward edges are resolved when the target node is
/// added. [`validate_chain`] checks that no dangling edge remains.
#[allow(clippy::too_many_arguments)]
pub fn add_quest_node(
    env: &Env,
    creator: Address,
    chain_id: u64,
    objective: Symbol,
    target_count: u32,
    reward: QuestReward,
    narrative_tier: Symbol,
    title: String,
    description: String,
    branches: Vec<QuestBranch>,
    duration_secs: u64,
) -> Result<u64, QuestError> {
    creator.require_auth();

    let mut chain: QuestChain = env
        .storage()
        .persistent()
        .get(&QuestKey::Chain(chain_id))
        .ok_or(QuestError::ChainNotFound)?;

    if chain.creator != creator {
        return Err(QuestError::Unauthorized);
    }
    if chain.node_count >= MAX_CHAIN_LENGTH {
        return Err(QuestError::ChainFull);
    }
    if branches.len() > MAX_BRANCHES_PER_NODE {
        return Err(QuestError::TooManyBranches);
    }
    if target_count == 0 {
        return Err(QuestError::InvalidTarget);
    }
    // A zero choice_id would collide with the "no branch chosen" sentinel in
    // `QuestState::chosen_branch`.
    for branch in branches.iter() {
        if branch.choice_id == 0 {
            return Err(QuestError::InvalidBranch);
        }
    }

    let quest_id: u64 = env
        .storage()
        .persistent()
        .get::<QuestKey, u64>(&QuestKey::NodeCounter)
        .unwrap_or(0)
        .saturating_add(1);
    env.storage()
        .persistent()
        .set(&QuestKey::NodeCounter, &quest_id);

    let node = QuestNode {
        quest_id,
        chain_id,
        step_index: chain.node_count,
        objective: objective.clone(),
        target_count,
        reward,
        narrative_tier,
        title,
        description,
        branches,
        duration_secs,
    };
    env.storage()
        .persistent()
        .set(&QuestKey::Node(quest_id), &node);

    chain.node_count = chain.node_count.saturating_add(1);
    if chain.root_quest_id == CHAIN_TERMINUS {
        chain.root_quest_id = quest_id;
    }
    env.storage()
        .persistent()
        .set(&QuestKey::Chain(chain_id), &chain);

    env.events().publish(
        (symbol_short!("quest"), symbol_short!("nodeadd")),
        (chain_id, quest_id, objective),
    );

    Ok(quest_id)
}

/// Check that every branch in `chain_id` points at an existing node.
///
/// Authors should call this once a chain is fully written; it is not enforced at
/// node-insert time because forward references are legitimate while authoring.
pub fn validate_chain(env: &Env, chain_id: u64) -> Result<(), QuestError> {
    let chain: QuestChain = env
        .storage()
        .persistent()
        .get(&QuestKey::Chain(chain_id))
        .ok_or(QuestError::ChainNotFound)?;

    // Node IDs are globally sequential, so walking from the root over the
    // chain's node count covers exactly this chain's nodes.
    let first = chain.root_quest_id;
    for quest_id in first..first.saturating_add(chain.node_count as u64) {
        let node: QuestNode = match env.storage().persistent().get(&QuestKey::Node(quest_id)) {
            Some(node) => node,
            None => continue,
        };
        if node.chain_id != chain_id {
            continue;
        }
        for branch in node.branches.iter() {
            if branch.next_quest_id != CHAIN_TERMINUS
                && !env
                    .storage()
                    .persistent()
                    .has(&QuestKey::Node(branch.next_quest_id))
            {
                return Err(QuestError::DanglingBranch);
            }
        }
    }

    Ok(())
}

// ─── Active-quest bookkeeping ─────────────────────────────────────────────────

fn active_quests(env: &Env, player: &Address) -> Vec<u64> {
    env.storage()
        .persistent()
        .get(&QuestKey::ActiveQuests(player.clone()))
        .unwrap_or_else(|| Vec::new(env))
}

fn push_active(env: &Env, player: &Address, quest_id: u64) -> Result<(), QuestError> {
    let mut active = active_quests(env, player);
    if active.len() >= MAX_ACTIVE_QUESTS {
        return Err(QuestError::TooManyActiveQuests);
    }
    active.push_back(quest_id);
    env.storage()
        .persistent()
        .set(&QuestKey::ActiveQuests(player.clone()), &active);
    Ok(())
}

fn remove_active(env: &Env, player: &Address, quest_id: u64) {
    let active = active_quests(env, player);
    let mut remaining = Vec::new(env);
    for id in active.iter() {
        if id != quest_id {
            remaining.push_back(id);
        }
    }
    env.storage()
        .persistent()
        .set(&QuestKey::ActiveQuests(player.clone()), &remaining);
}

fn save_state(env: &Env, state: &QuestState) {
    env.storage().persistent().set(
        &QuestKey::QuestState(state.player.clone(), state.quest_id),
        state,
    );
}

fn load_state(env: &Env, player: &Address, quest_id: u64) -> Result<QuestState, QuestError> {
    env.storage()
        .persistent()
        .get(&QuestKey::QuestState(player.clone(), quest_id))
        .ok_or(QuestError::QuestNotStarted)
}

fn load_node(env: &Env, quest_id: u64) -> Result<QuestNode, QuestError> {
    env.storage()
        .persistent()
        .get(&QuestKey::Node(quest_id))
        .ok_or(QuestError::QuestNotFound)
}

/// Activate `node` for `player`, writing state and progress records.
fn activate(env: &Env, player: &Address, node: &QuestNode) -> Result<QuestState, QuestError> {
    let now = env.ledger().timestamp();
    let expires_at = if node.duration_secs == 0 {
        0
    } else {
        now.saturating_add(node.duration_secs)
    };

    let state = QuestState {
        player: player.clone(),
        quest_id: node.quest_id,
        chain_id: node.chain_id,
        status: QuestStatus::Active,
        progress: 0,
        target: node.target_count,
        started_at: now,
        expires_at,
        completed_at: 0,
        chosen_branch: 0,
    };

    push_active(env, player, node.quest_id)?;
    save_state(env, &state);

    env.events().publish(
        (symbol_short!("quest"), symbol_short!("started")),
        (
            player.clone(),
            node.quest_id,
            node.chain_id,
            node.step_index,
        ),
    );

    Ok(state)
}

// ─── Player flow ──────────────────────────────────────────────────────────────

/// Begin `chain_id` for `player`, activating its root node.
pub fn start_chain(env: &Env, player: Address, chain_id: u64) -> Result<QuestState, QuestError> {
    player.require_auth();

    let chain: QuestChain = env
        .storage()
        .persistent()
        .get(&QuestKey::Chain(chain_id))
        .ok_or(QuestError::ChainNotFound)?;

    if chain.root_quest_id == CHAIN_TERMINUS {
        return Err(QuestError::QuestNotFound);
    }
    if env
        .storage()
        .persistent()
        .has(&QuestKey::Progress(player.clone(), chain_id))
    {
        return Err(QuestError::ChainAlreadyStarted);
    }

    let node = load_node(env, chain.root_quest_id)?;
    let state = activate(env, &player, &node)?;

    let progress = ChainProgress {
        player: player.clone(),
        chain_id,
        current_quest_id: node.quest_id,
        steps_completed: 0,
        total_essence_earned: 0,
        path: Vec::new(env),
        finished: false,
    };
    env.storage()
        .persistent()
        .set(&QuestKey::Progress(player.clone(), chain_id), &progress);

    env.events().publish(
        (symbol_short!("quest"), symbol_short!("chainstr")),
        (player, chain_id, node.quest_id),
    );

    Ok(state)
}

/// Add `amount` to a player's progress on an active quest.
///
/// Transitions the quest to `Completed` once the target is met, and to `Expired`
/// if the deadline has already passed. Idempotent once the quest leaves
/// `Active`: further calls return `InvalidStatus` rather than over-counting.
pub fn record_progress(
    env: &Env,
    player: Address,
    quest_id: u64,
    amount: u32,
) -> Result<QuestState, QuestError> {
    player.require_auth();
    record_progress_unchecked(env, &player, quest_id, amount)
}

/// Progress without an auth check, for callers that already authorized `player`.
fn record_progress_unchecked(
    env: &Env,
    player: &Address,
    quest_id: u64,
    amount: u32,
) -> Result<QuestState, QuestError> {
    let mut state = load_state(env, player, quest_id)?;

    if state.status != QuestStatus::Active {
        return Err(QuestError::InvalidStatus);
    }

    let now = env.ledger().timestamp();
    if state.expires_at != 0 && now > state.expires_at {
        state.status = QuestStatus::Expired;
        save_state(env, &state);
        remove_active(env, player, quest_id);

        env.events().publish(
            (symbol_short!("quest"), symbol_short!("expired")),
            (player.clone(), quest_id),
        );
        return Err(QuestError::QuestExpired);
    }

    state.progress = state.progress.saturating_add(amount);

    if state.progress >= state.target {
        state.progress = state.target;
        state.status = QuestStatus::Completed;
        state.completed_at = now;
        remove_active(env, player, quest_id);

        env.events().publish(
            (symbol_short!("quest"), symbol_short!("complete")),
            (player.clone(), quest_id, state.chain_id),
        );
    } else {
        env.events().publish(
            (symbol_short!("quest"), symbol_short!("progress")),
            (player.clone(), quest_id, state.progress, state.target),
        );
    }

    save_state(env, &state);
    Ok(state)
}

/// Claim the reward for a completed quest.
///
/// Credits essence to the player's profile and resources to their balance, then
/// marks the quest `Claimed`. A node whose only branches are
/// [`CHAIN_TERMINUS`] — or which has no branches at all — finishes the chain
/// here.
pub fn claim_quest_reward(
    env: &Env,
    player: Address,
    quest_id: u64,
) -> Result<QuestReward, QuestError> {
    player.require_auth();

    let mut state = load_state(env, &player, quest_id)?;

    match state.status {
        QuestStatus::Completed => {}
        QuestStatus::Claimed => return Err(QuestError::AlreadyClaimed),
        QuestStatus::Expired => return Err(QuestError::QuestExpired),
        _ => return Err(QuestError::InvalidStatus),
    }

    let node = load_node(env, quest_id)?;
    let reward = node.reward.clone();

    // ── Pay out ───────────────────────────────────────────────────────────
    let profile = player_profile::get_profile_by_owner(env, &player)?;
    if reward.essence > 0 {
        player_profile::credit_essence(env, profile.id, reward.essence)?;
    }
    if reward.resource_amount > 0 {
        resource_minter::credit_balance(
            env,
            &player,
            &reward.resource_type,
            reward.resource_amount,
        )
        .map_err(|_| QuestError::ArithmeticOverflow)?;
    }

    state.status = QuestStatus::Claimed;
    save_state(env, &state);

    // ── Chain progress ────────────────────────────────────────────────────
    let mut progress =
        get_chain_progress(env, &player, state.chain_id).ok_or(QuestError::QuestNotStarted)?;
    progress.steps_completed = progress.steps_completed.saturating_add(1);
    progress.total_essence_earned = progress
        .total_essence_earned
        .checked_add(reward.essence)
        .ok_or(QuestError::ArithmeticOverflow)?;
    progress.current_quest_id = quest_id;

    // No onward edge means this path of the narrative ends here.
    let has_successor = node
        .branches
        .iter()
        .any(|b| b.next_quest_id != CHAIN_TERMINUS);
    if !has_successor {
        progress.finished = true;
        env.events().publish(
            (symbol_short!("quest"), symbol_short!("chainend")),
            (player.clone(), state.chain_id, progress.steps_completed),
        );
    }
    env.storage().persistent().set(
        &QuestKey::Progress(player.clone(), state.chain_id),
        &progress,
    );

    env.events().publish(
        (symbol_short!("quest"), symbol_short!("claimed")),
        (player, quest_id, reward.essence, reward.xp),
    );

    Ok(reward)
}

/// Take a branch out of a claimed quest, activating the successor node.
///
/// Returns the newly activated state, or `None` when the chosen branch is a
/// [`CHAIN_TERMINUS`] leaf. The choice is appended to
/// [`ChainProgress::path`], which is the record of the narrative walked.
pub fn choose_branch(
    env: &Env,
    player: Address,
    quest_id: u64,
    choice_id: u32,
) -> Result<Option<QuestState>, QuestError> {
    player.require_auth();

    let mut state = load_state(env, &player, quest_id)?;

    // Branching happens after the reward is taken, so the player cannot skip
    // past a node's payout by racing ahead in the narrative.
    if state.status != QuestStatus::Claimed {
        return Err(QuestError::InvalidStatus);
    }
    if state.chosen_branch != 0 {
        return Err(QuestError::InvalidStatus);
    }

    let node = load_node(env, quest_id)?;
    let branch = node
        .branches
        .iter()
        .find(|b| b.choice_id == choice_id)
        .ok_or(QuestError::InvalidBranch)?;

    state.chosen_branch = choice_id;
    save_state(env, &state);

    let mut progress =
        get_chain_progress(env, &player, state.chain_id).ok_or(QuestError::QuestNotStarted)?;
    progress.path.push_back(choice_id);

    env.events().publish(
        (symbol_short!("quest"), symbol_short!("branch")),
        (
            player.clone(),
            quest_id,
            choice_id,
            branch.label.clone(),
            branch.next_quest_id,
        ),
    );

    if branch.next_quest_id == CHAIN_TERMINUS {
        progress.finished = true;
        env.storage().persistent().set(
            &QuestKey::Progress(player.clone(), state.chain_id),
            &progress,
        );

        env.events().publish(
            (symbol_short!("quest"), symbol_short!("chainend")),
            (player, state.chain_id, progress.steps_completed),
        );
        return Ok(None);
    }

    let next_node = load_node(env, branch.next_quest_id)?;
    let next_state = activate(env, &player, &next_node)?;

    progress.current_quest_id = next_node.quest_id;
    env.storage()
        .persistent()
        .set(&QuestKey::Progress(player, state.chain_id), &progress);

    Ok(Some(next_state))
}

// ─── Mission bridge ───────────────────────────────────────────────────────────

/// Forward a completed mission into the quest system.
///
/// Advances every active quest whose `objective` equals `objective` by
/// `amount`, and returns how many quests were advanced. Errors on individual
/// quests (expired, already complete) are swallowed: a mission completion must
/// never fail because of unrelated quest state.
pub fn on_mission_completed(env: &Env, player: &Address, objective: Symbol, amount: u32) -> u32 {
    let mut advanced = 0u32;

    for quest_id in active_quests(env, player).iter() {
        let node = match env
            .storage()
            .persistent()
            .get::<QuestKey, QuestNode>(&QuestKey::Node(quest_id))
        {
            Some(node) => node,
            None => continue,
        };
        if node.objective != objective {
            continue;
        }
        if record_progress_unchecked(env, player, quest_id, amount).is_ok() {
            advanced = advanced.saturating_add(1);
        }
    }

    if advanced > 0 {
        env.events().publish(
            (symbol_short!("quest"), symbol_short!("misfeed")),
            (player.clone(), objective, advanced),
        );
    }

    advanced
}

// ─── Queries ──────────────────────────────────────────────────────────────────

/// Fetch a quest node definition.
pub fn get_quest_node(env: &Env, quest_id: u64) -> Option<QuestNode> {
    env.storage().persistent().get(&QuestKey::Node(quest_id))
}

/// Fetch a chain definition.
pub fn get_chain(env: &Env, chain_id: u64) -> Option<QuestChain> {
    env.storage().persistent().get(&QuestKey::Chain(chain_id))
}

/// Fetch a player's state for one quest.
pub fn get_quest_state(env: &Env, player: &Address, quest_id: u64) -> Option<QuestState> {
    env.storage()
        .persistent()
        .get(&QuestKey::QuestState(player.clone(), quest_id))
}

/// Fetch a player's progress through one chain.
pub fn get_chain_progress(env: &Env, player: &Address, chain_id: u64) -> Option<ChainProgress> {
    env.storage()
        .persistent()
        .get(&QuestKey::Progress(player.clone(), chain_id))
}

/// Quest IDs the player currently has active.
pub fn get_active_quests(env: &Env, player: &Address) -> Vec<u64> {
    active_quests(env, player)
}

/// Full state records for the player's active quests.
pub fn get_active_quest_states(env: &Env, player: &Address) -> Vec<QuestState> {
    let mut states = Vec::new(env);
    for quest_id in active_quests(env, player).iter() {
        if let Some(state) = get_quest_state(env, player, quest_id) {
            states.push_back(state);
        }
    }
    states
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────────────────────
