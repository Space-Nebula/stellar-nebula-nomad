//! Authenticated inter-player communication primitives.
//!
use soroban_sdk::{contracterror, contracttype, symbol_short, Address, BytesN, Env, Vec};

/// Pair lifetime: 30 days in seconds.
pub const PAIR_LIFETIME_SECS: u64 = 30 * 86_400;

/// Maximum messages sent in one burst call.
pub const MAX_MESSAGE_BURST: u32 = 20;

/// Default maximum number of messages allowed per entanglement pair.
pub const DEFAULT_PAIR_USE_LIMIT: u64 = 100;

// ─── Storage Keys ──────────────────────────────────────────────────────────────

#[derive(Clone)]
#[contracttype]
pub enum EntanglementKey {
    /// Auto-incrementing pair ID counter.
    EntanglementCounter,
    /// Entanglement pair by ID.
    Pair(u64),
    /// Message log index per pair (pair_id → total messages sent).
    MsgCount(u64),
    /// Stored encrypted message by `(pair_id, sequence)`.
    Message(u64, u64),
    /// Custom message limit per pair.
    PairUseLimit(u64),
    /// Shared encryption key commitment hash per pair.
    PairEncryptionKeyHash(u64),
    /// Guild coordination channel by `guild_id`.
    GuildChannel(u64),
    /// Stored guild coordination message by `(guild_id, sequence)`.
    GuildMessage(u64, u64),
}

// ─── Errors ─────────────────────────────────────────────────────────────────

#[contracterror]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum EntanglementError {
    /// The entanglement pair has expired or was never active.
    PairNotActive = 1,
    /// Caller is not a participant of this pair.
    NotAuthorized = 2,
    /// Pair does not exist.
    PairNotFound = 3,
    /// A ship cannot be entangled with itself.
    SameShip = 4,
    /// Burst size exceeded MAX_MESSAGE_BURST.
    BurstTooLarge = 5,
    /// Message batch is empty.
    EmptyBatch = 6,
    /// Pair has exhausted its message use limit.
    UsageLimitReached = 7,
    /// Provided use limit is invalid (must be > 0).
    InvalidLimit = 8,
    /// No encryption key commitment has been set for this pair.
    EncryptionKeyNotSet = 9,
    /// Guild coordination channel does not exist.
    GuildChannelNotFound = 10,
}

impl crate::error_standard::StandardContractError for EntanglementError {
    fn descriptor(self) -> crate::error_standard::ErrorDescriptor {
        use crate::error_standard::ErrorKind;
        let (kind, retryable) = match self {
            Self::PairNotActive => (ErrorKind::Conflict, false),
            Self::NotAuthorized => (ErrorKind::Authorization, false),
            Self::PairNotFound | Self::EncryptionKeyNotSet | Self::GuildChannelNotFound => {
                (ErrorKind::NotFound, false)
            }
            Self::SameShip | Self::EmptyBatch | Self::InvalidLimit => (ErrorKind::Validation, false),
            Self::BurstTooLarge | Self::UsageLimitReached => (ErrorKind::ResourceLimit, false),
        };
        crate::error_standard::ErrorDescriptor {
            module: "entanglement_comms",
            code: self as u32,
            kind,
            retryable,
        }
    }
}

// ─── Data Types ─────────────────────────────────────────────────────────────

#[derive(Clone, Debug, PartialEq)]
#[contracttype]
pub struct EntanglementPair {
    pub id: u64,
    pub ship_a: u64,
    pub ship_b: u64,
    /// Addresses authorized to send on behalf of each ship.
    pub owner_a: Address,
    pub owner_b: Address,
    pub created_at: u64,
    /// Pair expires at this ledger timestamp.
    pub expires_at: u64,
    pub active: bool,
}

#[derive(Clone, Debug, PartialEq)]
#[contracttype]
pub struct EntangledMessage {
    pub pair_id: u64,
    pub sender_ship: u64,
    /// 64-byte encrypted payload.
    pub payload: BytesN<64>,
    pub sent_at: u64,
    pub sequence: u64,
}

// ─── Helpers ────────────────────────────────────────────────────────────────

fn require_active_pair(pair: &EntanglementPair, now: u64) -> Result<(), EntanglementError> {
    if !pair.active || now > pair.expires_at {
        return Err(EntanglementError::PairNotActive);
    }
    Ok(())
}

fn require_participant(
    pair: &EntanglementPair,
    caller: &Address,
) -> Result<u64, EntanglementError> {
    if *caller == pair.owner_a {
        return Ok(pair.ship_a);
    }
    if *caller == pair.owner_b {
        return Ok(pair.ship_b);
    }
    Err(EntanglementError::NotAuthorized)
}

// ─── Public API ─────────────────────────────────────────────────────────────

/// Establish a new entanglement pair between two ships.
///
/// `owner_a` initiates and must authorize the call. Both ship IDs must differ.
/// The pair decays after [`PAIR_LIFETIME_SECS`] seconds.
pub fn create_entanglement_pair(
    env: &Env,
    owner_a: &Address,
    ship_a: u64,
    owner_b: &Address,
    ship_b: u64,
) -> Result<u64, EntanglementError> {
    owner_a.require_auth();

    if ship_a == ship_b {
        return Err(EntanglementError::SameShip);
    }

    let pair_id: u64 = env
        .storage()
        .instance()
        .get(&EntanglementKey::EntanglementCounter)
        .unwrap_or(0);

    let now = env.ledger().timestamp();
    let pair = EntanglementPair {
        id: pair_id,
        ship_a,
        ship_b,
        owner_a: owner_a.clone(),
        owner_b: owner_b.clone(),
        created_at: now,
        expires_at: now + PAIR_LIFETIME_SECS,
        active: true,
    };

    env.storage()
        .persistent()
        .set(&EntanglementKey::Pair(pair_id), &pair);
    env.storage()
        .instance()
        .set(&EntanglementKey::EntanglementCounter, &(pair_id + 1));
    env.storage()
        .instance()
        .set(&EntanglementKey::MsgCount(pair_id), &0u64);

    env.events().publish(
        (symbol_short!("entangle"), symbol_short!("create")),
        (owner_a.clone(), ship_a, ship_b, pair_id, now),
    );

    Ok(pair_id)
}

/// Transmit an encrypted 64-byte message over an active entanglement pair.
///
/// Only the pair participants (owner_a or owner_b) may send. The message
/// payload is treated as an opaque encrypted blob; the contract does not
/// decrypt it, ensuring end-to-end confidentiality.
pub fn send_entangled_message(
    env: &Env,
    caller: &Address,
    pair_id: u64,
    _message: &BytesN<64>,
) -> Result<u64, EntanglementError> {
    caller.require_auth();

    let pair: EntanglementPair = env
        .storage()
        .persistent()
        .get(&EntanglementKey::Pair(pair_id))
        .ok_or(EntanglementError::PairNotFound)?;

    let now = env.ledger().timestamp();
    require_active_pair(&pair, now)?;
    let sender_ship = require_participant(&pair, caller)?;

    let seq: u64 = env
        .storage()
        .instance()
        .get(&EntanglementKey::MsgCount(pair_id))
        .unwrap_or(0);

    let limit = pair_use_limit(env, pair_id);
    if seq >= limit {
        return Err(EntanglementError::UsageLimitReached);
    }

    let new_seq = seq + 1;
    let stored = EntangledMessage {
        pair_id,
        sender_ship,
        payload: _message.clone(),
        sent_at: now,
        sequence: new_seq,
    };
    env.storage()
        .persistent()
        .set(&EntanglementKey::Message(pair_id, new_seq), &stored);
    env.storage()
        .instance()
        .set(&EntanglementKey::MsgCount(pair_id), &new_seq);

    env.events().publish(
        (symbol_short!("entangle"), symbol_short!("msg")),
        (pair_id, sender_ship, new_seq, now),
    );

    Ok(new_seq)
}

/// Send up to [`MAX_MESSAGE_BURST`] messages in a single transaction.
///
/// Returns the sequence number of the last message sent.
pub fn send_entangled_message_batch(
    env: &Env,
    caller: &Address,
    pair_id: u64,
    messages: &Vec<BytesN<64>>,
) -> Result<u64, EntanglementError> {
    caller.require_auth();

    let n = messages.len();
    if n == 0 {
        return Err(EntanglementError::EmptyBatch);
    }
    if n > MAX_MESSAGE_BURST {
        return Err(EntanglementError::BurstTooLarge);
    }

    let mut last_seq = 0u64;
    for msg in messages.iter() {
        last_seq = send_entangled_message(env, caller, pair_id, &msg)?;
    }

    Ok(last_seq)
}

/// Deactivate a pair before it naturally expires.
///
/// Either participant may dissolve the pair.
pub fn dissolve_pair(env: &Env, caller: &Address, pair_id: u64) -> Result<(), EntanglementError> {
    caller.require_auth();

    let mut pair: EntanglementPair = env
        .storage()
        .persistent()
        .get(&EntanglementKey::Pair(pair_id))
        .ok_or(EntanglementError::PairNotFound)?;

    require_participant(&pair, caller)?;

    pair.active = false;
    env.storage()
        .persistent()
        .set(&EntanglementKey::Pair(pair_id), &pair);

    env.events().publish(
        (symbol_short!("entangle"), symbol_short!("dissolv")),
        (pair_id, caller.clone()),
    );

    Ok(())
}

/// Get pair data by ID.
pub fn get_entanglement_pair(
    env: &Env,
    pair_id: u64,
) -> Result<EntanglementPair, EntanglementError> {
    env.storage()
        .persistent()
        .get(&EntanglementKey::Pair(pair_id))
        .ok_or(EntanglementError::PairNotFound)
}

/// Return the total messages sent over a pair.
pub fn get_message_count(env: &Env, pair_id: u64) -> u64 {
    env.storage()
        .instance()
        .get(&EntanglementKey::MsgCount(pair_id))
        .unwrap_or(0)
}

#[derive(Clone, Debug, PartialEq)]
#[contracttype]
pub struct GuildCommsChannel {
    pub guild_id: u64,
    pub creator: Address,
    pub encrypted_topic: BytesN<32>,
    pub message_count: u64,
    pub active: bool,
}

#[derive(Clone, Debug, PartialEq)]
#[contracttype]
pub struct GuildMessage {
    pub guild_id: u64,
    pub sender: Address,
    pub sender_region: u64,
    pub payload: BytesN<64>,
    pub sent_at: u64,
    pub sequence: u64,
}

fn pair_use_limit(env: &Env, pair_id: u64) -> u64 {
    env.storage()
        .instance()
        .get(&EntanglementKey::PairUseLimit(pair_id))
        .unwrap_or(DEFAULT_PAIR_USE_LIMIT)
}

/// Set a custom message use limit for an active entanglement pair.
pub fn set_pair_use_limit(
    env: &Env,
    caller: &Address,
    pair_id: u64,
    limit: u64,
) -> Result<(), EntanglementError> {
    caller.require_auth();
    if limit == 0 {
        return Err(EntanglementError::InvalidLimit);
    }
    let pair = get_entanglement_pair(env, pair_id)?;
    require_participant(&pair, caller)?;
    env.storage()
        .instance()
        .set(&EntanglementKey::PairUseLimit(pair_id), &limit);
    Ok(())
}

/// Return remaining message uses for a pair.
pub fn get_pair_remaining_uses(env: &Env, pair_id: u64) -> Result<u64, EntanglementError> {
    let _ = get_entanglement_pair(env, pair_id)?;
    let limit = pair_use_limit(env, pair_id);
    let used = get_message_count(env, pair_id);
    Ok(limit.saturating_sub(used))
}

/// Register or rotate the shared encryption key commitment hash for a pair.
pub fn set_pair_encryption_key(
    env: &Env,
    caller: &Address,
    pair_id: u64,
    key_hash: BytesN<32>,
) -> Result<(), EntanglementError> {
    caller.require_auth();
    let pair = get_entanglement_pair(env, pair_id)?;
    require_active_pair(&pair, env.ledger().timestamp())?;
    require_participant(&pair, caller)?;
    env.storage()
        .persistent()
        .set(&EntanglementKey::PairEncryptionKeyHash(pair_id), &key_hash);
    env.events().publish(
        (symbol_short!("entangle"), symbol_short!("keyset")),
        (pair_id, caller.clone()),
    );
    Ok(())
}

/// Read the shared encryption key commitment hash for a pair.
pub fn get_pair_encryption_key(
    env: &Env,
    pair_id: u64,
) -> Result<BytesN<32>, EntanglementError> {
    env.storage()
        .persistent()
        .get(&EntanglementKey::PairEncryptionKeyHash(pair_id))
        .ok_or(EntanglementError::EncryptionKeyNotSet)
}

/// Read a bounded page of encrypted messages sent over a pair.
pub fn get_entangled_messages(
    env: &Env,
    pair_id: u64,
    offset: u64,
    limit: u32,
) -> Vec<EntangledMessage> {
    let total = get_message_count(env, pair_id);
    let mut out = Vec::new(env);
    let capped = core::cmp::min(limit, MAX_MESSAGE_BURST) as u64;
    let start = offset.saturating_add(1);
    let end = core::cmp::min(total, offset.saturating_add(capped));
    let mut seq = start;
    while seq <= end {
        if let Some(msg) = env
            .storage()
            .persistent()
            .get::<EntanglementKey, EntangledMessage>(&EntanglementKey::Message(pair_id, seq))
        {
            out.push_back(msg);
        }
        seq += 1;
    }
    out
}

/// Create or activate a guild-wide instant communication channel backed by `alliance_manager`.
pub fn create_guild_channel(
    env: &Env,
    caller: &Address,
    guild_id: u64,
    encrypted_topic: BytesN<32>,
) -> Result<GuildCommsChannel, EntanglementError> {
    caller.require_auth();
    let alliance = crate::alliance_manager::get_alliance(env, guild_id)
        .map_err(|_| EntanglementError::GuildChannelNotFound)?;
    if alliance.founder != *caller {
        return Err(EntanglementError::NotAuthorized);
    }

    let channel = GuildCommsChannel {
        guild_id,
        creator: caller.clone(),
        encrypted_topic,
        message_count: 0,
        active: true,
    };
    env.storage()
        .persistent()
        .set(&EntanglementKey::GuildChannel(guild_id), &channel);
    env.events().publish(
        (symbol_short!("entangle"), symbol_short!("g_chan")),
        (guild_id, caller.clone()),
    );
    Ok(channel)
}

/// Broadcast an encrypted coordination message to a guild channel across regions.
pub fn send_guild_message(
    env: &Env,
    caller: &Address,
    guild_id: u64,
    sender_region: u64,
    payload: BytesN<64>,
) -> Result<u64, EntanglementError> {
    caller.require_auth();
    let mut channel: GuildCommsChannel = env
        .storage()
        .persistent()
        .get(&EntanglementKey::GuildChannel(guild_id))
        .ok_or(EntanglementError::GuildChannelNotFound)?;
    if !channel.active {
        return Err(EntanglementError::PairNotActive);
    }

    let alliance = crate::alliance_manager::get_alliance(env, guild_id)
        .map_err(|_| EntanglementError::GuildChannelNotFound)?;
    if !alliance.members.contains(caller) {
        return Err(EntanglementError::NotAuthorized);
    }

    channel.message_count = channel.message_count.saturating_add(1);
    let seq = channel.message_count;
    let now = env.ledger().timestamp();
    let msg = GuildMessage {
        guild_id,
        sender: caller.clone(),
        sender_region,
        payload,
        sent_at: now,
        sequence: seq,
    };
    env.storage()
        .persistent()
        .set(&EntanglementKey::GuildMessage(guild_id, seq), &msg);
    env.storage()
        .persistent()
        .set(&EntanglementKey::GuildChannel(guild_id), &channel);
    env.events().publish(
        (symbol_short!("entangle"), symbol_short!("g_msg")),
        (guild_id, sender_region, seq, now),
    );
    Ok(seq)
}

/// Fetch a bounded page of guild coordination messages.
pub fn get_guild_messages(
    env: &Env,
    guild_id: u64,
    offset: u64,
    limit: u32,
) -> Vec<GuildMessage> {
    let total = env
        .storage()
        .persistent()
        .get::<EntanglementKey, GuildCommsChannel>(&EntanglementKey::GuildChannel(guild_id))
        .map(|c| c.message_count)
        .unwrap_or(0);
    let mut out = Vec::new(env);
    let capped = core::cmp::min(limit, MAX_MESSAGE_BURST) as u64;
    let start = offset.saturating_add(1);
    let end = core::cmp::min(total, offset.saturating_add(capped));
    let mut seq = start;
    while seq <= end {
        if let Some(msg) = env
            .storage()
            .persistent()
            .get::<EntanglementKey, GuildMessage>(&EntanglementKey::GuildMessage(guild_id, seq))
        {
            out.push_back(msg);
        }
        seq += 1;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use soroban_sdk::testutils::Address as _;
    use soroban_sdk::{contract, contractimpl, BytesN, Env};

    #[contract]
    struct Stub;
    #[contractimpl]
    impl Stub {}

    #[test]
    fn test_pair_messaging_limits_and_encryption_key() {
        let env = Env::default();
        env.mock_all_auths();
        let cid = env.register(Stub, ());
        let a = Address::generate(&env);
        let b = Address::generate(&env);

        env.as_contract(&cid, || {
            let pair_id = create_entanglement_pair(&env, &a, 1, &b, 2).unwrap();
            set_pair_use_limit(&env, &a, pair_id, 2).unwrap();
            let key_hash = BytesN::from_array(&env, &[9u8; 32]);
            set_pair_encryption_key(&env, &a, pair_id, key_hash.clone()).unwrap();
            assert_eq!(get_pair_encryption_key(&env, pair_id).unwrap(), key_hash);

            let payload = BytesN::from_array(&env, &[7u8; 64]);
            assert_eq!(send_entangled_message(&env, &a, pair_id, &payload).unwrap(), 1);
            assert_eq!(send_entangled_message(&env, &b, pair_id, &payload).unwrap(), 2);
            assert_eq!(get_pair_remaining_uses(&env, pair_id).unwrap(), 0);
            assert_eq!(
                send_entangled_message(&env, &a, pair_id, &payload),
                Err(EntanglementError::UsageLimitReached)
            );

            let history = get_entangled_messages(&env, pair_id, 0, 10);
            assert_eq!(history.len(), 2);
            assert_eq!(history.get(0).unwrap().sender_ship, 1);
            assert_eq!(history.get(1).unwrap().sender_ship, 2);
        });
    }
}
