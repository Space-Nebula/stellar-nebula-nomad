//! Centralized configuration management system.
//! Provides unified configuration structures, environment overrides, validation, and dynamic updates.

use soroban_sdk::{contracttype, Address, Env, Symbol};

/// Deployment environment tier.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[contracttype]
pub enum Environment {
    Local = 0,
    Testnet = 1,
    Mainnet = 2,
}

/// Global system configuration.
#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct CentralConfig {
    pub env: Environment,
    pub admin: Address,
    pub paused: bool,
    pub max_batch_size: u32,
    pub ttl_default: u32,
    pub gas_price_ceiling: u64,
}

impl CentralConfig {
    /// Create a new configuration with sane defaults.
    pub fn new(admin: Address, env: Environment) -> Self {
        let (ttl_default, max_batch_size) = match env {
            Environment::Local => (10_000, 50),
            Environment::Testnet => (518_400, 100),
            Environment::Mainnet => (3_110_400, 200),
        };

        Self {
            env,
            admin,
            paused: false,
            max_batch_size,
            ttl_default,
            gas_price_ceiling: 1_000_000,
        }
    }

    /// Validate configuration constraints.
    pub fn validate(&self) -> bool {
        if self.max_batch_size == 0 || self.max_batch_size > 1000 {
            return false;
        }
        if self.ttl_default == 0 {
            return false;
        }
        true
    }

    /// Update dynamic parameters safely.
    pub fn update_batch_size(&mut self, new_size: u32) -> Result<(), ()> {
        if new_size == 0 || new_size > 1000 {
            return Err(());
        }
        self.max_batch_size = new_size;
        Ok(())
    }

    /// Toggle pause state.
    pub fn set_paused(&mut self, paused: bool) {
        self.paused = paused;
    }
}
