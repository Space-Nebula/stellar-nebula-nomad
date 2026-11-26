//! Ethereum <-> Stellar bridge (Issue #713).
//!
//! Lock/release model. Each bridged asset is a Stellar token contract (for
//! example a Stellar Asset Contract) that this contract holds in custody; the
//! Ethereum side mints and burns a wrapped token. The bridge admin is the
//! relayer that watches both chains:
//!
//! * **Stellar -> Ethereum**: [`initiate_bridge`] locks the player's tokens and
//!   emits `("bridge", "initiated")`. After minting on Ethereum the relayer calls
//!   [`confirm_bridge`] with the Ethereum transaction hash and its confirmation
//!   count, then [`complete_bridge`]. A request that never reached Ethereum is
//!   refunded by [`cancel_bridge`] (player) or [`fail_bridge`] (relayer).
//! * **Ethereum -> Stellar**: after a burn on Ethereum the relayer calls
//!   [`release_from_ethereum`] with the burn's transaction hash. Each hash is
//!   accepted once.
//! * **Messages**: [`send_message`] emits a payload for Ethereum and
//!   [`receive_message`] records an inbound one. Each `(chain_id, nonce)` is
//!   accepted once.
//!
//! Per asset, `WrappedAsset::locked` equals the wrapped supply on Ethereum, and
//! the custody balance equals `locked + fees + amounts of pending requests`.

use soroban_sdk::{contracterror, contracttype, symbol_short, token, Address, Bytes, BytesN, Env, Symbol};

// ─── Constants ────────────────────────────────────────────────────────────────

/// Maximum amount that can be bridged per transaction.
pub const MAX_BRIDGE_AMOUNT: i128 = 1_000_000_000_000_000;
/// Maximum pending bridge requests per player.
pub const MAX_PENDING_REQUESTS: u32 = 100;
/// Minimum confirmations required for Ethereum transactions.
pub const MIN_CONFIRMATIONS: u32 = 12;
/// Bridge fee in basis points (0.3%).
pub const BRIDGE_FEE_BPS: u32 = 30;
/// Highest fee the admin can configure, in basis points (1%).
pub const MAX_BRIDGE_FEE_BPS: u32 = 100;
/// Largest cross-chain message payload, in bytes.
pub const MAX_MESSAGE_BYTES: u32 = 1_024;

// ─── Storage Keys ─────────────────────────────────────────────────────────────

#[derive(Clone)]
#[contracttype]
pub enum BridgeKey {
    /// Bridge request by request ID (persistent).
    Request(u64),
    /// Global request counter.
    RequestCounter,
    /// Admin address.
    StellarEthAdmin,
    /// Bridge configuration.
    StellarEthConfig,
    /// Paused state.
    StellarEthPaused,
    /// Total bridged amount per address (persistent).
    BridgedTotal(Address),
    /// Wrapped asset registry: (asset_symbol, chain_id) -> WrappedAsset.
    WrappedAsset(Symbol, u32),
    /// Asset counter.
    AssetCounter,
    /// Number of a player's requests still `Pending` (persistent).
    StellarEthPending(Address),
    /// Ethereum burn transactions already released (persistent).
    StellarEthProcessedTx(BytesN<32>),
    /// Last outbound message nonce.
    StellarEthMsgNonce,
    /// Inbound messages already received: (chain_id, nonce) (persistent).
    StellarEthProcessedMsg(u32, u64),
}

// ─── Data Types ───────────────────────────────────────────────────────────────

/// Status of a bridge request.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[contracttype]
pub enum BridgeStatus {
    /// Tokens locked, waiting for the Ethereum mint.
    Pending = 0,
    /// Ethereum mint confirmed.
    Confirmed = 1,
    /// Bridge transfer completed.
    Completed = 2,
    /// Rejected by the relayer; tokens refunded.
    Failed = 3,
    /// Cancelled by the player; tokens refunded.
    Cancelled = 4,
}

/// Direction of the bridge transfer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[contracttype]
pub enum BridgeDirection {
    /// Stellar -> Ethereum: tokens locked here, wrapped tokens minted on Ethereum.
    StellarToEth,
    /// Ethereum -> Stellar: wrapped tokens burned on Ethereum, tokens released here.
    EthToStellar,
}

/// A bridge transfer request record.
#[derive(Clone, Debug, PartialEq)]
#[contracttype]
pub struct BridgeRequest {
    /// Unique request identifier.
    pub request_id: u64,
    /// Stellar side: sender for `StellarToEth`, recipient for `EthToStellar`.
    pub player: Address,
    /// Asset being bridged.
    pub asset: Symbol,
    /// Ethereum chain ID of the asset.
    pub chain_id: u32,
    /// Amount bridged, fee included.
    pub amount: i128,
    /// Direction of transfer.
    pub direction: BridgeDirection,
    /// Ethereum side: recipient for `StellarToEth`, sender for `EthToStellar`.
    pub eth_address: BytesN<20>,
    /// Current status.
    pub status: BridgeStatus,
    /// Ethereum burn transaction (`EthToStellar`).
    pub source_tx_hash: Option<BytesN<32>>,
    /// Ethereum mint transaction (`StellarToEth`), set on confirmation.
    pub dest_tx_hash: Option<BytesN<32>>,
    /// Number of Ethereum confirmations reported by the relayer.
    pub confirmations: u32,
    /// Timestamp when request was created.
    pub created_at: u64,
    /// Timestamp when request was completed.
    pub completed_at: Option<u64>,
    /// Bridge fee charged.
    pub fee: i128,
}

/// A Stellar token bridged to an Ethereum token.
#[derive(Clone, Debug, PartialEq)]
#[contracttype]
pub struct WrappedAsset {
    /// Asset symbol on Stellar.
    pub symbol: Symbol,
    /// Stellar token contract held in custody.
    pub token: Address,
    /// Ethereum token contract address.
    pub eth_contract: BytesN<20>,
    /// Chain ID of the Ethereum network.
    pub chain_id: u32,
    /// Amount locked here backing the wrapped supply on Ethereum.
    pub locked: i128,
    /// Fees collected and not yet withdrawn.
    pub fees: i128,
    /// Whether bridging is enabled for this asset.
    pub enabled: bool,
}

/// Bridge configuration.
#[derive(Clone, Debug, PartialEq)]
#[contracttype]
pub struct BridgeConfig {
    /// Admin address for bridge operations.
    pub admin: Address,
    /// Bridge fee in basis points.
    pub fee_bps: u32,
    /// Minimum confirmations required.
    pub min_confirmations: u32,
    /// Maximum amount per transaction.
    pub max_amount: i128,
}

// ─── Errors ───────────────────────────────────────────────────────────────────

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum BridgeError {
    /// Bridge is paused.
    BridgePaused = 90,
    /// Invalid amount (zero or negative).
    InvalidAmount = 91,
    /// Amount exceeds bridge limit.
    AmountExceeded = 92,
    /// Not enough locked liquidity to release.
    InsufficientBalance = 93,
    /// Request not found.
    RequestNotFound = 94,
    /// Request is not in a valid state for this operation.
    InvalidRequestState = 95,
    /// Insufficient confirmations.
    InsufficientConfirmations = 96,
    /// Unauthorized action.
    Unauthorized = 97,
    /// The player has `MAX_PENDING_REQUESTS` pending requests.
    TooManyPendingRequests = 98,
    /// Invalid Ethereum address (zero address).
    InvalidEthAddress = 99,
    /// Asset not registered.
    AssetNotRegistered = 100,
    /// Bridge already initialized.
    AlreadyInitialized = 101,
    /// Configuration out of range or admin mismatch.
    InvalidConfig = 102,
    /// Ethereum transaction or message nonce already processed.
    AlreadyProcessed = 103,
    /// Message payload empty or larger than `MAX_MESSAGE_BYTES`.
    InvalidMessage = 104,
    /// Bridging is disabled for this asset.
    AssetDisabled = 105,
    /// Asset already registered for this chain.
    AssetAlreadyRegistered = 106,
}

// ─── Helper Functions ─────────────────────────────────────────────────────────

fn require_admin(env: &Env, caller: &Address) -> Result<(), BridgeError> {
    caller.require_auth();
    let admin: Address = env
        .storage()
        .instance()
        .get(&BridgeKey::StellarEthAdmin)
        .ok_or(BridgeError::Unauthorized)?;
    if *caller != admin {
        return Err(BridgeError::Unauthorized);
    }
    Ok(())
}

fn is_paused(env: &Env) -> bool {
    env.storage()
        .instance()
        .get(&BridgeKey::StellarEthPaused)
        .unwrap_or(false)
}

fn require_not_paused(env: &Env) -> Result<(), BridgeError> {
    if is_paused(env) {
        return Err(BridgeError::BridgePaused);
    }
    Ok(())
}

fn next_request_id(env: &Env) -> u64 {
    let current: u64 = env
        .storage()
        .instance()
        .get(&BridgeKey::RequestCounter)
        .unwrap_or(0);
    let next = current + 1;
    env.storage()
        .instance()
        .set(&BridgeKey::RequestCounter, &next);
    next
}

fn calculate_fee(amount: i128, fee_bps: u32) -> i128 {
    amount * i128::from(fee_bps) / 10_000
}

fn validate_amount(config: &BridgeConfig, amount: i128) -> Result<(), BridgeError> {
    if amount <= 0 {
        return Err(BridgeError::InvalidAmount);
    }
    if amount > config.max_amount {
        return Err(BridgeError::AmountExceeded);
    }
    Ok(())
}

fn validate_eth_address(address: &BytesN<20>) -> Result<(), BridgeError> {
    if address.to_array() == [0u8; 20] {
        return Err(BridgeError::InvalidEthAddress);
    }
    Ok(())
}

fn validate_payload(payload: &Bytes) -> Result<(), BridgeError> {
    if payload.is_empty() || payload.len() > MAX_MESSAGE_BYTES {
        return Err(BridgeError::InvalidMessage);
    }
    Ok(())
}

fn save_request(env: &Env, request: &BridgeRequest) {
    env.storage()
        .persistent()
        .set(&BridgeKey::Request(request.request_id), request);
}

fn load_asset(env: &Env, symbol: Symbol, chain_id: u32) -> Result<WrappedAsset, BridgeError> {
    env.storage()
        .instance()
        .get(&BridgeKey::WrappedAsset(symbol, chain_id))
        .ok_or(BridgeError::AssetNotRegistered)
}

fn load_enabled_asset(env: &Env, symbol: Symbol, chain_id: u32) -> Result<WrappedAsset, BridgeError> {
    let asset = load_asset(env, symbol, chain_id)?;
    if !asset.enabled {
        return Err(BridgeError::AssetDisabled);
    }
    Ok(asset)
}

fn save_asset(env: &Env, asset: &WrappedAsset) {
    env.storage().instance().set(
        &BridgeKey::WrappedAsset(asset.symbol.clone(), asset.chain_id),
        asset,
    );
}

fn set_pending_count(env: &Env, player: &Address, count: u32) {
    env.storage()
        .persistent()
        .set(&BridgeKey::StellarEthPending(player.clone()), &count);
}

/// Refund a pending outbound request and close it with `status`.
fn refund_pending(
    env: &Env,
    mut request: BridgeRequest,
    status: BridgeStatus,
) -> Result<(), BridgeError> {
    if request.status != BridgeStatus::Pending {
        return Err(BridgeError::InvalidRequestState);
    }
    let asset = load_asset(env, request.asset.clone(), request.chain_id)?;

    request.status = status;
    request.completed_at = Some(env.ledger().timestamp());
    save_request(env, &request);
    set_pending_count(env, &request.player, count_pending_requests(env, &request.player) - 1);

    token::TokenClient::new(env, &asset.token).transfer(
        &env.current_contract_address(),
        &request.player,
        &request.amount,
    );
    Ok(())
}

// ─── Initialization ───────────────────────────────────────────────────────────

/// Initialize the bridge with admin and default configuration. Runs once.
pub fn initialize_bridge(env: &Env, admin: &Address) -> Result<(), BridgeError> {
    admin.require_auth();
    if env.storage().instance().has(&BridgeKey::StellarEthAdmin) {
        return Err(BridgeError::AlreadyInitialized);
    }
    env.storage().instance().set(&BridgeKey::StellarEthAdmin, admin);
    env.storage().instance().set(&BridgeKey::RequestCounter, &0u64);
    env.storage().instance().set(
        &BridgeKey::StellarEthConfig,
        &BridgeConfig {
            admin: admin.clone(),
            fee_bps: BRIDGE_FEE_BPS,
            min_confirmations: MIN_CONFIRMATIONS,
            max_amount: MAX_BRIDGE_AMOUNT,
        },
    );
    Ok(())
}

/// Pause or unpause the bridge (admin only).
pub fn set_bridge_paused(env: &Env, admin: &Address, paused: bool) -> Result<(), BridgeError> {
    require_admin(env, admin)?;
    env.storage().instance().set(&BridgeKey::StellarEthPaused, &paused);
    Ok(())
}

/// Update bridge configuration (admin only).
///
/// The fee is capped at `MAX_BRIDGE_FEE_BPS`, confirmations and the per-transfer
/// limit must be non-zero, and `config.admin` must stay the current admin.
pub fn update_bridge_config(
    env: &Env,
    admin: &Address,
    config: BridgeConfig,
) -> Result<(), BridgeError> {
    require_admin(env, admin)?;
    if config.fee_bps > MAX_BRIDGE_FEE_BPS
        || config.min_confirmations == 0
        || config.max_amount <= 0
        || config.admin != *admin
    {
        return Err(BridgeError::InvalidConfig);
    }
    env.storage().instance().set(&BridgeKey::StellarEthConfig, &config);
    Ok(())
}

// ─── Asset Registration ──────────────────────────────────────────────────────

/// Register a Stellar token for bridging to an Ethereum token (admin only).
pub fn register_wrapped_asset(
    env: &Env,
    admin: &Address,
    symbol: Symbol,
    token: Address,
    eth_contract: BytesN<20>,
    chain_id: u32,
) -> Result<u64, BridgeError> {
    require_admin(env, admin)?;
    validate_eth_address(&eth_contract)?;
    if load_asset(env, symbol.clone(), chain_id).is_ok() {
        return Err(BridgeError::AssetAlreadyRegistered);
    }

    let asset_counter: u64 = env
        .storage()
        .instance()
        .get(&BridgeKey::AssetCounter)
        .unwrap_or(0);
    let asset_id = asset_counter + 1;
    env.storage()
        .instance()
        .set(&BridgeKey::AssetCounter, &asset_id);

    save_asset(
        env,
        &WrappedAsset {
            symbol: symbol.clone(),
            token,
            eth_contract,
            chain_id,
            locked: 0,
            fees: 0,
            enabled: true,
        },
    );

    env.events().publish(
        (symbol_short!("bridge"), symbol_short!("asset_reg")),
        (asset_id, symbol, chain_id),
    );

    Ok(asset_id)
}

/// Enable or disable bridging for one asset (admin only).
pub fn set_wrapped_asset_enabled(
    env: &Env,
    admin: &Address,
    symbol: Symbol,
    chain_id: u32,
    enabled: bool,
) -> Result<(), BridgeError> {
    require_admin(env, admin)?;
    let mut asset = load_asset(env, symbol, chain_id)?;
    asset.enabled = enabled;
    save_asset(env, &asset);
    Ok(())
}

/// Get wrapped asset info, including locked liquidity and collected fees.
pub fn get_wrapped_asset(
    env: &Env,
    symbol: Symbol,
    chain_id: u32,
) -> Result<WrappedAsset, BridgeError> {
    load_asset(env, symbol, chain_id)
}

// ─── Bridge Operations ───────────────────────────────────────────────────────

/// Lock `amount` of `asset` and request its release to `eth_recipient` on Ethereum.
///
/// The fee is taken from `amount`; `amount - fee` is minted on Ethereum.
pub fn initiate_bridge(
    env: &Env,
    player: Address,
    asset: Symbol,
    chain_id: u32,
    amount: i128,
    eth_recipient: BytesN<20>,
) -> Result<u64, BridgeError> {
    player.require_auth();
    require_not_paused(env)?;

    let config = get_bridge_config(env)?;
    validate_amount(&config, amount)?;
    validate_eth_address(&eth_recipient)?;
    let wrapped = load_enabled_asset(env, asset.clone(), chain_id)?;

    let pending_count = count_pending_requests(env, &player);
    if pending_count >= MAX_PENDING_REQUESTS {
        return Err(BridgeError::TooManyPendingRequests);
    }

    let fee = calculate_fee(amount, config.fee_bps);
    let request_id = next_request_id(env);

    save_request(
        env,
        &BridgeRequest {
            request_id,
            player: player.clone(),
            asset: asset.clone(),
            chain_id,
            amount,
            direction: BridgeDirection::StellarToEth,
            eth_address: eth_recipient.clone(),
            status: BridgeStatus::Pending,
            source_tx_hash: None,
            dest_tx_hash: None,
            confirmations: 0,
            created_at: env.ledger().timestamp(),
            completed_at: None,
            fee,
        },
    );
    set_pending_count(env, &player, pending_count + 1);

    let current_total = get_bridged_total(env, &player);
    env.storage().persistent().set(
        &BridgeKey::BridgedTotal(player.clone()),
        &(current_total + amount),
    );

    token::TokenClient::new(env, &wrapped.token).transfer(
        &player,
        &env.current_contract_address(),
        &amount,
    );

    env.events().publish(
        (symbol_short!("bridge"), symbol_short!("initiated")),
        (request_id, player, asset, chain_id, amount, fee, eth_recipient),
    );

    Ok(request_id)
}

/// Confirm the Ethereum mint for a pending request (relayer only).
///
/// Moves the bridged amount into `locked` and the fee into `fees`.
pub fn confirm_bridge(
    env: &Env,
    admin: Address,
    request_id: u64,
    dest_tx_hash: BytesN<32>,
    confirmations: u32,
) -> Result<(), BridgeError> {
    require_admin(env, &admin)?;

    let config = get_bridge_config(env)?;
    if confirmations < config.min_confirmations {
        return Err(BridgeError::InsufficientConfirmations);
    }

    let mut request = get_bridge_request(env, request_id)?;
    if request.status != BridgeStatus::Pending {
        return Err(BridgeError::InvalidRequestState);
    }

    let mut asset = load_asset(env, request.asset.clone(), request.chain_id)?;
    asset.locked += request.amount - request.fee;
    asset.fees += request.fee;
    save_asset(env, &asset);

    request.status = BridgeStatus::Confirmed;
    request.dest_tx_hash = Some(dest_tx_hash);
    request.confirmations = confirmations;
    save_request(env, &request);
    set_pending_count(env, &request.player, count_pending_requests(env, &request.player) - 1);

    env.events().publish(
        (symbol_short!("bridge"), symbol_short!("confirmed")),
        (request_id, confirmations),
    );

    Ok(())
}

/// Complete a confirmed request (relayer only).
pub fn complete_bridge(env: &Env, admin: Address, request_id: u64) -> Result<(), BridgeError> {
    require_admin(env, &admin)?;

    let mut request = get_bridge_request(env, request_id)?;
    if request.status != BridgeStatus::Confirmed {
        return Err(BridgeError::InvalidRequestState);
    }

    request.status = BridgeStatus::Completed;
    request.completed_at = Some(env.ledger().timestamp());
    save_request(env, &request);

    env.events().publish(
        (symbol_short!("bridge"), symbol_short!("completed")),
        (request_id, request.player, request.amount),
    );

    Ok(())
}

/// Cancel a pending request and refund the full amount (player only).
pub fn cancel_bridge(env: &Env, player: Address, request_id: u64) -> Result<(), BridgeError> {
    player.require_auth();

    let request = get_bridge_request(env, request_id)?;
    if request.player != player {
        return Err(BridgeError::Unauthorized);
    }
    refund_pending(env, request, BridgeStatus::Cancelled)?;

    env.events().publish(
        (symbol_short!("bridge"), symbol_short!("cancelled")),
        (request_id, player),
    );

    Ok(())
}

/// Reject a pending request that could not be minted on Ethereum and refund
/// the full amount (relayer only).
pub fn fail_bridge(env: &Env, admin: Address, request_id: u64) -> Result<(), BridgeError> {
    require_admin(env, &admin)?;

    let request = get_bridge_request(env, request_id)?;
    let player = request.player.clone();
    refund_pending(env, request, BridgeStatus::Failed)?;

    env.events().publish(
        (symbol_short!("bridge"), symbol_short!("failed")),
        (request_id, player),
    );

    Ok(())
}

/// Release tokens on Stellar for a burn on Ethereum (relayer only).
///
/// `eth_tx_hash` is the burn transaction; each hash is released once. The
/// recipient receives `amount - fee`, and `amount` leaves `locked`.
#[allow(clippy::too_many_arguments)]
pub fn release_from_ethereum(
    env: &Env,
    admin: Address,
    eth_tx_hash: BytesN<32>,
    eth_sender: BytesN<20>,
    asset: Symbol,
    chain_id: u32,
    recipient: Address,
    amount: i128,
    confirmations: u32,
) -> Result<u64, BridgeError> {
    require_admin(env, &admin)?;
    require_not_paused(env)?;

    let config = get_bridge_config(env)?;
    if confirmations < config.min_confirmations {
        return Err(BridgeError::InsufficientConfirmations);
    }
    validate_amount(&config, amount)?;
    let processed_key = BridgeKey::StellarEthProcessedTx(eth_tx_hash.clone());
    if env.storage().persistent().has(&processed_key) {
        return Err(BridgeError::AlreadyProcessed);
    }
    let mut wrapped = load_enabled_asset(env, asset.clone(), chain_id)?;
    if amount > wrapped.locked {
        return Err(BridgeError::InsufficientBalance);
    }

    let fee = calculate_fee(amount, config.fee_bps);
    wrapped.locked -= amount;
    wrapped.fees += fee;
    save_asset(env, &wrapped);
    env.storage().persistent().set(&processed_key, &true);

    let now = env.ledger().timestamp();
    let request_id = next_request_id(env);
    save_request(
        env,
        &BridgeRequest {
            request_id,
            player: recipient.clone(),
            asset: asset.clone(),
            chain_id,
            amount,
            direction: BridgeDirection::EthToStellar,
            eth_address: eth_sender,
            status: BridgeStatus::Completed,
            source_tx_hash: Some(eth_tx_hash),
            dest_tx_hash: None,
            confirmations,
            created_at: now,
            completed_at: Some(now),
            fee,
        },
    );

    token::TokenClient::new(env, &wrapped.token).transfer(
        &env.current_contract_address(),
        &recipient,
        &(amount - fee),
    );

    env.events().publish(
        (symbol_short!("bridge"), symbol_short!("released")),
        (request_id, recipient, asset, chain_id, amount, fee),
    );

    Ok(request_id)
}

/// Whether an Ethereum burn transaction has already been released.
pub fn is_eth_tx_processed(env: &Env, eth_tx_hash: BytesN<32>) -> bool {
    env.storage()
        .persistent()
        .has(&BridgeKey::StellarEthProcessedTx(eth_tx_hash))
}

// ─── Fee Management ───────────────────────────────────────────────────────────

/// Transfer the fees collected for an asset to `to` (admin only). Returns the amount.
pub fn withdraw_bridge_fees(
    env: &Env,
    admin: Address,
    asset: Symbol,
    chain_id: u32,
    to: Address,
) -> Result<i128, BridgeError> {
    require_admin(env, &admin)?;

    let mut wrapped = load_asset(env, asset.clone(), chain_id)?;
    let amount = wrapped.fees;
    if amount == 0 {
        return Ok(0);
    }
    wrapped.fees = 0;
    save_asset(env, &wrapped);

    token::TokenClient::new(env, &wrapped.token).transfer(
        &env.current_contract_address(),
        &to,
        &amount,
    );

    env.events().publish(
        (symbol_short!("bridge"), symbol_short!("fees_out")),
        (asset, chain_id, to, amount),
    );

    Ok(amount)
}

// ─── Cross-Chain Messaging ────────────────────────────────────────────────────

/// Send `payload` to `eth_target` on `chain_id`. Returns the message nonce,
/// which the relayer carries to Ethereum with the payload from the
/// `("bridge", "msg_out")` event.
pub fn send_message(
    env: &Env,
    sender: Address,
    chain_id: u32,
    eth_target: BytesN<20>,
    payload: Bytes,
) -> Result<u64, BridgeError> {
    sender.require_auth();
    require_not_paused(env)?;
    validate_eth_address(&eth_target)?;
    validate_payload(&payload)?;

    let nonce: u64 = env
        .storage()
        .instance()
        .get(&BridgeKey::StellarEthMsgNonce)
        .unwrap_or(0)
        + 1;
    env.storage()
        .instance()
        .set(&BridgeKey::StellarEthMsgNonce, &nonce);

    env.events().publish(
        (symbol_short!("bridge"), symbol_short!("msg_out")),
        (nonce, sender, chain_id, eth_target, payload),
    );

    Ok(nonce)
}

/// Record a message sent from Ethereum (relayer only) and emit it as
/// `("bridge", "msg_in")` for `recipient`. Each `(chain_id, nonce)` is accepted once.
pub fn receive_message(
    env: &Env,
    admin: Address,
    chain_id: u32,
    nonce: u64,
    eth_sender: BytesN<20>,
    recipient: Address,
    payload: Bytes,
) -> Result<(), BridgeError> {
    require_admin(env, &admin)?;
    require_not_paused(env)?;
    validate_payload(&payload)?;

    let key = BridgeKey::StellarEthProcessedMsg(chain_id, nonce);
    if env.storage().persistent().has(&key) {
        return Err(BridgeError::AlreadyProcessed);
    }
    env.storage().persistent().set(&key, &true);

    env.events().publish(
        (symbol_short!("bridge"), symbol_short!("msg_in")),
        (chain_id, nonce, eth_sender, recipient, payload),
    );

    Ok(())
}

/// Whether the inbound message `(chain_id, nonce)` has been received.
pub fn is_message_processed(env: &Env, chain_id: u32, nonce: u64) -> bool {
    env.storage()
        .persistent()
        .has(&BridgeKey::StellarEthProcessedMsg(chain_id, nonce))
}

// ─── Read Queries ─────────────────────────────────────────────────────────────

/// Get a bridge request by ID.
pub fn get_bridge_request(env: &Env, request_id: u64) -> Result<BridgeRequest, BridgeError> {
    env.storage()
        .persistent()
        .get(&BridgeKey::Request(request_id))
        .ok_or(BridgeError::RequestNotFound)
}

/// Get total amount bridged by an address.
pub fn get_bridged_total(env: &Env, player: &Address) -> i128 {
    env.storage()
        .persistent()
        .get(&BridgeKey::BridgedTotal(player.clone()))
        .unwrap_or(0)
}

/// Get bridge configuration.
pub fn get_bridge_config(env: &Env) -> Result<BridgeConfig, BridgeError> {
    env.storage()
        .instance()
        .get(&BridgeKey::StellarEthConfig)
        .ok_or(BridgeError::Unauthorized)
}

/// Count a player's pending bridge requests. Per player, so one account
/// cannot fill a shared queue and block everyone else.
fn count_pending_requests(env: &Env, player: &Address) -> u32 {
    env.storage()
        .persistent()
        .get(&BridgeKey::StellarEthPending(player.clone()))
        .unwrap_or(0)
}

// ─── Security Audit Helpers ──────────────────────────────────────────────────

/// Verify the integrity of a bridge request.
/// Checks that all required fields are populated and consistent.
pub fn verify_request_integrity(env: &Env, request_id: u64) -> Result<bool, BridgeError> {
    let request = get_bridge_request(env, request_id)?;

    // Verify request_id matches
    if request.request_id != request_id {
        return Ok(false);
    }

    // Verify amount is positive
    if request.amount <= 0 {
        return Ok(false);
    }

    // Verify fee is non-negative
    if request.fee < 0 {
        return Ok(false);
    }

    // Verify fee doesn't exceed amount
    if request.fee >= request.amount {
        return Ok(false);
    }

    Ok(true)
}

/// Check if an address has exceeded their daily bridge limit.
pub fn check_daily_limit(env: &Env, player: &Address, daily_limit: i128) -> bool {
    get_bridged_total(env, player) < daily_limit
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_helpers::{has_event_topics, make_env};
    use crate::NebulaNomadContractClient;
    use soroban_sdk::testutils::Address as _;
    use soroban_sdk::token::{StellarAssetClient, TokenClient};

    const CHAIN: u32 = 1;

    struct Bridge {
        env: Env,
        client: NebulaNomadContractClient<'static>,
        admin: Address,
        player: Address,
        token: TokenClient<'static>,
    }

    /// Bridge initialized with an `XLM` asset; the player holds 1_000_000.
    fn setup() -> Bridge {
        let (env, client, player) = make_env();
        let admin = Address::generate(&env);
        let sac = env.register_stellar_asset_contract_v2(admin.clone());
        StellarAssetClient::new(&env, &sac.address()).mint(&player, &1_000_000);
        client.bridge_initialize(&admin);
        client.bridge_register_asset(
            &admin,
            &symbol_short!("XLM"),
            &sac.address(),
            &BytesN::from_array(&env, &[7u8; 20]),
            &CHAIN,
        );
        let token = TokenClient::new(&env, &sac.address());
        Bridge { env, client, admin, player, token }
    }

    impl Bridge {
        fn initiate(&self, amount: i128) -> u64 {
            self.client.bridge_initiate(
                &self.player,
                &symbol_short!("XLM"),
                &CHAIN,
                &amount,
                &BytesN::from_array(&self.env, &[1u8; 20]),
            )
        }

        fn eth(&self, byte: u8) -> BytesN<20> {
            BytesN::from_array(&self.env, &[byte; 20])
        }

        fn hash(&self, byte: u8) -> BytesN<32> {
            BytesN::from_array(&self.env, &[byte; 32])
        }

        fn asset(&self) -> WrappedAsset {
            self.client.bridge_get_asset(&symbol_short!("XLM"), &CHAIN)
        }

        fn custody(&self) -> i128 {
            self.token.balance(&self.client.address)
        }
    }

    #[test]
    fn test_initialize_bridge_once() {
        let b = setup();
        let config = b.client.bridge_get_config();
        assert_eq!(config.fee_bps, BRIDGE_FEE_BPS);
        assert_eq!(config.min_confirmations, MIN_CONFIRMATIONS);

        let intruder = Address::generate(&b.env);
        assert_eq!(
            b.client.try_bridge_initialize(&intruder),
            Err(Ok(BridgeError::AlreadyInitialized))
        );
    }

    #[test]
    fn test_config_validation_caps_fee() {
        let b = setup();
        let mut config = b.client.bridge_get_config();
        config.fee_bps = MAX_BRIDGE_FEE_BPS + 1;
        assert_eq!(
            b.client.try_bridge_update_config(&b.admin, &config),
            Err(Ok(BridgeError::InvalidConfig))
        );

        config.fee_bps = MAX_BRIDGE_FEE_BPS;
        config.min_confirmations = 0;
        assert_eq!(
            b.client.try_bridge_update_config(&b.admin, &config),
            Err(Ok(BridgeError::InvalidConfig))
        );

        config.min_confirmations = 6;
        b.client.bridge_update_config(&b.admin, &config);
        assert_eq!(b.client.bridge_get_config().fee_bps, MAX_BRIDGE_FEE_BPS);

        let stranger = Address::generate(&b.env);
        assert_eq!(
            b.client.try_bridge_update_config(&stranger, &config),
            Err(Ok(BridgeError::Unauthorized))
        );
    }

    #[test]
    fn test_initiate_locks_tokens_and_emits_event() {
        let b = setup();
        let id = b.initiate(10_000);

        let request = b.client.bridge_get_request(&id);
        assert_eq!(request.status, BridgeStatus::Pending);
        assert_eq!(request.amount, 10_000);
        assert_eq!(request.fee, 30);
        assert_eq!(request.direction, BridgeDirection::StellarToEth);
        assert_eq!(b.token.balance(&b.player), 990_000);
        assert_eq!(b.custody(), 10_000);
        assert_eq!(b.client.bridge_get_bridged_total(&b.player), 10_000);
        assert!(has_event_topics(&b.env, &["bridge", "initiated"]));
        let client_id = b.client.address.clone();
        assert!(b
            .env
            .as_contract(&client_id, || verify_request_integrity(&b.env, id))
            .unwrap());
    }

    #[test]
    fn test_initiate_rejections() {
        let b = setup();
        let recipient = BytesN::from_array(&b.env, &[1u8; 20]);
        let xlm = symbol_short!("XLM");

        assert_eq!(
            b.client.try_bridge_initiate(&b.player, &xlm, &CHAIN, &0, &recipient),
            Err(Ok(BridgeError::InvalidAmount))
        );
        assert_eq!(
            b.client
                .try_bridge_initiate(&b.player, &xlm, &CHAIN, &(MAX_BRIDGE_AMOUNT + 1), &recipient),
            Err(Ok(BridgeError::AmountExceeded))
        );
        assert_eq!(
            b.client.try_bridge_initiate(
                &b.player,
                &xlm,
                &CHAIN,
                &100,
                &BytesN::from_array(&b.env, &[0u8; 20])
            ),
            Err(Ok(BridgeError::InvalidEthAddress))
        );
        assert_eq!(
            b.client
                .try_bridge_initiate(&b.player, &symbol_short!("USDC"), &CHAIN, &100, &recipient),
            Err(Ok(BridgeError::AssetNotRegistered))
        );

        b.client.bridge_set_asset_enabled(&b.admin, &xlm, &CHAIN, &false);
        assert_eq!(
            b.client.try_bridge_initiate(&b.player, &xlm, &CHAIN, &100, &recipient),
            Err(Ok(BridgeError::AssetDisabled))
        );
        b.client.bridge_set_asset_enabled(&b.admin, &xlm, &CHAIN, &true);

        b.client.bridge_set_paused(&b.admin, &true);
        assert_eq!(
            b.client.try_bridge_initiate(&b.player, &xlm, &CHAIN, &100, &recipient),
            Err(Ok(BridgeError::BridgePaused))
        );
        assert_eq!(b.custody(), 0);
    }

    #[test]
    fn test_full_outbound_flow_tracks_locked_and_fees() {
        let b = setup();
        let id = b.initiate(10_000);

        assert_eq!(
            b.client
                .try_bridge_confirm(&b.admin, &id, &b.hash(9), &(MIN_CONFIRMATIONS - 1)),
            Err(Ok(BridgeError::InsufficientConfirmations))
        );
        assert_eq!(
            b.client.try_bridge_complete(&b.admin, &id),
            Err(Ok(BridgeError::InvalidRequestState))
        );

        b.client
            .bridge_confirm(&b.admin, &id, &b.hash(9), &MIN_CONFIRMATIONS);
        let asset = b.asset();
        assert_eq!(asset.locked, 9_970);
        assert_eq!(asset.fees, 30);

        b.client.bridge_complete(&b.admin, &id);
        let request = b.client.bridge_get_request(&id);
        assert_eq!(request.status, BridgeStatus::Completed);
        assert_eq!(request.dest_tx_hash, Some(b.hash(9)));
        assert!(request.completed_at.is_some());

        // Confirmed requests can no longer be refunded.
        assert_eq!(
            b.client.try_bridge_cancel(&b.player, &id),
            Err(Ok(BridgeError::InvalidRequestState))
        );
        assert_eq!(b.custody(), asset.locked + asset.fees);
    }

    #[test]
    fn test_cancel_and_fail_refund_pending_requests() {
        let b = setup();
        let cancelled = b.initiate(10_000);
        let failed = b.initiate(20_000);
        assert_eq!(b.token.balance(&b.player), 970_000);

        let stranger = Address::generate(&b.env);
        assert_eq!(
            b.client.try_bridge_cancel(&stranger, &cancelled),
            Err(Ok(BridgeError::Unauthorized))
        );

        b.client.bridge_cancel(&b.player, &cancelled);
        b.client.bridge_fail(&b.admin, &failed);

        assert_eq!(b.token.balance(&b.player), 1_000_000);
        assert_eq!(b.custody(), 0);
        assert_eq!(
            b.client.bridge_get_request(&cancelled).status,
            BridgeStatus::Cancelled
        );
        assert_eq!(b.client.bridge_get_request(&failed).status, BridgeStatus::Failed);
        assert_eq!(
            b.client.try_bridge_cancel(&b.player, &cancelled),
            Err(Ok(BridgeError::InvalidRequestState))
        );
        assert!(has_event_topics(&b.env, &["bridge", "failed"]));
    }

    #[test]
    fn test_pending_limit_is_per_player() {
        let b = setup();
        for _ in 0..MAX_PENDING_REQUESTS {
            b.initiate(10);
        }
        assert_eq!(
            b.client.try_bridge_initiate(
                &b.player,
                &symbol_short!("XLM"),
                &CHAIN,
                &10,
                &BytesN::from_array(&b.env, &[1u8; 20])
            ),
            Err(Ok(BridgeError::TooManyPendingRequests))
        );

        // The cap is per player: others can still bridge.
        let other = Address::generate(&b.env);
        StellarAssetClient::new(&b.env, &b.token.address).mint(&other, &10);
        b.client.bridge_initiate(
            &other,
            &symbol_short!("XLM"),
            &CHAIN,
            &10,
            &BytesN::from_array(&b.env, &[1u8; 20]),
        );

        // Settling one request frees a slot.
        b.client.bridge_cancel(&b.player, &1);
        b.initiate(10);
    }

    #[test]
    fn test_release_from_ethereum_is_replay_protected() {
        let b = setup();
        let id = b.initiate(100_000);
        b.client
            .bridge_confirm(&b.admin, &id, &b.hash(9), &MIN_CONFIRMATIONS);
        let recipient = Address::generate(&b.env);
        let burn = b.hash(5);
        let xlm = symbol_short!("XLM");

        let release = b.client.bridge_release(
            &b.admin,
            &burn,
            &b.eth(3),
            &xlm,
            &CHAIN,
            &recipient,
            &50_000,
            &MIN_CONFIRMATIONS,
        );

        assert_eq!(b.token.balance(&recipient), 49_850);
        let asset = b.asset();
        assert_eq!(asset.locked, 99_700 - 50_000);
        assert_eq!(asset.fees, 300 + 150);
        assert_eq!(b.custody(), asset.locked + asset.fees);
        assert!(b.client.bridge_is_eth_tx_processed(&burn));
        let request = b.client.bridge_get_request(&release);
        assert_eq!(request.direction, BridgeDirection::EthToStellar);
        assert_eq!(request.status, BridgeStatus::Completed);
        assert_eq!(request.source_tx_hash, Some(burn.clone()));
        assert!(has_event_topics(&b.env, &["bridge", "released"]));

        assert_eq!(
            b.client.try_bridge_release(
                &b.admin,
                &burn,
                &b.eth(3),
                &xlm,
                &CHAIN,
                &recipient,
                &1_000,
                &MIN_CONFIRMATIONS
            ),
            Err(Ok(BridgeError::AlreadyProcessed))
        );
        assert_eq!(
            b.client.try_bridge_release(
                &b.admin,
                &b.hash(6),
                &b.eth(3),
                &xlm,
                &CHAIN,
                &recipient,
                &(asset.locked + 1),
                &MIN_CONFIRMATIONS
            ),
            Err(Ok(BridgeError::InsufficientBalance))
        );
        let stranger = Address::generate(&b.env);
        assert_eq!(
            b.client.try_bridge_release(
                &stranger,
                &b.hash(6),
                &b.eth(3),
                &xlm,
                &CHAIN,
                &recipient,
                &1_000,
                &MIN_CONFIRMATIONS
            ),
            Err(Ok(BridgeError::Unauthorized))
        );
    }

    #[test]
    fn test_withdraw_fees() {
        let b = setup();
        let id = b.initiate(1_000_000);
        b.client
            .bridge_confirm(&b.admin, &id, &b.hash(9), &MIN_CONFIRMATIONS);
        let treasury = Address::generate(&b.env);

        let xlm = symbol_short!("XLM");
        assert_eq!(
            b.client.bridge_withdraw_fees(&b.admin, &xlm, &CHAIN, &treasury),
            3_000
        );
        assert_eq!(b.token.balance(&treasury), 3_000);
        assert_eq!(b.asset().fees, 0);
        assert_eq!(b.custody(), b.asset().locked);
        assert_eq!(
            b.client.bridge_withdraw_fees(&b.admin, &xlm, &CHAIN, &treasury),
            0
        );
    }

    #[test]
    fn test_messaging() {
        let b = setup();
        let payload = Bytes::from_slice(&b.env, b"claim:ship:42");

        let first = b.client.bridge_send_message(&b.player, &CHAIN, &b.eth(4), &payload);
        let second = b.client.bridge_send_message(&b.player, &CHAIN, &b.eth(4), &payload);
        assert_eq!((first, second), (1, 2));
        assert!(has_event_topics(&b.env, &["bridge", "msg_out"]));
        assert_eq!(
            b.client
                .try_bridge_send_message(&b.player, &CHAIN, &b.eth(4), &Bytes::new(&b.env)),
            Err(Ok(BridgeError::InvalidMessage))
        );
        let oversized = Bytes::from_array(&b.env, &[1u8; MAX_MESSAGE_BYTES as usize + 1]);
        assert_eq!(
            b.client
                .try_bridge_send_message(&b.player, &CHAIN, &b.eth(4), &oversized),
            Err(Ok(BridgeError::InvalidMessage))
        );

        assert!(!b.client.bridge_is_message_processed(&CHAIN, &7));
        b.client
            .bridge_receive_message(&b.admin, &CHAIN, &7, &b.eth(4), &b.player, &payload);
        assert!(b.client.bridge_is_message_processed(&CHAIN, &7));
        assert!(has_event_topics(&b.env, &["bridge", "msg_in"]));
        assert_eq!(
            b.client
                .try_bridge_receive_message(&b.admin, &CHAIN, &7, &b.eth(4), &b.player, &payload),
            Err(Ok(BridgeError::AlreadyProcessed))
        );
        // Same nonce from another chain is a different message.
        b.client
            .bridge_receive_message(&b.admin, &(CHAIN + 1), &7, &b.eth(4), &b.player, &payload);
    }

    #[test]
    fn test_register_asset_twice_rejected() {
        let b = setup();
        let token = b.token.address.clone();
        assert_eq!(
            b.client.try_bridge_register_asset(
                &b.admin,
                &symbol_short!("XLM"),
                &token,
                &b.eth(7),
                &CHAIN
            ),
            Err(Ok(BridgeError::AssetAlreadyRegistered))
        );
    }

    #[test]
    fn test_calculate_fee() {
        assert_eq!(calculate_fee(10_000, 30), 30);
        assert_eq!(calculate_fee(1_000_000, 30), 3_000);
        assert_eq!(calculate_fee(0, 30), 0);
        assert_eq!(calculate_fee(MAX_BRIDGE_AMOUNT, MAX_BRIDGE_FEE_BPS), MAX_BRIDGE_AMOUNT / 100);
    }
}
