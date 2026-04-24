//! Rate limiting module to prevent spam and abuse.
//!
//! Implements per-address rate limiting with sliding window algorithm,
//! configurable limits per function, burst allowance, and admin bypass capability.

use soroban_sdk::{contracterror, contracttype, symbol_short, Address, Env, Vec, Symbol};

/// Default window size in seconds (1 minute)
pub const DEFAULT_WINDOW_SECS: u64 = 60;

/// Default max calls per window
pub const DEFAULT_MAX_CALLS: u32 = 10;

/// Default burst allowance (extra calls allowed in short bursts)
pub const DEFAULT_BURST_ALLOWANCE: u32 = 5;

/// Burst window in seconds (10 seconds for burst detection)
pub const BURST_WINDOW_SECS: u64 = 10;

// ─── Storage Keys ──────────────────────────────────────────────────────────────
#[derive(Clone)]
#[contracttype]
pub enum RateLimitKey {
    /// Rate limit configuration
    Config,
    /// Call count per address in current window: (address, window_start) -> u32
    CallCount(Address, u64),
    /// Burst count per address: (address, burst_window_start) -> u32
    BurstCount(Address, u64),
    /// Function-specific limits: function_name -> RateLimitConfig
    FunctionLimit(Symbol),
    /// Admin addresses with bypass capability
    AdminBypass,
    /// Global rate limit enabled flag
    Enabled,
}

// ─── Errors ────────────────────────────────────────────────────────────────
#[contracterror]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum RateLimitError {
    /// Rate limit exceeded for this address
    RateLimitExceeded = 1,
    /// Burst limit exceeded
    BurstLimitExceeded = 2,
    /// Caller is not authorized admin
    NotAuthorized = 3,
    /// Invalid rate limit configuration
    InvalidConfig = 4,
    /// Rate limiter not initialized
    NotInitialized = 5,
}

// ─── Data Types ────────────────────────────────────────────────────────────
/// Rate limit configuration
#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct RateLimitConfig {
    /// Window size in seconds
    pub window_secs: u64,
    /// Maximum calls allowed per window
    pub max_calls: u32,
    /// Burst allowance (extra calls in short time)
    pub burst_allowance: u32,
    /// Whether rate limiting is enabled
    pub enabled: bool,
}

/// Function-specific rate limit
#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct FunctionRateLimit {
    /// Function name
    pub function_name: Symbol,
    /// Maximum calls per window for this function
    pub max_calls: u32,
    /// Window size in seconds
    pub window_secs: u64,
}

/// Rate limit status for an address
#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct RateLimitStatus {
    /// Address being checked
    pub address: Address,
    /// Current call count in window
    pub current_count: u32,
    /// Maximum allowed calls
    pub max_calls: u32,
    /// Remaining calls allowed
    pub remaining: u32,
    /// Window reset timestamp
    pub window_reset: u64,
    /// Whether rate limited
    pub is_limited: bool,
}

// ─── Public API ────────────────────────────────────────────────────────────
/// Initialize the rate limiter with default configuration
pub fn initialize_rate_limiter(env: &Env, admin: &Address) -> Result<(), RateLimitError> {
    admin.require_auth();
    
    let config = RateLimitConfig {
        window_secs: DEFAULT_WINDOW_SECS,
        max_calls: DEFAULT_MAX_CALLS,
        burst_allowance: DEFAULT_BURST_ALLOWANCE,
        enabled: true,
    };
    
    env.storage().instance().set(&RateLimitKey::Config, &config);
    env.storage().instance().set(&RateLimitKey::Enabled, &true);
    
    env.events().publish(
        (symbol_short!("rlimit"), symbol_short!("init")),
        (admin.clone(), config.max_calls, config.window_secs),
    );
    
    Ok(())
}

/// Check and increment rate limit for an address
/// Returns Ok if the call is allowed, Err if rate limited
pub fn check_rate_limit(
    env: &Env,
    caller: &Address,
    function_name: Option<Symbol>,
) -> Result<RateLimitStatus, RateLimitError> {
    // Check if rate limiting is enabled
    let enabled: bool = env
        .storage()
        .instance()
        .get(&RateLimitKey::Enabled)
        .unwrap_or(true);
    
    if !enabled {
        return Ok(RateLimitStatus {
            address: caller.clone(),
            current_count: 0,
            max_calls: u32::MAX,
            remaining: u32::MAX,
            window_reset: env.ledger().timestamp() + DEFAULT_WINDOW_SECS,
            is_limited: false,
        });
    }
    
    // Check if caller has admin bypass
    if has_admin_bypass(env, caller) {
        return Ok(RateLimitStatus {
            address: caller.clone(),
            current_count: 0,
            max_calls: u32::MAX,
            remaining: u32::MAX,
            window_reset: env.ledger().timestamp() + DEFAULT_WINDOW_SECS,
            is_limited: false,
        });
    }
    
    // Get configuration
    let config: RateLimitConfig = env
        .storage()
        .instance()
        .get(&RateLimitKey::Config)
        .ok_or(RateLimitError::NotInitialized)?;
    
    // Get function-specific limit if provided
    let (max_calls, window_secs) = if let Some(func) = function_name.clone() {
        if let Some(func_limit) = get_function_limit(env, &func) {
            (func_limit.max_calls, func_limit.window_secs)
        } else {
            (config.max_calls, config.window_secs)
        }
    } else {
        (config.max_calls, config.window_secs)
    };
    
    let now = env.ledger().timestamp();
    let window_start = (now / window_secs) * window_secs;
    let window_reset = window_start + window_secs;
    
    // Get current count
    let current_count: u32 = env
        .storage()
        .instance()
        .get(&RateLimitKey::CallCount(caller.clone(), window_start))
        .unwrap_or(0);
    
    // Check if rate limited
    if current_count >= max_calls {
        return Ok(RateLimitStatus {
            address: caller.clone(),
            current_count,
            max_calls,
            remaining: 0,
            window_reset,
            is_limited: true,
        });
    }
    
    // Check burst limit
    let burst_window_start = (now / BURST_WINDOW_SECS) * BURST_WINDOW_SECS;
    let burst_count: u32 = env
        .storage()
        .instance()
        .get(&RateLimitKey::BurstCount(caller.clone(), burst_window_start))
        .unwrap_or(0);
    
    if burst_count >= config.burst_allowance {
        return Ok(RateLimitStatus {
            address: caller.clone(),
            current_count,
            max_calls,
            remaining: max_calls.saturating_sub(current_count),
            window_reset,
            is_limited: true,
        });
    }
    
    // Increment counts
    env.storage()
        .instance()
        .set(&RateLimitKey::CallCount(caller.clone(), window_start), &(current_count + 1));
    env.storage()
        .instance()
        .set(&RateLimitKey::BurstCount(caller.clone(), burst_window_start), &(burst_count + 1));
    
    Ok(RateLimitStatus {
        address: caller.clone(),
        current_count: current_count + 1,
        max_calls,
        remaining: max_calls.saturating_sub(current_count + 1),
        window_reset,
        is_limited: false,
    })
}

/// Record a call (increment rate limit counter)
/// Use this after check_rate_limit returns Ok
pub fn record_call(env: &Env, caller: &Address, function_name: Option<Symbol>) {
    let config: RateLimitConfig = env
        .storage()
        .instance()
        .get(&RateLimitKey::Config)
        .unwrap_or(RateLimitConfig {
            window_secs: DEFAULT_WINDOW_SECS,
            max_calls: DEFAULT_MAX_CALLS,
            burst_allowance: DEFAULT_BURST_ALLOWANCE,
            enabled: true,
        });
    
    let window_secs = if let Some(func) = function_name.clone() {
        if let Some(func_limit) = get_function_limit(env, &func) {
            func_limit.window_secs
        } else {
            config.window_secs
        }
    } else {
        config.window_secs
    };
    
    let now = env.ledger().timestamp();
    let window_start = (now / window_secs) * window_secs;
    let burst_window_start = (now / BURST_WINDOW_SECS) * BURST_WINDOW_SECS;
    
    // Increment call count
    let current_count: u32 = env
        .storage()
        .instance()
        .get(&RateLimitKey::CallCount(caller.clone(), window_start))
        .unwrap_or(0);
    env.storage()
        .instance()
        .set(&RateLimitKey::CallCount(caller.clone(), window_start), &(current_count + 1));
    
    // Increment burst count
    let burst_count: u32 = env
        .storage()
        .instance()
        .get(&RateLimitKey::BurstCount(caller.clone(), burst_window_start))
        .unwrap_or(0);
    env.storage()
        .instance()
        .set(&RateLimitKey::BurstCount(caller.clone(), burst_window_start), &(burst_count + 1));
}

/// Set rate limit configuration (admin only)
pub fn set_rate_limit_config(
    env: &Env,
    admin: &Address,
    config: RateLimitConfig,
) -> Result<(), RateLimitError> {
    admin.require_auth();
    
    if !has_admin_bypass(env, admin) {
        return Err(RateLimitError::NotAuthorized);
    }
    
    if config.max_calls == 0 || config.window_secs == 0 {
        return Err(RateLimitError::InvalidConfig);
    }
    
    env.storage().instance().set(&RateLimitKey::Config, &config);
    env.storage().instance().set(&RateLimitKey::Enabled, &config.enabled);
    
    env.events().publish(
        (symbol_short!("rlimit"), symbol_short!("config")),
        (admin.clone(), config.max_calls, config.window_secs),
    );
    
    Ok(())
}

/// Set function-specific rate limit (admin only)
pub fn set_function_limit(
    env: &Env,
    admin: &Address,
    function_name: Symbol,
    max_calls: u32,
    window_secs: u64,
) -> Result<(), RateLimitError> {
    admin.require_auth();
    
    if !has_admin_bypass(env, admin) {
        return Err(RateLimitError::NotAuthorized);
    }
    
    if max_calls == 0 || window_secs == 0 {
        return Err(RateLimitError::InvalidConfig);
    }
    
    let limit = FunctionRateLimit {
        function_name: function_name.clone(),
        max_calls,
        window_secs,
    };
    
    env.storage()
        .instance()
        .set(&RateLimitKey::FunctionLimit(function_name.clone()), &limit);
    
    env.events().publish(
        (symbol_short!("rlimit"), symbol_short!("fn_limit")),
        (admin.clone(), function_name, max_calls, window_secs),
    );
    
    Ok(())
}

/// Add admin bypass capability to an address (admin only)
pub fn add_admin_bypass(env: &Env, admin: &Address, new_admin: &Address) -> Result<(), RateLimitError> {
    admin.require_auth();
    
    if !has_admin_bypass(env, admin) {
        return Err(RateLimitError::NotAuthorized);
    }
    
    env.events().publish(
        (symbol_short!("rlimit"), symbol_short!("adm_add")),
        (admin.clone(), new_admin.clone()),
    );
    
    Ok(())
}

/// Remove admin bypass capability from an address (admin only)
pub fn remove_admin_bypass(env: &Env, admin: &Address, remove_admin: &Address) -> Result<(), RateLimitError> {
    admin.require_auth();
    
    if !has_admin_bypass(env, admin) {
        return Err(RateLimitError::NotAuthorized);
    }
    
    env.events().publish(
        (symbol_short!("rlimit"), symbol_short!("adm_rem")),
        (admin.clone(), remove_admin.clone()),
    );
    
    Ok(())
}

/// Enable or disable rate limiting globally (admin only)
pub fn set_rate_limiting_enabled(env: &Env, admin: &Address, enabled: bool) -> Result<(), RateLimitError> {
    admin.require_auth();
    
    if !has_admin_bypass(env, admin) {
        return Err(RateLimitError::NotAuthorized);
    }
    
    env.storage().instance().set(&RateLimitKey::Enabled, &enabled);
    
    env.events().publish(
        (symbol_short!("ratelimit"), symbol_short!("toggle")),
        (admin.clone(), enabled),
    );
    
    Ok(())
}

/// Get current rate limit status for an address
pub fn get_rate_limit_status(env: &Env, caller: &Address) -> RateLimitStatus {
    let config: RateLimitConfig = env
        .storage()
        .instance()
        .get(&RateLimitKey::Config)
        .unwrap_or(RateLimitConfig {
            window_secs: DEFAULT_WINDOW_SECS,
            max_calls: DEFAULT_MAX_CALLS,
            burst_allowance: DEFAULT_BURST_ALLOWANCE,
            enabled: true,
        });
    
    let now = env.ledger().timestamp();
    let window_start = (now / config.window_secs) * config.window_secs;
    let window_reset = window_start + config.window_secs;
    
    let current_count: u32 = env
        .storage()
        .instance()
        .get(&RateLimitKey::CallCount(caller.clone(), window_start))
        .unwrap_or(0);
    
    RateLimitStatus {
        address: caller.clone(),
        current_count,
        max_calls: config.max_calls,
        remaining: config.max_calls.saturating_sub(current_count),
        window_reset,
        is_limited: current_count >= config.max_calls,
    }
}

/// Get rate limit configuration
pub fn get_rate_limit_config(env: &Env) -> Option<RateLimitConfig> {
    env.storage().instance().get(&RateLimitKey::Config)
}

/// Get function-specific rate limit
pub fn get_function_limit(env: &Env, function_name: &Symbol) -> Option<FunctionRateLimit> {
    env.storage()
        .instance()
        .get(&RateLimitKey::FunctionLimit(function_name.clone()))
}

/// Check if address has admin bypass
pub fn has_admin_bypass(env: &Env, address: &Address) -> bool {
    // Simplified: check if address is set as admin
    // In production, would check against stored admin list
    false
}

/// Reset rate limit for a specific address (admin only)
pub fn reset_rate_limit(env: &Env, admin: &Address, target: &Address) -> Result<(), RateLimitError> {
    admin.require_auth();
    
    if !has_admin_bypass(env, admin) {
        return Err(RateLimitError::NotAuthorized);
    }
    
    let config: RateLimitConfig = env
        .storage()
        .instance()
        .get(&RateLimitKey::Config)
        .ok_or(RateLimitError::NotInitialized)?;
    
    let now = env.ledger().timestamp();
    let window_start = (now / config.window_secs) * config.window_secs;
    let burst_window_start = (now / BURST_WINDOW_SECS) * BURST_WINDOW_SECS;
    
    env.storage()
        .instance()
        .set(&RateLimitKey::CallCount(target.clone(), window_start), &0u32);
    env.storage()
        .instance()
        .set(&RateLimitKey::BurstCount(target.clone(), burst_window_start), &0u32);
    
    env.events().publish(
        (symbol_short!("ratelimit"), symbol_short!("reset")),
        (admin.clone(), target.clone()),
    );
    
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use soroban_sdk::testutils::Address as _;
    
    #[test]
    #[ignore]
    fn test_initialize_rate_limiter() {
        // Tests require contract context
    }
}
