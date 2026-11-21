//! Market oracle and price feeds (Issues #78, #654).
//!
//! Prices reach the contract through two paths:
//!
//! 1. **Source feeds (normal path).** Each registered oracle source signs and
//!    submits its own observation with [`submit_source_price`]. Anyone (a
//!    keeper) can then call [`aggregate_resource_price`], which builds a
//!    consensus price from the fresh observations:
//!    * observations older than [`MAX_OBSERVATION_AGE_SECS`] are ignored;
//!    * the median of the fresh observations is computed;
//!    * observations further than [`MAX_SOURCE_DEVIATION_BPS`] from that
//!      median are discarded as outliers;
//!    * consensus requires at least [`MIN_ORACLE_SOURCES`] agreeing sources
//!      **and** a strict majority of the fresh sources;
//!    * the published price is the median of the agreeing sources;
//!    * a circuit breaker rejects a consensus price that moves more than
//!      [`MAX_PRICE_JUMP_BPS`] from the last fresh published price.
//! 2. **Admin override.** [`update_resource_price`] / [`batch_update_prices`]
//!    let the oracle admin publish a price directly, e.g. to recover after the
//!    circuit breaker trips. Overrides still go through price validation.
//!
//! Consumers read the published price with [`get_current_market_rate`], which
//! refuses prices older than [`MAX_PRICE_AGE_SECS`]. The dynamic pricing engine
//! consumes it through `dynamic_pricing::observe_oracle_price`.

use soroban_sdk::{contracterror, contracttype, symbol_short, Address, Env, Symbol, Vec};

// Oracle configuration constants
/// A published price older than this is stale and is not served.
pub const MAX_PRICE_AGE_SECS: u64 = 86_400; // 24 hours
/// A source observation older than this does not count towards consensus.
pub const MAX_OBSERVATION_AGE_SECS: u64 = 3_600; // 1 hour
pub const MAX_BATCH_UPDATE: u32 = 20;
/// Minimum number of agreeing sources (quorum) for a consensus price.
pub const MIN_ORACLE_SOURCES: u32 = 2;
/// Upper bound on registered sources; bounds aggregation cost.
pub const MAX_ORACLE_SOURCES: u32 = 20;
/// A source further than this from the median is treated as an outlier (20%).
pub const MAX_SOURCE_DEVIATION_BPS: i128 = 2_000;
/// Circuit breaker: max move of a consensus price vs. the last fresh price (50%).
pub const MAX_PRICE_JUMP_BPS: i128 = 5_000;
/// Sanity ceiling for any single price.
pub const MAX_PRICE: i128 = 1_000_000_000_000_000_000;
pub const BPS_DENOMINATOR: i128 = 10_000;
/// Number of published prices kept in the history ring.
pub const PRICE_HISTORY_LEN: u32 = 24;

#[derive(Clone)]
#[contracttype]
pub enum OracleKey {
    OracleAdmin,
    ResourcePrice(Symbol),        // resource -> PriceData
    PriceHistory(Symbol),         // resource -> Vec<PriceData> (last 24 publications)
    OracleSources,                // -> Vec<Address>
    EventTriggers,                // -> Vec<EventTrigger>
    PriceFeed(Symbol),            // reserved (kept for storage-key ABI stability)
    SourcePrice(Symbol, Address), // latest observation for a resource/source
    TriggerCounter,               // -> u64
}

#[contracterror]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum OracleError {
    Unauthorized = 1,
    StalePrice = 2,
    InvalidPrice = 3,
    ResourceNotFound = 4,
    TooManyUpdates = 5,
    NoOracleSources = 6,
    EventTriggerNotFound = 7,
    InvalidTriggerPrice = 8,
    /// The caller is not a registered oracle source.
    SourceNotRegistered = 9,
    /// Fewer fresh observations than the quorum.
    InsufficientSources = 10,
    /// Fresh observations disagree too much to reach consensus.
    SourceDeviationTooHigh = 11,
    /// Trigger type is not `above`, `below` or `equal`.
    InvalidTriggerType = 12,
    /// The oracle has already been initialized.
    AlreadyInitialized = 13,
    /// The source is already registered.
    DuplicateSource = 14,
    /// Registering the source would exceed `MAX_ORACLE_SOURCES`.
    TooManySources = 15,
    /// Consensus moved further than `MAX_PRICE_JUMP_BPS` from the last price.
    PriceJumpTooLarge = 16,
}

impl crate::error_standard::StandardContractError for OracleError {
    fn descriptor(self) -> crate::error_standard::ErrorDescriptor {
        use crate::error_standard::ErrorKind;
        let (kind, retryable) = match self {
            Self::Unauthorized | Self::SourceNotRegistered => (ErrorKind::Authorization, false),
            Self::StalePrice => (ErrorKind::Conflict, false),
            Self::AlreadyInitialized | Self::DuplicateSource => (ErrorKind::Conflict, false),
            Self::InvalidPrice | Self::InvalidTriggerPrice | Self::InvalidTriggerType => {
                (ErrorKind::Validation, false)
            }
            // More sources may report, or outliers may correct themselves.
            Self::InsufficientSources | Self::SourceDeviationTooHigh => (ErrorKind::Conflict, true),
            Self::PriceJumpTooLarge => (ErrorKind::Validation, false),
            Self::ResourceNotFound | Self::NoOracleSources | Self::EventTriggerNotFound => {
                (ErrorKind::NotFound, false)
            }
            Self::TooManyUpdates | Self::TooManySources => (ErrorKind::ResourceLimit, false),
        };
        crate::error_standard::ErrorDescriptor {
            module: "market_oracle",
            code: self as u32,
            kind,
            retryable,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct PriceData {
    pub resource: Symbol,
    pub price: i128,
    pub timestamp: u64,
    pub source_count: u32,
}

/// A price submitted by one registered oracle source.
#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct SourcePrice {
    pub resource: Symbol,
    pub source: Address,
    pub price: i128,
    pub timestamp: u64,
}

/// Event trigger configuration for real-world data events.
#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct EventTrigger {
    pub id: u64,
    pub resource: Symbol,
    pub trigger_price: i128,
    pub trigger_type: Symbol, // "above", "below", "equal"
    pub active: bool,
    pub created_at: u64,
}

// ─── Internal helpers ─────────────────────────────────────────────────────

/// Authenticate `admin` and check it is the stored oracle admin.
fn require_admin(env: &Env, admin: &Address) -> Result<(), OracleError> {
    admin.require_auth();
    let stored_admin: Address = env
        .storage()
        .instance()
        .get(&OracleKey::OracleAdmin)
        .ok_or(OracleError::Unauthorized)?;
    if *admin != stored_admin {
        return Err(OracleError::Unauthorized);
    }
    Ok(())
}

/// Reject prices that are non-positive or absurdly large.
pub fn validate_price(price: i128) -> Result<(), OracleError> {
    if price <= 0 || price > MAX_PRICE {
        return Err(OracleError::InvalidPrice);
    }
    Ok(())
}

fn get_sources(env: &Env) -> Result<Vec<Address>, OracleError> {
    env.storage()
        .persistent()
        .get(&OracleKey::OracleSources)
        .ok_or(OracleError::NoOracleSources)
}

fn is_registered(sources: &Vec<Address>, source: &Address) -> bool {
    sources.iter().any(|candidate| candidate == *source)
}

/// `|a - b| * 10_000 / reference`, saturating instead of overflowing.
fn deviation_bps(value: i128, reference: i128) -> i128 {
    if reference <= 0 {
        return i128::MAX;
    }
    (value - reference).abs().saturating_mul(BPS_DENOMINATOR) / reference
}

/// Median of a sorted, non-empty slice (lower-biased average for even counts).
fn median_of_sorted(prices: &[i128]) -> i128 {
    let n = prices.len();
    if n % 2 == 1 {
        prices[n / 2]
    } else {
        // Average without overflow: a + (b - a) / 2, both operands positive.
        let a = prices[n / 2 - 1];
        let b = prices[n / 2];
        a + (b - a) / 2
    }
}

/// Store a published price and append it to the bounded history.
fn publish_price(env: &Env, resource: Symbol, price: i128, source_count: u32) -> PriceData {
    let price_data = PriceData {
        resource: resource.clone(),
        price,
        timestamp: env.ledger().timestamp(),
        source_count,
    };

    env.storage()
        .persistent()
        .set(&OracleKey::ResourcePrice(resource.clone()), &price_data);

    let mut history = env
        .storage()
        .persistent()
        .get::<OracleKey, Vec<PriceData>>(&OracleKey::PriceHistory(resource.clone()))
        .unwrap_or(Vec::new(env));
    history.push_back(price_data.clone());
    while history.len() > PRICE_HISTORY_LEN {
        history.pop_front();
    }
    env.storage()
        .persistent()
        .set(&OracleKey::PriceHistory(resource.clone()), &history);

    env.events().publish(
        (symbol_short!("oracle"), symbol_short!("price")),
        (resource, price, price_data.timestamp, source_count),
    );

    price_data
}

// ─── Setup and source management ──────────────────────────────────────────

/// Initialize the market oracle with admin and default sources (one-shot).
pub fn initialize_oracle(
    env: &Env,
    admin: Address,
    sources: Vec<Address>,
) -> Result<(), OracleError> {
    admin.require_auth();

    if env.storage().instance().has(&OracleKey::OracleAdmin) {
        return Err(OracleError::AlreadyInitialized);
    }
    if sources.is_empty() {
        return Err(OracleError::NoOracleSources);
    }
    if sources.len() > MAX_ORACLE_SOURCES {
        return Err(OracleError::TooManySources);
    }
    // A source listed twice would get two votes in consensus.
    let mut unique: Vec<Address> = Vec::new(env);
    for source in sources.iter() {
        if is_registered(&unique, &source) {
            return Err(OracleError::DuplicateSource);
        }
        unique.push_back(source);
    }

    env.storage()
        .instance()
        .set(&OracleKey::OracleAdmin, &admin);
    env.storage()
        .persistent()
        .set(&OracleKey::OracleSources, &unique);

    env.events().publish(
        (symbol_short!("oracle"), symbol_short!("init")),
        (admin, unique.len()),
    );

    Ok(())
}

/// Add oracle source (admin only).
pub fn add_oracle_source(
    env: &Env,
    admin: Address,
    new_source: Address,
) -> Result<(), OracleError> {
    require_admin(env, &admin)?;

    let mut sources = env
        .storage()
        .persistent()
        .get::<OracleKey, Vec<Address>>(&OracleKey::OracleSources)
        .unwrap_or(Vec::new(env));

    if is_registered(&sources, &new_source) {
        return Err(OracleError::DuplicateSource);
    }
    if sources.len() >= MAX_ORACLE_SOURCES {
        return Err(OracleError::TooManySources);
    }

    sources.push_back(new_source.clone());
    env.storage()
        .persistent()
        .set(&OracleKey::OracleSources, &sources);

    env.events().publish(
        (symbol_short!("oracle"), symbol_short!("source")),
        (new_source,),
    );

    Ok(())
}

/// Remove a (e.g. compromised) oracle source (admin only). Its stored
/// observations stop counting immediately because aggregation only reads
/// registered sources.
pub fn remove_oracle_source(env: &Env, admin: Address, source: Address) -> Result<(), OracleError> {
    require_admin(env, &admin)?;

    let sources = get_sources(env)?;
    let mut kept: Vec<Address> = Vec::new(env);
    let mut found = false;
    for candidate in sources.iter() {
        if candidate == source {
            found = true;
        } else {
            kept.push_back(candidate);
        }
    }
    if !found {
        return Err(OracleError::SourceNotRegistered);
    }

    env.storage()
        .persistent()
        .set(&OracleKey::OracleSources, &kept);
    env.events()
        .publish((symbol_short!("oracle"), symbol_short!("srcrm")), (source,));
    Ok(())
}

/// List the registered oracle sources.
pub fn get_oracle_sources(env: &Env) -> Vec<Address> {
    env.storage()
        .persistent()
        .get(&OracleKey::OracleSources)
        .unwrap_or(Vec::new(env))
}

// ─── Source feeds and consensus ───────────────────────────────────────────

/// Submit a signed source observation. Only registered sources may submit;
/// each source has exactly one (its latest) observation per resource, so a
/// source cannot stack votes by submitting repeatedly.
pub fn submit_source_price(
    env: &Env,
    source: Address,
    resource: Symbol,
    price: i128,
) -> Result<SourcePrice, OracleError> {
    source.require_auth();
    let sources = get_sources(env)?;
    if !is_registered(&sources, &source) {
        return Err(OracleError::SourceNotRegistered);
    }
    validate_price(price)?;

    let observation = SourcePrice {
        resource: resource.clone(),
        source: source.clone(),
        price,
        timestamp: env.ledger().timestamp(),
    };
    env.storage().persistent().set(
        &OracleKey::SourcePrice(resource.clone(), source.clone()),
        &observation,
    );
    env.events().publish(
        (symbol_short!("oracle"), symbol_short!("submit")),
        (resource, source, price),
    );
    Ok(observation)
}

/// Read the latest observation a source submitted for a resource.
pub fn get_source_price(env: &Env, resource: Symbol, source: Address) -> Option<SourcePrice> {
    env.storage()
        .persistent()
        .get(&OracleKey::SourcePrice(resource, source))
}

/// Pure consensus over a set of fresh observations.
///
/// Returns `(price, agreeing_sources)`. `prices` is sorted in place.
pub fn compute_consensus(prices: &mut [i128]) -> Result<(i128, u32), OracleError> {
    let count = prices.len();
    if count < MIN_ORACLE_SOURCES as usize {
        return Err(OracleError::InsufficientSources);
    }
    prices.sort_unstable();
    let median = median_of_sorted(prices);

    // Keep only sources within the allowed band around the median. `prices`
    // is sorted, so the agreeing sources form one contiguous window.
    let mut first = count;
    let mut last = 0usize;
    for (i, price) in prices.iter().enumerate() {
        if deviation_bps(*price, median) <= MAX_SOURCE_DEVIATION_BPS {
            if first == count {
                first = i;
            }
            last = i;
        }
    }
    let accepted = if first == count { 0 } else { last - first + 1 };

    // Quorum and strict majority: a minority of colluding sources can neither
    // publish a price on their own nor drag the median of the honest majority.
    if (accepted as u32) < MIN_ORACLE_SOURCES || accepted * 2 <= count {
        return Err(OracleError::SourceDeviationTooHigh);
    }

    Ok((median_of_sorted(&prices[first..=last]), accepted as u32))
}

/// Aggregate fresh source observations into a consensus price and publish it.
///
/// Permissionless: the result depends only on signed source observations, so
/// any keeper may trigger aggregation.
pub fn aggregate_resource_price(env: &Env, resource: Symbol) -> Result<PriceData, OracleError> {
    let sources = get_sources(env)?;
    let now = env.ledger().timestamp();

    let mut prices: [i128; MAX_ORACLE_SOURCES as usize] = [0; MAX_ORACLE_SOURCES as usize];
    let mut count = 0usize;
    for source in sources.iter() {
        if count == prices.len() {
            break;
        }
        if let Some(observation) = env
            .storage()
            .persistent()
            .get::<OracleKey, SourcePrice>(&OracleKey::SourcePrice(resource.clone(), source))
        {
            if now.saturating_sub(observation.timestamp) <= MAX_OBSERVATION_AGE_SECS {
                prices[count] = observation.price;
                count += 1;
            }
        }
    }

    let (price, accepted) = compute_consensus(&mut prices[..count])?;

    // Circuit breaker against a fresh previous price.
    if let Some(previous) = env
        .storage()
        .persistent()
        .get::<OracleKey, PriceData>(&OracleKey::ResourcePrice(resource.clone()))
    {
        let fresh = now.saturating_sub(previous.timestamp) <= MAX_PRICE_AGE_SECS;
        if fresh && deviation_bps(price, previous.price) > MAX_PRICE_JUMP_BPS {
            env.events().publish(
                (symbol_short!("oracle"), symbol_short!("breaker")),
                (resource, previous.price, price),
            );
            return Err(OracleError::PriceJumpTooLarge);
        }
    }

    env.events().publish(
        (symbol_short!("oracle"), symbol_short!("consensus")),
        (resource.clone(), price, accepted, count as u32),
    );
    Ok(publish_price(env, resource, price, accepted))
}

// ─── Admin override ───────────────────────────────────────────────────────

/// Publish a resource price directly (admin override; still validated).
pub fn update_resource_price(
    env: &Env,
    admin: Address,
    resource: Symbol,
    new_price: i128,
) -> Result<PriceData, OracleError> {
    require_admin(env, &admin)?;
    validate_price(new_price)?;
    Ok(publish_price(env, resource, new_price, 1))
}

/// Batch update multiple resource prices (admin override).
///
/// Validates every price before writing any, and authenticates the admin
/// once: calling `update_resource_price` per item would re-run
/// `require_auth` for the same address in one frame, which the host rejects.
pub fn batch_update_prices(
    env: &Env,
    admin: Address,
    resources: Vec<Symbol>,
    prices: Vec<i128>,
) -> Result<Vec<PriceData>, OracleError> {
    require_admin(env, &admin)?;

    if resources.len() != prices.len() {
        return Err(OracleError::InvalidPrice);
    }
    if resources.len() > MAX_BATCH_UPDATE {
        return Err(OracleError::TooManyUpdates);
    }
    for price in prices.iter() {
        validate_price(price)?;
    }

    let mut results = Vec::new(env);
    for i in 0..resources.len() {
        let resource = resources.get(i).unwrap();
        let price = prices.get(i).unwrap();
        results.push_back(publish_price(env, resource, price, 1));
    }

    Ok(results)
}

// ─── Reads ────────────────────────────────────────────────────────────────

/// Get current market rate for a resource (pure view). Stale prices fail.
pub fn get_current_market_rate(env: &Env, resource: Symbol) -> Result<i128, OracleError> {
    let price_data = env
        .storage()
        .persistent()
        .get::<OracleKey, PriceData>(&OracleKey::ResourcePrice(resource.clone()))
        .ok_or(OracleError::ResourceNotFound)?;

    let current_time = env.ledger().timestamp();
    let age = current_time.saturating_sub(price_data.timestamp);

    if age > MAX_PRICE_AGE_SECS {
        return Err(OracleError::StalePrice);
    }

    Ok(price_data.price)
}

/// Get price data with metadata
pub fn get_price_data(env: &Env, resource: Symbol) -> Result<PriceData, OracleError> {
    env.storage()
        .persistent()
        .get::<OracleKey, PriceData>(&OracleKey::ResourcePrice(resource))
        .ok_or(OracleError::ResourceNotFound)
}

/// Get the recent price history for a resource
pub fn get_price_history(env: &Env, resource: Symbol) -> Vec<PriceData> {
    env.storage()
        .persistent()
        .get::<OracleKey, Vec<PriceData>>(&OracleKey::PriceHistory(resource))
        .unwrap_or(Vec::new(env))
}

// ─── Event Triggers for Real-World Data Events ────────────────────────────

/// Register a price trigger that fires an event when price threshold is reached.
pub fn register_price_trigger(
    env: &Env,
    admin: Address,
    resource: Symbol,
    trigger_price: i128,
    trigger_type: Symbol,
) -> Result<u64, OracleError> {
    require_admin(env, &admin)?;

    if trigger_price < 0 {
        return Err(OracleError::InvalidTriggerPrice);
    }
    if trigger_type != symbol_short!("above")
        && trigger_type != symbol_short!("below")
        && trigger_type != symbol_short!("equal")
    {
        return Err(OracleError::InvalidTriggerType);
    }

    let mut triggers: Vec<EventTrigger> = env
        .storage()
        .persistent()
        .get(&OracleKey::EventTriggers)
        .unwrap_or(Vec::new(env));

    // A counter, not the ledger sequence: two triggers in one ledger must not
    // share an ID.
    let trigger_id: u64 = env
        .storage()
        .persistent()
        .get::<OracleKey, u64>(&OracleKey::TriggerCounter)
        .unwrap_or(0)
        .saturating_add(1);
    env.storage()
        .persistent()
        .set(&OracleKey::TriggerCounter, &trigger_id);

    let trigger = EventTrigger {
        id: trigger_id,
        resource: resource.clone(),
        trigger_price,
        trigger_type,
        active: true,
        created_at: env.ledger().timestamp(),
    };

    triggers.push_back(trigger);
    env.storage()
        .persistent()
        .set(&OracleKey::EventTriggers, &triggers);

    env.events().publish(
        (symbol_short!("oracle"), symbol_short!("trgger")),
        (trigger_id, resource, trigger_price),
    );

    Ok(trigger_id)
}

/// Check and fire events based on registered triggers.
pub fn check_and_fire_triggers(env: &Env, resource: Symbol) -> Result<Vec<u64>, OracleError> {
    let current_price = get_current_market_rate(env, resource.clone())?;

    let triggers: Vec<EventTrigger> = env
        .storage()
        .persistent()
        .get(&OracleKey::EventTriggers)
        .unwrap_or(Vec::new(env));

    let mut fired_triggers: Vec<u64> = Vec::new(env);

    for trigger in triggers.iter() {
        if trigger.resource != resource || !trigger.active {
            continue;
        }

        let should_fire = if trigger.trigger_type == symbol_short!("above") {
            current_price >= trigger.trigger_price
        } else if trigger.trigger_type == symbol_short!("below") {
            current_price <= trigger.trigger_price
        } else if trigger.trigger_type == symbol_short!("equal") {
            current_price == trigger.trigger_price
        } else {
            false
        };

        if should_fire {
            fired_triggers.push_back(trigger.id);
            env.events().publish(
                (symbol_short!("oracle"), symbol_short!("fired")),
                (trigger.id, resource.clone(), current_price),
            );
        }
    }

    Ok(fired_triggers)
}

/// Deactivate a trigger.
pub fn disable_trigger(env: &Env, admin: Address, trigger_id: u64) -> Result<(), OracleError> {
    require_admin(env, &admin)?;

    let mut triggers: Vec<EventTrigger> = env
        .storage()
        .persistent()
        .get(&OracleKey::EventTriggers)
        .ok_or(OracleError::EventTriggerNotFound)?;

    for i in 0..triggers.len() {
        let mut trigger = triggers.get(i).unwrap();
        if trigger.id == trigger_id {
            trigger.active = false;
            triggers.set(i, trigger);
            env.storage()
                .persistent()
                .set(&OracleKey::EventTriggers, &triggers);

            env.events().publish(
                (symbol_short!("oracle"), symbol_short!("disable")),
                trigger_id,
            );

            return Ok(());
        }
    }

    Err(OracleError::EventTriggerNotFound)
}

/// Get all active triggers for a resource.
pub fn get_active_triggers(env: &Env, resource: Symbol) -> Vec<EventTrigger> {
    let triggers: Vec<EventTrigger> = env
        .storage()
        .persistent()
        .get(&OracleKey::EventTriggers)
        .unwrap_or(Vec::new(env));

    let mut active: Vec<EventTrigger> = Vec::new(env);
    for trigger in triggers.iter() {
        if trigger.resource == resource && trigger.active {
            active.push_back(trigger);
        }
    }

    active
}

// ─── Tests (Issue #654) ───────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use soroban_sdk::testutils::{Address as _, Ledger};
    use soroban_sdk::{contract, contractimpl};

    #[contract]
    struct Stub;
    #[contractimpl]
    impl Stub {}

    struct Setup {
        env: Env,
        contract: Address,
        admin: Address,
        sources: [Address; 5],
    }

    impl Setup {
        /// Run `f` in its own contract frame. Each authenticated call needs a
        /// fresh frame: the host rejects a second `require_auth()` for the same
        /// address within one frame.
        fn call<T>(&self, f: impl FnOnce() -> T) -> T {
            self.env.as_contract(&self.contract, f)
        }
    }

    fn setup() -> Setup {
        let env = Env::default();
        env.mock_all_auths();
        env.ledger().set_timestamp(1_000_000);
        let contract = env.register(Stub, ());
        let admin = Address::generate(&env);
        let sources = [
            Address::generate(&env),
            Address::generate(&env),
            Address::generate(&env),
            Address::generate(&env),
            Address::generate(&env),
        ];
        let mut list = Vec::new(&env);
        for s in sources.iter() {
            list.push_back(s.clone());
        }
        env.as_contract(&contract, || {
            initialize_oracle(&env, admin.clone(), list).unwrap();
        });
        Setup {
            env,
            contract,
            admin,
            sources,
        }
    }

    fn submit_all(s: &Setup, resource: &Symbol, prices: &[i128]) {
        for (source, price) in s.sources.iter().zip(prices.iter()) {
            s.call(|| {
                submit_source_price(&s.env, source.clone(), resource.clone(), *price).unwrap();
            });
        }
    }

    fn aggregate(s: &Setup, resource: &Symbol) -> Result<PriceData, OracleError> {
        s.call(|| aggregate_resource_price(&s.env, resource.clone()))
    }

    fn admin_set(s: &Setup, resource: &Symbol, price: i128) -> Result<PriceData, OracleError> {
        s.call(|| update_resource_price(&s.env, s.admin.clone(), resource.clone(), price))
    }

    // ── Pure consensus ──

    #[test]
    fn consensus_is_median_of_agreeing_sources() {
        let mut prices = [100, 104, 98, 101, 99];
        assert_eq!(compute_consensus(&mut prices), Ok((100, 5)));
    }

    #[test]
    fn consensus_even_count_averages_middle_pair() {
        let mut prices = [100, 102];
        assert_eq!(compute_consensus(&mut prices), Ok((101, 2)));
    }

    #[test]
    fn consensus_needs_quorum() {
        assert_eq!(
            compute_consensus(&mut [100]),
            Err(OracleError::InsufficientSources)
        );
        assert_eq!(
            compute_consensus(&mut []),
            Err(OracleError::InsufficientSources)
        );
    }

    #[test]
    fn consensus_fails_when_sources_disagree() {
        // Two sources, 3x apart: neither is within 20% of the median.
        assert_eq!(
            compute_consensus(&mut [100, 300]),
            Err(OracleError::SourceDeviationTooHigh)
        );
    }

    #[test]
    fn consensus_requires_strict_majority() {
        // 2 of 4 agree around 100, 2 of 4 around 1000: no majority.
        assert_eq!(
            compute_consensus(&mut [100, 101, 1_000, 1_001]),
            Err(OracleError::SourceDeviationTooHigh)
        );
    }

    #[test]
    fn manipulated_minority_cannot_move_price() {
        // Two of five sources report a 100x price; the honest median wins.
        let mut prices = [100, 101, 99, 10_000, 10_000];
        let (price, accepted) = compute_consensus(&mut prices).unwrap();
        assert_eq!(price, 100);
        assert_eq!(accepted, 3);
    }

    #[test]
    fn extreme_values_do_not_overflow() {
        let mut prices = [MAX_PRICE, MAX_PRICE, MAX_PRICE - 1];
        assert_eq!(compute_consensus(&mut prices), Ok((MAX_PRICE, 3)));
    }

    // ── Validation ──

    #[test]
    fn validation_rejects_bad_prices() {
        assert_eq!(validate_price(0), Err(OracleError::InvalidPrice));
        assert_eq!(validate_price(-5), Err(OracleError::InvalidPrice));
        assert_eq!(
            validate_price(MAX_PRICE + 1),
            Err(OracleError::InvalidPrice)
        );
        assert_eq!(validate_price(1), Ok(()));
    }

    #[test]
    fn source_cannot_submit_invalid_price() {
        let s = setup();
        assert_eq!(
            s.call(|| submit_source_price(&s.env, s.sources[0].clone(), symbol_short!("ore"), 0)),
            Err(OracleError::InvalidPrice)
        );
    }

    // ── Feeds end to end ──

    #[test]
    fn feeds_aggregate_into_published_price_and_history() {
        let s = setup();
        let r = symbol_short!("ore");
        submit_all(&s, &r, &[100, 102, 98, 101, 10_000]);

        let data = aggregate(&s, &r).unwrap();
        assert_eq!(data.price, 100);
        assert_eq!(data.source_count, 4);

        s.call(|| {
            assert_eq!(get_current_market_rate(&s.env, r.clone()), Ok(100));
            assert_eq!(get_price_history(&s.env, r.clone()).len(), 1);
        });
    }

    #[test]
    fn unregistered_source_cannot_submit() {
        let s = setup();
        let outsider = Address::generate(&s.env);
        assert_eq!(
            s.call(|| submit_source_price(&s.env, outsider, symbol_short!("ore"), 1)),
            Err(OracleError::SourceNotRegistered)
        );
    }

    #[test]
    fn resubmitting_replaces_rather_than_stacking_votes() {
        let s = setup();
        let r = symbol_short!("ore");
        // One source spams manipulated prices; another reports honestly.
        for _ in 0..5 {
            s.call(|| {
                submit_source_price(&s.env, s.sources[0].clone(), r.clone(), 500).unwrap();
            });
        }
        s.call(|| {
            submit_source_price(&s.env, s.sources[1].clone(), r.clone(), 100).unwrap();
        });
        // Only two observations exist and they disagree: no consensus.
        assert_eq!(aggregate(&s, &r), Err(OracleError::SourceDeviationTooHigh));
    }

    #[test]
    fn stale_observations_are_ignored() {
        let s = setup();
        let r = symbol_short!("ore");
        submit_all(&s, &r, &[100, 100, 100]);
        s.env
            .ledger()
            .set_timestamp(1_000_000 + MAX_OBSERVATION_AGE_SECS + 1);
        assert_eq!(aggregate(&s, &r), Err(OracleError::InsufficientSources));
    }

    #[test]
    fn circuit_breaker_blocks_sudden_jump() {
        let s = setup();
        let r = symbol_short!("ore");
        submit_all(&s, &r, &[100, 100, 100, 100, 100]);
        aggregate(&s, &r).unwrap();

        // All sources collude on a 3x price within the freshness window.
        s.env.ledger().set_timestamp(1_000_060);
        submit_all(&s, &r, &[300, 300, 300, 300, 300]);
        assert_eq!(aggregate(&s, &r), Err(OracleError::PriceJumpTooLarge));
        s.call(|| {
            assert_eq!(get_current_market_rate(&s.env, r.clone()), Ok(100));
        });

        // A gradual move is accepted.
        submit_all(&s, &r, &[130, 130, 130, 130, 130]);
        assert_eq!(aggregate(&s, &r).unwrap().price, 130);
    }

    #[test]
    fn removed_source_no_longer_counts() {
        let s = setup();
        let r = symbol_short!("ore");
        submit_all(&s, &r, &[100, 100]);
        s.call(|| {
            remove_oracle_source(&s.env, s.admin.clone(), s.sources[1].clone()).unwrap();
        });
        assert_eq!(aggregate(&s, &r), Err(OracleError::InsufficientSources));
    }

    #[test]
    fn stale_published_price_is_not_served() {
        let s = setup();
        let r = symbol_short!("ore");
        submit_all(&s, &r, &[100, 100]);
        aggregate(&s, &r).unwrap();
        s.env
            .ledger()
            .set_timestamp(1_000_000 + MAX_PRICE_AGE_SECS + 1);
        s.call(|| {
            assert_eq!(
                get_current_market_rate(&s.env, r.clone()),
                Err(OracleError::StalePrice)
            );
        });
    }

    // ── Admin and setup ──

    #[test]
    fn initialize_is_one_shot() {
        let s = setup();
        let attacker = Address::generate(&s.env);
        let mut list = Vec::new(&s.env);
        list.push_back(attacker.clone());
        assert_eq!(
            s.call(|| initialize_oracle(&s.env, attacker, list)),
            Err(OracleError::AlreadyInitialized)
        );
    }

    #[test]
    fn duplicate_sources_are_rejected() {
        let s = setup();
        assert_eq!(
            s.call(|| add_oracle_source(&s.env, s.admin.clone(), s.sources[0].clone())),
            Err(OracleError::DuplicateSource)
        );

        let env = Env::default();
        env.mock_all_auths();
        let contract = env.register(Stub, ());
        let admin = Address::generate(&env);
        let dup = Address::generate(&env);
        let mut list = Vec::new(&env);
        list.push_back(dup.clone());
        list.push_back(dup);
        env.as_contract(&contract, || {
            assert_eq!(
                initialize_oracle(&env, admin, list),
                Err(OracleError::DuplicateSource)
            );
        });
    }

    #[test]
    fn only_admin_can_override() {
        let s = setup();
        let other = Address::generate(&s.env);
        assert_eq!(
            s.call(|| update_resource_price(&s.env, other, symbol_short!("ore"), 5)),
            Err(OracleError::Unauthorized)
        );
        assert_eq!(
            admin_set(&s, &symbol_short!("ore"), 5).map(|d| d.price),
            Ok(5)
        );
        assert_eq!(
            admin_set(&s, &symbol_short!("ore"), 0),
            Err(OracleError::InvalidPrice)
        );
    }

    #[test]
    fn batch_update_validates_before_writing() {
        let s = setup();
        let mut resources = Vec::new(&s.env);
        resources.push_back(symbol_short!("ore"));
        resources.push_back(symbol_short!("gas"));
        let mut bad = Vec::new(&s.env);
        bad.push_back(10);
        bad.push_back(-1);
        assert_eq!(
            s.call(|| batch_update_prices(&s.env, s.admin.clone(), resources.clone(), bad)),
            Err(OracleError::InvalidPrice)
        );
        s.call(|| {
            assert_eq!(
                get_price_data(&s.env, symbol_short!("ore")),
                Err(OracleError::ResourceNotFound)
            );
        });

        let mut good = Vec::new(&s.env);
        good.push_back(10);
        good.push_back(20);
        let out = s
            .call(|| batch_update_prices(&s.env, s.admin.clone(), resources, good))
            .unwrap();
        assert_eq!(out.len(), 2);
    }

    #[test]
    fn history_is_bounded() {
        let s = setup();
        let r = symbol_short!("ore");
        for i in 0..(PRICE_HISTORY_LEN + 5) {
            admin_set(&s, &r, 100 + i as i128).unwrap();
        }
        s.call(|| {
            assert_eq!(
                get_price_history(&s.env, r.clone()).len(),
                PRICE_HISTORY_LEN
            );
        });
    }

    #[test]
    fn triggers_register_fire_and_disable() {
        let s = setup();
        let r = symbol_short!("ore");
        admin_set(&s, &r, 150).unwrap();
        let register = |price: i128, kind: Symbol| {
            s.call(|| register_price_trigger(&s.env, s.admin.clone(), r.clone(), price, kind))
        };
        let a = register(100, symbol_short!("above")).unwrap();
        let b = register(100, symbol_short!("below")).unwrap();
        assert_ne!(a, b);
        assert_eq!(
            register(1, symbol_short!("sideways")),
            Err(OracleError::InvalidTriggerType)
        );

        let fired = s
            .call(|| check_and_fire_triggers(&s.env, r.clone()))
            .unwrap();
        assert_eq!(fired.len(), 1);
        assert_eq!(fired.get(0), Some(a));

        s.call(|| disable_trigger(&s.env, s.admin.clone(), a))
            .unwrap();
        s.call(|| {
            assert_eq!(check_and_fire_triggers(&s.env, r.clone()).unwrap().len(), 0);
            assert_eq!(get_active_triggers(&s.env, r.clone()).len(), 1);
        });
    }
}
