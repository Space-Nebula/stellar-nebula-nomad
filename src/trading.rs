//! Advanced trading — limit orders and trading history — Issue #141
//!
//! Provides a limit-order book (buy/sell at a specified price) and a
//! ring-buffered trading history for price charts and portfolio tracking.
//! Stop-loss orders are modelled as sell-side limit orders and executed
//! by an off-chain keeper that calls `cancel_limit_order` + market sell.

use soroban_sdk::{contracterror, contracttype, symbol_short, Address, Env, Symbol, Vec};

// ── Constants ─────────────────────────────────────────────────────────────────

/// Maximum open orders per trader.
pub const MAX_ORDERS_PER_TRADER: u32 = 50;
/// Maximum records kept in the global trading history ring buffer.
pub const MAX_HISTORY: u32 = 50;

// ── Storage Keys ──────────────────────────────────────────────────────────────

#[derive(Clone)]
#[contracttype]
pub enum TradingKey {
    /// Order counter.
    OrderCounter,
    /// Open order by order_id.
    Order(u64),
    /// Open order IDs per trader.
    TraderOrders(Address),
    /// Trading history ring buffer (flat vec, newest first).
    History,
}

// ── Types ──────────────────────────────────────────────────────────────────────

#[derive(Clone, PartialEq)]
#[contracttype]
pub enum OrderSide {
    Buy,
    Sell,
}

/// A limit order placed by a trader.
#[derive(Clone)]
#[contracttype]
pub struct LimitOrder {
    pub id: u64,
    pub trader: Address,
    pub side: OrderSide,
    /// Resource symbol being traded.
    pub resource: Symbol,
    /// Quantity in resource units.
    pub quantity: i128,
    /// Limit price per unit in stroops.
    pub limit_price: i128,
    /// Timestamp when the order was placed.
    pub placed_at: u64,
    /// True if this is a stop-loss order.
    pub is_stop_loss: bool,
}

/// A completed trade recorded in history.
#[derive(Clone)]
#[contracttype]
pub struct TradeRecord {
    pub order_id: u64,
    pub trader: Address,
    pub side: OrderSide,
    pub resource: Symbol,
    pub quantity: i128,
    pub price: i128,
    pub executed_at: u64,
}

// ── Errors ────────────────────────────────────────────────────────────────────

#[contracterror]
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum TradingError {
    InvalidOrder = 1,
    OrderNotFound = 2,
    NotOrderOwner = 3,
    OrderCapReached = 4,
    InvalidPrice = 5,
    InvalidQuantity = 6,
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn next_order_id(env: &Env) -> u64 {
    let n: u64 = env
        .storage()
        .instance()
        .get(&TradingKey::OrderCounter)
        .unwrap_or(0);
    env.storage()
        .instance()
        .set(&TradingKey::OrderCounter, &(n + 1));
    n + 1
}

// ── Functions ─────────────────────────────────────────────────────────────────

/// Place a limit order (buy or sell). Emits `OrderPlaced`.
///
/// Returns the new order ID.
pub fn place_limit_order(
    env: &Env,
    trader: &Address,
    mut order: LimitOrder,
) -> Result<u64, TradingError> {
    trader.require_auth();

    if order.limit_price <= 0 {
        return Err(TradingError::InvalidPrice);
    }
    if order.quantity <= 0 {
        return Err(TradingError::InvalidQuantity);
    }

    // Enforce per-trader open order cap
    let mut ids: Vec<u64> = env
        .storage()
        .persistent()
        .get(&TradingKey::TraderOrders(trader.clone()))
        .unwrap_or_else(|| Vec::new(env));

    if ids.len() >= MAX_ORDERS_PER_TRADER {
        return Err(TradingError::OrderCapReached);
    }

    let id = next_order_id(env);
    order.id = id;
    order.trader = trader.clone();
    order.placed_at = env.ledger().timestamp();

    env.storage()
        .persistent()
        .set(&TradingKey::Order(id), &order);

    ids.push_back(id);
    env.storage()
        .persistent()
        .set(&TradingKey::TraderOrders(trader.clone()), &ids);

    env.events().publish(
        (symbol_short!("trade"), symbol_short!("placed")),
        (trader.clone(), id, order.limit_price, order.quantity),
    );

    Ok(id)
}

/// Cancel an open limit order (owner only). Emits `OrderCancelled`.
pub fn cancel_limit_order(
    env: &Env,
    trader: &Address,
    order_id: u64,
) -> Result<(), TradingError> {
    trader.require_auth();

    let order: LimitOrder = env
        .storage()
        .persistent()
        .get(&TradingKey::Order(order_id))
        .ok_or(TradingError::OrderNotFound)?;

    if &order.trader != trader {
        return Err(TradingError::NotOrderOwner);
    }

    env.storage()
        .persistent()
        .remove(&TradingKey::Order(order_id));

    // Remove from trader's order list
    let mut ids: Vec<u64> = env
        .storage()
        .persistent()
        .get(&TradingKey::TraderOrders(trader.clone()))
        .unwrap_or_else(|| Vec::new(env));
    let mut new_ids: Vec<u64> = Vec::new(env);
    for i in 0..ids.len() {
        let oid = ids.get(i).unwrap();
        if oid != order_id {
            new_ids.push_back(oid);
        }
    }
    env.storage()
        .persistent()
        .set(&TradingKey::TraderOrders(trader.clone()), &new_ids);

    env.events().publish(
        (symbol_short!("trade"), symbol_short!("cancel")),
        (trader.clone(), order_id),
    );

    Ok(())
}

/// Get a limit order by ID.
pub fn get_limit_order(env: &Env, order_id: u64) -> Option<LimitOrder> {
    env.storage().persistent().get(&TradingKey::Order(order_id))
}

/// Get all open order IDs for a trader and return the order structs.
pub fn get_trader_orders(env: &Env, trader: &Address) -> Vec<LimitOrder> {
    let ids: Vec<u64> = env
        .storage()
        .persistent()
        .get(&TradingKey::TraderOrders(trader.clone()))
        .unwrap_or_else(|| Vec::new(env));

    let mut orders: Vec<LimitOrder> = Vec::new(env);
    for i in 0..ids.len() {
        let oid = ids.get(i).unwrap();
        if let Some(order) = env.storage().persistent().get(&TradingKey::Order(oid)) {
            orders.push_back(order);
        }
    }
    orders
}

/// Record a completed trade in the history ring buffer. Emits `TradeExecuted`.
pub fn record_trade(
    env: &Env,
    caller: &Address,
    trade: TradeRecord,
) -> Result<(), TradingError> {
    caller.require_auth();

    let mut history: Vec<TradeRecord> = env
        .storage()
        .persistent()
        .get(&TradingKey::History)
        .unwrap_or_else(|| Vec::new(env));

    // Keep only the most recent MAX_HISTORY records (trim oldest)
    if history.len() >= MAX_HISTORY {
        let mut trimmed: Vec<TradeRecord> = Vec::new(env);
        let start = history.len() - MAX_HISTORY + 1;
        for i in start..history.len() {
            trimmed.push_back(history.get(i).unwrap());
        }
        history = trimmed;
    }

    history.push_back(trade.clone());
    env.storage()
        .persistent()
        .set(&TradingKey::History, &history);

    env.events().publish(
        (symbol_short!("trade"), symbol_short!("exec")),
        (caller.clone(), trade.order_id, trade.price, trade.quantity),
    );

    Ok(())
}

/// Return the full trading history (up to `MAX_HISTORY` records).
pub fn get_trading_history(env: &Env) -> Vec<TradeRecord> {
    env.storage()
        .persistent()
        .get(&TradingKey::History)
        .unwrap_or_else(|| Vec::new(env))
}
