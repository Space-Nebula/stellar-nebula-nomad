use soroban_sdk::{
    contract, contractimpl, contracttype, contracterror, symbol_short,
    Address, Env, Vec,
};

/// Soul-bound status for a ship NFT
#[derive(Clone)]
#[contracttype]
pub struct SoulBinding {
    pub ship_id: u64,
    pub bound_to: Address,
    pub bound_at: u64,
}

#[derive(Clone)]
#[contracttype]
pub enum DataKey {
    Binding(u64),
}

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum BindingError {
    AlreadyBound = 1,
    NotOwner = 2,
    BurstLimitExceeded = 3,
}

const MAX_BATCH_BIND: u32 = 3;

#[contract]
pub struct SoulBinder;

#[contractimpl]
impl SoulBinder {
    /// Permanently bind a ship to its current owner
    pub fn bind_ship_to_owner(
        env: Env,
        owner: Address,
        ship_id: u64,
    ) -> Result<SoulBinding, BindingError> {
        owner.require_auth();

        if env.storage().persistent().has(&DataKey::Binding(ship_id)) {
            return Err(BindingError::AlreadyBound);
        }

        let bound_at = env.ledger().timestamp();
        let binding = SoulBinding {
            ship_id,
            bound_to: owner.clone(),
            bound_at,
        };

        env.storage()
            .persistent()
            .set(&DataKey::Binding(ship_id), &binding);

        env.events().publish(
            (symbol_short!("soulbnd"), owner),
            (ship_id, bound_at),
        );

        Ok(binding)
    }

    /// Check if a ship is soul-bound
    pub fn check_binding_status(env: Env, ship_id: u64) -> Option<SoulBinding> {
        env.storage()
            .persistent()
            .get(&DataKey::Binding(ship_id))
    }

    /// Verify if a ship is bound to a specific owner
    pub fn is_bound_to(env: Env, ship_id: u64, owner: Address) -> bool {
        match Self::check_binding_status(env, ship_id) {
            Some(binding) => binding.bound_to == owner,
            None => false,
        }
    }

    /// Batch bind up to 3 ships in one transaction
    pub fn batch_bind_ships(
        env: Env,
        owner: Address,
        ship_ids: Vec<u64>,
    ) -> Result<Vec<SoulBinding>, BindingError> {
        owner.require_auth();

        if ship_ids.len() > MAX_BATCH_BIND {
            return Err(BindingError::BurstLimitExceeded);
        }

        let mut bindings = Vec::new(&env);
        for i in 0..ship_ids.len() {
            let ship_id = ship_ids.get(i).unwrap();
            let binding = Self::bind_ship_to_owner(env.clone(), owner.clone(), ship_id)?;
            bindings.push_back(binding);
        }

        Ok(bindings)
    }
}
