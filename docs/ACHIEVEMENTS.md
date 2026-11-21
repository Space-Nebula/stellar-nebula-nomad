# Achievement & Badge System Integration (#641)

## Overview
Integrates `src/achievement_engine.rs`, `src/achievements.rs`, `src/badges.rs`, and `src/player_housing.rs` through `NebulaNomadContract` in `src/lib.rs`.

## Contract Entrypoints
- **Catalog & Progress (`achievements.rs`)**: `get_achievement_def`, `unlock_catalog_achievement`, `query_achievement_progress`, `get_achievement_stats`, `get_achievement_notifications`, `batch_unlock_catalog_achvs`.
- **Badge NFTs (`badges.rs`)**: `mint_badge_nft`, `get_badge_metadata`, `transfer_badge`, `list_badges_by_owner`.
- **Player Housing (`player_housing.rs`)**: `display_achievement` verifies unlocked achievement status and showcases it in housing display cases.

## Frontend
- `frontend/src/components/display/AchievementBadge.tsx` renders rarity-tier borders (0–4), category tags, and lock/progress state.
