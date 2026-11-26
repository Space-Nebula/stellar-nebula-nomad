//! Quest system — Issue #282
//!
//! Guided, multi-step content on top of the one-shot dailies produced by
//! [`crate::mission_generator`]. Where a mission is a single objective that
//! expires in a day, a quest is a **node in a chain**: completing it unlocks
//! one of several successors, so a chain describes a branching narrative rather
//! than a fixed list.
//!
//! ```text
//!                       ┌── choice 1 ──► node 2 ──► node 4
//!   chain root: node 1 ─┤
//!                       └── choice 2 ──► node 3 ──► (end)
//! ```
//!
//! Lifecycle of one node for one player:
//!
//! ```text
//!   Locked ──start/choose──► Active ──progress reaches target──► Completed
//!                              │                                    │
//!                              └────────── expires ──────► Expired  ├─claim─► Claimed
//!                                                                   │
//!                                                       choose_branch └──► next node Active
//! ```
//!
//! Progress can be recorded directly or fed in from the mission system: a
//! completed mission calls [`on_mission_completed`], which advances every
//! active quest whose objective matches the mission type. That means normal
//! play drives quest chains forward without the player tracking two systems.

use soroban_sdk::{contracterror, contracttype, symbol_short, Address, Env, String, Symbol, Vec};

use crate::player_profile::{self, ProfileError};
use crate::resource_minter::{self, ResourceType};

// ─── Constants ────────────────────────────────────────────────────────────────

/// Maximum nodes in a single chain — bounds the storage a chain can occupy.
pub const MAX_CHAIN_LENGTH: u32 = 20;

/// Maximum branch choices offered by one node.
pub const MAX_BRANCHES_PER_NODE: u32 = 4;

/// Maximum quests a player may have active at once. Also bounds the iteration
/// in [`on_mission_completed`], keeping that call's cost constant.
pub const MAX_ACTIVE_QUESTS: u32 = 10;

/// Sentinel used by [`QuestBranch::next_quest_id`] to mean "this path ends
/// here" — chains terminate on a leaf rather than needing an explicit end node.
pub const CHAIN_TERMINUS: u64 = 0;

// ─── Storage Keys ─────────────────────────────────────────────────────────────

#[derive(Clone)]
#[contracttype]
pub enum QuestKey {
    /// Chain definition by chain ID.
    Chain(u64),
    /// Node definition by (globally unique) quest ID.
    Node(u64),
    /// Auto-increment chain ID counter.
    ChainCounter,
    /// Auto-increment quest (node) ID counter.
    NodeCounter,
    /// Per-player state for one node.
    QuestState(Address, u64),
    /// Per-player progress through one chain.
    Progress(Address, u64),
    /// Quest IDs currently active for a player.
    ActiveQuests(Address),
}

// ─── Data Types ───────────────────────────────────────────────────────────────

/// Where a player stands on a single quest node.
#[contracttype]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum QuestStatus {
    /// Defined but not yet reachable for this player.
    Locked,
    /// Accepted and accruing progress.
    Active,
    /// Target met; reward not yet taken.
    Completed,
    /// Reward taken.
    Claimed,
    /// Deadline passed before the target was met.
    Expired,
}

/// A quest payout. Multiple currencies so chains can mix incentives instead of
/// paying escalating essence at every step.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct QuestReward {
    /// Essence credited to the player's profile.
    pub essence: i128,
    /// Resource type granted (ignored when `resource_amount` is 0).
    pub resource_type: ResourceType,
    pub resource_amount: u64,
    /// Experience toward account progression.
    pub xp: u64,
    /// Whether the node grants a cosmetic roll.
    pub cosmetic_roll: bool,
}

/// One outgoing edge from a quest node — the unit of branching narrative.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct QuestBranch {
    /// Identifier the player passes to [`choose_branch`]. Must be non-zero.
    pub choice_id: u32,
    /// Short label for the choice, e.g. `sabotage` / `negotiate`.
    pub label: Symbol,
    /// Node unlocked by this choice, or [`CHAIN_TERMINUS`] to end the chain.
    pub next_quest_id: u64,
}

/// A quest node: one objective plus the choices that follow it.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct QuestNode {
    pub quest_id: u64,
    pub chain_id: u64,
    /// 0-based position in the order the node was added to its chain.
    pub step_index: u32,
    /// Objective tag, matched against `Mission::mission_type` when mission
    /// completions are forwarded in (`scan`, `harvest`, `explore`, `trade`, …).
    pub objective: Symbol,
    pub target_count: u32,
    pub reward: QuestReward,
    /// Narrative bucket for flavour text selection.
    pub narrative_tier: Symbol,
    pub title: String,
    pub description: String,
    pub branches: Vec<QuestBranch>,
    /// Seconds the player has to finish once active; 0 means no deadline.
    pub duration_secs: u64,
}

/// A named chain of quest nodes.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct QuestChain {
    pub chain_id: u64,
    pub name: Symbol,
    pub creator: Address,
    /// First node added to the chain; where [`start_chain`] begins.
    pub root_quest_id: u64,
    pub node_count: u32,
}

/// Per-player state for one quest node.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct QuestState {
    pub player: Address,
    pub quest_id: u64,
    pub chain_id: u64,
    pub status: QuestStatus,
    pub progress: u32,
    pub target: u32,
    pub started_at: u64,
    /// 0 when the node has no deadline.
    pub expires_at: u64,
    /// 0 until the target is met.
    pub completed_at: u64,
    /// Branch taken after completion; 0 while none has been chosen.
    pub chosen_branch: u32,
}

/// Per-player progress through a chain, including the narrative path walked.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ChainProgress {
    pub player: Address,
    pub chain_id: u64,
    /// Node the player is currently on, or the last one they finished.
    pub current_quest_id: u64,
    pub steps_completed: u32,
    pub total_essence_earned: i128,
    /// Branch choice IDs in the order they were taken.
    pub path: Vec<u32>,
    /// True once a terminal node has been claimed.
    pub finished: bool,
}

// ─── Errors ───────────────────────────────────────────────────────────────────

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum QuestError {
    /// No chain with the given ID.
    ChainNotFound = 1,
    /// No node with the given quest ID.
    QuestNotFound = 2,
    /// The player has no state for this quest.
    QuestNotStarted = 3,
    /// The quest is not in a state that allows this operation.
    InvalidStatus = 4,
    /// The quest deadline has passed.
    QuestExpired = 5,
    /// The reward for this quest was already taken.
    AlreadyClaimed = 6,
    /// Chain already holds [`MAX_CHAIN_LENGTH`] nodes.
    ChainFull = 7,
    /// Node declares more than [`MAX_BRANCHES_PER_NODE`] branches.
    TooManyBranches = 8,
    /// `choice_id` is not offered by this node.
    InvalidBranch = 9,
    /// The player already has [`MAX_ACTIVE_QUESTS`] quests active.
    TooManyActiveQuests = 10,
    /// Only the chain's creator may extend it.
    Unauthorized = 11,
    /// The player already started this chain.
    ChainAlreadyStarted = 12,
    /// `target_count` must be greater than zero.
    InvalidTarget = 13,
    /// No profile exists for the player, so rewards cannot be credited.
    ProfileNotFound = 14,
    /// Reward accounting overflowed.
    ArithmeticOverflow = 15,
    /// A branch points at a node that does not exist.
    DanglingBranch = 16,
}

impl crate::error_standard::StandardContractError for QuestError {
    fn descriptor(self) -> crate::error_standard::ErrorDescriptor {
        use crate::error_standard::ErrorKind;
        let (kind, retryable) = match self {
            Self::ChainNotFound | Self::QuestNotFound | Self::ProfileNotFound => {
                (ErrorKind::NotFound, false)
            }
            Self::QuestNotStarted
            | Self::InvalidStatus
            | Self::QuestExpired
            | Self::AlreadyClaimed
            | Self::ChainAlreadyStarted => (ErrorKind::Conflict, false),
            Self::ChainFull
            | Self::TooManyBranches
            | Self::TooManyActiveQuests
            | Self::ArithmeticOverflow => (ErrorKind::ResourceLimit, false),
            Self::InvalidBranch | Self::InvalidTarget => (ErrorKind::Validation, false),
            Self::Unauthorized => (ErrorKind::Authorization, false),
            Self::DanglingBranch => (ErrorKind::Internal, false),
        };
        crate::error_standard::ErrorDescriptor {
            module: "quest_system",
            code: self as u32,
            kind,
            retryable,
        }
    }
}

impl From<ProfileError> for QuestError {
    fn from(e: ProfileError) -> Self {
        match e {
            ProfileError::ArithmeticOverflow => QuestError::ArithmeticOverflow,
            _ => QuestError::ProfileNotFound,
        }
    }
}

// ─── Authoring ────────────────────────────────────────────────────────────────

/// Create an empty quest chain owned by `creator`.
