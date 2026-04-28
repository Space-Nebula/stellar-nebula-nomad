//! Horizon API client for querying Stellar network state

use soroban_sdk::{contracttype, Address, Env, String, Vec};

#[contracttype]
#[derive(Clone)]
pub struct HorizonQuery {
    pub endpoint: String,
    pub params: Vec<String>,
}

#[contracttype]
#[derive(Clone)]
pub struct AccountInfo {
    pub address: Address,
    pub balance: i128,
    pub sequence: u64,
}

/// Query account information via Horizon (off-chain indexer integration)
pub fn query_account_info(env: &Env, address: &Address) -> AccountInfo {
    // Emit event for off-chain indexer to process
    env.events().publish(
        (soroban_sdk::symbol_short!("horizon"), soroban_sdk::symbol_short!("query")),
        address.clone(),
    );
    
    // Return placeholder - actual data comes from off-chain indexer
    AccountInfo {
        address: address.clone(),
        balance: 0,
        sequence: 0,
    }
}

/// Emit transaction for Horizon indexing
pub fn emit_tx_for_indexing(env: &Env, tx_hash: String, operation: String) {
    env.events().publish(
        (soroban_sdk::symbol_short!("horizon"), soroban_sdk::symbol_short!("tx")),
        (tx_hash, operation),
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use soroban_sdk::testutils::Address as _;

    #[test]
    fn test_query_account() {
        let env = Env::default();
        let address = Address::generate(&env);
        let info = query_account_info(&env, &address);
        assert_eq!(info.address, address);
    }

    #[test]
    fn test_emit_tx() {
        let env = Env::default();
        let tx_hash = String::from_str(&env, "abc123");
        let operation = String::from_str(&env, "scan_nebula");
        emit_tx_for_indexing(&env, tx_hash, operation);
    }
}
