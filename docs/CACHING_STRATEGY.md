# Caching strategy

Caching is layered by data lifetime:

- Contract layer: `src/cache_ttl_manager.rs` provides TTL storage, stale detection, bounded pruning, and namespace invalidation.
- Cross-layer helpers: `src/cache_layers.rs` maps contract, API, state, and asset data to namespaces and TTLs.
- Frontend API responses: `apiResponseCache` uses a 60-second TTL.
- Frontend state: `frontendStateCache` uses a 5-minute TTL.
- Frontend assets: `assetCache` uses a 1-hour TTL.

All frontend caches are backed by `ApplicationCache`, which stores JSON records with expiry metadata, evicts least-recently-stored entries, and supports per-key and full-cache invalidation. `invalidateCaches()` clears all application caches after privileged mutations or deployments.

Contract caches use `invalidate_layer()` for full namespace invalidation and `invalidate_cache_entry()` for targeted invalidation.
