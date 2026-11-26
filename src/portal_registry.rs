use soroban_sdk::{contracterror, contracttype, symbol_short, Address, Env, String, Symbol, Vec};

/// Stability percentage below which travel is refused.
pub const MIN_STABLE_PCT: u32 = 10;

/// Maximum portals allowed per single register_portal_batch call.
pub const MAX_PORTALS_PER_TX: u32 = 8;

/// Portal stability decays by this many percentage points per day.
pub const DECAY_RATE_PER_DAY: u32 = 2;

/// Seconds in one day.
const SECONDS_PER_DAY: u64 = 86_400;

/// Base travel cost in resource units.
pub const BASE_TRAVEL_COST: i128 = 100;

/// Maximum links returned by one `get_portal_topology` page.
pub const MAX_TOPOLOGY_PAGE: u32 = 50;
/// Metadata limits.
pub const MAX_PORTAL_NAME_LEN: u32 = 32;
pub const MAX_PORTAL_DESCRIPTION_LEN: u32 = 256;
pub const MAX_PORTAL_TAGS: u32 = 8;

// ─── Storage Keys ──────────────────────────────────────────────────────────────

#[derive(Clone)]
#[contracttype]
pub enum PortalKey {
    /// Auto-incrementing portal ID counter.
    PortalCounter,
    /// Portal data keyed by portal ID.
    Portal(u64),
    /// Authorized registrar address.
    PortalAdmin,
    /// Outgoing portal IDs per source nebula (discovery index).
    PortalsFromNebula(u64),
    /// Present when a portal is closed to travel.
    PortalClosed(u64),
    /// Access mode per portal (absent = Public).
    PortalAccess(u64),
    /// Allowlist entry for an `Allowlist` portal.
    PortalAllowed(u64, Address),
    /// Optional descriptive metadata.
    PortalMeta(u64),
}

// ─── Errors ─────────────────────────────────────────────────────────────────

#[contracterror]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum PortalError {
    /// Portal stability is below threshold — travel refused.
    PortalUnstable = 1,
    /// Caller is not the portal owner or admin.
    NotAuthorized = 2,
    /// Portal does not exist.
    PortalNotFound = 3,
    /// Batch size exceeded MAX_PORTALS_PER_TX.
    BatchTooLarge = 4,
    /// Source and target nebula IDs must differ.
    SameNebula = 5,
    /// Portal has been closed by its owner or the admin.
    PortalClosed = 6,
    /// Metadata exceeds the length or tag limits.
    InvalidMetadata = 7,
}

impl crate::error_standard::StandardContractError for PortalError {
    fn descriptor(self) -> crate::error_standard::ErrorDescriptor {
        use crate::error_standard::ErrorKind;
        let (kind, retryable) = match self {
            Self::PortalUnstable | Self::PortalClosed => (ErrorKind::Conflict, false),
            Self::NotAuthorized => (ErrorKind::Authorization, false),
            Self::PortalNotFound => (ErrorKind::NotFound, false),
            Self::BatchTooLarge => (ErrorKind::ResourceLimit, false),
            Self::SameNebula | Self::InvalidMetadata => (ErrorKind::Validation, false),
        };
        crate::error_standard::ErrorDescriptor {
            module: "portal_registry",
            code: self as u32,
            kind,
            retryable,
        }
    }
}

// ─── Data Types ─────────────────────────────────────────────────────────────

#[derive(Clone, Debug, PartialEq)]
#[contracttype]
pub struct Portal {
    pub id: u64,
    pub owner: Address,
    pub source_nebula: u64,
    pub target_nebula: u64,
    /// Stability at creation time (always 100).
    pub initial_stability: u32,
    pub created_at: u64,
    /// Timestamp of the last stability refresh.
    pub last_refreshed: u64,
    pub level: u32,
}

/// Who may travel through a portal.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[contracttype]
pub enum PortalAccessMode {
    Public = 0,
    OwnerOnly = 1,
    Allowlist = 2,
}

#[derive(Clone, Debug, PartialEq)]
#[contracttype]
pub struct PortalMetadata {
    pub name: String,
    pub description: String,
    pub tags: Vec<Symbol>,
}

/// One edge of the portal network, with its live state.
#[derive(Clone, Debug, PartialEq)]
#[contracttype]
pub struct PortalLink {
    pub id: u64,
    pub source_nebula: u64,
    pub target_nebula: u64,
    pub stability: u32,
    pub travel_cost: i128,
    pub open: bool,
    pub access: PortalAccessMode,
}

// ─── Helpers ────────────────────────────────────────────────────────────────

/// Compute current stability accounting for decay since the portal was
/// last refreshed.
fn current_stability(portal: &Portal, now: u64) -> u32 {
    let elapsed_secs = now.saturating_sub(portal.last_refreshed);
    let elapsed_days = (elapsed_secs / SECONDS_PER_DAY) as u32;
    let decay = DECAY_RATE_PER_DAY.saturating_mul(elapsed_days);
    portal.initial_stability.saturating_sub(decay)
}

/// Travel cost scales inversely with stability: unstable portals cost more.
fn travel_cost(stability_pct: u32) -> i128 {
    if stability_pct == 0 {
        return i128::MAX;
    }
    // cost = BASE * (100 / stability)
    BASE_TRAVEL_COST * 100 / stability_pct as i128
}

// ─── Public API ─────────────────────────────────────────────────────────────

/// Initialize the portal registry with an admin address.
pub fn initialize_portal_registry(env: &Env, admin: &Address) {
    admin.require_auth();
    env.storage().instance().set(&PortalKey::PortalAdmin, admin);
    env.storage()
        .instance()
        .set(&PortalKey::PortalCounter, &0u64);
}

/// Register a new portal between two nebulae (owner-only).
///
/// Returns the newly assigned portal ID.
pub fn register_portal(
    env: &Env,
    owner: &Address,
    source_nebula: u64,
    target_nebula: u64,
) -> Result<u64, PortalError> {
    owner.require_auth();

    if source_nebula == target_nebula {
        return Err(PortalError::SameNebula);
    }

    let portal_id: u64 = env
        .storage()
        .instance()
        .get(&PortalKey::PortalCounter)
        .unwrap_or(0);

    let now = env.ledger().timestamp();
    let portal = Portal {
        id: portal_id,
        owner: owner.clone(),
        source_nebula,
        target_nebula,
        initial_stability: 100,
        created_at: now,
        last_refreshed: now,
        level: 1,
    };

    env.storage()
        .persistent()
        .set(&PortalKey::Portal(portal_id), &portal);
    env.storage()
        .instance()
        .set(&PortalKey::PortalCounter, &(portal_id + 1));

    let index_key = PortalKey::PortalsFromNebula(source_nebula);
    let mut outgoing: Vec<u64> = env
        .storage()
        .persistent()
        .get(&index_key)
        .unwrap_or_else(|| Vec::new(env));
    outgoing.push_back(portal_id);
    env.storage().persistent().set(&index_key, &outgoing);

    env.events().publish(
        (symbol_short!("portal"), symbol_short!("regist")),
        (owner.clone(), portal_id, source_nebula, target_nebula, now),
    );

    Ok(portal_id)
}

/// Register up to [`MAX_PORTALS_PER_TX`] portals in one transaction.
///
/// `connections` is a list of `(source_nebula, target_nebula)` pairs.
pub fn register_portal_batch(
    env: &Env,
    owner: &Address,
    connections: &Vec<(u64, u64)>,
) -> Result<Vec<u64>, PortalError> {
    owner.require_auth();

    if connections.len() > MAX_PORTALS_PER_TX {
        return Err(PortalError::BatchTooLarge);
    }

    let mut ids: Vec<u64> = Vec::new(env);
    for pair in connections.iter() {
        let id = register_portal(env, owner, pair.0, pair.1)?;
        ids.push_back(id);
    }

    Ok(ids)
}

/// Return the current stability percentage and travel cost for a portal.
pub fn query_portal_status(env: &Env, portal_id: u64) -> Result<(u32, i128), PortalError> {
    let portal: Portal = env
        .storage()
        .persistent()
        .get(&PortalKey::Portal(portal_id))
        .ok_or(PortalError::PortalNotFound)?;

    let now = env.ledger().timestamp();
    let stability = current_stability(&portal, now);
    let cost = travel_cost(stability);

    Ok((stability, cost))
}

/// Refresh (re-stabilize) a portal to 100 % stability.
///
/// Only the portal owner may call this.
pub fn refresh_portal(env: &Env, owner: &Address, portal_id: u64) -> Result<(), PortalError> {
    owner.require_auth();

    let mut portal: Portal = env
        .storage()
        .persistent()
        .get(&PortalKey::Portal(portal_id))
        .ok_or(PortalError::PortalNotFound)?;

    if portal.owner != *owner {
        return Err(PortalError::NotAuthorized);
    }

    portal.initial_stability = 100;
    portal.last_refreshed = env.ledger().timestamp();
    env.storage()
        .persistent()
        .set(&PortalKey::Portal(portal_id), &portal);

    Ok(())
}

/// Attempt anonymous travel through a portal. Errors if stability <
/// [`MIN_STABLE_PCT`], the portal is closed, or it is not `Public`
/// (restricted portals need [`travel_through_portal_as`]).
///
/// Returns travel cost that should be charged by the calling contract.
pub fn travel_through_portal(env: &Env, portal_id: u64) -> Result<i128, PortalError> {
    let portal = get_portal(env, portal_id)?;
    if get_portal_access(env, portal_id) != PortalAccessMode::Public {
        return Err(PortalError::NotAuthorized);
    }
    checked_travel_cost(env, &portal)
}

/// Authenticated travel: enforces the portal's access mode for `traveler`.
pub fn travel_through_portal_as(
    env: &Env,
    traveler: &Address,
    portal_id: u64,
) -> Result<i128, PortalError> {
    traveler.require_auth();
    let portal = get_portal(env, portal_id)?;
    if !can_travel(env, traveler, &portal) {
        return Err(PortalError::NotAuthorized);
    }
    let cost = checked_travel_cost(env, &portal)?;
    env.events().publish(
        (symbol_short!("portal"), symbol_short!("travel")),
        (traveler.clone(), portal_id, cost),
    );
    Ok(cost)
}

fn checked_travel_cost(env: &Env, portal: &Portal) -> Result<i128, PortalError> {
    if !is_portal_open(env, portal.id) {
        return Err(PortalError::PortalClosed);
    }
    let now = env.ledger().timestamp();
    let stability = current_stability(&portal, now);

    if stability < MIN_STABLE_PCT {
        return Err(PortalError::PortalUnstable);
    }

    Ok(travel_cost(stability))
}

/// Retrieve raw portal data.
pub fn get_portal(env: &Env, portal_id: u64) -> Result<Portal, PortalError> {
    env.storage()
        .persistent()
        .get(&PortalKey::Portal(portal_id))
        .ok_or(PortalError::PortalNotFound)
}

// ─── Discovery, connections, topology, access, metadata (#711) ─────────────

/// Number of portals ever registered (IDs are `0..count`).
pub fn get_portal_count(env: &Env) -> u64 {
    env.storage()
        .instance()
        .get(&PortalKey::PortalCounter)
        .unwrap_or(0)
}

/// IDs of portals leaving `nebula_id`, in registration order.
pub fn discover_portals(env: &Env, nebula_id: u64) -> Vec<u64> {
    env.storage()
        .persistent()
        .get(&PortalKey::PortalsFromNebula(nebula_id))
        .unwrap_or_else(|| Vec::new(env))
}

/// One page of the portal network starting at `start_id` (at most
/// [`MAX_TOPOLOGY_PAGE`] links).
pub fn get_portal_topology(env: &Env, start_id: u64, limit: u32) -> Vec<PortalLink> {
    let end =
        get_portal_count(env).min(start_id.saturating_add(u64::from(limit.min(MAX_TOPOLOGY_PAGE))));
    let now = env.ledger().timestamp();
    let mut links = Vec::new(env);
    for id in start_id..end {
        if let Ok(portal) = get_portal(env, id) {
            let stability = current_stability(&portal, now);
            links.push_back(PortalLink {
                id,
                source_nebula: portal.source_nebula,
                target_nebula: portal.target_nebula,
                stability,
                travel_cost: travel_cost(stability),
                open: is_portal_open(env, id),
                access: get_portal_access(env, id),
            });
        }
    }
    links
}

fn require_owner_or_admin(env: &Env, caller: &Address, portal: &Portal) -> Result<(), PortalError> {
    caller.require_auth();
    let admin: Option<Address> = env.storage().instance().get(&PortalKey::PortalAdmin);
    if portal.owner == *caller || admin.as_ref() == Some(caller) {
        Ok(())
    } else {
        Err(PortalError::NotAuthorized)
    }
}

pub fn is_portal_open(env: &Env, portal_id: u64) -> bool {
    !env.storage()
        .persistent()
        .has(&PortalKey::PortalClosed(portal_id))
}

/// Close (`open = false`) or reopen a portal. Owner or registry admin.
pub fn set_portal_open(
    env: &Env,
    caller: &Address,
    portal_id: u64,
    open: bool,
) -> Result<(), PortalError> {
    let portal = get_portal(env, portal_id)?;
    require_owner_or_admin(env, caller, &portal)?;
    let key = PortalKey::PortalClosed(portal_id);
    if open {
        env.storage().persistent().remove(&key);
    } else {
        env.storage().persistent().set(&key, &true);
    }
    env.events().publish(
        (symbol_short!("portal"), symbol_short!("state")),
        (portal_id, open),
    );
    Ok(())
}

pub fn get_portal_access(env: &Env, portal_id: u64) -> PortalAccessMode {
    env.storage()
        .persistent()
        .get(&PortalKey::PortalAccess(portal_id))
        .unwrap_or(PortalAccessMode::Public)
}

/// Set who may travel through a portal. Owner or registry admin.
pub fn set_portal_access(
    env: &Env,
    caller: &Address,
    portal_id: u64,
    mode: PortalAccessMode,
) -> Result<(), PortalError> {
    let portal = get_portal(env, portal_id)?;
    require_owner_or_admin(env, caller, &portal)?;
    env.storage()
        .persistent()
        .set(&PortalKey::PortalAccess(portal_id), &mode);
    Ok(())
}

/// Add or remove `traveler` from a portal's allowlist. Owner or registry admin.
pub fn set_portal_allowed(
    env: &Env,
    caller: &Address,
    portal_id: u64,
    traveler: &Address,
    allowed: bool,
) -> Result<(), PortalError> {
    let portal = get_portal(env, portal_id)?;
    require_owner_or_admin(env, caller, &portal)?;
    let key = PortalKey::PortalAllowed(portal_id, traveler.clone());
    if allowed {
        env.storage().persistent().set(&key, &true);
    } else {
        env.storage().persistent().remove(&key);
    }
    Ok(())
}

/// Whether `traveler` passes the portal's access mode (ignores stability).
pub fn can_travel(env: &Env, traveler: &Address, portal: &Portal) -> bool {
    match get_portal_access(env, portal.id) {
        PortalAccessMode::Public => true,
        PortalAccessMode::OwnerOnly => portal.owner == *traveler,
        PortalAccessMode::Allowlist => {
            portal.owner == *traveler
                || env
                    .storage()
                    .persistent()
                    .has(&PortalKey::PortalAllowed(portal.id, traveler.clone()))
        }
    }
}

/// Attach or replace a portal's metadata. Owner only.
pub fn set_portal_metadata(
    env: &Env,
    owner: &Address,
    portal_id: u64,
    metadata: PortalMetadata,
) -> Result<(), PortalError> {
    owner.require_auth();
    let portal = get_portal(env, portal_id)?;
    if portal.owner != *owner {
        return Err(PortalError::NotAuthorized);
    }
    if metadata.name.len() == 0
        || metadata.name.len() > MAX_PORTAL_NAME_LEN
        || metadata.description.len() > MAX_PORTAL_DESCRIPTION_LEN
        || metadata.tags.len() > MAX_PORTAL_TAGS
    {
        return Err(PortalError::InvalidMetadata);
    }
    env.storage()
        .persistent()
        .set(&PortalKey::PortalMeta(portal_id), &metadata);
    Ok(())
}

pub fn get_portal_metadata(env: &Env, portal_id: u64) -> Option<PortalMetadata> {
    env.storage()
        .persistent()
        .get(&PortalKey::PortalMeta(portal_id))
}

#[cfg(test)]
mod tests {
    use super::*;
    use soroban_sdk::testutils::{Address as _, Ledger as _};
    use soroban_sdk::{contract, contractimpl};

    #[contract]
    struct Stub;
    #[contractimpl]
    impl Stub {}

    struct Setup {
        env: Env,
        id: Address,
        admin: Address,
        owner: Address,
    }

    impl Setup {
        /// One auth-checked call per frame (the test host rejects repeats).
        fn run<T>(&self, f: impl FnOnce() -> T) -> T {
            self.env.as_contract(&self.id, f)
        }
    }

    fn setup() -> Setup {
        let env = Env::default();
        env.mock_all_auths();
        let id = env.register(Stub, ());
        let admin = Address::generate(&env);
        let owner = Address::generate(&env);
        let s = Setup {
            env,
            id,
            admin,
            owner,
        };
        s.run(|| initialize_portal_registry(&s.env, &s.admin));
        s
    }

    fn register(s: &Setup, from: u64, to: u64) -> u64 {
        s.run(|| register_portal(&s.env, &s.owner, from, to).unwrap())
    }

    #[test]
    fn discovery_lists_outgoing_portals() {
        let s = setup();
        let a = register(&s, 1, 2);
        let b = register(&s, 1, 3);
        let c = register(&s, 2, 1);
        s.run(|| {
            assert_eq!(discover_portals(&s.env, 1), Vec::from_array(&s.env, [a, b]));
            assert_eq!(discover_portals(&s.env, 2), Vec::from_array(&s.env, [c]));
            assert_eq!(discover_portals(&s.env, 9).len(), 0);
            assert_eq!(get_portal_count(&s.env), 3);
        });
    }

    #[test]
    fn topology_pages_with_live_state() {
        let s = setup();
        for n in 0..4 {
            register(&s, n, n + 1);
        }
        s.run(|| set_portal_open(&s.env, &s.owner, 1, false).unwrap());
        s.env.ledger().set_timestamp(10 * 86_400); // 20 points of decay
        s.run(|| {
            let page = get_portal_topology(&s.env, 1, 2);
            assert_eq!(page.len(), 2);
            let first = page.get(0).unwrap();
            assert_eq!((first.id, first.open, first.stability), (1, false, 80));
            assert_eq!(first.travel_cost, BASE_TRAVEL_COST * 100 / 80);
            assert_eq!(get_portal_topology(&s.env, 3, 10).len(), 1);
            assert_eq!(get_portal_topology(&s.env, 0, 1_000).len(), 4);
        });
    }

    #[test]
    fn closed_portals_refuse_travel_until_reopened() {
        let s = setup();
        let p = register(&s, 1, 2);
        let stranger = Address::generate(&s.env);
        s.run(|| {
            assert_eq!(
                set_portal_open(&s.env, &stranger, p, false),
                Err(PortalError::NotAuthorized)
            );
        });
        // The registry admin can close any portal.
        s.run(|| set_portal_open(&s.env, &s.admin, p, false).unwrap());
        s.run(|| {
            assert_eq!(
                travel_through_portal(&s.env, p),
                Err(PortalError::PortalClosed)
            );
        });
        s.run(|| set_portal_open(&s.env, &s.owner, p, true).unwrap());
        s.run(|| assert_eq!(travel_through_portal(&s.env, p), Ok(BASE_TRAVEL_COST)));
    }

    #[test]
    fn access_modes_are_enforced() {
        let s = setup();
        let p = register(&s, 1, 2);
        let guest = Address::generate(&s.env);
        let travel = |who: &Address| s.run(|| travel_through_portal_as(&s.env, who, p));

        assert_eq!(travel(&guest), Ok(BASE_TRAVEL_COST));

        s.run(|| set_portal_access(&s.env, &s.owner, p, PortalAccessMode::OwnerOnly).unwrap());
        assert_eq!(travel(&guest), Err(PortalError::NotAuthorized));
        assert_eq!(travel(&s.owner), Ok(BASE_TRAVEL_COST));
        // Anonymous travel cannot bypass a restricted portal.
        s.run(|| {
            assert_eq!(
                travel_through_portal(&s.env, p),
                Err(PortalError::NotAuthorized)
            );
        });

        s.run(|| set_portal_access(&s.env, &s.owner, p, PortalAccessMode::Allowlist).unwrap());
        assert_eq!(travel(&guest), Err(PortalError::NotAuthorized));
        s.run(|| set_portal_allowed(&s.env, &s.owner, p, &guest, true).unwrap());
        assert_eq!(travel(&guest), Ok(BASE_TRAVEL_COST));
        s.run(|| set_portal_allowed(&s.env, &s.owner, p, &guest, false).unwrap());
        assert_eq!(travel(&guest), Err(PortalError::NotAuthorized));
    }

    #[test]
    fn metadata_is_owner_only_and_bounded() {
        let s = setup();
        let p = register(&s, 1, 2);
        let meta = PortalMetadata {
            name: String::from_str(&s.env, "Orion Gate"),
            description: String::from_str(&s.env, "Fast lane to the trade hub"),
            tags: Vec::from_array(&s.env, [symbol_short!("trade")]),
        };
        s.run(|| set_portal_metadata(&s.env, &s.owner, p, meta.clone()).unwrap());
        s.run(|| assert_eq!(get_portal_metadata(&s.env, p), Some(meta.clone())));

        let stranger = Address::generate(&s.env);
        s.run(|| {
            assert_eq!(
                set_portal_metadata(&s.env, &stranger, p, meta.clone()),
                Err(PortalError::NotAuthorized)
            );
        });
        let empty = PortalMetadata {
            name: String::from_str(&s.env, ""),
            ..meta
        };
        s.run(|| {
            assert_eq!(
                set_portal_metadata(&s.env, &s.owner, p, empty),
                Err(PortalError::InvalidMetadata)
            );
        });
    }

    #[test]
    fn unstable_portals_still_refuse_travel() {
        let s = setup();
        let p = register(&s, 1, 2);
        s.env.ledger().set_timestamp(46 * 86_400); // stability 8 < 10
        s.run(|| {
            assert_eq!(
                travel_through_portal(&s.env, p),
                Err(PortalError::PortalUnstable)
            );
        });
    }
}
