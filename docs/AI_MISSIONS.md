# AI Mission Generation System (#639)

## Overview
`src/ai_mission_engine.rs` and `src/mission_generator.rs` generate player-tailored procedural missions with adaptive difficulty scaling, 54 archetype templates, template cooldowns, story hooks, and reward scaling.

## Contract Entrypoints
- `generate_ai_mission(player)` — Analyzes player stats via `analyze_player_behavior`, computes adaptive difficulty (`100..=500` bps) including seasonal modifiers, selects a non-cooldown template (`TEMPLATE_COOLDOWN_SECS = 3600`), persists a `Mission` with `narrative_tier`, `archetype_tag`, `title`, and `description`, and emits `(mission, ai_gen)`.
- `generate_daily_mission`, `update_mission_progress`, `complete_mission`, `get_player_missions`.

## Frontend
- `frontend/src/views/Missions.tsx` displays dynamic AI missions, archetype/narrative tier tags, progress, and reward claiming.
