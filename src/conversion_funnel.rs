//! Conversion funnel tracking for analyzing player progression and optimizing conversions.
//!
//! Tracks multi-step conversion funnels, measures dropoff at each stage,
//! and provides optimization recommendations.

use soroban_sdk::{contracterror, contracttype, symbol_short, Address, Env, Symbol, Vec};
use crate::error_standard::{ErrorDescriptor, ErrorKind, StandardContractError};

// ── Error ─────────────────────────────────────────────────────────────────────

#[contracterror]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum FunnelError {
    /// Invalid funnel stage index.
    InvalidStage = 1,
    /// Funnel not found.
    FunnelNotFound = 2,
    /// Invalid conversion rate threshold.
    InvalidThreshold = 3,
}

impl StandardContractError for FunnelError {
    fn descriptor(self) -> ErrorDescriptor {
        ErrorDescriptor {
            module: "conversion_funnel",
            code: self as u32,
            kind: ErrorKind::Validation,
            retryable: false,
        }
    }
}

// ── Storage Keys ──────────────────────────────────────────────────────────────

#[contracttype]
#[derive(Clone)]
pub enum FunnelKey {
    /// Funnel definition and stages.
    FunnelConfig(Symbol),
    /// Stage-level metrics for a funnel.
    FunnelMetrics(Symbol),
    /// Player progress through a funnel.
    PlayerFunnelProgress(Address, Symbol),
    /// Conversion events log.
    ConversionLog,
    /// Dropoff analysis and recommendations.
    DropoffAnalysis(Symbol),
}

// ── Data Types ────────────────────────────────────────────────────────────────

/// A stage in a conversion funnel.
#[contracttype]
#[derive(Clone)]
pub struct FunnelStage {
    /// Stage identifier (1-based).
    pub stage_num: u32,
    /// Stage name.
    pub name: Symbol,
    /// Description.
    pub description: Symbol,
    /// Expected conversion rate to next stage (basis points).
    pub expected_conversion_bps: u32,
    /// Alert threshold if actual conversion drops below this (basis points).
    pub alert_threshold_bps: u32,
}

/// Conversion funnel definition.
#[contracttype]
#[derive(Clone)]
pub struct ConversionFunnel {
    pub funnel_name: Symbol,
    pub description: Symbol,
    pub stages: Vec<FunnelStage>,
    pub created_at: u64,
    pub last_updated: u64,
}

/// Stage-level metrics.
#[contracttype]
#[derive(Clone)]
pub struct StageMetrics {
    pub funnel_name: Symbol,
    pub stage_num: u32,
    pub entries: u64,              // Players entering this stage
    pub completions: u64,          // Players completing this stage
    pub conversion_rate_bps: u32,  // basis points
    pub avg_time_to_complete: u64, // seconds
    pub last_updated: u64,
}

/// Player's progression through a funnel.
#[contracttype]
#[derive(Clone)]
pub struct PlayerFunnelProgress {
    pub player: Address,
    pub funnel_name: Symbol,
    pub current_stage: u32,
    pub stages_completed: u32,
    pub total_stages: u32,
    pub conversion_rate: u32, // total completions / entries (basis points)
    pub started_at: u64,
    pub last_stage_completion: u64,
}

/// Conversion event in the funnel.
#[contracttype]
#[derive(Clone)]
pub struct ConversionEvent {
    pub event_type: Symbol,  // "enter", "complete", "abandon"
    pub player: Address,
    pub funnel_name: Symbol,
    pub stage: u32,
    pub timestamp: u64,
}

/// Dropoff analysis for a stage.
#[contracttype]
#[derive(Clone)]
pub struct DropoffAnalysis {
    pub funnel_name: Symbol,
    pub stage_num: u32,
    pub dropoff_rate_bps: u32,      // percentage of users who drop off
    pub users_dropped: u64,
    pub avg_time_before_dropoff: u64, // seconds
    pub primary_cause: Symbol,      // "timeout", "friction", "cost", "unknown"
    pub recommended_action: Symbol,
    pub last_analyzed: u64,
}

/// Overall funnel health status.
#[contracttype]
#[derive(Clone, Copy, Debug)]
pub enum FunnelHealth {
    /// All stages performing well.
    Excellent = 1,
    /// Most stages healthy, some concerns.
    Good = 2,
    /// Some stages underperforming.
    Fair = 3,
    /// Major issues in funnel performance.
    Poor = 4,
}

/// Funnel performance summary.
#[contracttype]
#[derive(Clone)]
pub struct FunnelPerformanceSummary {
    pub funnel_name: Symbol,
    pub total_entries: u64,
    pub total_completions: u64,
    pub overall_conversion_rate: u32, // basis points
    pub health_status: FunnelHealth,
    pub bottleneck_stage: u32,
    pub estimated_optimizable_conversions: u64,
    pub last_calculated: u64,
}

// ── Write Helpers ─────────────────────────────────────────────────────────────

/// Create a new conversion funnel.
pub fn create_funnel(
    env: &Env,
    admin: &Address,
    funnel_name: Symbol,
    description: Symbol,
    stage_names: Vec<Symbol>,
) {
    admin.require_auth();

    let mut stages: Vec<FunnelStage> = Vec::new(env);

    for i in 0..stage_names.len() {
        if let Some(name) = stage_names.get(i as u32) {
            let stage = FunnelStage {
                stage_num: (i as u32) + 1,
                name,
                description: symbol_short!("stage"),
                expected_conversion_bps: 7000, // 70% default
                alert_threshold_bps: 5000,     // 50% alert threshold
            };
            stages.push_back(stage);
        }
    }

    let funnel = ConversionFunnel {
        funnel_name: funnel_name.clone(),
        description,
        stages,
        created_at: env.ledger().timestamp(),
        last_updated: env.ledger().timestamp(),
    };

    env.storage()
        .persistent()
        .set(&FunnelKey::FunnelConfig(funnel_name.clone()), &funnel);

    env.events().publish(
        (symbol_short!("funnel"), symbol_short!("create")),
        (funnel_name,),
    );
}

/// Record player entering a funnel stage.
pub fn record_stage_entry(env: &Env, player: &Address, funnel_name: Symbol, stage: u32) {
    let mut progress: PlayerFunnelProgress = env
        .storage()
        .persistent()
        .get(&FunnelKey::PlayerFunnelProgress(player.clone(), funnel_name.clone()))
        .unwrap_or(PlayerFunnelProgress {
            player: player.clone(),
            funnel_name: funnel_name.clone(),
            current_stage: 0,
            stages_completed: 0,
            total_stages: 0,
            conversion_rate: 0,
            started_at: 0,
            last_stage_completion: 0,
        });

    if progress.started_at == 0 {
        progress.started_at = env.ledger().timestamp();
    }

    progress.current_stage = stage;

    // Get funnel config to know total stages
    let funnel: ConversionFunnel = env
        .storage()
        .persistent()
        .get(&FunnelKey::FunnelConfig(funnel_name.clone()))
        .unwrap_or(ConversionFunnel {
            funnel_name: funnel_name.clone(),
            description: symbol_short!("unknown"),
            stages: Vec::new(env),
            created_at: 0,
            last_updated: 0,
        });

    progress.total_stages = funnel.stages.len() as u32;

    env.storage()
        .persistent()
        .set(
            &FunnelKey::PlayerFunnelProgress(player.clone(), funnel_name.clone()),
            &progress,
        );

    // Record entry in metrics
    update_stage_metrics(env, &funnel_name, stage, true, false);

    // Log conversion event
    log_conversion_event(env, symbol_short!("enter"), player, &funnel_name, stage);
}

/// Record player completing a funnel stage.
pub fn record_stage_completion(env: &Env, player: &Address, funnel_name: Symbol, stage: u32) {
    let mut progress: PlayerFunnelProgress = env
        .storage()
        .persistent()
        .get(&FunnelKey::PlayerFunnelProgress(player.clone(), funnel_name.clone()))
        .unwrap_or(PlayerFunnelProgress {
            player: player.clone(),
            funnel_name: funnel_name.clone(),
            current_stage: stage,
            stages_completed: 0,
            total_stages: 0,
            conversion_rate: 0,
            started_at: env.ledger().timestamp(),
            last_stage_completion: 0,
        });

    progress.stages_completed = progress.stages_completed.saturating_add(1);
    progress.last_stage_completion = env.ledger().timestamp();

    // Calculate conversion rate
    if progress.started_at > 0 {
        progress.conversion_rate =
            ((progress.stages_completed as u64 * 10000) / progress.total_stages.max(1) as u64) as u32;
    }

    env.storage()
        .persistent()
        .set(
            &FunnelKey::PlayerFunnelProgress(player.clone(), funnel_name.clone()),
            &progress,
        );

    // Record completion in metrics
    update_stage_metrics(env, &funnel_name, stage, false, true);

    // Log conversion event
    log_conversion_event(env, symbol_short!("complete"), player, &funnel_name, stage);

    env.events().publish(
        (symbol_short!("funnel"), symbol_short!("complete")),
        (player.clone(), funnel_name, stage),
    );
}

/// Record player abandoning a funnel.
pub fn record_funnel_abandonment(env: &Env, player: &Address, funnel_name: Symbol, stage: u32) {
    log_conversion_event(env, symbol_short!("abandon"), player, &funnel_name, stage);

    env.events().publish(
        (symbol_short!("funnel"), symbol_short!("abandon")),
        (player.clone(), funnel_name, stage),
    );
}

/// Update stage metrics.
fn update_stage_metrics(env: &Env, funnel_name: &Symbol, stage: u32, is_entry: bool, is_completion: bool) {
    let key = FunnelKey::FunnelMetrics(funnel_name.clone());
    let mut metrics: StageMetrics = env
        .storage()
        .persistent()
        .get(&key)
        .unwrap_or(StageMetrics {
            funnel_name: funnel_name.clone(),
            stage_num: stage,
            entries: 0,
            completions: 0,
            conversion_rate_bps: 0,
            avg_time_to_complete: 0,
            last_updated: 0,
        });

    if is_entry {
        metrics.entries = metrics.entries.saturating_add(1);
    }

    if is_completion {
        metrics.completions = metrics.completions.saturating_add(1);
    }

    // Calculate conversion rate
    metrics.conversion_rate_bps = if metrics.entries > 0 {
        ((metrics.completions as u64 * 10000) / metrics.entries as u64) as u32
    } else {
        0
    };

    metrics.last_updated = env.ledger().timestamp();

    env.storage()
        .persistent()
        .set(&key, &metrics);
}

/// Log a conversion event.
fn log_conversion_event(
    env: &Env,
    event_type: Symbol,
    player: &Address,
    funnel_name: &Symbol,
    stage: u32,
) {
    let event = ConversionEvent {
        event_type,
        player: player.clone(),
        funnel_name: funnel_name.clone(),
        stage,
        timestamp: env.ledger().timestamp(),
    };

    let mut log: Vec<ConversionEvent> = env
        .storage()
        .persistent()
        .get(&FunnelKey::ConversionLog)
        .unwrap_or_else(|| Vec::new(env));

    // Keep last 1000 events
    if log.len() >= 1000 {
        let mut new_log: Vec<ConversionEvent> = Vec::new(env);
        for i in 1..log.len() {
            if let Some(e) = log.get(i as u32) {
                new_log.push_back(e.clone());
            }
        }
        log = new_log;
    }

    log.push_back(event);
    env.storage()
        .persistent()
        .set(&FunnelKey::ConversionLog, &log);
}

/// Analyze funnel performance and recommend optimizations.
pub fn analyze_funnel_performance(env: &Env, admin: &Address, funnel_name: Symbol) {
    admin.require_auth();

    let funnel: ConversionFunnel = match env
        .storage()
        .persistent()
        .get(&FunnelKey::FunnelConfig(funnel_name.clone()))
    {
        Some(f) => f,
        None => return,
    };

    let metrics: StageMetrics = env
        .storage()
        .persistent()
        .get(&FunnelKey::FunnelMetrics(funnel_name.clone()))
        .unwrap_or(StageMetrics {
            funnel_name: funnel_name.clone(),
            stage_num: 0,
            entries: 0,
            completions: 0,
            conversion_rate_bps: 0,
            avg_time_to_complete: 0,
            last_updated: 0,
        });

    // Find bottleneck (lowest conversion stage)
    let mut bottleneck_stage = 1;
    let mut lowest_conversion = 10000u32;

    for i in 0..funnel.stages.len() {
        if let Some(stage) = funnel.stages.get(i as u32) {
            if metrics.conversion_rate_bps < lowest_conversion {
                lowest_conversion = metrics.conversion_rate_bps;
                bottleneck_stage = stage.stage_num;
            }
        }
    }

    // Calculate dropoff analysis
    let dropoff_rate = if metrics.entries > 0 {
        ((metrics.entries - metrics.completions) as u64 * 10000) / metrics.entries as u64
    } else {
        0
    } as u32;

    let analysis = DropoffAnalysis {
        funnel_name: funnel_name.clone(),
        stage_num: bottleneck_stage,
        dropoff_rate_bps: dropoff_rate,
        users_dropped: metrics.entries.saturating_sub(metrics.completions),
        avg_time_before_dropoff: metrics.avg_time_to_complete,
        primary_cause: if dropoff_rate > 5000 {
            symbol_short!("friction")
        } else if metrics.avg_time_to_complete > 3600 {
            symbol_short!("timeout")
        } else {
            symbol_short!("unknown")
        },
        recommended_action: if dropoff_rate > 5000 {
            symbol_short!("simplify")
        } else if metrics.avg_time_to_complete > 3600 {
            symbol_short!("speed_up")
        } else {
            symbol_short!("analyze")
        },
        last_analyzed: env.ledger().timestamp(),
    };

    env.storage()
        .persistent()
        .set(&FunnelKey::DropoffAnalysis(funnel_name.clone()), &analysis);

    env.events().publish(
        (symbol_short!("funnel"), symbol_short!("analyze")),
        (funnel_name, bottleneck_stage, dropoff_rate),
    );
}

// ── Read-Only View Functions ──────────────────────────────────────────────────

/// Get a funnel configuration.
pub fn get_funnel(env: &Env, funnel_name: Symbol) -> ConversionFunnel {
    env.storage()
        .persistent()
        .get(&FunnelKey::FunnelConfig(funnel_name.clone()))
        .unwrap_or(ConversionFunnel {
            funnel_name,
            description: symbol_short!("unknown"),
            stages: Vec::new(env),
            created_at: 0,
            last_updated: 0,
        })
}

/// Get metrics for a specific stage.
pub fn get_stage_metrics(env: &Env, funnel_name: Symbol) -> StageMetrics {
    env.storage()
        .persistent()
        .get(&FunnelKey::FunnelMetrics(funnel_name.clone()))
        .unwrap_or(StageMetrics {
            funnel_name,
            stage_num: 0,
            entries: 0,
            completions: 0,
            conversion_rate_bps: 0,
            avg_time_to_complete: 0,
            last_updated: 0,
        })
}

/// Get player's progress through a funnel.
pub fn get_player_progress(env: &Env, player: &Address, funnel_name: Symbol) -> PlayerFunnelProgress {
    env.storage()
        .persistent()
        .get(&FunnelKey::PlayerFunnelProgress(player.clone(), funnel_name.clone()))
        .unwrap_or(PlayerFunnelProgress {
            player: player.clone(),
            funnel_name,
            current_stage: 0,
            stages_completed: 0,
            total_stages: 0,
            conversion_rate: 0,
            started_at: 0,
            last_stage_completion: 0,
        })
}

/// Get dropoff analysis for a funnel.
pub fn get_dropoff_analysis(env: &Env, funnel_name: Symbol) -> DropoffAnalysis {
    env.storage()
        .persistent()
        .get(&FunnelKey::DropoffAnalysis(funnel_name.clone()))
        .unwrap_or(DropoffAnalysis {
            funnel_name,
            stage_num: 0,
            dropoff_rate_bps: 0,
            users_dropped: 0,
            avg_time_before_dropoff: 0,
            primary_cause: symbol_short!("unknown"),
            recommended_action: symbol_short!("analyze"),
            last_analyzed: 0,
        })
}

/// Get recent conversion events (limited to last 20).
pub fn get_conversion_events(env: &Env, limit: u32) -> Vec<ConversionEvent> {
    let all_events: Vec<ConversionEvent> = env
        .storage()
        .persistent()
        .get(&FunnelKey::ConversionLog)
        .unwrap_or_else(|| Vec::new(env));

    let mut result: Vec<ConversionEvent> = Vec::new(env);
    let start_idx = if (limit as usize) < all_events.len() {
        all_events.len() - (limit as usize)
    } else {
        0
    };

    for i in start_idx..all_events.len() {
        if let Some(event) = all_events.get(i as u32) {
            result.push_back(event.clone());
        }
    }

    result
}

/// Calculate overall funnel performance summary.
pub fn get_funnel_performance(env: &Env, funnel_name: Symbol) -> FunnelPerformanceSummary {
    let metrics = get_stage_metrics(env, funnel_name.clone());

    let health_status = if metrics.conversion_rate_bps > 7000 {
        FunnelHealth::Excellent
    } else if metrics.conversion_rate_bps > 5000 {
        FunnelHealth::Good
    } else if metrics.conversion_rate_bps > 3000 {
        FunnelHealth::Fair
    } else {
        FunnelHealth::Poor
    };

    // Estimate optimizable conversions (users who dropped off)
    let optimizable = metrics.entries.saturating_sub(metrics.completions);

    FunnelPerformanceSummary {
        funnel_name,
        total_entries: metrics.entries,
        total_completions: metrics.completions,
        overall_conversion_rate: metrics.conversion_rate_bps,
        health_status,
        bottleneck_stage: metrics.stage_num,
        estimated_optimizable_conversions: optimizable,
        last_calculated: env.ledger().timestamp(),
    }
}
