//! Classification of suspicious gameplay and telemetry anomalies.
//!
use soroban_sdk::{contracterror, contracttype, symbol_short, Env, Symbol, Vec};

#[derive(Clone)]
#[contracttype]
pub enum AnomalyKey {
    Classification(u64),
}

#[derive(Clone, Debug, PartialEq)]
#[contracttype]
pub struct ClassificationRecord {
    pub anomaly_id: u64,
    pub anomaly_type: Symbol,
    pub confidence: u32,
    pub last_updated: u64,
    pub scan_count: u32,
}

#[contracterror]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum AnomalyError {
    InsufficientFeatures = 1,
    NotFound = 2,
    Unauthorized = 3,
}

impl crate::error_standard::StandardContractError for AnomalyError {
    fn descriptor(self) -> crate::error_standard::ErrorDescriptor {
        use crate::error_standard::ErrorKind;
        let (kind, retryable) = match self {
            Self::InsufficientFeatures => (ErrorKind::ResourceLimit, false),
            Self::NotFound => (ErrorKind::NotFound, false),
            Self::Unauthorized => (ErrorKind::Authorization, false),
        };
        crate::error_standard::ErrorDescriptor {
            module: "anomaly_classifier",
            code: self as u32,
            kind,
            retryable,
        }
    }
}

pub fn classify_anomaly(
    env: &Env,
    anomaly_id: u64,
    features: Vec<u32>,
) -> Result<ClassificationRecord, AnomalyError> {
    if features.len() < 3 {
        return Err(AnomalyError::InsufficientFeatures);
    }

    let mut score = 0u32;
    for f in features.iter() {
        score = score.saturating_add(f);
    }

    let anomaly_type = if score > 200 {
        symbol_short!("blackhole")
    } else if score > 120 {
        symbol_short!("wormhole")
    } else {
        symbol_short!("nebula")
    };

    let confidence = core::cmp::min(100, score / 3);
    let record = ClassificationRecord {
        anomaly_id,
        anomaly_type: anomaly_type.clone(),
        confidence,
        last_updated: env.ledger().timestamp(),
        scan_count: 1,
    };

    env.storage()
        .instance()
        .set(&AnomalyKey::Classification(anomaly_id), &record);

    env.events().publish(
        (symbol_short!("anomaly"), symbol_short!("classify")),
        (anomaly_id, anomaly_type, confidence),
    );

    Ok(record)
}

pub fn classify_batch(env: &Env, records: Vec<(u64, Vec<u32>)>) -> Vec<ClassificationRecord> {
    let mut out = Vec::new(env);

    for rec in records.into_iter() {
        let (id, features) = rec;
        if let Ok(classified) = classify_anomaly(env, id, features.clone()) {
            out.push_back(classified);
        }
    }

    out
}

pub fn refine_classification(
    env: &Env,
    anomaly_id: u64,
    new_data: Vec<u32>,
) -> Result<ClassificationRecord, AnomalyError> {
    let mut existing = env
        .storage()
        .instance()
        .get::<AnomalyKey, ClassificationRecord>(&AnomalyKey::Classification(anomaly_id))
        .ok_or(AnomalyError::NotFound)?;

    if new_data.len() < 1 {
        return Err(AnomalyError::InsufficientFeatures);
    }

    let mut score = existing.confidence * existing.scan_count;
    let mut new_score = 0u32;
    for f in new_data.iter() {
        new_score = new_score.saturating_add(f);
    }
    score = score.saturating_add(new_score);
    existing.scan_count = existing.scan_count.saturating_add(1);
    existing.confidence = core::cmp::min(100, score / existing.scan_count);
    existing.last_updated = env.ledger().timestamp();
    existing.anomaly_type = if existing.confidence > 80 {
        symbol_short!("wormhole")
    } else if existing.confidence > 50 {
        symbol_short!("nebula")
    } else {
        symbol_short!("anomaly")
    };

    env.storage()
        .instance()
        .set(&AnomalyKey::Classification(anomaly_id), &existing);

    env.events().publish(
        (symbol_short!("anomaly"), symbol_short!("refined")),
        (
            anomaly_id,
            existing.anomaly_type.clone(),
            existing.confidence,
        ),
    );

    Ok(existing)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[contracttype]
#[repr(u32)]
pub enum AnomalyCategory {
    Cosmic = 1,
    Spatial = 2,
    Temporal = 3,
    Energetic = 4,
    Gravitational = 5,
}

#[derive(Clone, Debug, PartialEq)]
#[contracttype]
pub struct AnomalyEffect {
    pub category: AnomalyCategory,
    pub risk_score: u32,
    pub reward_multiplier_bps: u32,
    pub energy_cost_delta: i32,
    pub shield_damage: u32,
    pub yield_bonus_pct: u32,
}

pub fn get_anomaly_effect(anomaly_type: &Symbol, confidence: u32) -> AnomalyEffect {
    let conf = confidence.min(100);
    if *anomaly_type == symbol_short!("blackhole") {
        AnomalyEffect {
            category: AnomalyCategory::Gravitational,
            risk_score: 85 + (conf / 10),
            reward_multiplier_bps: 200 + conf,
            energy_cost_delta: 25,
            shield_damage: 30,
            yield_bonus_pct: 60 + (conf / 2),
        }
    } else if *anomaly_type == symbol_short!("wormhole") {
        AnomalyEffect {
            category: AnomalyCategory::Spatial,
            risk_score: 55 + (conf / 10),
            reward_multiplier_bps: 150 + (conf / 2),
            energy_cost_delta: 10,
            shield_damage: 15,
            yield_bonus_pct: 35 + (conf / 4),
        }
    } else if *anomaly_type == symbol_short!("nebula") {
        AnomalyEffect {
            category: AnomalyCategory::Energetic,
            risk_score: 25 + (conf / 10),
            reward_multiplier_bps: 120 + (conf / 4),
            energy_cost_delta: -5,
            shield_damage: 5,
            yield_bonus_pct: 20 + (conf / 5),
        }
    } else {
        AnomalyEffect {
            category: AnomalyCategory::Cosmic,
            risk_score: 15,
            reward_multiplier_bps: 110,
            energy_cost_delta: 0,
            shield_damage: 0,
            yield_bonus_pct: 10,
        }
    }
}

pub fn calculate_risk_reward(env: &Env, anomaly_id: u64) -> Result<AnomalyEffect, AnomalyError> {
    let record = get_classification(env, anomaly_id).ok_or(AnomalyError::NotFound)?;
    Ok(get_anomaly_effect(&record.anomaly_type, record.confidence))
}

pub fn classify_nebula_anomaly(
    env: &Env,
    anomaly_id: u64,
    anomaly: &crate::nebula_gen::Anomaly,
) -> Result<ClassificationRecord, AnomalyError> {
    use crate::nebula_gen::{AnomalyType, ResourceClass};

    let type_weight = match anomaly.anomaly_type {
        AnomalyType::DustCloud => 20u32,
        AnomalyType::IonStorm => 45u32,
        AnomalyType::CrystalFormation => 55u32,
        AnomalyType::PlasmaVent => 70u32,
        AnomalyType::DarkMatterPocket => 90u32,
    };
    let resource_weight = match anomaly.resource_class {
        ResourceClass::Sparse => 15u32,
        ResourceClass::Moderate => 35u32,
        ResourceClass::Abundant => 65u32,
    };
    let rarity_weight = (anomaly.rarity.min(100)) as u32;

    let mut features = Vec::new(env);
    features.push_back(type_weight);
    features.push_back(resource_weight);
    features.push_back(rarity_weight);
    classify_anomaly(env, anomaly_id, features)
}

pub fn get_classification(env: &Env, anomaly_id: u64) -> Option<ClassificationRecord> {
    env.storage()
        .instance()
        .get::<AnomalyKey, ClassificationRecord>(&AnomalyKey::Classification(anomaly_id))
}

#[cfg(test)]
mod tests {
    use super::*;
    use soroban_sdk::{contract, contractimpl, vec, Address, Env};

    #[contract]
    struct Stub;
    #[contractimpl]
    impl Stub {}

    fn make_env() -> (Env, Address) {
        let env = Env::default();
        let id = env.register(Stub, ());
        (env, id)
    }

    #[test]
    fn test_classify_anomaly_rejects_too_few_features() {
        let (env, contract_id) = make_env();
        env.as_contract(&contract_id, || {
            let result = classify_anomaly(&env, 1, vec![&env, 1u32, 2u32]);
            assert_eq!(result, Err(AnomalyError::InsufficientFeatures));
        });
    }

    #[test]
    fn test_classify_anomaly_boundary_scores() {
        let (env, contract_id) = make_env();
        env.as_contract(&contract_id, || {
            // score exactly 120 -> "nebula" (not > 120)
            let r = classify_anomaly(&env, 1, vec![&env, 40u32, 40u32, 40u32]).unwrap();
            assert_eq!(r.anomaly_type, symbol_short!("nebula"));

            // score exactly 201 -> "blackhole" (> 200)
            let r = classify_anomaly(&env, 2, vec![&env, 100u32, 100u32, 1u32]).unwrap();
            assert_eq!(r.anomaly_type, symbol_short!("blackhole"));

            // score exactly 121 -> "wormhole" (> 120, <= 200)
            let r = classify_anomaly(&env, 3, vec![&env, 100u32, 20u32, 1u32]).unwrap();
            assert_eq!(r.anomaly_type, symbol_short!("wormhole"));
        });
    }

    #[test]
    fn test_classify_anomaly_saturates_on_overflow() {
        let (env, contract_id) = make_env();
        env.as_contract(&contract_id, || {
            let r = classify_anomaly(&env, 1, vec![&env, u32::MAX, u32::MAX, u32::MAX]).unwrap();
            // confidence must clamp to 100, never overflow/panic.
            assert_eq!(r.confidence, 100);
        });
    }

    #[test]
    fn test_refine_classification_missing_record() {
        let (env, contract_id) = make_env();
        env.as_contract(&contract_id, || {
            let result = refine_classification(&env, 999, vec![&env, 1u32]);
            assert_eq!(result, Err(AnomalyError::NotFound));
        });
    }

    #[test]
    fn test_refine_classification_rejects_empty_data() {
        let (env, contract_id) = make_env();
        env.as_contract(&contract_id, || {
            classify_anomaly(&env, 1, vec![&env, 1u32, 2u32, 3u32]).unwrap();
            let result = refine_classification(&env, 1, vec![&env]);
            assert_eq!(result, Err(AnomalyError::InsufficientFeatures));
        });
    }

    #[test]
    fn test_classify_batch_skips_invalid_entries() {
        let (env, contract_id) = make_env();
        env.as_contract(&contract_id, || {
            let batch = vec![
                &env,
                (1u64, vec![&env, 1u32, 2u32, 3u32]),
                (2u64, vec![&env, 1u32]), // invalid: too few features, silently skipped
            ];
            let out = classify_batch(&env, batch);
            assert_eq!(out.len(), 1);
            assert_eq!(out.get(0).unwrap().anomaly_id, 1);
            assert!(get_classification(&env, 2).is_none());
        });
    }

    #[test]
    fn test_get_classification_missing_returns_none() {
        let (env, contract_id) = make_env();
        env.as_contract(&contract_id, || {
            assert!(get_classification(&env, 42).is_none());
        });
    }

    #[test]
    fn test_anomaly_effects_and_nebula_integration() {
        let (env, contract_id) = make_env();
        env.as_contract(&contract_id, || {
            let anomaly = crate::nebula_gen::Anomaly {
                x: 4,
                y: 9,
                rarity: 80,
                anomaly_type: crate::nebula_gen::AnomalyType::DarkMatterPocket,
                resource_class: crate::nebula_gen::ResourceClass::Abundant,
            };
            let record = classify_nebula_anomaly(&env, 101, &anomaly).unwrap();
            assert_eq!(record.anomaly_type, symbol_short!("blackhole"));
            let effect = calculate_risk_reward(&env, 101).unwrap();
            assert_eq!(effect.category, AnomalyCategory::Gravitational);
            assert!(effect.reward_multiplier_bps > 200);
        });
    }
}
