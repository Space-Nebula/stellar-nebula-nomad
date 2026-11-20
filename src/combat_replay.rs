//! Combat replay recording, bounded storage, and deterministic playback
//! (Issue #592).
//!
//! The PvP engine remains authoritative: this module only observes committed
//! `CombatState` transitions. A replay is a typed header plus fixed-width
//! frames packed into `Bytes` chunks. Frames capture the complete post-move
//! state (both health bars, both energy bars, turn, status, winner, action and
//! timestamp), so playback never needs to re-run game logic or fetch historical
//! combat records.
//!
//! Storage is deliberately bounded: at most [`MAX_REPLAY_FRAMES`] frames per
//! combat, [`FRAMES_PER_CHUNK`] frames per `Bytes` chunk, and
//! [`MAX_REPLAYS_PER_OWNER`] replay ids per player index.

use soroban_sdk::{
    contracterror, contracttype, symbol_short, Address, Bytes, BytesN, Env, Symbol, Vec,
};

use crate::pvp_combat::{CombatState, MAX_MOVE_POWER};

// ── Constants ────────────────────────────────────────────────────────────────

/// Wire format version. Bump when the frame layout changes.
pub const REPLAY_FORMAT_VERSION: u32 = 1;
/// Symbol name of the serialized payload format.
pub const REPLAY_FORMAT_SYMBOL: &str = "snreplay1";
/// Hard cap on frames stored for a single combat.
pub const MAX_REPLAY_FRAMES: u32 = 512;
/// Number of fixed-width frames in one storage chunk (28 * 32 = 896 bytes).
pub const FRAMES_PER_CHUNK: u32 = 32;
/// Bytes per packed frame.
pub const REPLAY_FRAME_BYTES: usize = 28;
/// Maximum frames returned by one paged read.
pub const MAX_FRAME_PAGE: u32 = 64;
/// Maximum replay ids retained per player index.
pub const MAX_REPLAYS_PER_OWNER: u32 = 50;
/// TTL extension for replay entries (ledger sequences, ~30 days).
pub const REPLAY_TTL: u32 = 518_400;
/// Maximum TTL ceiling for replay entries.
pub const REPLAY_MAX_TTL: u32 = 3_110_400;

/// Playback speeds are expressed in basis points to avoid floats.
pub const SPEED_HALF_BPS: u32 = 5_000;
pub const SPEED_NORMAL_BPS: u32 = 10_000;
pub const SPEED_DOUBLE_BPS: u32 = 20_000;
/// One frame at 1x lasts one second.
pub const BASE_FRAME_INTERVAL_MS: u64 = 1_000;

// Frame kinds (byte 0 of a packed frame).
pub const FRAME_START: u8 = 0;
pub const FRAME_MOVE: u8 = 1;
pub const FRAME_FINISH: u8 = 2;
pub const FRAME_CANCEL: u8 = 3;

// Action codes (byte 2 of a packed frame).
pub const ACTION_NONE: u8 = 0;
pub const ACTION_ATTACK: u8 = 1;
pub const ACTION_DEFEND: u8 = 2;
pub const ACTION_SPECIAL: u8 = 3;
pub const ACTION_HEAL: u8 = 4;

// Combat status codes (byte 3 of a packed frame).
pub const STATUS_ACTIVE: u8 = 0;
pub const STATUS_FINISHED: u8 = 1;
pub const STATUS_CANCELLED: u8 = 2;

/// Actor / winner sentinel: no actor (setup frame).
pub const ACTOR_NONE: u8 = u8::MAX;
/// Winner sentinel: no winner yet.
pub const WINNER_NONE: u8 = 0;
/// Winner code for player1.
pub const WINNER_P1: u8 = 1;
/// Winner code for player2.
pub const WINNER_P2: u8 = 2;

const BASE32: [u8; 32] = *b"abcdefghijklmnopqrstuvwxyz234567";
const SHARE_ID_DIGITS: usize = 13;
const SHARE_TOKEN_LEN: usize = 2 + SHARE_ID_DIGITS + 1;

// ── Storage Keys ─────────────────────────────────────────────────────────────

#[contracttype]
#[derive(Clone)]
pub enum ReplayKey {
    /// Monotonic replay id counter.
    ReplayCounter,
    /// Combat id -> replay id.
    ByCombat(u64),
    /// Replay header/metadata.
    Header(u64),
    /// Packed frames, `FRAMES_PER_CHUNK` frames per value.
    Chunk(u64, u32),
    /// Share token -> replay id.
    Share(Symbol),
    /// Bounded list of replays involving a player.
    OwnerReplays(Address),
}

// ── Errors ───────────────────────────────────────────────────────────────────

#[contracterror]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum ReplayError {
    ReplayNotFound = 1,
    InvalidFrame = 2,
    FrameLimitExceeded = 3,
    CorruptedReplay = 4,
    Unauthorized = 5,
    ShareNotFound = 6,
    UnsupportedFormat = 7,
}

impl crate::error_standard::StandardContractError for ReplayError {
    fn descriptor(self) -> crate::error_standard::ErrorDescriptor {
        use crate::error_standard::ErrorKind;
        let (kind, retryable) = match self {
            Self::ReplayNotFound | Self::ShareNotFound => (ErrorKind::NotFound, false),
            Self::InvalidFrame | Self::CorruptedReplay | Self::UnsupportedFormat => {
                (ErrorKind::Validation, false)
            }
            Self::FrameLimitExceeded => (ErrorKind::ResourceLimit, false),
            Self::Unauthorized => (ErrorKind::Authorization, false),
        };
        crate::error_standard::ErrorDescriptor {
            module: "combat_replay",
            code: self as u32,
            kind,
            retryable,
        }
    }
}

// ── Data Types ───────────────────────────────────────────────────────────────

/// Immutable metadata for one recorded combat.
#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub struct CombatReplay {
    pub replay_id: u64,
    pub version: u32,
    pub combat_id: u64,
    pub player1: Address,
    pub player2: Address,
    pub started_at: u64,
    /// 0 while the combat is still running.
    pub ended_at: u64,
    pub frame_count: u32,
    /// Highest turn number recorded (0 means setup frame only).
    pub final_turn: u32,
    pub winner: Option<Address>,
    pub status: Symbol,
    /// SHA-256 over header scalars and the packed frame payload.
    pub integrity_hash: BytesN<32>,
}

/// One decoded frame, ready for a playback UI.
#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub struct ReplayFrameView {
    pub index: u32,
    pub kind: Symbol,
    pub action: Symbol,
    pub actor: u32,
    /// 0 for the setup frame, 1-based for moves.
    pub turn_number: u32,
    pub power: u32,
    pub player1_hp: u32,
    pub player2_hp: u32,
    pub player1_energy: u32,
    pub player2_energy: u32,
    pub winner: Option<Address>,
    pub status: Symbol,
    pub timestamp: u64,
}

/// Header plus decoded frames, returned by fetch/share endpoints.
#[contracttype]
#[derive(Clone, Debug)]
pub struct ReplayBundle {
    pub header: CombatReplay,
    pub frames: Vec<ReplayFrameView>,
}

/// Compact transport form of a replay (typed header + packed frame bytes).
#[contracttype]
#[derive(Clone, Debug)]
pub struct ReplayExport {
    pub header: CombatReplay,
    pub format: Symbol,
    pub payload: Bytes,
}

/// Result of creating a share link.
#[contracttype]
#[derive(Clone, Debug)]
pub struct ReplayShare {
    pub token: Symbol,
    pub replay_id: u64,
}

/// Playback cursor returned by the contract-side playback helpers.
#[contracttype]
#[derive(Clone, Debug)]
pub struct PlaybackState {
    pub replay_id: u64,
    pub frame_index: u32,
    pub turn_number: u32,
    /// Effective speed in basis points.
    pub speed_bps: u32,
    /// Milliseconds to wait before advancing one frame.
    pub interval_ms: u64,
    pub frame_count: u32,
    pub at_end: bool,
}

/// `no_std`, `Env`-free frame representation used by the codec.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FrameState {
    pub kind: u8,
    pub actor: u8,
    pub action: u8,
    pub status: u8,
    pub winner: u8,
    pub turn: u8,
    pub turn_number: u16,
    pub power: u16,
    pub player1_hp: u16,
    pub player2_hp: u16,
    pub player1_energy: u16,
    pub player2_energy: u16,
    pub timestamp: u64,
}

// ── Pure helpers (indexing, speed, seek, codec) ─────────────────────────────

/// Clamp a requested frame index into `[0, frame_count)`.
/// An empty replay always seeks to frame 0.
pub fn clamp_frame_index(frame_count: u32, requested: u32) -> u32 {
    if frame_count == 0 {
        return 0;
    }
    let last = frame_count - 1;
    if requested > last {
        last
    } else {
        requested
    }
}

/// Chunk holding a given frame index.
pub fn chunk_index(frame_index: u32) -> u32 {
    frame_index / FRAMES_PER_CHUNK
}

/// Byte offset of a frame inside its chunk.
pub fn frame_offset_in_chunk(frame_index: u32) -> u32 {
    (frame_index % FRAMES_PER_CHUNK) * REPLAY_FRAME_BYTES as u32
}

/// Snap arbitrary input to one of the three supported speeds.
pub fn normalize_speed_bps(speed_bps: u32) -> u32 {
    if speed_bps <= SPEED_HALF_BPS {
        SPEED_HALF_BPS
    } else if speed_bps >= SPEED_DOUBLE_BPS {
        SPEED_DOUBLE_BPS
    } else {
        SPEED_NORMAL_BPS
    }
}

/// Delay between frames for a speed expressed in basis points.
pub fn frame_interval_ms(speed_bps: u32) -> u64 {
    let speed = normalize_speed_bps(speed_bps) as u64;
    BASE_FRAME_INTERVAL_MS * SPEED_NORMAL_BPS as u64 / speed
}

/// Frame index for a 1-based turn number in a packed payload, if present.
pub fn find_turn_in_packed(payload: &[u8], target_turn: u16) -> Option<u32> {
    if target_turn == 0 || payload.len() < REPLAY_FRAME_BYTES {
        return None;
    }
    let mut index = 0u32;
    let mut offset = 0usize;
    while offset + REPLAY_FRAME_BYTES <= payload.len() {
        let mut packed = [0u8; REPLAY_FRAME_BYTES];
        packed.copy_from_slice(&payload[offset..offset + REPLAY_FRAME_BYTES]);
        let frame = unpack_frame(&packed);
        if frame.turn_number == target_turn {
            return Some(index);
        }
        index += 1;
        offset += REPLAY_FRAME_BYTES;
    }
    None
}

/// Deterministic share token bytes (`rp` + 13 base32 digits + checksum).
pub fn share_token_bytes(replay_id: u64) -> [u8; SHARE_TOKEN_LEN] {
    let mut out = [b'a'; SHARE_TOKEN_LEN];
    out[0] = b'r';
    out[1] = b'p';
    let mut i = 0usize;
    while i < SHARE_ID_DIGITS {
        let shift = ((SHARE_ID_DIGITS - 1 - i) * 5) as u32;
        let digit = if shift >= 64 {
            0usize
        } else {
            ((replay_id >> shift) & 31) as usize
        };
        out[2 + i] = BASE32[digit];
        i += 1;
    }
    let mut checksum = 0u32;
    i = 0;
    while i < SHARE_ID_DIGITS + 2 {
        checksum += out[i] as u32;
        i += 1;
    }
    out[SHARE_TOKEN_LEN - 1] = BASE32[(checksum % 32) as usize];
    out
}

fn write_u16(target: &mut [u8; REPLAY_FRAME_BYTES], offset: usize, value: u16) {
    let bytes = value.to_be_bytes();
    target[offset] = bytes[0];
    target[offset + 1] = bytes[1];
}

fn write_u64(target: &mut [u8; REPLAY_FRAME_BYTES], offset: usize, value: u64) {
    let bytes = value.to_be_bytes();
    let mut i = 0usize;
    while i < 8 {
        target[offset + i] = bytes[i];
        i += 1;
    }
}

fn read_u16(source: &[u8; REPLAY_FRAME_BYTES], offset: usize) -> u16 {
    u16::from_be_bytes([source[offset], source[offset + 1]])
}

fn read_u64(source: &[u8; REPLAY_FRAME_BYTES], offset: usize) -> u64 {
    let mut bytes = [0u8; 8];
    let mut i = 0usize;
    while i < 8 {
        bytes[i] = source[offset + i];
        i += 1;
    }
    u64::from_be_bytes(bytes)
}

/// Serialize one frame into the fixed 28-byte layout:
/// `kind,actor,action,status,winner,turn,reserved[2],turn_no[2],power[2],
/// p1_hp[2],p2_hp[2],p1_energy[2],p2_energy[2],timestamp[8]` (big-endian).
pub fn pack_frame(frame: &FrameState) -> [u8; REPLAY_FRAME_BYTES] {
    let mut out = [0u8; REPLAY_FRAME_BYTES];
    out[0] = frame.kind;
    out[1] = frame.actor;
    out[2] = frame.action;
    out[3] = frame.status;
    out[4] = frame.winner;
    out[5] = frame.turn;
    // Bytes 6..8 are reserved and stay zero for forward compatibility.
    write_u16(&mut out, 8, frame.turn_number);
    write_u16(&mut out, 10, frame.power);
    write_u16(&mut out, 12, frame.player1_hp);
    write_u16(&mut out, 14, frame.player2_hp);
    write_u16(&mut out, 16, frame.player1_energy);
    write_u16(&mut out, 18, frame.player2_energy);
    write_u64(&mut out, 20, frame.timestamp);
    out
}

/// Deserialize a frame. Reserved bytes are ignored so older replays remain
/// readable when the layout is extended.
pub fn unpack_frame(packed: &[u8; REPLAY_FRAME_BYTES]) -> FrameState {
    FrameState {
        kind: packed[0],
        actor: packed[1],
        action: packed[2],
        status: packed[3],
        winner: packed[4],
        turn: packed[5],
        turn_number: read_u16(packed, 8),
        power: read_u16(packed, 10),
        player1_hp: read_u16(packed, 12),
        player2_hp: read_u16(packed, 14),
        player1_energy: read_u16(packed, 16),
        player2_energy: read_u16(packed, 18),
        timestamp: read_u64(packed, 20),
    }
}

fn narrow(value: u32) -> u16 {
    if value > u16::MAX as u32 {
        u16::MAX
    } else {
        value as u16
    }
}

fn turn_index(combat: &CombatState) -> u8 {
    if combat.turn == combat.player1 {
        0
    } else {
        1
    }
}

fn winner_code(combat: &CombatState) -> u8 {
    match &combat.winner {
        Some(w) if *w == combat.player1 => WINNER_P1,
        Some(_) => WINNER_P2,
        None => WINNER_NONE,
    }
}

fn status_code(combat: &CombatState) -> u8 {
    if combat.status == symbol_short!("finished") {
        STATUS_FINISHED
    } else if combat.status == symbol_short!("cancelled") {
        STATUS_CANCELLED
    } else {
        STATUS_ACTIVE
    }
}

fn action_code(action: &Symbol) -> u8 {
    if *action == symbol_short!("attack") {
        ACTION_ATTACK
    } else if *action == symbol_short!("defend") {
        ACTION_DEFEND
    } else if *action == symbol_short!("special") {
        ACTION_SPECIAL
    } else if *action == symbol_short!("heal") {
        ACTION_HEAL
    } else {
        ACTION_NONE
    }
}

fn frame_from_combat(
    combat: &CombatState,
    kind: u8,
    actor: u8,
    action: u8,
    power: u32,
    timestamp: u64,
) -> FrameState {
    FrameState {
        kind,
        actor,
        action,
        status: status_code(combat),
        winner: winner_code(combat),
        turn: turn_index(combat),
        turn_number: 0, // assigned by append_frame
        power: narrow(power),
        player1_hp: narrow(combat.player1_hp),
        player2_hp: narrow(combat.player2_hp),
        player1_energy: narrow(combat.player1_energy),
        player2_energy: narrow(combat.player2_energy),
        timestamp,
    }
}

// ── Storage helpers ──────────────────────────────────────────────────────────

fn next_replay_id(env: &Env) -> u64 {
    let key = ReplayKey::ReplayCounter;
    let current: u64 = env.storage().persistent().get(&key).unwrap_or(0);
    let next = current.saturating_add(1);
    env.storage().persistent().set(&key, &next);
    next
}

fn read_header(env: &Env, replay_id: u64) -> Result<CombatReplay, ReplayError> {
    env.storage()
        .persistent()
        .get(&ReplayKey::Header(replay_id))
        .ok_or(ReplayError::ReplayNotFound)
}

fn write_header(env: &Env, header: &CombatReplay) {
    let key = ReplayKey::Header(header.replay_id);
    env.storage().persistent().set(&key, header);
    env.storage()
        .persistent()
        .extend_ttl(&key, REPLAY_TTL, REPLAY_MAX_TTL);
}

fn touch_chunk(env: &Env, replay_id: u64, chunk: u32) {
    env.storage().persistent().extend_ttl(
        &ReplayKey::Chunk(replay_id, chunk),
        REPLAY_TTL,
        REPLAY_MAX_TTL,
    );
}

fn index_owner(env: &Env, player: &Address, replay_id: u64) {
    let key = ReplayKey::OwnerReplays(player.clone());
    let mut ids: Vec<u64> = env
        .storage()
        .persistent()
        .get(&key)
        .unwrap_or_else(|| Vec::new(env));
    let mut i = 0u32;
    while i < ids.len() {
        if ids.get(i) == Some(replay_id) {
            return;
        }
        i += 1;
    }
    ids.push_back(replay_id);
    while ids.len() > MAX_REPLAYS_PER_OWNER {
        ids.remove(0);
    }
    env.storage().persistent().set(&key, &ids);
    env.storage()
        .persistent()
        .extend_ttl(&key, REPLAY_TTL, REPLAY_MAX_TTL);
}

fn create_replay(env: &Env, combat: &CombatState) -> u64 {
    let existing: Option<u64> = env
        .storage()
        .persistent()
        .get(&ReplayKey::ByCombat(combat.combat_id));
    if let Some(id) = existing {
        return id;
    }

    let replay_id = next_replay_id(env);
    let header = CombatReplay {
        replay_id,
        version: REPLAY_FORMAT_VERSION,
        combat_id: combat.combat_id,
        player1: combat.player1.clone(),
        player2: combat.player2.clone(),
        started_at: combat.started_at,
        ended_at: 0,
        frame_count: 0,
        final_turn: 0,
        winner: None,
        status: Symbol::new(env, "active"),
        integrity_hash: BytesN::from_array(env, &[0u8; 32]),
    };
    write_header(env, &header);
    env.storage().persistent().set(&ReplayKey::ByCombat(combat.combat_id), &replay_id);
    index_owner(env, &combat.player1, replay_id);
    index_owner(env, &combat.player2, replay_id);
    replay_id
}

fn append_frame(env: &Env, replay_id: u64, mut frame: FrameState) -> bool {
    let mut header = match read_header(env, replay_id) {
        Ok(h) => h,
        Err(_) => return false,
    };
    if header.frame_count >= MAX_REPLAY_FRAMES {
        return false; // bounded, deterministic truncation
    }

    if frame.kind == FRAME_START {
        frame.turn_number = 0;
    } else {
        frame.turn_number = narrow(header.final_turn.saturating_add(1));
    }

    let index = header.frame_count;
    let chunk_no = chunk_index(index);
    let chunk_key = ReplayKey::Chunk(replay_id, chunk_no);
    let mut chunk: Bytes = env
        .storage()
        .persistent()
        .get(&chunk_key)
        .unwrap_or_else(|| Bytes::new(env));
    chunk.append(&Bytes::from_slice(env, &pack_frame(&frame)));
    env.storage().persistent().set(&chunk_key, &chunk);
    touch_chunk(env, replay_id, chunk_no);

    header.frame_count = index.saturating_add(1);
    header.final_turn = frame.turn_number as u32;
    header.ended_at = frame.timestamp;
    header.status = status_symbol(env, frame.status);
    header.winner = if frame.winner == WINNER_NONE {
        None
    } else if frame.winner == WINNER_P1 {
        Some(header.player1.clone())
    } else {
        Some(header.player2.clone())
    };
    write_header(env, &header);

    env.events().publish(
        (symbol_short!("replay"), symbol_short!("frame")),
        (replay_id, index, frame.turn_number as u32),
    );
    true
}

fn status_symbol(env: &Env, code: u8) -> Symbol {
    match code {
        STATUS_FINISHED => symbol_short!("finished"),
        STATUS_CANCELLED => symbol_short!("cancelled"),
        _ => symbol_short!("active"),
    }
}

fn action_symbol(env: &Env, frame: &FrameState) -> Symbol {
    match frame.kind {
        FRAME_START => symbol_short!("start"),
        FRAME_FINISH => symbol_short!("finish"),
        FRAME_CANCEL => symbol_short!("cancel"),
        _ => match frame.action {
            ACTION_ATTACK => symbol_short!("attack"),
            ACTION_DEFEND => symbol_short!("defend"),
            ACTION_SPECIAL => symbol_short!("special"),
            ACTION_HEAL => symbol_short!("heal"),
            _ => symbol_short!("none"),
        },
    }
}

fn frame_view(env: &Env, header: &CombatReplay, index: u32, frame: &FrameState) -> ReplayFrameView {
    ReplayFrameView {
        index,
        kind: match frame.kind {
            FRAME_START => symbol_short!("start"),
            FRAME_FINISH => symbol_short!("finish"),
            FRAME_CANCEL => symbol_short!("cancel"),
            _ => symbol_short!("move"),
        },
        action: action_symbol(env, frame),
        actor: if frame.actor == ACTOR_NONE {
            u32::MAX
        } else {
            frame.actor as u32
        },
        turn_number: frame.turn_number as u32,
        power: frame.power as u32,
        player1_hp: frame.player1_hp as u32,
        player2_hp: frame.player2_hp as u32,
        player1_energy: frame.player1_energy as u32,
        player2_energy: frame.player2_energy as u32,
        winner: if frame.winner == WINNER_NONE {
            None
        } else if frame.winner == WINNER_P1 {
            Some(header.player1.clone())
        } else {
            Some(header.player2.clone())
        },
        status: status_symbol(env, frame.status),
        timestamp: frame.timestamp,
    }
}

fn read_frame_state(env: &Env, replay_id: u64, index: u32) -> Result<FrameState, ReplayError> {
    let header = read_header(env, replay_id)?;
    if index >= header.frame_count {
        return Err(ReplayError::InvalidFrame);
    }
    let chunk_no = chunk_index(index);
    let chunk: Bytes = env
        .storage()
        .persistent()
        .get(&ReplayKey::Chunk(replay_id, chunk_no))
        .ok_or(ReplayError::CorruptedReplay)?;
    let offset = frame_offset_in_chunk(index);
    let mut packed = [0u8; REPLAY_FRAME_BYTES];
    let mut i = 0u32;
    while i < REPLAY_FRAME_BYTES as u32 {
        packed[i as usize] = chunk
            .get(offset.saturating_add(i))
            .ok_or(ReplayError::CorruptedReplay)?;
        i += 1;
    }
    Ok(unpack_frame(&packed))
}

fn serialize_frames(env: &Env, replay_id: u64) -> Result<Bytes, ReplayError> {
    let header = read_header(env, replay_id)?;
    let mut payload = Bytes::new(env);
    let chunks = (header.frame_count.saturating_add(FRAMES_PER_CHUNK - 1)) / FRAMES_PER_CHUNK;
    let mut i = 0u32;
    while i < chunks {
        let chunk: Bytes = env
            .storage()
            .persistent()
            .get(&ReplayKey::Chunk(replay_id, i))
            .ok_or(ReplayError::CorruptedReplay)?;
        payload.append(&chunk);
        i += 1;
    }
    if payload.len() != header.frame_count.saturating_mul(REPLAY_FRAME_BYTES as u32) {
        return Err(ReplayError::CorruptedReplay);
    }
    Ok(payload)
}

fn append_hash_scalar(target: &mut Bytes, value: u64) {
    let bytes = value.to_be_bytes();
    let mut i = 0usize;
    while i < 8 {
        target.push_back(bytes[i]);
        i += 1;
    }
}

fn append_hash_u32(target: &mut Bytes, value: u32) {
    let bytes = value.to_be_bytes();
    let mut i = 0usize;
    while i < 4 {
        target.push_back(bytes[i]);
        i += 1;
    }
}

/// Integrity covers version, combat id, start time, frame count and every
/// packed frame. Player addresses are stored in the typed header and are not
/// hashed; mutation of a recorded replay is prevented by the contract API.
fn compute_integrity_hash(
    env: &Env,
    version: u32,
    combat_id: u64,
    started_at: u64,
    frame_count: u32,
    payload: &Bytes,
) -> BytesN<32> {
    let mut data = Bytes::new(env);
    append_hash_u32(&mut data, version);
    append_hash_scalar(&mut data, combat_id);
    append_hash_scalar(&mut data, started_at);
    append_hash_u32(&mut data, frame_count);
    data.append(payload);
    env.crypto().sha256(&data).to_bytes()
}

fn refresh_integrity(env: &Env, replay_id: u64) {
    let mut header = match read_header(env, replay_id) {
        Ok(h) => h,
        Err(_) => return,
    };
    let payload = match serialize_frames(env, replay_id) {
        Ok(p) => p,
        Err(_) => return,
    };
    header.integrity_hash = compute_integrity_hash(
        env,
        header.version,
        header.combat_id,
        header.started_at,
        header.frame_count,
        &payload,
    );
    write_header(env, &header);
}

fn finalize(env: &Env, replay_id: u64) {
    refresh_integrity(env, replay_id);
    env.events().publish(
        (symbol_short!("replay"), symbol_short!("final")),
        (replay_id,),
    );
}

// ── Recording hooks (called from pvp_combat) ────────────────────────────────

/// Record the initial state of a newly started combat. Idempotent per combat.
/// Infallible by design: replay bookkeeping must never break a PvP transaction.
pub fn record_start(env: &Env, combat: &CombatState) {
    if env
        .storage()
        .persistent()
        .has(&ReplayKey::ByCombat(combat.combat_id))
    {
        return; // already seeded (idempotent hook)
    }
    let replay_id = create_replay(env, combat);
    let timestamp = combat.started_at;
    let frame = frame_from_combat(combat, FRAME_START, ACTOR_NONE, ACTION_NONE, 0, timestamp);
    if append_frame(env, replay_id, frame) {
        refresh_integrity(env, replay_id);
        env.events().publish(
            (symbol_short!("replay"), symbol_short!("start")),
            (replay_id, combat.combat_id),
        );
    }
}

/// Record a committed move and the state it produced.
/// If a combat predates the recording feature, a replay is lazily seeded from
/// the first state observed and the move itself becomes turn 1.
pub fn record_move(env: &Env, combat: &CombatState, move_type: &Symbol, power: u32) {
    let replay_id = create_replay(env, combat);
    let actor = if move_actor_is_player1(combat) { 0 } else { 1 };
    let mut frame = frame_from_combat(
        combat,
        FRAME_MOVE,
        actor,
        action_code(move_type),
        power,
        env.ledger().timestamp(),
    );
    if combat.status == symbol_short!("finished") {
        frame.kind = FRAME_FINISH;
    }
    if append_frame(env, replay_id, frame) && combat.status == symbol_short!("finished") {
        finalize(env, replay_id);
    }
}

/// Record a combat cancelled by timeout at the point of resolution.
pub fn record_cancellation(env: &Env, combat: &CombatState) {
    let replay_id = create_replay(env, combat);
    let frame = frame_from_combat(
        combat,
        FRAME_CANCEL,
        ACTOR_NONE,
        ACTION_NONE,
        0,
        env.ledger().timestamp(),
    );
    if append_frame(env, replay_id, frame) {
        finalize(env, replay_id);
    }
}

/// The move actor is the player whose turn was consumed. `record_move` runs
/// after `execute_move` has switched `combat.turn` for non-finishing moves, so
/// the actor is the opposite of the current turn. Finishing moves do not
/// switch turns, so `combat.turn` is already the actor.
fn move_actor_is_player1(combat: &CombatState) -> bool {
    let turn_is_p1 = combat.turn == combat.player1;
    if combat.status == symbol_short!("finished") {
        turn_is_p1
    } else {
        !turn_is_p1
    }
}

// ── Queries & playback ───────────────────────────────────────────────────────

pub fn get_replay_id_for_combat(env: &Env, combat_id: u64) -> Result<u64, ReplayError> {
    env.storage()
        .persistent()
        .get(&ReplayKey::ByCombat(combat_id))
        .ok_or(ReplayError::ReplayNotFound)
}

pub fn get_replay(env: &Env, replay_id: u64) -> Result<CombatReplay, ReplayError> {
    read_header(env, replay_id)
}

pub fn get_replay_frame(
    env: &Env,
    replay_id: u64,
    index: u32,
) -> Result<ReplayFrameView, ReplayError> {
    let header = read_header(env, replay_id)?;
    let index = clamp_frame_index(header.frame_count, index);
    if header.frame_count == 0 {
        return Err(ReplayError::InvalidFrame);
    }
    let state = read_frame_state(env, replay_id, index)?;
    Ok(frame_view(env, &header, index, &state))
}

/// Bounded paged read for UIs that do not need the whole replay.
pub fn get_replay_frames(
    env: &Env,
    replay_id: u64,
    start: u32,
    limit: u32,
) -> Result<Vec<ReplayFrameView>, ReplayError> {
    let header = read_header(env, replay_id)?;
    let from = clamp_frame_index(header.frame_count, start);
    let count = limit.min(MAX_FRAME_PAGE.min(header.frame_count.saturating_sub(from)));
    let mut frames = Vec::new(env);
    let mut i = 0u32;
    while i < count {
        let state = read_frame_state(env, replay_id, from.saturating_add(i))?;
        frames.push_back(frame_view(env, &header, from.saturating_add(i), &state));
        i += 1;
    }
    Ok(frames)
}

pub fn get_replay_bundle(env: &Env, replay_id: u64) -> Result<ReplayBundle, ReplayError> {
    let header = read_header(env, replay_id)?;
    let mut frames = Vec::new(env);
    let mut i = 0u32;
    while i < header.frame_count {
        let state = read_frame_state(env, replay_id, i)?;
        frames.push_back(frame_view(env, &header, i, &state));
        i += 1;
    }
    Ok(ReplayBundle { header, frames })
}

pub fn get_player_replays(env: &Env, player: &Address) -> Vec<u64> {
    env.storage()
        .persistent()
        .get(&ReplayKey::OwnerReplays(player.clone()))
        .unwrap_or_else(|| Vec::new(env))
}

/// Contract-side playback cursor. Timing is returned so clients can drive a
/// timer with 0.5x / 1x / 2x without duplicating speed math.
pub fn playback_state(
    env: &Env,
    replay_id: u64,
    frame_index: u32,
    speed_bps: u32,
) -> Result<PlaybackState, ReplayError> {
    let header = read_header(env, replay_id)?;
    let index = clamp_frame_index(header.frame_count, frame_index);
    let speed = normalize_speed_bps(speed_bps);
    let turn_number = if header.frame_count == 0 {
        0
    } else {
        read_frame_state(env, replay_id, index)?.turn_number as u32
    };
    Ok(PlaybackState {
        replay_id,
        frame_index: index,
        turn_number,
        speed_bps: speed,
        interval_ms: frame_interval_ms(speed),
        frame_count: header.frame_count,
        at_end: header.frame_count == 0 || index + 1 >= header.frame_count,
    })
}

/// Seek to a 1-based turn number; turn 0 resolves to the setup frame.
pub fn seek_to_turn(
    env: &Env,
    replay_id: u64,
    turn_number: u32,
    speed_bps: u32,
) -> Result<PlaybackState, ReplayError> {
    // Presence check: `serialize_frames` reports ReplayNotFound/CorruptedReplay.
    let payload = serialize_frames(env, replay_id)?;
    let target = narrow(turn_number);
    let mut found = None;
    let mut frame_index = 0u32;
    let mut offset = 0u32;
    while offset.saturating_add(REPLAY_FRAME_BYTES as u32) <= payload.len() {
        let mut frame_bytes = [0u8; REPLAY_FRAME_BYTES];
        let mut j = 0u32;
        while j < REPLAY_FRAME_BYTES as u32 {
            frame_bytes[j as usize] = payload
                .get(offset.saturating_add(j))
                .ok_or(ReplayError::CorruptedReplay)?;
            j += 1;
        }
        if unpack_frame(&frame_bytes).turn_number == target {
            found = Some(frame_index);
            break;
        }
        frame_index += 1;
        offset += REPLAY_FRAME_BYTES as u32;
    }
    let index = if turn_number == 0 {
        0
    } else {
        found.ok_or(ReplayError::InvalidFrame)?
    };
    playback_state(env, replay_id, index, speed_bps)
}

// ── Serialization / import-export ───────────────────────────────────────────

/// Serialize a stored replay into the compact transport form.
pub fn serialize_replay(env: &Env, replay_id: u64) -> Result<ReplayExport, ReplayError> {
    let header = read_header(env, replay_id)?;
    Ok(ReplayExport {
        header,
        format: Symbol::new(env, REPLAY_FORMAT_SYMBOL),
        payload: serialize_frames(env, replay_id)?,
    })
}

/// Deserialize and publish an exported replay. Verifies format version, frame
/// bounds, per-frame codes and the integrity hash before writing any state.
pub fn deserialize_replay(
    env: &Env,
    caller: &Address,
    export: &ReplayExport,
) -> Result<u64, ReplayError> {
    caller.require_auth();
    if export.format != Symbol::new(env, REPLAY_FORMAT_SYMBOL) {
        return Err(ReplayError::UnsupportedFormat);
    }
    if export.header.version != REPLAY_FORMAT_VERSION {
        return Err(ReplayError::UnsupportedFormat);
    }
    let frame_count = export.header.frame_count;
    if frame_count == 0 || frame_count > MAX_REPLAY_FRAMES {
        return Err(ReplayError::FrameLimitExceeded);
    }
    let expected_len = frame_count.saturating_mul(REPLAY_FRAME_BYTES as u32);
    if export.payload.len() != expected_len {
        return Err(ReplayError::InvalidFrame);
    }

    let mut i = 0u32;
    while i < frame_count {
        let mut packed = [0u8; REPLAY_FRAME_BYTES];
        let mut j = 0u32;
        while j < REPLAY_FRAME_BYTES as u32 {
            packed[j as usize] = export
                .payload
                .get(i.saturating_mul(REPLAY_FRAME_BYTES as u32).saturating_add(j))
                .ok_or(ReplayError::InvalidFrame)?;
            j += 1;
        }
        let frame = unpack_frame(&packed);
        if frame.kind > FRAME_CANCEL
            || frame.action > ACTION_HEAL
            || frame.status > STATUS_CANCELLED
            || frame.winner > WINNER_P2
            || (frame.actor != ACTOR_NONE && frame.actor > 1)
            || frame.turn > 1
            || frame.power > narrow(MAX_MOVE_POWER)
        {
            return Err(ReplayError::InvalidFrame);
        }
        i += 1;
    }

    let expected_hash = compute_integrity_hash(
        env,
        export.header.version,
        export.header.combat_id,
        export.header.started_at,
        frame_count,
        &export.payload,
    );
    if expected_hash != export.header.integrity_hash {
        return Err(ReplayError::CorruptedReplay);
    }

    let replay_id = next_replay_id(env);
    let mut header = export.header.clone();
    header.replay_id = replay_id;
    write_header(env, &header);

    // Store validated frames in the same chunk layout used by live recording.
    let mut frame_index = 0u32;
    let mut chunk_no = 0u32;
    while frame_index < frame_count {
        let in_chunk = FRAMES_PER_CHUNK.min(frame_count.saturating_sub(frame_index));
        let mut chunk = Bytes::new(env);
        let mut f = 0u32;
        while f < in_chunk {
            let base = frame_index.saturating_add(f).saturating_mul(REPLAY_FRAME_BYTES as u32);
            let mut j = 0u32;
            while j < REPLAY_FRAME_BYTES as u32 {
                chunk.push_back(
                    export
                        .payload
                        .get(base.saturating_add(j))
                        .ok_or(ReplayError::InvalidFrame)?,
                );
                j += 1;
            }
            f += 1;
        }
        let key = ReplayKey::Chunk(replay_id, chunk_no);
        env.storage().persistent().set(&key, &chunk);
        touch_chunk(env, replay_id, chunk_no);
        frame_index += in_chunk;
        chunk_no += 1;
    }

    if !env
        .storage()
        .persistent()
        .has(&ReplayKey::ByCombat(header.combat_id))
    {
        env.storage()
            .persistent()
            .set(&ReplayKey::ByCombat(header.combat_id), &replay_id);
    }
    index_owner(env, &header.player1, replay_id);
    index_owner(env, &header.player2, replay_id);

    env.events().publish(
        (symbol_short!("replay"), symbol_short!("import")),
        (replay_id, frame_count),
    );
    Ok(replay_id)
}

// ── Sharing ──────────────────────────────────────────────────────────────────

fn share_symbol(env: &Env, replay_id: u64) -> Symbol {
    let bytes = share_token_bytes(replay_id);
    let token = core::str::from_utf8(&bytes).unwrap_or("rpa000000000000");
    Symbol::new(env, token)
}

/// Create (or return) a stable share token for a replay. Only the two duelists
/// may publish a link. The frontend builds `/replays?share=<token>`.
pub fn share_replay(
    env: &Env,
    caller: &Address,
    replay_id: u64,
) -> Result<ReplayShare, ReplayError> {
    caller.require_auth();
    let header = read_header(env, replay_id)?;
    if *caller != header.player1 && *caller != header.player2 {
        return Err(ReplayError::Unauthorized);
    }
    let token = share_symbol(env, replay_id);
    env.storage()
        .persistent()
        .set(&ReplayKey::Share(token.clone()), &replay_id);
    env.storage().persistent().extend_ttl(
        &ReplayKey::Share(token.clone()),
        REPLAY_TTL,
        REPLAY_MAX_TTL,
    );
    env.events().publish(
        (symbol_short!("replay"), symbol_short!("share")),
        (replay_id, token.clone()),
    );
    Ok(ReplayShare { token, replay_id })
}

pub fn replay_id_for_share(env: &Env, token: Symbol) -> Result<u64, ReplayError> {
    env.storage()
        .persistent()
        .get(&ReplayKey::Share(token))
        .ok_or(ReplayError::ShareNotFound)
}

pub fn get_replay_by_share(env: &Env, token: Symbol) -> Result<ReplayBundle, ReplayError> {
    let replay_id = replay_id_for_share(env, token)?;
    get_replay_bundle(env, replay_id)
}

// ── Tests (pure logic only) ──────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_frame() -> FrameState {
        FrameState {
            kind: FRAME_MOVE,
            actor: 1,
            action: ACTION_SPECIAL,
            status: STATUS_ACTIVE,
            winner: WINNER_NONE,
            turn: 0,
            turn_number: 7,
            power: 25,
            player1_hp: 90,
            player2_hp: 75,
            player1_energy: 88,
            player2_energy: 60,
            timestamp: 1_700_000_000,
        }
    }

    #[test]
    fn frame_pack_unpack_roundtrip() {
        let frame = sample_frame();
        let packed = pack_frame(&frame);
        assert_eq!(packed.len(), REPLAY_FRAME_BYTES);
        assert_eq!(unpack_frame(&packed), frame);
        // Reserved bytes stay zero for forward compatibility.
        assert_eq!(packed[6], 0);
        assert_eq!(packed[7], 0);
    }

    #[test]
    fn seek_and_chunk_indexing_are_bounded() {
        assert_eq!(clamp_frame_index(0, 5), 0);
        assert_eq!(clamp_frame_index(10, 99), 9);
        assert_eq!(clamp_frame_index(10, 3), 3);
        assert_eq!(chunk_index(0), 0);
        assert_eq!(chunk_index(FRAMES_PER_CHUNK - 1), 0);
        assert_eq!(chunk_index(FRAMES_PER_CHUNK), 1);
        assert_eq!(frame_offset_in_chunk(FRAMES_PER_CHUNK + 1), REPLAY_FRAME_BYTES as u32);
    }

    #[test]
    fn speed_control_is_exact_and_clamped() {
        assert_eq!(normalize_speed_bps(SPEED_HALF_BPS), SPEED_HALF_BPS);
        assert_eq!(normalize_speed_bps(SPEED_NORMAL_BPS), SPEED_NORMAL_BPS);
        assert_eq!(normalize_speed_bps(SPEED_DOUBLE_BPS), SPEED_DOUBLE_BPS);
        assert_eq!(normalize_speed_bps(7_777), SPEED_NORMAL_BPS);
        assert_eq!(frame_interval_ms(SPEED_HALF_BPS), 2_000);
        assert_eq!(frame_interval_ms(SPEED_NORMAL_BPS), 1_000);
        assert_eq!(frame_interval_ms(SPEED_DOUBLE_BPS), 500);
    }

    #[test]
    fn find_turn_scans_packed_frames() {
        let mut payload = [0u8; REPLAY_FRAME_BYTES * 4];
        let mut turn = 0u16;
        while turn < 4 {
            let mut frame = sample_frame();
            frame.turn_number = turn;
            let packed = pack_frame(&frame);
            let mut i = 0usize;
            while i < REPLAY_FRAME_BYTES {
                payload[turn as usize * REPLAY_FRAME_BYTES + i] = packed[i];
                i += 1;
            }
            turn += 1;
        }
        assert_eq!(find_turn_in_packed(&payload, 3), Some(3));
        assert_eq!(find_turn_in_packed(&payload, 0), None);
        assert_eq!(find_turn_in_packed(&payload, 99), None);
    }

    #[test]
    fn share_token_is_stable_and_url_safe() {
        let a = share_token_bytes(42);
        let b = share_token_bytes(42);
        let c = share_token_bytes(43);
        assert_eq!(a, b);
        assert_ne!(a, c);
        assert_eq!(a.len(), SHARE_TOKEN_LEN);
        assert_eq!(&a[0..2], b"rp");
        assert!(a.iter().all(|ch| ch.is_ascii_alphanumeric()));
    }
}
