use soroban_sdk::symbol_short;

use crate::cache_ttl_manager::{
    cache_with_ttl, get_cached_with_ttl, invalidate_namespace, CacheTtlError,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[soroban_sdk::contracttype]
pub enum CacheLayer {
    Contract,
    Api,
    State,
    Asset,
}

pub const API_RESPONSE_TTL: u64 = 60;
pub const ASSET_METADATA_TTL: u64 = 3_600;

pub fn layer_namespace(layer: CacheLayer) -> soroban_sdk::Symbol {
    match layer {
        CacheLayer::Contract => symbol_short!("contract"),
        CacheLayer::Api => symbol_short!("api"),
        CacheLayer::State => symbol_short!("state"),
        CacheLayer::Asset => symbol_short!("asset"),
    }
}

pub fn layer_ttl(layer: CacheLayer) -> u64 {
    match layer {
        CacheLayer::Contract => crate::cache_ttl_manager::STATE_SNAPSHOT_TTL,
        CacheLayer::Api => API_RESPONSE_TTL,
        CacheLayer::State => crate::cache_ttl_manager::PLAYER_PROFILE_TTL,
        CacheLayer::Asset => ASSET_METADATA_TTL,
    }
}

pub fn cache_layer_value(
    env: &soroban_sdk::Env,
    layer: CacheLayer,
    key: soroban_sdk::Symbol,
    value: soroban_sdk::Bytes,
) -> Result<(), CacheTtlError> {
    cache_with_ttl(
        env,
        layer_namespace(layer),
        key,
        value,
        layer_ttl(layer),
    )
}

pub fn get_layer_value(
    env: &soroban_sdk::Env,
    layer: CacheLayer,
    key: soroban_sdk::Symbol,
) -> Result<soroban_sdk::Bytes, CacheTtlError> {
    get_cached_with_ttl(env, layer_namespace(layer), key)
}

pub fn invalidate_layer(
    env: &soroban_sdk::Env,
    layer: CacheLayer,
    reason: soroban_sdk::Symbol,
) -> u32 {
    invalidate_namespace(env, layer_namespace(layer), reason)
}
