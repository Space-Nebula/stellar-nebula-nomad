use soroban_sdk::{contracttype, Address, String};

/// Ship NFT data structure for explorer vessels.
#[derive(Clone)]
#[contracttype]
pub struct Ship {
    pub id: u64,
    pub owner: Address,
    pub name: String,
    pub level: u32,
    pub scan_range: u32,
    /// Marketplace-compatible NFT metadata URI (for example, ipfs://..., https://..., or ar://...).
    pub metadata_uri: String,
}
