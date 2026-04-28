//! StellarExpert API integration for contract visibility and analytics

use soroban_sdk::{contracttype, Address, Env, String};

#[contracttype]
#[derive(Clone)]
pub struct ExpertMetadata {
    pub contract_id: String,
    pub name: String,
    pub description: String,
    pub homepage: String,
    pub repository: String,
}

/// Register contract metadata for StellarExpert indexing
pub fn register_expert_metadata(env: &Env, admin: &Address, metadata: ExpertMetadata) {
    admin.require_auth();
    env.events().publish(
        (soroban_sdk::symbol_short!("expert"), soroban_sdk::symbol_short!("meta")),
        metadata,
    );
}

/// Emit contract interaction event for StellarExpert analytics
pub fn emit_interaction_event(env: &Env, user: &Address, action: String, value: i128) {
    env.events().publish(
        (soroban_sdk::symbol_short!("expert"), soroban_sdk::symbol_short!("action")),
        (user.clone(), action, value),
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use soroban_sdk::testutils::Address as _;

    #[test]
    fn test_register_metadata() {
        let env = Env::default();
        env.mock_all_auths();
        let admin = Address::generate(&env);
        let metadata = ExpertMetadata {
            contract_id: String::from_str(&env, "CXXXXX"),
            name: String::from_str(&env, "Nebula Nomad"),
            description: String::from_str(&env, "Space exploration game"),
            homepage: String::from_str(&env, "https://nebulanomad.io"),
            repository: String::from_str(&env, "https://github.com/Space-Nebula/stellar-nebula-nomad"),
        };
        register_expert_metadata(&env, &admin, metadata);
    }
}
