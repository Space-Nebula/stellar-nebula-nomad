use soroban_sdk::contracttype;

/// Placeholder for nebula scan result (see issue #1).
#[derive(Clone)]
#[contracttype]
pub struct NebulaScan {
    pub region_id: u64,
    pub density: u32,
}

