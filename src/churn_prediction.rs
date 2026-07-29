//! Predictive churn detection system for identifying at-risk players.
//!
//! Uses player engagement metrics and behavioral patterns to predict churn risk
//! and trigger early intervention campaigns.

use soroban_sdk::{contracterror, contracttype, symbol_short, Address, Env, Vec};
use crate::error_standard::{ErrorDescriptor, ErrorKind, StandardContractError};

// ── Error ─────────────────────────────────────────────────────────────────────

#[contracterror]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum ChurnPredictionError {
    /// Insufficient data for prediction.
    InsufficientData = 1,
    /// Invalid risk threshold.
    InvalidThreshold = 2,
}

impl StandardContractError for ChurnPredictionError {
    fn descriptor(self) -> ErrorDescriptor {
        ErrorDescriptor {
            module: "churn_prediction",
            code: self as u32,
            kind: ErrorKind::Validation,
            retryable: false,
        }
    }
}

// ── Storage Keys ──────────────────────────────────────────────────────────────

#[contracttype]
#[derive(Clone)]
pub enum ChurnKey {
    /// Player's churn risk score and prediction data.
    ChurnRisk(Address),
    /// List of players identified as at-risk.
    AtRiskPlayers,
    /// Interventions sent to players.
    InterventionHistory(Address),
    /// Model parameters for churn prediction.
    ModelParameters,
    /// Churn prediction metrics snapshot.
    PredictionMetrics,
}

// ── Data Types ────────────────────────────────────────────────────────────────

/// Churn risk levels.
#[contracttype]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ChurnRiskLevel {
    /// Low risk (0-20%).
    Low = 1,
    /// Medium risk (20-50%).
    Medium = 2,
    /// High risk (50-80%).
    High = 3,
    /// Critical risk (80-100%).
    Critical = 4,
}

/// Player churn risk prediction.
#[contracttype]
#[derive(Clone)]
pub struct ChurnRiskPrediction {
    pub player: Address,
    /// Churn probability (0-10000 basis points).
    pub risk_score: u32,
    pub risk_level: ChurnRiskLevel,
    /// Days until predicted churn (0 = imminent).
    pub days_to_churn: u32,
    /// Key risk factors contributing to prediction.
    pub risk_factors: Vec<Symbol>,
    /// Timestamp of last prediction update.
    pub last_updated: u64,
    /// Whether intervention has been triggered.
    pub intervention_sent: bool,
}

/// Intervention campaign.
#[contracttype]
#[derive(Clone)]
pub struct ChurnIntervention {
    pub player: Address,
    pub intervention_type: Symbol,
    /// Incentive offered (e.g., bonus essence amount).
    pub incentive_amount: u64,
    /// Has player responded to intervention?
    pub responded: bool,
    pub created_at: u64,
    pub responded_at: u64,
}

/// Model parameters for churn prediction algorithm.
#[contracttype]
#[derive(Clone)]
pub struct ChurnPredictionModel {
    /// Weight for inactivity factor (0-1000 = 0-100%).
    pub inactivity_weight: u32,
    /// Weight for declining engagement.
    pub engagement_decline_weight: u32,
    /// Weight for low lifetime value.
    pub ltv_weight: u32,
    /// Inactivity threshold (days).
    pub inactivity_threshold: u32,
    /// Engagement decline threshold (percentage).
    pub engagement_decline_threshold: u32,
    /// LTV threshold for low-value identification.
    pub ltv_threshold: u64,
}

/// Aggregate churn prediction metrics.
#[contracttype]
#[derive(Clone)]
pub struct ChurnPredictionMetrics {
    pub total_players_analyzed: u32,
    pub low_risk_count: u32,
    pub medium_risk_count: u32,
    pub high_risk_count: u32,
    pub critical_risk_count: u32,
    pub interventions_sent: u32,
    pub intervention_success_rate: u32, // basis points
    pub avg_risk_score: u32,
    pub last_calculation: u64,
}

// ── Write Helpers ─────────────────────────────────────────────────────────────

/// Initialize churn prediction model with default parameters.
pub fn initialize_churn_model(env: &Env, admin: &Address) {
    admin.require_auth();

    let model = ChurnPredictionModel {
        inactivity_weight: 400,   // 40%
        engagement_decline_weight: 300, // 30%
        ltv_weight: 300,         // 30%
        inactivity_threshold: 14, // 14 days
        engagement_decline_threshold: 50, // 50% decline
        ltv_threshold: 1_000,    // 1000 essence
    };

    env.storage()
        .persistent()
        .set(&ChurnKey::ModelParameters, &model);

    env.events().publish(
        (symbol_short!("churn"), symbol_short!("init")),
        (),
    );
}

/// Calculate churn risk for a player based on engagement metrics.
pub fn predict_churn_risk(
    env: &Env,
    player: &Address,
    days_inactive: u32,
    sessions_last_30d: u32,
    sessions_last_60d: u32,
    lifetime_essence: u64,
    total_sessions_ever: u32,
) -> ChurnRiskPrediction {
    let model: ChurnPredictionModel = env
        .storage()
        .persistent()
        .get(&ChurnKey::ModelParameters)
        .unwrap_or(ChurnPredictionModel {
            inactivity_weight: 400,
            engagement_decline_weight: 300,
            ltv_weight: 300,
            inactivity_threshold: 14,
            engagement_decline_threshold: 50,
            ltv_threshold: 1_000,
        });

    let mut risk_score: u32 = 0;
    let mut risk_factors: Vec<Symbol> = Vec::new(env);

    // Factor 1: Inactivity score
    let inactivity_score = calculate_inactivity_score(
        days_inactive,
        model.inactivity_threshold,
    );
    risk_score = risk_score.saturating_add(
        (inactivity_score as u32 * model.inactivity_weight) / 1000
    );

    if inactivity_score > 5000 {
        risk_factors.push_back(symbol_short!("inactive"));
    }

    // Factor 2: Engagement decline
    let engagement_score = if sessions_last_60d > 0 {
        ((sessions_last_30d as u64 * 10000) / sessions_last_60d as u64) as u32
    } else {
        0
    };

    let engagement_decline = if engagement_score < 5000 {
        10000 - engagement_score
    } else {
        0
    };

    risk_score = risk_score.saturating_add(
        (engagement_decline * model.engagement_decline_weight) / 1000
    );

    if engagement_decline > 5000 {
        risk_factors.push_back(symbol_short!("decline"));
    }

    // Factor 3: Low lifetime value
    let ltv_score = if lifetime_essence < model.ltv_threshold {
        ((model.ltv_threshold - lifetime_essence.min(model.ltv_threshold)) as u32
            * 10000)
            / model.ltv_threshold as u32
    } else {
        0
    };

    risk_score = risk_score.saturating_add(
        (ltv_score * model.ltv_weight) / 1000
    );

    if ltv_score > 5000 {
        risk_factors.push_back(symbol_short!("lowltv"));
    }

    // Clamp risk score to 0-10000
    risk_score = risk_score.min(10000);

    let risk_level = calculate_risk_level(risk_score);
    let days_to_churn = estimate_churn_timeline(risk_score);

    let prediction = ChurnRiskPrediction {
        player: player.clone(),
        risk_score,
        risk_level,
        days_to_churn,
        risk_factors,
        last_updated: env.ledger().timestamp(),
        intervention_sent: false,
    };

    env.storage()
        .persistent()
        .set(&ChurnKey::ChurnRisk(player.clone()), &prediction);

    env.events().publish(
        (symbol_short!("churn"), symbol_short!("predict")),
        (player.clone(), risk_score, risk_level as u32),
    );

    prediction
}

/// Send intervention to at-risk player.
pub fn send_intervention(
    env: &Env,
    admin: &Address,
    player: &Address,
    intervention_type: Symbol,
    incentive: u64,
) {
    admin.require_auth();

    let intervention = ChurnIntervention {
        player: player.clone(),
        intervention_type: intervention_type.clone(),
        incentive_amount: incentive,
        responded: false,
        created_at: env.ledger().timestamp(),
        responded_at: 0,
    };

    let mut history: Vec<ChurnIntervention> = env
        .storage()
        .persistent()
        .get(&ChurnKey::InterventionHistory(player.clone()))
        .unwrap_or_else(|| Vec::new(env));

    history.push_back(intervention.clone());

    env.storage()
        .persistent()
        .set(&ChurnKey::InterventionHistory(player.clone()), &history);

    // Mark prediction as having intervention sent
    let mut prediction: ChurnRiskPrediction = env
        .storage()
        .persistent()
        .get(&ChurnKey::ChurnRisk(player.clone()))
        .unwrap_or(ChurnRiskPrediction {
            player: player.clone(),
            risk_score: 0,
            risk_level: ChurnRiskLevel::Low,
            days_to_churn: 0,
            risk_factors: Vec::new(env),
            last_updated: 0,
            intervention_sent: false,
        });

    prediction.intervention_sent = true;
    env.storage()
        .persistent()
        .set(&ChurnKey::ChurnRisk(player.clone()), &prediction);

    // Add to at-risk players list
    let mut at_risk: Vec<Address> = env
        .storage()
        .persistent()
        .get(&ChurnKey::AtRiskPlayers)
        .unwrap_or_else(|| Vec::new(env));

    // Avoid duplicates
    let mut found = false;
    for i in 0..at_risk.len() {
        if let Some(p) = at_risk.get(i) {
            if p == *player {
                found = true;
                break;
            }
        }
    }

    if !found {
        at_risk.push_back(player.clone());
        env.storage()
            .persistent()
            .set(&ChurnKey::AtRiskPlayers, &at_risk);
    }

    env.events().publish(
        (symbol_short!("churn"), symbol_short!("intervention")),
        (player.clone(), intervention_type, incentive),
    );
}

/// Mark player as responded to intervention.
pub fn record_intervention_response(env: &Env, player: &Address) {
    let mut history: Vec<ChurnIntervention> = env
        .storage()
        .persistent()
        .get(&ChurnKey::InterventionHistory(player.clone()))
        .unwrap_or_else(|| Vec::new(env));

    if history.len() > 0 {
        let mut last_intervention = history
            .get((history.len() - 1) as u32)
            .unwrap_or(ChurnIntervention {
                player: player.clone(),
                intervention_type: symbol_short!("unknown"),
                incentive_amount: 0,
                responded: false,
                created_at: 0,
                responded_at: 0,
            })
            .clone();

        last_intervention.responded = true;
        last_intervention.responded_at = env.ledger().timestamp();

        // Update the last element
        let mut new_history: Vec<ChurnIntervention> = Vec::new(env);
        for i in 0..(history.len() - 1) {
            if let Some(h) = history.get(i as u32) {
                new_history.push_back(h.clone());
            }
        }
        new_history.push_back(last_intervention);

        env.storage()
            .persistent()
            .set(&ChurnKey::InterventionHistory(player.clone()), &new_history);
    }

    env.events().publish(
        (symbol_short!("churn"), symbol_short!("respond")),
        (player.clone(),),
    );
}

/// Update model parameters.
pub fn update_model_parameters(
    env: &Env,
    admin: &Address,
    inactivity_weight: u32,
    engagement_weight: u32,
    ltv_weight: u32,
) {
    admin.require_auth();

    let model = ChurnPredictionModel {
        inactivity_weight,
        engagement_decline_weight: engagement_weight,
        ltv_weight,
        inactivity_threshold: 14,
        engagement_decline_threshold: 50,
        ltv_threshold: 1_000,
    };

    env.storage()
        .persistent()
        .set(&ChurnKey::ModelParameters, &model);

    env.events().publish(
        (symbol_short!("churn"), symbol_short!("update")),
        (inactivity_weight, engagement_weight, ltv_weight),
    );
}

// ── Helper Functions ──────────────────────────────────────────────────────────

/// Calculate inactivity risk score (0-10000).
fn calculate_inactivity_score(days_inactive: u32, threshold: u32) -> u32 {
    if days_inactive < threshold {
        0
    } else {
        let excess = days_inactive.saturating_sub(threshold);
        // Each day past threshold adds ~100 points (linear up to 100 days)
        (excess as u32 * 100).min(10000)
    }
}

/// Determine risk level from score.
fn calculate_risk_level(risk_score: u32) -> ChurnRiskLevel {
    match risk_score {
        0..=2000 => ChurnRiskLevel::Low,
        2001..=5000 => ChurnRiskLevel::Medium,
        5001..=8000 => ChurnRiskLevel::High,
        _ => ChurnRiskLevel::Critical,
    }
}

/// Estimate days until predicted churn based on risk score.
fn estimate_churn_timeline(risk_score: u32) -> u32 {
    match risk_score {
        0..=2000 => 90,  // Low risk: ~3 months
        2001..=5000 => 30, // Medium risk: ~1 month
        5001..=8000 => 14, // High risk: ~2 weeks
        _ => 3,           // Critical: ~3 days
    }
}

// ── Read-Only View Functions ──────────────────────────────────────────────────

/// Get churn risk prediction for a player.
pub fn get_churn_risk(env: &Env, player: &Address) -> ChurnRiskPrediction {
    env.storage()
        .persistent()
        .get(&ChurnKey::ChurnRisk(player.clone()))
        .unwrap_or(ChurnRiskPrediction {
            player: player.clone(),
            risk_score: 0,
            risk_level: ChurnRiskLevel::Low,
            days_to_churn: 90,
            risk_factors: Vec::new(env),
            last_updated: 0,
            intervention_sent: false,
        })
}

/// Get intervention history for a player (limited to last 10).
pub fn get_intervention_history(env: &Env, player: &Address, limit: u32) -> Vec<ChurnIntervention> {
    let all_interventions: Vec<ChurnIntervention> = env
        .storage()
        .persistent()
        .get(&ChurnKey::InterventionHistory(player.clone()))
        .unwrap_or_else(|| Vec::new(env));

    let mut result: Vec<ChurnIntervention> = Vec::new(env);
    let start_idx = if (limit as usize) < all_interventions.len() {
        all_interventions.len() - (limit as usize)
    } else {
        0
    };

    for i in start_idx..all_interventions.len() {
        if let Some(intervention) = all_interventions.get(i as u32) {
            result.push_back(intervention.clone());
        }
    }

    result
}

/// Get list of at-risk players (limited to avoid unbounded iteration).
pub fn get_at_risk_players(env: &Env, limit: u32) -> Vec<Address> {
    let all_at_risk: Vec<Address> = env
        .storage()
        .persistent()
        .get(&ChurnKey::AtRiskPlayers)
        .unwrap_or_else(|| Vec::new(env));

    let mut result: Vec<Address> = Vec::new(env);
    let count = if (limit as usize) < all_at_risk.len() {
        limit as usize
    } else {
        all_at_risk.len()
    };

    for i in 0..count {
        if let Some(player) = all_at_risk.get(i as u32) {
            result.push_back(player.clone());
        }
    }

    result
}

/// Get current churn prediction model.
pub fn get_churn_model(env: &Env) -> ChurnPredictionModel {
    env.storage()
        .persistent()
        .get(&ChurnKey::ModelParameters)
        .unwrap_or(ChurnPredictionModel {
            inactivity_weight: 400,
            engagement_decline_weight: 300,
            ltv_weight: 300,
            inactivity_threshold: 14,
            engagement_decline_threshold: 50,
            ltv_threshold: 1_000,
        })
}

/// Get aggregate churn prediction metrics.
pub fn get_churn_metrics(env: &Env) -> ChurnPredictionMetrics {
    env.storage()
        .persistent()
        .get(&ChurnKey::PredictionMetrics)
        .unwrap_or(ChurnPredictionMetrics {
            total_players_analyzed: 0,
            low_risk_count: 0,
            medium_risk_count: 0,
            high_risk_count: 0,
            critical_risk_count: 0,
            interventions_sent: 0,
            intervention_success_rate: 0,
            avg_risk_score: 0,
            last_calculation: 0,
        })
}

/// Update aggregate metrics (called periodically by monitoring systems).
pub fn update_churn_metrics(
    env: &Env,
    total_analyzed: u32,
    low: u32,
    medium: u32,
    high: u32,
    critical: u32,
    interventions: u32,
    success_rate: u32,
    avg_score: u32,
) {
    let metrics = ChurnPredictionMetrics {
        total_players_analyzed: total_analyzed,
        low_risk_count: low,
        medium_risk_count: medium,
        high_risk_count: high,
        critical_risk_count: critical,
        interventions_sent: interventions,
        intervention_success_rate: success_rate,
        avg_risk_score: avg_score,
        last_calculation: env.ledger().timestamp(),
    };

    env.storage()
        .persistent()
        .set(&ChurnKey::PredictionMetrics, &metrics);

    env.events().publish(
        (symbol_short!("churn"), symbol_short!("metrics")),
        (total_analyzed, critical, success_rate),
    );
}
