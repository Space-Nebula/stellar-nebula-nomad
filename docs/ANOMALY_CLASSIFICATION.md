# Cosmic Anomaly Classification and Effects (#640)

## Overview
`src/anomaly_classifier.rs` and `src/nebula_gen.rs` classify spatial/cosmic anomalies, assign gameplay effects and risk/reward multipliers, and integrate directly with procedurally generated nebula layouts.

## Contract Entrypoints
- `classify_anomaly(anomaly_id, features)` / `classify_batch(items)` / `refine_classification(anomaly_id, new_data)` / `get_classification(anomaly_id)`
- `get_anomaly_effect(anomaly_type, confidence)` — Maps classified anomalies to `AnomalyCategory` (`Cosmic`, `Spatial`, `Temporal`, `Energetic`, `Gravitational`) and `AnomalyEffect` (`risk_score`, `reward_multiplier_bps`, `energy_cost_delta`, `shield_damage`, `yield_bonus_pct`).
- `calculate_anomaly_risk_reward(anomaly_id)` — Computes risk/reward for a stored classification.
- `NebulaGen::classify_layout_anomaly_at(ship_id, anomaly_index)` — Classifies an anomaly from a ship's active `NebulaGenLayout`.

## Frontend
- `frontend/src/components/AnomalyIndicator.tsx` renders anomaly category, confidence, risk level, and reward/yield modifiers.
